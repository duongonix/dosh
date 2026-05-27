use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpHeaders(pub BTreeMap<String, String>);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryParams(pub BTreeMap<String, String>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthSpec {
    Bearer(String),
    Basic { username: String, password: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestBody {
    Empty,
    Json(serde_json::Value),
    Text(String),
    Binary(Vec<u8>),
    Form(BTreeMap<String, String>),
    Multipart {
        fields: BTreeMap<String, String>,
        files: Vec<(String, String)>,
    },
}

impl Default for RequestBody {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSpec {
    pub method: String,
    pub url: String,
    pub headers: HttpHeaders,
    pub query: QueryParams,
    pub body: RequestBody,
    pub auth: Option<AuthSpec>,
    pub timeout: Option<Duration>,
    pub retry: usize,
    pub follow_redirects: bool,
    pub raw: bool,
    pub full: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseBody {
    Json(serde_json::Value),
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub status: u16,
    pub status_text: String,
    pub headers: HttpHeaders,
    pub body: ResponseBody,
    pub url: String,
    pub method: String,
    pub duration_ms: u128,
}
