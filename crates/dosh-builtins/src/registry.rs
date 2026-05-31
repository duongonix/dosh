use anyhow::Result;
use dosh_config::DoshPaths;
use dosh_env::EnvContext;
use dosh_value::{FilesizeValue, Record, Table, Value};
use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

mod core_shell_builtins;
mod date_time_builtins;
mod env_builtins;
mod file_pipeline_builtins;
mod format_builtins;
mod fs_builtins;
mod math_builtins;
mod network_builtins;
mod process_builtins;
mod structured_builtins;
mod system_info_builtins;
mod text_builtins;
mod type_list_builtins;
mod type_number_builtins;
mod type_string_builtins;

pub static ALIASES: Lazy<Mutex<BTreeMap<String, String>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineData {
    Empty,
    Text(String),
    Value(Value),
    RowStream(crate::stream::RowStream),
}

impl PipelineData {
    pub fn into_text(self) -> String {
        match self {
            PipelineData::Empty => String::new(),
            PipelineData::Text(v) => v,
            PipelineData::Value(v) => value_to_display_text(&v),
            PipelineData::RowStream(s) => value_to_display_text(&s.materialize_value()),
        }
    }
}

fn value_to_display_text(v: &Value) -> String {
    match v {
        Value::Table(_) | Value::Record(_) => {
            crate::render::render_value_as_table(v, crate::render::TableRenderOptions::default())
        }
        Value::List(items) if items.iter().all(|x| matches!(x, Value::Record(_))) => {
            crate::render::render_value_as_table(v, crate::render::TableRenderOptions::default())
        }
        Value::List(items) => items
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => format!("{v}"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinOutcome {
    pub exit_code: i32,
    pub should_exit: bool,
    pub output: PipelineData,
}

impl BuiltinOutcome {
    pub fn ok(output: PipelineData) -> Self {
        Self {
            exit_code: 0,
            should_exit: false,
            output,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuiltinMetadata {
    pub name: &'static str,
    pub usage: &'static str,
    pub description: &'static str,
    pub examples: &'static [&'static str],
}

pub struct BuiltinContext<'a> {
    pub env: &'a mut EnvContext,
}

pub trait Builtin: Send + Sync {
    fn metadata(&self) -> BuiltinMetadata;
    fn run(
        &self,
        args: &[String],
        input: PipelineData,
        ctx: &mut BuiltinContext<'_>,
    ) -> Result<BuiltinOutcome>;
}

pub type BuiltinFactory = fn() -> Box<dyn Builtin>;

#[derive(Default)]
pub struct BuiltinRegistry {
    builtins: BTreeMap<String, Box<dyn Builtin>>,
}

impl BuiltinRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            builtins: BTreeMap::new(),
        };
        for factory in builtin_factories() {
            let b = factory();
            registry.builtins.insert(b.metadata().name.to_string(), b);
        }
        registry
    }

    pub fn run(
        &self,
        name: &str,
        args: &[String],
        input: PipelineData,
        env: &mut EnvContext,
    ) -> Result<Option<BuiltinOutcome>> {
        let Some(builtin) = self.builtins.get(name) else {
            return Ok(None);
        };
        let mut ctx = BuiltinContext { env };
        Ok(Some(builtin.run(args, input, &mut ctx)?))
    }

    pub fn metadata(&self, name: Option<&str>) -> Vec<BuiltinMetadata> {
        if let Some(name) = name {
            return self
                .builtins
                .get(name)
                .map(|b| vec![b.metadata()])
                .unwrap_or_default();
        }
        self.builtins.values().map(|b| b.metadata()).collect()
    }

    pub fn expand_alias(&self, name: &str, args: &[String]) -> Option<(String, Vec<String>)> {
        let alias = ALIASES.lock().ok()?.get(name).cloned()?;
        let mut split = shell_words::split(&alias).ok()?;
        if split.is_empty() {
            return None;
        }
        let new_name = split.remove(0);
        let mut new_args = split;
        new_args.extend(args.iter().cloned());
        Some((new_name, new_args))
    }
}

macro_rules! simple_builtin {
    ($name:ident, $n:literal, $u:literal, $d:literal, $e:expr, $body:expr) => {
        pub(super) struct $name;
        impl Builtin for $name {
            fn metadata(&self) -> BuiltinMetadata {
                BuiltinMetadata {
                    name: $n,
                    usage: $u,
                    description: $d,
                    examples: $e,
                }
            }
            fn run(
                &self,
                args: &[String],
                input: PipelineData,
                ctx: &mut BuiltinContext<'_>,
            ) -> Result<BuiltinOutcome> {
                let handler: fn(
                    &[String],
                    PipelineData,
                    &mut BuiltinContext<'_>,
                ) -> Result<BuiltinOutcome> = $body;
                handler(args, input, ctx)
            }
        }
    };
}
pub(super) use simple_builtin;

macro_rules! factory {
    ($name:ident) => {
        (|| Box::new($name) as Box<dyn Builtin>) as BuiltinFactory
    };
}
pub(super) use factory;

fn builtin_factories() -> Vec<BuiltinFactory> {
    let mut v: Vec<BuiltinFactory> = Vec::new();
    v.extend(core_shell_builtins::factories());
    v.extend(date_time_builtins::factories());
    v.extend(env_builtins::factories());
    v.extend(fs_builtins::factories());
    v.extend(file_pipeline_builtins::factories());
    v.extend(process_builtins::factories());
    v.extend(network_builtins::factories());
    v.extend(system_info_builtins::factories());
    v.extend(structured_builtins::factories());
    v.extend(format_builtins::factories());
    v.extend(text_builtins::factories());
    v.extend(math_builtins::factories());
    v.extend(type_string_builtins::factories());
    v.extend(type_number_builtins::factories());
    v.extend(type_list_builtins::factories());
    v
}

pub(super) fn to_rows(value: Value) -> Vec<Record> {
    match value {
        Value::Table(t) => t.rows,
        Value::List(v) => v
            .into_iter()
            .filter_map(|x| {
                if let Value::Record(r) = x {
                    Some(r)
                } else {
                    None
                }
            })
            .collect(),
        Value::Record(r) => vec![r],
        _ => Vec::new(),
    }
}

pub(super) fn history_file_path() -> PathBuf {
    if let Ok(paths) = DoshPaths::detect() {
        return paths.history_text_file();
    }
    PathBuf::from(".dosh_reedline.history")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn where_filters_filesize() {
        let mut row_big = Record::new();
        row_big.insert(
            "size".into(),
            Value::Filesize(FilesizeValue {
                bytes: 2 * 1024 * 1024,
            }),
        );
        let mut row_small = Record::new();
        row_small.insert(
            "size".into(),
            Value::Filesize(FilesizeValue { bytes: 128 * 1024 }),
        );
        let val = Value::Table(Table::new(vec![row_big, row_small]));
        let mut env = EnvContext::from_current_dir().unwrap();
        let reg = BuiltinRegistry::new();
        let out_gt = reg
            .run(
                "where",
                &["size > 1mb".into()],
                PipelineData::Value(val.clone()),
                &mut env,
            )
            .unwrap()
            .unwrap();
        let out_le = reg
            .run(
                "where",
                &["size <= 1mb".into()],
                PipelineData::Value(val),
                &mut env,
            )
            .unwrap()
            .unwrap();
        match out_gt.output {
            PipelineData::Value(Value::Table(t)) => assert_eq!(t.rows.len(), 1),
            PipelineData::RowStream(s) => assert_eq!(s.materialize_rows().len(), 1),
            _ => panic!("expected table"),
        }
        match out_le.output {
            PipelineData::Value(Value::Table(t)) => assert_eq!(t.rows.len(), 1),
            PipelineData::RowStream(s) => assert_eq!(s.materialize_rows().len(), 1),
            _ => panic!("expected table"),
        }
    }
}
