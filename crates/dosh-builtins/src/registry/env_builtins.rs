use super::*;
use crate::registry::{factory, simple_builtin};
use crate::render::{TableRenderOptions, render_value_as_table};
use anyhow::{anyhow, bail};
use std::process::Command;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(EnvBuiltin),
        factory!(ExportBuiltin),
        factory!(UnsetBuiltin),
        factory!(PathBuiltin),
        factory!(WhichBuiltin),
        factory!(WhereisBuiltin),
    ]
}

simple_builtin!(
    EnvBuiltin,
    "env",
    "env",
    "Show environment variables",
    &["env"],
    |_args, _input, _ctx| {
        let mut rows = Vec::new();
        for (k, v) in std::env::vars() {
            let mut row = Record::new();
            row.insert("key".into(), Value::String(k));
            row.insert("value".into(), Value::String(v));
            rows.push(row);
        }
        let table = Value::Table(Table::new(rows));
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            render_value_as_table(&table, TableRenderOptions::default()),
        )))
    }
);

simple_builtin!(
    ExportBuiltin,
    "export",
    "export <KEY=VALUE|KEY VALUE>",
    "Set environment variable for current shell process",
    &["export EDITOR=nvim"],
    |args, _input, _ctx| {
        if args.is_empty() {
            bail!("export expects KEY=VALUE or KEY VALUE")
        }
        let (key, value) = if args.len() == 1 && args[0].contains('=') {
            let (k, v) = args[0]
                .split_once('=')
                .ok_or_else(|| anyhow!("invalid export assignment"))?;
            (k.to_string(), v.to_string())
        } else if args.len() >= 2 {
            (args[0].clone(), args[1..].join(" "))
        } else {
            bail!("export expects KEY=VALUE or KEY VALUE")
        };
        unsafe { std::env::set_var(&key, &value) };
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "exported {key}={value}"
        ))))
    }
);

simple_builtin!(
    UnsetBuiltin,
    "unset",
    "unset <KEY>",
    "Remove environment variable from current shell process",
    &["unset EDITOR"],
    |args, _input, _ctx| {
        let key = args.first().ok_or_else(|| anyhow!("unset expects KEY"))?;
        unsafe { std::env::remove_var(key) };
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "unset {key}"
        ))))
    }
);

simple_builtin!(
    PathBuiltin,
    "path",
    "path [add <dir>|remove <dir>]",
    "Inspect or modify PATH entries",
    &["path"],
    |args, _input, _ctx| {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let current = std::env::var("PATH").unwrap_or_default();
        let mut entries = current
            .split(sep)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if args.first().map(|s| s.as_str()) == Some("add") {
            let dir = args
                .get(1)
                .ok_or_else(|| anyhow!("path add expects directory"))?;
            if !entries.iter().any(|v| v == dir) {
                entries.insert(0, dir.clone());
            }
            unsafe { std::env::set_var("PATH", entries.join(&sep.to_string())) };
        } else if args.first().map(|s| s.as_str()) == Some("remove") {
            let dir = args
                .get(1)
                .ok_or_else(|| anyhow!("path remove expects directory"))?;
            entries.retain(|v| v != dir);
            unsafe { std::env::set_var("PATH", entries.join(&sep.to_string())) };
        } else if !args.is_empty() {
            bail!("path usage: path [add <dir>|remove <dir>]")
        }
        let mut rows = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let mut row = Record::new();
            row.insert("index".into(), Value::Int(i as i64));
            row.insert("path".into(), Value::String(entry.clone()));
            rows.push(row);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            render_value_as_table(
                &Value::Table(Table::new(rows)),
                TableRenderOptions::default(),
            ),
        )))
    }
);

simple_builtin!(
    WhichBuiltin,
    "which",
    "which <cmd>",
    "Find executable",
    &["which git"],
    |args, _input, _ctx| {
        let cmd = args
            .first()
            .ok_or_else(|| anyhow!("which expects command"))?;
        #[cfg(target_os = "windows")]
        let output = Command::new("where").arg(cmd).output()?;
        #[cfg(not(target_os = "windows"))]
        let output = Command::new("which").arg(cmd).output()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        )))
    }
);

simple_builtin!(
    WhereisBuiltin,
    "whereis",
    "whereis <cmd>",
    "Find all command locations on PATH",
    &["whereis git"],
    |args, _input, _ctx| {
        let cmd = args
            .first()
            .ok_or_else(|| anyhow!("whereis expects command"))?;
        #[cfg(target_os = "windows")]
        let output = Command::new("where").arg(cmd).output()?;
        #[cfg(not(target_os = "windows"))]
        let output = Command::new("whereis").arg(cmd).output()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        )))
    }
);
