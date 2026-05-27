use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::simple_builtin;
use anyhow::{anyhow, bail};

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(PrependBuiltin),
        factory!(InsertAtBuiltin),
        factory!(RemoveAtBuiltin),
        factory!(UniqueBuiltin),
        factory!(SortBuiltin),
    ]
}

simple_builtin!(
    PrependBuiltin,
    "prepend",
    "prepend <value>",
    "Prepend value to list",
    &["[1,2,3] | prepend 0"],
    |args, input, _ctx| {
        let item = parse_arg_value(args, 0)?;
        let mut list = to_list(pipeline_to_value(input)?)?;
        list.insert(0, item);
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(list))))
    }
);

simple_builtin!(
    InsertAtBuiltin,
    "insert-at",
    "insert-at <index> <value>",
    "Insert value at list index",
    &["[1,2,3] | insert-at 1 99"],
    |args, input, _ctx| {
        let idx = args
            .get(0)
            .ok_or_else(|| anyhow!("insert-at expects index"))?
            .parse::<usize>()?;
        let item = parse_arg_value(args, 1)?;
        let mut list = to_list(pipeline_to_value(input)?)?;
        if idx > list.len() {
            bail!("index out of range")
        }
        list.insert(idx, item);
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(list))))
    }
);

simple_builtin!(
    RemoveAtBuiltin,
    "remove-at",
    "remove-at <index>",
    "Remove value at list index",
    &["[1,2,3] | remove-at 1"],
    |args, input, _ctx| {
        let idx = args
            .get(0)
            .ok_or_else(|| anyhow!("remove-at expects index"))?
            .parse::<usize>()?;
        let mut list = to_list(pipeline_to_value(input)?)?;
        if idx >= list.len() {
            bail!("index out of range")
        }
        list.remove(idx);
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(list))))
    }
);

simple_builtin!(
    UniqueBuiltin,
    "unique",
    "unique",
    "Unique list values",
    &["[1,2,2,3] | unique"],
    |_args, input, _ctx| {
        let list = to_list(pipeline_to_value(input)?)?;
        let mut out: Vec<Value> = Vec::new();
        for v in list {
            if !out.iter().any(|x| x == &v) {
                out.push(v);
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
    }
);

simple_builtin!(
    SortBuiltin,
    "sort",
    "sort",
    "Sort scalar list",
    &["[3,1,2] | sort"],
    |_args, input, _ctx| {
        let mut list = to_list(pipeline_to_value(input)?)?;
        list.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(list))))
    }
);

fn to_list(v: Value) -> anyhow::Result<Vec<Value>> {
    match v {
        Value::List(items) => Ok(items),
        Value::Table(t) => Ok(t.rows.into_iter().map(Value::Record).collect()),
        _ => bail!("expected list input"),
    }
}

fn parse_arg_value(args: &[String], idx: usize) -> anyhow::Result<Value> {
    let raw = args
        .get(idx)
        .ok_or_else(|| anyhow!("missing value argument"))?;
    Ok(parse_scalar(raw))
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
