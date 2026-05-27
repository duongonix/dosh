use super::*;
use crate::helpers::pipeline_to_value;
use crate::registry::{factory, simple_builtin};
use anyhow::{anyhow, bail};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

pub(super) fn factories() -> Vec<BuiltinFactory> {
    vec![
        factory!(DurlBuiltin),
        factory!(HttpBuiltin),
        factory!(FetchBuiltin),
        factory!(CurlBuiltin),
        factory!(PingBuiltin),
        factory!(DnsBuiltin),
        factory!(PortBuiltin),
        factory!(ServeBuiltin),
    ]
}

simple_builtin!(
    DurlBuiltin,
    "durl",
    "durl [method] <url> [flags]",
    "Structured HTTP client (pipeline-native)",
    &[
        "durl get https://api.example.com/users | where age > 18 | select name email",
        "{ \"name\": \"dosh\" } | durl post https://api.example.com/projects --full"
    ],
    |args, input, _ctx| {
        let input_value = match input {
            PipelineData::Empty => None,
            other => Some(pipeline_to_value(other)?),
        };
        let out = dosh_durl::run(dosh_durl::DurlRunOptions {
            args: args.to_vec(),
            input: input_value,
        })?;
        Ok(BuiltinOutcome::ok(PipelineData::Value(out)))
    }
);

simple_builtin!(
    HttpBuiltin,
    "http",
    "http <url>",
    "HTTP GET via curl command",
    &["http https://example.com"],
    |args, _input, _ctx| {
        let url = args.first().ok_or_else(|| anyhow!("http expects url"))?;
        let out = Command::new("curl").args(["-sSL", url]).output()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            String::from_utf8_lossy(&out.stdout).to_string(),
        )))
    }
);

simple_builtin!(
    FetchBuiltin,
    "fetch",
    "fetch <url>",
    "Alias of http",
    &["fetch https://example.com"],
    |args, _input, _ctx| {
        let url = args.first().ok_or_else(|| anyhow!("fetch expects url"))?;
        let out = Command::new("curl").args(["-sSL", url]).output()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            String::from_utf8_lossy(&out.stdout).to_string(),
        )))
    }
);

simple_builtin!(
    CurlBuiltin,
    "curl",
    "curl <args...>",
    "Pass-through to system curl",
    &["curl -I https://example.com"],
    |args, _input, _ctx| {
        if args.is_empty() {
            bail!("curl expects args")
        }
        let out = Command::new("curl").args(args).output()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            String::from_utf8_lossy(&out.stdout).to_string(),
        )))
    }
);

simple_builtin!(
    PingBuiltin,
    "ping",
    "ping <host>",
    "Ping host",
    &["ping example.com"],
    |args, _input, _ctx| {
        let host = args.first().ok_or_else(|| anyhow!("ping expects host"))?;
        #[cfg(target_os = "windows")]
        let out = Command::new("ping").args(["-n", "4", host]).output()?;
        #[cfg(not(target_os = "windows"))]
        let out = Command::new("ping").args(["-c", "4", host]).output()?;
        Ok(BuiltinOutcome::ok(PipelineData::Text(
            String::from_utf8_lossy(&out.stdout).to_string(),
        )))
    }
);

simple_builtin!(
    DnsBuiltin,
    "dns",
    "dns <host>",
    "Resolve DNS host",
    &["dns example.com"],
    |args, _input, _ctx| {
        let host = args.first().ok_or_else(|| anyhow!("dns expects host"))?;
        let addrs = format!("{host}:0").to_socket_addrs()?;
        let mut rows = Vec::new();
        for a in addrs {
            let mut row = Record::new();
            row.insert("host".into(), Value::String(host.clone()));
            row.insert("ip".into(), Value::String(a.ip().to_string()));
            rows.push(row);
        }
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Table(
            Table::new(rows),
        ))))
    }
);

simple_builtin!(
    PortBuiltin,
    "port",
    "port <host> <port>",
    "Check TCP port connectivity",
    &["port 127.0.0.1 80"],
    |args, _input, _ctx| {
        if args.len() < 2 {
            bail!("port expects <host> <port>")
        }
        let host = &args[0];
        let port: u16 = args[1].parse().map_err(|_| anyhow!("invalid port"))?;
        let addr = format!("{host}:{port}");
        let ok = TcpStream::connect_timeout(
            &addr
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| anyhow!("cannot resolve address"))?,
            Duration::from_secs(2),
        )
        .is_ok();
        let mut rec = Record::new();
        rec.insert("host".into(), Value::String(host.clone()));
        rec.insert("port".into(), Value::Int(port as i64));
        rec.insert("open".into(), Value::Bool(ok));
        Ok(BuiltinOutcome::ok(PipelineData::Value(Value::Record(rec))))
    }
);

simple_builtin!(
    ServeBuiltin,
    "serve",
    "serve [dir] [port]",
    "Start simple static HTTP server in background thread",
    &["serve . 8080"],
    |args, _input, _ctx| {
        let dir = args.first().cloned().unwrap_or_else(|| ".".to_string());
        let port = args
            .get(1)
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(8080);
        let bind = format!("0.0.0.0:{port}");
        let listener = TcpListener::bind(&bind)?;
        let serve_dir = dir.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if let Ok(mut stream) = conn {
                    let _ = handle_http_client(&mut stream, &serve_dir);
                }
            }
        });
        Ok(BuiltinOutcome::ok(PipelineData::Text(format!(
            "serving {dir} on http://{bind}"
        ))))
    }
);

fn handle_http_client(stream: &mut TcpStream, dir: &str) -> anyhow::Result<()> {
    let mut buf = [0_u8; 1024];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("/");
    let rel = if path == "/" {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };
    let full = std::path::Path::new(dir).join(rel);
    if full.is_file() {
        let body = std::fs::read(full)?;
        stream.write_all(
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes(),
        )?;
        stream.write_all(&body)?;
    } else {
        let body = b"Not Found";
        stream.write_all(
            format!(
                "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )?;
        stream.write_all(body)?;
    }
    Ok(())
}
