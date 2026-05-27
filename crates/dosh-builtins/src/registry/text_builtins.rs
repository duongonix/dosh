use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::file_pipeline_builtins::apply_replace;
use crate::registry::simple_builtin;
use crate::registry::type_number_builtins::convert_to_unit;
use anyhow::{anyhow, bail};
use dosh_value::{from_json_str, from_toml_str, from_yaml_str};
use regex::Regex;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        || Box::new(StrBuiltin),
        || Box::new(ToBuiltin),
        || Box::new(AppendBuiltin),
        || Box::new(SplitBuiltin),
        || Box::new(LinesBuiltin),
        || Box::new(TrimBuiltin),
        || Box::new(ReplaceBuiltin),
        || Box::new(ContainsBuiltin),
        || Box::new(StartsWithBuiltin),
        || Box::new(EndsWithBuiltin),
        || Box::new(MatchBuiltin),
        || Box::new(ParseBuiltin),
        || Box::new(FormatBuiltin),
    ]
}

simple_builtin!(
    ToBuiltin,
    "to",
    "to <str|string>",
    "Convert input to target type foundation",
    &["1 | to str", "1mb | to kb", "90sec | to min"],
    |args, input, _ctx| {
        let target = args
            .first()
            .ok_or_else(|| anyhow!("to expects target type"))?;
        let value = pipeline_to_value(input)?;
        match target.as_str() {
            "str" | "string" => Ok(BuiltinOutcome::ok(PipelineData::Text(value.to_string()))),
            "int" => Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Int(
                value.to_string().trim().parse::<i64>()?,
            )))),
            "float" => Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(
                value.to_string().trim().parse::<f64>()?,
            )))),
            "bool" => Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(
                value.to_string().trim().parse::<bool>()?,
            )))),
            unit => {
                if let Some(v) = convert_to_unit(&value, unit) {
                    Ok(BuiltinOutcome::ok(PipelineData::Value(v)))
                } else {
                    bail!("unsupported to target: {target}")
                }
            }
        }
    }
);

simple_builtin!(
    StrBuiltin,
    "str",
    "str <subcommand> ...",
    "String toolkit: join|contains|trim|replace|index-of|substring|reverse|length|starts-with|ends-with",
    &[
        "echo hello | str contains ell",
        "echo a,b | split row , | str join -"
    ],
    |args, input, _ctx| {
        let sub = args
            .first()
            .map(|s| s.as_str())
            .ok_or_else(|| anyhow!("str expects subcommand"))?;
        let rest = &args[1..];
        let value = pipeline_to_value(input)?;
        let out = match sub {
            "join" => {
                let sep = rest.first().cloned().unwrap_or_else(|| "".to_string());
                match value {
                    Value::List(v) => Value::String(
                        v.into_iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .join(&sep),
                    ),
                    Value::Table(t) => Value::String(
                        t.rows
                            .into_iter()
                            .map(|r| Value::Record(r).to_string())
                            .collect::<Vec<_>>()
                            .join(&sep),
                    ),
                    v => Value::String(v.to_string()),
                }
            }
            "contains" => {
                let needle = rest
                    .first()
                    .ok_or_else(|| anyhow!("str contains expects needle"))?;
                Value::Bool(value.to_string().contains(needle))
            }
            "trim" => {
                let mut left = false;
                let mut right = false;
                let mut ch: Option<String> = None;
                let mut i = 0usize;
                while i < rest.len() {
                    match rest[i].as_str() {
                        "--left" | "-l" => left = true,
                        "--right" | "-r" => right = true,
                        "--char" | "-c" => {
                            ch = rest.get(i + 1).cloned();
                            i += 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                map_strings(value, |s| {
                    let mut out = s.to_string();
                    if let Some(c) = ch.as_deref() {
                        let chars: Vec<char> = c.chars().collect();
                        if chars.is_empty() {
                            return out;
                        }
                        if left && !right {
                            out = out.trim_start_matches(chars[0]).to_string();
                        } else if right && !left {
                            out = out.trim_end_matches(chars[0]).to_string();
                        } else {
                            out = out.trim_matches(chars[0]).to_string();
                        }
                    } else if left && !right {
                        out = out.trim_start().to_string();
                    } else if right && !left {
                        out = out.trim_end().to_string();
                    } else {
                        out = out.trim().to_string();
                    }
                    out
                })
            }
            "replace" => {
                let regex_mode = rest.iter().any(|a| a == "--regex" || a == "-r");
                let mut pos = Vec::new();
                for a in rest {
                    if a == "--regex" || a == "-r" {
                        continue;
                    }
                    pos.push(a.clone());
                }
                if pos.len() < 2 {
                    bail!("str replace expects <from> <to>")
                }
                let from = &pos[0];
                let to = &pos[1];
                if regex_mode {
                    let re = Regex::new(from)?;
                    map_strings(value, |s| re.replace_all(s, to.as_str()).to_string())
                } else {
                    map_strings(value, |s| s.replace(from, to))
                }
            }
            "index-of" => {
                let needle = rest
                    .first()
                    .ok_or_else(|| anyhow!("str index-of expects needle"))?;
                let idx = value
                    .to_string()
                    .find(needle)
                    .map(|v| v as i64)
                    .unwrap_or(-1);
                Value::Int(idx)
            }
            "substring" => {
                let range = rest
                    .first()
                    .ok_or_else(|| anyhow!("str substring expects range start..end"))?;
                let (start, end) = parse_range(range)?;
                map_strings(value, |s| {
                    s.chars()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .collect::<String>()
                })
            }
            "reverse" => map_strings(value, |s| s.chars().rev().collect::<String>()),
            "length" => Value::Int(value.to_string().chars().count() as i64),
            "starts-with" => {
                let p = rest
                    .first()
                    .ok_or_else(|| anyhow!("str starts-with expects prefix"))?;
                Value::Bool(value.to_string().starts_with(p))
            }
            "ends-with" => {
                let p = rest
                    .first()
                    .ok_or_else(|| anyhow!("str ends-with expects suffix"))?;
                Value::Bool(value.to_string().ends_with(p))
            }
            _ => bail!("unsupported str subcommand: {sub}"),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    AppendBuiltin,
    "append",
    "append <value...>",
    "Append items to list or append text tail",
    &["echo hello | append world", "open arr.json | append 42"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("append expects value")
        }
        let value = pipeline_to_value(input)?;
        let tail = if args.len() == 1 {
            parse_scalar(&args[0])
        } else {
            Value::String(args.join(" "))
        };
        let out = match value {
            Value::List(mut v) => {
                v.push(tail);
                Value::List(v)
            }
            Value::String(s) => Value::List(vec![Value::String(s), tail]),
            v => Value::List(vec![v, tail]),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);
simple_builtin!(
    SplitBuiltin,
    "split",
    "split <row|column|chars> [sep]",
    "Split string into list/table/chars",
    &[
        "echo a,b,c | split row ,",
        "echo a,b,c | split column ,",
        "echo aeiou | split chars"
    ],
    |args, input, _ctx| {
        let mode = args.first().map(|s| s.as_str()).unwrap_or("row");
        let text = input.into_text();
        match mode {
            "row" => {
                let sep = args.get(1).cloned().unwrap_or_else(|| " ".to_string());
                let out = text
                    .split(&sep)
                    .map(|s| Value::String(s.to_string()))
                    .collect::<Vec<_>>();
                Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
            }
            "column" => {
                let sep = args.get(1).cloned().unwrap_or_else(|| " ".to_string());
                let cells = text.split(&sep).map(|s| s.to_string()).collect::<Vec<_>>();
                let mut row = Record::new();
                for (i, cell) in cells.into_iter().enumerate() {
                    row.insert(format!("column{}", i + 1), Value::String(cell));
                }
                Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
                    Table::new(vec![row]),
                ))))
            }
            "chars" => {
                let out = text
                    .chars()
                    .map(|c| Value::String(c.to_string()))
                    .collect::<Vec<_>>();
                Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
            }
            other => {
                let sep = other.to_string();
                let out = text
                    .split(&sep)
                    .map(|s| Value::String(s.to_string()))
                    .collect::<Vec<_>>();
                Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
            }
        }
    }
);
simple_builtin!(
    LinesBuiltin,
    "lines",
    "lines",
    "Split text by newline",
    &["cat README.md | lines"],
    |_args, input, _ctx| {
        let out = input
            .into_text()
            .lines()
            .map(|s| Value::String(s.to_string()))
            .collect::<Vec<_>>();
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
    }
);

fn map_strings(value: Value, f: impl Fn(&str) -> String) -> Value {
    match value {
        Value::String(s) => Value::String(f(&s)),
        Value::List(v) => Value::List(
            v.into_iter()
                .map(|x| match x {
                    Value::String(s) => Value::String(f(&s)),
                    other => Value::String(f(&other.to_string())),
                })
                .collect(),
        ),
        other => Value::String(f(&other.to_string())),
    }
}

fn parse_range(input: &str) -> anyhow::Result<(usize, usize)> {
    let (a, b) = input
        .split_once("..")
        .ok_or_else(|| anyhow!("range must be start..end"))?;
    let start = a.trim().parse::<usize>()?;
    let end = b.trim().parse::<usize>()?;
    if end < start {
        bail!("range end must be >= start")
    }
    Ok((start, end))
}

fn parse_scalar(raw: &str) -> Value {
    if let Ok(i) = raw.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = raw.parse::<f64>() {
        return Value::Float(f);
    }
    if let Ok(b) = raw.parse::<bool>() {
        return Value::Bool(b);
    }
    Value::String(raw.trim_matches('"').to_string())
}
simple_builtin!(
    TrimBuiltin,
    "trim",
    "trim",
    "Trim leading/trailing whitespace",
    &["echo '  hi  ' | trim"],
    |_args, input, _ctx| Ok(BuiltinOutcome::ok(PipelineData::Text(
        input.into_text().trim().to_string()
    )))
);
simple_builtin!(
    ReplaceBuiltin,
    "replace",
    "replace [--regex] [--ignore-case] [--recursive] [--path <field.path>] <from> <to>",
    "Replace text in string or structured value",
    &[
        "echo hello | replace l x",
        "open a.json | replace --path user.name old new"
    ],
    |args, input, _ctx| {
        let mut regex = false;
        let mut ignore_case = false;
        let mut recursive = false;
        let mut path: Option<String> = None;
        let mut pos = Vec::new();
        let mut i = 0usize;
        while i < args.len() {
            match args[i].as_str() {
                "--regex" => regex = true,
                "--ignore-case" => ignore_case = true,
                "--recursive" => recursive = true,
                "--path" => {
                    path = args.get(i + 1).cloned();
                    i += 1;
                }
                _ => pos.push(args[i].clone()),
            }
            i += 1;
        }
        if pos.len() < 2 {
            bail!("replace expects <from> <to>")
        }
        let value = pipeline_to_value(input)?;
        let out = apply_replace(
            value,
            &pos[0],
            &pos[1],
            regex,
            ignore_case,
            recursive,
            path.as_deref(),
        )?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);
simple_builtin!(
    ContainsBuiltin,
    "contains",
    "contains <needle>",
    "Check if text contains needle",
    &["echo hello | contains ell"],
    |args, input, _ctx| {
        let needle = args
            .first()
            .ok_or_else(|| anyhow!("contains expects needle"))?;
        let value = pipeline_to_value(input)?;
        let out = match value {
            Value::List(items) => {
                let scalar = parse_scalar(needle);
                Value::Bool(
                    items
                        .iter()
                        .any(|x| x == &scalar || x.to_string() == *needle),
                )
            }
            other => Value::Bool(other.to_string().contains(needle)),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);
simple_builtin!(
    StartsWithBuiltin,
    "starts-with",
    "starts-with <prefix>",
    "Check text prefix",
    &["echo hello | starts-with he"],
    |args, input, _ctx| {
        let p = args
            .first()
            .ok_or_else(|| anyhow!("starts-with expects prefix"))?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(
            input.into_text().starts_with(p),
        ))))
    }
);
simple_builtin!(
    EndsWithBuiltin,
    "ends-with",
    "ends-with <suffix>",
    "Check text suffix",
    &["echo hello | ends-with lo"],
    |args, input, _ctx| {
        let p = args
            .first()
            .ok_or_else(|| anyhow!("ends-with expects suffix"))?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(
            input.into_text().ends_with(p),
        ))))
    }
);
simple_builtin!(
    MatchBuiltin,
    "match",
    "match <regex>",
    "Regex match and capture list",
    &["echo hello123 | match [a-z]+\\d+"],
    |args, input, _ctx| {
        let pattern = args
            .first()
            .ok_or_else(|| anyhow!("match expects regex pattern"))?;
        let re = Regex::new(pattern)?;
        let text = input.into_text();
        let out = re
            .captures_iter(&text)
            .map(|cap| {
                Value::String(
                    cap.get(0)
                        .map(|m| m.as_str())
                        .unwrap_or_default()
                        .to_string(),
                )
            })
            .collect::<Vec<_>>();
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
    }
);
simple_builtin!(
    ParseBuiltin,
    "parse",
    "parse <type>",
    "Parse text into typed value: int|float|bool|json|yaml|toml",
    &["echo 42 | parse int"],
    |args, input, _ctx| {
        let kind = args.first().ok_or_else(|| anyhow!("parse expects type"))?;
        let text = input.into_text();
        let value = match kind.as_str() {
            "int" => Value::Int(text.trim().parse::<i64>()?),
            "float" => Value::Float(text.trim().parse::<f64>()?),
            "bool" => Value::Bool(text.trim().parse::<bool>()?),
            "json" => from_json_str(&text)?,
            "yaml" => from_yaml_str(&text)?,
            "toml" => from_toml_str(&text)?,
            _ => bail!("unsupported parse type: {kind}"),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(value)))
    }
);
simple_builtin!(
    FormatBuiltin,
    "format",
    "format <template>",
    "Format template using {value}",
    &["echo dosh | format 'name={value}'"],
    |args, input, _ctx| {
        let tpl = args
            .first()
            .ok_or_else(|| anyhow!("format expects template"))?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            tpl.replace("{value}", &input.into_text()),
        )))
    }
);
