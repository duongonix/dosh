use durl_core::{RequestBody, RequestSpec, execute};

fn serve_once(response: &'static str) -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0u8; 2048];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(response.as_bytes());
    });
    format!("http://{}", addr)
}

fn base_spec(url: String) -> RequestSpec {
    RequestSpec {
        method: "GET".into(),
        url,
        headers: Default::default(),
        query: Default::default(),
        body: RequestBody::Empty,
        auth: None,
        timeout: Some(std::time::Duration::from_secs(5)),
        retry: 0,
        follow_redirects: true,
        raw: false,
        full: false,
    }
}

#[tokio::test]
async fn get_json_parses() {
    let url = serve_once(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"ok\":true}\n",
    );
    let res = execute(&base_spec(url)).await.expect("execute");
    assert_eq!(res.status, 200);
    match res.body {
        durl_core::ResponseBody::Json(v) => assert_eq!(v["ok"], true),
        _ => panic!("expected json"),
    }
}

#[tokio::test]
async fn raw_mode_forces_text() {
    let url = serve_once(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 7\r\n\r\n{\"a\":1}",
    );
    let mut spec = base_spec(url);
    spec.raw = true;
    let res = execute(&spec).await.expect("execute");
    match res.body {
        durl_core::ResponseBody::Text(s) => assert!(s.contains("\"a\":1")),
        _ => panic!("expected text"),
    }
}
