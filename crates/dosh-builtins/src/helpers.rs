use crate::registry::PipelineData;
use anyhow::Result;
use dosh_value::Value;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn resolve_path(cwd: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

pub(crate) fn pipeline_to_value(input: PipelineData) -> Result<Value> {
    match input {
        PipelineData::Value(v) => Ok(v),
        PipelineData::Text(t) => Ok(Value::String(t)),
        PipelineData::Empty => Ok(Value::Null),
        PipelineData::RowStream(s) => {
            if let Some(vs) = s.materialize_mapped_values() {
                Ok(Value::List(vs))
            } else {
                Ok(s.materialize_value())
            }
        }
    }
}

pub(crate) fn compare_ord(a: Option<&Value>, b: Option<&Value>) -> Ordering {
    match (a, b) {
        (Some(Value::Int(x)), Some(Value::Int(y))) => x.cmp(y),
        (Some(Value::Float(x)), Some(Value::Float(y))) => {
            x.partial_cmp(y).unwrap_or(Ordering::Equal)
        }
        (Some(Value::String(x)), Some(Value::String(y))) => x.cmp(y),
        (Some(Value::Filesize(x)), Some(Value::Filesize(y))) => x.bytes.cmp(&y.bytes),
        (Some(Value::Duration(x)), Some(Value::Duration(y))) => x.nanos.cmp(&y.nanos),
        _ => Ordering::Equal,
    }
}

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
