use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::simple_builtin;
use anyhow::anyhow;
use regex::Regex;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(UpperBuiltin),
        factory!(LowerBuiltin),
        factory!(TitleBuiltin),
        factory!(CapitalizeBuiltin),
        factory!(WordsBuiltin),
        factory!(ExtractBuiltin),
        factory!(RepeatBuiltin),
        factory!(PadLeftBuiltin),
        factory!(PadRightBuiltin),
        factory!(IsEmailBuiltin),
        factory!(IsUrlBuiltin),
        factory!(ToIntBuiltin),
        factory!(ToFloatBuiltin),
        factory!(ToBoolBuiltin),
        factory!(TrimStartBuiltin),
        factory!(TrimEndBuiltin),
    ]
}

simple_builtin!(
    UpperBuiltin,
    "upper",
    "upper",
    "Uppercase string/list<string>",
    &["\"hello\" | upper"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(map_strings(
            pipeline_to_value(input)?,
            |s| s.to_uppercase(),
        ))))
    }
);

simple_builtin!(
    LowerBuiltin,
    "lower",
    "lower",
    "Lowercase string/list<string>",
    &["\"HELLO\" | lower"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(map_strings(
            pipeline_to_value(input)?,
            |s| s.to_lowercase(),
        ))))
    }
);

simple_builtin!(
    TitleBuiltin,
    "title",
    "title",
    "Title-case string/list<string>",
    &["\"hello world\" | title"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(map_strings(
            pipeline_to_value(input)?,
            title_case,
        ))))
    }
);

simple_builtin!(
    CapitalizeBuiltin,
    "capitalize",
    "capitalize",
    "Capitalize first letter",
    &["\"hello world\" | capitalize"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(map_strings(
            pipeline_to_value(input)?,
            capitalize_first,
        ))))
    }
);

simple_builtin!(
    WordsBuiltin,
    "words",
    "words",
    "Split text into words",
    &["\"hello world\" | words"],
    |_args, input, _ctx| {
        let text = input.into_text();
        let out = text
            .split_whitespace()
            .map(|s| Value::String(s.to_string()))
            .collect::<Vec<_>>();
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
    }
);

simple_builtin!(
    ExtractBuiltin,
    "extract",
    "extract <regex>",
    "Extract first regex match",
    &["\"abc123\" | extract \"\\\\d+\""],
    |args, input, _ctx| {
        let pattern = args
            .first()
            .ok_or_else(|| anyhow!("extract expects regex pattern"))?;
        let re = Regex::new(pattern)?;
        let text = input.into_text();
        let out = re
            .find(&text)
            .map(|m| Value::String(m.as_str().to_string()))
            .unwrap_or(Value::Null);
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    RepeatBuiltin,
    "repeat",
    "repeat <n>",
    "Repeat string n times",
    &["\"hello\" | repeat 3"],
    |args, input, _ctx| {
        let n = args
            .first()
            .ok_or_else(|| anyhow!("repeat expects count"))?
            .parse::<usize>()?;
        let out = map_strings(pipeline_to_value(input)?, |s| s.repeat(n));
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    PadLeftBuiltin,
    "pad-left",
    "pad-left <width> [char]",
    "Left-pad string",
    &["\"hello\" | pad-left 10"],
    |args, input, _ctx| {
        let width = args
            .first()
            .ok_or_else(|| anyhow!("pad-left expects width"))?
            .parse::<usize>()?;
        let ch = args.get(1).and_then(|s| s.chars().next()).unwrap_or(' ');
        let out = map_strings(pipeline_to_value(input)?, |s| {
            let len = s.chars().count();
            if len >= width {
                return s.to_string();
            }
            format!("{}{}", ch.to_string().repeat(width - len), s)
        });
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    PadRightBuiltin,
    "pad-right",
    "pad-right <width> [char]",
    "Right-pad string",
    &["\"hello\" | pad-right 10"],
    |args, input, _ctx| {
        let width = args
            .first()
            .ok_or_else(|| anyhow!("pad-right expects width"))?
            .parse::<usize>()?;
        let ch = args.get(1).and_then(|s| s.chars().next()).unwrap_or(' ');
        let out = map_strings(pipeline_to_value(input)?, |s| {
            let len = s.chars().count();
            if len >= width {
                return s.to_string();
            }
            format!("{}{}", s, ch.to_string().repeat(width - len))
        });
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    IsEmailBuiltin,
    "is-email",
    "is-email",
    "Validate email shape",
    &["\"user@example.com\" | is-email"],
    |_args, input, _ctx| {
        let text = input.into_text();
        let ok = Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$")?.is_match(text.trim());
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(ok))))
    }
);

simple_builtin!(
    IsUrlBuiltin,
    "is-url",
    "is-url",
    "Validate URL shape",
    &["\"https://dosh.dev\" | is-url"],
    |_args, input, _ctx| {
        let text = input.into_text();
        let ok = Regex::new(r"^(https?://)[^\s/$.?#].[^\s]*$")?.is_match(text.trim());
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(ok))))
    }
);

simple_builtin!(
    ToIntBuiltin,
    "to-int",
    "to-int",
    "Parse to int",
    &["\"42\" | to-int"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Int(
            input.into_text().trim().parse::<i64>()?,
        ))))
    }
);

simple_builtin!(
    ToFloatBuiltin,
    "to-float",
    "to-float",
    "Parse to float",
    &["\"3.14\" | to-float"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Float(
            input.into_text().trim().parse::<f64>()?,
        ))))
    }
);

simple_builtin!(
    ToBoolBuiltin,
    "to-bool",
    "to-bool",
    "Parse to bool",
    &["\"true\" | to-bool"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(
            input.into_text().trim().parse::<bool>()?,
        ))))
    }
);

simple_builtin!(
    TrimStartBuiltin,
    "trim-start",
    "trim-start",
    "Trim start whitespace",
    &["\" hello\" | trim-start"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(map_strings(
            pipeline_to_value(input)?,
            |s| s.trim_start().to_string(),
        ))))
    }
);

simple_builtin!(
    TrimEndBuiltin,
    "trim-end",
    "trim-end",
    "Trim end whitespace",
    &["\"hello \" | trim-end"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(map_strings(
            pipeline_to_value(input)?,
            |s| s.trim_end().to_string(),
        ))))
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

fn capitalize_first(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), ch.as_str()),
        None => String::new(),
    }
}

fn title_case(s: &str) -> String {
    let mut out = Vec::new();
    for w in s.split_whitespace() {
        out.push(capitalize_first(&w.to_lowercase()));
    }
    if out.is_empty() {
        s.to_string()
    } else {
        out.join(" ")
    }
}
