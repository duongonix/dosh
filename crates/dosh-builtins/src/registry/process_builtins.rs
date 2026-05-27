use super::*;
use crate::registry::{factory, simple_builtin};
use anyhow::{anyhow, bail};
use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use std::process::{Child, Command};
use std::sync::Mutex;
use sysinfo::System;

struct JobEntry {
    command: String,
    child: Child,
}

static JOBS: Lazy<Mutex<BTreeMap<u32, JobEntry>>> = Lazy::new(|| Mutex::new(BTreeMap::new()));

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(PsBuiltin),
        factory!(KillBuiltin),
        factory!(JobsBuiltin),
        factory!(FgBuiltin),
        factory!(BgBuiltin),
        factory!(SpawnBuiltin),
        factory!(WaitBuiltin),
    ]
}

simple_builtin!(
    PsBuiltin,
    "ps",
    "ps",
    "List processes as table",
    &["ps"],
    |_args, _input, _ctx| {
        let mut sys = System::new_all();
        sys.refresh_all();
        let mut rows = Vec::new();
        for (pid, proc_) in sys.processes() {
            let mut row = Record::new();
            row.insert("pid".into(), Value::Int(pid.as_u32() as i64));
            row.insert("name".into(), Value::String(proc_.name().to_string()));
            row.insert("cpu".into(), Value::Float(proc_.cpu_usage() as f64));
            row.insert(
                "memory".into(),
                Value::Filesize(dosh_value::FilesizeValue {
                    bytes: proc_.memory() * 1024,
                }),
            );
            rows.push(row);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    SpawnBuiltin,
    "spawn",
    "spawn <command...>",
    "Spawn command in background job table",
    &["spawn ping localhost"],
    |args, _input, _ctx| {
        if args.is_empty() {
            bail!("spawn expects command")
        }
        let mut cmd = Command::new(&args[0]);
        cmd.args(&args[1..]);
        let child = cmd.spawn()?;
        let pid = child.id();
        let entry = JobEntry {
            command: args.join(" "),
            child,
        };
        JOBS.lock()
            .map_err(|_| anyhow!("job lock poisoned"))?
            .insert(pid, entry);
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "spawned pid={pid}"
        ))))
    }
);

simple_builtin!(
    JobsBuiltin,
    "jobs",
    "jobs",
    "List managed background jobs",
    &["jobs"],
    |_args, _input, _ctx| {
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        let mut done = Vec::new();
        let mut rows = Vec::new();
        for (pid, job) in jobs.iter_mut() {
            let state = match job.child.try_wait()? {
                Some(status) => {
                    done.push(*pid);
                    format!("done({})", status.code().unwrap_or(0))
                }
                None => "running".to_string(),
            };
            let mut row = Record::new();
            row.insert("pid".into(), Value::Int(*pid as i64));
            row.insert("state".into(), Value::String(state));
            row.insert("command".into(), Value::String(job.command.clone()));
            rows.push(row);
        }
        for pid in done {
            jobs.remove(&pid);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    WaitBuiltin,
    "wait",
    "wait [pid]",
    "Wait for a job or all jobs",
    &["wait", "wait 1234"],
    |args, _input, _ctx| {
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        if let Some(pid_s) = args.first() {
            let pid = pid_s.parse::<u32>().map_err(|_| anyhow!("invalid pid"))?;
            let mut job = jobs.remove(&pid).ok_or_else(|| anyhow!("pid not found"))?;
            let status = job.child.wait()?;
            return Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
                "pid={pid} exit={}",
                status.code().unwrap_or(1)
            ))));
        }
        let pids = jobs.keys().cloned().collect::<Vec<_>>();
        let mut out = Vec::new();
        for pid in pids {
            if let Some(mut job) = jobs.remove(&pid) {
                let status = job.child.wait()?;
                out.push(format!("pid={pid} exit={}", status.code().unwrap_or(1)));
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(out.join("\n"))))
    }
);

simple_builtin!(
    KillBuiltin,
    "kill",
    "kill <pid>",
    "Kill process by pid",
    &["kill 1234"],
    |args, _input, _ctx| {
        let pid = args
            .first()
            .ok_or_else(|| anyhow!("kill expects pid"))?
            .parse::<u32>()
            .map_err(|_| anyhow!("invalid pid"))?;
        if let Some(mut job) = JOBS
            .lock()
            .map_err(|_| anyhow!("job lock poisoned"))?
            .remove(&pid)
        {
            let _ = job.child.kill();
            return Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
                "killed pid={pid}"
            ))));
        }
        #[cfg(target_os = "windows")]
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()?;
        #[cfg(not(target_os = "windows"))]
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "kill requested for pid={pid}"
        ))))
    }
);

simple_builtin!(
    FgBuiltin,
    "fg",
    "fg <pid>",
    "Bring managed job to foreground (wait)",
    &["fg 1234"],
    |args, _input, _ctx| {
        let pid = args
            .first()
            .ok_or_else(|| anyhow!("fg expects pid"))?
            .parse::<u32>()
            .map_err(|_| anyhow!("invalid pid"))?;
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        let mut job = jobs.remove(&pid).ok_or_else(|| anyhow!("pid not found"))?;
        let status = job.child.wait()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "fg pid={pid} exit={}",
            status.code().unwrap_or(1)
        ))))
    }
);

simple_builtin!(
    BgBuiltin,
    "bg",
    "bg <pid>",
    "Keep a managed job running in background",
    &["bg 1234"],
    |args, _input, _ctx| {
        let pid = args
            .first()
            .ok_or_else(|| anyhow!("bg expects pid"))?
            .parse::<u32>()
            .map_err(|_| anyhow!("invalid pid"))?;
        let jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        if jobs.contains_key(&pid) {
            Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
                "job {pid} is running in background"
            ))))
        } else {
            bail!("pid not found")
        }
    }
);
