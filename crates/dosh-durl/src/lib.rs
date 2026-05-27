use anyhow::{anyhow, bail};
use dosh_value::{Record, Value};
use durl_core::{
    AuthSpec, HttpHeaders, QueryParams, RequestBody, RequestSpec, ResponseBody, execute,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DurlRunOptions {
    pub args: Vec<String>,
    pub input: Option<Value>,
}

pub fn run(options: DurlRunOptions) -> anyhow::Result<Value> {
    let spec = parse_spec(&options.args, options.input)?;
    let output_path = find_flag_value(&options.args, "--output").map(PathBuf::from);
    let rt =
        tokio::runtime::Runtime::new().map_err(|e| anyhow!("cannot init async runtime: {e}"))?;
    let res = rt.block_on(execute(&spec))?;
    if let Some(out) = output_path {
        match &res.body {
            ResponseBody::Binary(bytes) => std::fs::write(out, bytes)?,
            ResponseBody::Text(text) => std::fs::write(out, text)?,
            ResponseBody::Json(v) => std::fs::write(out, serde_json::to_vec_pretty(v)?)?,
        }
    }
    Ok(to_value(&res, spec.full))
}

fn parse_spec(args: &[String], input: Option<Value>) -> anyhow::Result<RequestSpec> {
    let mut i = 0usize;
    let mut method = "GET".to_string();
    let mut url: Option<String> = None;
    if let Some(first) = args.first() {
        match first.to_ascii_uppercase().as_str() {
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE" => {
                method = first.to_ascii_uppercase();
                i = 1;
            }
            _ => {}
        }
    }
    if let Some(u) = args.get(i) {
        url = Some(u.clone());
        i += 1;
    }
    let url = url.ok_or_else(|| anyhow!("durl expects url"))?;

    let mut headers = BTreeMap::new();
    let mut query = BTreeMap::new();
    let mut body = RequestBody::Empty;
    let mut auth: Option<AuthSpec> = None;
    let mut timeout = None;
    let mut retry = 0usize;
    let mut follow = true;
    let mut raw = false;
    let mut full = false;
    let mut verbose = false;

    while i < args.len() {
        match args[i].as_str() {
            "--raw" => raw = true,
            "--full" => full = true,
            "--verbose" => verbose = true,
            "--follow" => follow = true,
            "--no-follow" => follow = false,
            "--retry" => {
                retry = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--retry requires number"))?
                    .parse::<usize>()?;
                i += 1;
            }
            "--timeout" => {
                let t = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--timeout requires value"))?;
                timeout = Some(parse_duration(t)?);
                i += 1;
            }
            "-H" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("-H requires header"))?;
                if let Some((k, val)) = v.split_once(':') {
                    headers.insert(k.trim().to_string(), val.trim().to_string());
                } else {
                    bail!("invalid header syntax: {v}");
                }
                i += 1;
            }
            "--bearer" => {
                let tok = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--bearer requires token"))?;
                auth = Some(AuthSpec::Bearer(tok.clone()));
                i += 1;
            }
            "--basic" => {
                let user = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--basic requires USER PASS"))?;
                let pass = args
                    .get(i + 2)
                    .ok_or_else(|| anyhow!("--basic requires USER PASS"))?;
                auth = Some(AuthSpec::Basic {
                    username: user.clone(),
                    password: pass.clone(),
                });
                i += 2;
            }
            "--query" => {
                let q = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--query requires JSON object"))?;
                query.extend(parse_object_pairs(q)?);
                i += 1;
            }
            "--headers" => {
                let h = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--headers requires JSON object"))?;
                headers.extend(parse_object_pairs(h)?);
                i += 1;
            }
            "--json" => {
                let js = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--json requires JSON value"))?;
                let val = serde_json::from_str::<serde_json::Value>(js)
                    .map_err(|_| anyhow!("invalid JSON body"))?;
                headers
                    .entry("content-type".into())
                    .or_insert_with(|| "application/json".into());
                body = RequestBody::Json(val);
                i += 1;
            }
            "--form" => {
                let js = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--form requires object"))?;
                body = RequestBody::Form(parse_object_pairs(js)?);
                i += 1;
            }
            "--multipart" => {
                let js = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow!("--multipart requires object"))?;
                let mut fields = BTreeMap::new();
                let mut files = Vec::new();
                for (k, v) in parse_object_pairs(js)? {
                    if let Some(path) = v.strip_prefix('@') {
                        files.push((k, path.to_string()));
                    } else {
                        fields.insert(k, v);
                    }
                }
                body = RequestBody::Multipart { fields, files };
                i += 1;
            }
            "--output" => i += 1,
            _ => {}
        }
        i += 1;
    }

    if matches!(body, RequestBody::Empty)
        && let Some(v) = input
    {
        body = value_to_body(&v)?;
        if matches!(body, RequestBody::Json(_)) {
            headers
                .entry("content-type".into())
                .or_insert_with(|| "application/json".into());
        }
    }
    if verbose {
        eprintln!("durl> {} {}", method, url);
    }
    Ok(RequestSpec {
        method,
        url,
        headers: HttpHeaders(headers),
        query: QueryParams(query),
        body,
        auth,
        timeout,
        retry,
        follow_redirects: follow,
        raw,
        full,
    })
}

fn parse_object_pairs(s: &str) -> anyhow::Result<BTreeMap<String, String>> {
    let v = serde_json::from_str::<serde_json::Value>(s)
        .map_err(|_| anyhow!("expected JSON object, got: {s}"))?;
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow!("expected JSON object"))?;
    let mut out = BTreeMap::new();
    for (k, v) in obj {
        out.insert(k.clone(), json_to_string(v));
    }
    Ok(out)
}

fn json_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

fn parse_duration(v: &str) -> anyhow::Result<std::time::Duration> {
    let l = v.to_ascii_lowercase();
    if let Some(n) = l.strip_suffix("ms") {
        return Ok(std::time::Duration::from_millis(n.parse::<u64>()?));
    }
    if let Some(n) = l.strip_suffix("sec") {
        return Ok(std::time::Duration::from_secs(n.parse::<u64>()?));
    }
    if let Some(n) = l.strip_suffix('s') {
        return Ok(std::time::Duration::from_secs(n.parse::<u64>()?));
    }
    if let Some(n) = l.strip_suffix("min") {
        return Ok(std::time::Duration::from_secs(n.parse::<u64>()? * 60));
    }
    if let Some(n) = l.strip_suffix("hr") {
        return Ok(std::time::Duration::from_secs(n.parse::<u64>()? * 3600));
    }
    Ok(std::time::Duration::from_secs(v.parse::<u64>()?))
}

fn value_to_body(v: &Value) -> anyhow::Result<RequestBody> {
    Ok(match v {
        Value::Null => RequestBody::Empty,
        Value::String(s) => RequestBody::Text(s.clone()),
        Value::Binary(b) => RequestBody::Binary(b.clone()),
        Value::Record(_) | Value::List(_) | Value::Table(_) => RequestBody::Json(value_to_json(v)),
        _ => RequestBody::Text(v.to_string()),
    })
}

fn value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::json!(*i),
        Value::Float(f) => serde_json::json!(*f),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::List(xs) => serde_json::Value::Array(xs.iter().map(value_to_json).collect()),
        Value::Record(r) => {
            let mut m = serde_json::Map::new();
            for (k, v) in r {
                m.insert(k.clone(), value_to_json(v));
            }
            serde_json::Value::Object(m)
        }
        Value::Table(t) => serde_json::Value::Array(
            t.rows
                .iter()
                .map(|r| value_to_json(&Value::Record(r.clone())))
                .collect(),
        ),
        Value::Binary(b) => serde_json::json!(b),
        _ => serde_json::Value::String(v.to_string()),
    }
}

fn to_value(res: &durl_core::ResponseData, full: bool) -> Value {
    let body = match &res.body {
        ResponseBody::Json(v) => json_to_value(v),
        ResponseBody::Text(s) => Value::String(s.clone()),
        ResponseBody::Binary(b) => Value::Binary(b.clone()),
    };
    if !full {
        return body;
    }
    let mut rec = Record::new();
    rec.insert("status".into(), Value::Int(res.status as i64));
    rec.insert("status_text".into(), Value::String(res.status_text.clone()));
    rec.insert("url".into(), Value::String(res.url.clone()));
    rec.insert("method".into(), Value::String(res.method.clone()));
    rec.insert("duration_ms".into(), Value::Int(res.duration_ms as i64));
    let mut hs = Record::new();
    for (k, v) in &res.headers.0 {
        hs.insert(k.clone(), Value::String(v.clone()));
    }
    rec.insert("headers".into(), Value::Record(hs));
    rec.insert("body".into(), body);
    Value::Record(rec)
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(a) => Value::List(a.iter().map(json_to_value).collect()),
        serde_json::Value::Object(m) => {
            let mut r = Record::new();
            for (k, v) in m {
                r.insert(k.clone(), json_to_value(v));
            }
            Value::Record(r)
        }
    }
}

fn find_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}
