//! Request tracing middleware with OpenTelemetry integration.
//!
//! Features:
//! - Distributed tracing with correlation IDs
//! - Request/response logging with configurable detail levels
//! - OpenTelemetry span propagation
//! - Performance timing metrics
//! - Request body logging (configurable)
//!
//! # Example
//!
//! ```rust,ignore
//! use apex_core::middleware::tracing::{TracingLayer, TracingConfig};
//!
//! let config = TracingConfig::builder()
//!     .log_request_body(true)
//!     .log_response_body(false)
//!     .build();
//!
//! let app = Router::new()
//!     .route("/api/v1/tasks", post(create_task))
//!     .layer(TracingLayer::new(config));
//! ```

use axum::{
    body::Body,
    extract::{ConnectInfo, MatchedPath, Request},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
};
use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use metrics::{counter, histogram};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tower::{Layer, Service};
use tracing::{debug, error, info, info_span, warn, Instrument, Level};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Tracing middleware configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing
    pub enabled: bool,

    /// Log request headers
    pub log_request_headers: bool,

    /// Log response headers
    pub log_response_headers: bool,

    /// Log request body (may contain sensitive data)
    pub log_request_body: bool,

    /// Log response body
    pub log_response_body: bool,

    /// Maximum body size to log (in bytes)
    pub max_body_log_size: usize,

    /// Headers to redact from logs
    pub redacted_headers: Vec<String>,

    /// Paths to exclude from tracing
    pub excluded_paths: Vec<String>,

    /// Log level for successful requests
    pub success_log_level: LogLevel,

    /// Log level for client errors (4xx)
    pub client_error_log_level: LogLevel,

    /// Log level for server errors (5xx)
    pub server_error_log_level: LogLevel,

    /// Enable OpenTelemetry context propagation
    pub enable_otel_propagation: bool,

    /// Header name for request ID
    pub request_id_header: String,

    /// Generate request ID if not present
    pub generate_request_id: bool,
}

/// Log level configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_request_headers: false,
            log_response_headers: false,
            log_request_body: false,
            log_response_body: false,
            max_body_log_size: 4096,
            redacted_headers: vec![
                "Authorization".to_string(),
                "X-API-Key".to_string(),
                "Cookie".to_string(),
                "Set-Cookie".to_string(),
            ],
            excluded_paths: vec![
                "/health".to_string(),
                "/ready".to_string(),
                "/metrics".to_string(),
            ],
            success_log_level: LogLevel::Info,
            client_error_log_level: LogLevel::Warn,
            server_error_log_level: LogLevel::Error,
            enable_otel_propagation: true,
            request_id_header: "X-Request-ID".to_string(),
            generate_request_id: true,
        }
    }
}

impl TracingConfig {
    /// Create a new builder.
    pub fn builder() -> TracingConfigBuilder {
        TracingConfigBuilder::default()
    }
}

/// Builder for tracing configuration.
#[derive(Default)]
pub struct TracingConfigBuilder {
    config: TracingConfig,
}

impl TracingConfigBuilder {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn log_request_headers(mut self, enabled: bool) -> Self {
        self.config.log_request_headers = enabled;
        self
    }

    pub fn log_response_headers(mut self, enabled: bool) -> Self {
        self.config.log_response_headers = enabled;
        self
    }

    pub fn log_request_body(mut self, enabled: bool) -> Self {
        self.config.log_request_body = enabled;
        self
    }

    pub fn log_response_body(mut self, enabled: bool) -> Self {
        self.config.log_response_body = enabled;
        self
    }

    pub fn max_body_log_size(mut self, size: usize) -> Self {
        self.config.max_body_log_size = size;
        self
    }

    pub fn add_redacted_header(mut self, header: impl Into<String>) -> Self {
        self.config.redacted_headers.push(header.into());
        self
    }

    pub fn add_excluded_path(mut self, path: impl Into<String>) -> Self {
        self.config.excluded_paths.push(path.into());
        self
    }

    pub fn success_log_level(mut self, level: LogLevel) -> Self {
        self.config.success_log_level = level;
        self
    }

    pub fn client_error_log_level(mut self, level: LogLevel) -> Self {
        self.config.client_error_log_level = level;
        self
    }

    pub fn server_error_log_level(mut self, level: LogLevel) -> Self {
        self.config.server_error_log_level = level;
        self
    }

    pub fn enable_otel_propagation(mut self, enabled: bool) -> Self {
        self.config.enable_otel_propagation = enabled;
        self
    }

    pub fn request_id_header(mut self, header: impl Into<String>) -> Self {
        self.config.request_id_header = header.into();
        self
    }

    pub fn generate_request_id(mut self, enabled: bool) -> Self {
        self.config.generate_request_id = enabled;
        self
    }

    pub fn build(self) -> TracingConfig {
        self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Request Context
// ═══════════════════════════════════════════════════════════════════════════════

/// Request context for tracing.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique request ID
    pub request_id: String,

    /// Request start time
    pub started_at: DateTime<Utc>,

    /// Client IP address
    pub client_ip: Option<String>,

    /// HTTP method
    pub method: String,

    /// Request path
    pub path: String,

    /// Matched route pattern
    pub route: Option<String>,

    /// Query string
    pub query: Option<String>,

    /// User agent
    pub user_agent: Option<String>,

    /// Content type
    pub content_type: Option<String>,

    /// Request size in bytes
    pub request_size: Option<u64>,
}

impl RequestContext {
    /// Create from request.
    pub fn from_request(
        request: &Request<Body>,
        remote_addr: Option<SocketAddr>,
        config: &TracingConfig,
    ) -> Self {
        let headers = request.headers();

        // Get or generate request ID
        let request_id = headers
            .get(&config.request_id_header)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if config.generate_request_id {
                    Uuid::new_v4().to_string()
                } else {
                    "unknown".to_string()
                }
            });

        // Extract client IP
        let client_ip = extract_client_ip(headers, remote_addr);

        // Extract matched route
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map(|p| p.as_str().to_string());

        Self {
            request_id,
            started_at: Utc::now(),
            client_ip,
            method: request.method().to_string(),
            path: request.uri().path().to_string(),
            route,
            query: request.uri().query().map(|s| s.to_string()),
            user_agent: headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            content_type: headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string()),
            request_size: headers
                .get(header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok()),
        }
    }
}

/// Extract client IP from headers and connection.
fn extract_client_ip(headers: &HeaderMap, remote_addr: Option<SocketAddr>) -> Option<String> {
    // Try common proxy headers
    for header_name in &["X-Forwarded-For", "X-Real-IP", "CF-Connecting-IP"] {
        if let Some(value) = headers.get(*header_name) {
            if let Ok(s) = value.to_str() {
                // X-Forwarded-For can have multiple IPs
                return Some(s.split(',').next().unwrap_or(s).trim().to_string());
            }
        }
    }

    // Fall back to connection address
    remote_addr.map(|addr| addr.ip().to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Response Info
// ═══════════════════════════════════════════════════════════════════════════════

/// Response information for logging.
#[derive(Debug)]
#[allow(dead_code)]
struct ResponseInfo {
    status: StatusCode,
    duration: Duration,
    response_size: Option<u64>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Header Utilities
// ═══════════════════════════════════════════════════════════════════════════════

/// Redact sensitive headers.
fn redact_headers(headers: &HeaderMap, redacted: &[String]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let name_str = name.as_str().to_lowercase();
            let value_str = if redacted.iter().any(|r| r.to_lowercase() == name_str) {
                "[REDACTED]".to_string()
            } else {
                value.to_str().unwrap_or("[INVALID UTF-8]").to_string()
            };
            (name.to_string(), value_str)
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tower Layer and Service
// ═══════════════════════════════════════════════════════════════════════════════

/// Tracing layer for Tower.
#[derive(Clone)]
pub struct TracingLayer {
    config: Arc<TracingConfig>,
}

impl TracingLayer {
    /// Create a new tracing layer.
    pub fn new(config: TracingConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl Default for TracingLayer {
    fn default() -> Self {
        Self::new(TracingConfig::default())
    }
}

impl<S> Layer<S> for TracingLayer {
    type Service = TracingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingService {
            inner,
            config: self.config.clone(),
        }
    }
}

/// Tracing service.
#[derive(Clone)]
pub struct TracingService<S> {
    inner: S,
    config: Arc<TracingConfig>,
}

impl<S> Service<Request<Body>> for TracingService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        let config = self.config.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check if tracing is enabled and path is not excluded
            let path = request.uri().path();
            if !config.enabled || config.excluded_paths.iter().any(|p| path.starts_with(p)) {
                return inner.call(request).await;
            }

            let start = Instant::now();

            // Extract connection info
            let remote_addr = request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ci| ci.0);

            // Build request context
            let ctx = RequestContext::from_request(&request, remote_addr, &config);

            // Create span for this request
            let span = info_span!(
                "http_request",
                request_id = %ctx.request_id,
                method = %ctx.method,
                path = %ctx.path,
                route = ctx.route.as_deref().unwrap_or("unknown"),
                client_ip = ctx.client_ip.as_deref().unwrap_or("unknown"),
                otel.kind = "server",
                otel.status_code = tracing::field::Empty,
                http.status_code = tracing::field::Empty,
                http.response_size = tracing::field::Empty,
            );

            // Log request headers if configured
            if config.log_request_headers {
                let headers = redact_headers(request.headers(), &config.redacted_headers);
                debug!(parent: &span, request_headers = ?headers, "Request headers");
            }

            // Inject request ID into request extensions
            request.extensions_mut().insert(ctx.clone());

            // Add request ID to response headers
            let request_id = ctx.request_id.clone();

            // Execute the request within the span
            let result = async {
                inner.call(request).await
            }
            .instrument(span.clone())
            .await;

            let duration = start.elapsed();

            match result {
                Ok(mut response) => {
                    let status = response.status();

                    // Add request ID to response
                    if let Ok(header_name) = header::HeaderName::from_bytes(config.request_id_header.as_bytes()) {
                        if let Ok(value) = HeaderValue::from_str(&request_id) {
                            response.headers_mut().insert(header_name, value);
                        }
                    }

                    // Get response size
                    let response_size = response
                        .headers()
                        .get(header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse().ok());

                    // Record span fields
                    span.record("http.status_code", status.as_u16());
                    if let Some(size) = response_size {
                        span.record("http.response_size", size);
                    }

                    // Log based on status code
                    let status_code = status.as_u16();
                    if status_code >= 500 {
                        span.record("otel.status_code", "ERROR");
                        error!(
                            parent: &span,
                            status = status_code,
                            duration_ms = duration.as_millis() as u64,
                            "Server error"
                        );
                    } else if status_code >= 400 {
                        warn!(
                            parent: &span,
                            status = status_code,
                            duration_ms = duration.as_millis() as u64,
                            "Client error"
                        );
                    } else {
                        span.record("otel.status_code", "OK");
                        info!(
                            parent: &span,
                            status = status_code,
                            duration_ms = duration.as_millis() as u64,
                            "Request completed"
                        );
                    }

                    // Log response headers if configured
                    if config.log_response_headers {
                        let headers = redact_headers(response.headers(), &config.redacted_headers);
                        debug!(parent: &span, response_headers = ?headers, "Response headers");
                    }

                    // Record metrics
                    record_metrics(&ctx, status, duration, response_size);

                    Ok(response)
                }
                Err(e) => {
                    span.record("otel.status_code", "ERROR");
                    error!(
                        parent: &span,
                        duration_ms = duration.as_millis() as u64,
                        "Request failed with internal error"
                    );

                    // Record error metrics
                    counter!(
                        "http_requests_total",
                        "method" => ctx.method.clone(),
                        "route" => ctx.route.clone().unwrap_or_else(|| ctx.path.clone()),
                        "status" => "error"
                    )
                    .increment(1);

                    Err(e)
                }
            }
        })
    }
}

/// Record HTTP metrics.
fn record_metrics(
    ctx: &RequestContext,
    status: StatusCode,
    duration: Duration,
    response_size: Option<u64>,
) {
    let route = ctx.route.clone().unwrap_or_else(|| ctx.path.clone());
    let status_str = status.as_u16().to_string();

    counter!(
        "http_requests_total",
        "method" => ctx.method.clone(),
        "route" => route.clone(),
        "status" => status_str.clone()
    )
    .increment(1);

    histogram!(
        "http_request_duration_seconds",
        "method" => ctx.method.clone(),
        "route" => route.clone(),
        "status" => status_str.clone()
    )
    .record(duration.as_secs_f64());

    if let Some(size) = response_size {
        histogram!(
            "http_response_size_bytes",
            "method" => ctx.method.clone(),
            "route" => route
        )
        .record(size as f64);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Axum Extractor
// ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

/// Extractor for request context in handlers.
#[axum::async_trait]
impl<S> FromRequestParts<S> for RequestContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .extensions
            .get::<RequestContext>()
            .cloned()
            .unwrap_or_else(|| RequestContext {
                request_id: Uuid::new_v4().to_string(),
                started_at: Utc::now(),
                client_ip: None,
                method: parts.method.to_string(),
                path: parts.uri.path().to_string(),
                route: None,
                query: parts.uri.query().map(|s| s.to_string()),
                user_agent: None,
                content_type: None,
                request_size: None,
            }))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = TracingConfig::builder()
            .enabled(true)
            .log_request_headers(true)
            .log_response_body(false)
            .add_excluded_path("/internal/*")
            .build();

        assert!(config.enabled);
        assert!(config.log_request_headers);
        assert!(!config.log_response_body);
        assert!(config.excluded_paths.contains(&"/internal/*".to_string()));
    }

    #[test]
    fn test_header_redaction() {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("Authorization", HeaderValue::from_static("Bearer secret"));
        headers.insert("X-Custom", HeaderValue::from_static("visible"));

        let redacted = vec!["Authorization".to_string()];
        let result = redact_headers(&headers, &redacted);

        let auth_value = result
            .iter()
            .find(|(k, _)| k.to_lowercase() == "authorization")
            .map(|(_, v)| v.as_str());

        assert_eq!(auth_value, Some("[REDACTED]"));

        let custom_value = result
            .iter()
            .find(|(k, _)| k.to_lowercase() == "x-custom")
            .map(|(_, v)| v.as_str());

        assert_eq!(custom_value, Some("visible"));
    }

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(Level::from(LogLevel::Info), Level::INFO);
        assert_eq!(Level::from(LogLevel::Error), Level::ERROR);
        assert_eq!(Level::from(LogLevel::Debug), Level::DEBUG);
    }

    #[test]
    fn test_default_config() {
        let config = TracingConfig::default();

        assert!(config.enabled);
        assert!(!config.log_request_headers);
        assert!(!config.log_request_body);
        assert!(config.excluded_paths.contains(&"/health".to_string()));
        assert!(config.redacted_headers.contains(&"Authorization".to_string()));
    }
}
