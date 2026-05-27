use crate::value::{Record, Table, Value};
use anyhow::Result;
use serde_json::Value as JsonValue;

pub fn from_json_str(input: &str) -> Result<Value> {
    let parsed: JsonValue = serde_json::from_str(input)?;
    Ok(from_json_value(parsed))
}

pub fn to_json_string(value: &Value) -> Result<String> {
    let json = to_json_value(value);
    Ok(serde_json::to_string_pretty(&json)?)
}

pub fn from_yaml_str(input: &str) -> Result<Value> {
    let parsed: serde_yaml::Value = serde_yaml::from_str(input)?;
    let json = serde_json::to_value(parsed)?;
    Ok(from_json_value(json))
}

pub fn to_yaml_string(value: &Value) -> Result<String> {
    Ok(serde_yaml::to_string(&to_json_value(value))?)
}

pub fn from_toml_str(input: &str) -> Result<Value> {
    let parsed: toml::Value = toml::from_str(input)?;
    let json = serde_json::to_value(parsed)?;
    Ok(from_json_value(json))
}

pub fn to_toml_string(value: &Value) -> Result<String> {
    Ok(toml::to_string_pretty(&to_json_value(value))?)
}

fn from_json_value(value: JsonValue) -> Value {
    match value {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(v) => Value::Bool(v),
        JsonValue::Number(n) => n
            .as_i64()
            .map(Value::Int)
            .unwrap_or_else(|| Value::Float(n.as_f64().unwrap_or(0.0))),
        JsonValue::String(v) => Value::String(v),
        JsonValue::Array(vs) => {
            let values: Vec<Value> = vs.into_iter().map(from_json_value).collect();
            if values.iter().all(|v| matches!(v, Value::Record(_))) {
                let rows = values
                    .into_iter()
                    .filter_map(|v| match v {
                        Value::Record(r) => Some(r),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                Value::Table(Table::new(rows))
            } else {
                Value::List(values)
            }
        }
        JsonValue::Object(map) => {
            let rec: Record = map
                .into_iter()
                .map(|(k, v)| (k, from_json_value(v)))
                .collect();
            Value::Record(rec)
        }
    }
}

fn to_json_value(value: &Value) -> JsonValue {
    match value {
        Value::Null => JsonValue::Null,
        Value::Bool(v) => JsonValue::Bool(*v),
        Value::Int(v) => JsonValue::from(*v),
        Value::Float(v) => JsonValue::from(*v),
        Value::String(v) => JsonValue::String(v.clone()),
        Value::Duration(v) => JsonValue::String(format!("{}ns", v.nanos)),
        Value::Filesize(v) => JsonValue::String(format!("{}b", v.bytes)),
        Value::DateTime(v) => JsonValue::String(v.iso8601.clone()),
        Value::Binary(bytes) => {
            JsonValue::Array(bytes.iter().map(|b| JsonValue::from(*b)).collect())
        }
        Value::List(values) => JsonValue::Array(values.iter().map(to_json_value).collect()),
        Value::Record(map) => JsonValue::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), to_json_value(v)))
                .collect(),
        ),
        Value::Table(table) => JsonValue::Array(
            table
                .rows
                .iter()
                .map(|row| {
                    JsonValue::Object(
                        row.iter()
                            .map(|(k, v)| (k.clone(), to_json_value(v)))
                            .collect(),
                    )
                })
                .collect(),
        ),
    }
}
