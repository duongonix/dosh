use super::*;
use crate::registry::{factory, simple_builtin};
use anyhow::{anyhow, bail};
use chrono::{DateTime, Local, Utc};
use std::time::{Duration, Instant};

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(DateBuiltin),
        factory!(NowBuiltin),
        factory!(FormatDateBuiltin),
        factory!(ParseDateBuiltin),
        factory!(SleepBuiltin),
        factory!(TimerBuiltin),
    ]
}

simple_builtin!(
    DateBuiltin,
    "date",
    "date",
    "Show current local date/time",
    &["date"],
    |_args, _input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            Local::now().to_rfc3339(),
        )))
    }
);

simple_builtin!(
    NowBuiltin,
    "now",
    "now",
    "Show current unix timestamp seconds",
    &["now"],
    |_args, _input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Int(
            Utc::now().timestamp(),
        ))))
    }
);

simple_builtin!(
    FormatDateBuiltin,
    "format-date",
    "format-date <input> <fmt>",
    "Format RFC3339 or unix-seconds date",
    &["now | format-date %Y-%m-%d"],
    |args, input, _ctx| {
        if args.is_empty() {
            bail!("format-date expects format string")
        }
        let fmt = if args.len() == 1 {
            args[0].clone()
        } else {
            args[1..].join(" ")
        };
        let source = if args.len() > 1 {
            args[0].clone()
        } else {
            input.into_text()
        };
        let dt = if let Ok(ts) = source.trim().parse::<i64>() {
            DateTime::<Utc>::from_timestamp(ts, 0)
                .ok_or_else(|| anyhow!("invalid unix timestamp"))?
                .with_timezone(&Local)
        } else {
            DateTime::parse_from_rfc3339(source.trim())?.with_timezone(&Local)
        };
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            dt.format(&fmt).to_string(),
        )))
    }
);

simple_builtin!(
    ParseDateBuiltin,
    "parse-date",
    "parse-date <text> [fmt]",
    "Parse date text to unix timestamp",
    &["parse-date '2026-01-01T00:00:00+00:00'"],
    |args, input, _ctx| {
        let text = if args.is_empty() {
            input.into_text()
        } else {
            args[0].clone()
        };
        if text.trim().is_empty() {
            bail!("parse-date expects date text")
        }
        let ts = if args.len() > 1 {
            let dt = chrono::NaiveDateTime::parse_from_str(&text, &args[1])?;
            dt.and_utc().timestamp()
        } else {
            DateTime::parse_from_rfc3339(text.trim())?.timestamp()
        };
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Int(ts))))
    }
);

simple_builtin!(
    SleepBuiltin,
    "sleep",
    "sleep <seconds>",
    "Sleep for seconds",
    &["sleep 1"],
    |args, _input, _ctx| {
        let secs = args
            .first()
            .and_then(|v| v.parse::<f64>().ok())
            .ok_or_else(|| anyhow!("sleep expects seconds"))?;
        std::thread::sleep(Duration::from_secs_f64(secs.max(0.0)));
        Ok(BuiltinOutcome::ok(PipelineData::Empty))
    }
);

simple_builtin!(
    TimerBuiltin,
    "timer",
    "timer <command...>",
    "Measure execution time of a command",
    &["timer ls"],
    |args, _input, _ctx| {
        if args.is_empty() {
            bail!("timer expects command")
        }
        let mut cmd = std::process::Command::new(&args[0]);
        cmd.args(&args[1..]);
        let start = Instant::now();
        let output = cmd.output()?;
        let elapsed = start.elapsed();
        let mut text = String::new();
        text.push_str(&String::from_utf8_lossy(&output.stdout));
        text.push_str(&format!("\n[elapsed: {:.3?}]", elapsed));
        Ok(BuiltinOutcome::ok(PipelineData::Text(text)))
    }
);
