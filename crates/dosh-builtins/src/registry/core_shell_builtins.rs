use super::*;
use crate::registry::{factory, simple_builtin};
use anyhow::{anyhow, bail};
use dosh_config::DoshPaths;
use sha2::{Digest, Sha256};
use std::fs;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(CdBuiltin),
        factory!(PwdBuiltin),
        factory!(EchoBuiltin),
        factory!(PrintBuiltin),
        factory!(AssertBuiltin),
        factory!(ExitBuiltin),
        factory!(ClearBuiltin),
        factory!(HistoryBuiltin),
        factory!(AliasBuiltin),
        factory!(UnaliasBuiltin),
        factory!(SourceBuiltin),
        factory!(HelpBuiltin),
        factory!(RunExternalBuiltin),
        factory!(ConfigBuiltin),
        factory!(ConfirmBuiltin),
        factory!(HashBuiltin),
        factory!(PermissionsBuiltin),
        factory!(CaptureBuiltin),
        factory!(CompleteBuiltin),
        factory!(StdoutBuiltin),
        factory!(StderrBuiltin),
        factory!(ExitCodeBuiltin),
        factory!(DeditBuiltin),
    ]
}

simple_builtin!(
    CdBuiltin,
    "cd",
    "cd [path]",
    "Change current directory",
    &["cd src"],
    |args, _input, ctx| {
        let target = args
            .first()
            .cloned()
            .or_else(|| std::env::var("HOME").ok())
            .or_else(|| std::env::var("USERPROFILE").ok())
            .ok_or_else(|| anyhow!("home directory is not available"))?;
        ctx.env.change_dir(&target)?;
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    PwdBuiltin,
    "pwd",
    "pwd",
    "Print current directory",
    &["pwd"],
    |_args, _input, ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            ctx.env.cwd().display().to_string(),
        )))
    }
);

simple_builtin!(
    EchoBuiltin,
    "echo",
    "echo <text...>",
    "Echo text",
    &["echo hello"],
    |args, _input, _ctx| { Ok(BuiltinOutcome::ok(PipelineData::Text(args.join(" ")))) }
);

simple_builtin!(
    PrintBuiltin,
    "print",
    "print [value...]",
    "Print values or pipeline input",
    &["print $name", "ls | first | print"],
    |args, input, _ctx| {
        let rendered = if !args.is_empty() {
            args.join(" ")
        } else {
            input.into_text()
        };
        println!("{rendered}");
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    AssertBuiltin,
    "assert",
    "assert <eq|ne|contains|true|false> <args...>",
    "Script assertions for dosh test scripts",
    &["assert eq 1 1", "assert true $flag"],
    |args, _input, _ctx| {
        let kind = args
            .first()
            .ok_or_else(|| anyhow!("assert expects a mode"))?
            .as_str();
        match kind {
            "eq" => {
                if args.len() != 3 {
                    bail!("assert eq expects 2 arguments");
                }
                if args[1] != args[2] {
                    bail!("assert eq failed: left=`{}` right=`{}`", args[1], args[2]);
                }
            }
            "ne" => {
                if args.len() != 3 {
                    bail!("assert ne expects 2 arguments");
                }
                if args[1] == args[2] {
                    bail!("assert ne failed: both are `{}`", args[1]);
                }
            }
            "contains" => {
                if args.len() != 3 {
                    bail!("assert contains expects 2 arguments");
                }
                if !args[1].contains(&args[2]) {
                    bail!(
                        "assert contains failed: `{}` does not contain `{}`",
                        args[1],
                        args[2]
                    );
                }
            }
            "true" => {
                if args.len() != 2 {
                    bail!("assert true expects 1 argument");
                }
                if !args[1].eq_ignore_ascii_case("true") {
                    bail!("assert true failed: got `{}`", args[1]);
                }
            }
            "false" => {
                if args.len() != 2 {
                    bail!("assert false expects 1 argument");
                }
                if !args[1].eq_ignore_ascii_case("false") {
                    bail!("assert false failed: got `{}`", args[1]);
                }
            }
            _ => bail!("unknown assert mode `{kind}`"),
        }
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    ExitBuiltin,
    "exit",
    "exit",
    "Exit shell",
    &["exit"],
    |_args, _input, _ctx| {
        Ok(BuiltinOutcome {
            exit_code: 0,
            should_exit: true,
            output: PipelineData::Empty,
        })
    }
);

simple_builtin!(
    ClearBuiltin,
    "clear",
    "clear",
    "Clear terminal screen",
    &["clear"],
    |_args, _input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            "\x1B[2J\x1B[H".to_string(),
        )))
    }
);

simple_builtin!(
    HistoryBuiltin,
    "history",
    "history [limit]",
    "Show command history",
    &["history", "history 50"],
    |args, _input, _ctx| {
        let limit = args
            .first()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100);
        let path = history_file_path();
        let content = fs::read_to_string(path).unwrap_or_default();
        let mut lines = content
            .lines()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if lines.len() > limit {
            lines = lines[lines.len() - limit..].to_vec();
        }
        let out = lines
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>4}  {}", i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(BuiltinOutcome::ok(PipelineData::Text(out)))
    }
);

simple_builtin!(
    AliasBuiltin,
    "alias",
    "alias [name [value...]]",
    "Define or list aliases",
    &["alias", "alias ll ls -la"],
    |args, _input, _ctx| {
        let mut aliases = ALIASES.lock().map_err(|_| anyhow!("alias lock poisoned"))?;
        if args.is_empty() {
            let text = aliases
                .iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(BuiltinOutcome::ok(PipelineData::Text(text)));
        }
        if args.len() == 1 {
            let key = &args[0];
            let text = aliases
                .get(key)
                .map(|v| format!("{key} = {v}"))
                .unwrap_or_else(|| format!("alias `{key}` not found"));
            return Ok(BuiltinOutcome::ok(PipelineData::Text(text)));
        }
        let key = args[0].clone();
        let value = args[1..].join(" ");
        aliases.insert(key.clone(), value.clone());
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "set alias {key} = {value}"
        ))))
    }
);

simple_builtin!(
    UnaliasBuiltin,
    "unalias",
    "unalias <name>",
    "Remove alias",
    &["unalias ll"],
    |args, _input, _ctx| {
        let key = args
            .first()
            .ok_or_else(|| anyhow!("unalias expects name"))?;
        let mut aliases = ALIASES.lock().map_err(|_| anyhow!("alias lock poisoned"))?;
        let text = if aliases.remove(key).is_some() {
            format!("removed alias `{key}`")
        } else {
            format!("alias `{key}` not found")
        };
        Ok(BuiltinOutcome::ok(PipelineData::Text(text)))
    }
);

simple_builtin!(
    SourceBuiltin,
    "source",
    "source <file>",
    "Load and execute commands from a script file",
    &["source ./scripts/init.dosh"],
    |_args, _input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            "source is handled by runtime".to_string(),
        )))
    }
);

simple_builtin!(
    RunExternalBuiltin,
    "run-external",
    "run-external <command> [args...]",
    "Run external command explicitly",
    &["run-external git status"],
    |args, _input, _ctx| {
        let cmd = args
            .first()
            .ok_or_else(|| anyhow!("run-external expects command"))?;
        let out = std::process::Command::new(cmd).args(&args[1..]).output()?;
        let mut text = String::new();
        text.push_str(&String::from_utf8_lossy(&out.stdout));
        if !out.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&String::from_utf8_lossy(&out.stderr));
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(text)))
    }
);

simple_builtin!(
    ConfigBuiltin,
    "config",
    "config <path|show|reload>",
    "Config operations",
    &["config path", "config show", "config reload"],
    |args, _input, _ctx| {
        let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
        let paths = DoshPaths::detect()?;
        match sub {
            "path" => {
                let mut rec = Record::new();
                rec.insert(
                    "config.toml".into(),
                    Value::String(paths.config_file().display().to_string()),
                );
                rec.insert(
                    "theme.toml".into(),
                    Value::String(paths.theme_file().display().to_string()),
                );
                rec.insert(
                    "startup.dosh".into(),
                    Value::String(paths.startup_file().display().to_string()),
                );
                rec.insert(
                    "aliases.dosh".into(),
                    Value::String(paths.aliases_file().display().to_string()),
                );
                rec.insert(
                    "plugins.toml".into(),
                    Value::String(paths.plugins_file().display().to_string()),
                );
                rec.insert(
                    "commands_dir".into(),
                    Value::String(paths.commands_dir().display().to_string()),
                );
                rec.insert(
                    "completions_dir".into(),
                    Value::String(paths.completions_dir().display().to_string()),
                );
                rec.insert(
                    "modules_dir".into(),
                    Value::String(paths.modules_dir().display().to_string()),
                );
                rec.insert(
                    "plugins_dir".into(),
                    Value::String(paths.plugins_dir().display().to_string()),
                );
                rec.insert(
                    "history.db".into(),
                    Value::String(paths.history_db_file().display().to_string()),
                );
                Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
            }
            "show" => {
                let cfg = dosh_config::load_default_config()?;
                Ok(BuiltinOutcome::ok(PipelineData::Text(
                    toml::to_string_pretty(&cfg)?,
                )))
            }
            "reload" => {
                let _ = dosh_config::load_default_config()?;
                Ok(BuiltinOutcome::ok(PipelineData::Text(
                    "config reloaded".to_string(),
                )))
            }
            _ => bail!("config expects subcommand path|show|reload"),
        }
    }
);

simple_builtin!(
    ConfirmBuiltin,
    "confirm",
    "confirm [message]",
    "Ask for explicit yes/no confirmation",
    &["confirm Delete files?"],
    |args, _input, _ctx| {
        use std::io::{self, Write};
        let msg = if args.is_empty() {
            "Confirm?".to_string()
        } else {
            args.join(" ")
        };
        print!("{msg} [y/N]: ");
        io::stdout().flush()?;
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
        let ok = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Bool(ok))))
    }
);

simple_builtin!(
    HashBuiltin,
    "hash",
    "hash <text...>",
    "SHA-256 hash of input text",
    &["echo hello | hash", "hash hello world"],
    |args, input, _ctx| {
        let text = if args.is_empty() {
            input.into_text()
        } else {
            args.join(" ")
        };
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let digest = format!("{:x}", hasher.finalize());
        Ok(BuiltinOutcome::ok(PipelineData::Text(digest)))
    }
);

simple_builtin!(
    PermissionsBuiltin,
    "permissions",
    "permissions",
    "Show permission mode foundation",
    &["permissions"],
    |_args, _input, _ctx| {
        let mut rec = Record::new();
        rec.insert("safe_mode".into(), Value::Bool(false));
        rec.insert("plugins_enabled".into(), Value::Bool(true));
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    CaptureBuiltin,
    "capture",
    "capture <command...>",
    "Run external command and return structured stdout/stderr/exit_code/duration_ms",
    &["capture cargo test"],
    |args, _input, _ctx| {
        if args.is_empty() {
            bail!("capture expects command")
        }
        let started = std::time::Instant::now();
        let out = std::process::Command::new(&args[0])
            .args(&args[1..])
            .output()?;
        let mut rec = Record::new();
        rec.insert(
            "stdout".into(),
            Value::String(String::from_utf8_lossy(&out.stdout).to_string()),
        );
        rec.insert(
            "stderr".into(),
            Value::String(String::from_utf8_lossy(&out.stderr).to_string()),
        );
        rec.insert(
            "exit_code".into(),
            Value::Int(out.status.code().unwrap_or(1) as i64),
        );
        rec.insert(
            "duration_ms".into(),
            Value::Int(started.elapsed().as_millis() as i64),
        );
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    CompleteBuiltin,
    "complete",
    "complete [command...]",
    "Structured complete output. If input is text, wrap it as stdout.",
    &["cargo test | complete", "complete cargo test"],
    |args, input, ctx| {
        if !args.is_empty() {
            return CaptureBuiltin.run(args, PipelineData::Empty, ctx);
        }
        let mut rec = Record::new();
        rec.insert("stdout".into(), Value::String(input.into_text()));
        rec.insert("stderr".into(), Value::String(String::new()));
        rec.insert("exit_code".into(), Value::Int(0));
        rec.insert("duration_ms".into(), Value::Int(0));
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    StdoutBuiltin,
    "stdout",
    "stdout",
    "Extract stdout from complete/capture record",
    &["capture cargo test | stdout"],
    |_args, input, _ctx| {
        let v = crate::helpers::pipeline_to_value(input)?;
        let out = if let Value::Record(r) = v {
            r.get("stdout")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new()))
        } else {
            Value::String(String::new())
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    StderrBuiltin,
    "stderr",
    "stderr",
    "Extract stderr from complete/capture record",
    &["capture cargo test | stderr"],
    |_args, input, _ctx| {
        let v = crate::helpers::pipeline_to_value(input)?;
        let out = if let Value::Record(r) = v {
            r.get("stderr")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new()))
        } else {
            Value::String(String::new())
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    ExitCodeBuiltin,
    "exit-code",
    "exit-code",
    "Extract exit_code from complete/capture record",
    &["capture cargo test | exit-code"],
    |_args, input, _ctx| {
        let v = crate::helpers::pipeline_to_value(input)?;
        let out = if let Value::Record(r) = v {
            r.get("exit_code").cloned().unwrap_or(Value::Int(0))
        } else {
            Value::Int(0)
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    DeditBuiltin,
    "dedit",
    "dedit <path>",
    "Open modern terminal editor (syntax highlight, diagnostics, autocomplete, line numbers)",
    &["dedit src/main.rs"],
    |args, _input, ctx| {
        let path = args
            .first()
            .ok_or_else(|| anyhow!("dedit expects path to file"))?;
        let resolved = crate::helpers::resolve_path(ctx.env.cwd(), path);
        dosh_dedit::run(&resolved)?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "dedit closed: {}",
            resolved.display()
        ))))
    }
);

simple_builtin!(
    HelpBuiltin,
    "help",
    "help [command]",
    "Show builtin help",
    &["help", "help ls"],
    |args, _input, _ctx| {
        let registry = BuiltinRegistry::new();
        let mut entries = registry.metadata(args.first().map(|s| s.as_str()));
        entries.sort_by(|a, b| a.name.cmp(b.name));
        if entries.is_empty() {
            bail!("no builtin help found");
        }
        let text = entries
            .into_iter()
            .map(|m| {
                format!(
                    "┌ {}\n│ usage    : {}\n│ summary  : {}\n│ examples : {}\n└",
                    m.name,
                    m.usage,
                    m.description,
                    m.examples.join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(BuiltinOutcome::ok(PipelineData::Text(text)))
    }
);
