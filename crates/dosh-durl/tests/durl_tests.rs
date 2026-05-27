use dosh_durl::{DurlRunOptions, run};
use dosh_value::Value;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn spawn_server(handler: fn(String) -> String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let resp = handler(req);
        let _ = stream.write_all(resp.as_bytes());
    });
    format!("http://{}", addr)
}

fn spawn_server_n(handler: fn(String) -> String, n: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    thread::spawn(move || {
        for _ in 0..n {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 4096];
            let k = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..k]).to_string();
            let resp = handler(req);
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://{}", addr)
}

#[test]
fn get_json_structured() {
    let url = spawn_server(|_| {
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"a\":1,\"b\":2}"
            .to_string()
    });
    let out = run(DurlRunOptions {
        args: vec!["get".into(), url],
        input: None,
    })
    .expect("run");
    match out {
        Value::Record(r) => assert_eq!(r.get("a"), Some(&Value::Int(1))),
        _ => panic!("expected record"),
    }
}

#[test]
fn full_mode_has_metadata() {
    let url = spawn_server(|_| {
        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}"
            .to_string()
    });
    let out = run(DurlRunOptions {
        args: vec!["post".into(), url, "--full".into()],
        input: Some(Value::Record({
            let mut r = dosh_value::Record::new();
            r.insert("x".into(), Value::Int(1));
            r
        })),
    })
    .expect("run");
    match out {
        Value::Record(r) => {
            assert_eq!(r.get("status"), Some(&Value::Int(201)));
            assert!(r.get("body").is_some());
        }
        _ => panic!("expected full record"),
    }
}

#[test]
fn raw_mode_returns_text() {
    let url = spawn_server(|_| {
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 7\r\n\r\n{\"x\":1}"
            .to_string()
    });
    let out = run(DurlRunOptions {
        args: vec!["get".into(), url, "--raw".into()],
        input: None,
    })
    .expect("run");
    match out {
        Value::String(s) => assert!(s.contains("\"x\":1")),
        _ => panic!("expected raw text"),
    }
}

#[test]
fn query_and_header_and_auth_work() {
    let url = spawn_server_n(
        |req| {
            assert!(req.contains("GET /x?"));
            assert!(req.contains("page=1"));
            assert!(req.contains("limit=2"));
            assert!(req.to_ascii_lowercase().contains("x-test: hello"));
            assert!(req.contains("authorization: Bearer tok"));
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok".to_string()
        },
        1,
    );
    let out = run(DurlRunOptions {
        args: vec![
            "get".into(),
            format!("{url}/x"),
            "--query".into(),
            "{\"page\":\"1\",\"limit\":\"2\"}".into(),
            "-H".into(),
            "X-Test: hello".into(),
            "--bearer".into(),
            "tok".into(),
        ],
        input: None,
    })
    .expect("run");
    assert_eq!(out, Value::String("ok".into()));
}

#[test]
fn output_download_writes_file() {
    let dir = tempfile::tempdir().expect("tmp");
    let out_path = dir.path().join("out.bin");
    let url = spawn_server(|_| {
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 4\r\n\r\nABCD"
            .to_string()
    });
    let _ = run(DurlRunOptions {
        args: vec![
            "get".into(),
            url,
            "--output".into(),
            out_path.to_string_lossy().to_string(),
        ],
        input: None,
    })
    .expect("run");
    let bytes = std::fs::read(out_path).expect("read file");
    assert_eq!(bytes, b"ABCD");
}
