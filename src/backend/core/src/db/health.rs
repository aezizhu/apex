//! Database health monitoring, migration validation, connection pool metrics,
//! slow query logging, and backup validation.
//!
//! This module extends the Database with health-oriented capabilities:
//! - Connection pool metrics and monitoring
//! - Migration validation on startup
//! - Slow query detection and logging
//! - Database backup infrastructure validation
//! - Graceful connection failure handling

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::error::{ApexError, Result};

// ═══════════════════════════════════════════════════════════════════════════════
// Slow Query Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for slow query logging.
#[derive(Debug, Clone)]
pub struct SlowQueryConfig {
    /// Threshold in milliseconds above which queries are logged as slow.
    pub threshold_ms: u64,
    /// Whether slow query logging is enabled.
    pub enabled: bool,
}

impl Default for SlowQueryConfig {
    fn default() -> Self {
        Self {
            threshold_ms: 200,
            enabled: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Connection Pool Metrics
// ═══════════════════════════════════════════════════════════════════════════════

/// Metrics collected from the database connection pool.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionPoolMetrics {
    /// Total connections in the pool.
    pub pool_size: u32,
    /// Number of idle connections.
    pub idle_connections: u32,
    /// Number of active (in-use) connections.
    pub active_connections: u32,
    /// Maximum configured pool size.
    pub max_connections: u32,
    /// Minimum configured pool size.
    pub min_connections: u32,
    /// Pool utilization percentage.
    pub utilization_pct: f64,
    /// Total queries executed (tracked).
    pub total_queries: u64,
    /// Total slow queries detected.
    pub slow_queries: u64,
    /// Average query latency in microseconds.
    pub avg_query_latency_us: u64,
}

/// Internal counters for pool monitoring.
#[derive(Debug, Default)]
pub struct PoolCounters {
    pub total_queries: AtomicU64,
    pub slow_queries: AtomicU64,
    pub total_query_time_us: AtomicU64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Migration Validation
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of migration validation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MigrationValidationResult {
    /// Whether migrations are up to date.
    pub is_current: bool,
    /// Number of applied migrations.
    pub applied_count: usize,
    /// Number of pending migrations.
    pub pending_count: usize,
    /// List of applied migration descriptions.
    pub applied_migrations: Vec<String>,
    /// List of pending migration descriptions.
    pub pending_migrations: Vec<String>,
    /// Validation timestamp.
    pub validated_at: DateTime<Utc>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Backup Validation
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of database backup validation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupValidationResult {
    /// Whether WAL archiving is enabled.
    pub wal_archiving_enabled: bool,
    /// Number of successful WAL archives.
    pub archived_count: Option<i64>,
    /// Number of failed WAL archives.
    pub failed_count: Option<i64>,
    /// Last archive time.
    pub last_archived_at: Option<DateTime<Utc>>,
    /// Last failed archive time.
    pub last_failed_at: Option<DateTime<Utc>>,
    /// Database size in bytes.
    pub database_size_bytes: Option<i64>,
    /// Whether replication slots are configured.
    pub has_replication_slots: bool,
    /// Overall backup health status.
    pub status: BackupStatus,
    /// Validation timestamp.
    pub validated_at: DateTime<Utc>,
}

/// Backup health status.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum BackupStatus {
    /// Backup infrastructure looks healthy.
    Healthy,
    /// Backup has warnings (e.g., recent failures).
    Warning,
    /// No backup infrastructure detected.
    NotConfigured,
    /// Backup check could not complete.
    Unknown,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Database Health Monitor
// ═══════════════════════════════════════════════════════════════════════════════

/// Database health monitor providing connection pool metrics, migration
/// validation, slow query logging, and backup validation.
///
/// This is a companion to `Database` that adds health-oriented capabilities
/// without modifying the core database struct.
#[derive(Clone)]
pub struct DatabaseHealthMonitor {
    pool: PgPool,
    slow_query_config: SlowQueryConfig,
    counters: Arc<PoolCounters>,
    max_connections: u32,
    min_connections: u32,
    started_at: Instant,
}

impl DatabaseHealthMonitor {
    /// Create a new health monitor for the given pool.
    pub fn new(pool: PgPool, max_connections: u32, min_connections: u32) -> Self {
        Self {
            pool,
            slow_query_config: SlowQueryConfig::default(),
            counters: Arc::new(PoolCounters::default()),
            max_connections,
            min_connections,
            started_at: Instant::now(),
        }
    }

    /// Create with custom slow query configuration.
    pub fn with_slow_query_config(mut self, config: SlowQueryConfig) -> Self {
        self.slow_query_config = config;
        self
    }

    /// Run migrations with logging.
    pub async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations...");
        let start = Instant::now();
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                error!(error = %e, "Database migration failed");
                ApexError::from(sqlx::Error::Migrate(Box::new(e)))
            })?;
        info!(
            duration_ms = start.elapsed().as_millis() as u64,
            "Database migrations completed"
        );
        Ok(())
    }

    /// Validate that all migrations are applied and return status.
    pub async fn validate_migrations(&self) -> Result<MigrationValidationResult> {
        let migrator = sqlx::migrate!("./migrations");
        let applied: Vec<String> = sqlx::query_scalar(
            "SELECT description FROM _sqlx_migrations ORDER BY installed_on",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let all_migrations: Vec<String> =
            migrator.iter().map(|m| m.description.to_string()).collect();
        let pending: Vec<String> = all_migrations
            .iter()
            .filter(|m| !applied.contains(m))
            .cloned()
            .collect();

        let result = MigrationValidationResult {
            is_current: pending.is_empty(),
            applied_count: applied.len(),
            pending_count: pending.len(),
            applied_migrations: applied,
            pending_migrations: pending.clone(),
            validated_at: Utc::now(),
        };

        if !result.is_current {
            warn!(
                pending_count = result.pending_count,
                "Database has pending migrations"
            );
        } else {
            info!(
                applied_count = result.applied_count,
                "All database migrations are applied"
            );
        }

        Ok(result)
    }

    /// Run startup validation: execute migrations, validate, and check connectivity.
    pub async fn startup_validation(&self) -> Result<()> {
        // Step 1: Run pending migrations
        self.run_migrations().await?;

        // Step 2: Validate migrations are applied
        let validation = self.validate_migrations().await?;
        if !validation.is_current {
            return Err(ApexError::new(
                crate::error::ErrorCode::DatabaseError,
                format!(
                    "Database has {} pending migrations after migration run",
                    validation.pending_count
                ),
            ));
        }

        // Step 3: Validate connectivity with a test query
        self.check_connectivity().await?;

        info!("Database startup validation passed");
        Ok(())
    }

    /// Check database connectivity by executing a simple query.
    pub async fn check_connectivity(&self) -> Result<Duration> {
        let start = Instant::now();
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!(error = %e, "Database connectivity check failed");
                ApexError::from(e)
            })?;
        let latency = start.elapsed();
        if latency > Duration::from_millis(100) {
            warn!(
                latency_ms = latency.as_millis() as u64,
                "Database connectivity check latency is high"
            );
        }
        Ok(latency)
    }

    /// Get connection pool metrics.
    pub fn pool_metrics(&self) -> ConnectionPoolMetrics {
        let pool_size = self.pool.size();
        let idle = self.pool.num_idle() as u32;
        let active = pool_size.saturating_sub(idle);
        let total_queries = self.counters.total_queries.load(Ordering::Relaxed);
        let total_time = self.counters.total_query_time_us.load(Ordering::Relaxed);
        let avg_latency = if total_queries > 0 {
            total_time / total_queries
        } else {
            0
        };
        let utilization = if self.max_connections > 0 {
            (active as f64 / self.max_connections as f64) * 100.0
        } else {
            0.0
        };

        ConnectionPoolMetrics {
            pool_size,
            idle_connections: idle,
            active_connections: active,
            max_connections: self.max_connections,
            min_connections: self.min_connections,
            utilization_pct: utilization,
            total_queries,
            slow_queries: self.counters.slow_queries.load(Ordering::Relaxed),
            avg_query_latency_us: avg_latency,
        }
    }

    /// Record a query execution for monitoring.
    pub fn record_query(&self, duration: Duration, query_label: &str) {
        let micros = duration.as_micros() as u64;
        self.counters
            .total_queries
            .fetch_add(1, Ordering::Relaxed);
        self.counters
            .total_query_time_us
            .fetch_add(micros, Ordering::Relaxed);

        if self.slow_query_config.enabled
            && duration.as_millis() as u64 > self.slow_query_config.threshold_ms
        {
            self.counters.slow_queries.fetch_add(1, Ordering::Relaxed);
            warn!(
                query = query_label,
                duration_ms = duration.as_millis() as u64,
                threshold_ms = self.slow_query_config.threshold_ms,
                "Slow query detected"
            );
        }
    }

    /// Validate database backup infrastructure.
    pub async fn validate_backups(&self) -> BackupValidationResult {
        let now = Utc::now();

        // Check WAL archiver status
        let archiver_result = sqlx::query(
            "SELECT archived_count::bigint, failed_count::bigint, \
             last_archived_time, last_failed_time FROM pg_stat_archiver",
        )
        .fetch_optional(&self.pool)
        .await;

        let (wal_archiving_enabled, archived_count, failed_count, last_archived_at, last_failed_at) =
            match archiver_result {
                Ok(Some(row)) => {
                    let archived: Option<i64> = row.try_get("archived_count").ok();
                    let failed: Option<i64> = row.try_get("failed_count").ok();
                    let last_arch: Option<DateTime<Utc>> =
                        row.try_get("last_archived_time").ok();
                    let last_fail: Option<DateTime<Utc>> =
                        row.try_get("last_failed_time").ok();
                    let enabled =
                        archived.unwrap_or(0) > 0 || failed.unwrap_or(0) > 0;
                    (enabled, archived, failed, last_arch, last_fail)
                }
                _ => (false, None, None, None, None),
            };

        // Check database size
        let db_size: Option<i64> =
            sqlx::query_scalar("SELECT pg_database_size(current_database())")
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();

        // Check replication slots
        let replication_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM pg_replication_slots",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);

        let has_replication_slots = replication_count > 0;

        // Determine backup status
        let status = if wal_archiving_enabled && failed_count.unwrap_or(0) == 0 {
            BackupStatus::Healthy
        } else if wal_archiving_enabled && failed_count.unwrap_or(0) > 0 {
            BackupStatus::Warning
        } else if has_replication_slots {
            BackupStatus::Healthy
        } else {
            BackupStatus::NotConfigured
        };

        BackupValidationResult {
            wal_archiving_enabled,
            archived_count,
            failed_count,
            last_archived_at,
            last_failed_at,
            database_size_bytes: db_size,
            has_replication_slots,
            status,
            validated_at: now,
        }
    }

    /// Get the pool reference.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// How long the monitor has been active.
    pub fn uptime(&self) -> Duration {
        self.started_at.elapsed()
    }
}
