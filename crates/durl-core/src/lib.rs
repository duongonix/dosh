mod client;
mod model;

pub use client::execute;
pub use model::{
    AuthSpec, HttpHeaders, QueryParams, RequestBody, RequestSpec, ResponseBody, ResponseData,
};
