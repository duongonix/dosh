use crate::value::{Record, Table, Value};
use indexmap::IndexMap;

pub fn select_fields(value: &Value, fields: &[String]) -> Value {
    match value {
        Value::Record(record) => {
            let mut out = IndexMap::new();
            for key in fields {
                if let Some(v) = record.get(key) {
                    out.insert(key.clone(), v.clone());
                }
            }
            Value::Record(out)
        }
        Value::Table(table) => {
            let rows = table
                .rows
                .iter()
                .map(|row| project_record(row, fields))
                .collect::<Vec<_>>();
            Value::Table(Table::with_columns(fields.to_vec(), rows))
        }
        Value::List(items) => Value::List(items.iter().map(|v| select_fields(v, fields)).collect()),
        _ => value.clone(),
    }
}

fn project_record(row: &Record, fields: &[String]) -> Record {
    let mut out = Record::new();
    for key in fields {
        if let Some(v) = row.get(key) {
            out.insert(key.clone(), v.clone());
        }
    }
    out
}
