use crate::context::CompletionContext;
use crate::model::CompletionItem;
use dosh_ast::{Expression, Pipeline, Script, Statement};
use dosh_env::EnvContext;
use dosh_parser::{Parser, parse_expression_result};
use dosh_runtime::Runtime;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderScript {
    pub source: String,
}

pub fn eval_provider_script(
    script: &ProviderScript,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let source = build_wrapped_script(script, ctx);
    let parser = Parser::new();
    let Ok(ast) = parser.parse_script(&source) else {
        return Vec::new();
    };
    if !sandbox_allows(&ast) {
        return Vec::new();
    }

    let runtime = Runtime::new();
    let cwd = PathBuf::from(&ctx.cwd);
    let mut env = EnvContext::new(cwd);
    let Ok(outcome) = runtime.execute(&ast, &mut env) else {
        return Vec::new();
    };
    let Some(output) = outcome.output else {
        return Vec::new();
    };
    normalize_script_output(&output)
}

fn build_wrapped_script(script: &ProviderScript, ctx: &CompletionContext) -> String {
    let mut s = String::new();
    s.push_str("$ctx = ");
    s.push_str(&ctx_record_literal(ctx));
    s.push('\n');
    s.push_str(&normalize_pipeline_continuations(script.source.trim()));
    s.push('\n');
    s
}

fn ctx_record_literal(ctx: &CompletionContext) -> String {
    let words = ctx
        .words
        .iter()
        .map(|w| format!("\"{}\"", escape_string(w)))
        .collect::<Vec<_>>()
        .join(", ");
    let args = ctx
        .args
        .iter()
        .map(|a| format!("\"{}\"", escape_string(a)))
        .collect::<Vec<_>>()
        .join(", ");
    let previous = ctx
        .previous
        .as_ref()
        .map(|p| format!("\"{}\"", escape_string(p)))
        .unwrap_or_else(|| "null".to_string());
    let flag = ctx
        .flag
        .as_ref()
        .map(|p| format!("\"{}\"", escape_string(p)))
        .unwrap_or_else(|| "null".to_string());
    let command_path = ctx
        .command_path
        .as_ref()
        .map(|p| format!("\"{}\"", escape_string(p)))
        .unwrap_or_else(|| "null".to_string());
    format!(
        "{{ line: \"{}\", cursor: {}, words: [{}], command: \"{}\", args: [{}], current: \"{}\", previous: {}, position: {}, cwd: \"{}\", is_flag: {}, flag: {}, command_path: {} }}",
        escape_string(&ctx.line),
        ctx.cursor,
        words,
        escape_string(&ctx.command),
        args,
        escape_string(&ctx.current),
        previous,
        ctx.position,
        escape_string(&ctx.cwd),
        if ctx.is_flag { "true" } else { "false" },
        flag,
        command_path,
    )
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_pipeline_continuations(src: &str) -> String {
    let mut out = Vec::<String>::new();
    for raw in src.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if trimmed.starts_with('|') || trimmed.starts_with("&&") || trimmed.starts_with("||") {
            if let Some(last) = out.last_mut() {
                last.push(' ');
                last.push_str(trimmed);
            } else {
                out.push(trimmed.to_string());
            }
            continue;
        }
        if line.ends_with('|') || line.ends_with("&&") || line.ends_with("||") {
            out.push(line.to_string());
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

fn sandbox_allows(script: &Script) -> bool {
    script.statements.iter().all(sandbox_statement)
}

fn sandbox_statement(stmt: &Statement) -> bool {
    match stmt {
        Statement::Assignment(assign) => assign.name == "ctx",
        Statement::Command(cmd) => !cmd.force_external,
        Statement::Pipeline(p) => sandbox_pipeline(p),
        Statement::Expr(expr) => sandbox_expr(expr),
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            sandbox_expr(condition)
                && then_branch.iter().all(sandbox_statement)
                && else_branch.iter().all(sandbox_statement)
        }
        Statement::Match { expression, arms } => {
            sandbox_expr(expression)
                && arms
                    .iter()
                    .all(|(p, body)| sandbox_expr(p) && body.iter().all(sandbox_statement))
        }
        Statement::Let { .. }
        | Statement::For { .. }
        | Statement::Function { .. }
        | Statement::Module { .. }
        | Statement::Import { .. }
        | Statement::Test { .. }
        | Statement::Return(_)
        | Statement::Break
        | Statement::Continue => false,
    }
}

fn sandbox_pipeline(p: &Pipeline) -> bool {
    p.commands.iter().all(|c| !c.force_external)
}

fn sandbox_expr(expr: &Expression) -> bool {
    match expr {
        Expression::Pipeline(p) => sandbox_pipeline(p),
        Expression::Binary { left, right, .. } => sandbox_expr(left) && sandbox_expr(right),
        Expression::Unary { expr, .. } => sandbox_expr(expr),
        Expression::List(items) => items.iter().all(sandbox_expr),
        Expression::Record(fields) => fields.iter().all(|(_, v)| sandbox_expr(v)),
        Expression::Call { .. }
        | Expression::Variable { .. }
        | Expression::Identifier(_)
        | Expression::StringLiteral(_)
        | Expression::Integer(_)
        | Expression::Float(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Range { .. } => true,
        Expression::MemberAccess { object, .. } => sandbox_expr(object),
        Expression::Lambda { .. } | Expression::Closure { .. } => false,
    }
}

fn normalize_script_output(output: &str) -> Vec<CompletionItem> {
    if let Ok(expr) = parse_expression_result(output) {
        let items = expression_to_items(&expr);
        if !items.is_empty() {
            return items;
        }
    }
    output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| CompletionItem::new(l.to_string(), None))
        .collect()
}

fn expression_to_items(expr: &Expression) -> Vec<CompletionItem> {
    match expr {
        Expression::List(items) => items
            .iter()
            .filter_map(item_from_expression)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    }
}

fn item_from_expression(expr: &Expression) -> Option<CompletionItem> {
    match expr {
        Expression::StringLiteral(v) => Some(CompletionItem::new(v.clone(), None)),
        Expression::Record(fields) => {
            let mut item = CompletionItem::new(String::new(), None);
            for (k, v) in fields {
                let text = scalar_as_string(v)?;
                match k.as_str() {
                    "value" => item.value = text,
                    "description" => item.description = Some(text),
                    "kind" => item.kind = Some(text),
                    "icon" => item.icon = Some(text),
                    "insert" | "insert_text" => item.insert_text = Some(text),
                    "priority" => item.priority = text.parse::<i64>().ok(),
                    _ => {}
                }
            }
            if item.value.is_empty() {
                None
            } else {
                Some(item)
            }
        }
        _ => None,
    }
}

fn scalar_as_string(expr: &Expression) -> Option<String> {
    match expr {
        Expression::StringLiteral(v) => Some(v.clone()),
        Expression::Integer(v) => Some(v.to_string()),
        Expression::Bool(v) => Some(v.to_string()),
        Expression::Float(v) => Some(v.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_blocks_external_commands() {
        let parser = Parser::new();
        let ast = parser.parse_script("^git status").unwrap();
        assert!(!sandbox_allows(&ast));
    }

    #[test]
    fn normalize_lines_output() {
        let items = normalize_script_output("Toyota\nHonda\n");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].value, "Toyota");
    }

    #[test]
    fn normalize_pipeline_lines() {
        let src = "ls\n| where type == \"dir\"\n| select name";
        let normalized = normalize_pipeline_continuations(src);
        assert_eq!(normalized, "ls | where type == \"dir\" | select name");
    }
}
