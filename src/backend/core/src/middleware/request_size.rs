//! Request body size limiting middleware.
use axum::{extract::Request, http::StatusCode, response::{IntoResponse, Response}, Json};
use futures::future::BoxFuture;
use metrics::counter;
use serde_json::json;
use std::{collections::HashMap, task::{Context, Poll}};
use tower::{Layer, Service};
use tracing::warn;
#[derive(Debug, Clone)]
pub struct RequestSizeConfig { pub default_limit: usize, pub endpoint_limits: HashMap<String, usize>, pub excluded_paths: Vec<String> }
impl Default for RequestSizeConfig { fn default() -> Self { Self { default_limit: 1_048_576, endpoint_limits: HashMap::new(), excluded_paths: vec![] } } }
impl RequestSizeConfig { fn limit_for_path(&self, path: &str) -> Option<usize> { for e in &self.excluded_paths { if path.starts_with(e) { return None; } } for (p, l) in &self.endpoint_limits { if path.starts_with(p) { return Some(*l); } } Some(self.default_limit) } }
#[derive(Debug, Clone)] pub struct RequestSizeLayer { config: RequestSizeConfig }
impl RequestSizeLayer { pub fn new(config: RequestSizeConfig) -> Self { Self { config } } }
impl<S> Layer<S> for RequestSizeLayer { type Service = RequestSizeService<S>; fn layer(&self, inner: S) -> Self::Service { RequestSizeService { inner, config: self.config.clone() } } }
#[derive(Debug, Clone)] pub struct RequestSizeService<S> { inner: S, config: RequestSizeConfig }
impl<S> Service<Request> for RequestSizeService<S> where S: Service<Request, Response = Response> + Clone + Send + 'static, S::Future: Send + 'static, {
    type Response = Response; type Error = S::Error; type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { self.inner.poll_ready(cx) }
    fn call(&mut self, req: Request) -> Self::Future { let config = self.config.clone(); let mut inner = self.inner.clone(); Box::pin(async move { let path = req.uri().path().to_string(); if let Some(max) = config.limit_for_path(&path) { if let Some(cl) = req.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<usize>().ok()) { if cl > max { counter!("http_request_size_exceeded_total").increment(1); warn!(path=%path, "Body exceeds limit"); return Ok((StatusCode::PAYLOAD_TOO_LARGE, Json(json!({"success":false,"error":"Payload too large","error_code":"PAYLOAD_TOO_LARGE"}))).into_response()); } } } inner.call(req).await }) }
}
