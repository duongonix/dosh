use crate::model::*;
use anyhow::{Context, Result, anyhow};
use reqwest::redirect::Policy;
use reqwest::{Client, Method};
use std::time::Instant;
use tokio::time::sleep;

pub async fn execute(spec: &RequestSpec) -> Result<ResponseData> {
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 0..=spec.retry {
        match execute_once(spec).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt < spec.retry {
                    sleep(std::time::Duration::from_millis(100 * (attempt as u64 + 1))).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("request failed")))
}

async fn execute_once(spec: &RequestSpec) -> Result<ResponseData> {
    let mut builder = Client::builder().redirect(if spec.follow_redirects {
        Policy::limited(10)
    } else {
        Policy::none()
    });
    if let Some(t) = spec.timeout {
        builder = builder.timeout(t);
    }
    let client = builder
        .build()
        .context("failed to build HTTP client configuration")?;
    let method = Method::from_bytes(spec.method.as_bytes())
        .with_context(|| format!("invalid HTTP method: {}", spec.method))?;

    let mut req = client.request(method.clone(), &spec.url);
    for (k, v) in &spec.headers.0 {
        req = req.header(k, v);
    }
    if !spec.query.0.is_empty() {
        req = req.query(&spec.query.0);
    }
    if let Some(auth) = &spec.auth {
        match auth {
            AuthSpec::Bearer(t) => req = req.bearer_auth(t),
            AuthSpec::Basic { username, password } => {
                req = req.basic_auth(username, Some(password))
            }
        }
    }
    req = apply_body(req, &spec.body).await?;

    let started = Instant::now();
    let resp = req
        .send()
        .await
        .context("failed to connect or send request")?;
    let duration_ms = started.elapsed().as_millis();
    let status = resp.status();
    let final_url = resp.url().to_string();
    let mut headers = std::collections::BTreeMap::new();
    for (k, v) in resp.headers() {
        headers.insert(k.to_string(), v.to_str().unwrap_or_default().to_string());
    }
    let bytes = resp.bytes().await.context("failed to read response body")?;
    let ctype = headers.get("content-type").cloned().unwrap_or_default();
    let body = if spec.raw {
        ResponseBody::Text(String::from_utf8_lossy(&bytes).to_string())
    } else if ctype.contains("application/json") {
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(v) => ResponseBody::Json(v),
            Err(_) => ResponseBody::Text(String::from_utf8_lossy(&bytes).to_string()),
        }
    } else if ctype.starts_with("text/")
        || ctype.contains("xml")
        || ctype.contains("html")
        || ctype.contains("yaml")
        || ctype.contains("toml")
    {
        ResponseBody::Text(String::from_utf8_lossy(&bytes).to_string())
    } else {
        ResponseBody::Binary(bytes.to_vec())
    };

    Ok(ResponseData {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        headers: HttpHeaders(headers),
        body,
        url: final_url,
        method: method.as_str().to_string(),
        duration_ms,
    })
}

async fn apply_body(
    mut req: reqwest::RequestBuilder,
    body: &RequestBody,
) -> Result<reqwest::RequestBuilder> {
    req = match body {
        RequestBody::Empty => req,
        RequestBody::Json(v) => req.json(v),
        RequestBody::Text(s) => req
            .header("content-type", "text/plain; charset=utf-8")
            .body(s.clone()),
        RequestBody::Binary(b) => req
            .header("content-type", "application/octet-stream")
            .body(b.clone()),
        RequestBody::Form(m) => req.form(m),
        RequestBody::Multipart { fields, files } => {
            let mut mp = reqwest::multipart::Form::new();
            for (k, v) in fields {
                mp = mp.text(k.clone(), v.clone());
            }
            for (field, path) in files {
                let bytes = tokio::fs::read(path)
                    .await
                    .with_context(|| format!("multipart file not found: {path}"))?;
                let part = reqwest::multipart::Part::bytes(bytes);
                mp = mp.part(field.clone(), part);
            }
            req.multipart(mp)
        }
    };
    Ok(req)
}
