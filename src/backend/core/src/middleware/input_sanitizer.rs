//! Input sanitization middleware for injection attack detection.
use axum::{extract::Request, http::StatusCode, response::{IntoResponse, Response}, Json};
use futures::future::BoxFuture;
use metrics::counter;
use regex::Regex;
use serde::Serialize;
use serde_json::json;
use std::{sync::Arc, task::{Context, Poll}};
use tower::{Layer, Service};
use tracing::warn;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum InjectionType { SqlInjection, Xss, CommandInjection, PathTraversal, LdapInjection, LogInjection }
impl std::fmt::Display for InjectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::SqlInjection => write!(f,"SQL Injection"), Self::Xss => write!(f,"XSS"), Self::CommandInjection => write!(f,"Command Injection"), Self::PathTraversal => write!(f,"Path Traversal"), Self::LdapInjection => write!(f,"LDAP Injection"), Self::LogInjection => write!(f,"Log Injection") }
    }
}
#[derive(Debug, Clone)]
pub struct SanitizeConfig { pub block_on_detection: bool, pub skip_headers: Vec<String>, pub exempt_paths: Vec<String>, pub max_url_length: usize, pub max_header_value_length: usize }
impl Default for SanitizeConfig { fn default() -> Self { Self { block_on_detection: true, skip_headers: vec!["authorization".into(),"cookie".into(),"user-agent".into(),"accept".into(),"content-type".into(),"content-length".into(),"host".into(),"x-request-id".into(),"x-forwarded-for".into(),"x-csrf-token".into(),"x-api-key".into()], exempt_paths: vec![], max_url_length: 2048, max_header_value_length: 8192 } } }
struct Patterns { sql: Vec<Regex>, xss: Vec<Regex>, path: Vec<Regex>, log: Vec<Regex> }
impl Patterns {
    fn new() -> Self { Self { sql: vec![Regex::new(r"(?i)(\bunion\b\s+\bselect\b)").unwrap(), Regex::new(r"(?i)(\bselect\b.+\bfrom\b)").unwrap(), Regex::new(r"(?i)(\bdrop\b\s+\btable\b)").unwrap(), Regex::new(r"(?i)('?\s*(or|and)\s+\d+\s*=\s*\d+)").unwrap(), Regex::new(r"(/\*|\*/|--)").unwrap(), Regex::new(r"(?i)(\bsleep\b\s*\()").unwrap()], xss: vec![Regex::new(r"(?i)(<\s*script)").unwrap(), Regex::new(r"(?i)(javascript\s*:)").unwrap(), Regex::new(r"(?i)(on(error|load|click|mouseover)\s*=)").unwrap(), Regex::new(r"(?i)(<\s*(iframe|object|embed|svg)\b)").unwrap()], path: vec![Regex::new(r"(\.\./|\.\.\\)").unwrap(), Regex::new(r"(%2e%2e%2f|%2e%2e/)").unwrap(), Regex::new(r"(%00|%0d%0a)").unwrap()], log: vec![Regex::new(r"[\r\n]").unwrap()] } }
    fn detect(&self, s: &str) -> Option<InjectionType> { for p in &self.path { if p.is_match(s) { return Some(InjectionType::PathTraversal); } } for p in &self.sql { if p.is_match(s) { return Some(InjectionType::SqlInjection); } } for p in &self.xss { if p.is_match(s) { return Some(InjectionType::Xss); } } for p in &self.log { if p.is_match(s) { return Some(InjectionType::LogInjection); } } None }
    fn detect_url(&self, s: &str) -> Option<InjectionType> { for p in &self.path { if p.is_match(s) { return Some(InjectionType::PathTraversal); } } for p in &self.xss { if p.is_match(s) { return Some(InjectionType::Xss); } } None }
}
#[derive(Clone)] pub struct InputSanitizerLayer { config: Arc<SanitizeConfig>, patterns: Arc<Patterns> }
impl InputSanitizerLayer { pub fn new(config: SanitizeConfig) -> Self { Self { config: Arc::new(config), patterns: Arc::new(Patterns::new()) } } }
impl<S> Layer<S> for InputSanitizerLayer { type Service = InputSanitizerService<S>; fn layer(&self, inner: S) -> Self::Service { InputSanitizerService { inner, config: self.config.clone(), patterns: self.patterns.clone() } } }
#[derive(Clone)] pub struct InputSanitizerService<S> { inner: S, config: Arc<SanitizeConfig>, patterns: Arc<Patterns> }
impl<S> Service<Request> for InputSanitizerService<S> where S: Service<Request, Response = Response> + Clone + Send + 'static, S::Future: Send + 'static, {
    type Response = Response; type Error = S::Error; type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { self.inner.poll_ready(cx) }
    fn call(&mut self, req: Request) -> Self::Future { let config = self.config.clone(); let patterns = self.patterns.clone(); let mut inner = self.inner.clone(); Box::pin(async move { let path = req.uri().path().to_string(); if config.exempt_paths.iter().any(|p| path.starts_with(p)) { return inner.call(req).await; } if req.uri().to_string().len() > config.max_url_length { counter!("sanitizer_url_too_long").increment(1); if config.block_on_detection { return Ok(bad_req("URL too long")); } } if let Some(t) = patterns.detect_url(&path) { counter!("sanitizer_blocked").increment(1); warn!(path=%path, "Injection in URL"); if config.block_on_detection { return Ok(bad_req(&t.to_string())); } } if let Some(q) = req.uri().query() { if let Some(t) = patterns.detect(q) { counter!("sanitizer_blocked").increment(1); if config.block_on_detection { return Ok(bad_req(&t.to_string())); } } } for (n, v) in req.headers() { let h = n.as_str().to_lowercase(); if config.skip_headers.contains(&h) { continue; } if let Ok(vs) = v.to_str() { if vs.len() > config.max_header_value_length { if config.block_on_detection { return Ok(bad_req("Header too long")); } } if let Some(t) = patterns.detect(vs) { counter!("sanitizer_blocked").increment(1); if config.block_on_detection { return Ok(bad_req(&t.to_string())); } } } } inner.call(req).await }) }
}
fn bad_req(detail: &str) -> Response { (StatusCode::BAD_REQUEST, Json(json!({"success":false,"error":"Malicious input detected","error_code":"MALICIOUS_INPUT","detail":detail}))).into_response() }
