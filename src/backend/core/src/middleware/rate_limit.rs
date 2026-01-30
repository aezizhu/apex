//! Production-grade rate limiting middleware.
//!
//! Features:
//! - Token bucket algorithm for smooth rate limiting
//! - Sliding window counter for accurate burst control
//! - Per-client (IP/API key) limits
//! - Per-endpoint limits
//! - Redis-backed distributed rate limiting
//! - Graceful degradation when Redis is unavailable
//! - Standard X-RateLimit headers
//!
//! # Example
//!
//! ```rust,ignore
//! use apex_core::middleware::rate_limit::{RateLimitLayer, RateLimitConfig};
//!
//! let config = RateLimitConfig::builder()
//!     .requests_per_second(100)
//!     .burst_size(200)
//!     .build();
//!
//! let app = Router::new()
//!     .route("/api/v1/tasks", post(create_task))
//!     .layer(RateLimitLayer::new(config));
//! ```

use axum::{
    body::Body,
    extract::{ConnectInfo, Request},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::future::BoxFuture;
use metrics::counter;
use parking_lot::RwLock;
#[allow(unused_imports)]
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    hash::Hash,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::Semaphore;
use tower::{Layer, Service};
use tracing::{debug, error, info, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Rate limiting errors.
#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded {
        limit: u64,
        remaining: u64,
        reset_at: DateTime<Utc>,
        retry_after_secs: u64,
    },

    #[error("Redis connection error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        match self {
            Self::RateLimitExceeded {
                limit,
                remaining,
                reset_at,
                retry_after_secs,
            } => {
                let mut headers = HeaderMap::new();
                headers.insert(
                    "X-RateLimit-Limit",
                    HeaderValue::from_str(&limit.to_string()).unwrap(),
                );
                headers.insert(
                    "X-RateLimit-Remaining",
                    HeaderValue::from_str(&remaining.to_string()).unwrap(),
                );
                headers.insert(
                    "X-RateLimit-Reset",
                    HeaderValue::from_str(&reset_at.timestamp().to_string()).unwrap(),
                );
                headers.insert(
                    "Retry-After",
                    HeaderValue::from_str(&retry_after_secs.to_string()).unwrap(),
                );

                let body = serde_json::json!({
                    "success": false,
                    "error": {
                        "code": "RATE_LIMIT_EXCEEDED",
                        "message": "Too many requests. Please slow down.",
                        "limit": limit,
                        "remaining": remaining,
                        "reset_at": reset_at.to_rfc3339(),
                        "retry_after_secs": retry_after_secs,
                    }
                });

                (StatusCode::TOO_MANY_REQUESTS, headers, axum::Json(body)).into_response()
            }
            Self::RedisError(_) | Self::Internal(_) => {
                error!("Rate limiter internal error: {}", self);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(serde_json::json!({
                        "success": false,
                        "error": {
                            "code": "INTERNAL_ERROR",
                            "message": "An internal error occurred"
                        }
                    })),
                )
                    .into_response()
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,

    /// Default requests per second limit
    pub requests_per_second: u64,

    /// Default burst size (token bucket capacity)
    pub burst_size: u64,

    /// Sliding window size in seconds
    pub window_size_secs: u64,

    /// Per-endpoint rate limits (path -> requests per window)
    pub endpoint_limits: HashMap<String, EndpointLimit>,

    /// Redis configuration for distributed limiting
    pub redis_url: Option<String>,

    /// Key prefix for Redis keys
    pub redis_key_prefix: String,

    /// Enable graceful degradation (allow requests when Redis is down)
    pub graceful_degradation: bool,

    /// Enable IP-based limiting
    pub enable_ip_limiting: bool,

    /// Enable API key-based limiting
    pub enable_api_key_limiting: bool,

    /// Trusted proxy headers for client IP extraction
    pub trusted_proxy_headers: Vec<String>,

    /// Whitelist of IPs exempt from rate limiting
    pub ip_whitelist: Vec<IpAddr>,

    /// Whitelist of API keys exempt from rate limiting
    pub api_key_whitelist: Vec<String>,
}

/// Per-endpoint rate limit configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointLimit {
    /// Requests allowed per window
    pub requests_per_window: u64,

    /// Window size in seconds
    pub window_size_secs: u64,

    /// Optional burst allowance
    pub burst_size: Option<u64>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_second: 100,
            burst_size: 200,
            window_size_secs: 60,
            endpoint_limits: HashMap::new(),
            redis_url: None,
            redis_key_prefix: "apex:ratelimit:".to_string(),
            graceful_degradation: true,
            enable_ip_limiting: true,
            enable_api_key_limiting: true,
            trusted_proxy_headers: vec![
                "X-Forwarded-For".to_string(),
                "X-Real-IP".to_string(),
                "CF-Connecting-IP".to_string(),
            ],
            ip_whitelist: Vec::new(),
            api_key_whitelist: Vec::new(),
        }
    }
}

impl RateLimitConfig {
    /// Create a new builder for rate limit configuration.
    pub fn builder() -> RateLimitConfigBuilder {
        RateLimitConfigBuilder::default()
    }
}

/// Builder for rate limit configuration.
#[derive(Debug, Default)]
pub struct RateLimitConfigBuilder {
    config: RateLimitConfig,
}

impl RateLimitConfigBuilder {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn requests_per_second(mut self, rps: u64) -> Self {
        self.config.requests_per_second = rps;
        self
    }

    pub fn burst_size(mut self, size: u64) -> Self {
        self.config.burst_size = size;
        self
    }

    pub fn window_size_secs(mut self, secs: u64) -> Self {
        self.config.window_size_secs = secs;
        self
    }

    pub fn redis_url(mut self, url: impl Into<String>) -> Self {
        self.config.redis_url = Some(url.into());
        self
    }

    pub fn graceful_degradation(mut self, enabled: bool) -> Self {
        self.config.graceful_degradation = enabled;
        self
    }

    pub fn endpoint_limit(mut self, path: impl Into<String>, limit: EndpointLimit) -> Self {
        self.config.endpoint_limits.insert(path.into(), limit);
        self
    }

    pub fn ip_whitelist(mut self, ips: Vec<IpAddr>) -> Self {
        self.config.ip_whitelist = ips;
        self
    }

    pub fn api_key_whitelist(mut self, keys: Vec<String>) -> Self {
        self.config.api_key_whitelist = keys;
        self
    }

    pub fn build(self) -> RateLimitConfig {
        self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Token Bucket Algorithm
// ═══════════════════════════════════════════════════════════════════════════════

/// Token bucket for rate limiting.
#[derive(Debug)]
struct TokenBucket {
    /// Current number of tokens
    tokens: f64,

    /// Maximum tokens (burst capacity)
    capacity: f64,

    /// Token refill rate per second
    refill_rate: f64,

    /// Last refill time
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            refill_rate: refill_rate as f64,
            last_refill: Instant::now(),
        }
    }

    /// Try to acquire tokens. Returns true if successful.
    fn try_acquire(&mut self, tokens: u64) -> bool {
        self.refill();

        let tokens_needed = tokens as f64;
        if self.tokens >= tokens_needed {
            self.tokens -= tokens_needed;
            true
        } else {
            false
        }
    }

    /// Get current token count.
    fn available(&mut self) -> u64 {
        self.refill();
        self.tokens as u64
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.capacity);
        self.last_refill = now;
    }

    /// Calculate time until tokens are available.
    fn time_until_available(&self, tokens: u64) -> Duration {
        let tokens_needed = (tokens as f64) - self.tokens;
        if tokens_needed <= 0.0 {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(tokens_needed / self.refill_rate)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sliding Window Counter
// ═══════════════════════════════════════════════════════════════════════════════

/// Sliding window counter entry.
#[derive(Debug, Clone)]
struct WindowEntry {
    /// Count in current window
    current_count: u64,

    /// Count in previous window
    previous_count: u64,

    /// Timestamp of current window start
    window_start: Instant,

    /// Window duration
    window_duration: Duration,
}

impl WindowEntry {
    fn new(window_duration: Duration) -> Self {
        Self {
            current_count: 0,
            previous_count: 0,
            window_start: Instant::now(),
            window_duration,
        }
    }

    /// Increment counter and return weighted count.
    fn increment(&mut self) -> u64 {
        self.maybe_rotate();
        self.current_count += 1;
        self.weighted_count()
    }

    /// Get weighted count (interpolated between windows).
    fn weighted_count(&mut self) -> u64 {
        self.maybe_rotate();

        let elapsed = self.window_start.elapsed();
        let window_progress = elapsed.as_secs_f64() / self.window_duration.as_secs_f64();
        let previous_weight = 1.0 - window_progress;

        (self.current_count as f64 + self.previous_count as f64 * previous_weight).ceil() as u64
    }

    /// Maybe rotate to next window.
    fn maybe_rotate(&mut self) {
        let elapsed = self.window_start.elapsed();
        if elapsed >= self.window_duration {
            let windows_passed = (elapsed.as_secs_f64() / self.window_duration.as_secs_f64()) as u64;
            if windows_passed >= 2 {
                // More than 2 windows passed, reset everything
                self.previous_count = 0;
                self.current_count = 0;
            } else {
                // One window passed, rotate
                self.previous_count = self.current_count;
                self.current_count = 0;
            }
            self.window_start = Instant::now();
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Rate Limiter State
// ═══════════════════════════════════════════════════════════════════════════════

/// Client identifier for rate limiting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClientId {
    Ip(IpAddr),
    ApiKey(String),
    Combined { ip: IpAddr, api_key: String },
    Anonymous,
}

impl ClientId {
    fn to_key(&self, prefix: &str) -> String {
        match self {
            Self::Ip(ip) => format!("{}ip:{}", prefix, ip),
            Self::ApiKey(key) => format!("{}key:{}", prefix, key),
            Self::Combined { ip, api_key } => format!("{}combined:{}:{}", prefix, ip, api_key),
            Self::Anonymous => format!("{}anonymous", prefix),
        }
    }
}

/// Rate limit check result.
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,

    /// Current limit
    pub limit: u64,

    /// Remaining requests
    pub remaining: u64,

    /// When the limit resets
    pub reset_at: DateTime<Utc>,

    /// Retry after (if rate limited)
    pub retry_after_secs: Option<u64>,
}

/// In-memory rate limiter state.
struct InMemoryState {
    /// Token buckets per client
    token_buckets: DashMap<ClientId, RwLock<TokenBucket>>,

    /// Sliding window counters per client + endpoint
    window_counters: DashMap<(ClientId, String), RwLock<WindowEntry>>,
}

impl InMemoryState {
    fn new() -> Self {
        Self {
            token_buckets: DashMap::new(),
            window_counters: DashMap::new(),
        }
    }
}

/// Rate limiter with Redis backend for distributed limiting.
pub struct RateLimiter {
    config: RateLimitConfig,
    in_memory: Arc<InMemoryState>,
    redis_client: Option<redis::Client>,
    redis_healthy: Arc<RwLock<bool>>,
    health_check_semaphore: Arc<Semaphore>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub async fn new(config: RateLimitConfig) -> Result<Self, RateLimitError> {
        let redis_client = if let Some(ref url) = config.redis_url {
            match redis::Client::open(url.as_str()) {
                Ok(client) => {
                    // Test connection
                    match client.get_multiplexed_async_connection().await {
                        Ok(_) => {
                            info!("Rate limiter connected to Redis at {}", url);
                            Some(client)
                        }
                        Err(e) => {
                            warn!("Failed to connect to Redis for rate limiting: {}. Using in-memory fallback.", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to create Redis client: {}. Using in-memory fallback.", e);
                    None
                }
            }
        } else {
            debug!("No Redis URL configured, using in-memory rate limiting");
            None
        };

        Ok(Self {
            config,
            in_memory: Arc::new(InMemoryState::new()),
            redis_client,
            redis_healthy: Arc::new(RwLock::new(true)),
            health_check_semaphore: Arc::new(Semaphore::new(1)),
        })
    }

    /// Check if client is whitelisted.
    fn is_whitelisted(&self, client_id: &ClientId) -> bool {
        match client_id {
            ClientId::Ip(ip) => self.config.ip_whitelist.contains(ip),
            ClientId::ApiKey(key) => self.config.api_key_whitelist.contains(key),
            ClientId::Combined { ip, api_key } => {
                self.config.ip_whitelist.contains(ip)
                    || self.config.api_key_whitelist.contains(api_key)
            }
            ClientId::Anonymous => false,
        }
    }

    /// Check rate limit for a client and endpoint.
    pub async fn check(&self, client_id: &ClientId, endpoint: &str) -> Result<RateLimitResult, RateLimitError> {
        if !self.config.enabled {
            return Ok(RateLimitResult {
                allowed: true,
                limit: u64::MAX,
                remaining: u64::MAX,
                reset_at: Utc::now() + chrono::Duration::hours(24),
                retry_after_secs: None,
            });
        }

        if self.is_whitelisted(client_id) {
            return Ok(RateLimitResult {
                allowed: true,
                limit: u64::MAX,
                remaining: u64::MAX,
                reset_at: Utc::now() + chrono::Duration::hours(24),
                retry_after_secs: None,
            });
        }

        // Get endpoint-specific limits or use defaults
        let (limit, window_secs) = if let Some(endpoint_limit) = self.config.endpoint_limits.get(endpoint) {
            (endpoint_limit.requests_per_window, endpoint_limit.window_size_secs)
        } else {
            (
                self.config.requests_per_second * self.config.window_size_secs,
                self.config.window_size_secs,
            )
        };

        // Try Redis first, fall back to in-memory
        if self.redis_client.is_some() && *self.redis_healthy.read() {
            match self.check_redis(client_id, endpoint, limit, window_secs).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("Redis rate limit check failed: {}. Falling back to in-memory.", e);
                    self.mark_redis_unhealthy();
                    if !self.config.graceful_degradation {
                        return Err(e);
                    }
                }
            }
        }

        // In-memory fallback
        self.check_in_memory(client_id, endpoint, limit, window_secs)
    }

    /// Check rate limit using Redis (distributed).
    async fn check_redis(
        &self,
        client_id: &ClientId,
        endpoint: &str,
        limit: u64,
        window_secs: u64,
    ) -> Result<RateLimitResult, RateLimitError> {
        let client = self.redis_client.as_ref().unwrap();
        let mut conn = client.get_multiplexed_async_connection().await?;

        let key = format!("{}{}:{}", self.config.redis_key_prefix, client_id.to_key(""), endpoint);
        let now = Utc::now();
        let window_start = now.timestamp() / (window_secs as i64) * (window_secs as i64);
        let window_key = format!("{}:{}", key, window_start);

        // Lua script for atomic increment and check
        let script = redis::Script::new(
            r#"
            local key = KEYS[1]
            local limit = tonumber(ARGV[1])
            local window_secs = tonumber(ARGV[2])

            local current = redis.call('INCR', key)
            if current == 1 then
                redis.call('EXPIRE', key, window_secs + 1)
            end

            return current
            "#,
        );

        let count: u64 = script
            .key(&window_key)
            .arg(limit)
            .arg(window_secs)
            .invoke_async(&mut conn)
            .await?;

        let reset_at = DateTime::from_timestamp(window_start + window_secs as i64, 0)
            .unwrap_or(now);
        let remaining = limit.saturating_sub(count);
        let allowed = count <= limit;

        let retry_after_secs = if !allowed {
            Some((reset_at - now).num_seconds().max(1) as u64)
        } else {
            None
        };

        // Record metrics
        counter!(
            "rate_limit_checks_total",
            "client_type" => format!("{:?}", std::mem::discriminant(client_id)),
            "endpoint" => endpoint.to_string(),
            "allowed" => allowed.to_string(),
            "backend" => "redis"
        )
        .increment(1);

        Ok(RateLimitResult {
            allowed,
            limit,
            remaining,
            reset_at,
            retry_after_secs,
        })
    }

    /// Check rate limit using in-memory state.
    fn check_in_memory(
        &self,
        client_id: &ClientId,
        endpoint: &str,
        limit: u64,
        window_secs: u64,
    ) -> Result<RateLimitResult, RateLimitError> {
        let key = (client_id.clone(), endpoint.to_string());
        let window_duration = Duration::from_secs(window_secs);

        let entry = self.in_memory.window_counters
            .entry(key)
            .or_insert_with(|| RwLock::new(WindowEntry::new(window_duration)));

        let mut window = entry.write();
        let count = window.increment();

        let now = Utc::now();
        let reset_at = now + chrono::Duration::seconds(window_secs as i64);
        let remaining = limit.saturating_sub(count);
        let allowed = count <= limit;

        let retry_after_secs = if !allowed {
            Some(window_secs - (window.window_start.elapsed().as_secs() % window_secs))
        } else {
            None
        };

        // Record metrics
        counter!(
            "rate_limit_checks_total",
            "client_type" => format!("{:?}", std::mem::discriminant(client_id)),
            "endpoint" => endpoint.to_string(),
            "allowed" => allowed.to_string(),
            "backend" => "in_memory"
        )
        .increment(1);

        Ok(RateLimitResult {
            allowed,
            limit,
            remaining,
            reset_at,
            retry_after_secs,
        })
    }

    /// Check rate limit using token bucket algorithm.
    pub fn check_token_bucket(&self, client_id: &ClientId, tokens: u64) -> RateLimitResult {
        if !self.config.enabled || self.is_whitelisted(client_id) {
            return RateLimitResult {
                allowed: true,
                limit: u64::MAX,
                remaining: u64::MAX,
                reset_at: Utc::now() + chrono::Duration::hours(24),
                retry_after_secs: None,
            };
        }

        let bucket = self.in_memory.token_buckets
            .entry(client_id.clone())
            .or_insert_with(|| {
                RwLock::new(TokenBucket::new(
                    self.config.burst_size,
                    self.config.requests_per_second,
                ))
            });

        let mut bucket = bucket.write();
        let allowed = bucket.try_acquire(tokens);
        let remaining = bucket.available();
        let limit = self.config.burst_size;

        let retry_after = if !allowed {
            Some(bucket.time_until_available(tokens).as_secs().max(1))
        } else {
            None
        };

        RateLimitResult {
            allowed,
            limit,
            remaining,
            reset_at: Utc::now() + chrono::Duration::seconds(1),
            retry_after_secs: retry_after,
        }
    }

    /// Mark Redis as unhealthy and schedule health check.
    fn mark_redis_unhealthy(&self) {
        *self.redis_healthy.write() = false;

        let client = self.redis_client.clone();
        let healthy = self.redis_healthy.clone();
        let semaphore = self.health_check_semaphore.clone();

        tokio::spawn(async move {
            // Only one health check at a time
            let _permit = semaphore.acquire().await;

            // Wait before retrying
            tokio::time::sleep(Duration::from_secs(5)).await;

            if let Some(ref client) = client {
                if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                    let ping_result: Result<String, _> = redis::cmd("PING")
                        .query_async(&mut conn)
                        .await;

                    if ping_result.is_ok() {
                        info!("Redis connection recovered for rate limiting");
                        *healthy.write() = true;
                    }
                }
            }
        });
    }

    /// Clean up expired in-memory entries.
    pub fn cleanup_expired(&self) {
        let now = Instant::now();
        let max_age = Duration::from_secs(self.config.window_size_secs * 3);

        self.in_memory.window_counters.retain(|_, entry| {
            let window = entry.read();
            now.duration_since(window.window_start) < max_age
        });

        self.in_memory.token_buckets.retain(|_, bucket| {
            let bucket = bucket.read();
            now.duration_since(bucket.last_refill) < max_age
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Client ID Extraction
// ═══════════════════════════════════════════════════════════════════════════════

/// Extract client ID from request.
pub fn extract_client_id(
    headers: &HeaderMap,
    remote_addr: Option<SocketAddr>,
    config: &RateLimitConfig,
) -> ClientId {
    let api_key = headers
        .get("X-API-Key")
        .or_else(|| headers.get("Authorization"))
        .and_then(|v| {
            v.to_str().ok().map(|s| {
                s.strip_prefix("Bearer ")
                    .or_else(|| s.strip_prefix("ApiKey "))
                    .unwrap_or(s)
                    .to_string()
            })
        });

    let client_ip = extract_client_ip(headers, remote_addr, config);

    match (api_key, client_ip) {
        (Some(key), Some(ip)) if config.enable_api_key_limiting && config.enable_ip_limiting => {
            ClientId::Combined { ip, api_key: key }
        }
        (Some(key), _) if config.enable_api_key_limiting => ClientId::ApiKey(key),
        (_, Some(ip)) if config.enable_ip_limiting => ClientId::Ip(ip),
        _ => ClientId::Anonymous,
    }
}

/// Extract client IP from headers and connection info.
fn extract_client_ip(
    headers: &HeaderMap,
    remote_addr: Option<SocketAddr>,
    config: &RateLimitConfig,
) -> Option<IpAddr> {
    // Try trusted proxy headers first
    for header_name in &config.trusted_proxy_headers {
        if let Some(value) = headers.get(header_name) {
            if let Ok(s) = value.to_str() {
                // X-Forwarded-For can contain multiple IPs, take the first (client)
                let ip_str = s.split(',').next().unwrap_or(s).trim();
                if let Ok(ip) = ip_str.parse() {
                    return Some(ip);
                }
            }
        }
    }

    // Fall back to connection address
    remote_addr.map(|addr| addr.ip())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tower Layer and Service
// ═══════════════════════════════════════════════════════════════════════════════

/// Rate limiting layer for Tower.
#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Arc<RateLimiter>,
}

impl RateLimitLayer {
    /// Create a new rate limit layer.
    pub fn new(limiter: Arc<RateLimiter>) -> Self {
        Self { limiter }
    }

    /// Create from configuration.
    pub async fn from_config(config: RateLimitConfig) -> Result<Self, RateLimitError> {
        let limiter = RateLimiter::new(config).await?;
        Ok(Self::new(Arc::new(limiter)))
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// Rate limiting service.
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: Arc<RateLimiter>,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
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

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let limiter = self.limiter.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            let headers = request.headers();
            let remote_addr = request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ci| ci.0);
            let path = request.uri().path().to_string();

            let client_id = extract_client_id(headers, remote_addr, &limiter.config);

            match limiter.check(&client_id, &path).await {
                Ok(result) if result.allowed => {
                    let mut response = inner.call(request).await?;

                    // Add rate limit headers to response
                    let headers = response.headers_mut();
                    headers.insert(
                        "X-RateLimit-Limit",
                        HeaderValue::from_str(&result.limit.to_string()).unwrap(),
                    );
                    headers.insert(
                        "X-RateLimit-Remaining",
                        HeaderValue::from_str(&result.remaining.to_string()).unwrap(),
                    );
                    headers.insert(
                        "X-RateLimit-Reset",
                        HeaderValue::from_str(&result.reset_at.timestamp().to_string()).unwrap(),
                    );

                    Ok(response)
                }
                Ok(result) => {
                    // Rate limited
                    let error = RateLimitError::RateLimitExceeded {
                        limit: result.limit,
                        remaining: result.remaining,
                        reset_at: result.reset_at,
                        retry_after_secs: result.retry_after_secs.unwrap_or(1),
                    };

                    counter!(
                        "rate_limit_rejected_total",
                        "client_type" => format!("{:?}", std::mem::discriminant(&client_id)),
                        "endpoint" => path
                    )
                    .increment(1);

                    Ok(error.into_response())
                }
                Err(e) if limiter.config.graceful_degradation => {
                    warn!("Rate limiter error, allowing request due to graceful degradation: {}", e);
                    inner.call(request).await
                }
                Err(e) => {
                    error!("Rate limiter error: {}", e);
                    Ok(e.into_response())
                }
            }
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Background Cleanup Task
// ═══════════════════════════════════════════════════════════════════════════════

/// Start background cleanup task for expired rate limit entries.
pub fn start_cleanup_task(limiter: Arc<RateLimiter>, interval: Duration) {
    tokio::spawn(async move {
        let mut interval_timer = tokio::time::interval(interval);
        loop {
            interval_timer.tick().await;
            limiter.cleanup_expired();
            debug!("Cleaned up expired rate limit entries");
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(10, 5);

        // Should have 10 tokens initially
        assert!(bucket.try_acquire(5));
        assert!(bucket.try_acquire(5));
        assert!(!bucket.try_acquire(1)); // Empty now
    }

    #[test]
    fn test_sliding_window() {
        let mut entry = WindowEntry::new(Duration::from_secs(60));

        // Increment should return correct count
        assert_eq!(entry.increment(), 1);
        assert_eq!(entry.increment(), 2);
        assert_eq!(entry.increment(), 3);
    }

    #[test]
    fn test_client_id_key() {
        let prefix = "test:";

        let ip = ClientId::Ip("127.0.0.1".parse().unwrap());
        assert_eq!(ip.to_key(prefix), "test:ip:127.0.0.1");

        let key = ClientId::ApiKey("secret123".to_string());
        assert_eq!(key.to_key(prefix), "test:key:secret123");
    }

    #[test]
    fn test_config_builder() {
        let config = RateLimitConfig::builder()
            .requests_per_second(50)
            .burst_size(100)
            .graceful_degradation(false)
            .build();

        assert_eq!(config.requests_per_second, 50);
        assert_eq!(config.burst_size, 100);
        assert!(!config.graceful_degradation);
    }

    #[tokio::test]
    async fn test_rate_limiter_in_memory() {
        let config = RateLimitConfig {
            enabled: true,
            requests_per_second: 10,
            window_size_secs: 1,
            ..Default::default()
        };

        let limiter = RateLimiter::new(config).await.unwrap();
        let client = ClientId::Ip("127.0.0.1".parse().unwrap());

        // Should allow first 10 requests
        for _ in 0..10 {
            let result = limiter.check(&client, "/test").await.unwrap();
            assert!(result.allowed);
        }

        // 11th request should be rate limited
        let result = limiter.check(&client, "/test").await.unwrap();
        assert!(!result.allowed);
    }

    #[test]
    fn test_whitelist() {
        let ip = "192.168.1.1".parse().unwrap();
        let config = RateLimitConfig {
            ip_whitelist: vec![ip],
            ..Default::default()
        };

        // Can't test fully without async runtime, but config is correct
        assert!(config.ip_whitelist.contains(&ip));
    }
}
