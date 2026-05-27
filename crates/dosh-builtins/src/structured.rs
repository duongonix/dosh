use anyhow::{Result, bail};
use dosh_value::{Value, from_json_str, select_fields, to_json_string};

use crate::registry::BuiltinOutcome;

pub fn run_structured_builtin(name: &str, args: &[String]) -> Result<Option<BuiltinOutcome>> {
    let outcome = match name {
        "from-json" => Some(from_json_builtin(args)?),
        "to-json" => Some(to_json_builtin(args)?),
        "table" => Some(table_builtin(args)?),
        "select" => Some(select_builtin(args)?),
        _ => None,
    };
    Ok(outcome)
}

fn from_json_builtin(args: &[String]) -> Result<BuiltinOutcome> {
    let input = args.join(" ");
    if input.trim().is_empty() {
        bail!("from-json expects a JSON string")
    }
    let value = from_json_str(&input)?;
    Ok(BuiltinOutcome::ok(Some(format!("{value:?}"))))
}

fn to_json_builtin(args: &[String]) -> Result<BuiltinOutcome> {
    let input = args.join(" ");
    if input.trim().is_empty() {
        bail!("to-json expects a JSON-like input (currently JSON string)")
    }
    let value = from_json_str(&input)?;
    Ok(BuiltinOutcome::ok(Some(to_json_string(&value)?)))
}

fn table_builtin(args: &[String]) -> Result<BuiltinOutcome> {
    let input = args.join(" ");
    if input.trim().is_empty() {
        bail!("table expects a JSON array of objects")
    }
    let value = from_json_str(&input)?;
    let rows = match value {
        Value::List(items) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::Record(row) => Some(row),
                _ => None,
            })
            .collect::<Vec<_>>(),
        _ => bail!("table expects a JSON array"),
    };

    let headers = rows
        .first()
        .map(|r| r.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let mut lines = Vec::new();
    if !headers.is_empty() {
        lines.push(headers.join("\t"));
        for row in rows {
            let line = headers
                .iter()
                .map(|h| format!("{:?}", row.get(h).cloned().unwrap_or(Value::Null)))
                .collect::<Vec<_>>()
                .join("\t");
            lines.push(line);
        }
    }

    Ok(BuiltinOutcome::ok(Some(lines.join("\n"))))
}

fn select_builtin(args: &[String]) -> Result<BuiltinOutcome> {
    if args.len() < 2 {
        bail!("select expects: select <field1,field2,...> <json>")
    }
    let fields = args[0]
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let json_input = args[1..].join(" ");
    let value = from_json_str(&json_input)?;
    let projected = select_fields(&value, &fields);
    Ok(BuiltinOutcome::ok(Some(to_json_string(&projected)?)))
}
