//! Security headers middleware.
use axum::{extract::Request, http::{HeaderName, HeaderValue}, response::Response};
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameOptions { Deny, SameOrigin }
impl Default for FrameOptions { fn default() -> Self { Self::Deny } }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferrerPolicy { NoReferrer, NoReferrerWhenDowngrade, Origin, OriginWhenCrossOrigin, SameOrigin, StrictOrigin, StrictOriginWhenCrossOrigin, UnsafeUrl }
impl Default for ReferrerPolicy { fn default() -> Self { Self::StrictOriginWhenCrossOrigin } }
impl std::fmt::Display for ReferrerPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoReferrer => write!(f, "no-referrer"), Self::NoReferrerWhenDowngrade => write!(f, "no-referrer-when-downgrade"),
            Self::Origin => write!(f, "origin"), Self::OriginWhenCrossOrigin => write!(f, "origin-when-cross-origin"),
            Self::SameOrigin => write!(f, "same-origin"), Self::StrictOrigin => write!(f, "strict-origin"),
            Self::StrictOriginWhenCrossOrigin => write!(f, "strict-origin-when-cross-origin"), Self::UnsafeUrl => write!(f, "unsafe-url"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    pub frame_options: FrameOptions, pub referrer_policy: ReferrerPolicy, pub hsts_max_age: u64,
    pub hsts_include_subdomains: bool, pub hsts_preload: bool, pub content_security_policy: String,
    pub permissions_policy: String, pub enable_request_id: bool, pub remove_server_header: bool, pub api_no_cache: bool,
}
impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self { frame_options: FrameOptions::Deny, referrer_policy: ReferrerPolicy::StrictOriginWhenCrossOrigin, hsts_max_age: 31_536_000,
            hsts_include_subdomains: true, hsts_preload: false,
            content_security_policy: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'".into(),
            permissions_policy: "camera=(), microphone=(), geolocation=(), payment=()".into(),
            enable_request_id: true, remove_server_header: true, api_no_cache: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SecurityHeadersLayer { config: SecurityHeadersConfig }
impl SecurityHeadersLayer { pub fn new(config: SecurityHeadersConfig) -> Self { Self { config } } }
impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersService<S>;
    fn layer(&self, inner: S) -> Self::Service { SecurityHeadersService { inner, config: self.config.clone() } }
}

#[derive(Debug, Clone)]
pub struct SecurityHeadersService<S> { inner: S, config: SecurityHeadersConfig }
impl<S> Service<Request> for SecurityHeadersService<S>
where S: Service<Request, Response = Response> + Clone + Send + 'static, S::Future: Send + 'static, {
    type Response = Response; type Error = S::Error; type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> { self.inner.poll_ready(cx) }
    fn call(&mut self, mut req: Request) -> Self::Future {
        let config = self.config.clone(); let mut inner = self.inner.clone();
        let request_id = if config.enable_request_id {
            let id = req.headers().get("x-request-id").and_then(|v| v.to_str().ok()).map(|s| s.to_string()).unwrap_or_else(|| Uuid::new_v4().to_string());
            if let Ok(val) = HeaderValue::from_str(&id) { req.headers_mut().insert(HeaderName::from_static("x-request-id"), val); }
            Some(id)
        } else { None };
        let is_api = req.uri().path().starts_with("/api/");
        Box::pin(async move {
            let mut response = inner.call(req).await?; let headers = response.headers_mut();
            headers.insert(HeaderName::from_static("x-content-type-options"), HeaderValue::from_static("nosniff"));
            headers.insert(HeaderName::from_static("x-frame-options"), HeaderValue::from_static(match &config.frame_options { FrameOptions::Deny => "DENY", FrameOptions::SameOrigin => "SAMEORIGIN" }));
            headers.insert(HeaderName::from_static("x-xss-protection"), HeaderValue::from_static("0"));
            let mut hsts = format!("max-age={}", config.hsts_max_age);
            if config.hsts_include_subdomains { hsts.push_str("; includeSubDomains"); }
            if config.hsts_preload { hsts.push_str("; preload"); }
            if let Ok(v) = HeaderValue::from_str(&hsts) { headers.insert(HeaderName::from_static("strict-transport-security"), v); }
            if !config.content_security_policy.is_empty() { if let Ok(v) = HeaderValue::from_str(&config.content_security_policy) { headers.insert(HeaderName::from_static("content-security-policy"), v); } }
            if let Ok(v) = HeaderValue::from_str(&config.referrer_policy.to_string()) { headers.insert(HeaderName::from_static("referrer-policy"), v); }
            if !config.permissions_policy.is_empty() { if let Ok(v) = HeaderValue::from_str(&config.permissions_policy) { headers.insert(HeaderName::from_static("permissions-policy"), v); } }
            if let Some(id) = request_id { if let Ok(v) = HeaderValue::from_str(&id) { headers.insert(HeaderName::from_static("x-request-id"), v); } }
            if config.remove_server_header { headers.remove("server"); }
            if config.api_no_cache && is_api { headers.insert(HeaderName::from_static("cache-control"), HeaderValue::from_static("no-store, no-cache, must-revalidate, private")); }
            Ok(response)
        })
    }
}
