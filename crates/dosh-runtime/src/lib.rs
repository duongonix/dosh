mod commands;
mod external;
mod io;
mod model;
mod pipeline;
mod state;
pub use state::RuntimeState;

use anyhow::Result;
use dosh_ast::{BinaryOp, CellPathSegment, Expression, Script, Statement, TypeExpr, UnaryOp};
use dosh_builtins::{BuiltinOutcome, BuiltinRegistry};
use dosh_config::DoshPaths;
use dosh_env::EnvContext;
use dosh_plugin::{PermissionPolicy, TrustPolicy, TrustStore};
use dosh_wasm::WasmPluginRuntime;
use state::{FunctionDef, evaluate_truthy, pattern_matches};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControlFlow {
    None,
    Return(Option<Expression>),
    Break,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeOutcome {
    pub exit_code: i32,
    pub should_exit: bool,
    pub output: Option<String>,
    flow: ControlFlow,
}

#[derive(Debug, Clone)]
pub struct ScriptTestCaseResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptTestReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub cases: Vec<ScriptTestCaseResult>,
}

impl RuntimeOutcome {
    fn from_builtin(out: BuiltinOutcome) -> Self {
        let output = io::pipeline_data_to_text(out.output);
        Self {
            exit_code: out.exit_code,
            should_exit: out.should_exit,
            output,
            flow: ControlFlow::None,
        }
    }

    pub fn ok() -> Self {
        Self {
            exit_code: 0,
            should_exit: false,
            output: None,
            flow: ControlFlow::None,
        }
    }
}

pub struct Runtime {
    builtins: BuiltinRegistry,
    wasm_plugins: Mutex<WasmPluginRuntime>,
}

impl Runtime {
    pub fn new() -> Self {
        let mut wasm_plugins = WasmPluginRuntime::new(PermissionPolicy::allow_all());
        if let Ok(paths) = DoshPaths::detect() {
            let plugins_dir = paths.plugins_dir();
            let keyring_path = plugins_dir.join("trusted-keys.toml");
            let trust_store_path = plugins_dir.join("trust.toml");
            let grants_path = plugins_dir.join("permission-grants.toml");
            if keyring_path.exists() {
                if let Ok(policy) = TrustPolicy::from_keyring_file(&keyring_path) {
                    let trust_store = TrustStore::load(&trust_store_path).unwrap_or_default();
                    wasm_plugins = wasm_plugins.with_trust_policy(policy.with_store(trust_store));
                }
            }
            wasm_plugins = wasm_plugins.with_plugin_root(plugins_dir);
            wasm_plugins = wasm_plugins.with_permission_grants_file(grants_path);
            let _ = wasm_plugins.load_from_filesystem();
        }
        Self {
            builtins: BuiltinRegistry::new(),
            wasm_plugins: Mutex::new(wasm_plugins),
        }
    }

    pub fn execute(&self, script: &Script, env: &mut EnvContext) -> Result<RuntimeOutcome> {
        let mut state = RuntimeState::new();
        self.seed_global_vars(env, &mut state)?;
        let out = self.execute_with_state(script, env, &mut state)?;
        Ok(out)
    }

    pub fn new_state(&self, env: &EnvContext) -> Result<RuntimeState> {
        let mut state = RuntimeState::new();
        self.seed_global_vars(env, &mut state)?;
        Ok(state)
    }

    pub fn execute_with_state(
        &self,
        script: &Script,
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<RuntimeOutcome> {
        let out = self.execute_statements(&script.statements, env, state)?;
        match out.flow {
            ControlFlow::Return(_) => anyhow::bail!("Return outside function"),
            ControlFlow::Break => anyhow::bail!("Break outside loop"),
            ControlFlow::Continue => anyhow::bail!("Continue outside loop"),
            ControlFlow::None => {}
        }
        Ok(out)
    }

    pub fn execute_tests(&self, script: &Script, env: &mut EnvContext) -> Result<ScriptTestReport> {
        let mut state = RuntimeState::new();
        self.seed_global_vars(env, &mut state)?;
        let mut report = ScriptTestReport::default();

        let mut test_blocks: Vec<(String, Vec<Statement>)> = Vec::new();
        let mut prelude: Vec<Statement> = Vec::new();
        for stmt in &script.statements {
            match stmt {
                Statement::Test { name, body } => test_blocks.push((name.clone(), body.clone())),
                _ => prelude.push(stmt.clone()),
            }
        }

        self.execute_statements(&prelude, env, &mut state)?;

        for (name, body) in test_blocks {
            report.total += 1;
            state.push_scope();
            let result = self.execute_statements(&body, env, &mut state);
            state.pop_scope();
            match result {
                Ok(out) if matches!(out.flow, ControlFlow::None) => {
                    report.passed += 1;
                    report.cases.push(ScriptTestCaseResult {
                        name,
                        passed: true,
                        error: None,
                    });
                }
                Ok(out) => {
                    report.failed += 1;
                    report.cases.push(ScriptTestCaseResult {
                        name,
                        passed: false,
                        error: Some(format!("invalid control flow in test: {:?}", out.flow)),
                    });
                }
                Err(err) => {
                    report.failed += 1;
                    report.cases.push(ScriptTestCaseResult {
                        name,
                        passed: false,
                        error: Some(err.to_string()),
                    });
                }
            }
        }
        Ok(report)
    }

    fn seed_global_vars(&self, env: &EnvContext, state: &mut RuntimeState) -> Result<()> {
        let home = detect_home_dir().unwrap_or_else(|| env.cwd().to_path_buf());
        state.assign_var(
            "HOME".to_string(),
            &[],
            Expression::StringLiteral(home.display().to_string()),
            true,
        )?;

        let dosh_root = DoshPaths::detect()
            .map(|p| p.shared_root().to_path_buf())
            .unwrap_or_else(|_| env.cwd().to_path_buf());
        state.assign_var(
            "DOSH".to_string(),
            &[],
            Expression::StringLiteral(dosh_root.display().to_string()),
            true,
        )?;
        Ok(())
    }

    fn execute_statements(
        &self,
        statements: &[Statement],
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<RuntimeOutcome> {
        let mut last = RuntimeOutcome {
            exit_code: 0,
            should_exit: false,
            output: None,
            flow: ControlFlow::None,
        };

        for stmt in statements {
            last = self.execute_statement(stmt, env, state)?;
            if last.should_exit {
                break;
            }
            if !matches!(last.flow, ControlFlow::None) {
                break;
            }
        }

        Ok(last)
    }

    fn execute_statement(
        &self,
        stmt: &Statement,
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<RuntimeOutcome> {
        match stmt {
            Statement::Command(cmd) => self.execute_command(cmd, env, state),
            Statement::Pipeline(pipeline) => self.execute_pipeline(pipeline, env, state),
            Statement::Assignment(assign) => {
                let resolved = self.evaluate_expression(&assign.value, env, state)?;
                state.assign_var(
                    assign.name.clone(),
                    &assign.cell_path,
                    resolved,
                    assign.is_constant,
                )?;
                Ok(ok_outcome())
            }
            Statement::Let { name, ty, value } => {
                let mut resolved = self.evaluate_expression(value, env, state)?;
                if let Expression::Lambda { params, body } = resolved {
                    resolved = Expression::Closure {
                        params,
                        body,
                        captures: state.visible_vars(),
                    };
                }
                if let Some(ty) = ty {
                    self.ensure_type(&resolved, ty)?;
                }
                state.assign_var(name.clone(), &[], resolved, false)?;
                Ok(ok_outcome())
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_expr = self.evaluate_expression(condition, env, state)?;
                let cond = evaluate_truthy(&cond_expr, state);
                state.push_scope();
                let result = if cond {
                    self.execute_statements(then_branch, env, state)
                } else {
                    self.execute_statements(else_branch, env, state)
                };
                state.pop_scope();
                result
            }
            Statement::For {
                variable,
                iterable,
                body,
            } => {
                let iter_value = self.evaluate_expression(iterable, env, state)?;
                let items = self.evaluate_iterable(&iter_value, state);
                let mut last = ok_outcome();

                for item in items {
                    state.push_scope();
                    state.set_var(variable.clone(), item);
                    last = self.execute_statements(body, env, state)?;
                    state.pop_scope();
                    if last.should_exit {
                        break;
                    }
                    match last.flow {
                        ControlFlow::Break => {
                            last.flow = ControlFlow::None;
                            break;
                        }
                        ControlFlow::Continue => {
                            last.flow = ControlFlow::None;
                            continue;
                        }
                        ControlFlow::Return(_) => break,
                        ControlFlow::None => {}
                    }
                }

                Ok(last)
            }
            Statement::Match { expression, arms } => {
                let value = self.evaluate_expression(expression, env, state)?;
                for (pattern, body) in arms {
                    if pattern_matches(pattern, &value, state) {
                        state.push_scope();
                        let result = self.execute_statements(body, env, state);
                        state.pop_scope();
                        return result;
                    }
                }

                Ok(ok_outcome())
            }
            Statement::Function {
                name,
                params,
                return_type,
                is_exported: _,
                body,
            } => {
                state.functions.insert(
                    name.clone(),
                    FunctionDef {
                        params: params.clone(),
                        return_type: return_type.clone(),
                        body: body.clone(),
                        captures: state.visible_vars(),
                    },
                );
                Ok(ok_outcome())
            }
            Statement::Module { name, body } => {
                state.modules.insert(name.clone(), body.clone());
                Ok(ok_outcome())
            }
            Statement::Import { module, alias } => self.execute_import(module, alias, env, state),
            Statement::Test { .. } => Ok(ok_outcome()),
            Statement::Return(value) => Ok(RuntimeOutcome {
                exit_code: 0,
                should_exit: false,
                output: None,
                flow: ControlFlow::Return(match value {
                    Some(expr) => Some(self.evaluate_expression(expr, env, state)?),
                    None => None,
                }),
            }),
            Statement::Break => Ok(RuntimeOutcome {
                exit_code: 0,
                should_exit: false,
                output: None,
                flow: ControlFlow::Break,
            }),
            Statement::Continue => Ok(RuntimeOutcome {
                exit_code: 0,
                should_exit: false,
                output: None,
                flow: ControlFlow::Continue,
            }),
            Statement::Expr(expr) => self.execute_expression_statement(expr, env, state),
        }
    }

    fn execute_expression_statement(
        &self,
        expr: &Expression,
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<RuntimeOutcome> {
        match expr {
            Expression::Call { name, args } => {
                let _ = self.invoke_function(name, args, env, state)?;
                Ok(ok_outcome())
            }
            _ => {
                let _ = self.evaluate_expression(expr, env, state)?;
                Ok(ok_outcome())
            }
        }
    }

    fn evaluate_expression(
        &self,
        expr: &Expression,
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<Expression> {
        match expr {
            Expression::Variable { name, cell_path } => {
                let value = state
                    .get_var(name)
                    .ok_or_else(|| anyhow::anyhow!("Variable not found: ${name}"))?;
                Ok(resolve_cell_path(value, cell_path)?)
            }
            Expression::Binary { left, op, right } => {
                let left = self.evaluate_expression(left, env, state)?;
                let right = self.evaluate_expression(right, env, state)?;
                eval_binary(op, left, right)
            }
            Expression::Unary { op, expr } => {
                let value = self.evaluate_expression(expr, env, state)?;
                eval_unary(op, value)
            }
            Expression::Pipeline(p) => {
                let out = self.execute_pipeline(p, env, state)?;
                Ok(out
                    .output
                    .map(Expression::StringLiteral)
                    .unwrap_or(Expression::Null))
            }
            Expression::List(items) => {
                let mut out = Vec::new();
                for item in items {
                    out.push(self.evaluate_expression(item, env, state)?);
                }
                Ok(Expression::List(out))
            }
            Expression::Record(fields) => {
                let mut out = Vec::new();
                for (k, v) in fields {
                    out.push((k.clone(), self.evaluate_expression(v, env, state)?));
                }
                Ok(Expression::Record(out))
            }
            Expression::Call { name, args } => self.invoke_function(name, args, env, state),
            _ => Ok(self.resolve_expression(expr, state)),
        }
    }

    fn invoke_function(
        &self,
        name: &str,
        args: &[Expression],
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<Expression> {
        let function = if let Some(function) = state.functions.get(name).cloned() {
            function
        } else if let Some(Expression::Closure {
            params,
            body,
            captures,
        }) = state.get_var(name)
        {
            FunctionDef {
                params,
                return_type: None,
                body,
                captures,
            }
        } else {
            anyhow::bail!("Function not found: {name}");
        };

        if function.params.len() != args.len() {
            anyhow::bail!(
                "Wrong argument count for `{name}`: expected {}, got {}",
                function.params.len(),
                args.len()
            );
        }

        state.push_scope();
        for (param, arg) in function.params.iter().zip(args.iter()) {
            let value = self.evaluate_expression(arg, env, state)?;
            if let Some(ty) = &param.ty {
                self.ensure_type(&value, ty)?;
            }
            state.set_var(param.name.clone(), value);
        }
        for (cap, value) in function.captures {
            if state.get_var(&cap).is_none() {
                state.set_var(cap, value);
            }
        }

        let result = self.execute_statements(&function.body, env, state);
        state.pop_scope();
        let result = result?;
        if matches!(result.flow, ControlFlow::Break) {
            anyhow::bail!("Break outside loop");
        }
        if matches!(result.flow, ControlFlow::Continue) {
            anyhow::bail!("Continue outside loop");
        }
        let return_expr = match result.flow {
            ControlFlow::Return(v) => v.unwrap_or(Expression::Null),
            _ => Expression::Null,
        };
        if let Some(ret_ty) = &function.return_type {
            self.ensure_type(&return_expr, ret_ty)?;
        }
        Ok(return_expr)
    }

    fn resolve_expression(&self, expr: &Expression, state: &RuntimeState) -> Expression {
        match expr {
            Expression::Identifier(name) => state.get_var(name).unwrap_or_else(|| expr.clone()),
            Expression::Variable { name, .. } => state
                .get_var(name)
                .unwrap_or_else(|| Expression::Identifier(name.clone())),
            Expression::MemberAccess { object, field } => {
                let base = self.resolve_expression(object, state);
                match base {
                    Expression::Identifier(name) => state
                        .modules
                        .get(&name)
                        .and_then(|body| extract_module_field(body, field))
                        .unwrap_or(Expression::Identifier(format!("{name}.{field}"))),
                    _ => Expression::Identifier(field.clone()),
                }
            }
            Expression::Call { name, args } => Expression::Call {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| self.resolve_expression(arg, state))
                    .collect(),
            },
            Expression::List(items) => Expression::List(
                items
                    .iter()
                    .map(|item| self.resolve_expression(item, state))
                    .collect(),
            ),
            Expression::Record(fields) => Expression::Record(fields.clone()),
            Expression::Closure {
                params,
                body,
                captures,
            } => Expression::Closure {
                params: params.clone(),
                body: body.clone(),
                captures: captures.clone(),
            },
            _ => expr.clone(),
        }
    }

    fn evaluate_iterable(&self, expr: &Expression, state: &RuntimeState) -> Vec<Expression> {
        let value = self.resolve_expression(expr, state);
        match value {
            Expression::Null => Vec::new(),
            Expression::StringLiteral(v) => v
                .split_whitespace()
                .map(|part| Expression::StringLiteral(part.to_string()))
                .collect(),
            Expression::Integer(v) if v >= 0 => (0..v).map(Expression::Integer).collect(),
            Expression::Integer(_) => Vec::new(),
            Expression::Identifier(v) => vec![Expression::Identifier(v)],
            Expression::Bool(v) => vec![Expression::Bool(v)],
            Expression::List(items) => items,
            Expression::Record(_) => Vec::new(),
            Expression::Range { start, end } => (start..end).map(Expression::Integer).collect(),
            Expression::Lambda { .. } => Vec::new(),
            Expression::Closure { .. } => Vec::new(),
            Expression::MemberAccess { .. } => Vec::new(),
            Expression::Call { .. } => Vec::new(),
            Expression::Variable { .. } => Vec::new(),
            _ => Vec::new(),
        }
    }

    fn ensure_type(&self, value: &Expression, expected: &TypeExpr) -> Result<()> {
        match expected {
            TypeExpr::Any => Ok(()),
            TypeExpr::Int => match value {
                Expression::Integer(_) => Ok(()),
                _ => anyhow::bail!("type mismatch: expected int"),
            },
            TypeExpr::Bool => match value {
                Expression::Bool(_) => Ok(()),
                _ => anyhow::bail!("type mismatch: expected bool"),
            },
            TypeExpr::String => match value {
                Expression::StringLiteral(_) => Ok(()),
                _ => anyhow::bail!("type mismatch: expected string"),
            },
            TypeExpr::List(inner) | TypeExpr::Iterator(inner) => match value {
                Expression::List(items) => {
                    for item in items {
                        self.ensure_type(item, inner)?;
                    }
                    Ok(())
                }
                Expression::Range { .. } if matches!(**inner, TypeExpr::Int) => Ok(()),
                _ => anyhow::bail!("type mismatch: expected list/iterator"),
            },
            TypeExpr::Function { .. } => match value {
                Expression::Lambda { .. } | Expression::Closure { .. } => Ok(()),
                _ => anyhow::bail!("type mismatch: expected function"),
            },
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn ok_outcome() -> RuntimeOutcome {
    RuntimeOutcome {
        exit_code: 0,
        should_exit: false,
        output: None,
        flow: ControlFlow::None,
    }
}

fn extract_module_field(body: &[Statement], field: &str) -> Option<Expression> {
    for stmt in body {
        if let Statement::Let { name, value, .. } = stmt
            && name == field
        {
            return Some(value.clone());
        }
        if let Statement::Assignment(assign) = stmt
            && assign.name == field
        {
            return Some(assign.value.clone());
        }
    }
    None
}

fn resolve_cell_path(value: Expression, path: &[CellPathSegment]) -> Result<Expression> {
    let mut cur = value;
    for seg in path {
        cur = match (cur, seg) {
            (Expression::Record(fields), CellPathSegment::Field(name)) => fields
                .into_iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v)
                .ok_or_else(|| anyhow::anyhow!("Invalid cell path: missing field `{name}`"))?,
            (Expression::List(items), CellPathSegment::Index(i)) => items
                .into_iter()
                .nth(*i)
                .ok_or_else(|| anyhow::anyhow!("Invalid cell path: index out of bounds"))?,
            _ => anyhow::bail!("Invalid cell path"),
        };
    }
    Ok(cur)
}

fn eval_unary(op: &UnaryOp, value: Expression) -> Result<Expression> {
    match op {
        UnaryOp::Not => Ok(Expression::Bool(!matches!(value, Expression::Bool(true)))),
        UnaryOp::Neg => match value {
            Expression::Integer(v) => Ok(Expression::Integer(-v)),
            _ => anyhow::bail!("Type mismatch for unary '-'"),
        },
    }
}

fn eval_binary(op: &BinaryOp, left: Expression, right: Expression) -> Result<Expression> {
    match op {
        BinaryOp::Add => match (left, right) {
            (Expression::Integer(a), Expression::Integer(b)) => Ok(Expression::Integer(a + b)),
            (Expression::StringLiteral(a), Expression::StringLiteral(b)) => {
                Ok(Expression::StringLiteral(format!("{a}{b}")))
            }
            (a, b) => Ok(Expression::StringLiteral(format!("{a:?}{b:?}"))),
        },
        BinaryOp::Sub => match (left, right) {
            (Expression::Integer(a), Expression::Integer(b)) => Ok(Expression::Integer(a - b)),
            _ => anyhow::bail!("Type mismatch for '-'"),
        },
        BinaryOp::Mul => match (left, right) {
            (Expression::Integer(a), Expression::Integer(b)) => Ok(Expression::Integer(a * b)),
            _ => anyhow::bail!("Type mismatch for '*'"),
        },
        BinaryOp::Div => match (left, right) {
            (Expression::Integer(a), Expression::Integer(b)) => Ok(Expression::Integer(a / b)),
            _ => anyhow::bail!("Type mismatch for '/'"),
        },
        BinaryOp::Eq => Ok(Expression::Bool(left == right)),
        BinaryOp::Ne => Ok(Expression::Bool(left != right)),
        BinaryOp::Gt => compare_int(left, right, |a, b| a > b),
        BinaryOp::Gte => compare_int(left, right, |a, b| a >= b),
        BinaryOp::Lt => compare_int(left, right, |a, b| a < b),
        BinaryOp::Lte => compare_int(left, right, |a, b| a <= b),
        BinaryOp::And => Ok(Expression::Bool(
            matches!(left, Expression::Bool(true)) && matches!(right, Expression::Bool(true)),
        )),
        BinaryOp::Or => Ok(Expression::Bool(
            matches!(left, Expression::Bool(true)) || matches!(right, Expression::Bool(true)),
        )),
    }
}

fn compare_int(left: Expression, right: Expression, f: fn(i64, i64) -> bool) -> Result<Expression> {
    if let (Expression::Integer(a), Expression::Integer(b)) = (left, right) {
        Ok(Expression::Bool(f(a, b)))
    } else {
        anyhow::bail!("Type mismatch for comparison")
    }
}

fn detect_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dosh_builtins::PipelineData;
    use dosh_parser::Parser;

    #[test]
    fn runtime_executes_function_for_match_module_flow() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Script {
            statements: vec![
                Statement::Function {
                    name: "ping".to_string(),
                    params: vec![dosh_ast::Param {
                        name: "x".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    is_exported: false,
                    body: vec![Statement::If {
                        condition: Expression::Identifier("x".to_string()),
                        then_branch: vec![Statement::Let {
                            name: "ok".to_string(),
                            ty: None,
                            value: Expression::Bool(true),
                        }],
                        else_branch: vec![],
                    }],
                },
                Statement::Module {
                    name: "util".to_string(),
                    body: vec![Statement::Let {
                        name: "seed".to_string(),
                        ty: None,
                        value: Expression::Integer(1),
                    }],
                },
                Statement::For {
                    variable: "item".to_string(),
                    iterable: Expression::StringLiteral("a b".to_string()),
                    body: vec![Statement::Expr(Expression::Call {
                        name: "ping".to_string(),
                        args: vec![Expression::Bool(true)],
                    })],
                },
                Statement::Match {
                    expression: Expression::Integer(2),
                    arms: vec![
                        (
                            Expression::Integer(1),
                            vec![Statement::Let {
                                name: "m".to_string(),
                                ty: None,
                                value: Expression::Integer(1),
                            }],
                        ),
                        (
                            Expression::Identifier("_".to_string()),
                            vec![Statement::Let {
                                name: "m".to_string(),
                                ty: None,
                                value: Expression::Integer(2),
                            }],
                        ),
                    ],
                },
            ],
        };

        let outcome = runtime.execute(&script, &mut env).unwrap();
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn runtime_executes_structured_builtin_pipeline() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let out = runtime
            .builtins
            .run(
                "from-json",
                &[],
                PipelineData::Text("{\"name\":\"dosh\"}".to_string()),
                &mut env,
            )
            .unwrap()
            .unwrap();
        let out = runtime
            .builtins
            .run("get", &["name".to_string()], out.output, &mut env)
            .unwrap()
            .unwrap();
        match out.output {
            PipelineData::Value(v) => assert_eq!(v.to_string(), "dosh"),
            _ => panic!("expected structured value"),
        }
    }

    #[test]
    fn runtime_table_list_record_select_renders_output() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new()
            .parse_script("[{name:\"a\", age:20},{name:\"b\", age:21}] | select name | table")
            .unwrap();
        let out = runtime.execute(&script, &mut env).unwrap();
        let text = out.output.unwrap_or_default();
        assert!(!text.trim().is_empty(), "expected rendered output");
        assert!(text.contains("name"), "expected table header");
        assert!(text.contains("a"), "expected row value");
        assert!(text.contains("b"), "expected row value");
    }

    #[test]
    fn runtime_rejects_constant_reassign() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new()
            .parse_script("$NAME = \"DoSH\"\n$NAME = \"Other\"")
            .unwrap();
        let err = runtime.execute(&script, &mut env).unwrap_err();
        assert!(
            err.to_string()
                .contains("Cannot reassign constant variable")
        );
    }

    #[test]
    fn runtime_mutable_reassign_works() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new()
            .parse_script("$count = 1\n$count = $count + 1")
            .unwrap();
        let out = runtime.execute(&script, &mut env).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn runtime_function_return_assignment_works() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new()
            .parse_script("fn add($a, $b) { return $a + $b }\n$result = add 1 2")
            .unwrap();
        let out = runtime.execute(&script, &mut env).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn runtime_nested_assignment_works() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new()
            .parse_script("$user = { name: \"donix\" }\n$user.name = \"other\"")
            .unwrap();
        let out = runtime.execute(&script, &mut env).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn runtime_return_outside_function_errors() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new().parse_script("return 1").unwrap();
        let err = runtime.execute(&script, &mut env).unwrap_err();
        assert!(err.to_string().contains("Return outside function"));
    }

    #[test]
    fn runtime_break_outside_loop_errors() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new().parse_script("break").unwrap();
        let err = runtime.execute(&script, &mut env).unwrap_err();
        assert!(err.to_string().contains("Break outside loop"));
    }

    #[test]
    fn runtime_use_export_module_works() {
        let runtime = Runtime::new();
        let tmp = std::env::temp_dir().join(format!("dosh_mod_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let mod_file = tmp.join("utils.dosh");
        std::fs::write(
            &mod_file,
            "export $NAME = \"DoSH\"\nexport fn ping($x) { return $x }",
        )
        .unwrap();

        let mut env = EnvContext::new(tmp.clone());
        let script = Parser::new()
            .parse_script("use \"./utils.dosh\" as utils\nprint $utils.NAME")
            .unwrap();
        let out = runtime.execute(&script, &mut env).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn runtime_execute_tests_reports_pass() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new()
            .parse_script(
                "fn add($a, $b) { return $a + $b }\n$result = add 1 2\ntest \"add\" { assert eq $result 3 }",
            )
            .unwrap();
        let report = runtime.execute_tests(&script, &mut env).unwrap();
        assert_eq!(report.total, 1);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn runtime_has_home_and_dosh_globals() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let script = Parser::new()
            .parse_script("print $HOME\nprint $DOSH")
            .unwrap();
        let out = runtime.execute(&script, &mut env).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn runtime_pipeline_interpolates_home_in_save_path() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().unwrap());
        let tmp = std::env::temp_dir().join(format!("dosh_home_interp_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let home = tmp.to_string_lossy().replace('\\', "/");
        let out_file = format!("{home}/interp.txt");
        let script = Parser::new()
            .parse_script(&format!(
                "$homex = \"{home}\"\n\"ok\" | save \"$homex/interp.txt\""
            ))
            .unwrap();
        let out = runtime.execute(&script, &mut env).unwrap();
        assert_eq!(out.exit_code, 0);
        assert!(std::path::Path::new(&out_file).exists());
    }

    #[test]
    fn runtime_state_persists_between_exec_calls() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().expect("cwd"));
        let mut state = runtime.new_state(&env).expect("new state");
        let s1 = Parser::new().parse_script("$a = 2").expect("parse 1");
        let s2 = Parser::new().parse_script("print $a").expect("parse 2");
        let _ = runtime
            .execute_with_state(&s1, &mut env, &mut state)
            .expect("exec 1");
        let out = runtime
            .execute_with_state(&s2, &mut env, &mut state)
            .expect("exec 2");
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn runtime_pipeline_closure_ops_do_not_strip_it_acc_vars() {
        let runtime = Runtime::new();
        let mut env = EnvContext::new(std::env::current_dir().expect("cwd"));
        let script = Parser::new()
            .parse_script(
                "[1,2,3,4] | filter { $it > 2 }\n[1,2,3] | map { $it * 2 }\n[1,2,3] | reduce 0 { $acc + $it }",
            )
            .expect("parse");
        let out = runtime.execute(&script, &mut env).expect("execute");
        assert_eq!(out.exit_code, 0);
    }
}
