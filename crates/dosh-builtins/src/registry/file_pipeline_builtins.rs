use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::{factory, simple_builtin};
use anyhow::{anyhow, bail};
use dosh_value::{
    from_json_str, from_toml_str, from_yaml_str, to_json_string, to_toml_string, to_yaml_string,
};
use globset::{Glob, GlobSetBuilder};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

const META: &str = "_meta";
const VALUE: &str = "_value";
const ORIGINAL: &str = "_original";
const DIRTY: &str = "_dirty";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileFormat {
    Json,
    Yaml,
    Toml,
    Csv,
    Text,
    Xml,
    Ini,
    Sqlite,
    Xlsx,
    Unknown,
}

trait FormatCodec {
    fn decode(&self, text: &str) -> anyhow::Result<Value>;
    fn encode(&self, value: &Value) -> anyhow::Result<String>;
}

struct JsonCodec;
impl FormatCodec for JsonCodec {
    fn decode(&self, text: &str) -> anyhow::Result<Value> {
        from_json_str(text)
    }
    fn encode(&self, value: &Value) -> anyhow::Result<String> {
        to_json_string(value)
    }
}

struct YamlCodec;
impl FormatCodec for YamlCodec {
    fn decode(&self, text: &str) -> anyhow::Result<Value> {
        from_yaml_str(text)
    }
    fn encode(&self, value: &Value) -> anyhow::Result<String> {
        to_yaml_string(value)
    }
}

struct TomlCodec;
impl FormatCodec for TomlCodec {
    fn decode(&self, text: &str) -> anyhow::Result<Value> {
        from_toml_str(text)
    }
    fn encode(&self, value: &Value) -> anyhow::Result<String> {
        to_toml_string(value)
    }
}

struct CsvCodec;
impl FormatCodec for CsvCodec {
    fn decode(&self, text: &str) -> anyhow::Result<Value> {
        super::format_builtins::parse_csv_public(text)
    }
    fn encode(&self, value: &Value) -> anyhow::Result<String> {
        Ok(to_csv_text(value))
    }
}

struct TextCodec;
impl FormatCodec for TextCodec {
    fn decode(&self, text: &str) -> anyhow::Result<Value> {
        Ok(Value::String(text.to_string()))
    }
    fn encode(&self, value: &Value) -> anyhow::Result<String> {
        Ok(match value {
            Value::String(s) => s.clone(),
            _ => value.to_string(),
        })
    }
}

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(GlobBuiltin),
        factory!(OpenEachBuiltin),
        factory!(EditBuiltin),
        factory!(SaveBuiltin),
        factory!(RunBuiltin),
        factory!(DiffBuiltin),
        factory!(PreviewBuiltin),
        factory!(WatchFsBuiltin),
        factory!(DebounceBuiltin),
        factory!(ThrottleBuiltin),
        factory!(ChangedFilesBuiltin),
    ]
}

simple_builtin!(
    GlobBuiltin,
    "glob",
    "glob <pattern> [--hidden] [--max-depth N] [--follow-links]",
    "Discover files as structured table",
    &["glob \"**/*.md\""],
    |args, _input, ctx| {
        let rows = discover_files(args, ctx.env.cwd())?.rows;
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    OpenEachBuiltin,
    "open-each",
    "open-each [--raw] [--format fmt]",
    "Open each file from glob-table/list",
    &["glob \"**/*.json\" | open-each"],
    |args, input, _ctx| {
        let raw = args.iter().any(|a| a == "--raw");
        let force_format = arg_value(args, "--format");
        let files = extract_paths(input)?;
        let mut out = Vec::new();
        for path in files {
            let p = PathBuf::from(&path);
            let text = fs::read_to_string(&p)?;
            let format = force_format
                .as_deref()
                .map(parse_format_name)
                .unwrap_or_else(|| detect_format(&p));
            let val = if raw {
                Value::String(text.clone())
            } else {
                decode_with_format(format, &text)?
            };
            out.push(document_record(&path, format, val));
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
    }
);

simple_builtin!(
    EditBuiltin,
    "edit",
    "edit <glob> [--raw] [--format fmt]",
    "Shortcut for glob <pattern> | open-each",
    &["edit \"src/**/*.rs\" --raw"],
    |args, _input, ctx| {
        let pattern = args
            .iter()
            .find(|a| !a.starts_with('-'))
            .ok_or_else(|| anyhow!("edit expects glob pattern"))?
            .clone();
        let files_out = GlobBuiltin.run(&[pattern], PipelineData::Empty, ctx)?;
        OpenEachBuiltin.run(args, files_out.output, ctx)
    }
);

simple_builtin!(
    SaveBuiltin,
    "save",
    "save [path]|--in-place [--backup] [--dry-run] [--append]",
    "Save transformed document(s) safely",
    &["open a.json | replace x y | save --in-place --backup"],
    |args, input, _ctx| {
        let in_place = args.iter().any(|a| a == "--in-place");
        let backup = args.iter().any(|a| a == "--backup");
        let dry_run = args.iter().any(|a| a == "--dry-run");
        let append = args.iter().any(|a| a == "--append");
        let pretty = args.iter().any(|a| a == "--pretty");
        let format_override = arg_value(args, "--format");
        let path_arg = args.iter().find(|a| !a.starts_with('-')).cloned();
        if !in_place && path_arg.is_none() {
            bail!("save requires output path or --in-place")
        }
        let value = pipeline_to_value(input)?;
        let mut rows = Vec::new();
        if let Ok(docs) = as_documents(&value) {
            let multi = docs.len() > 1;
            for doc in docs {
                let target = if in_place {
                    doc_path(&doc).ok_or_else(|| anyhow!("save --in-place requires source_path"))?
                } else if multi {
                    bail!("bulk save without --in-place is not supported")
                } else {
                    path_arg.clone().unwrap_or_default()
                };
                let target_pb = PathBuf::from(&target);
                let content = encode_document_with_opts(
                    &doc,
                    &target_pb,
                    format_override.as_deref(),
                    pretty,
                )?;
                write_one(&target_pb, &content, backup, dry_run, append, &mut rows)?;
            }
        } else {
            if in_place {
                bail!("save --in-place requires source metadata")
            }
            let target = path_arg.ok_or_else(|| anyhow!("save requires output path"))?;
            let target_pb = PathBuf::from(&target);
            let content =
                encode_plain_value(&value, &target_pb, format_override.as_deref(), pretty)?;
            write_one(&target_pb, &content, backup, dry_run, append, &mut rows)?;
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    RunBuiltin,
    "run",
    "run <command...>",
    "Run external command once or per list item (foundation)",
    &["watch . --glob \"**/*.rs\" | changed-files | run cargo test"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("run expects command")
        }
        let cmd = &args[0];
        let cmd_args = &args[1..];
        let value = pipeline_to_value(input)?;
        let mut logs = Vec::new();
        match value {
            Value::List(items) if !items.is_empty() => {
                for item in items {
                    let mut full_args = cmd_args.to_vec();
                    if let Value::String(s) = item {
                        full_args.push(s);
                    }
                    let out = std::process::Command::new(cmd).args(&full_args).output()?;
                    logs.push(run_log_row(cmd, &out));
                }
            }
            _ => {
                let out = std::process::Command::new(cmd).args(cmd_args).output()?;
                logs.push(run_log_row(cmd, &out));
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(logs),
        ))))
    }
);

simple_builtin!(
    WatchFsBuiltin,
    "watch",
    "watch <path> [--glob pattern] [--duration ms]",
    "Watch filesystem and emit structured events table (foundation)",
    &["watch . --glob \"**/*.rs\" --duration 2000"],
    |args, _input, ctx| {
        let target = args
            .iter()
            .find(|a| !a.starts_with('-'))
            .cloned()
            .unwrap_or_else(|| ".".to_string());
        let glob = arg_value(args, "--glob");
        let duration_ms = arg_value(args, "--duration")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(2000);
        let matcher = if let Some(p) = glob.as_deref() {
            Some(glob_matcher(p)?)
        } else {
            None
        };
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )?;
        let path = PathBuf::from(resolve_watch_path(ctx.env.cwd(), &target));
        watcher.watch(&path, RecursiveMode::Recursive)?;
        let started = std::time::Instant::now();
        let mut rows = Vec::new();
        while started.elapsed() < Duration::from_millis(duration_ms) {
            if let Ok(Ok(ev)) = rx.recv_timeout(Duration::from_millis(100)) {
                for p in ev.paths {
                    let rel = p
                        .strip_prefix(ctx.env.cwd())
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .replace('\\', "/");
                    if let Some(m) = &matcher
                        && !m.is_match(&rel)
                    {
                        continue;
                    }
                    let mut row = Record::new();
                    row.insert(
                        "event".into(),
                        Value::String(normalize_event_kind(&ev.kind)),
                    );
                    row.insert("path".into(), Value::String(p.display().to_string()));
                    row.insert("timestamp".into(), Value::Int(now_ms() as i64));
                    rows.push(row);
                }
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    DebounceBuiltin,
    "debounce",
    "debounce <ms>",
    "Debounce event rows by path+event (foundation)",
    &["watch . | debounce 500"],
    |args, input, _ctx| {
        let ms = args
            .first()
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or_else(|| anyhow!("debounce expects milliseconds"))?;
        let rows = match pipeline_to_value(input)? {
            Value::Table(t) => t.rows,
            _ => Vec::new(),
        };
        let mut last: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
        let mut out = Vec::new();
        for row in rows {
            let key = format!(
                "{}:{}",
                row.get("event").map(|v| v.to_string()).unwrap_or_default(),
                row.get("path").map(|v| v.to_string()).unwrap_or_default()
            );
            let ts = match row.get("timestamp") {
                Some(Value::Int(v)) => *v,
                _ => 0,
            };
            if let Some(prev) = last.get(&key)
                && ts - prev < ms
            {
                continue;
            }
            last.insert(key, ts);
            out.push(row);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(out),
        ))))
    }
);

simple_builtin!(
    ThrottleBuiltin,
    "throttle",
    "throttle <ms>",
    "Keep at most one event every N ms (foundation)",
    &["watch . | throttle 500"],
    |args, input, _ctx| {
        let ms = args
            .first()
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or_else(|| anyhow!("throttle expects milliseconds"))?;
        let rows = match pipeline_to_value(input)? {
            Value::Table(t) => t.rows,
            _ => Vec::new(),
        };
        let mut out = Vec::new();
        let mut last_ts = i64::MIN / 2;
        for row in rows {
            let ts = match row.get("timestamp") {
                Some(Value::Int(v)) => *v,
                _ => 0,
            };
            if ts - last_ts >= ms {
                out.push(row);
                last_ts = ts;
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(out),
        ))))
    }
);

simple_builtin!(
    ChangedFilesBuiltin,
    "changed-files",
    "changed-files",
    "Project distinct changed file paths from event stream",
    &["watch . | changed-files"],
    |_args, input, _ctx| {
        let rows = match pipeline_to_value(input)? {
            Value::Table(t) => t.rows,
            _ => Vec::new(),
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for row in rows {
            if let Some(path) = row.get("path").map(|v| v.to_string())
                && seen.insert(path.clone())
            {
                out.push(Value::String(path));
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::List(out))))
    }
);

simple_builtin!(
    DiffBuiltin,
    "diff",
    "diff",
    "Show original vs current differences",
    &["edit \"**/*.md\" --raw | replace old new | diff"],
    |_args, input, _ctx| {
        let value = pipeline_to_value(input)?;
        let docs = as_documents(&value)?;
        let mut out = String::new();
        for doc in docs {
            let path = doc_path(&doc).unwrap_or_else(|| "<memory>".to_string());
            let before = doc.get(ORIGINAL).map(|v| v.to_string()).unwrap_or_default();
            let after = doc.get(VALUE).map(|v| v.to_string()).unwrap_or_default();
            if before == after {
                continue;
            }
            out.push_str(&format!("--- {path}\n+++ {path}\n"));
            out.push_str(&line_diff(&before, &after));
            out.push('\n');
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(out)))
    }
);

simple_builtin!(
    PreviewBuiltin,
    "preview",
    "preview",
    "Preview modified content without saving",
    &["open file.json | update a.b c | preview"],
    |_args, input, _ctx| {
        let value = pipeline_to_value(input)?;
        let docs = as_documents(&value)?;
        let mut rows = Vec::new();
        for doc in docs {
            let mut row = Record::new();
            row.insert(
                "path".into(),
                Value::String(doc_path(&doc).unwrap_or_else(|| "<memory>".to_string())),
            );
            row.insert(
                "dirty".into(),
                doc.get(DIRTY).cloned().unwrap_or(Value::Bool(false)),
            );
            let sample = doc
                .get(VALUE)
                .cloned()
                .map(|v| v.to_string())
                .unwrap_or_default();
            row.insert(
                "sample".into(),
                Value::String(sample.chars().take(200).collect::<String>()),
            );
            rows.push(row);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn glob_matcher(pattern: &str) -> anyhow::Result<globset::GlobSet> {
    let mut b = GlobSetBuilder::new();
    b.add(Glob::new(pattern)?);
    Ok(b.build()?)
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn parse_format_name(name: &str) -> FileFormat {
    match name.to_ascii_lowercase().as_str() {
        "json" => FileFormat::Json,
        "yaml" | "yml" => FileFormat::Yaml,
        "toml" => FileFormat::Toml,
        "csv" => FileFormat::Csv,
        "xml" => FileFormat::Xml,
        "ini" => FileFormat::Ini,
        "sqlite" | "db" => FileFormat::Sqlite,
        "xlsx" => FileFormat::Xlsx,
        "txt" | "text" | "md" | "html" | "raw" => FileFormat::Text,
        _ => FileFormat::Unknown,
    }
}

fn detect_format(path: &Path) -> FileFormat {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    parse_format_name(ext)
}

pub(crate) fn decode_path(
    path: &Path,
    raw: bool,
    force_format: Option<&str>,
) -> anyhow::Result<Value> {
    let format = force_format
        .map(parse_format_name)
        .unwrap_or_else(|| detect_format(path));
    let val = if raw {
        Value::String(fs::read_to_string(path)?)
    } else {
        match format {
            FileFormat::Sqlite => {
                let mut rec = Record::new();
                rec.insert("kind".into(), Value::String("sqlite".into()));
                rec.insert("path".into(), Value::String(path.display().to_string()));
                Value::Record(rec)
            }
            FileFormat::Xlsx => {
                let mut rec = Record::new();
                rec.insert("kind".into(), Value::String("xlsx".into()));
                rec.insert("path".into(), Value::String(path.display().to_string()));
                rec.insert(
                    "sheets".into(),
                    Value::List(
                        list_xlsx_sheets(path)?
                            .into_iter()
                            .map(Value::String)
                            .collect(),
                    ),
                );
                Value::Record(rec)
            }
            _ => {
                let text = fs::read_to_string(path)?;
                decode_with_format(format, &text)?
            }
        }
    };
    Ok(document_record(&path.display().to_string(), format, val))
}

fn decode_with_format(format: FileFormat, text: &str) -> anyhow::Result<Value> {
    match format {
        FileFormat::Json => JsonCodec.decode(text),
        FileFormat::Yaml => YamlCodec.decode(text),
        FileFormat::Toml => TomlCodec.decode(text),
        FileFormat::Csv => CsvCodec.decode(text),
        _ => TextCodec.decode(text),
    }
}

pub(crate) fn sqlite_table(path: &Path, table_name: &str) -> anyhow::Result<Value> {
    let conn = Connection::open(path)?;
    let sql = format!("SELECT * FROM \"{}\"", table_name.replace('"', "\"\""));
    let mut stmt = conn.prepare(&sql)?;
    let columns = stmt
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut query = stmt.query([])?;
    let mut rows = Vec::new();
    while let Some(row) = query.next()? {
        let mut out = Record::new();
        for (idx, name) in columns.iter().enumerate() {
            let v = row.get_ref(idx)?;
            let cell = match v {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(i) => Value::Int(i),
                rusqlite::types::ValueRef::Real(f) => Value::Float(f),
                rusqlite::types::ValueRef::Text(t) => {
                    Value::String(String::from_utf8_lossy(t).to_string())
                }
                rusqlite::types::ValueRef::Blob(b) => Value::Binary(b.to_vec()),
            };
            out.insert(name.clone(), cell);
        }
        rows.push(out);
    }
    Ok(Value::Table(Table::new(rows)))
}

pub(crate) fn sqlite_query(path: &Path, sql: &str) -> anyhow::Result<Value> {
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare(sql)?;
    let columns = stmt
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut query = stmt.query([])?;
    let mut rows = Vec::new();
    while let Some(row) = query.next()? {
        let mut out = Record::new();
        for (idx, name) in columns.iter().enumerate() {
            let v = row.get_ref(idx)?;
            let cell = match v {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(i) => Value::Int(i),
                rusqlite::types::ValueRef::Real(f) => Value::Float(f),
                rusqlite::types::ValueRef::Text(t) => {
                    Value::String(String::from_utf8_lossy(t).to_string())
                }
                rusqlite::types::ValueRef::Blob(b) => Value::Binary(b.to_vec()),
            };
            out.insert(name.clone(), cell);
        }
        rows.push(out);
    }
    Ok(Value::Table(Table::new(rows)))
}

pub(crate) fn list_xlsx_sheets(path: &Path) -> anyhow::Result<Vec<String>> {
    use calamine::{Reader, open_workbook_auto};
    let workbook = open_workbook_auto(path)?;
    Ok(workbook.sheet_names().to_vec())
}

pub(crate) fn xlsx_sheet(path: &Path, sheet_name: &str) -> anyhow::Result<Value> {
    use calamine::{Reader, open_workbook_auto};
    let mut workbook = open_workbook_auto(path)?;
    let range = workbook.worksheet_range(sheet_name)?;
    let mut it = range.rows();
    let headers = it
        .next()
        .map(|r| r.iter().map(|c| c.to_string()).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut rows = Vec::new();
    for r in it {
        let mut row = Record::new();
        for (idx, c) in r.iter().enumerate() {
            let key = headers
                .get(idx)
                .cloned()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("col{}", idx + 1));
            row.insert(key, Value::String(c.to_string()));
        }
        rows.push(row);
    }
    Ok(Value::Table(Table::new(rows)))
}

fn encode_with_format(format: FileFormat, value: &Value) -> anyhow::Result<String> {
    match format {
        FileFormat::Json => JsonCodec.encode(value),
        FileFormat::Yaml => YamlCodec.encode(value),
        FileFormat::Toml => TomlCodec.encode(value),
        FileFormat::Csv => CsvCodec.encode(value),
        _ => TextCodec.encode(value),
    }
}

fn document_record(path: &str, format: FileFormat, value: Value) -> Value {
    let mut meta = Record::new();
    meta.insert("source_path".into(), Value::String(path.to_string()));
    meta.insert(
        "source_format".into(),
        Value::String(format!("{:?}", format).to_ascii_lowercase()),
    );
    let mut rec = Record::new();
    rec.insert(META.into(), Value::Record(meta));
    rec.insert(ORIGINAL.into(), value.clone());
    rec.insert(VALUE.into(), value);
    rec.insert(DIRTY.into(), Value::Bool(false));
    Value::Record(rec)
}

fn extract_paths(input: PipelineData) -> anyhow::Result<Vec<String>> {
    let value = pipeline_to_value(input)?;
    match value {
        Value::Table(t) => Ok(t
            .rows
            .into_iter()
            .filter_map(|r| r.get("path").cloned())
            .map(|v| v.to_string())
            .collect()),
        Value::List(vs) => Ok(vs
            .into_iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s),
                Value::Record(r) => r.get("path").map(|x| x.to_string()),
                _ => None,
            })
            .collect()),
        Value::String(s) => Ok(vec![s]),
        _ => bail!("open-each expects files table/list"),
    }
}

fn as_documents(value: &Value) -> anyhow::Result<Vec<Record>> {
    match value {
        Value::Record(r) if r.contains_key(META) && r.contains_key(VALUE) => Ok(vec![r.clone()]),
        Value::List(vs) => {
            let docs = vs
                .iter()
                .filter_map(|v| match v {
                    Value::Record(r) if r.contains_key(META) && r.contains_key(VALUE) => {
                        Some(r.clone())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if docs.is_empty() {
                bail!("expected document(s) from open-each/edit")
            }
            Ok(docs)
        }
        _ => bail!("expected document(s) from open-each/edit"),
    }
}

fn doc_path(rec: &Record) -> Option<String> {
    rec.get(META)
        .and_then(|v| v.as_record())
        .and_then(|m| m.get("source_path"))
        .map(|v| v.to_string())
}

fn encode_document_with_opts(
    rec: &Record,
    target: &Path,
    format_override: Option<&str>,
    pretty: bool,
) -> anyhow::Result<String> {
    let value = rec
        .get(VALUE)
        .ok_or_else(|| anyhow!("document has no _value"))?;
    let format = format_override.map(parse_format_name).unwrap_or_else(|| {
        rec.get(META)
            .and_then(|v| v.as_record())
            .and_then(|m| m.get("source_format"))
            .map(|v| parse_format_name(&v.to_string()))
            .unwrap_or_else(|| detect_format(target))
    });
    encode_with_format_with_opts(format, value, pretty)
}

fn line_diff(before: &str, after: &str) -> String {
    let b = before.lines().collect::<Vec<_>>();
    let a = after.lines().collect::<Vec<_>>();
    let max = b.len().max(a.len());
    let mut out = String::new();
    for i in 0..max {
        match (b.get(i), a.get(i)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                out.push_str(&format!("-{x}\n+{y}\n"));
            }
            (Some(x), None) => out.push_str(&format!("-{x}\n")),
            (None, Some(y)) => out.push_str(&format!("+{y}\n")),
            (None, None) => {}
        }
    }
    out
}

fn to_csv_text(value: &Value) -> String {
    match value {
        Value::Table(t) => {
            let mut out = String::new();
            out.push_str(&t.columns.join(","));
            out.push('\n');
            for row in &t.rows {
                let line = t
                    .columns
                    .iter()
                    .map(|c| row.get(c).map(|v| v.to_string()).unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(",");
                out.push_str(&line);
                out.push('\n');
            }
            out
        }
        _ => value.to_string(),
    }
}

fn encode_plain_value(
    value: &Value,
    target: &Path,
    format_override: Option<&str>,
    pretty: bool,
) -> anyhow::Result<String> {
    let fmt = format_override
        .map(parse_format_name)
        .unwrap_or_else(|| detect_format(target));
    encode_with_format_with_opts(fmt, value, pretty)
}

fn encode_with_format_with_opts(
    format: FileFormat,
    value: &Value,
    pretty: bool,
) -> anyhow::Result<String> {
    match format {
        FileFormat::Json => {
            if pretty {
                to_json_string(value)
            } else {
                Ok(serde_json::to_string(&serde_json::from_str::<
                    serde_json::Value,
                >(
                    &to_json_string(value)?
                )?)?)
            }
        }
        FileFormat::Yaml => to_yaml_string(value),
        FileFormat::Toml => to_toml_string(value),
        FileFormat::Csv => Ok(to_csv_text(value)),
        _ => encode_with_format(format, value),
    }
}

fn write_one(
    target: &Path,
    content: &str,
    backup: bool,
    dry_run: bool,
    append: bool,
    rows: &mut Vec<Record>,
) -> anyhow::Result<()> {
    let old = fs::read_to_string(target).ok();
    let new_content = if append {
        let mut merged = old.clone().unwrap_or_default();
        merged.push_str(content);
        merged
    } else {
        content.to_string()
    };
    let mut changed = true;
    if let Some(prev) = &old {
        changed = prev != &new_content;
    }
    if !dry_run {
        if backup && target.exists() {
            let bak = target.with_extension(format!(
                "{}.bak",
                target.extension().and_then(|s| s.to_str()).unwrap_or("bak")
            ));
            fs::copy(target, bak)?;
        }
        fs::write(target, new_content.as_bytes())?;
    }
    let mut row = Record::new();
    row.insert("path".into(), Value::String(target.display().to_string()));
    row.insert("changed".into(), Value::Bool(changed));
    row.insert("written".into(), Value::Bool(!dry_run));
    rows.push(row);
    Ok(())
}

fn discover_files(args: &[String], cwd: &Path) -> anyhow::Result<Table> {
    let mut show_hidden = false;
    let mut follow_links = false;
    let mut max_depth: Option<usize> = None;
    let mut absolute = false;
    let mut relative = false;
    let mut pattern = None::<String>;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--hidden" => show_hidden = true,
            "--follow-links" => follow_links = true,
            "--max-depth" | "--depth" => {
                max_depth = args.get(i + 1).and_then(|s| s.parse::<usize>().ok());
                i += 1;
            }
            "--absolute" => absolute = true,
            "--relative" => relative = true,
            other if !other.starts_with('-') && pattern.is_none() => {
                pattern = Some(args[i].clone())
            }
            _ => {}
        }
        i += 1;
    }
    let pattern = pattern.ok_or_else(|| anyhow!("glob/files expects a glob pattern"))?;
    let matcher = glob_matcher(&pattern)?;
    let mut walker = WalkDir::new(cwd).follow_links(follow_links);
    if let Some(depth) = max_depth {
        walker = walker.max_depth(depth);
    }
    let mut rows = Vec::new();
    for entry in walker.into_iter().filter_map(Result::ok) {
        let p = entry.path();
        if p == cwd {
            continue;
        }
        let rel = p
            .strip_prefix(cwd)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        if !matcher.is_match(&rel) {
            continue;
        }
        if !show_hidden && is_hidden(p) {
            continue;
        }
        let md = fs::symlink_metadata(p)?;
        let mut row = Record::new();
        let path_text = if absolute {
            p.canonicalize()
                .unwrap_or_else(|_| p.to_path_buf())
                .display()
                .to_string()
        } else if relative || !p.is_absolute() {
            rel.clone()
        } else {
            p.display().to_string()
        };
        row.insert("path".into(), Value::String(path_text));
        row.insert(
            "name".into(),
            Value::String(
                p.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string(),
            ),
        );
        row.insert(
            "extension".into(),
            Value::String(
                p.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string(),
            ),
        );
        row.insert(
            "size".into(),
            Value::Filesize(FilesizeValue { bytes: md.len() }),
        );
        row.insert("is_dir".into(), Value::Bool(md.is_dir()));
        row.insert(
            "modified".into(),
            Value::String(super::fs_builtins::format_modified(md.modified().ok())),
        );
        rows.push(row);
    }
    Ok(Table::new(rows))
}

fn resolve_watch_path(cwd: &Path, p: &str) -> String {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb.display().to_string()
    } else {
        cwd.join(pb).display().to_string()
    }
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn run_log_row(cmd: &str, out: &std::process::Output) -> Record {
    let mut row = Record::new();
    row.insert("command".into(), Value::String(cmd.to_string()));
    row.insert(
        "exit_code".into(),
        Value::Int(out.status.code().unwrap_or(1) as i64),
    );
    row.insert(
        "stdout".into(),
        Value::String(String::from_utf8_lossy(&out.stdout).to_string()),
    );
    row.insert(
        "stderr".into(),
        Value::String(String::from_utf8_lossy(&out.stderr).to_string()),
    );
    row
}

fn normalize_event_kind(kind: &notify::EventKind) -> String {
    use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
    match kind {
        notify::EventKind::Create(CreateKind::Any)
        | notify::EventKind::Create(CreateKind::File)
        | notify::EventKind::Create(CreateKind::Folder) => "created".to_string(),
        notify::EventKind::Modify(ModifyKind::Name(RenameMode::Any))
        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::From))
        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::To))
        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => "renamed".to_string(),
        notify::EventKind::Modify(_) => "modified".to_string(),
        notify::EventKind::Remove(RemoveKind::Any)
        | notify::EventKind::Remove(RemoveKind::File)
        | notify::EventKind::Remove(RemoveKind::Folder) => "deleted".to_string(),
        _ => "other".to_string(),
    }
}

pub(crate) fn apply_replace(
    value: Value,
    from: &str,
    to: &str,
    regex: bool,
    ignore_case: bool,
    recursive: bool,
    path: Option<&str>,
) -> anyhow::Result<Value> {
    let re = if regex {
        let p = if ignore_case {
            format!("(?i){from}")
        } else {
            from.to_string()
        };
        Some(Regex::new(&p)?)
    } else {
        None
    };
    Ok(match value {
        Value::Record(mut r) if r.contains_key(META) && r.contains_key(VALUE) => {
            let current = r.get(VALUE).cloned().unwrap_or(Value::Null);
            let new_val =
                replace_value(current, from, to, re.as_ref(), ignore_case, recursive, path)?;
            let dirty = new_val != r.get(ORIGINAL).cloned().unwrap_or(Value::Null);
            r.insert(VALUE.into(), new_val);
            r.insert(DIRTY.into(), Value::Bool(dirty));
            Value::Record(r)
        }
        other => replace_value(other, from, to, re.as_ref(), ignore_case, recursive, path)?,
    })
}

fn replace_value(
    value: Value,
    from: &str,
    to: &str,
    re: Option<&Regex>,
    ignore_case: bool,
    recursive: bool,
    path: Option<&str>,
) -> anyhow::Result<Value> {
    if let Some(path) = path {
        return replace_at_path(value, path, from, to, re, ignore_case);
    }
    Ok(match value {
        Value::String(s) => Value::String(replace_text(&s, from, to, re, ignore_case)),
        Value::List(items) if recursive => Value::List(
            items
                .into_iter()
                .map(|v| replace_value(v, from, to, re, ignore_case, true, None))
                .collect::<anyhow::Result<Vec<_>>>()?,
        ),
        Value::Record(rec) if recursive => {
            let mut out = Record::new();
            for (k, v) in rec {
                out.insert(k, replace_value(v, from, to, re, ignore_case, true, None)?);
            }
            Value::Record(out)
        }
        Value::Table(mut t) if recursive => {
            for row in &mut t.rows {
                for val in row.values_mut() {
                    *val = replace_value(val.clone(), from, to, re, ignore_case, true, None)?;
                }
            }
            Value::Table(t)
        }
        Value::Record(_) | Value::Table(_) | Value::List(_) => {
            bail!("replace on structured data requires --recursive or --path")
        }
        v => v,
    })
}

fn replace_at_path(
    value: Value,
    path: &str,
    from: &str,
    to: &str,
    re: Option<&Regex>,
    ignore_case: bool,
) -> anyhow::Result<Value> {
    let parts = path
        .split('.')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(value);
    }
    fn walk(
        cur: Value,
        parts: &[&str],
        from: &str,
        to: &str,
        re: Option<&Regex>,
        ignore_case: bool,
    ) -> anyhow::Result<Value> {
        if parts.is_empty() {
            return Ok(cur);
        }
        if parts.len() == 1 {
            if let Value::Record(mut r) = cur {
                if let Some(v) = r.get(parts[0]).cloned() {
                    if let Value::String(s) = v {
                        r.insert(
                            parts[0].to_string(),
                            Value::String(replace_text(&s, from, to, re, ignore_case)),
                        );
                    }
                }
                return Ok(Value::Record(r));
            }
            return Ok(cur);
        }
        if let Value::Record(mut r) = cur {
            if let Some(next) = r.get(parts[0]).cloned() {
                r.insert(
                    parts[0].to_string(),
                    walk(next, &parts[1..], from, to, re, ignore_case)?,
                );
            }
            return Ok(Value::Record(r));
        }
        Ok(cur)
    }
    walk(value, &parts, from, to, re, ignore_case)
}

fn replace_text(s: &str, from: &str, to: &str, re: Option<&Regex>, ignore_case: bool) -> String {
    if let Some(re) = re {
        return re.replace_all(s, to).to_string();
    }
    if ignore_case {
        let p = Regex::new(&format!("(?i){}", regex::escape(from))).expect("regex literal");
        return p.replace_all(s, to).to_string();
    }
    s.replace(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("dosh_file_pipeline_{n}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn replace_regex_works() {
        let v = Value::String("goodnight".into());
        let out = apply_replace(
            v,
            "good(.*?)night",
            "good $1 night",
            true,
            false,
            false,
            None,
        )
        .unwrap();
        assert_eq!(out.to_string(), "good  night");
    }

    #[test]
    fn save_requires_target() {
        let mut env = dosh_env::EnvContext::from_current_dir().unwrap();
        let out = SaveBuiltin.run(
            &[],
            PipelineData::Value(Value::String("a".into())),
            &mut BuiltinContext { env: &mut env },
        );
        assert!(out.is_err());
    }

    #[test]
    fn open_each_and_save_in_place() {
        let dir = temp_dir();
        let file = dir.join("a.txt");
        fs::write(&file, "hello").unwrap();
        let mut row = Record::new();
        row.insert("path".into(), Value::String(file.display().to_string()));
        let list = Value::Table(Table::new(vec![row]));
        let mut env = dosh_env::EnvContext::new(dir.clone());
        let opened = OpenEachBuiltin
            .run(
                &["--raw".into()],
                PipelineData::Value(list),
                &mut BuiltinContext { env: &mut env },
            )
            .unwrap()
            .output;
        let replaced = Value::List(
            as_documents(&pipeline_to_value(opened).unwrap())
                .unwrap()
                .into_iter()
                .map(|mut d| {
                    d.insert(VALUE.into(), Value::String("hi".into()));
                    d.insert(DIRTY.into(), Value::Bool(true));
                    Value::Record(d)
                })
                .collect(),
        );
        SaveBuiltin
            .run(
                &["--in-place".into()],
                PipelineData::Value(replaced),
                &mut BuiltinContext { env: &mut env },
            )
            .unwrap();
        assert_eq!(fs::read_to_string(file).unwrap(), "hi");
        let _ = fs::remove_dir_all(dir);
    }
}
