use anyhow::Result;
use dosh_ast::{BinaryOp, CellPathSegment, Expression, Pipeline, UnaryOp};

use crate::command::parse_pipeline_commands;
use crate::ident::is_identifier;
use crate::syntax::split_csv_like;

pub fn parse_expression(value: &str) -> Expression {
    parse_expression_result(value)
        .unwrap_or_else(|_| Expression::Identifier(value.trim().to_string()))
}

pub fn parse_expression_result(value: &str) -> Result<Expression> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("empty expression");
    }

    if let Some(expr) = parse_pipeline_expr(value)? {
        return Ok(expr);
    }
    if let Some(expr) = parse_binary_expr(value)? {
        return Ok(expr);
    }
    if let Some(expr) = parse_unary_expr(value)? {
        return Ok(expr);
    }
    if let Some(expr) = parse_record_literal(value)? {
        return Ok(expr);
    }
    if let Some(expr) = parse_list_literal(value)? {
        return Ok(expr);
    }
    if let Some(expr) = parse_range_literal(value) {
        return Ok(expr);
    }
    if value.eq_ignore_ascii_case("null") {
        return Ok(Expression::Null);
    }
    if value.eq_ignore_ascii_case("true") {
        return Ok(Expression::Bool(true));
    }
    if value.eq_ignore_ascii_case("false") {
        return Ok(Expression::Bool(false));
    }
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        return Ok(Expression::StringLiteral(
            value[1..value.len() - 1].to_string(),
        ));
    }

    if let Some(var) = parse_variable_expr(value) {
        return Ok(var);
    }

    if let Some(expr) = parse_number_or_special_literal(value) {
        return Ok(expr);
    }

    if looks_like_call(value) {
        if let Ok(call) = parse_call_expression(value) {
            return Ok(call);
        }
    }

    if let Ok(call) = parse_space_call_expression(value) {
        return Ok(call);
    }

    Ok(Expression::Identifier(value.to_string()))
}

fn parse_pipeline_expr(value: &str) -> Result<Option<Expression>> {
    let tokens = shell_words::split(value)?;
    if tokens.iter().any(|t| t == "|") {
        let commands = parse_pipeline_commands(&tokens)?;
        return Ok(Some(Expression::Pipeline(Pipeline { commands })));
    }
    Ok(None)
}

fn parse_binary_expr(value: &str) -> Result<Option<Expression>> {
    let ops: [(&str, BinaryOp); 12] = [
        ("||", BinaryOp::Or),
        ("&&", BinaryOp::And),
        (">=", BinaryOp::Gte),
        ("<=", BinaryOp::Lte),
        ("!=", BinaryOp::Ne),
        ("==", BinaryOp::Eq),
        (">", BinaryOp::Gt),
        ("<", BinaryOp::Lt),
        ("+", BinaryOp::Add),
        ("-", BinaryOp::Sub),
        ("*", BinaryOp::Mul),
        ("/", BinaryOp::Div),
    ];

    for (op_text, op) in ops {
        if let Some((left, right)) = split_once_top_level(value, op_text) {
            let left = parse_expression_result(left.trim())?;
            let right = parse_expression_result(right.trim())?;
            return Ok(Some(Expression::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            }));
        }
    }
    Ok(None)
}

fn parse_unary_expr(value: &str) -> Result<Option<Expression>> {
    if let Some(rest) = value.strip_prefix('!') {
        return Ok(Some(Expression::Unary {
            op: UnaryOp::Not,
            expr: Box::new(parse_expression_result(rest.trim())?),
        }));
    }
    if let Some(rest) = value.strip_prefix('-') {
        if !rest.trim().is_empty() {
            return Ok(Some(Expression::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(parse_expression_result(rest.trim())?),
            }));
        }
    }
    Ok(None)
}

fn parse_record_literal(value: &str) -> Result<Option<Expression>> {
    let Some(inner) = value.strip_prefix('{').and_then(|s| s.strip_suffix('}')) else {
        return Ok(None);
    };
    if inner.trim().is_empty() {
        return Ok(Some(Expression::Record(Vec::new())));
    }
    let mut fields = Vec::new();
    for item in split_csv_like(inner) {
        let Some((key, val)) = item.split_once(':') else {
            anyhow::bail!("invalid record literal entry `{item}`");
        };
        let key = key.trim();
        if !is_identifier(key) {
            anyhow::bail!("invalid record field `{key}`");
        }
        fields.push((key.to_string(), parse_expression_result(val.trim())?));
    }
    Ok(Some(Expression::Record(fields)))
}

fn parse_list_literal(value: &str) -> Result<Option<Expression>> {
    let Some(inner) = value.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return Ok(None);
    };
    let mut items = Vec::new();
    for item in split_csv_like(inner) {
        if item.trim().is_empty() {
            continue;
        }
        items.push(parse_expression_result(item.trim())?);
    }
    Ok(Some(Expression::List(items)))
}

fn parse_range_literal(value: &str) -> Option<Expression> {
    let (left, right) = split_once_top_level(value, "..")?;
    let start = left.trim().parse::<i64>().ok()?;
    let end = right.trim().parse::<i64>().ok()?;
    Some(Expression::Range { start, end })
}

fn parse_number_or_special_literal(value: &str) -> Option<Expression> {
    if let Ok(v) = value.parse::<i64>() {
        return Some(Expression::Integer(v));
    }
    if value.parse::<f64>().is_ok() {
        return Some(Expression::Float(value.to_string()));
    }
    let lower = value.to_ascii_lowercase();
    let starts_with_digit = lower.chars().next().is_some_and(|c| c.is_ascii_digit());
    if starts_with_digit
        && (lower.ends_with("kb")
            || lower.ends_with("mb")
            || lower.ends_with("gb")
            || lower.ends_with("tb")
            || lower.ends_with("b")
            || lower.ends_with("sec")
            || lower.ends_with("min")
            || lower.ends_with("hr")
            || lower.ends_with("day")
            || lower.ends_with("ms"))
    {
        return Some(Expression::StringLiteral(value.to_string()));
    }
    None
}

fn parse_variable_expr(value: &str) -> Option<Expression> {
    let mut parts = value.split('.');
    let root = parts.next()?;
    let name = root.strip_prefix('$')?;
    if !is_identifier(name) {
        return None;
    }
    let mut cell_path = Vec::new();
    for p in parts {
        if p.is_empty() {
            return None;
        }
        if let Ok(i) = p.parse::<usize>() {
            cell_path.push(CellPathSegment::Index(i));
        } else if is_identifier(p) {
            cell_path.push(CellPathSegment::Field(p.to_string()));
        } else {
            return None;
        }
    }
    Some(Expression::Variable {
        name: name.to_string(),
        cell_path,
    })
}

pub fn parse_call_expression(text: &str) -> Result<Expression> {
    let open = text
        .find('(')
        .ok_or_else(|| anyhow::anyhow!("invalid function call, missing '('"))?;
    let close = text
        .rfind(')')
        .ok_or_else(|| anyhow::anyhow!("invalid function call, missing ')'"))?;
    if close <= open {
        anyhow::bail!("invalid function call expression");
    }

    let name = text[..open].trim();
    if !is_identifier(name) {
        anyhow::bail!("invalid function call name `{name}`");
    }
    if !text[close + 1..].trim().is_empty() {
        anyhow::bail!("unexpected tokens after function call");
    }

    let args_text = &text[open + 1..close];
    let args = split_csv_like(args_text)
        .into_iter()
        .filter(|v| !v.trim().is_empty())
        .map(|v| parse_expression(v.trim()))
        .collect();

    Ok(Expression::Call {
        name: name.to_string(),
        args,
    })
}

fn parse_space_call_expression(text: &str) -> Result<Expression> {
    let tokens = shell_words::split(text)?;
    if tokens.len() < 2 {
        anyhow::bail!("not a space call expression");
    }
    if !is_identifier(&tokens[0]) {
        anyhow::bail!("invalid call name");
    }
    let args = tokens[1..]
        .iter()
        .map(|s| parse_expression(s))
        .collect::<Vec<_>>();
    Ok(Expression::Call {
        name: tokens[0].clone(),
        args,
    })
}

pub fn looks_like_call(text: &str) -> bool {
    let text = text.trim();
    text.ends_with(')') && text.contains('(')
}

fn split_once_top_level<'a>(input: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut depth_bracket = 0i32;
    let mut in_string = false;
    let mut escaped = false;

    let bytes = input.as_bytes();
    let n = needle.as_bytes();
    let mut i = 0usize;
    while i + n.len() <= bytes.len() {
        let ch = input[i..].chars().next()?;
        let len = ch.len_utf8();

        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += len;
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
            _ => {}
        }

        if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 && &bytes[i..i + n.len()] == n
        {
            let left = &input[..i];
            let right = &input[i + n.len()..];
            return Some((left, right));
        }

        i += len;
    }
    None
}
