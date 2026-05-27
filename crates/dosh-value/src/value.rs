use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

pub type Record = IndexMap<String, Value>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Record>,
}

impl Table {
    pub fn new(rows: Vec<Record>) -> Self {
        let mut columns = Vec::new();
        for row in &rows {
            for key in row.keys() {
                if !columns.contains(key) {
                    columns.push(key.clone());
                }
            }
        }
        Self { columns, rows }
    }

    pub fn with_columns(columns: Vec<String>, rows: Vec<Record>) -> Self {
        Self { columns, rows }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DurationValue {
    pub nanos: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FilesizeValue {
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DateTimeValue {
    pub iso8601: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ValueType {
    Null,
    Bool,
    Int,
    Float,
    String,
    Duration,
    Filesize,
    DateTime,
    Binary,
    List,
    Record,
    Table,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Duration(DurationValue),
    Filesize(FilesizeValue),
    DateTime(DateTimeValue),
    Binary(Vec<u8>),
    List(Vec<Value>),
    Record(Record),
    Table(Table),
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Duration(_) => "duration",
            Value::Filesize(_) => "filesize",
            Value::DateTime(_) => "datetime",
            Value::Binary(_) => "binary",
            Value::List(_) => "list",
            Value::Record(_) => "record",
            Value::Table(_) => "table",
        }
    }

    pub fn value_type(&self) -> ValueType {
        match self {
            Value::Null => ValueType::Null,
            Value::Bool(_) => ValueType::Bool,
            Value::Int(_) => ValueType::Int,
            Value::Float(_) => ValueType::Float,
            Value::String(_) => ValueType::String,
            Value::Duration(_) => ValueType::Duration,
            Value::Filesize(_) => ValueType::Filesize,
            Value::DateTime(_) => ValueType::DateTime,
            Value::Binary(_) => ValueType::Binary,
            Value::List(_) => ValueType::List,
            Value::Record(_) => ValueType::Record,
            Value::Table(_) => ValueType::Table,
        }
    }

    pub fn as_record(&self) -> Option<&Record> {
        match self {
            Value::Record(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Value::Table(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(v) => *v,
            Value::Int(v) => *v != 0,
            Value::Float(v) => *v != 0.0,
            Value::String(v) => !v.is_empty(),
            Value::Duration(v) => v.nanos != 0,
            Value::Filesize(v) => v.bytes != 0,
            Value::DateTime(_) => true,
            Value::Binary(v) => !v.is_empty(),
            Value::List(v) => !v.is_empty(),
            Value::Record(v) => !v.is_empty(),
            Value::Table(v) => !v.rows.is_empty(),
        }
    }

    pub fn get_path<'a>(&'a self, path: &str) -> Option<&'a Value> {
        let mut cur = self;
        for part in path.split('.').filter(|s| !s.is_empty()) {
            cur = match cur {
                Value::Record(map) => map.get(part)?,
                Value::List(items) => items.get(part.parse::<usize>().ok()?)?,
                Value::Table(_) => return None,
                _ => return None,
            };
        }
        Some(cur)
    }

    pub fn parse_filesize(input: &str) -> Option<FilesizeValue> {
        parse_filesize(input).map(|bytes| FilesizeValue { bytes })
    }

    pub fn parse_duration(input: &str) -> Option<DurationValue> {
        parse_duration(input).map(|nanos| DurationValue { nanos })
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::Int(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::String(v) => write!(f, "{v}"),
            Value::Duration(v) => write!(f, "{}ns", v.nanos),
            Value::Filesize(v) => write!(f, "{}b", v.bytes),
            Value::DateTime(v) => write!(f, "{}", v.iso8601),
            Value::Binary(v) => write!(f, "<binary:{}>", v.len()),
            Value::List(v) => write!(f, "list<{}>", v.len()),
            Value::Record(v) => write!(f, "record<{}>", v.len()),
            Value::Table(v) => write!(f, "table<rows:{} cols:{}>", v.rows.len(), v.columns.len()),
        }
    }
}

fn parse_filesize(input: &str) -> Option<u64> {
    let s = input.trim().to_ascii_lowercase();
    for (suffix, mult) in [
        ("tb", 1024_u64.pow(4)),
        ("gb", 1024_u64.pow(3)),
        ("mb", 1024_u64.pow(2)),
        ("kb", 1024_u64),
        ("b", 1),
    ] {
        if let Some(raw) = s.strip_suffix(suffix) {
            return raw
                .trim()
                .parse::<u64>()
                .ok()
                .map(|n| n.saturating_mul(mult));
        }
    }
    None
}

fn parse_duration(input: &str) -> Option<i64> {
    let s = input.trim().to_ascii_lowercase();
    for (suffix, mult) in [
        ("day", 86_400_000_000_000_i64),
        ("hr", 3_600_000_000_000),
        ("min", 60_000_000_000),
        ("sec", 1_000_000_000),
        ("ms", 1_000_000),
    ] {
        if let Some(raw) = s.strip_suffix(suffix) {
            return raw
                .trim()
                .parse::<i64>()
                .ok()
                .map(|n| n.saturating_mul(mult));
        }
    }
    None
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}
impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_path_works() {
        let mut user = Record::new();
        user.insert("email".into(), Value::String("a@b.c".into()));
        let mut root = Record::new();
        root.insert("user".into(), Value::Record(user));
        let v = Value::Record(root);
        assert_eq!(
            v.get_path("user.email"),
            Some(&Value::String("a@b.c".into()))
        );
    }

    #[test]
    fn list_path_works() {
        let v = Value::List(vec![
            Value::String("a".into()),
            Value::String("b".into()),
            Value::String("c".into()),
        ]);
        assert_eq!(v.get_path("1"), Some(&Value::String("b".into())));
    }

    #[test]
    fn parse_size_and_duration() {
        assert_eq!(
            Value::parse_filesize("1tb").map(|v| v.bytes),
            Some(1024_u64.pow(4))
        );
        assert_eq!(
            Value::parse_duration("1day").map(|v| v.nanos),
            Some(86_400_000_000_000)
        );
        assert_eq!(
            Value::parse_duration("1ms").map(|v| v.nanos),
            Some(1_000_000)
        );
    }
}
