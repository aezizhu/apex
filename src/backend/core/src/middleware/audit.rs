//! Audit logging middleware.
use axum::{extract::Request, http::{Method, StatusCode}, response::Response};
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use serde::Serialize;
use std::{sync::Arc, task::{Context, Poll}, time::Instant};
use tokio::sync::mpsc;
use tower::{Layer, Service};
use tracing::{error, info};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum AuditLevel { Minimal, Standard, Full }
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry { pub timestamp: DateTime<Utc>, pub request_id: Option<String>, pub method: String, pub path: String, pub status: u16, pub user_id: Option<String>, pub client_ip: Option<String>, pub auth_method: Option<String>, pub duration_ms: u64, pub audit_level: AuditLevel, pub user_agent: Option<String> }
#[derive(Debug, Clone)] pub struct AuditRule { pub path_prefix: String, pub level: AuditLevel }
#[derive(Debug, Clone)]
pub struct AuditConfig { pub rules: Vec<AuditRule>, pub always_audit_methods: Vec<Method>, pub always_audit_statuses: Vec<StatusCode>, pub default_level: Option<AuditLevel>, pub channel_buffer_size: usize }
impl Default for AuditConfig { fn default() -> Self { Self { rules: vec![AuditRule{path_prefix:"/api/v1/admin".into(),level:AuditLevel::Full},AuditRule{path_prefix:"/api/v2/admin".into(),level:AuditLevel::Full},AuditRule{path_prefix:"/api/v1/auth".into(),level:AuditLevel::Standard},AuditRule{path_prefix:"/api/v2/auth".into(),level:AuditLevel::Standard},AuditRule{path_prefix:"/api/v1/keys".into(),level:AuditLevel::Full},AuditRule{path_prefix:"/api/v2/keys".into(),level:AuditLevel::Full}], always_audit_methods: vec![Method::DELETE,Method::PUT,Method::PATCH], always_audit_statuses: vec![StatusCode::UNAUTHORIZED,StatusCode::FORBIDDEN,StatusCode::INTERNAL_SERVER_ERROR], default_level: None, channel_buffer_size: 1024 } } }
const SENSITIVE_FIELDS: &[&str] = &["password","passwd","secret","token","api_key","apikey","credential","private_key","access_token","refresh_token","authorization"];
pub fn is_sensitive_field(name: &str) -> bool { let l = name.to_lowercase(); SENSITIVE_FIELDS.iter().any(|p| l.contains(p)) }
pub fn redact_if_sensitive(name: &str, value: &str) -> String { if is_sensitive_field(name) { "[REDACTED]".into() } else { value.into() } }
#[derive(Debug, Clone)] pub struct AuditLogger { sender: mpsc::Sender<AuditEntry> }
impl AuditLogger {
    pub fn new(buf: usize) -> Self { let (tx, mut rx) = mpsc::channel::<AuditEntry>(buf); tokio::spawn(async move { while let Some(e) = rx.recv().await { info!(target:"audit", method=%e.method, path=%e.path, status=e.status, "AUDIT"); } }); Self { sender: tx } }
    pub async fn log(&self, entry: AuditEntry) { if let Err(e) = self.sender.send(entry).await { error!("Audit send failed: {}", e); } }
}
#[derive(Clone)] pub struct AuditLayer { config: Arc<AuditConfig>, logger: AuditLogger }
impl AuditLayer { pub fn new(config: AuditConfig) -> Self { let logger = AuditLogger::new(config.channel_buffer_size); Self { config: Arc::new(config), logger } } }
impl<S> Layer<S> for AuditLayer { type Service = AuditService<S>; fn layer(&self, inner: S) -> Self::Service { AuditService { inner, config: self.config.clone(), logger: self.logger.clone() } } }
#[derive(Clone)] pub struct AuditService<S> { inner: S, config: Arc<AuditConfig>, logger: AuditLogger }
impl<S> Service<Request> for AuditService<S> where S: Service<Request, Response = Response> + Clone + Send + 'static, S::Future: Send + 'static, {
    type Response = Response; type Error = S::Error; type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { self.inner.poll_ready(cx) }
    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone(); let config = self.config.clone(); let logger = self.logger.clone();
        let method = req.method().clone(); let path = req.uri().path().to_string();
        let request_id = req.headers().get("x-request-id").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        let user_agent = req.headers().get("user-agent").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        let client_ip = req.headers().get("x-forwarded-for").or_else(|| req.headers().get("x-real-ip")).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
        let auth_method = if req.headers().contains_key("authorization") { Some("bearer".into()) } else if req.headers().contains_key("x-api-key") { Some("api_key".into()) } else { None };
        let mut lvl: Option<AuditLevel> = None;
        for r in &config.rules { if path.starts_with(&r.path_prefix) { lvl = Some(r.level); break; } }
        if lvl.is_none() && config.always_audit_methods.contains(&method) { lvl = Some(AuditLevel::Minimal); }
        if lvl.is_none() { lvl = config.default_level; }
        let start = Instant::now();
        Box::pin(async move {
            let resp = inner.call(req).await?; let status = resp.status(); let dur = start.elapsed().as_millis() as u64;
            let mut al = lvl; if al.is_none() && config.always_audit_statuses.contains(&status) { al = Some(AuditLevel::Standard); }
            if let Some(level) = al { logger.log(AuditEntry { timestamp: Utc::now(), request_id, method: method.to_string(), path, status: status.as_u16(), user_id: None, client_ip, auth_method, duration_ms: dur, audit_level: level, user_agent }).await; }
            Ok(resp)
        })
    }
}
