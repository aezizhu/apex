//! HTTP caching middleware.
//!
//! This module provides:
//! - ETag support for cache validation
//! - Cache-Control header generation
//! - Conditional request handling (If-None-Match, If-Modified-Since)
//! - Response caching for idempotent requests

use crate::cache::{Cache, CacheKey, KeyType};
use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use metrics::counter;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::{Layer, Service};
use tracing::{debug, instrument, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Control Directives
// ═══════════════════════════════════════════════════════════════════════════════

/// Cache-Control directive types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheDirective {
    /// max-age=<seconds>
    MaxAge(u64),

    /// s-maxage=<seconds> (shared cache)
    SMaxAge(u64),

    /// no-cache
    NoCache,

    /// no-store
    NoStore,

    /// no-transform
    NoTransform,

    /// must-revalidate
    MustRevalidate,

    /// proxy-revalidate
    ProxyRevalidate,

    /// private
    Private,

    /// public
    Public,

    /// immutable
    Immutable,

    /// stale-while-revalidate=<seconds>
    StaleWhileRevalidate(u64),

    /// stale-if-error=<seconds>
    StaleIfError(u64),
}

impl CacheDirective {
    /// Convert to header string representation.
    pub fn to_header_string(&self) -> String {
        match self {
            Self::MaxAge(secs) => format!("max-age={}", secs),
            Self::SMaxAge(secs) => format!("s-maxage={}", secs),
            Self::NoCache => "no-cache".to_string(),
            Self::NoStore => "no-store".to_string(),
            Self::NoTransform => "no-transform".to_string(),
            Self::MustRevalidate => "must-revalidate".to_string(),
            Self::ProxyRevalidate => "proxy-revalidate".to_string(),
            Self::Private => "private".to_string(),
            Self::Public => "public".to_string(),
            Self::Immutable => "immutable".to_string(),
            Self::StaleWhileRevalidate(secs) => format!("stale-while-revalidate={}", secs),
            Self::StaleIfError(secs) => format!("stale-if-error={}", secs),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Control Builder
// ═══════════════════════════════════════════════════════════════════════════════

/// Builder for Cache-Control headers.
#[derive(Debug, Clone, Default)]
pub struct CacheControl {
    directives: HashSet<CacheDirective>,
}

impl CacheControl {
    /// Create a new Cache-Control builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create Cache-Control for static assets.
    pub fn static_assets() -> Self {
        Self::new()
            .public()
            .max_age(Duration::from_secs(31536000)) // 1 year
            .immutable()
    }

    /// Create Cache-Control for API responses.
    pub fn api_response(max_age: Duration) -> Self {
        Self::new()
            .private()
            .max_age(max_age)
            .must_revalidate()
    }

    /// Create Cache-Control for no caching.
    pub fn no_cache() -> Self {
        Self::new()
            .with_directive(CacheDirective::NoCache)
            .with_directive(CacheDirective::NoStore)
            .with_directive(CacheDirective::MustRevalidate)
    }

    /// Add a directive.
    pub fn with_directive(mut self, directive: CacheDirective) -> Self {
        self.directives.insert(directive);
        self
    }

    /// Set max-age.
    pub fn max_age(self, duration: Duration) -> Self {
        self.with_directive(CacheDirective::MaxAge(duration.as_secs()))
    }

    /// Set s-maxage (shared cache max-age).
    pub fn s_maxage(self, duration: Duration) -> Self {
        self.with_directive(CacheDirective::SMaxAge(duration.as_secs()))
    }

    /// Mark as private (only cacheable by browsers).
    pub fn private(self) -> Self {
        self.with_directive(CacheDirective::Private)
    }

    /// Mark as public (cacheable by CDNs).
    pub fn public(self) -> Self {
        self.with_directive(CacheDirective::Public)
    }

    /// Mark as immutable.
    pub fn immutable(self) -> Self {
        self.with_directive(CacheDirective::Immutable)
    }

    /// Add must-revalidate.
    pub fn must_revalidate(self) -> Self {
        self.with_directive(CacheDirective::MustRevalidate)
    }

    /// Add stale-while-revalidate.
    pub fn stale_while_revalidate(self, duration: Duration) -> Self {
        self.with_directive(CacheDirective::StaleWhileRevalidate(duration.as_secs()))
    }

    /// Add stale-if-error.
    pub fn stale_if_error(self, duration: Duration) -> Self {
        self.with_directive(CacheDirective::StaleIfError(duration.as_secs()))
    }

    /// Build the Cache-Control header value.
    pub fn build(&self) -> String {
        self.directives
            .iter()
            .map(|d| d.to_header_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Convert to HeaderValue.
    pub fn to_header_value(&self) -> HeaderValue {
        HeaderValue::from_str(&self.build()).unwrap_or_else(|_| {
            HeaderValue::from_static("no-cache")
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ETag Generator
// ═══════════════════════════════════════════════════════════════════════════════

/// ETag generation utilities.
pub struct ETagGenerator;

impl ETagGenerator {
    /// Generate a strong ETag from content.
    pub fn strong(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash = hasher.finalize();
        format!("\"{}\"", hex::encode(&hash[..16]))
    }

    /// Generate a weak ETag from content.
    pub fn weak(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash = hasher.finalize();
        format!("W/\"{}\"", hex::encode(&hash[..8]))
    }

    /// Generate an ETag from a version/timestamp.
    pub fn from_version(version: u64) -> String {
        format!("\"v{}\"", version)
    }

    /// Generate an ETag from a timestamp.
    pub fn from_timestamp(timestamp: DateTime<Utc>) -> String {
        format!("\"{}\"", timestamp.timestamp())
    }

    /// Check if two ETags match (handling weak comparison).
    pub fn matches(etag1: &str, etag2: &str) -> bool {
        let e1 = etag1.trim_start_matches("W/");
        let e2 = etag2.trim_start_matches("W/");
        e1 == e2
    }

    /// Check if ETag matches any in a list (from If-None-Match header).
    pub fn matches_any(etag: &str, if_none_match: &str) -> bool {
        if if_none_match.trim() == "*" {
            return true;
        }

        for candidate in if_none_match.split(',') {
            let candidate = candidate.trim();
            if Self::matches(etag, candidate) {
                return true;
            }
        }

        false
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Middleware Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for cache middleware.
#[derive(Debug, Clone)]
pub struct CacheMiddlewareConfig {
    /// Enable response caching
    pub enable_caching: bool,

    /// Enable ETag generation
    pub enable_etag: bool,

    /// Enable Cache-Control headers
    pub enable_cache_control: bool,

    /// Default max-age for cacheable responses
    pub default_max_age: Duration,

    /// Methods that are cacheable
    pub cacheable_methods: Vec<Method>,

    /// Status codes that are cacheable
    pub cacheable_status_codes: Vec<StatusCode>,

    /// Paths to exclude from caching (glob patterns)
    pub exclude_paths: Vec<String>,

    /// Vary headers to include
    pub vary_headers: Vec<String>,

    /// Use weak ETags
    pub use_weak_etag: bool,
}

impl Default for CacheMiddlewareConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            enable_etag: true,
            enable_cache_control: true,
            default_max_age: Duration::from_secs(60),
            cacheable_methods: vec![Method::GET, Method::HEAD],
            cacheable_status_codes: vec![
                StatusCode::OK,
                StatusCode::NON_AUTHORITATIVE_INFORMATION,
                StatusCode::NO_CONTENT,
                StatusCode::PARTIAL_CONTENT,
                StatusCode::MULTIPLE_CHOICES,
                StatusCode::MOVED_PERMANENTLY,
                StatusCode::NOT_FOUND,
                StatusCode::METHOD_NOT_ALLOWED,
                StatusCode::GONE,
            ],
            exclude_paths: vec![
                "/health".to_string(),
                "/metrics".to_string(),
                "/api/*/stream".to_string(),
            ],
            vary_headers: vec![
                "Accept".to_string(),
                "Accept-Encoding".to_string(),
                "Authorization".to_string(),
            ],
            use_weak_etag: false,
        }
    }
}

impl CacheMiddlewareConfig {
    /// Create a builder for cache middleware configuration.
    pub fn builder() -> CacheMiddlewareConfigBuilder {
        CacheMiddlewareConfigBuilder::default()
    }

    /// Check if a path should be excluded from caching.
    pub fn is_excluded(&self, path: &str) -> bool {
        for pattern in &self.exclude_paths {
            if glob_matches(pattern, path) {
                return true;
            }
        }
        false
    }
}

/// Builder for cache middleware configuration.
#[derive(Debug, Default)]
pub struct CacheMiddlewareConfigBuilder {
    config: CacheMiddlewareConfig,
}

impl CacheMiddlewareConfigBuilder {
    pub fn enable_caching(mut self, enabled: bool) -> Self {
        self.config.enable_caching = enabled;
        self
    }

    pub fn enable_etag(mut self, enabled: bool) -> Self {
        self.config.enable_etag = enabled;
        self
    }

    pub fn default_max_age(mut self, duration: Duration) -> Self {
        self.config.default_max_age = duration;
        self
    }

    pub fn exclude_path(mut self, path: impl Into<String>) -> Self {
        self.config.exclude_paths.push(path.into());
        self
    }

    pub fn use_weak_etag(mut self, weak: bool) -> Self {
        self.config.use_weak_etag = weak;
        self
    }

    pub fn build(self) -> CacheMiddlewareConfig {
        self.config
    }
}

/// Simple glob matching.
fn glob_matches(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if let Some(found) = text[pos..].find(part) {
            if i == 0 && found != 0 {
                return false;
            }
            pos += found + part.len();
        } else {
            return false;
        }
    }

    // If pattern ends with *, match remainder
    if !pattern.ends_with('*') {
        pos == text.len()
    } else {
        true
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Middleware Layer
// ═══════════════════════════════════════════════════════════════════════════════

/// Layer for adding cache middleware.
#[derive(Clone)]
pub struct CacheMiddlewareLayer {
    cache: Option<Arc<Cache>>,
    config: CacheMiddlewareConfig,
}

impl CacheMiddlewareLayer {
    /// Create a new cache middleware layer.
    pub fn new(cache: Arc<Cache>) -> Self {
        Self {
            cache: Some(cache),
            config: CacheMiddlewareConfig::default(),
        }
    }

    /// Create a cache middleware layer with configuration.
    pub fn with_config(cache: Arc<Cache>, config: CacheMiddlewareConfig) -> Self {
        Self {
            cache: Some(cache),
            config,
        }
    }

    /// Create a layer for ETag-only (no response caching).
    pub fn etag_only(config: CacheMiddlewareConfig) -> Self {
        let mut config = config;
        config.enable_caching = false;
        config.enable_etag = true;
        Self {
            cache: None,
            config,
        }
    }
}

impl<S> Layer<S> for CacheMiddlewareLayer {
    type Service = CacheMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CacheMiddleware {
            inner,
            cache: self.cache.clone(),
            config: self.config.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Middleware Service
// ═══════════════════════════════════════════════════════════════════════════════

/// Cache middleware service.
#[derive(Clone)]
pub struct CacheMiddleware<S> {
    inner: S,
    cache: Option<Arc<Cache>>,
    config: CacheMiddlewareConfig,
}

#[allow(dead_code)]
impl<S> CacheMiddleware<S> {
    /// Generate cache key for request.
    fn cache_key(&self, method: &Method, uri: &str, headers: &HeaderMap) -> CacheKey {
        let mut segments = vec![
            method.as_str().to_string(),
            uri.to_string(),
        ];

        // Include vary headers in key
        for header_name in &self.config.vary_headers {
            if let Some(value) = headers.get(header_name.as_str()) {
                if let Ok(v) = value.to_str() {
                    segments.push(format!("{}:{}", header_name, v));
                }
            }
        }

        let hash = crate::cache::key::hash_composite_key(&segments);
        CacheKey::new(KeyType::ApiResponse)
            .with_segment(method.as_str())
            .with_id(hash)
    }

    /// Check if request is cacheable.
    fn is_cacheable_request(&self, method: &Method, path: &str) -> bool {
        if !self.config.enable_caching {
            return false;
        }

        if !self.config.cacheable_methods.contains(method) {
            return false;
        }

        if self.config.is_excluded(path) {
            return false;
        }

        true
    }

    /// Check if response is cacheable.
    fn is_cacheable_response(&self, status: StatusCode) -> bool {
        self.config.cacheable_status_codes.contains(&status)
    }

    /// Build cache headers for response.
    fn build_cache_headers(&self, body: &[u8], existing_etag: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();

        // ETag
        if self.config.enable_etag {
            let etag = existing_etag.map(|e| e.to_string()).unwrap_or_else(|| {
                if self.config.use_weak_etag {
                    ETagGenerator::weak(body)
                } else {
                    ETagGenerator::strong(body)
                }
            });

            if let Ok(value) = HeaderValue::from_str(&etag) {
                headers.insert(header::ETAG, value);
            }
        }

        // Cache-Control
        if self.config.enable_cache_control {
            let cache_control = CacheControl::api_response(self.config.default_max_age);
            headers.insert(header::CACHE_CONTROL, cache_control.to_header_value());
        }

        // Vary
        if !self.config.vary_headers.is_empty() {
            let vary = self.config.vary_headers.join(", ");
            if let Ok(value) = HeaderValue::from_str(&vary) {
                headers.insert(header::VARY, value);
            }
        }

        headers
    }
}

impl<S> Service<Request<Body>> for CacheMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, std::result::Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let cache = self.cache.clone();
        let config = self.config.clone();
        let mut inner = self.inner.clone();

        let method = request.method().clone();
        let uri = request.uri().path().to_string();
        let headers = request.headers().clone();

        // Check for conditional request headers
        let if_none_match = headers
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let is_cacheable = self.is_cacheable_request(&method, &uri);
        let cache_key = self.cache_key(&method, &uri, &headers);
        let config_clone = self.config.clone();

        Box::pin(async move {
            // Try to get from cache
            if is_cacheable {
                if let Some(ref cache) = cache {
                    if let Ok(Some(cached)) = cache.get::<CachedResponse>(&cache_key).await {
                        // Check If-None-Match
                        if let Some(ref inm) = if_none_match {
                            if let Some(ref etag) = cached.etag {
                                if ETagGenerator::matches_any(etag, inm) {
                                    counter!("cache_middleware_not_modified_total").increment(1);
                                    return Ok(not_modified_response(&cached.etag));
                                }
                            }
                        }

                        counter!("cache_middleware_hits_total").increment(1);
                        debug!("Cache hit for {} {}", method, uri);
                        return Ok(cached.into_response());
                    }
                }
            }

            counter!("cache_middleware_misses_total").increment(1);

            // Call inner service
            let response = inner.call(request).await?;

            // Check if response is cacheable
            let status = response.status();
            let is_cacheable_response = config_clone.cacheable_status_codes.contains(&status);

            if !is_cacheable || !is_cacheable_response {
                // Just add headers without caching
                if config.enable_etag || config.enable_cache_control {
                    let (parts, body) = response.into_parts();
                    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();

                    let cache_headers = CacheMiddleware::<S>::build_cache_headers_static(
                        &config,
                        &bytes,
                        None,
                    );

                    let mut response = Response::from_parts(parts, Body::from(bytes));
                    for (key, value) in cache_headers.iter() {
                        response.headers_mut().insert(key.clone(), value.clone());
                    }

                    return Ok(response);
                }

                return Ok(response);
            }

            // Cache the response
            let (parts, body) = response.into_parts();
            let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();

            let etag = if config.use_weak_etag {
                ETagGenerator::weak(&bytes)
            } else {
                ETagGenerator::strong(&bytes)
            };

            // Check If-None-Match before caching
            if let Some(ref inm) = if_none_match {
                if ETagGenerator::matches_any(&etag, inm) {
                    counter!("cache_middleware_not_modified_total").increment(1);
                    return Ok(not_modified_response(&Some(etag)));
                }
            }

            let cached_response = CachedResponse {
                status: parts.status.as_u16(),
                headers: parts.headers.iter()
                    .filter_map(|(k, v)| {
                        v.to_str().ok().map(|v| (k.to_string(), v.to_string()))
                    })
                    .collect(),
                body: bytes.to_vec(),
                etag: Some(etag.clone()),
                created_at: Utc::now(),
            };

            // Store in cache
            if let Some(ref cache) = cache {
                if let Err(e) = cache.set(&cache_key, &cached_response).await {
                    warn!("Failed to cache response: {}", e);
                }
            }

            // Build response with cache headers
            let cache_headers = CacheMiddleware::<S>::build_cache_headers_static(
                &config,
                &bytes,
                Some(&etag),
            );

            let mut response = Response::from_parts(parts, Body::from(bytes));
            for (key, value) in cache_headers.iter() {
                response.headers_mut().insert(key.clone(), value.clone());
            }

            Ok(response)
        })
    }
}

impl<S> CacheMiddleware<S> {
    /// Static version of build_cache_headers for use in async block.
    fn build_cache_headers_static(
        config: &CacheMiddlewareConfig,
        body: &[u8],
        existing_etag: Option<&str>,
    ) -> HeaderMap {
        let mut headers = HeaderMap::new();

        if config.enable_etag {
            let etag = existing_etag.map(|e| e.to_string()).unwrap_or_else(|| {
                if config.use_weak_etag {
                    ETagGenerator::weak(body)
                } else {
                    ETagGenerator::strong(body)
                }
            });

            if let Ok(value) = HeaderValue::from_str(&etag) {
                headers.insert(header::ETAG, value);
            }
        }

        if config.enable_cache_control {
            let cache_control = CacheControl::api_response(config.default_max_age);
            headers.insert(header::CACHE_CONTROL, cache_control.to_header_value());
        }

        if !config.vary_headers.is_empty() {
            let vary = config.vary_headers.join(", ");
            if let Ok(value) = HeaderValue::from_str(&vary) {
                headers.insert(header::VARY, value);
            }
        }

        headers
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cached Response
// ═══════════════════════════════════════════════════════════════════════════════

/// Cached HTTP response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    etag: Option<String>,
    created_at: DateTime<Utc>,
}

impl CachedResponse {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::OK);
        let mut response = Response::builder()
            .status(status);

        for (name, value) in self.headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::try_from(name),
                HeaderValue::from_str(&value),
            ) {
                response = response.header(name, value);
            }
        }

        if let Some(etag) = self.etag {
            if let Ok(value) = HeaderValue::from_str(&etag) {
                response = response.header(header::ETAG, value);
            }
        }

        response
            .header(header::AGE, (Utc::now() - self.created_at).num_seconds().to_string())
            .header("X-Cache", "HIT")
            .body(Body::from(self.body))
            .unwrap_or_else(|_| Response::new(Body::empty()))
    }
}

/// Create a 304 Not Modified response.
fn not_modified_response(etag: &Option<String>) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::NOT_MODIFIED);

    if let Some(ref etag) = etag {
        if let Ok(value) = HeaderValue::from_str(etag) {
            response = response.header(header::ETAG, value);
        }
    }

    response
        .header("X-Cache", "NOT_MODIFIED")
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Axum Middleware Function
// ═══════════════════════════════════════════════════════════════════════════════

/// Axum middleware function for cache headers.
#[instrument(skip_all)]
pub async fn cache_middleware(
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let _path = request.uri().path().to_string();
    let if_none_match = request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let response = next.run(request).await;

    // Only add cache headers for successful GET/HEAD requests
    if !matches!(method, Method::GET | Method::HEAD) {
        return response;
    }

    if !response.status().is_success() {
        return response;
    }

    let (parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();

    let etag = ETagGenerator::strong(&bytes);

    // Check If-None-Match
    if let Some(inm) = if_none_match {
        if ETagGenerator::matches_any(&etag, &inm) {
            return not_modified_response(&Some(etag));
        }
    }

    let cache_control = CacheControl::api_response(Duration::from_secs(60));

    let mut response = Response::from_parts(parts, Body::from(bytes));
    response.headers_mut().insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
    response.headers_mut().insert(header::CACHE_CONTROL, cache_control.to_header_value());
    response.headers_mut().insert(header::VARY, HeaderValue::from_static("Accept, Accept-Encoding, Authorization"));

    response
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_control_builder() {
        let cc = CacheControl::new()
            .public()
            .max_age(Duration::from_secs(3600))
            .immutable();

        let header = cc.build();
        assert!(header.contains("public"));
        assert!(header.contains("max-age=3600"));
        assert!(header.contains("immutable"));
    }

    #[test]
    fn test_cache_control_presets() {
        let static_cc = CacheControl::static_assets();
        let header = static_cc.build();
        assert!(header.contains("public"));
        assert!(header.contains("immutable"));
        assert!(header.contains("max-age=31536000"));

        let no_cache = CacheControl::no_cache();
        let header = no_cache.build();
        assert!(header.contains("no-cache"));
        assert!(header.contains("no-store"));
    }

    #[test]
    fn test_etag_generator() {
        let content = b"test content";
        let etag1 = ETagGenerator::strong(content);
        let etag2 = ETagGenerator::strong(content);
        let etag3 = ETagGenerator::strong(b"different content");

        // Same content should produce same ETag
        assert_eq!(etag1, etag2);

        // Different content should produce different ETag
        assert_ne!(etag1, etag3);

        // Strong ETag format
        assert!(etag1.starts_with('"'));
        assert!(etag1.ends_with('"'));
        assert!(!etag1.starts_with("W/"));
    }

    #[test]
    fn test_weak_etag() {
        let content = b"test content";
        let etag = ETagGenerator::weak(content);

        assert!(etag.starts_with("W/\""));
        assert!(etag.ends_with('"'));
    }

    #[test]
    fn test_etag_matching() {
        assert!(ETagGenerator::matches("\"abc\"", "\"abc\""));
        assert!(ETagGenerator::matches("W/\"abc\"", "\"abc\""));
        assert!(ETagGenerator::matches("\"abc\"", "W/\"abc\""));
        assert!(!ETagGenerator::matches("\"abc\"", "\"xyz\""));
    }

    #[test]
    fn test_etag_matches_any() {
        assert!(ETagGenerator::matches_any("\"abc\"", "\"abc\""));
        assert!(ETagGenerator::matches_any("\"abc\"", "\"xyz\", \"abc\", \"def\""));
        assert!(ETagGenerator::matches_any("\"abc\"", "*"));
        assert!(!ETagGenerator::matches_any("\"abc\"", "\"xyz\", \"def\""));
    }

    #[test]
    fn test_glob_matches() {
        assert!(glob_matches("/health", "/health"));
        assert!(glob_matches("/api/*", "/api/users"));
        assert!(glob_matches("/api/*/stream", "/api/v1/stream"));
        assert!(!glob_matches("/api/*", "/other/path"));
        assert!(!glob_matches("/api/*/stream", "/api/v1/notstream"));
    }

    #[test]
    fn test_config_exclusion() {
        let config = CacheMiddlewareConfig::default();

        assert!(config.is_excluded("/health"));
        assert!(config.is_excluded("/metrics"));
        assert!(!config.is_excluded("/api/users"));
    }

    #[test]
    fn test_cache_directive_to_string() {
        assert_eq!(CacheDirective::MaxAge(3600).to_header_string(), "max-age=3600");
        assert_eq!(CacheDirective::NoCache.to_header_string(), "no-cache");
        assert_eq!(CacheDirective::Private.to_header_string(), "private");
        assert_eq!(CacheDirective::StaleWhileRevalidate(60).to_header_string(), "stale-while-revalidate=60");
    }
}
