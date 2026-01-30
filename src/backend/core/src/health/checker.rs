//! Health checker implementations for various system components.
//!
//! This module provides health checkers for:
//! - **Database**: PostgreSQL connection and query health
//! - **Redis**: Cache connection and memory health
//! - **Workers**: Worker pool health and heartbeat
//! - **External APIs**: External service availability
//!
//! # Example
//!
//! ```rust,ignore
//! use apex_core::health::{HealthChecker, DatabaseHealthChecker, HealthCheckConfig};
//!
//! let db_checker = DatabaseHealthChecker::new(pool.clone());
//! let health = db_checker.check().await;
//! ```

use async_trait::async_trait;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

use super::check::{ComponentHealth, HealthStatus};
use crate::orchestrator::WorkerPoolStats;

// ═══════════════════════════════════════════════════════════════════════════════
// Health Check Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for health checks.
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Timeout for health checks
    pub timeout: Duration,
    /// Latency threshold for degraded status (milliseconds)
    pub latency_threshold_ms: u64,
    /// Enable detailed checks
    pub detailed: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            latency_threshold_ms: 100,
            detailed: true,
        }
    }
}

impl HealthCheckConfig {
    /// Create a fast check configuration (shorter timeout).
    pub fn fast() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            latency_threshold_ms: 50,
            detailed: false,
        }
    }

    /// Create a thorough check configuration.
    pub fn thorough() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            latency_threshold_ms: 200,
            detailed: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Health Checker Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for health checkers.
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// The component name.
    fn name(&self) -> &str;

    /// Perform a health check.
    async fn check(&self) -> ComponentHealth;

    /// Perform a health check with configuration.
    async fn check_with_config(&self, config: &HealthCheckConfig) -> ComponentHealth {
        let start = Instant::now();
        let timeout = config.timeout;

        match tokio::time::timeout(timeout, self.check()).await {
            Ok(mut health) => {
                health.latency_ms = Some(start.elapsed().as_millis() as u64);
                health.check_latency_threshold(config.latency_threshold_ms);
                health
            }
            Err(_) => ComponentHealth::unhealthy(self.name())
                .with_error(format!("Health check timed out after {:?}", timeout))
                .with_latency(start.elapsed()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Database Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// Health checker for PostgreSQL database.
#[allow(dead_code)]
pub struct DatabaseHealthChecker {
    pool: PgPool,
    config: HealthCheckConfig,
}

impl DatabaseHealthChecker {
    /// Create a new database health checker.
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            config: HealthCheckConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(pool: PgPool, config: HealthCheckConfig) -> Self {
        Self { pool, config }
    }

    /// Check connection pool health.
    async fn check_pool(&self) -> Result<(), String> {
        let pool_size = self.pool.size();
        let idle = self.pool.num_idle();

        if pool_size == 0 {
            return Err("No connections in pool".to_string());
        }

        debug!(
            pool_size = pool_size,
            idle_connections = idle,
            "Database pool status"
        );

        Ok(())
    }

    /// Execute a simple query to verify connectivity.
    async fn check_query(&self) -> Result<(), String> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Query failed: {}", e))?;
        Ok(())
    }
}

#[async_trait]
impl HealthChecker for DatabaseHealthChecker {
    fn name(&self) -> &str {
        "database"
    }

    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();

        // Check pool first
        if let Err(e) = self.check_pool().await {
            return ComponentHealth::unhealthy(self.name())
                .with_error(e)
                .with_latency(start.elapsed());
        }

        // Execute test query
        match self.check_query().await {
            Ok(()) => {
                let latency = start.elapsed();
                let pool_size = self.pool.size();
                let idle = self.pool.num_idle();

                ComponentHealth::healthy(self.name())
                    .with_message("Connected to PostgreSQL")
                    .with_latency(latency)
                    .with_metadata("pool_size", pool_size)
                    .with_metadata("idle_connections", idle)
            }
            Err(e) => {
                error!(error = %e, "Database health check failed");
                ComponentHealth::unhealthy(self.name())
                    .with_error(e)
                    .with_latency(start.elapsed())
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Redis Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// Health checker for Redis cache.
pub struct RedisHealthChecker {
    client: redis::Client,
    config: HealthCheckConfig,
    /// Memory usage threshold for degraded status (percentage)
    memory_threshold_pct: f64,
}

impl RedisHealthChecker {
    /// Create a new Redis health checker.
    pub fn new(client: redis::Client) -> Self {
        Self {
            client,
            config: HealthCheckConfig::default(),
            memory_threshold_pct: 80.0,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(client: redis::Client, config: HealthCheckConfig) -> Self {
        Self {
            client,
            config,
            memory_threshold_pct: 80.0,
        }
    }

    /// Set memory threshold percentage.
    pub fn with_memory_threshold(mut self, threshold_pct: f64) -> Self {
        self.memory_threshold_pct = threshold_pct;
        self
    }

    /// Get a connection.
    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, String> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| format!("Failed to connect: {}", e))
    }

    /// Ping Redis.
    async fn ping(&self, conn: &mut redis::aio::MultiplexedConnection) -> Result<(), String> {
        let pong: String = redis::cmd("PING")
            .query_async(conn)
            .await
            .map_err(|e| format!("PING failed: {}", e))?;

        if pong != "PONG" {
            return Err(format!("Unexpected PING response: {}", pong));
        }

        Ok(())
    }

    /// Check memory usage.
    async fn check_memory(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
    ) -> Result<RedisMemoryInfo, String> {
        let info: String = redis::cmd("INFO")
            .arg("memory")
            .query_async(conn)
            .await
            .map_err(|e| format!("INFO failed: {}", e))?;

        let mut used_memory: u64 = 0;
        let mut max_memory: u64 = 0;

        for line in info.lines() {
            if line.starts_with("used_memory:") {
                used_memory = line
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
            if line.starts_with("maxmemory:") {
                max_memory = line
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }

        Ok(RedisMemoryInfo {
            used_bytes: used_memory,
            max_bytes: max_memory,
        })
    }
}

/// Redis memory information.
#[derive(Debug, Clone)]
struct RedisMemoryInfo {
    used_bytes: u64,
    max_bytes: u64,
}

impl RedisMemoryInfo {
    fn usage_percentage(&self) -> Option<f64> {
        if self.max_bytes > 0 {
            Some((self.used_bytes as f64 / self.max_bytes as f64) * 100.0)
        } else {
            None
        }
    }
}

#[async_trait]
impl HealthChecker for RedisHealthChecker {
    fn name(&self) -> &str {
        "redis"
    }

    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();

        // Get connection
        let mut conn = match self.get_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "Redis connection failed");
                return ComponentHealth::unhealthy(self.name())
                    .with_error(e)
                    .with_latency(start.elapsed());
            }
        };

        // Ping
        if let Err(e) = self.ping(&mut conn).await {
            return ComponentHealth::unhealthy(self.name())
                .with_error(e)
                .with_latency(start.elapsed());
        }

        // Check memory if detailed checks enabled
        let mut health = ComponentHealth::healthy(self.name())
            .with_message("Redis is responding")
            .with_latency(start.elapsed());

        if self.config.detailed {
            match self.check_memory(&mut conn).await {
                Ok(mem_info) => {
                    health = health.with_metadata("used_memory_bytes", mem_info.used_bytes);

                    if let Some(usage_pct) = mem_info.usage_percentage() {
                        health = health.with_metadata("memory_usage_pct", usage_pct);

                        if usage_pct > self.memory_threshold_pct {
                            health = health
                                .with_status(HealthStatus::Degraded)
                                .with_message(format!(
                                    "Memory usage high: {:.1}% (threshold: {:.1}%)",
                                    usage_pct, self.memory_threshold_pct
                                ));
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to get Redis memory info");
                    // Not fatal, just skip memory info
                }
            }
        }

        health
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Worker Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// Health checker for worker pools.
#[allow(dead_code)]
pub struct WorkerHealthChecker {
    /// Function to get worker stats
    get_stats: Box<dyn Fn() -> WorkerPoolStats + Send + Sync>,
    /// Configuration
    config: HealthCheckConfig,
    /// Utilization threshold for degraded status
    utilization_threshold_pct: f64,
    /// Failure rate threshold for degraded status
    failure_rate_threshold_pct: f64,
    /// Last heartbeat time
    last_heartbeat: Arc<RwLock<Option<Instant>>>,
    /// Heartbeat timeout
    heartbeat_timeout: Duration,
}

impl WorkerHealthChecker {
    /// Create a new worker health checker.
    pub fn new<F>(get_stats: F) -> Self
    where
        F: Fn() -> WorkerPoolStats + Send + Sync + 'static,
    {
        Self {
            get_stats: Box::new(get_stats),
            config: HealthCheckConfig::default(),
            utilization_threshold_pct: 90.0,
            failure_rate_threshold_pct: 50.0,
            last_heartbeat: Arc::new(RwLock::new(Some(Instant::now()))),
            heartbeat_timeout: Duration::from_secs(60),
        }
    }

    /// Set utilization threshold.
    pub fn with_utilization_threshold(mut self, threshold_pct: f64) -> Self {
        self.utilization_threshold_pct = threshold_pct;
        self
    }

    /// Set failure rate threshold.
    pub fn with_failure_rate_threshold(mut self, threshold_pct: f64) -> Self {
        self.failure_rate_threshold_pct = threshold_pct;
        self
    }

    /// Set heartbeat timeout.
    pub fn with_heartbeat_timeout(mut self, timeout: Duration) -> Self {
        self.heartbeat_timeout = timeout;
        self
    }

    /// Record a heartbeat.
    pub async fn heartbeat(&self) {
        *self.last_heartbeat.write().await = Some(Instant::now());
    }

    /// Check heartbeat status.
    async fn check_heartbeat(&self) -> Result<(), String> {
        let heartbeat = self.last_heartbeat.read().await;
        if let Some(last) = *heartbeat {
            if last.elapsed() > self.heartbeat_timeout {
                return Err(format!(
                    "No heartbeat for {:?} (timeout: {:?})",
                    last.elapsed(),
                    self.heartbeat_timeout
                ));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl HealthChecker for WorkerHealthChecker {
    fn name(&self) -> &str {
        "workers"
    }

    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();

        // Get stats
        let stats = (self.get_stats)();

        // Check heartbeat
        if let Err(e) = self.check_heartbeat().await {
            return ComponentHealth::unhealthy(self.name())
                .with_error(e)
                .with_latency(start.elapsed());
        }

        let utilization = stats.utilization();
        let success_rate = stats.success_rate();
        let failure_rate = 100.0 - success_rate;

        let mut health = ComponentHealth::healthy(self.name())
            .with_latency(start.elapsed())
            .with_metadata("max_workers", stats.max_workers)
            .with_metadata("active_workers", stats.active_workers)
            .with_metadata("available_permits", stats.available_permits)
            .with_metadata("utilization_pct", utilization)
            .with_metadata("success_rate_pct", success_rate)
            .with_metadata("tasks_succeeded", stats.tasks_succeeded)
            .with_metadata("tasks_failed", stats.tasks_failed);

        // Check utilization
        if utilization > self.utilization_threshold_pct {
            health = health
                .with_status(HealthStatus::Degraded)
                .with_message(format!(
                    "Worker pool utilization high: {:.1}% (threshold: {:.1}%)",
                    utilization, self.utilization_threshold_pct
                ));
        }

        // Check failure rate
        if failure_rate > self.failure_rate_threshold_pct {
            health = health
                .with_status(HealthStatus::Unhealthy)
                .with_error(format!(
                    "Worker pool failure rate high: {:.1}% (threshold: {:.1}%)",
                    failure_rate, self.failure_rate_threshold_pct
                ));
        }

        health
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// External API Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// Health checker for external APIs.
#[allow(dead_code)]
pub struct ExternalApiHealthChecker {
    /// HTTP client
    client: reqwest::Client,
    /// Name of the external API
    api_name: String,
    /// Health check URL
    health_url: String,
    /// Expected status codes
    expected_status: Vec<u16>,
    /// Configuration
    config: HealthCheckConfig,
    /// Last successful check
    last_success: Arc<AtomicU64>,
    /// Consecutive failures
    consecutive_failures: Arc<AtomicU64>,
    /// Failure threshold for unhealthy
    failure_threshold: u64,
}

impl ExternalApiHealthChecker {
    /// Create a new external API health checker.
    pub fn new(api_name: impl Into<String>, health_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_name: api_name.into(),
            health_url: health_url.into(),
            expected_status: vec![200, 204],
            config: HealthCheckConfig::default(),
            last_success: Arc::new(AtomicU64::new(0)),
            consecutive_failures: Arc::new(AtomicU64::new(0)),
            failure_threshold: 3,
        }
    }

    /// Set expected status codes.
    pub fn with_expected_status(mut self, codes: Vec<u16>) -> Self {
        self.expected_status = codes;
        self
    }

    /// Set custom HTTP client.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Set failure threshold.
    pub fn with_failure_threshold(mut self, threshold: u64) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Record success.
    fn record_success(&self) {
        self.last_success
            .store(Utc::now().timestamp() as u64, Ordering::Relaxed);
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// Record failure.
    fn record_failure(&self) {
        self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl HealthChecker for ExternalApiHealthChecker {
    fn name(&self) -> &str {
        &self.api_name
    }

    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();

        let result = self.client.get(&self.health_url).send().await;

        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                let latency = start.elapsed();

                if self.expected_status.contains(&status) {
                    self.record_success();

                    ComponentHealth::healthy(&self.api_name)
                        .with_message(format!("{} is reachable", self.api_name))
                        .with_latency(latency)
                        .with_metadata("http_status", status)
                        .with_metadata("url", &self.health_url)
                } else {
                    self.record_failure();
                    let failures = self.consecutive_failures.load(Ordering::Relaxed);

                    let status = if failures >= self.failure_threshold {
                        HealthStatus::Unhealthy
                    } else {
                        HealthStatus::Degraded
                    };

                    ComponentHealth::new_with_status(&self.api_name, status)
                        .with_message(format!(
                            "Unexpected status: {} (expected {:?})",
                            status, self.expected_status
                        ))
                        .with_latency(latency)
                        .with_metadata("http_status", status)
                        .with_metadata("consecutive_failures", failures)
                }
            }
            Err(e) => {
                self.record_failure();
                let failures = self.consecutive_failures.load(Ordering::Relaxed);

                error!(
                    api = %self.api_name,
                    url = %self.health_url,
                    error = %e,
                    failures = failures,
                    "External API health check failed"
                );

                let status = if failures >= self.failure_threshold {
                    HealthStatus::Unhealthy
                } else {
                    HealthStatus::Degraded
                };

                ComponentHealth::new_with_status(&self.api_name, status)
                    .with_error(format!("Request failed: {}", e))
                    .with_latency(start.elapsed())
                    .with_metadata("consecutive_failures", failures)
            }
        }
    }
}

// Helper for ComponentHealth
impl ComponentHealth {
    fn new_with_status(name: impl Into<String>, status: HealthStatus) -> Self {
        Self {
            name: name.into(),
            status,
            message: None,
            latency_ms: None,
            checked_at: Utc::now(),
            metadata: std::collections::HashMap::new(),
            error: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Composite Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// A composite health checker that runs multiple checks.
pub struct CompositeHealthChecker {
    checkers: Vec<Arc<dyn HealthChecker>>,
    config: HealthCheckConfig,
}

impl CompositeHealthChecker {
    /// Create a new composite health checker.
    pub fn new() -> Self {
        Self {
            checkers: Vec::new(),
            config: HealthCheckConfig::default(),
        }
    }

    /// Add a health checker.
    pub fn add_checker(mut self, checker: Arc<dyn HealthChecker>) -> Self {
        self.checkers.push(checker);
        self
    }

    /// Set configuration.
    pub fn with_config(mut self, config: HealthCheckConfig) -> Self {
        self.config = config;
        self
    }

    /// Run all health checks concurrently.
    pub async fn check_all(&self) -> Vec<ComponentHealth> {
        let futures: Vec<_> = self
            .checkers
            .iter()
            .map(|checker| {
                let checker = checker.clone();
                let config = self.config.clone();
                async move { checker.check_with_config(&config).await }
            })
            .collect();

        futures::future::join_all(futures).await
    }

    /// Run all health checks and return a combined status.
    pub async fn check_combined(&self) -> HealthStatus {
        let results = self.check_all().await;
        results
            .into_iter()
            .fold(HealthStatus::Healthy, |acc, r| acc.combine(r.status))
    }
}

impl Default for CompositeHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_config() {
        let default_config = HealthCheckConfig::default();
        assert_eq!(default_config.timeout, Duration::from_secs(5));

        let fast_config = HealthCheckConfig::fast();
        assert_eq!(fast_config.timeout, Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_worker_health_checker() {
        let checker = WorkerHealthChecker::new(|| WorkerPoolStats {
            name: "test".to_string(),
            max_workers: 10,
            available_permits: 5,
            active_workers: 5,
            tasks_submitted: 100,
            tasks_succeeded: 90,
            tasks_failed: 10,
            tasks_unknown: 0,
            acquire_timeouts: 0,
            peak_concurrent: 8,
            avg_wait_time_us: 100,
            avg_exec_time_us: 5000,
            uptime_secs: 3600,
        });

        let health = checker.check().await;
        assert_eq!(health.name, "workers");
        // 50% utilization should be healthy
        assert!(health.is_healthy());
    }

    #[tokio::test]
    async fn test_worker_health_checker_degraded() {
        let checker = WorkerHealthChecker::new(|| WorkerPoolStats {
            name: "test".to_string(),
            max_workers: 10,
            available_permits: 0, // Full utilization
            active_workers: 10,
            tasks_submitted: 100,
            tasks_succeeded: 90,
            tasks_failed: 10,
            tasks_unknown: 0,
            acquire_timeouts: 0,
            peak_concurrent: 10,
            avg_wait_time_us: 100,
            avg_exec_time_us: 5000,
            uptime_secs: 3600,
        })
        .with_utilization_threshold(90.0);

        let health = checker.check().await;
        // 100% utilization should be degraded
        assert_eq!(health.status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn test_external_api_checker_failure_tracking() {
        let checker = ExternalApiHealthChecker::new("test-api", "http://localhost:99999/health")
            .with_failure_threshold(2);

        // First failure should be unhealthy
        let health = checker.check().await;
        assert_eq!(health.status, HealthStatus::Unhealthy);

        // Second failure should also be unhealthy
        let health = checker.check().await;
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_composite_checker() {
        struct MockChecker {
            name: &'static str,
            status: HealthStatus,
        }

        #[async_trait]
        impl HealthChecker for MockChecker {
            fn name(&self) -> &str {
                self.name
            }

            async fn check(&self) -> ComponentHealth {
                ComponentHealth::new_with_status(self.name, self.status)
            }
        }

        let composite = CompositeHealthChecker::new()
            .add_checker(Arc::new(MockChecker {
                name: "healthy",
                status: HealthStatus::Healthy,
            }))
            .add_checker(Arc::new(MockChecker {
                name: "degraded",
                status: HealthStatus::Degraded,
            }));

        let status = composite.check_combined().await;
        assert_eq!(status, HealthStatus::Degraded);
    }
}
