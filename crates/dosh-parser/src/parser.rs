use anyhow::{Context, Result};
use dosh_ast::{Assignment, Command, Expression, Pipeline, Script, Statement};

use crate::command::{parse_pipeline_commands, parse_single_command_tokens};
use crate::expr::{looks_like_call, parse_call_expression, parse_expression_result};
use crate::ident::{is_constant_name, is_identifier, is_variable_name};
use crate::syntax::{
    parse_block_and_tail, parse_param_list, split_before_block, split_top_level_statements,
    split_top_level_statements_with_offsets,
};
use crate::types::parse_type_expr;

#[derive(Debug, Default)]
pub struct Parser;

impl Parser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_line(&self, input: &str) -> Result<Script> {
        self.parse_script(input)
    }

    pub fn parse_script(&self, input: &str) -> Result<Script> {
        let parts = split_top_level_statements_with_offsets(input)?;
        let mut statements = Vec::new();

        for (idx, (part, start)) in parts.iter().enumerate() {
            let stmt = self.parse_statement(part).with_context(|| {
                let (line, col) = byte_to_line_col(input, *start);
                format!(
                    "failed to parse statement {} at {}:{} (byte {}): `{}`",
                    idx + 1,
                    line,
                    col,
                    start,
                    part
                )
            })?;
            statements.push(stmt);
        }

        Ok(Script { statements })
    }

    fn parse_statement(&self, text: &str) -> Result<Statement> {
        let line = text.trim();
        if line.is_empty() {
            anyhow::bail!("empty statement")
        }
        if line.starts_with('#') {
            return Ok(Statement::Expr(Expression::Null));
        }
        if let Some(pipeline) = parse_pipeline_with_literal_source(line)? {
            return Ok(Statement::Pipeline(pipeline));
        }
        if line.starts_with('{') {
            let (inner, tail) = parse_block_and_tail(line)?;
            if !tail.trim().is_empty() {
                anyhow::bail!("unexpected tokens after block");
            }
            let nested = self.parse_script(&inner)?;
            return Ok(Statement::Module {
                name: "__block__".to_string(),
                body: nested.statements,
            });
        }

        if let Some(stmt) = self.parse_assignment_statement(line)? {
            return Ok(stmt);
        }

        if let Some(rest) = line.strip_prefix("if ") {
            return self.parse_if_statement(rest);
        }

        if let Some(rest) = line.strip_prefix("for ") {
            return self.parse_for_statement(rest);
        }

        if let Some(rest) = line.strip_prefix("match ") {
            return self.parse_match_statement(rest);
        }

        if let Some(rest) = line.strip_prefix("fn ") {
            return self.parse_function_statement(rest);
        }
        if let Some(rest) = line.strip_prefix("test ") {
            return self.parse_test_statement(rest);
        }

        if let Some(rest) = line.strip_prefix("module ") {
            return self.parse_module_statement(rest);
        }
        if let Some(rest) = line.strip_prefix("mod ") {
            return self.parse_module_statement(rest);
        }

        if let Some(rest) = line.strip_prefix("use ") {
            let (module, alias) = parse_import_clause(rest.trim())?;
            return Ok(Statement::Import { module, alias });
        }
        if let Some(rest) = line.strip_prefix("import ") {
            let (module, alias) = parse_import_clause(rest.trim())?;
            return Ok(Statement::Import { module, alias });
        }
        if let Some(rest) = line.strip_prefix("export ") {
            return self.parse_export_statement(rest);
        }
        if line == "break" {
            return Ok(Statement::Break);
        }
        if line == "continue" {
            return Ok(Statement::Continue);
        }
        if let Some(rest) = line.strip_prefix("return") {
            let payload = rest.trim();
            if payload.is_empty() {
                return Ok(Statement::Return(None));
            }
            return Ok(Statement::Return(Some(
                self.parse_expression_with_lambda(payload)?,
            )));
        }

        let normalized = rewrite_leading_string_literal_pipeline(line);
        let tokens = shell_words::split(&normalized)?;
        if tokens.is_empty() {
            return Ok(Statement::Expr(Expression::StringLiteral(String::new())));
        }

        if tokens.iter().any(|t| t == "|") {
            let commands = parse_pipeline_commands(&tokens)?;
            return Ok(Statement::Pipeline(Pipeline { commands }));
        }

        if looks_like_call(line) {
            let call = parse_call_expression(line)?;
            return Ok(Statement::Expr(call));
        }

        let command = parse_single_command_tokens(&tokens)?;
        Ok(Statement::Command(command))
    }

    fn parse_if_statement(&self, rest: &str) -> Result<Statement> {
        let (condition_text, after_condition) = split_before_block(rest)?;
        let condition = self.parse_expression_with_lambda(condition_text)?;

        let (then_body, after_then) = parse_block_and_tail(after_condition)?;
        let then_script = self.parse_script(&then_body)?;

        let else_branch = self.parse_else_chain(after_then.trim())?;

        Ok(Statement::If {
            condition,
            then_branch: then_script.statements,
            else_branch,
        })
    }

    fn parse_else_chain(&self, tail: &str) -> Result<Vec<Statement>> {
        if tail.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(after_elif) = tail.strip_prefix("elif ") {
            let nested = self.parse_if_statement(after_elif)?;
            return Ok(vec![nested]);
        }
        if let Some(after_else) = tail.strip_prefix("else") {
            let after_else = after_else.trim_start();
            if let Some(after_if) = after_else.strip_prefix("if ") {
                let nested = self.parse_if_statement(after_if)?;
                return Ok(vec![nested]);
            }
            let (else_body, remain) = parse_block_and_tail(after_else)?;
            if !remain.trim().is_empty() {
                anyhow::bail!("unexpected tokens after else block: `{}`", remain.trim());
            }
            return Ok(self.parse_script(&else_body)?.statements);
        }
        anyhow::bail!("unexpected tokens after if block: `{tail}`");
    }

    fn parse_for_statement(&self, rest: &str) -> Result<Statement> {
        let (head, after_head) = split_before_block(rest)?;
        let mut pieces = head.split_whitespace();
        let variable = pieces.next().unwrap_or_default();
        let in_kw = pieces.next().unwrap_or_default();
        let iterable_text = pieces.collect::<Vec<_>>().join(" ");

        let Some(variable_name) = variable.strip_prefix('$') else {
            anyhow::bail!("invalid for statement, expected: for $<var> in <expr> {{ ... }}");
        };
        if !is_identifier(variable_name) || in_kw != "in" || iterable_text.is_empty() {
            anyhow::bail!("invalid for statement, expected: for $<var> in <expr> {{ ... }}");
        }

        let (body_text, tail) = parse_block_and_tail(after_head)?;
        if !tail.trim().is_empty() {
            anyhow::bail!("unexpected tokens after for block: `{}`", tail.trim());
        }

        let body = self.parse_script(&body_text)?.statements;
        Ok(Statement::For {
            variable: variable_name.to_string(),
            iterable: self.parse_expression_with_lambda(&iterable_text)?,
            body,
        })
    }

    fn parse_match_statement(&self, rest: &str) -> Result<Statement> {
        let (expr_text, after_expr) = split_before_block(rest)?;
        let expression = self.parse_expression_with_lambda(expr_text)?;
        let (body_text, tail) = parse_block_and_tail(after_expr)?;
        if !tail.trim().is_empty() {
            anyhow::bail!("unexpected tokens after match block: `{}`", tail.trim());
        }

        let mut arms = Vec::new();
        for arm_text in split_top_level_statements(&body_text)? {
            let Some((pat_text, arm_body_text)) = arm_text.split_once("=>") else {
                anyhow::bail!("invalid match arm, expected: <pattern> => <statement|block>");
            };
            let pattern = self.parse_expression_with_lambda(pat_text.trim())?;
            let body = if arm_body_text.trim().starts_with('{') {
                let (inner, remain) = parse_block_and_tail(arm_body_text.trim())?;
                if !remain.trim().is_empty() {
                    anyhow::bail!(
                        "unexpected tokens after match arm block: `{}`",
                        remain.trim()
                    );
                }
                self.parse_script(&inner)?.statements
            } else {
                vec![self.parse_statement(arm_body_text.trim())?]
            };
            arms.push((pattern, body));
        }

        Ok(Statement::Match { expression, arms })
    }

    fn parse_function_statement(&self, rest: &str) -> Result<Statement> {
        let (signature, after_signature) = split_before_block(rest)?;
        let open = signature
            .find('(')
            .ok_or_else(|| anyhow::anyhow!("invalid function signature, missing '('"))?;
        let close = signature
            .rfind(')')
            .ok_or_else(|| anyhow::anyhow!("invalid function signature, missing ')'"))?;
        if close <= open {
            anyhow::bail!("invalid function signature");
        }

        let name = signature[..open].trim();
        if !is_identifier(name) {
            anyhow::bail!("invalid function name `{name}`");
        }

        let params_text = &signature[open + 1..close];
        let params = parse_param_list(params_text)?;
        let after_close = signature[close + 1..].trim();
        let return_type = if let Some(rest) = after_close.strip_prefix("->") {
            Some(parse_type_expr(rest.trim())?)
        } else if after_close.is_empty() {
            None
        } else {
            anyhow::bail!("invalid function signature tail `{after_close}`");
        };

        let (body_text, tail) = parse_block_and_tail(after_signature)?;
        if !tail.trim().is_empty() {
            anyhow::bail!("unexpected tokens after function block: `{}`", tail.trim());
        }

        let body = self.parse_script(&body_text)?.statements;
        Ok(Statement::Function {
            name: name.to_string(),
            params,
            return_type,
            is_exported: false,
            body,
        })
    }

    fn parse_module_statement(&self, rest: &str) -> Result<Statement> {
        let (name, after_name) = split_before_block(rest)?;
        let module = name.trim();
        if !is_identifier(module) {
            anyhow::bail!("invalid module statement, expected: module <name> {{ ... }}");
        }

        let (body_text, tail) = parse_block_and_tail(after_name)?;
        if !tail.trim().is_empty() {
            anyhow::bail!("unexpected tokens after module block: `{}`", tail.trim());
        }

        let body = self.parse_script(&body_text)?.statements;
        Ok(Statement::Module {
            name: module.to_string(),
            body,
        })
    }

    fn parse_test_statement(&self, rest: &str) -> Result<Statement> {
        let (name_expr, after_name) = split_before_block(rest)?;
        let test_name = match parse_expression_result(name_expr.trim())? {
            Expression::StringLiteral(s) => s,
            _ => anyhow::bail!("test name must be a string literal"),
        };
        let (body_text, tail) = parse_block_and_tail(after_name)?;
        if !tail.trim().is_empty() {
            anyhow::bail!("unexpected tokens after test block: `{}`", tail.trim());
        }
        let body = self.parse_script(&body_text)?.statements;
        Ok(Statement::Test {
            name: test_name,
            body,
        })
    }

    fn parse_export_statement(&self, rest: &str) -> Result<Statement> {
        let line = rest.trim();
        if let Some(after_fn) = line.strip_prefix("fn ") {
            let stmt = self.parse_function_statement(after_fn)?;
            if let Statement::Function {
                name,
                params,
                return_type,
                body,
                ..
            } = stmt
            {
                return Ok(Statement::Function {
                    name,
                    params,
                    return_type,
                    is_exported: true,
                    body,
                });
            }
            anyhow::bail!("invalid export function statement");
        }
        if let Some(Statement::Assignment(mut assign)) = self.parse_assignment_statement(line)? {
            assign.is_exported = true;
            return Ok(Statement::Assignment(assign));
        }
        anyhow::bail!("export currently supports only function and variable assignment")
    }
}

fn rewrite_leading_string_literal_pipeline(line: &str) -> String {
    let s = line.trim_start();
    let mut chars = s.chars();
    let Some(q) = chars.next() else {
        return line.to_string();
    };
    if q != '"' && q != '\'' {
        return line.to_string();
    }
    let mut escaped = false;
    let mut end_idx = None;
    for (idx, ch) in s.char_indices().skip(1) {
        if q == '"' && ch == '\\' && !escaped {
            escaped = true;
            continue;
        }
        if ch == q && !escaped {
            end_idx = Some(idx);
            break;
        }
        escaped = false;
    }
    let Some(end) = end_idx else {
        return line.to_string();
    };
    let rest = s[end + q.len_utf8()..].trim_start();
    if !rest.starts_with('|') {
        return line.to_string();
    }
    let literal = &s[..=end];
    format!("echo {literal} {rest}")
}

fn parse_pipeline_with_literal_source(line: &str) -> Result<Option<Pipeline>> {
    let Some((head, tail)) = split_first_pipeline_segment(line) else {
        return Ok(None);
    };
    if tail.trim().is_empty() {
        return Ok(None);
    }
    let expr = match parse_expression_result(head.trim()) {
        Ok(expr) => expr,
        Err(_) => return Ok(None),
    };
    if !is_literal_expr(&expr) {
        return Ok(None);
    }
    let tokens = shell_words::split(tail.trim_start_matches('|').trim())?;
    if tokens.is_empty() {
        anyhow::bail!("invalid pipeline: missing commands after '|'");
    }
    let mut commands = vec![Command {
        name: "__literal__".to_string(),
        args: vec![head.trim().to_string()],
        redirects: Vec::new(),
        background: false,
        force_external: false,
    }];
    let mut rest = parse_pipeline_commands(&tokens)?;
    commands.append(&mut rest);
    Ok(Some(Pipeline { commands }))
}

fn split_first_pipeline_segment(line: &str) -> Option<(&str, &str)> {
    let mut in_string = false;
    let mut escaped = false;
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut depth_bracket = 0i32;
    for (idx, ch) in line.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            '|' if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 => {
                return Some((&line[..idx], &line[idx + 1..]));
            }
            _ => {}
        }
    }
    None
}

fn is_literal_expr(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::Null
            | Expression::StringLiteral(_)
            | Expression::Integer(_)
            | Expression::Float(_)
            | Expression::Bool(_)
            | Expression::List(_)
            | Expression::Record(_)
    )
}

fn byte_to_line_col(input: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (idx, ch) in input.char_indices() {
        if idx >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

impl Parser {
    fn parse_assignment_statement(&self, line: &str) -> Result<Option<Statement>> {
        let Some((lhs, rhs)) = split_assignment(line) else {
            return Ok(None);
        };
        let target = lhs.trim();
        if !target.starts_with('$') {
            return Ok(None);
        }
        let (name, cell_path) = parse_assignment_target(target)?;
        if !is_variable_name(&format!("${name}")) {
            anyhow::bail!("invalid variable name `{target}`");
        }
        let is_constant = is_constant_name(&name);
        let value = self.parse_expression_with_lambda(rhs.trim())?;
        Ok(Some(Statement::Assignment(Assignment {
            name,
            cell_path,
            is_constant,
            is_exported: false,
            value,
        })))
    }
}

impl Parser {
    fn parse_expression_with_lambda(&self, text: &str) -> Result<Expression> {
        let text = text.trim();
        if let Some(after_open) = text.strip_prefix('|') {
            let Some((param_text, body_text)) = after_open.split_once('|') else {
                anyhow::bail!("invalid lambda expression, expected `|params| expr`");
            };
            let params = parse_param_list(param_text)?;
            let body = if body_text.trim().starts_with('{') {
                let (inner, tail) = parse_block_and_tail(body_text.trim())?;
                if !tail.trim().is_empty() {
                    anyhow::bail!("unexpected tokens after lambda body: `{}`", tail.trim());
                }
                self.parse_script(&inner)?.statements
            } else {
                vec![self.parse_statement(body_text.trim())?]
            };
            return Ok(Expression::Lambda { params, body });
        }
        parse_expression_result(text)
    }
}

fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == '=' {
            let prev = line[..idx].chars().last().unwrap_or(' ');
            let next = line[idx + 1..].chars().next().unwrap_or(' ');
            if matches!(prev, '>' | '<' | '!' | '=') || next == '=' {
                continue;
            }
            return Some((&line[..idx], &line[idx + 1..]));
        }
    }
    None
}

fn parse_assignment_target(target: &str) -> Result<(String, Vec<dosh_ast::CellPathSegment>)> {
    let raw = target
        .trim()
        .strip_prefix('$')
        .ok_or_else(|| anyhow::anyhow!("invalid assignment target"))?;
    let mut parts = raw.split('.');
    let root = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid assignment target"))?;
    if root.is_empty() || !is_identifier(root) {
        anyhow::bail!("invalid assignment target `{target}`");
    }

    let mut path = Vec::new();
    for p in parts {
        if p.is_empty() {
            anyhow::bail!("Invalid cell path");
        }
        if let Ok(i) = p.parse::<usize>() {
            path.push(dosh_ast::CellPathSegment::Index(i));
        } else if is_identifier(p) {
            path.push(dosh_ast::CellPathSegment::Field(p.to_string()));
        } else {
            anyhow::bail!("Invalid cell path");
        }
    }
    Ok((root.to_string(), path))
}

fn parse_import_clause(input: &str) -> Result<(String, Option<String>)> {
    let mut tokens = input.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        anyhow::bail!("invalid import statement, expected: import <module|\"path\"> [as alias]");
    }

    let module = tokens.remove(0).trim().trim_matches('"').to_string();
    let alias = if tokens.is_empty() {
        None
    } else if tokens.len() == 2 && tokens[0] == "as" && is_identifier(tokens[1]) {
        Some(tokens[1].to_string())
    } else {
        anyhow::bail!("invalid import clause, expected optional `as <alias>`");
    };

    if module.is_empty() {
        anyhow::bail!("import target cannot be empty");
    }
    Ok((module, alias))
}
