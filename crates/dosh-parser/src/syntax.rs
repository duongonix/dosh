use anyhow::Result;

use crate::ident::is_identifier;
use crate::types::parse_type_expr;
use dosh_ast::Param;

pub fn split_top_level_statements(input: &str) -> Result<Vec<String>> {
    Ok(split_top_level_statements_with_offsets(input)?
        .into_iter()
        .map(|(s, _)| s)
        .collect())
}

pub fn split_top_level_statements_with_offsets(input: &str) -> Result<Vec<(String, usize)>> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut in_here_single = false;
    let mut escaped = false;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < input.len() {
        if in_here_single {
            if input[i..].starts_with("'@") {
                in_here_single = false;
                i += 2;
                continue;
            }
            i += input[i..].chars().next().map(char::len_utf8).unwrap_or(1);
            continue;
        }

        let ch = input[i..]
            .chars()
            .next()
            .ok_or_else(|| anyhow::anyhow!("invalid input"))?;

        if in_string {
            if escaped {
                escaped = false;
                i += ch.len_utf8();
                continue;
            }
            if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += ch.len_utf8();
            continue;
        }

        if input[i..].starts_with("@'") {
            in_here_single = true;
            i += 2;
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    anyhow::bail!("unmatched closing brace at byte {i}");
                }
                depth -= 1;
            }
            ';' | '\n' if depth == 0 => {
                let part = input[start..i].trim();
                if !part.is_empty() {
                    parts.push((part.to_string(), start));
                }
                start = i + ch.len_utf8();
            }
            _ => {}
        }
        i += ch.len_utf8();
    }

    if in_string {
        anyhow::bail!("unterminated string literal");
    }
    if in_here_single {
        anyhow::bail!("unterminated here-string literal");
    }
    if depth != 0 {
        anyhow::bail!("unclosed block: missing '}}'");
    }

    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push((tail.to_string(), start));
    }

    Ok(parts)
}

pub fn split_before_block(text: &str) -> Result<(&str, &str)> {
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in text.char_indices() {
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

        if ch == '{' {
            return Ok((text[..idx].trim_end(), &text[idx..]));
        }
    }

    anyhow::bail!("expected block starting with '{{'")
}

pub fn parse_block_and_tail(text: &str) -> Result<(String, &str)> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with('{') {
        anyhow::bail!("expected block starting with '{{'");
    }

    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in trimmed.char_indices() {
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

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let body = trimmed[1..idx].to_string();
                    let tail = &trimmed[idx + 1..];
                    return Ok((body, tail));
                }
            }
            _ => {}
        }
    }

    anyhow::bail!("unclosed block: missing '}}'")
}

pub fn split_csv_like(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut paren = 0i32;
    let mut brace = 0i32;
    let mut bracket = 0i32;
    let mut in_string = false;
    let mut escaped = false;

    for ch in input.chars() {
        if in_string {
            current.push(ch);
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

        match ch {
            '"' => {
                in_string = true;
                current.push(ch);
            }
            '(' => {
                paren += 1;
                current.push(ch);
            }
            ')' => {
                paren -= 1;
                current.push(ch);
            }
            '{' => {
                brace += 1;
                current.push(ch);
            }
            '}' => {
                brace -= 1;
                current.push(ch);
            }
            '[' => {
                bracket += 1;
                current.push(ch);
            }
            ']' => {
                bracket -= 1;
                current.push(ch);
            }
            ',' if paren == 0 && brace == 0 && bracket == 0 => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }

    out
}

pub fn parse_param_list(input: &str) -> Result<Vec<Param>> {
    let mut out = Vec::new();
    for raw in split_csv_like(input) {
        if raw.is_empty() {
            continue;
        }
        let (name, ty) = if let Some((left, right)) = raw.split_once(':') {
            let name = left.trim();
            let stripped = name.strip_prefix('$').ok_or_else(|| {
                anyhow::anyhow!("Function parameter must start with `$`: `{name}`")
            })?;
            if !is_identifier(stripped) {
                anyhow::bail!("invalid identifier `{name}` in parameter list");
            }
            (stripped.to_string(), Some(parse_type_expr(right.trim())?))
        } else {
            let stripped = raw.strip_prefix('$').ok_or_else(|| {
                anyhow::anyhow!("Function parameter must start with `$`: `{raw}`")
            })?;
            if !is_identifier(stripped) {
                anyhow::bail!("invalid identifier `{raw}` in parameter list");
            }
            (stripped.to_string(), None)
        };
        out.push(Param { name, ty });
    }
    Ok(out)
}
