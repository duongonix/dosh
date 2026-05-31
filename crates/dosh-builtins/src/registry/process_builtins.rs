use super::*;
use crate::registry::{factory, simple_builtin};
use anyhow::{anyhow, bail};
use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::System;

struct JobEntry {
    id: u32,
    pid: u32,
    command: String,
    child: Child,
    started_unix_ms: u128,
    detached: bool,
}

static JOBS: Lazy<Mutex<BTreeMap<u32, JobEntry>>> = Lazy::new(|| Mutex::new(BTreeMap::new()));
static NEXT_JOB_ID: AtomicU32 = AtomicU32::new(1);

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(PsBuiltin),
        factory!(KillBuiltin),
        factory!(JobsBuiltin),
        factory!(FgBuiltin),
        factory!(BgBuiltin),
        factory!(SpawnBuiltin),
        factory!(WaitBuiltin),
        factory!(DetachBuiltin),
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
    "spawn [--detached] <command...>",
    "Spawn command in background job table",
    &["spawn ping localhost", "spawn --detached cargo build"],
    |args, _input, _ctx| {
        if args.is_empty() {
            bail!("spawn expects command")
        }
        let mut detached = false;
        let mut idx = 0usize;
        if args.first().is_some_and(|a| a == "--detached") {
            detached = true;
            idx = 1;
        }
        if idx >= args.len() {
            bail!("spawn expects command")
        }
        let mut cmd = Command::new(&args[idx]);
        cmd.args(&args[idx + 1..]);
        let child = cmd.spawn()?;
        let pid = child.id();
        let id = NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed);
        let entry = JobEntry {
            id,
            pid,
            command: args[idx..].join(" "),
            child,
            started_unix_ms: now_unix_ms(),
            detached,
        };
        JOBS.lock()
            .map_err(|_| anyhow!("job lock poisoned"))?
            .insert(id, entry);
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "spawned job={id} pid={pid}"
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
        for (id, job) in jobs.iter_mut() {
            let state = match job.child.try_wait()? {
                Some(status) => {
                    done.push(*id);
                    format!("done({})", status.code().unwrap_or(0))
                }
                None => "running".to_string(),
            };
            let mut row = Record::new();
            row.insert("job".into(), Value::Int(job.id as i64));
            row.insert("pid".into(), Value::Int(job.pid as i64));
            row.insert("state".into(), Value::String(state));
            row.insert("detached".into(), Value::Bool(job.detached));
            row.insert("started_ms".into(), Value::Int(job.started_unix_ms as i64));
            row.insert("command".into(), Value::String(job.command.clone()));
            rows.push(row);
        }
        for id in done {
            jobs.remove(&id);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    WaitBuiltin,
    "wait",
    "wait [job|pid|%job]",
    "Wait for a job or all jobs",
    &["wait", "wait 1", "wait %1", "wait 1234"],
    |args, _input, _ctx| {
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        if let Some(sel) = args.first() {
            let id = resolve_job_selector(&jobs, sel)?;
            let mut job = jobs.remove(&id).ok_or_else(|| anyhow!("job not found"))?;
            let status = job.child.wait()?;
            return Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
                "job={} pid={} exit={}",
                job.id,
                job.pid,
                status.code().unwrap_or(1)
            ))));
        }
        let ids = jobs.keys().cloned().collect::<Vec<_>>();
        let mut out = Vec::new();
        for id in ids {
            if let Some(mut job) = jobs.remove(&id) {
                let status = job.child.wait()?;
                out.push(format!(
                    "job={} pid={} exit={}",
                    job.id,
                    job.pid,
                    status.code().unwrap_or(1)
                ));
            }
        }
        Ok(BuiltinOutcome::ok(PipelineData::Text(out.join("\n"))))
    }
);

simple_builtin!(
    KillBuiltin,
    "kill",
    "kill <job|pid|%job>",
    "Kill process by job id or pid",
    &["kill 1", "kill %1", "kill 1234"],
    |args, _input, _ctx| {
        let selector = args
            .first()
            .ok_or_else(|| anyhow!("kill expects job or pid"))?;
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        if let Ok(id) = resolve_job_selector(&jobs, selector)
            && let Some(mut job) = jobs.remove(&id)
        {
            let _ = job.child.kill();
            return Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
                "killed job={} pid={}",
                job.id, job.pid
            ))));
        }
        let pid = selector
            .trim_start_matches('%')
            .parse::<u32>()
            .map_err(|_| anyhow!("invalid job/pid"))?;
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
    "fg <job|pid|%job>",
    "Bring managed job to foreground (wait)",
    &["fg 1", "fg %1", "fg 1234"],
    |args, _input, _ctx| {
        let selector = args
            .first()
            .ok_or_else(|| anyhow!("fg expects job or pid"))?;
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        let id = resolve_job_selector(&jobs, selector)?;
        let mut job = jobs.remove(&id).ok_or_else(|| anyhow!("job not found"))?;
        let status = job.child.wait()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "fg job={} pid={} exit={}",
            job.id,
            job.pid,
            status.code().unwrap_or(1)
        ))))
    }
);

simple_builtin!(
    BgBuiltin,
    "bg",
    "bg <job|pid|%job>",
    "Keep a managed job running in background",
    &["bg 1", "bg %1", "bg 1234"],
    |args, _input, _ctx| {
        let selector = args
            .first()
            .ok_or_else(|| anyhow!("bg expects job or pid"))?;
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        let id = resolve_job_selector(&jobs, selector)?;
        if let Some(job) = jobs.get_mut(&id) {
            job.detached = true;
            Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
                "job {} (pid={}) is running in background",
                job.id, job.pid
            ))))
        } else {
            bail!("job not found")
        }
    }
);

simple_builtin!(
    DetachBuiltin,
    "detach",
    "detach <job|pid|%job>",
    "Mark a managed job detached",
    &["detach 1", "detach %1"],
    |args, _input, _ctx| {
        let selector = args
            .first()
            .ok_or_else(|| anyhow!("detach expects job or pid"))?;
        let mut jobs = JOBS.lock().map_err(|_| anyhow!("job lock poisoned"))?;
        let id = resolve_job_selector(&jobs, selector)?;
        let Some(job) = jobs.get_mut(&id) else {
            bail!("job not found")
        };
        job.detached = true;
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "detached job {} (pid={})",
            job.id, job.pid
        ))))
    }
);

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn resolve_job_selector(jobs: &BTreeMap<u32, JobEntry>, selector: &str) -> anyhow::Result<u32> {
    let raw = selector.trim();
    let id_or_pid = raw
        .trim_start_matches('%')
        .parse::<u32>()
        .map_err(|_| anyhow!("invalid job/pid selector"))?;

    if raw.starts_with('%') && jobs.contains_key(&id_or_pid) {
        return Ok(id_or_pid);
    }
    if jobs.contains_key(&id_or_pid) {
        return Ok(id_or_pid);
    }
    if let Some((id, _)) = jobs.iter().find(|(_, j)| j.pid == id_or_pid) {
        return Ok(*id);
    }
    bail!("job/pid not found: {selector}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_job_selector_by_id_and_pid() {
        let mut map = BTreeMap::new();
        let child = if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", "exit 0"])
                .spawn()
                .expect("spawn")
        } else {
            Command::new("sh")
                .args(["-c", "true"])
                .spawn()
                .expect("spawn")
        };
        map.insert(
            7,
            JobEntry {
                id: 7,
                pid: child.id(),
                command: "noop".into(),
                child,
                started_unix_ms: 0,
                detached: false,
            },
        );
        assert_eq!(resolve_job_selector(&map, "7").unwrap(), 7);
        assert_eq!(resolve_job_selector(&map, "%7").unwrap(), 7);
        let pid = map.get(&7).unwrap().pid.to_string();
        assert_eq!(resolve_job_selector(&map, &pid).unwrap(), 7);
    }
}
