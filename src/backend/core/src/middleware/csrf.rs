//! CSRF protection middleware.
use axum::{extract::Request, http::{Method, StatusCode}, response::{IntoResponse, Response}, Json};
use dashmap::DashMap;
use futures::future::BoxFuture;
use metrics::counter;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{sync::Arc, task::{Context, Poll}, time::{Duration, Instant}};
use tower::{Layer, Service};
use tracing::{debug, warn};
use uuid::Uuid;
#[derive(Debug, Clone)]
pub struct CsrfConfig { pub secret: String, pub token_ttl: Duration, pub exempt_paths: Vec<String>, pub protected_methods: Vec<Method>, pub token_header: String }
impl Default for CsrfConfig { fn default() -> Self { Self { secret: Uuid::new_v4().to_string(), token_ttl: Duration::from_secs(3600), exempt_paths: vec!["/api/".into(),"/health".into(),"/ready".into(),"/metrics".into(),"/ws".into()], protected_methods: vec![Method::POST,Method::PUT,Method::PATCH,Method::DELETE], token_header: "x-csrf-token".into() } } }
impl CsrfConfig { fn is_exempt(&self, p: &str) -> bool { self.exempt_paths.iter().any(|e| p.starts_with(e)) } fn is_protected(&self, m: &Method) -> bool { self.protected_methods.contains(m) } }
#[derive(Debug, Clone)] struct StoredToken { expires_at: Instant }
#[derive(Debug, Clone)] struct TokenStore { tokens: Arc<DashMap<String, StoredToken>>, ttl: Duration }
impl TokenStore {
    fn new(ttl: Duration) -> Self { Self { tokens: Arc::new(DashMap::new()), ttl } }
    fn generate(&self, secret: &str) -> String { let n = Uuid::new_v4().to_string(); let mut h = Sha256::new(); h.update(secret.as_bytes()); h.update(n.as_bytes()); let t = format!("{}:{}", n, hex::encode(h.finalize())); self.tokens.insert(t.clone(), StoredToken { expires_at: Instant::now() + self.ttl }); t }
    fn validate(&self, t: &str) -> bool { if let Some(e) = self.tokens.get(t) { if e.expires_at > Instant::now() { return true; } drop(e); self.tokens.remove(t); } false }
    fn cleanup(&self) { let now = Instant::now(); self.tokens.retain(|_, v| v.expires_at > now); }
}
#[derive(Clone)] pub struct CsrfLayer { config: Arc<CsrfConfig>, store: TokenStore }
impl CsrfLayer { pub fn new(config: CsrfConfig) -> Self { let store = TokenStore::new(config.token_ttl); Self { config: Arc::new(config), store } } }
impl<S> Layer<S> for CsrfLayer { type Service = CsrfService<S>; fn layer(&self, inner: S) -> Self::Service { CsrfService { inner, config: self.config.clone(), store: self.store.clone() } } }
#[derive(Clone)] pub struct CsrfService<S> { inner: S, config: Arc<CsrfConfig>, store: TokenStore }
impl<S> Service<Request> for CsrfService<S> where S: Service<Request, Response = Response> + Clone + Send + 'static, S::Future: Send + 'static, {
    type Response = Response; type Error = S::Error; type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { self.inner.poll_ready(cx) }
    fn call(&mut self, req: Request) -> Self::Future {
        let config = self.config.clone(); let store = self.store.clone(); let mut inner = self.inner.clone();
        Box::pin(async move {
            let method = req.method().clone(); let path = req.uri().path().to_string();
            if config.is_exempt(&path) { return inner.call(req).await; }
            let has_bearer = req.headers().get("authorization").and_then(|v| v.to_str().ok()).map(|s| s.starts_with("Bearer ")).unwrap_or(false);
            if has_bearer || req.headers().contains_key("x-api-key") { return inner.call(req).await; }
            if config.is_protected(&method) {
                match req.headers().get(config.token_header.as_str()).and_then(|v| v.to_str().ok()) {
                    Some(t) if store.validate(t) => { debug!("CSRF ok"); }
                    Some(_) => { counter!("csrf_failed_total").increment(1); return Ok((StatusCode::FORBIDDEN, Json(json!({"success":false,"error":"CSRF validation failed","error_code":"CSRF_VALIDATION_FAILED"}))).into_response()); }
                    None => { counter!("csrf_missing_total").increment(1); return Ok((StatusCode::FORBIDDEN, Json(json!({"success":false,"error":"CSRF token required","error_code":"CSRF_TOKEN_MISSING"}))).into_response()); }
                }
            }
            if rand::random::<u8>() == 0 { store.cleanup(); }
            inner.call(req).await
        })
    }
}
