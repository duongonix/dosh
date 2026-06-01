use super::*;
use crate::helpers::{copy_dir_recursive, resolve_path};
use crate::registry::rm_engine::{
    DeleteSummary, RmMode, RmOptions, detect_fast_candidate, dry_run_record, execute_delete,
    reject_protected_path, scan_path,
};
use crate::registry::{factory, simple_builtin};
use crate::render::{TableRenderOptions, render_value_as_table};
use anyhow::{anyhow, bail};
use chrono::{DateTime, Local};
use globset::{Glob, GlobMatcher};
use regex::Regex;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use walkdir::WalkDir;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(MkdirBuiltin),
        factory!(LsBuiltin),
        factory!(OpenBuiltin),
        factory!(RmBuiltin),
        factory!(CpBuiltin),
        factory!(MvBuiltin),
        factory!(TouchBuiltin),
        factory!(CatBuiltin),
        factory!(HeadBuiltin),
        factory!(TailBuiltin),
        factory!(DuBuiltin),
        factory!(StatBuiltin),
        factory!(FindBuiltin),
        factory!(WatchBuiltin),
        factory!(ChmodBuiltin),
        factory!(LnBuiltin),
    ]
}

simple_builtin!(
    MkdirBuiltin,
    "mkdir",
    "mkdir <path>",
    "Create directory",
    &["mkdir tmp"],
    |args, _input, ctx| {
        let path = args.first().ok_or_else(|| anyhow!("mkdir expects path"))?;
        fs::create_dir_all(resolve_path(ctx.env.cwd(), path))?;
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    LsBuiltin,
    "ls",
    "ls [-a] [-l] [-R] [path]",
    "List files as structured table",
    &["ls -la", "ls -R src"],
    |args, _input, ctx| {
        let mut show_all = false;
        let mut long_view = false;
        let mut recursive = false;
        let mut target: Option<String> = None;
        for arg in args {
            match arg.as_str() {
                "-a" | "--all" => show_all = true,
                "-l" | "--long" => long_view = true,
                "-R" | "--recursive" => recursive = true,
                "-la" | "-al" => {
                    show_all = true;
                    long_view = true;
                }
                s if s.starts_with('-') => {}
                _ => target = Some(arg.clone()),
            }
        }
        let base = target
            .map(|p| resolve_path(ctx.env.cwd(), &p))
            .unwrap_or_else(|| ctx.env.cwd().to_path_buf());
        let mut rows = Vec::new();

        if recursive {
            for entry in WalkDir::new(&base).into_iter().filter_map(Result::ok) {
                let path = entry.path();
                if path == base {
                    continue;
                }
                if !show_all && is_hidden(path) {
                    continue;
                }
                rows.push(file_row(path, long_view)?);
            }
        } else {
            for entry in fs::read_dir(&base)? {
                let entry = entry?;
                let path = entry.path();
                if !show_all && is_hidden(&path) {
                    continue;
                }
                rows.push(file_row(&path, long_view)?);
            }
        }

        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    OpenBuiltin,
    "open",
    "open <file> [--raw] [--format fmt] [--encoding utf-8]",
    "Open and parse file",
    &["open package.json"],
    |args, _input, ctx| {
        let mut raw = false;
        let mut force_format: Option<String> = None;
        let path_arg = args
            .iter()
            .find(|a| !a.starts_with('-'))
            .ok_or_else(|| anyhow!("open expects a file path"))?;
        let mut i = 0usize;
        while i < args.len() {
            match args[i].as_str() {
                "--raw" => raw = true,
                "--format" => {
                    force_format = args.get(i + 1).cloned();
                    i += 1;
                }
                "--encoding" => {
                    i += 1;
                }
                _ => {}
            }
            i += 1;
        }
        let path = resolve_path(ctx.env.cwd(), path_arg);
        let value =
            super::file_pipeline_builtins::decode_path(&path, raw, force_format.as_deref())?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(value)))
    }
);

simple_builtin!(
    RmBuiltin,
    "rm",
    "rm [-r] [-f|--force] [-F|--fast] [-t|--trash] [-n|--dry-run] [-s|--safe] <path...>",
    "Remove files/directories with safety, dry-run and fast engine.",
    &[
        "rm -r node_modules --fast",
        "rm build --dry-run",
        "rm dist --trash"
    ],
    |args, _input, ctx| {
        let mut recursive = false;
        let mut force = false;
        let mut fast = false;
        let mut trash = false;
        let mut dry_run = false;
        let mut safe = false;
        let mut targets = Vec::new();
        for a in args {
            match a.as_str() {
                "-r" | "-R" | "--recursive" => recursive = true,
                "-f" | "--force" => force = true,
                "-F" | "--fast" => fast = true,
                "-t" | "--trash" => trash = true,
                "-n" | "--dry-run" => dry_run = true,
                "-s" | "--safe" => safe = true,
                "--permanent" => {}
                _ if a.starts_with('-') => {}
                _ => targets.push(a.clone()),
            }
        }
        if targets.is_empty() {
            bail!("rm expects path(s)")
        }
        if safe {
            force = false;
            fast = false;
        }

        let mut rows = Vec::new();
        let mut total = DeleteSummary::default();

        for t in targets {
            let p = resolve_path(ctx.env.cwd(), &t);
            if !p.exists() {
                if force {
                    continue;
                }
                bail!("path not found: {}", p.display());
            }
            if let Err(e) = reject_protected_path(&p) {
                if !force {
                    return Err(e);
                }
                continue;
            }
            let mode = if trash {
                RmMode::Trash
            } else if fast || detect_fast_candidate(&p) {
                RmMode::Fast
            } else {
                RmMode::Normal
            };
            let stats = scan_path(&p).stats;
            if dry_run {
                rows.push(dry_run_record(&p, mode, &stats));
                continue;
            }
            let opts = RmOptions { recursive, mode };
            match execute_delete(&p, &opts) {
                Ok(s) => {
                    total.deleted_files += s.deleted_files;
                    total.deleted_dirs += s.deleted_dirs;
                    total.skipped += s.skipped;
                    total.elapsed_ms += s.elapsed_ms;
                    total.mode = s.mode.clone();
                    total.failed.extend(s.failed);
                }
                Err(e) => {
                    if !force {
                        return Err(e);
                    }
                }
            }
        }
        if dry_run {
            return Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
                Table::new(rows),
            ))));
        }
        if !total.failed.is_empty() && !force {
            let first = &total.failed[0];
            bail!("delete failed: {} ({})", first.0.display(), first.1);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(
            total.to_record(),
        ))))
    }
);

simple_builtin!(
    MvBuiltin,
    "mv",
    "mv [--overwrite|--no-clobber] <src> <dst>",
    "Move/rename with overwrite policy",
    &["mv --no-clobber a.txt b.txt"],
    |args, _input, ctx| {
        let mut overwrite = true;
        let mut paths = Vec::new();
        for a in args {
            match a.as_str() {
                "--overwrite" => overwrite = true,
                "--no-clobber" | "-n" => overwrite = false,
                _ if a.starts_with('-') => {}
                _ => paths.push(a.clone()),
            }
        }
        if paths.len() != 2 {
            bail!("mv expects <src> <dst>")
        }
        let src = resolve_path(ctx.env.cwd(), &paths[0]);
        let dst = resolve_path(ctx.env.cwd(), &paths[1]);
        if dst.exists() && !overwrite {
            bail!("destination exists (use --overwrite)")
        }
        if dst.exists() {
            if dst.is_dir() {
                fs::remove_dir_all(&dst)?;
            } else {
                fs::remove_file(&dst)?;
            }
        }
        fs::rename(src, dst)?;
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    CpBuiltin,
    "cp",
    "cp [-r] [--overwrite|--no-clobber] <src> <dst>",
    "Copy files/directories",
    &["cp -r src backup"],
    |args, _input, ctx| {
        let mut recursive = false;
        let mut overwrite = true;
        let mut paths = Vec::new();
        for a in args {
            match a.as_str() {
                "-r" | "-R" | "--recursive" => recursive = true,
                "--overwrite" => overwrite = true,
                "--no-clobber" | "-n" => overwrite = false,
                _ if a.starts_with('-') => {}
                _ => paths.push(a.clone()),
            }
        }
        if paths.len() != 2 {
            bail!("cp expects <src> <dst>")
        }
        let src = resolve_path(ctx.env.cwd(), &paths[0]);
        let dst = resolve_path(ctx.env.cwd(), &paths[1]);

        if dst.exists() && !overwrite {
            bail!("destination exists (use --overwrite)")
        }
        if src.is_dir() {
            if !recursive {
                bail!("cp directory requires -r")
            }
            if dst.exists() {
                if dst.is_dir() {
                    fs::remove_dir_all(&dst)?;
                } else {
                    fs::remove_file(&dst)?;
                }
            }
            copy_dir_recursive(&src, &dst)?;
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            if dst.exists() {
                fs::remove_file(&dst)?;
            }
            fs::copy(src, dst)?;
        }
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    TouchBuiltin,
    "touch",
    "touch <file>",
    "Create/update file",
    &["touch notes.txt"],
    |args, _input, ctx| {
        let path = resolve_path(
            ctx.env.cwd(),
            args.first()
                .ok_or_else(|| anyhow!("touch expects file path"))?,
        );
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::File::create(path)?;
        } else {
            let mut f = fs::OpenOptions::new().append(true).open(path)?;
            f.write_all(b"")?;
        }
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    CatBuiltin,
    "cat",
    "cat <file>",
    "Read file content",
    &["cat Cargo.toml"],
    |args, _input, ctx| {
        let path = resolve_path(
            ctx.env.cwd(),
            args.first()
                .ok_or_else(|| anyhow!("cat expects file path"))?,
        );
        let mut s = String::new();
        fs::File::open(path)?.read_to_string(&mut s)?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(s)))
    }
);

simple_builtin!(
    HeadBuiltin,
    "head",
    "head <file> [n]",
    "Print first n lines",
    &["head README.md 20"],
    |args, _input, ctx| {
        let path = resolve_path(
            ctx.env.cwd(),
            args.first()
                .ok_or_else(|| anyhow!("head expects file path"))?,
        );
        let n = args
            .get(1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);
        let lines = BufReader::new(fs::File::open(path)?)
            .lines()
            .take(n)
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(lines.join("\n"))))
    }
);

simple_builtin!(
    TailBuiltin,
    "tail",
    "tail <file> [n]",
    "Print last n lines",
    &["tail app.log 50"],
    |args, _input, ctx| {
        let path = resolve_path(
            ctx.env.cwd(),
            args.first()
                .ok_or_else(|| anyhow!("tail expects file path"))?,
        );
        let n = args
            .get(1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);
        let mut lines = BufReader::new(fs::File::open(path)?)
            .lines()
            .collect::<std::io::Result<Vec<_>>>()?;
        if lines.len() > n {
            lines = lines.split_off(lines.len() - n);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(lines.join("\n"))))
    }
);

simple_builtin!(
    DuBuiltin,
    "du",
    "du [path]",
    "Estimate disk usage recursively",
    &["du"],
    |args, _input, ctx| {
        let root = args
            .first()
            .map(|v| resolve_path(ctx.env.cwd(), v))
            .unwrap_or_else(|| ctx.env.cwd().to_path_buf());
        let mut rows = Vec::new();
        for entry in WalkDir::new(&root).into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if p.is_file() {
                let size = p.metadata().map(|m| m.len()).unwrap_or(0);
                let mut row = Record::new();
                row.insert("path".into(), Value::String(p.display().to_string()));
                row.insert(
                    "size".into(),
                    Value::Filesize(FilesizeValue { bytes: size }),
                );
                rows.push(row);
            }
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
    StatBuiltin,
    "stat",
    "stat <path>",
    "Show rich file metadata",
    &["stat Cargo.toml"],
    |args, _input, ctx| {
        let path = resolve_path(
            ctx.env.cwd(),
            args.first().ok_or_else(|| anyhow!("stat expects path"))?,
        );
        let md = fs::symlink_metadata(&path)?;
        let mut rec = Record::new();
        rec.insert("path".into(), Value::String(path.display().to_string()));
        rec.insert("exists".into(), Value::Bool(path.exists()));
        rec.insert("is_dir".into(), Value::Bool(md.is_dir()));
        rec.insert("is_file".into(), Value::Bool(md.is_file()));
        rec.insert(
            "is_symlink".into(),
            Value::Bool(md.file_type().is_symlink()),
        );
        rec.insert(
            "size".into(),
            Value::Filesize(FilesizeValue { bytes: md.len() }),
        );
        rec.insert("readonly".into(), Value::Bool(md.permissions().readonly()));
        rec.insert(
            "modified".into(),
            Value::String(format_modified(md.modified().ok())),
        );
        rec.insert(
            "accessed".into(),
            Value::String(format_modified(md.accessed().ok())),
        );
        rec.insert(
            "created".into(),
            Value::String(format_modified(md.created().ok())),
        );
        rec.insert(
            "extension".into(),
            Value::String(
                path.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
                    .to_string(),
            ),
        );
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    FindBuiltin,
    "find",
    "find [--glob p|--regex r|--max-depth n] [path]",
    "Find files with glob/regex/depth filters",
    &["find --glob '*.rs' src"],
    |args, _input, ctx| {
        let mut glob_pat: Option<GlobMatcher> = None;
        let mut regex_pat: Option<Regex> = None;
        let mut max_depth: Option<usize> = None;
        let mut root: Option<PathBuf> = None;
        let mut i = 0usize;
        while i < args.len() {
            match args[i].as_str() {
                "--glob" => {
                    let p = args
                        .get(i + 1)
                        .ok_or_else(|| anyhow!("--glob expects pattern"))?;
                    glob_pat = Some(Glob::new(p)?.compile_matcher());
                    i += 2;
                }
                "--regex" => {
                    let p = args
                        .get(i + 1)
                        .ok_or_else(|| anyhow!("--regex expects pattern"))?;
                    regex_pat = Some(Regex::new(p)?);
                    i += 2;
                }
                "--max-depth" => {
                    max_depth = Some(
                        args.get(i + 1)
                            .ok_or_else(|| anyhow!("--max-depth expects number"))?
                            .parse::<usize>()?,
                    );
                    i += 2;
                }
                "--type" => {
                    i += 2;
                }
                s if s.starts_with('-') => {
                    i += 1;
                }
                _ => {
                    root = Some(resolve_path(ctx.env.cwd(), &args[i]));
                    i += 1;
                }
            }
        }
        let root = root.unwrap_or_else(|| ctx.env.cwd().to_path_buf());
        let mut walker = WalkDir::new(&root);
        if let Some(d) = max_depth {
            walker = walker.max_depth(d);
        }

        let mut rows = Vec::new();
        for entry in walker.into_iter().filter_map(Result::ok) {
            let p = entry.path();
            if p == root {
                continue;
            }
            let rel = p
                .strip_prefix(&root)
                .unwrap_or(p)
                .to_string_lossy()
                .to_string();
            let matched_glob = glob_pat.as_ref().map(|m| m.is_match(&rel)).unwrap_or(true);
            let matched_regex = regex_pat.as_ref().map(|r| r.is_match(&rel)).unwrap_or(true);
            if matched_glob && matched_regex {
                rows.push(file_row(p, true)?);
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    WatchBuiltin,
    "watch",
    "watch <interval_sec> <command...>",
    "Run command repeatedly every interval seconds",
    &["watch 2 ls"],
    |args, _input, _ctx| {
        if args.len() < 2 {
            bail!("watch expects: watch <interval_sec> <command...>")
        }
        let interval = args[0].parse::<u64>().unwrap_or(2);
        let cmd = &args[1];
        let cmd_args = &args[2..];
        let mut output = String::new();
        for _ in 0..3 {
            let out = Command::new(cmd).args(cmd_args).output()?;
            output.push_str(&String::from_utf8_lossy(&out.stdout));
            std::thread::sleep(Duration::from_secs(interval));
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(output)))
    }
);

simple_builtin!(
    ChmodBuiltin,
    "chmod",
    "chmod <mode> <path>",
    "Change file mode (unix) or readonly flag (windows)",
    &["chmod 755 script.sh"],
    |args, _input, ctx| {
        if args.len() != 2 {
            bail!("chmod expects <mode> <path>")
        }
        let mode = &args[0];
        let path = resolve_path(ctx.env.cwd(), &args[1]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let value = u32::from_str_radix(mode, 8)
                .map_err(|_| anyhow!("mode must be octal, e.g. 755"))?;
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(value);
            fs::set_permissions(path, perms)?;
        }
        #[cfg(windows)]
        {
            let mut perms = fs::metadata(&path)?.permissions();
            let readonly = matches!(mode.as_str(), "444" | "400");
            perms.set_readonly(readonly);
            fs::set_permissions(path, perms)?;
        }
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    LnBuiltin,
    "ln",
    "ln -s <target> <link>",
    "Create symlink with cross-platform fallback",
    &["ln -s source.txt link.txt"],
    |args, _input, ctx| {
        if args.len() < 3 || args[0] != "-s" {
            bail!("ln usage: ln -s <target> <link>")
        }
        let target = resolve_path(ctx.env.cwd(), &args[1]);
        let link = resolve_path(ctx.env.cwd(), &args[2]);
        if link.exists() {
            if link.is_dir() {
                fs::remove_dir_all(&link)?;
            } else {
                fs::remove_file(&link)?;
            }
        }

        let symlink_result = create_symlink(&target, &link);
        if symlink_result.is_err() {
            if target.is_file() {
                if fs::hard_link(&target, &link).is_err() {
                    fs::copy(&target, &link)?;
                }
            } else if target.is_dir() {
                copy_dir_recursive(&target, &link)?;
            } else {
                return Err(symlink_result.err().unwrap().into());
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

pub(super) fn format_modified(st: Option<std::time::SystemTime>) -> String {
    st.map(|t| {
        let dt: DateTime<Local> = DateTime::from(t);
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    })
    .unwrap_or_else(|| "unknown".to_string())
}

fn file_row(path: &Path, long_view: bool) -> anyhow::Result<Record> {
    let md = fs::symlink_metadata(path)?;
    let mut row = Record::new();
    row.insert(
        "name".into(),
        Value::String(
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string(),
        ),
    );
    row.insert("path".into(), Value::String(path.display().to_string()));
    row.insert(
        "type".into(),
        Value::String(
            if md.is_dir() {
                "dir"
            } else if md.file_type().is_symlink() {
                "symlink"
            } else {
                "file"
            }
            .to_string(),
        ),
    );
    row.insert(
        "size".into(),
        Value::Filesize(FilesizeValue { bytes: md.len() }),
    );
    if long_view {
        row.insert("readonly".into(), Value::Bool(md.permissions().readonly()));
        row.insert("hidden".into(), Value::Bool(is_hidden(path)));
    }
    row.insert(
        "modified".into(),
        Value::String(format_modified(md.modified().ok())),
    );
    Ok(row)
}

fn is_hidden(path: &Path) -> bool {
    let name_hidden = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(md) = fs::metadata(path) {
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            return name_hidden || (md.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0;
        }
    }
    name_hidden
}

fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
}
