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
// Disk Space Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// Health checker for disk space.
pub struct DiskSpaceHealthChecker {
    /// Path to check disk space for.
    path: String,
    /// Warning threshold percentage (disk usage above this = degraded).
    warning_threshold_pct: f64,
    /// Critical threshold percentage (disk usage above this = unhealthy).
    critical_threshold_pct: f64,
}

impl DiskSpaceHealthChecker {
    /// Create a new disk space health checker.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            warning_threshold_pct: 80.0,
            critical_threshold_pct: 95.0,
        }
    }

    /// Set warning threshold percentage.
    pub fn with_warning_threshold(mut self, pct: f64) -> Self {
        self.warning_threshold_pct = pct;
        self
    }

    /// Set critical threshold percentage.
    pub fn with_critical_threshold(mut self, pct: f64) -> Self {
        self.critical_threshold_pct = pct;
        self
    }

    /// Get disk usage information.
    fn get_disk_usage(&self) -> std::result::Result<DiskUsageInfo, String> {
        #[cfg(unix)]
        {
            use std::ffi::CString;
            let c_path = CString::new(self.path.as_str())
                .map_err(|e| format\!("Invalid path: {}", e))?;

            unsafe {
                let mut stat: libc::statvfs = std::mem::zeroed();
                if libc::statvfs(c_path.as_ptr(), &mut stat) \!= 0 {
                    return Err(format\!(
                        "statvfs failed for {}: {}",
                        self.path,
                        std::io::Error::last_os_error()
                    ));
                }

                let total = stat.f_blocks as u64 * stat.f_frsize as u64;
                let available = stat.f_bavail as u64 * stat.f_frsize as u64;
                let used = total.saturating_sub(stat.f_bfree as u64 * stat.f_frsize as u64);
                let usage_pct = if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                };

                Ok(DiskUsageInfo {
                    total_bytes: total,
                    used_bytes: used,
                    available_bytes: available,
                    usage_pct,
                })
            }
        }

        #[cfg(not(unix))]
        {
            Err("Disk space check not supported on this platform".to_string())
        }
    }
}

/// Disk usage information.
#[derive(Debug, Clone)]
struct DiskUsageInfo {
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
    usage_pct: f64,
}

#[async_trait]
impl HealthChecker for DiskSpaceHealthChecker {
    fn name(&self) -> &str {
        "disk_space"
    }

    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();

        match self.get_disk_usage() {
            Ok(info) => {
                let status = if info.usage_pct >= self.critical_threshold_pct {
                    HealthStatus::Unhealthy
                } else if info.usage_pct >= self.warning_threshold_pct {
                    HealthStatus::Degraded
                } else {
                    HealthStatus::Healthy
                };

                let message = format\!(
                    "Disk usage: {:.1}% ({} / {} available)",
                    info.usage_pct,
                    format_bytes(info.used_bytes),
                    format_bytes(info.total_bytes)
                );

                ComponentHealth::new_with_status(self.name(), status)
                    .with_message(message)
                    .with_latency(start.elapsed())
                    .with_metadata("path", &self.path)
                    .with_metadata("total_bytes", info.total_bytes)
                    .with_metadata("used_bytes", info.used_bytes)
                    .with_metadata("available_bytes", info.available_bytes)
                    .with_metadata("usage_pct", info.usage_pct)
            }
            Err(e) => {
                warn\!(error = %e, path = %self.path, "Disk space check failed");
                ComponentHealth::unhealthy(self.name())
                    .with_error(e)
                    .with_latency(start.elapsed())
            }
        }
    }
}

/// Format bytes as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format\!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format\!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format\!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format\!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format\!("{} B", bytes)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Memory Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// Health checker for system memory.
pub struct MemoryHealthChecker {
    /// Warning threshold percentage (memory usage above this = degraded).
    warning_threshold_pct: f64,
    /// Critical threshold percentage (memory usage above this = unhealthy).
    critical_threshold_pct: f64,
}

impl MemoryHealthChecker {
    /// Create a new memory health checker.
    pub fn new() -> Self {
        Self {
            warning_threshold_pct: 85.0,
            critical_threshold_pct: 95.0,
        }
    }

    /// Set warning threshold.
    pub fn with_warning_threshold(mut self, pct: f64) -> Self {
        self.warning_threshold_pct = pct;
        self
    }

    /// Set critical threshold.
    pub fn with_critical_threshold(mut self, pct: f64) -> Self {
        self.critical_threshold_pct = pct;
        self
    }

    /// Get process memory usage.
    fn get_process_memory(&self) -> std::result::Result<ProcessMemoryInfo, String> {
        #[cfg(unix)]
        {
            unsafe {
                let mut usage: libc::rusage = std::mem::zeroed();
                if libc::getrusage(libc::RUSAGE_SELF, &mut usage) \!= 0 {
                    return Err(format\!(
                        "getrusage failed: {}",
                        std::io::Error::last_os_error()
                    ));
                }
                // ru_maxrss is in bytes on macOS, kilobytes on Linux
                #[cfg(target_os = "macos")]
                let rss_bytes = usage.ru_maxrss as u64;
                #[cfg(not(target_os = "macos"))]
                let rss_bytes = usage.ru_maxrss as u64 * 1024;

                Ok(ProcessMemoryInfo { rss_bytes })
            }
        }

        #[cfg(not(unix))]
        {
            Err("Memory check not supported on this platform".to_string())
        }
    }

    /// Get total system memory.
    fn get_total_memory(&self) -> std::result::Result<u64, String> {
        #[cfg(target_os = "macos")]
        {
            unsafe {
                let mut size: u64 = 0;
                let mut len = std::mem::size_of::<u64>();
                let mib = [libc::CTL_HW, libc::HW_MEMSIZE];
                if libc::sysctl(
                    mib.as_ptr() as *mut _,
                    2,
                    &mut size as *mut u64 as *mut _,
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                ) \!= 0
                {
                    return Err("sysctl HW_MEMSIZE failed".to_string());
                }
                Ok(size)
            }
        }

        #[cfg(target_os = "linux")]
        {
            use std::fs;
            let meminfo = fs::read_to_string("/proc/meminfo")
                .map_err(|e| format\!("Failed to read /proc/meminfo: {}", e))?;
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let kb: u64 = parts[1]
                            .parse()
                            .map_err(|e| format\!("Failed to parse MemTotal: {}", e))?;
                        return Ok(kb * 1024);
                    }
                }
            }
            Err("MemTotal not found in /proc/meminfo".to_string())
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err("Total memory check not supported on this platform".to_string())
        }
    }
}

/// Process memory information.
#[derive(Debug, Clone)]
struct ProcessMemoryInfo {
    rss_bytes: u64,
}

impl Default for MemoryHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HealthChecker for MemoryHealthChecker {
    fn name(&self) -> &str {
        "memory"
    }

    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();

        let process_mem = match self.get_process_memory() {
            Ok(info) => info,
            Err(e) => {
                return ComponentHealth::unhealthy(self.name())
                    .with_error(e)
                    .with_latency(start.elapsed());
            }
        };

        let mut health = ComponentHealth::healthy(self.name())
            .with_latency(start.elapsed())
            .with_metadata("process_rss_bytes", process_mem.rss_bytes)
            .with_metadata("process_rss_mb", process_mem.rss_bytes / (1024 * 1024));

        // Try to get total memory and compute usage percentage
        if let Ok(total_memory) = self.get_total_memory() {
            let usage_pct = (process_mem.rss_bytes as f64 / total_memory as f64) * 100.0;
            health = health
                .with_metadata("total_memory_bytes", total_memory)
                .with_metadata("memory_usage_pct", usage_pct);

            if usage_pct >= self.critical_threshold_pct {
                health = health
                    .with_status(HealthStatus::Unhealthy)
                    .with_message(format\!(
                        "Process memory usage critical: {:.1}% ({} / {})",
                        usage_pct,
                        format_bytes(process_mem.rss_bytes),
                        format_bytes(total_memory)
                    ));
            } else if usage_pct >= self.warning_threshold_pct {
                health = health
                    .with_status(HealthStatus::Degraded)
                    .with_message(format\!(
                        "Process memory usage high: {:.1}% ({} / {})",
                        usage_pct,
                        format_bytes(process_mem.rss_bytes),
                        format_bytes(total_memory)
                    ));
            } else {
                health = health.with_message(format\!(
                    "Process memory: {} ({:.1}% of {})",
                    format_bytes(process_mem.rss_bytes),
                    usage_pct,
                    format_bytes(total_memory)
                ));
            }
        } else {
            health = health.with_message(format\!(
                "Process RSS: {}",
                format_bytes(process_mem.rss_bytes)
            ));
        }

        health
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Backup Health Checker
// ═══════════════════════════════════════════════════════════════════════════════

/// Health checker for database backup infrastructure.
pub struct BackupHealthChecker {
    pool: PgPool,
}

impl BackupHealthChecker {
    /// Create a new backup health checker.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl HealthChecker for BackupHealthChecker {
    fn name(&self) -> &str {
        "database_backup"
    }

    async fn check(&self) -> ComponentHealth {
        let start = Instant::now();
        let monitor = crate::db::health::DatabaseHealthMonitor::new(
            self.pool.clone(),
            0,
            0,
        );
        let backup_result = monitor.validate_backups().await;

        let status = match &backup_result.status {
            crate::db::health::BackupStatus::Healthy => HealthStatus::Healthy,
            crate::db::health::BackupStatus::Warning => HealthStatus::Degraded,
            crate::db::health::BackupStatus::NotConfigured => HealthStatus::Degraded,
            crate::db::health::BackupStatus::Unknown => HealthStatus::Unhealthy,
        };

        let message = match &backup_result.status {
            crate::db::health::BackupStatus::Healthy => "Backup infrastructure is healthy",
            crate::db::health::BackupStatus::Warning => "Backup has warnings (recent failures detected)",
            crate::db::health::BackupStatus::NotConfigured => "No backup infrastructure detected",
            crate::db::health::BackupStatus::Unknown => "Backup status unknown",
        };

        ComponentHealth::new_with_status(self.name(), status)
            .with_message(message)
            .with_latency(start.elapsed())
            .with_metadata("wal_archiving_enabled", backup_result.wal_archiving_enabled)
            .with_metadata("has_replication_slots", backup_result.has_replication_slots)
            .with_metadata("archived_count", backup_result.archived_count)
            .with_metadata("failed_count", backup_result.failed_count)
            .with_metadata("database_size_bytes", backup_result.database_size_bytes)
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

    #[test]
    fn test_health_check_config_thorough() {
        let config = HealthCheckConfig::thorough();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.include_details);
    }

    #[test]
    fn test_health_check_config_fast() {
        let config = HealthCheckConfig::fast();
        assert_eq!(config.timeout, Duration::from_secs(2));
    }

    #[test]
    fn test_health_check_config_default() {
        let config = HealthCheckConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_composite_checker_all_healthy() {
        struct HealthyChecker;
        #[async_trait]
        impl HealthChecker for HealthyChecker {
            fn name(&self) -> &str { "healthy" }
            async fn check(&self) -> ComponentHealth {
                ComponentHealth::healthy("healthy")
            }
        }

        let composite = CompositeHealthChecker::new()
            .add_checker(Arc::new(HealthyChecker))
            .add_checker(Arc::new(HealthyChecker));

        let status = composite.check_combined().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[tokio::test]
    async fn test_composite_checker_check_all_returns_components() {
        struct NamedChecker(&'static str);
        #[async_trait]
        impl HealthChecker for NamedChecker {
            fn name(&self) -> &str { self.0 }
            async fn check(&self) -> ComponentHealth {
                ComponentHealth::healthy(self.0)
            }
        }

        let composite = CompositeHealthChecker::new()
            .add_checker(Arc::new(NamedChecker("a")))
            .add_checker(Arc::new(NamedChecker("b")));

        let results = composite.check_all().await;
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_composite_checker_empty() {
        let composite = CompositeHealthChecker::new();
        let status = composite.check_combined().await;
        assert_eq!(status, HealthStatus::Healthy);
    }

    #[test]
    fn test_composite_checker_default() {
        let composite = CompositeHealthChecker::default();
        assert!(composite.checkers.is_empty());
    }

    #[tokio::test]
    async fn test_worker_checker_name() {
        let checker = WorkerHealthChecker::new(|| WorkerPoolStats {
            name: "test".into(),
            max_workers: 10,
            available_permits: 10,
            active_workers: 0,
            tasks_submitted: 0,
            tasks_succeeded: 0,
            tasks_failed: 0,
            tasks_unknown: 0,
            acquire_timeouts: 0,
            peak_concurrent: 0,
            avg_wait_time_us: 0,
            avg_exec_time_us: 0,
            uptime_secs: 0,
        });
        assert_eq!(checker.name(), "workers");
    }

    #[tokio::test]
    async fn test_worker_checker_high_failure_rate() {
        let checker = WorkerHealthChecker::new(|| WorkerPoolStats {
            name: "test".into(),
            max_workers: 10,
            available_permits: 5,
            active_workers: 5,
            tasks_submitted: 100,
            tasks_succeeded: 20,
            tasks_failed: 80,
            tasks_unknown: 0,
            acquire_timeouts: 0,
            peak_concurrent: 5,
            avg_wait_time_us: 100,
            avg_exec_time_us: 1000,
            uptime_secs: 3600,
        })
        .with_failure_rate_threshold(50.0);

        let health = checker.check().await;
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_worker_checker_heartbeat() {
        let checker = WorkerHealthChecker::new(|| WorkerPoolStats {
            name: "test".into(),
            max_workers: 4,
            available_permits: 4,
            active_workers: 0,
            tasks_submitted: 0,
            tasks_succeeded: 0,
            tasks_failed: 0,
            tasks_unknown: 0,
            acquire_timeouts: 0,
            peak_concurrent: 0,
            avg_wait_time_us: 0,
            avg_exec_time_us: 0,
            uptime_secs: 100,
        });
        // record_heartbeat should not panic
        checker.record_heartbeat();
        let health = checker.check().await;
        assert!(health.is_healthy());
    }

    #[test]
    fn test_disk_space_checker_creation() {
        let checker = DiskSpaceHealthChecker::new("/tmp")
            .with_warning_threshold(70.0)
            .with_critical_threshold(90.0);
        assert_eq!(checker.path, "/tmp");
        assert_eq!(checker.warning_threshold_pct, 70.0);
        assert_eq!(checker.critical_threshold_pct, 90.0);
    }

    #[tokio::test]
    async fn test_disk_space_checker_name() {
        let checker = DiskSpaceHealthChecker::new("/");
        assert_eq!(checker.name(), "disk_space");
    }

    #[test]
    fn test_memory_checker_creation() {
        let checker = MemoryHealthChecker::new()
            .with_warning_threshold(80.0)
            .with_critical_threshold(95.0);
        assert_eq!(checker.warning_threshold_pct, 80.0);
        assert_eq!(checker.critical_threshold_pct, 95.0);
    }

    #[test]
    fn test_memory_checker_default() {
        let checker = MemoryHealthChecker::default();
        assert_eq!(checker.warning_threshold_pct, 85.0);
        assert_eq!(checker.critical_threshold_pct, 95.0);
    }

    #[tokio::test]
    async fn test_memory_checker_name() {
        let checker = MemoryHealthChecker::new();
        assert_eq!(checker.name(), "memory");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(1024u64 * 1024 * 1024 * 1024), "1.0 TB");
    }

    #[test]
    fn test_format_bytes_fractional() {
        assert_eq!(format_bytes(1536), "1.5 KB");
    }

    #[test]
    fn test_external_api_checker_creation() {
        let checker = ExternalApiHealthChecker::new("my-api", "http://localhost/health")
            .with_expected_status(vec![200, 204])
            .with_failure_threshold(5);
        assert_eq!(checker.api_name, "my-api");
        assert_eq!(checker.health_url, "http://localhost/health");
        assert_eq!(checker.failure_threshold, 5);
    }
}
