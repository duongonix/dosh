use super::*;
use crate::registry::{factory, simple_builtin};
use std::process::Command;
use sysinfo::System;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(SysBuiltin),
        factory!(CpuBuiltin),
        factory!(MemBuiltin),
        factory!(DiskBuiltin),
        factory!(BatteryBuiltin),
        factory!(OsBuiltin),
        factory!(HostnameBuiltin),
        factory!(WhoamiBuiltin),
    ]
}

simple_builtin!(
    SysBuiltin,
    "sys",
    "sys",
    "Summary system info",
    &["sys"],
    |_args, _input, _ctx| {
        let mut sys = System::new_all();
        sys.refresh_all();
        let mut rec = Record::new();
        rec.insert("os".into(), Value::String(std::env::consts::OS.to_string()));
        rec.insert(
            "arch".into(),
            Value::String(std::env::consts::ARCH.to_string()),
        );
        rec.insert("hostname".into(), Value::String(hostname_text()));
        rec.insert("whoami".into(), Value::String(whoami_text()));
        rec.insert("cpu_count".into(), Value::Int(sys.cpus().len() as i64));
        rec.insert(
            "memory_total".into(),
            Value::Filesize(dosh_value::FilesizeValue {
                bytes: sys.total_memory() * 1024,
            }),
        );
        rec.insert(
            "memory_used".into(),
            Value::Filesize(dosh_value::FilesizeValue {
                bytes: sys.used_memory() * 1024,
            }),
        );
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    CpuBuiltin,
    "cpu",
    "cpu",
    "CPU information",
    &["cpu"],
    |_args, _input, _ctx| {
        let mut sys = System::new_all();
        sys.refresh_cpu();
        let mut rows = Vec::new();
        for cpu in sys.cpus() {
            let mut row = Record::new();
            row.insert("name".into(), Value::String(cpu.name().to_string()));
            row.insert("usage".into(), Value::Float(cpu.cpu_usage() as f64));
            row.insert("vendor".into(), Value::String(cpu.vendor_id().to_string()));
            row.insert("brand".into(), Value::String(cpu.brand().to_string()));
            rows.push(row);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    MemBuiltin,
    "mem",
    "mem",
    "Memory information",
    &["mem"],
    |_args, _input, _ctx| {
        let mut sys = System::new_all();
        sys.refresh_memory();
        let mut rec = Record::new();
        rec.insert(
            "total".into(),
            Value::Filesize(dosh_value::FilesizeValue {
                bytes: sys.total_memory() * 1024,
            }),
        );
        rec.insert(
            "used".into(),
            Value::Filesize(dosh_value::FilesizeValue {
                bytes: sys.used_memory() * 1024,
            }),
        );
        rec.insert(
            "free".into(),
            Value::Filesize(dosh_value::FilesizeValue {
                bytes: sys.free_memory() * 1024,
            }),
        );
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    DiskBuiltin,
    "disk",
    "disk",
    "Disk information",
    &["disk"],
    |_args, _input, _ctx| {
        let mut disks = sysinfo::Disks::new_with_refreshed_list();
        disks.refresh();
        let mut rows = Vec::new();
        for d in disks.list() {
            let mut row = Record::new();
            row.insert(
                "name".into(),
                Value::String(d.name().to_string_lossy().to_string()),
            );
            row.insert(
                "mount".into(),
                Value::String(d.mount_point().to_string_lossy().to_string()),
            );
            row.insert(
                "total".into(),
                Value::Filesize(dosh_value::FilesizeValue {
                    bytes: d.total_space(),
                }),
            );
            row.insert(
                "free".into(),
                Value::Filesize(dosh_value::FilesizeValue {
                    bytes: d.available_space(),
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
    BatteryBuiltin,
    "battery",
    "battery",
    "Battery information",
    &["battery"],
    |_args, _input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            command_out(
                "wmic",
                &[
                    "path",
                    "Win32_Battery",
                    "get",
                    "EstimatedChargeRemaining,BatteryStatus",
                ],
            )
            .or_else(|_| {
                command_out(
                    "sh",
                    &[
                        "-lc",
                        "upower -i $(upower -e | grep BAT | head -n1) || pmset -g batt",
                    ],
                )
            })
            .unwrap_or_else(|_| "battery info unavailable".to_string()),
        )))
    }
);

simple_builtin!(
    OsBuiltin,
    "os",
    "os",
    "Operating system name",
    &["os"],
    |_args, _input, _ctx| {
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            std::env::consts::OS.to_string(),
        )))
    }
);

simple_builtin!(
    HostnameBuiltin,
    "hostname",
    "hostname",
    "Machine hostname",
    &["hostname"],
    |_args, _input, _ctx| { Ok(BuiltinOutcome::ok(PipelineData::Text(hostname_text()))) }
);

simple_builtin!(
    WhoamiBuiltin,
    "whoami",
    "whoami",
    "Current user",
    &["whoami"],
    |_args, _input, _ctx| { Ok(BuiltinOutcome::ok(PipelineData::Text(whoami_text()))) }
);

fn hostname_text() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string())
}

fn whoami_text() -> String {
    command_out("whoami", &[])
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("USERNAME").ok())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "unknown".to_string())
}

fn command_out(cmd: &str, args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new(cmd).args(args).output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
