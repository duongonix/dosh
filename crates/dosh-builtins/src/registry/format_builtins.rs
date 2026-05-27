use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::simple_builtin;
use anyhow::anyhow;
use regex::Regex;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        || Box::new(FromCsvBuiltin),
        || Box::new(ToCsvBuiltin),
        || Box::new(FromXmlBuiltin),
        || Box::new(ToXmlBuiltin),
        || Box::new(FromIniBuiltin),
        || Box::new(ToIniBuiltin),
    ]
}

simple_builtin!(
    FromCsvBuiltin,
    "from-csv",
    "from-csv",
    "Parse CSV text into table",
    &["cat data.csv | from-csv"],
    |_args, input, _ctx| {
        let text = input.into_text();
        Ok(BuiltinOutcome::ok(PipelineData::Value(parse_csv_text(
            &text,
        )?)))
    }
);
simple_builtin!(
    ToCsvBuiltin,
    "to-csv",
    "to-csv",
    "Convert table/records to CSV text",
    &["open users.json | to-csv"],
    |_args, input, _ctx| {
        let value = pipeline_to_value(input)?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(to_csv_text(&value))))
    }
);
simple_builtin!(
    FromXmlBuiltin,
    "from-xml",
    "from-xml",
    "Parse simple XML object into record",
    &["cat app.xml | from-xml"],
    |_args, input, _ctx| {
        let text = input.into_text();
        Ok(BuiltinOutcome::ok(PipelineData::Value(parse_simple_xml(
            &text,
        ))))
    }
);
simple_builtin!(
    ToXmlBuiltin,
    "to-xml",
    "to-xml",
    "Convert record/table to simple XML",
    &["open app.json | to-xml"],
    |_args, input, _ctx| {
        let value = pipeline_to_value(input)?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(to_simple_xml(
            &value,
        ))))
    }
);
simple_builtin!(
    FromIniBuiltin,
    "from-ini",
    "from-ini",
    "Parse INI text into nested record",
    &["cat app.ini | from-ini"],
    |_args, input, _ctx| {
        let text = input.into_text();
        Ok(BuiltinOutcome::ok(PipelineData::Value(parse_ini_text(
            &text,
        ))))
    }
);
simple_builtin!(
    ToIniBuiltin,
    "to-ini",
    "to-ini",
    "Convert record into INI text",
    &["open app.json | to-ini"],
    |_args, input, _ctx| {
        let value = pipeline_to_value(input)?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(to_ini_text(&value))))
    }
);

pub(super) fn parse_csv_public(text: &str) -> Result<Value> {
    parse_csv_text(text)
}

fn parse_csv_text(text: &str) -> Result<Value> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let headers = lines
        .next()
        .ok_or_else(|| anyhow!("csv input is empty"))?
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for line in lines {
        let cells = line
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>();
        let mut row = Record::new();
        for (i, h) in headers.iter().enumerate() {
            row.insert(
                h.clone(),
                Value::String(cells.get(i).cloned().unwrap_or_default()),
            );
        }
        rows.push(row);
    }
    Ok(Value::Table(Table::new(rows)))
}

fn to_csv_text(value: &Value) -> String {
    let rows = to_rows(value.clone());
    if rows.is_empty() {
        return String::new();
    }
    let headers = rows[0].keys().cloned().collect::<Vec<_>>();
    let mut out = vec![headers.join(",")];
    for row in rows {
        out.push(
            headers
                .iter()
                .map(|h| row.get(h).map(|v| v.to_string()).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out.join("\n")
}

fn parse_simple_xml(text: &str) -> Value {
    let mut rec = Record::new();
    let re = Regex::new(r"<([A-Za-z0-9_\-]+)>([^<]*)</([A-Za-z0-9_\-]+)>").ok();
    if let Some(re) = re {
        for cap in re.captures_iter(text) {
            if cap.get(1).map(|m| m.as_str()) == cap.get(3).map(|m| m.as_str()) {
                let k = cap
                    .get(1)
                    .map(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string();
                let v = cap
                    .get(2)
                    .map(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string();
                rec.insert(k, Value::String(v));
            }
        }
    }
    Value::Record(rec)
}

fn to_simple_xml(value: &Value) -> String {
    match value {
        Value::Record(r) => {
            let inner = r
                .iter()
                .map(|(k, v)| format!("<{k}>{}</{k}>", v))
                .collect::<Vec<_>>()
                .join("");
            format!("<root>{inner}</root>")
        }
        Value::Table(t) => {
            let rows = t
                .rows
                .iter()
                .map(|row| {
                    let inner = row
                        .iter()
                        .map(|(k, v)| format!("<{k}>{}</{k}>", v))
                        .collect::<Vec<_>>()
                        .join("");
                    format!("<row>{inner}</row>")
                })
                .collect::<Vec<_>>()
                .join("");
            format!("<rows>{rows}</rows>")
        }
        other => format!("<value>{other}</value>"),
    }
}

fn parse_ini_text(text: &str) -> Value {
    let mut root = Record::new();
    let mut section = "default".to_string();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let sec = root
                .entry(section.clone())
                .or_insert_with(|| Value::Record(Record::new()));
            if let Value::Record(map) = sec {
                map.insert(k.trim().to_string(), Value::String(v.trim().to_string()));
            }
        }
    }
    Value::Record(root)
}

fn to_ini_text(value: &Value) -> String {
    let mut out = Vec::new();
    if let Value::Record(root) = value {
        for (section, values) in root {
            out.push(format!("[{section}]"));
            if let Value::Record(map) = values {
                for (k, v) in map {
                    out.push(format!("{k}={v}"));
                }
            }
            out.push(String::new());
        }
    }
    out.join("\n")
}
