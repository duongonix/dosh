use crate::state::RuntimeState;
use crate::{ControlFlow, Runtime, RuntimeOutcome};
use anyhow::{Result, anyhow};
use dosh_ast::{Command, Expression};
use dosh_builtins::PipelineData;
use dosh_env::EnvContext;
use dosh_parser::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

impl Runtime {
    pub(crate) fn execute_command(
        &self,
        cmd: &Command,
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<RuntimeOutcome> {
        let interpolated_args = cmd
            .args
            .iter()
            .map(|a| interpolate_arg(a, state))
            .collect::<Vec<_>>();
        let cmd = Command {
            name: cmd.name.clone(),
            args: interpolated_args,
            redirects: cmd.redirects.clone(),
            background: cmd.background,
            force_external: cmd.force_external,
        };

        if cmd.name == "source" {
            return self.execute_source(&cmd, env, state);
        }

        if state.functions.contains_key(&cmd.name) || state.get_var(&cmd.name).is_some() {
            let expr = Expression::Call {
                name: cmd.name.clone(),
                args: cmd.args.iter().map(|a| parse_arg_expr(a)).collect(),
            };
            return self.execute_expression_statement(&expr, env, state);
        }

        if let Some((name, args)) = self.builtins.expand_alias(&cmd.name, &cmd.args) {
            let expanded = Command {
                name,
                args,
                redirects: cmd.redirects.clone(),
                background: cmd.background,
                force_external: cmd.force_external,
            };
            return self.execute_command(&expanded, env, state);
        }

        if let Ok(mut plugins) = self.wasm_plugins.lock() {
            if let Some(resp) =
                plugins.run_command(&cmd.name, &cmd.args, Some(env.cwd().display().to_string()))?
            {
                return Ok(RuntimeOutcome {
                    exit_code: resp.exit_code,
                    should_exit: false,
                    output: resp.output,
                    flow: ControlFlow::None,
                });
            }
        }

        if cmd.background {
            return self.execute_background_command(&cmd, env, state);
        }

        if !cmd.force_external
            && let Some(out) = self
                .builtins
                .run(&cmd.name, &cmd.args, PipelineData::Empty, env)?
        {
            return Ok(RuntimeOutcome::from_builtin(out));
        }

        let resolved_name = resolve_command_name(&cmd, state)?;
        let spawn_name = crate::external::resolve_program_for_spawn(&resolved_name);
        let mut process = ProcessCommand::new(&spawn_name);
        process.args(&cmd.args).current_dir(env.cwd());
        crate::io::apply_redirects(&mut process, &cmd.redirects)?;
        let status = process.status().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                return anyhow!(command_not_found_message(&resolved_name, &self.builtins));
            }
            anyhow!(e)
        })?;

        Ok(RuntimeOutcome {
            exit_code: status.code().unwrap_or(1),
            should_exit: false,
            output: None,
            flow: ControlFlow::None,
        })
    }

    fn execute_source(
        &self,
        cmd: &Command,
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<RuntimeOutcome> {
        let input = cmd
            .args
            .first()
            .ok_or_else(|| anyhow!("source expects file path"))?;
        let path = resolve_source_path(env.cwd(), input);
        let content = fs::read_to_string(path)?;
        let script = Parser::new().parse_script(&content)?;
        self.execute_statements(&script.statements, env, state)
    }

    pub(crate) fn execute_import(
        &self,
        module: &str,
        alias: &Option<String>,
        env: &mut EnvContext,
        state: &mut RuntimeState,
    ) -> Result<RuntimeOutcome> {
        if let Some(body) = state.modules.get(module).cloned() {
            state.push_scope();
            let out = self.execute_statements(&body, env, state)?;
            let snapshot = state.visible_vars();
            state.pop_scope();
            if let Some(alias_name) = alias {
                state.set_var(
                    alias_name.clone(),
                    Expression::Record(record_from_snapshot(&snapshot)),
                );
            }
            return Ok(out);
        }

        let resolved_path = resolve_import_path(env.cwd(), module);
        let module_key = resolved_path
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();

        if state.import_stack.iter().any(|m| m == &module_key) {
            anyhow::bail!("circular import detected for `{module}`");
        }

        if state.imported_modules.contains(&module_key) {
            if let Some(alias_name) = alias
                && let Some(exports) = state.module_exports.get(&module_key)
            {
                state.set_var(
                    alias_name.clone(),
                    Expression::Record(record_from_exports(exports)),
                );
            }
            return Ok(crate::ok_outcome());
        }

        let path = resolved_path;
        if !path.exists() {
            anyhow::bail!(
                "import not found: `{module}` (resolved `{}`)",
                path.display()
            );
        }

        let content = fs::read_to_string(&path)?;
        let script = Parser::new().parse_script(&content)?;
        state
            .modules
            .insert(module_key.clone(), script.statements.clone());
        state.import_stack.push(module_key.clone());

        state.push_scope();
        let out = self.execute_statements(&script.statements, env, state)?;
        let snapshot = state.visible_vars();
        state.pop_scope();
        state.import_stack.pop();

        let mut exports = collect_exports(&script.statements, &snapshot);
        if exports.is_empty() {
            exports = snapshot;
        }
        state
            .module_exports
            .insert(module_key.clone(), exports.clone());
        state.imported_modules.insert(module_key);

        if let Some(name) = alias {
            state.set_var(
                name.clone(),
                Expression::Record(record_from_exports(&exports)),
            );
        }

        Ok(out)
    }

    pub(crate) fn execute_background_command(
        &self,
        cmd: &Command,
        env: &mut EnvContext,
        state: &RuntimeState,
    ) -> Result<RuntimeOutcome> {
        if self
            .builtins
            .run(&cmd.name, &cmd.args, PipelineData::Empty, env)?
            .is_some()
            && !cmd.force_external
        {
            return Ok(RuntimeOutcome {
                exit_code: 2,
                should_exit: false,
                output: Some("background mode for builtins is not supported".to_string()),
                flow: ControlFlow::None,
            });
        }

        let resolved_name = resolve_command_name(cmd, state)?;
        let spawn_name = crate::external::resolve_program_for_spawn(&resolved_name);
        let mut process = ProcessCommand::new(&spawn_name);
        process.args(&cmd.args).current_dir(env.cwd());
        crate::io::apply_redirects(&mut process, &cmd.redirects)?;
        let mut child = process.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                return anyhow!(command_not_found_message(&resolved_name, &self.builtins));
            }
            anyhow!(e)
        })?;
        let pid = child.id();

        std::thread::spawn(move || {
            let _ = child.wait();
        });

        Ok(RuntimeOutcome {
            exit_code: 0,
            should_exit: false,
            output: Some(format!("started background job pid={pid}")),
            flow: ControlFlow::None,
        })
    }
}

fn command_not_found_message(name: &str, builtins: &dosh_builtins::BuiltinRegistry) -> String {
    let mut candidates = builtins
        .metadata(None)
        .into_iter()
        .map(|m| m.name.to_string())
        .collect::<Vec<_>>();
    candidates.sort();
    let mut best: Option<(&str, usize)> = None;
    for c in &candidates {
        let d = edit_distance(name, c);
        if d <= 3 {
            if let Some((_, best_d)) = best {
                if d < best_d {
                    best = Some((c.as_str(), d));
                }
            } else {
                best = Some((c.as_str(), d));
            }
        }
    }
    if let Some((s, _)) = best {
        format!("program not found: {name}. did you mean `{s}`?")
    } else {
        format!("program not found: {name}")
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let mut prev = (0..=b.len()).collect::<Vec<_>>();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (cur[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn parse_arg_expr(text: &str) -> Expression {
    if let Ok(v) = text.parse::<i64>() {
        return Expression::Integer(v);
    }
    if text.eq_ignore_ascii_case("true") {
        return Expression::Bool(true);
    }
    if text.eq_ignore_ascii_case("false") {
        return Expression::Bool(false);
    }
    if text.starts_with('$') {
        return Expression::Variable {
            name: text.trim_start_matches('$').to_string(),
            cell_path: Vec::new(),
        };
    }
    Expression::StringLiteral(text.to_string())
}

fn interpolate_arg(input: &str, state: &RuntimeState) -> String {
    let mut out = String::new();
    let chars = input.chars().collect::<Vec<_>>();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '$' {
            let mut j = i + 1;
            while j < chars.len()
                && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '.')
            {
                j += 1;
            }
            if j > i + 1 {
                let name = chars[i + 1..j].iter().collect::<String>();
                if let Some(v) = resolve_variable_path(&name, state) {
                    out.push_str(&expression_to_arg_string(&v));
                } else {
                    out.push('$');
                    out.push_str(&name);
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn resolve_command_name(cmd: &Command, state: &RuntimeState) -> Result<String> {
    if !cmd.force_external {
        return Ok(cmd.name.clone());
    }
    if let Some(var_name) = cmd.name.strip_prefix('$') {
        let value = state
            .get_var(var_name)
            .ok_or_else(|| anyhow!("Variable not found: ${var_name}"))?;
        return Ok(expression_to_arg_string(&value));
    }
    Ok(interpolate_arg(&cmd.name, state))
}

fn expression_to_arg_string(value: &Expression) -> String {
    match value {
        Expression::StringLiteral(s) => s.clone(),
        Expression::Integer(v) => v.to_string(),
        Expression::Float(v) => v.clone(),
        Expression::Bool(v) => v.to_string(),
        Expression::Identifier(v) => v.clone(),
        Expression::Null => String::new(),
        Expression::List(items) => {
            let inner = items
                .iter()
                .map(expression_to_arg_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        Expression::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", expression_to_arg_string(v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
        Expression::Variable { name, .. } => format!("${name}"),
        other => format!("{other:?}"),
    }
}

fn resolve_variable_path(path: &str, state: &RuntimeState) -> Option<Expression> {
    let mut parts = path.split('.');
    let root = parts.next()?;
    let mut value = state.get_var(root)?;
    for p in parts {
        value = match value {
            Expression::Record(fields) => fields.into_iter().find(|(k, _)| k == p)?.1,
            Expression::List(items) => items.into_iter().nth(p.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(value)
}

fn resolve_source_path(cwd: &Path, input: &str) -> PathBuf {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn resolve_import_path(cwd: &Path, module: &str) -> PathBuf {
    if let Some(std_mod) = module.strip_prefix("std/") {
        return cwd.join("std").join(format!("{std_mod}.dosh"));
    }
    let raw = PathBuf::from(module);
    if raw.is_absolute() {
        return raw;
    }
    if raw.extension().is_some() {
        return cwd.join(raw);
    }
    cwd.join(format!("{module}.dosh"))
}

fn collect_exports(
    statements: &[dosh_ast::Statement],
    snapshot: &std::collections::BTreeMap<String, Expression>,
) -> std::collections::BTreeMap<String, Expression> {
    let mut out = std::collections::BTreeMap::new();
    for stmt in statements {
        match stmt {
            dosh_ast::Statement::Assignment(assign) if assign.is_exported => {
                if let Some(value) = snapshot.get(&assign.name) {
                    out.insert(assign.name.clone(), value.clone());
                }
            }
            dosh_ast::Statement::Function {
                name, is_exported, ..
            } if *is_exported => {
                out.insert(name.clone(), Expression::Identifier(name.clone()));
            }
            _ => {}
        }
    }
    out
}

fn record_from_exports(
    exports: &std::collections::BTreeMap<String, Expression>,
) -> Vec<(String, Expression)> {
    exports
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn record_from_snapshot(
    snapshot: &std::collections::BTreeMap<String, Expression>,
) -> Vec<(String, Expression)> {
    snapshot
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{RuntimeState, edit_distance, interpolate_arg};

    #[test]
    fn edit_distance_basic() {
        assert_eq!(edit_distance("help", "help"), 0);
        assert_eq!(edit_distance("hep", "help"), 1);
        assert!(edit_distance("xyz", "help") >= 3);
    }

    #[test]
    fn interpolate_preserves_unknown_closure_vars() {
        let state = RuntimeState::new();
        assert_eq!(interpolate_arg("$it > 2", &state), "$it > 2");
        assert_eq!(interpolate_arg("$acc + $it", &state), "$acc + $it");
    }
}
