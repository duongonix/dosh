use super::*;
use crate::helpers::{compare_ord, pipeline_to_value};
use crate::registry::{factory, simple_builtin};
use crate::render::{TableRenderOptions, render_value_as_table};
use crate::stream::{RowOp, RowStream};
use anyhow::{anyhow, bail};
use dosh_value::{
    from_json_str, from_toml_str, from_yaml_str, parse_filter_expr, to_json_string, to_toml_string,
    to_yaml_string,
};

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(FromJsonBuiltin),
        factory!(ToJsonBuiltin),
        factory!(FromYamlBuiltin),
        factory!(ToYamlBuiltin),
        factory!(FromTomlBuiltin),
        factory!(ToTomlBuiltin),
        factory!(GetBuiltin),
        factory!(SelectBuiltin),
        factory!(RejectBuiltin),
        factory!(WhereBuiltin),
        factory!(FilterBuiltin),
        factory!(EachBuiltin),
        factory!(MapBuiltin),
        factory!(ReduceBuiltin),
        factory!(SortByBuiltin),
        factory!(GroupByBuiltin),
        factory!(CountBuiltin),
        factory!(LengthBuiltin),
        factory!(FirstBuiltin),
        factory!(LastBuiltin),
        factory!(SliceBuiltin),
        factory!(SkipBuiltin),
        factory!(TakeBuiltin),
        factory!(ReverseBuiltin),
        factory!(FlattenBuiltin),
        factory!(TransposeBuiltin),
        factory!(MergeBuiltin),
        factory!(JoinBuiltin),
        factory!(InsertBuiltin),
        factory!(UpdateBuiltin),
        factory!(RenameBuiltin),
        factory!(DropBuiltin),
        factory!(HasBuiltin),
        factory!(KeysBuiltin),
        factory!(ValuesBuiltin),
        factory!(InspectBuiltin),
        factory!(PipelineBuiltin),
        factory!(TableBuiltin),
        factory!(SheetBuiltin),
        factory!(QueryBuiltin),
    ]
}

simple_builtin!(
    FromJsonBuiltin,
    "from-json",
    "from-json",
    "Parse JSON from text",
    &["echo '{\"a\":1}' | from-json"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(from_json_str(
            &input.into_text(),
        )?)))
    }
);
simple_builtin!(
    ToJsonBuiltin,
    "to-json",
    "to-json",
    "Render input as JSON",
    &["open x.toml | to-json"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(to_json_string(
            &pipeline_to_value(input)?,
        )?)))
    }
);
simple_builtin!(
    FromYamlBuiltin,
    "from-yaml",
    "from-yaml",
    "Parse YAML from text",
    &["cat x.yaml | from-yaml"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(from_yaml_str(
            &input.into_text(),
        )?)))
    }
);
simple_builtin!(
    ToYamlBuiltin,
    "to-yaml",
    "to-yaml",
    "Render input as YAML",
    &["open x.json | to-yaml"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(to_yaml_string(
            &pipeline_to_value(input)?,
        )?)))
    }
);
simple_builtin!(
    FromTomlBuiltin,
    "from-toml",
    "from-toml",
    "Parse TOML from text",
    &["cat Cargo.toml | from-toml"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(from_toml_str(
            &input.into_text(),
        )?)))
    }
);
simple_builtin!(
    ToTomlBuiltin,
    "to-toml",
    "to-toml",
    "Render input as TOML",
    &["open x.json | to-toml"],
    |_args, input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(to_toml_string(
            &pipeline_to_value(input)?,
        )?)))
    }
);

simple_builtin!(
    GetBuiltin,
    "get",
    "get <path>",
    "Get field by dotted path",
    &["open package.json | get scripts"],
    |args, input, _ctx| {
        let path = args.first().ok_or_else(|| anyhow!("get expects path"))?;
        let value = unwrap_doc_value(pipeline_to_value(input)?);
        let out = match value {
            Value::Record(_) => value.get_path(path).cloned().unwrap_or(Value::Null),
            Value::Table(table) => Value::List(
                table
                    .rows
                    .iter()
                    .map(|r| {
                        Value::Record(r.clone())
                            .get_path(path)
                            .cloned()
                            .unwrap_or(Value::Null)
                    })
                    .collect(),
            ),
            _ => Value::Null,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    SelectBuiltin,
    "select",
    "select <col...>",
    "Project table/record columns",
    &["ls | select name size"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("select expects fields")
        }
        let value = pipeline_to_value(input.clone())?;
        if let Value::Record(r) = value {
            let mut out = Record::new();
            for field in args {
                if let Some(v) = r.get(field) {
                    out.insert(field.clone(), v.clone());
                }
            }
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(out))));
        }
        let mut s = into_stream(input)?;
        s.push_op(RowOp::Select(args.to_vec()));
        Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
    }
);

simple_builtin!(
    RejectBuiltin,
    "reject",
    "reject <col...>",
    "Remove columns from records/tables",
    &["ls | reject modified"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("reject expects fields")
        }
        let value = pipeline_to_value(input.clone())?;
        if let Value::Record(mut r) = value {
            for field in args {
                r.shift_remove(field);
            }
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(r))));
        }
        let mut s = into_stream(input)?;
        s.push_op(RowOp::Reject(args.to_vec()));
        Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
    }
);

simple_builtin!(
    WhereBuiltin,
    "where",
    "where <expr>",
    "Filter rows with expression AST",
    &["ls | where size > 1mb"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("where expects expression")
        }
        if matches!(
            args.first().map(|s| s.as_str()),
            Some("contains" | "starts-with" | "ends-with")
        ) && args.len() >= 2
        {
            let op = args[0].as_str();
            let needle = args[1..].join(" ").trim_matches('"').to_string();
            let items = value_to_items(pipeline_to_value(input)?);
            let out = items
                .into_iter()
                .filter(|v| {
                    let s = v.to_string();
                    match op {
                        "contains" => s.contains(&needle),
                        "starts-with" => s.starts_with(&needle),
                        "ends-with" => s.ends_with(&needle),
                        _ => false,
                    }
                })
                .collect::<Vec<_>>();
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))));
        }
        let expr_text = args.join(" ");
        let expr = parse_filter_expr(&expr_text)?;
        let mut s = into_stream(input)?;
        s.push_op(RowOp::Filter(expr));
        Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
    }
);

simple_builtin!(
    FilterBuiltin,
    "filter",
    "filter <expr>|{ expr }",
    "Filter rows/items with expression or closure body",
    &["ps | filter pid > 1000", "[1,2,3] | filter { $it > 1 }"],
    |args, input, ctx| {
        let joined = args.join(" ");
        if let Some(body) = parse_brace_body(&joined) {
            let items = value_to_items(pipeline_to_value(input)?);
            let mut out = Vec::new();
            for item in items {
                let keep = eval_expr(&body, &[("it", &item)])?;
                if keep.is_truthy() {
                    out.push(item);
                }
            }
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))));
        }
        WhereBuiltin.run(args, input, ctx)
    }
);

simple_builtin!(
    EachBuiltin,
    "each",
    "each <field>|{|it| expr}",
    "Project field or map with closure-style expression",
    &["ls | each name", "['foo','bar'] | each {|s| '~/' ++ $s}"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("each expects field path or closure")
        }
        let joined = args.join(" ");
        if let Some((param, body)) = parse_closure_syntax(&joined) {
            let items = value_to_items(pipeline_to_value(input)?);
            let out = items
                .iter()
                .map(|it| eval_closure_expr(body, param, it))
                .collect::<anyhow::Result<Vec<_>>>()?;
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))));
        } else if let Some(body) = parse_brace_body(&joined) {
            let items = value_to_items(pipeline_to_value(input)?);
            let out = items
                .iter()
                .map(|it| eval_expr(&body, &[("it", it)]))
                .collect::<anyhow::Result<Vec<_>>>()?;
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))));
        }
        let field = args
            .first()
            .ok_or_else(|| anyhow!("each expects field path"))?;
        let mut s = into_stream(input)?;
        s.push_op(RowOp::MapField(field.clone()));
        Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
    }
);

simple_builtin!(
    MapBuiltin,
    "map",
    "map <field>",
    "Alias of each",
    &["ls | map name"],
    |args, input, ctx| EachBuiltin.run(args, input, ctx)
);

simple_builtin!(
    ReduceBuiltin,
    "reduce",
    "reduce <field> <op> | reduce -f <init> {|elt, acc| expr}",
    "Reduce values with built-in ops or closure-style expression",
    &[
        "ps | reduce memory sum",
        "1..10 | reduce -f '' {|elt, acc| $acc + $elt}"
    ],
    |args, input, _ctx| {
        if let Some((init, expr)) = parse_reduce_closure_args(args) {
            let items = value_to_items(pipeline_to_value(input)?);
            let (elt_name, acc_name, body) = expr;
            let mut acc = init;
            for item in items {
                acc = eval_reduce_expr(&body, &elt_name, &acc_name, &item, &acc)?;
            }
            return Ok(BuiltinOutcome::ok(PipelineData::Value(acc)));
        } else if let Some((init, body)) = parse_reduce_brace_args(args) {
            let items = value_to_items(pipeline_to_value(input)?);
            let mut acc = init;
            for item in items {
                acc = eval_expr(&body, &[("it", &item), ("acc", &acc)])?;
            }
            return Ok(BuiltinOutcome::ok(PipelineData::Value(acc)));
        }
        if args.len() < 2 {
            bail!("reduce expects <field> <op>")
        }
        let field = &args[0];
        let op = &args[1];
        let vals = to_rows(pipeline_to_value(input)?)
            .into_iter()
            .filter_map(|r| Value::Record(r).get_path(field).cloned())
            .collect::<Vec<_>>();
        let out = match op.as_str() {
            "sum" => Value::Int(
                vals.iter()
                    .map(|v| if let Value::Int(i) = v { *i } else { 0 })
                    .sum(),
            ),
            "min" => vals
                .into_iter()
                .min_by(|a, b| compare_ord(Some(a), Some(b)))
                .unwrap_or(Value::Null),
            "max" => vals
                .into_iter()
                .max_by(|a, b| compare_ord(Some(a), Some(b)))
                .unwrap_or(Value::Null),
            _ => bail!("reduce op must be one of: sum|min|max"),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    SortByBuiltin,
    "sort-by",
    "sort-by <field>",
    "Sort rows by field",
    &["ps | sort-by cpu"],
    |args, input, _ctx| {
        let field = args
            .first()
            .ok_or_else(|| anyhow!("sort-by expects field"))?;
        let mut s = into_stream(input)?;
        s.push_op(RowOp::SortBy(field.clone()));
        Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
    }
);

simple_builtin!(
    GroupByBuiltin,
    "group-by",
    "group-by <field>",
    "Group rows by field",
    &["ls | group-by type"],
    |args, input, _ctx| {
        let field = args
            .first()
            .ok_or_else(|| anyhow!("group-by expects field"))?;
        let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for row in to_rows(pipeline_to_value(input)?) {
            let key = Value::Record(row.clone())
                .get_path(field)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string());
            groups.entry(key).or_default().push(Value::Record(row));
        }
        let mut out = Record::new();
        for (k, v) in groups {
            out.insert(k, Value::List(v));
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(out))))
    }
);

simple_builtin!(
    CountBuiltin,
    "count",
    "count [field]",
    "Count rows or by field",
    &["ls | count"],
    |args, input, _ctx| {
        let rows = into_stream(input)?.materialize_rows();
        if args.is_empty() {
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Int(
                rows.len() as i64,
            ))));
        }
        let field = &args[0];
        let mut counts: BTreeMap<String, i64> = BTreeMap::new();
        for row in rows {
            let key = Value::Record(row)
                .get_path(field)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string());
            *counts.entry(key).or_insert(0) += 1;
        }
        let mut rec = Record::new();
        for (k, v) in counts {
            rec.insert(k, Value::Int(v));
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    LengthBuiltin,
    "length",
    "length",
    "Count values",
    &["ls | length"],
    |_args, input, _ctx| {
        let n = match materialize_pipeline_value(input)? {
            Value::List(v) => v.len() as i64,
            Value::Table(t) => t.rows.len() as i64,
            Value::Record(r) => r.len() as i64,
            Value::String(s) => s.len() as i64,
            _ => 0,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Int(n))))
    }
);

simple_builtin!(
    FirstBuiltin,
    "first",
    "first [n]",
    "First item/row or first n items/rows",
    &["ls | first", "ls | first 5"],
    |args, input, _ctx| {
        let n = args.first().and_then(|v| v.parse::<usize>().ok());
        let out = match materialize_pipeline_value(input)? {
            Value::List(v) => {
                if let Some(n) = n {
                    Value::List(v.into_iter().take(n).collect())
                } else {
                    v.into_iter().next().unwrap_or(Value::Null)
                }
            }
            Value::Table(t) => {
                if let Some(n) = n {
                    Value::Table(Table::new(t.rows.into_iter().take(n).collect()))
                } else {
                    t.rows
                        .first()
                        .cloned()
                        .map(Value::Record)
                        .unwrap_or(Value::Null)
                }
            }
            other => other,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    LastBuiltin,
    "last",
    "last [n]",
    "Last item/row or last n items/rows",
    &["ls | last", "ls | last 5"],
    |args, input, _ctx| {
        let n = args.first().and_then(|v| v.parse::<usize>().ok());
        let out = match materialize_pipeline_value(input)? {
            Value::List(v) => {
                if let Some(n) = n {
                    let len = v.len();
                    Value::List(v.into_iter().skip(len.saturating_sub(n)).collect())
                } else {
                    v.last().cloned().unwrap_or(Value::Null)
                }
            }
            Value::Table(t) => {
                if let Some(n) = n {
                    let len = t.rows.len();
                    Value::Table(Table::new(
                        t.rows.into_iter().skip(len.saturating_sub(n)).collect(),
                    ))
                } else {
                    t.rows
                        .last()
                        .cloned()
                        .map(Value::Record)
                        .unwrap_or(Value::Null)
                }
            }
            other => other,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    SliceBuiltin,
    "slice",
    "slice <start> <end>",
    "Slice items/rows by index range [start, end)",
    &["ls | slice 2 6"],
    |args, input, _ctx| {
        if args.len() != 2 {
            bail!("slice expects <start> <end>")
        }
        let start = args[0]
            .parse::<usize>()
            .map_err(|_| anyhow!("slice expects integer <start>"))?;
        let end = args[1]
            .parse::<usize>()
            .map_err(|_| anyhow!("slice expects integer <end>"))?;
        if end < start {
            bail!("slice requires end >= start");
        }
        let out = match materialize_pipeline_value(input)? {
            Value::List(v) => Value::List(v.into_iter().skip(start).take(end - start).collect()),
            Value::Table(t) => Value::Table(Table::new(
                t.rows.into_iter().skip(start).take(end - start).collect(),
            )),
            Value::String(s) => Value::String(
                s.chars()
                    .skip(start)
                    .take(end.saturating_sub(start))
                    .collect(),
            ),
            other => other,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    SkipBuiltin,
    "skip",
    "skip <n>",
    "Skip first n",
    &["ls | skip 5"],
    |args, input, _ctx| {
        let n = args
            .first()
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| anyhow!("skip expects integer n"))?;
        let mut s = into_stream(input)?;
        s.push_op(RowOp::Skip(n));
        Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
    }
);

simple_builtin!(
    TakeBuiltin,
    "take",
    "take <n>",
    "Take first n",
    &["ls | take 10"],
    |args, input, _ctx| {
        let n = args
            .first()
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| anyhow!("take expects integer n"))?;
        let mut s = into_stream(input)?;
        s.push_op(RowOp::Take(n));
        Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
    }
);

simple_builtin!(
    ReverseBuiltin,
    "reverse",
    "reverse",
    "Reverse rows/items",
    &["ls | reverse"],
    |_args, input, _ctx| {
        let value = materialize_pipeline_value(input)?;
        match value {
            Value::String(s) => Ok(BuiltinOutcome::ok(PipelineData::Value(Value::String(
                s.chars().rev().collect(),
            )))),
            other => {
                let mut s = into_stream(PipelineData::Value(other))?;
                s.push_op(RowOp::Reverse);
                Ok(BuiltinOutcome::ok(PipelineData::RowStream(s)))
            }
        }
    }
);

simple_builtin!(
    FlattenBuiltin,
    "flatten",
    "flatten",
    "Flatten list by one level",
    &["open data.json | flatten"],
    |_args, input, _ctx| {
        let out = match pipeline_to_value(input)? {
            Value::List(v) => {
                let mut flat = Vec::new();
                for item in v {
                    match item {
                        Value::List(inner) => flat.extend(inner),
                        other => flat.push(other),
                    }
                }
                Value::List(flat)
            }
            other => other,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    TransposeBuiltin,
    "transpose",
    "transpose",
    "Transpose record to key/value table",
    &["open package.json | transpose"],
    |_args, input, _ctx| {
        let out = match pipeline_to_value(input)? {
            Value::Record(r) => {
                let mut rows = Vec::new();
                for (k, v) in r {
                    let mut row = Record::new();
                    row.insert("key".into(), Value::String(k));
                    row.insert("value".into(), v);
                    rows.push(row);
                }
                Value::Table(Table::new(rows))
            }
            other => other,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    MergeBuiltin,
    "merge",
    "merge <record>",
    "Merge input record with another",
    &[
        "open a.json | merge '{\"env\":\"dev\"}'",
        "{name:\"dosh\"} | merge {age: 1}"
    ],
    |args, input, _ctx| {
        let rhs = args.join(" ");
        let rhs_val = parse_inline_value(rhs.trim())?;
        let out = match (pipeline_to_value(input)?, rhs_val) {
            (Value::Record(mut l), Value::Record(r)) => {
                for (k, v) in r {
                    l.insert(k, v);
                }
                Value::Record(l)
            }
            (l, _) => l,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    JoinBuiltin,
    "join",
    "join <sep>",
    "Join list into string",
    &["ls | each name | join ', '"],
    |args, input, _ctx| {
        let sep = args.first().cloned().unwrap_or_else(|| " ".to_string());
        let out = match pipeline_to_value(input)? {
            Value::List(v) => Value::String(
                v.into_iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(&sep),
            ),
            other => other,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    InsertBuiltin,
    "insert",
    "insert <field> <value>",
    "Insert field into record/table",
    &["ls | insert source local", "rows | insert id { $index }"],
    |args, input, _ctx| {
        if args.len() < 2 {
            bail!("insert expects <field> <value>")
        }
        let field = args[0].clone();
        let raw_value = args[1..].join(" ");
        let out = match pipeline_to_value(input)? {
            Value::Record(mut r) => {
                let val = parse_insert_value(&raw_value, 0)?;
                r.insert(field, val);
                Value::Record(r)
            }
            Value::Table(mut t) => {
                for (idx, row) in t.rows.iter_mut().enumerate() {
                    let val = parse_insert_value(&raw_value, idx)?;
                    row.insert(field.clone(), val.clone());
                }
                Value::Table(t)
            }
            _ => Value::Null,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    UpdateBuiltin,
    "update",
    "update [--insert|--upsert] <field.path> <value>",
    "Update existing field path in record/table/docs",
    &["ls | update name renamed"],
    |args, input, _ctx| {
        if args.len() < 2 {
            bail!("update expects <field> <value>")
        }
        let mut insert = false;
        let mut pos = Vec::new();
        for arg in args {
            match arg.as_str() {
                "--insert" | "--upsert" => insert = true,
                _ => pos.push(arg.clone()),
            }
        }
        if pos.len() < 2 {
            bail!("update expects <field> <value>")
        }
        let field = pos[0].clone();
        let val = Value::String(pos[1..].join(" ").trim_matches('"').to_string());
        let out = match pipeline_to_value(input)? {
            Value::Record(mut r) => {
                update_path_record(&mut r, &field, val, insert);
                Value::Record(r)
            }
            Value::Table(mut t) => {
                for row in &mut t.rows {
                    update_path_record(row, &field, val.clone(), insert);
                }
                Value::Table(t)
            }
            Value::List(mut docs) => {
                for doc in &mut docs {
                    if let Value::Record(rec) = doc {
                        if let Some(Value::Record(inner)) = rec.get_mut("_value") {
                            update_path_record(inner, &field, val.clone(), insert);
                            let dirty = rec.get("_original").cloned().unwrap_or(Value::Null)
                                != rec.get("_value").cloned().unwrap_or(Value::Null);
                            rec.insert("_dirty".into(), Value::Bool(dirty));
                        }
                    }
                }
                Value::List(docs)
            }
            _ => Value::Null,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    RenameBuiltin,
    "rename",
    "rename <old> <new>",
    "Rename field",
    &["ls | rename name file_name"],
    |args, input, _ctx| {
        if args.len() != 2 {
            bail!("rename expects <old> <new>")
        }
        let old = &args[0];
        let new = &args[1];
        let out = match pipeline_to_value(input)? {
            Value::Record(mut r) => {
                if let Some(v) = r.shift_remove(old) {
                    r.insert(new.clone(), v);
                }
                Value::Record(r)
            }
            Value::Table(mut t) => {
                for row in &mut t.rows {
                    if let Some(v) = row.shift_remove(old) {
                        row.insert(new.clone(), v);
                    }
                }
                Value::Table(t)
            }
            other => other,
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    DropBuiltin,
    "drop",
    "drop <n|field...>",
    "Drop first n rows/items or drop fields from record/table",
    &["ls | drop 3", "{name:\"dosh\",age:1} | drop age"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("drop expects <n> or <field...>")
        }
        let value = pipeline_to_value(input)?;
        let out = if let Ok(n) = args[0].parse::<usize>() {
            match value {
                Value::List(v) => Value::List(v.into_iter().skip(n).collect()),
                Value::Table(t) => Value::Table(Table::new(t.rows.into_iter().skip(n).collect())),
                other => other,
            }
        } else {
            match value {
                Value::Record(mut r) => {
                    for f in args {
                        r.shift_remove(f);
                    }
                    Value::Record(r)
                }
                Value::Table(mut t) => {
                    for row in &mut t.rows {
                        for f in args {
                            row.shift_remove(f);
                        }
                    }
                    Value::Table(t)
                }
                other => other,
            }
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    HasBuiltin,
    "has",
    "has <field.path>",
    "Check whether record contains field path",
    &[
        "{name:\"dosh\"} | has name",
        "{meta:{lang:\"rust\"}} | has meta.lang"
    ],
    |args, input, _ctx| {
        let path = args
            .first()
            .ok_or_else(|| anyhow!("has expects field path"))?;
        let out = match pipeline_to_value(input)? {
            Value::Record(r) => Value::Bool(Value::Record(r).get_path(path).is_some()),
            Value::Table(t) => Value::Bool(
                t.rows
                    .first()
                    .map(|r| Value::Record(r.clone()).get_path(path).is_some())
                    .unwrap_or(false),
            ),
            _ => Value::Bool(false),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    KeysBuiltin,
    "keys",
    "keys",
    "List keys from record or table columns",
    &["open package.json | keys", "ls | keys"],
    |_args, input, _ctx| {
        let out = match materialize_pipeline_value(input)? {
            Value::Record(r) => Value::List(r.keys().cloned().map(Value::String).collect()),
            Value::Table(t) => Value::List(t.columns.into_iter().map(Value::String).collect()),
            _ => Value::List(Vec::new()),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    ValuesBuiltin,
    "values",
    "values",
    "List values from record or table row values",
    &["open package.json | values"],
    |_args, input, _ctx| {
        let out = match materialize_pipeline_value(input)? {
            Value::Record(r) => Value::List(r.into_values().collect()),
            Value::Table(t) => Value::List(
                t.rows
                    .into_iter()
                    .flat_map(|row| row.into_values().collect::<Vec<_>>())
                    .collect(),
            ),
            Value::List(v) => Value::List(v),
            v => Value::List(vec![v]),
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    InspectBuiltin,
    "inspect",
    "inspect",
    "Show input type/schema/sample rows",
    &["ls | inspect", "open data.json | inspect"],
    |_args, input, _ctx| {
        let value = materialize_pipeline_value(input)?;
        let mut rec = Record::new();
        rec.insert("type".into(), Value::String(value.type_name().to_string()));
        match &value {
            Value::Table(t) => {
                rec.insert("rows".into(), Value::Int(t.rows.len() as i64));
                rec.insert(
                    "columns".into(),
                    Value::List(t.columns.iter().cloned().map(Value::String).collect()),
                );
                rec.insert(
                    "sample".into(),
                    Value::List(t.rows.iter().take(3).cloned().map(Value::Record).collect()),
                );
            }
            Value::List(v) => {
                rec.insert("length".into(), Value::Int(v.len() as i64));
                rec.insert(
                    "sample".into(),
                    Value::List(v.iter().take(3).cloned().collect()),
                );
            }
            Value::Record(r) => {
                rec.insert(
                    "keys".into(),
                    Value::List(r.keys().cloned().map(Value::String).collect()),
                );
                rec.insert("length".into(), Value::Int(r.len() as i64));
            }
            _ => {}
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    PipelineBuiltin,
    "pipeline",
    "pipeline <inspect|trace>",
    "Pipeline diagnostics foundation",
    &[
        "ls | where size > 1mb | pipeline inspect",
        "ls | pipeline trace"
    ],
    |args, input, _ctx| {
        let sub = args.first().map(|s| s.as_str()).unwrap_or("inspect");
        let value = materialize_pipeline_value(input)?;
        match sub {
            "inspect" => InspectBuiltin.run(&[], PipelineData::Value(value), _ctx),
            "trace" => {
                let mut rec = Record::new();
                rec.insert("stage".into(), Value::String("pipeline.trace".into()));
                rec.insert(
                    "input_type".into(),
                    Value::String(value.type_name().to_string()),
                );
                rec.insert("truthy".into(), Value::Bool(value.is_truthy()));
                Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
            }
            _ => bail!("pipeline expects subcommand inspect|trace"),
        }
    }
);

simple_builtin!(
    QueryBuiltin,
    "query",
    "query <sql>",
    "Run SQL query from open sqlite document",
    &["open app.sqlite | query \"select * from users\""],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("query expects sql text")
        }
        let sql = args.join(" ");
        let value = materialize_pipeline_value(input)?;
        let (path, fmt) =
            doc_source_meta(&value).ok_or_else(|| anyhow!("query expects open sqlite document"))?;
        if fmt != "sqlite" {
            bail!("query expects sqlite source");
        }
        let loaded = crate::registry::file_pipeline_builtins::sqlite_query(
            std::path::Path::new(&path),
            &sql,
        )?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(loaded)))
    }
);

simple_builtin!(
    SheetBuiltin,
    "sheet",
    "sheet <name>",
    "Load xlsx sheet from open-doc",
    &["open users.xlsx | sheet Users"],
    |args, input, _ctx| {
        let sheet = args
            .first()
            .ok_or_else(|| anyhow!("sheet expects sheet name"))?;
        let value = materialize_pipeline_value(input)?;
        let (path, fmt) =
            doc_source_meta(&value).ok_or_else(|| anyhow!("sheet expects open xlsx document"))?;
        if fmt != "xlsx" {
            bail!("sheet expects xlsx source");
        }
        let loaded = crate::registry::file_pipeline_builtins::xlsx_sheet(
            std::path::Path::new(&path),
            sheet,
        )?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(loaded)))
    }
);

simple_builtin!(
    TableBuiltin,
    "table",
    "table [sqlite_table]",
    "Render value as aligned table or load sqlite table from open-doc",
    &["ls | table", "open app.sqlite | table users"],
    |args, input, _ctx| {
        let value = materialize_pipeline_value(input)?;
        if let Some(name) = args.first()
            && let Some((path, fmt)) = doc_source_meta(&value)
            && fmt == "sqlite"
        {
            let loaded = crate::registry::file_pipeline_builtins::sqlite_table(
                std::path::Path::new(&path),
                name,
            )?;
            return Ok(BuiltinOutcome::ok(PipelineData::Value(loaded)));
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            render_value_as_table(&value, TableRenderOptions::default()),
        )))
    }
);

fn into_stream(input: PipelineData) -> anyhow::Result<RowStream> {
    match input {
        PipelineData::RowStream(s) => Ok(s),
        other => Ok(RowStream::new(to_rows(pipeline_to_value(other)?))),
    }
}

fn materialize_pipeline_value(input: PipelineData) -> anyhow::Result<Value> {
    match input {
        PipelineData::RowStream(s) => {
            if let Some(vs) = s.materialize_mapped_values() {
                Ok(Value::List(vs))
            } else {
                Ok(s.materialize_value())
            }
        }
        other => pipeline_to_value(other),
    }
}

fn unwrap_doc_value(value: Value) -> Value {
    match value {
        Value::Record(mut r) => {
            if let Some(v) = r.shift_remove("_value") {
                v
            } else {
                Value::Record(r)
            }
        }
        other => other,
    }
}

fn doc_source_meta(value: &Value) -> Option<(String, String)> {
    let rec = value.as_record()?;
    let meta = rec.get("_meta")?.as_record()?;
    let path = meta.get("source_path")?.to_string();
    let fmt = meta.get("source_format")?.to_string();
    Some((path, fmt))
}

fn update_path_record(rec: &mut Record, path: &str, val: Value, insert: bool) {
    let parts = path
        .split('.')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        if insert || rec.contains_key(parts[0]) {
            rec.insert(parts[0].to_string(), val);
        }
        return;
    }
    let head = parts[0].to_string();
    let tail = parts[1..].join(".");
    let entry = rec
        .entry(head)
        .or_insert_with(|| Value::Record(Record::new()));
    if let Value::Record(inner) = entry {
        update_path_record(inner, &tail, val, insert);
    }
}

fn value_to_items(value: Value) -> Vec<Value> {
    match value {
        Value::List(v) => v,
        Value::Table(t) => t.rows.into_iter().map(Value::Record).collect(),
        v => vec![v],
    }
}

fn parse_closure_syntax(input: &str) -> Option<(&str, &str)> {
    let s = input.trim();
    let s = s.strip_prefix('{').unwrap_or(s);
    let s = s.strip_suffix('}').unwrap_or(s);
    let s = s.trim();
    if !s.starts_with('|') {
        return None;
    }
    let rest = &s[1..];
    let (params, body) = rest.split_once('|')?;
    let p = params
        .split(',')
        .next()?
        .trim()
        .trim_start_matches('$')
        .trim();
    if p.is_empty() {
        return None;
    }
    Some((p, body.trim()))
}

fn parse_brace_body(input: &str) -> Option<String> {
    let s = input.trim();
    if !(s.starts_with('{') && s.ends_with('}')) {
        return None;
    }
    Some(s[1..s.len() - 1].trim().to_string())
}

fn parse_reduce_closure_args(args: &[String]) -> Option<(Value, (String, String, String))> {
    let mut i = 0usize;
    let mut init: Option<Value> = None;
    while i < args.len() {
        if args[i] == "-f" || args[i] == "--fold" {
            init = args.get(i + 1).cloned().map(Value::String);
            break;
        }
        i += 1;
    }
    let init = init?;
    let joined = args.join(" ");
    let start = joined.find("{|").or_else(|| joined.find('|'))?;
    let s = joined[start..].trim();
    let s = s.strip_prefix('{').unwrap_or(s);
    let s = s.strip_suffix('}').unwrap_or(s);
    let s = s.trim();
    if !s.starts_with('|') {
        return None;
    }
    let rest = &s[1..];
    let (params, body) = rest.split_once('|')?;
    let mut it = params.split(',').map(|x| x.trim().trim_start_matches('$'));
    let elt = it.next()?.to_string();
    let acc = it.next()?.to_string();
    Some((init, (elt, acc, body.trim().to_string())))
}

fn parse_reduce_brace_args(args: &[String]) -> Option<(Value, String)> {
    if args.len() < 2 {
        return None;
    }
    let init = parse_literal_value(&args[0]);
    let body = parse_brace_body(&args[1..].join(" "))?;
    Some((init, body))
}

fn eval_closure_expr(body: &str, param: &str, item: &Value) -> anyhow::Result<Value> {
    eval_expr(body, &[(param, item)])
}

fn eval_reduce_expr(
    body: &str,
    elt: &str,
    acc: &str,
    item: &Value,
    acc_val: &Value,
) -> anyhow::Result<Value> {
    eval_expr(body, &[(elt, item), (acc, acc_val)])
}

fn eval_expr(body: &str, vars: &[(&str, &Value)]) -> anyhow::Result<Value> {
    let mut parser = ExprParser::new(body, vars);
    parser.parse_expression()
}

fn eval_binary(left: Value, right: Value, op: &str) -> anyhow::Result<Value> {
    match op {
        "==" => Ok(Value::Bool(left == right)),
        "!=" => Ok(Value::Bool(left != right)),
        ">" | ">=" | "<" | "<=" => {
            let (a, b) = (to_f64(&left)?, to_f64(&right)?);
            let ok = match op {
                ">" => a > b,
                ">=" => a >= b,
                "<" => a < b,
                "<=" => a <= b,
                _ => false,
            };
            Ok(Value::Bool(ok))
        }
        "++" => Ok(Value::String(format!("{}{}", left, right))),
        "+" => numeric_or_concat(left, right, |a, b| a + b),
        "-" => numeric_only(left, right, |a, b| a - b),
        "*" => numeric_only(left, right, |a, b| a * b),
        "/" => numeric_only(left, right, |a, b| a / b),
        "%" => numeric_only(left, right, |a, b| a % b),
        _ => bail!("unsupported operator: {op}"),
    }
}

fn numeric_or_concat(
    left: Value,
    right: Value,
    f: impl Fn(f64, f64) -> f64,
) -> anyhow::Result<Value> {
    match (to_f64_opt(&left), to_f64_opt(&right)) {
        (Some(a), Some(b)) => Ok(Value::Float(f(a, b))),
        _ => Ok(Value::String(format!("{}{}", left, right))),
    }
}

fn numeric_only(left: Value, right: Value, f: impl Fn(f64, f64) -> f64) -> anyhow::Result<Value> {
    let a = to_f64(&left)?;
    let b = to_f64(&right)?;
    Ok(Value::Float(f(a, b)))
}

fn to_f64(v: &Value) -> anyhow::Result<f64> {
    to_f64_opt(v).ok_or_else(|| anyhow!("expected numeric value, got {}", v.type_name()))
}

fn to_f64_opt(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_literal_value(atom: &str) -> Value {
    if atom.len() >= 2
        && ((atom.starts_with('"') && atom.ends_with('"'))
            || (atom.starts_with('\'') && atom.ends_with('\'')))
    {
        return Value::String(atom[1..atom.len() - 1].to_string());
    }
    if let Ok(i) = atom.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = atom.parse::<f64>() {
        return Value::Float(f);
    }
    if let Ok(b) = atom.parse::<bool>() {
        return Value::Bool(b);
    }
    Value::String(atom.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExprToken {
    LParen,
    RParen,
    Op(String),
    Atom(String),
}

struct ExprParser<'a> {
    tokens: Vec<ExprToken>,
    pos: usize,
    vars: &'a [(&'a str, &'a Value)],
}

impl<'a> ExprParser<'a> {
    fn new(input: &str, vars: &'a [(&'a str, &'a Value)]) -> Self {
        Self {
            tokens: tokenize_expr(input),
            pos: 0,
            vars,
        }
    }

    fn parse_expression(&mut self) -> anyhow::Result<Value> {
        self.parse_cmp()
    }

    fn parse_cmp(&mut self) -> anyhow::Result<Value> {
        let mut left = self.parse_add()?;
        while let Some(op) = self.peek_op(&["==", "!=", ">=", "<=", ">", "<"]) {
            self.pos += 1;
            let right = self.parse_add()?;
            left = eval_binary(left, right, op)?;
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> anyhow::Result<Value> {
        let mut left = self.parse_mul()?;
        while let Some(op) = self.peek_op(&["++", "+", "-"]) {
            self.pos += 1;
            let right = self.parse_mul()?;
            left = eval_binary(left, right, op)?;
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> anyhow::Result<Value> {
        let mut left = self.parse_primary()?;
        while let Some(op) = self.peek_op(&["*", "/", "%"]) {
            self.pos += 1;
            let right = self.parse_primary()?;
            left = eval_binary(left, right, op)?;
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> anyhow::Result<Value> {
        match self.tokens.get(self.pos).cloned() {
            Some(ExprToken::LParen) => {
                self.pos += 1;
                let v = self.parse_expression()?;
                match self.tokens.get(self.pos) {
                    Some(ExprToken::RParen) => {
                        self.pos += 1;
                        Ok(v)
                    }
                    _ => bail!("missing closing ')' in expression"),
                }
            }
            Some(ExprToken::Atom(atom)) => {
                self.pos += 1;
                self.resolve_atom(&atom)
            }
            Some(ExprToken::RParen) => bail!("unexpected ')' in expression"),
            Some(ExprToken::Op(op)) => bail!("unexpected operator '{op}'"),
            None => bail!("unexpected end of expression"),
        }
    }

    fn resolve_atom(&self, atom: &str) -> anyhow::Result<Value> {
        if let Some(name) = atom.strip_prefix('$') {
            let key = name.trim();
            if let Some((_, v)) = self.vars.iter().find(|(n, _)| *n == key) {
                return Ok((*v).clone());
            }
            bail!("unknown closure variable ${key}");
        }
        Ok(parse_literal_value(atom))
    }

    fn peek_op<'b>(&self, ops: &'b [&str]) -> Option<&'b str> {
        match self.tokens.get(self.pos) {
            Some(ExprToken::Op(op)) => ops.iter().copied().find(|candidate| *candidate == op),
            _ => None,
        }
    }
}

fn tokenize_expr(input: &str) -> Vec<ExprToken> {
    let mut tokens = Vec::new();
    let chars = input.chars().collect::<Vec<_>>();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '(' {
            tokens.push(ExprToken::LParen);
            i += 1;
            continue;
        }
        if ch == ')' {
            tokens.push(ExprToken::RParen);
            i += 1;
            continue;
        }
        if i + 1 < chars.len() {
            let pair = [chars[i], chars[i + 1]].iter().collect::<String>();
            if ["==", "!=", ">=", "<=", "++"].contains(&pair.as_str()) {
                tokens.push(ExprToken::Op(pair));
                i += 2;
                continue;
            }
        }
        if ['>', '<', '+', '-', '*', '/', '%'].contains(&ch) {
            tokens.push(ExprToken::Op(ch.to_string()));
            i += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let mut j = i + 1;
            let mut atom = String::new();
            while j < chars.len() {
                if chars[j] == quote {
                    break;
                }
                atom.push(chars[j]);
                j += 1;
            }
            tokens.push(ExprToken::Atom(format!("{quote}{atom}{quote}")));
            i = (j + 1).min(chars.len());
            continue;
        }
        let mut j = i;
        let mut atom = String::new();
        while j < chars.len() {
            let c = chars[j];
            if c.is_whitespace()
                || c == '('
                || c == ')'
                || ['>', '<', '+', '-', '*', '/', '%', '=', '!'].contains(&c)
            {
                break;
            }
            atom.push(c);
            j += 1;
        }
        if !atom.is_empty() {
            tokens.push(ExprToken::Atom(atom));
            i = j;
            continue;
        }
        i += 1;
    }
    tokens
}

fn parse_insert_value(raw: &str, index: usize) -> anyhow::Result<Value> {
    let trimmed = raw.trim();
    if matches!(
        trimmed,
        "{ $index }" | "{$index}" | "{ $index}" | "{$index }"
    ) {
        return Ok(Value::Int(index as i64));
    }
    parse_inline_value(trimmed)
}

fn parse_inline_value(raw: &str) -> anyhow::Result<Value> {
    let s = raw.trim();
    if s.starts_with('{') && s.ends_with('}') && s.contains(':') {
        return parse_record_literal(s);
    }
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Ok(Value::String(s[1..s.len() - 1].to_string()));
    }
    if let Ok(v) = from_json_str(s) {
        return Ok(v);
    }
    Ok(parse_literal_value(s))
}

fn parse_record_literal(raw: &str) -> anyhow::Result<Value> {
    let inner = &raw[1..raw.len() - 1];
    let mut rec = Record::new();
    for part in inner.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let (k, v) = p
            .split_once(':')
            .ok_or_else(|| anyhow!("invalid record literal: expected key: value"))?;
        let key = k.trim().trim_matches('"').trim_matches('\'').to_string();
        rec.insert(key, parse_inline_value(v.trim())?);
    }
    Ok(Value::Record(rec))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_closure_concat_works() {
        let input = Value::List(vec![
            Value::String("foo".into()),
            Value::String("bar".into()),
        ]);
        let items = value_to_items(input);
        let out = items
            .iter()
            .map(|it| eval_closure_expr("'~/' ++ $s", "s", it).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            out,
            vec![Value::String("~/foo".into()), Value::String("~/bar".into())]
        );
    }

    #[test]
    fn parse_reduce_closure_works() {
        let args = vec![
            "-f".to_string(),
            "".to_string(),
            "{|elt, acc| $acc + $elt + ' '}".to_string(),
        ];
        let parsed = parse_reduce_closure_args(&args);
        assert!(parsed.is_some());
    }

    #[test]
    fn filter_map_reduce_brace_expr_works() {
        let input = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let mut env = dosh_env::EnvContext::new(cwd);
        let mut ctx = BuiltinContext { env: &mut env };

        let out = FilterBuiltin
            .run(
                &["{".into(), "$it".into(), ">".into(), "1".into(), "}".into()],
                PipelineData::Value(input.clone()),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![Value::Int(2), Value::Int(3)]))
        );

        let out = EachBuiltin
            .run(
                &["{".into(), "$it".into(), "*".into(), "2".into(), "}".into()],
                PipelineData::Value(input.clone()),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![
                Value::Float(2.0),
                Value::Float(4.0),
                Value::Float(6.0)
            ]))
        );

        let out = ReduceBuiltin
            .run(
                &[
                    "0".into(),
                    "{".into(),
                    "$acc".into(),
                    "+".into(),
                    "$it".into(),
                    "}".into(),
                ],
                PipelineData::Value(input),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(out.output, PipelineData::Value(Value::Float(6.0)));
    }

    #[test]
    fn record_table_ops_match_examples() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let mut env = dosh_env::EnvContext::new(cwd);
        let mut ctx = BuiltinContext { env: &mut env };

        let mut rec = Record::new();
        rec.insert("name".into(), Value::String("dosh".into()));
        let out = HasBuiltin
            .run(
                &["name".into()],
                PipelineData::Value(Value::Record(rec.clone())),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(out.output, PipelineData::Value(Value::Bool(true)));

        let out = MergeBuiltin
            .run(
                &["{age:".into(), "1}".into()],
                PipelineData::Value(Value::Record(rec)),
                &mut ctx,
            )
            .unwrap();
        match out.output {
            PipelineData::Value(Value::Record(r)) => {
                assert_eq!(r.get("age"), Some(&Value::Int(1)));
            }
            _ => panic!("expected record"),
        }

        let rows = vec![
            {
                let mut r = Record::new();
                r.insert("name".into(), Value::String("a".into()));
                r
            },
            {
                let mut r = Record::new();
                r.insert("name".into(), Value::String("b".into()));
                r
            },
        ];
        let out = InsertBuiltin
            .run(
                &["id".into(), "{".into(), "$index".into(), "}".into()],
                PipelineData::Value(Value::Table(Table::new(rows))),
                &mut ctx,
            )
            .unwrap();
        match out.output {
            PipelineData::Value(Value::Table(t)) => {
                assert_eq!(t.rows[0].get("id"), Some(&Value::Int(0)));
                assert_eq!(t.rows[1].get("id"), Some(&Value::Int(1)));
            }
            _ => panic!("expected table"),
        }

        let mut rec2 = Record::new();
        rec2.insert("name".into(), Value::String("dosh".into()));
        rec2.insert("age".into(), Value::Int(1));
        let out = SelectBuiltin
            .run(
                &["name".into()],
                PipelineData::Value(Value::Record(rec2.clone())),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::Record({
                let mut r = Record::new();
                r.insert("name".into(), Value::String("dosh".into()));
                r
            }))
        );

        let out = RejectBuiltin
            .run(
                &["age".into()],
                PipelineData::Value(Value::Record(rec2)),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::Record({
                let mut r = Record::new();
                r.insert("name".into(), Value::String("dosh".into()));
                r
            }))
        );
    }

    #[test]
    fn closure_expr_supports_parentheses_precedence() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let mut env = dosh_env::EnvContext::new(cwd);
        let mut ctx = BuiltinContext { env: &mut env };
        let input = Value::List(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
        ]);

        let out = FilterBuiltin
            .run(
                &[
                    "{".into(),
                    "($it".into(),
                    "+".into(),
                    "2)".into(),
                    "*".into(),
                    "3".into(),
                    ">".into(),
                    "10".into(),
                    "}".into(),
                ],
                PipelineData::Value(input.clone()),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![
                Value::Int(2),
                Value::Int(3),
                Value::Int(4)
            ]))
        );

        let out = EachBuiltin
            .run(
                &[
                    "{".into(),
                    "($it".into(),
                    "+".into(),
                    "1)".into(),
                    "*".into(),
                    "2".into(),
                    "}".into(),
                ],
                PipelineData::Value(input),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![
                Value::Float(4.0),
                Value::Float(6.0),
                Value::Float(8.0),
                Value::Float(10.0)
            ]))
        );
    }

    #[test]
    fn where_supports_string_predicates() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let mut env = dosh_env::EnvContext::new(cwd);
        let mut ctx = BuiltinContext { env: &mut env };
        let input = Value::List(vec![
            Value::String("main.rs".into()),
            Value::String("Cargo.toml".into()),
        ]);

        let out = WhereBuiltin
            .run(
                &["ends-with".into(), "\".rs\"".into()],
                PipelineData::Value(input),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![Value::String("main.rs".into())]))
        );
    }

    #[test]
    fn query_sqlite_document_works() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("sample.sqlite");
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open sqlite");
            conn.execute("create table users(id integer, name text)", [])
                .expect("create table");
            conn.execute("insert into users(id,name) values(1,'alice')", [])
                .expect("insert 1");
            conn.execute("insert into users(id,name) values(2,'bob')", [])
                .expect("insert 2");
        }

        let mut meta = Record::new();
        meta.insert(
            "source_path".into(),
            Value::String(db_path.to_string_lossy().to_string()),
        );
        meta.insert("source_format".into(), Value::String("sqlite".into()));
        let mut doc = Record::new();
        doc.insert("_meta".into(), Value::Record(meta));
        doc.insert("_value".into(), Value::Null);
        doc.insert("_original".into(), Value::Null);
        doc.insert("_dirty".into(), Value::Bool(false));

        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let mut env = dosh_env::EnvContext::new(cwd);
        let mut ctx = BuiltinContext { env: &mut env };
        let out = QueryBuiltin
            .run(
                &[
                    "select".into(),
                    "*".into(),
                    "from".into(),
                    "users".into(),
                    "where".into(),
                    "id".into(),
                    ">".into(),
                    "1".into(),
                ],
                PipelineData::Value(Value::Record(doc)),
                &mut ctx,
            )
            .expect("query run");
        match out.output {
            PipelineData::Value(Value::Table(t)) => {
                assert_eq!(t.rows.len(), 1);
                assert_eq!(t.rows[0].get("name"), Some(&Value::String("bob".into())));
            }
            _ => panic!("expected table"),
        }
    }

    #[test]
    fn first_last_slice_support_counts() {
        let list = Value::List(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            Value::Int(5),
        ]);
        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let mut env = dosh_env::EnvContext::new(cwd);
        let mut ctx = BuiltinContext { env: &mut env };
        let out = FirstBuiltin
            .run(&["2".into()], PipelineData::Value(list.clone()), &mut ctx)
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![Value::Int(1), Value::Int(2)]))
        );

        let out = LastBuiltin
            .run(&["2".into()], PipelineData::Value(list.clone()), &mut ctx)
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![Value::Int(4), Value::Int(5)]))
        );

        let out = SliceBuiltin
            .run(
                &["1".into(), "4".into()],
                PipelineData::Value(list),
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            out.output,
            PipelineData::Value(Value::List(vec![
                Value::Int(2),
                Value::Int(3),
                Value::Int(4)
            ]))
        );
    }
}
