//! Worker Pool - Manages concurrent worker execution with tokio Semaphore.
//!
//! The `WorkerPool` provides:
//! - Configurable concurrency limits using tokio Semaphore
//! - Backpressure handling for overload situations
//! - Worker lifecycle management
//! - Pool statistics and monitoring

use std::future::Future;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{ApexError, Result};

/// Configuration for the worker pool.
#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    /// Maximum number of concurrent workers
    pub max_workers: usize,
    /// Timeout for acquiring a worker permit (milliseconds)
    pub acquire_timeout_ms: u64,
    /// Whether to track individual worker statistics
    pub track_worker_stats: bool,
    /// Name for this pool (for logging/metrics)
    pub name: String,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            max_workers: 100,
            acquire_timeout_ms: 30000,
            track_worker_stats: true,
            name: "default".to_string(),
        }
    }
}

impl WorkerPoolConfig {
    /// Create a small worker pool (10 workers).
    pub fn small() -> Self {
        Self {
            max_workers: 10,
            ..Default::default()
        }
    }

    /// Create a medium worker pool (50 workers).
    pub fn medium() -> Self {
        Self {
            max_workers: 50,
            ..Default::default()
        }
    }

    /// Create a large worker pool (200 workers).
    pub fn large() -> Self {
        Self {
            max_workers: 200,
            ..Default::default()
        }
    }

    /// Create with a specific name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// Statistics for an individual worker execution.
#[derive(Debug, Clone)]
pub struct WorkerExecution {
    /// Unique ID for this execution
    pub id: Uuid,
    /// When the worker started
    pub started_at: Instant,
    /// When the worker finished (None if still running)
    pub finished_at: Option<Instant>,
    /// Whether the execution succeeded
    pub succeeded: Option<bool>,
}

impl WorkerExecution {
    /// Create a new worker execution record.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            started_at: Instant::now(),
            finished_at: None,
            succeeded: None,
        }
    }

    /// Mark as completed.
    pub fn complete(&mut self, success: bool) {
        self.finished_at = Some(Instant::now());
        self.succeeded = Some(success);
    }

    /// Get duration (if completed).
    pub fn duration(&self) -> Option<Duration> {
        self.finished_at.map(|f| f.duration_since(self.started_at))
    }
}

impl Default for WorkerExecution {
    fn default() -> Self {
        Self::new()
    }
}

/// A handle to a worker permit that releases when dropped.
pub struct WorkerPermit {
    /// The semaphore permit
    _permit: OwnedSemaphorePermit,
    /// Pool reference for stats updates
    pool_stats: Arc<PoolStats>,
    /// Execution record
    execution: WorkerExecution,
}

impl WorkerPermit {
    /// Get the execution ID.
    pub fn id(&self) -> Uuid {
        self.execution.id
    }

    /// Mark this execution as successful.
    pub fn mark_success(mut self) {
        self.execution.complete(true);
        self.pool_stats.record_success();
        if let Some(duration) = self.execution.duration() {
            self.pool_stats.record_duration(duration);
        }
    }

    /// Mark this execution as failed.
    pub fn mark_failure(mut self) {
        self.execution.complete(false);
        self.pool_stats.record_failure();
        if let Some(duration) = self.execution.duration() {
            self.pool_stats.record_duration(duration);
        }
    }
}

impl Drop for WorkerPermit {
    fn drop(&mut self) {
        // If not explicitly completed, count as unknown
        if self.execution.finished_at.is_none() {
            self.pool_stats.record_unknown();
        }
    }
}

/// Internal statistics tracking.
struct PoolStats {
    /// Total tasks submitted
    tasks_submitted: AtomicU64,
    /// Successful completions
    tasks_succeeded: AtomicU64,
    /// Failed completions
    tasks_failed: AtomicU64,
    /// Unknown completions (permit dropped without marking)
    tasks_unknown: AtomicU64,
    /// Total time waiting for permits (microseconds)
    total_wait_time_us: AtomicU64,
    /// Total execution time (microseconds)
    total_exec_time_us: AtomicU64,
    /// Peak concurrent workers
    peak_concurrent: AtomicUsize,
    /// Current concurrent workers
    current_concurrent: AtomicUsize,
    /// Acquire timeouts
    acquire_timeouts: AtomicU64,
}

impl PoolStats {
    fn new() -> Self {
        Self {
            tasks_submitted: AtomicU64::new(0),
            tasks_succeeded: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
            tasks_unknown: AtomicU64::new(0),
            total_wait_time_us: AtomicU64::new(0),
            total_exec_time_us: AtomicU64::new(0),
            peak_concurrent: AtomicUsize::new(0),
            current_concurrent: AtomicUsize::new(0),
            acquire_timeouts: AtomicU64::new(0),
        }
    }

    fn record_submit(&self) {
        self.tasks_submitted.fetch_add(1, Ordering::Relaxed);
    }

    fn record_acquire(&self, wait_time: Duration) {
        self.total_wait_time_us
            .fetch_add(wait_time.as_micros() as u64, Ordering::Relaxed);
        let current = self.current_concurrent.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak_concurrent.fetch_max(current, Ordering::Relaxed);
    }

    fn record_release(&self) {
        self.current_concurrent.fetch_sub(1, Ordering::Relaxed);
    }

    fn record_success(&self) {
        self.tasks_succeeded.fetch_add(1, Ordering::Relaxed);
        self.record_release();
    }

    fn record_failure(&self) {
        self.tasks_failed.fetch_add(1, Ordering::Relaxed);
        self.record_release();
    }

    fn record_unknown(&self) {
        self.tasks_unknown.fetch_add(1, Ordering::Relaxed);
        self.record_release();
    }

    fn record_timeout(&self) {
        self.acquire_timeouts.fetch_add(1, Ordering::Relaxed);
    }

    fn record_duration(&self, duration: Duration) {
        self.total_exec_time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }
}

/// Manages a pool of concurrent workers using tokio Semaphore.
pub struct WorkerPool {
    /// Configuration
    config: WorkerPoolConfig,
    /// Semaphore for concurrency control
    semaphore: Arc<Semaphore>,
    /// Pool statistics
    stats: Arc<PoolStats>,
    /// Active tasks (for cancellation)
    active_tasks: RwLock<Vec<JoinHandle<()>>>,
    /// When the pool was created
    created_at: Instant,
}

impl WorkerPool {
    /// Create a new worker pool.
    pub fn new(config: WorkerPoolConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_workers));

        tracing::info!(
            pool_name = %config.name,
            max_workers = config.max_workers,
            "Worker pool created"
        );

        Self {
            config,
            semaphore,
            stats: Arc::new(PoolStats::new()),
            active_tasks: RwLock::new(Vec::new()),
            created_at: Instant::now(),
        }
    }

    /// Create a worker pool with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(WorkerPoolConfig::default())
    }

    /// Get the pool name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get maximum worker count.
    pub fn max_workers(&self) -> usize {
        self.config.max_workers
    }

    /// Get current available permits.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Get current number of active workers.
    pub fn active_workers(&self) -> usize {
        self.config.max_workers - self.semaphore.available_permits()
    }

    /// Check if the pool is at capacity.
    pub fn is_at_capacity(&self) -> bool {
        self.semaphore.available_permits() == 0
    }

    /// Acquire a worker permit.
    ///
    /// Returns a permit that must be held while doing work.
    /// The permit is automatically released when dropped.
    pub async fn acquire(&self) -> Result<WorkerPermit> {
        self.stats.record_submit();

        let start = Instant::now();
        let timeout = Duration::from_millis(self.config.acquire_timeout_ms);

        let permit = tokio::time::timeout(timeout, self.semaphore.clone().acquire_owned())
            .await
            .map_err(|_| {
                self.stats.record_timeout();
                tracing::warn!(
                    pool_name = %self.config.name,
                    timeout_ms = self.config.acquire_timeout_ms,
                    "Worker permit acquire timed out"
                );
                ApexError::internal(format!(
                    "Worker pool '{}' acquire timeout after {}ms",
                    self.config.name, self.config.acquire_timeout_ms
                ))
            })?
            .map_err(|_| {
                ApexError::internal(format!(
                    "Worker pool '{}' semaphore closed",
                    self.config.name
                ))
            })?;

        let wait_time = start.elapsed();
        self.stats.record_acquire(wait_time);

        tracing::debug!(
            pool_name = %self.config.name,
            wait_time_ms = wait_time.as_millis(),
            available = self.semaphore.available_permits(),
            "Worker permit acquired"
        );

        Ok(WorkerPermit {
            _permit: permit,
            pool_stats: self.stats.clone(),
            execution: WorkerExecution::new(),
        })
    }

    /// Try to acquire a worker permit without waiting.
    pub fn try_acquire(&self) -> Option<WorkerPermit> {
        self.stats.record_submit();

        match self.semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                self.stats.record_acquire(Duration::ZERO);
                Some(WorkerPermit {
                    _permit: permit,
                    pool_stats: self.stats.clone(),
                    execution: WorkerExecution::new(),
                })
            }
            Err(_) => None,
        }
    }

    /// Spawn a task on the worker pool.
    ///
    /// Acquires a permit, runs the future, and automatically
    /// releases the permit when done.
    pub async fn spawn<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let permit = self.acquire().await?;
        let result = f().await;

        match &result {
            Ok(_) => permit.mark_success(),
            Err(_) => permit.mark_failure(),
        }

        result
    }

    /// Spawn a task in the background (fire and forget).
    ///
    /// The task will run when a permit becomes available.
    pub fn spawn_background<F, Fut>(&self, f: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let semaphore = self.semaphore.clone();
        let stats = self.stats.clone();
        let pool_name = self.config.name.clone();

        let handle = tokio::spawn(async move {
            stats.record_submit();
            let start = Instant::now();

            let permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    tracing::error!(pool_name = %pool_name, "Semaphore closed");
                    return;
                }
            };

            stats.record_acquire(start.elapsed());

            f().await;

            drop(permit);
            stats.record_unknown();
        });

        self.active_tasks.write().push(handle);
    }

    /// Wait for all active background tasks to complete.
    pub async fn join_all(&self) {
        let handles: Vec<_> = self.active_tasks.write().drain(..).collect();

        for handle in handles {
            let _ = handle.await;
        }
    }

    /// Cancel all active background tasks.
    pub fn cancel_all(&self) {
        let handles: Vec<_> = self.active_tasks.write().drain(..).collect();

        for handle in handles {
            handle.abort();
        }

        tracing::info!(
            pool_name = %self.config.name,
            "All background tasks cancelled"
        );
    }

    /// Get pool statistics.
    pub fn stats(&self) -> WorkerPoolStats {
        let stats = &self.stats;
        let tasks_submitted = stats.tasks_submitted.load(Ordering::Relaxed);
        let tasks_succeeded = stats.tasks_succeeded.load(Ordering::Relaxed);
        let tasks_failed = stats.tasks_failed.load(Ordering::Relaxed);
        let total_completed = tasks_succeeded + tasks_failed;

        let avg_wait_time_us = if tasks_submitted > 0 {
            stats.total_wait_time_us.load(Ordering::Relaxed) / tasks_submitted
        } else {
            0
        };

        let avg_exec_time_us = if total_completed > 0 {
            stats.total_exec_time_us.load(Ordering::Relaxed) / total_completed
        } else {
            0
        };

        WorkerPoolStats {
            name: self.config.name.clone(),
            max_workers: self.config.max_workers,
            available_permits: self.semaphore.available_permits(),
            active_workers: self.active_workers(),
            tasks_submitted,
            tasks_succeeded,
            tasks_failed,
            tasks_unknown: stats.tasks_unknown.load(Ordering::Relaxed),
            acquire_timeouts: stats.acquire_timeouts.load(Ordering::Relaxed),
            peak_concurrent: stats.peak_concurrent.load(Ordering::Relaxed),
            avg_wait_time_us,
            avg_exec_time_us,
            uptime_secs: self.created_at.elapsed().as_secs(),
        }
    }

    /// Check if the pool is healthy.
    pub fn is_healthy(&self) -> bool {
        let stats = self.stats();

        // Consider unhealthy if:
        // - High timeout rate (>10%)
        // - Always at capacity
        // - High failure rate (>50%)

        let timeout_rate = if stats.tasks_submitted > 0 {
            stats.acquire_timeouts as f64 / stats.tasks_submitted as f64
        } else {
            0.0
        };

        let failure_rate = if stats.tasks_succeeded + stats.tasks_failed > 0 {
            stats.tasks_failed as f64 / (stats.tasks_succeeded + stats.tasks_failed) as f64
        } else {
            0.0
        };

        timeout_rate < 0.1 && failure_rate < 0.5
    }

    /// Resize the pool (adds or removes permits).
    ///
    /// Note: This only affects the maximum, it won't interrupt running tasks.
    pub fn resize(&mut self, new_max: usize) {
        let current_max = self.config.max_workers;

        if new_max > current_max {
            // Add permits
            self.semaphore.add_permits(new_max - current_max);
        } else if new_max < current_max {
            // Can't actually remove permits from a semaphore,
            // so we create a new one (existing permits will still work)
            tracing::warn!(
                pool_name = %self.config.name,
                old_max = current_max,
                new_max = new_max,
                "Reducing pool size - existing workers will complete"
            );
            // In production, you'd want a more sophisticated approach
        }

        self.config.max_workers = new_max;

        tracing::info!(
            pool_name = %self.config.name,
            old_max = current_max,
            new_max = new_max,
            "Worker pool resized"
        );
    }
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Statistics for the worker pool.
#[derive(Debug, Clone)]
pub struct WorkerPoolStats {
    /// Pool name
    pub name: String,
    /// Maximum workers configured
    pub max_workers: usize,
    /// Currently available permits
    pub available_permits: usize,
    /// Currently active workers
    pub active_workers: usize,
    /// Total tasks submitted
    pub tasks_submitted: u64,
    /// Successfully completed tasks
    pub tasks_succeeded: u64,
    /// Failed tasks
    pub tasks_failed: u64,
    /// Tasks with unknown outcome
    pub tasks_unknown: u64,
    /// Number of acquire timeouts
    pub acquire_timeouts: u64,
    /// Peak concurrent workers observed
    pub peak_concurrent: usize,
    /// Average wait time for permits (microseconds)
    pub avg_wait_time_us: u64,
    /// Average execution time (microseconds)
    pub avg_exec_time_us: u64,
    /// Pool uptime in seconds
    pub uptime_secs: u64,
}

impl WorkerPoolStats {
    /// Calculate success rate as a percentage.
    pub fn success_rate(&self) -> f64 {
        let total = self.tasks_succeeded + self.tasks_failed;
        if total == 0 {
            100.0
        } else {
            (self.tasks_succeeded as f64 / total as f64) * 100.0
        }
    }

    /// Calculate utilization as a percentage.
    pub fn utilization(&self) -> f64 {
        ((self.max_workers - self.available_permits) as f64 / self.max_workers as f64) * 100.0
    }

    /// Calculate throughput (tasks per second).
    pub fn throughput(&self) -> f64 {
        if self.uptime_secs == 0 {
            0.0
        } else {
            (self.tasks_succeeded + self.tasks_failed) as f64 / self.uptime_secs as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_pool_creation() {
        let pool = WorkerPool::with_defaults();

        assert_eq!(pool.max_workers(), 100);
        assert_eq!(pool.available_permits(), 100);
        assert_eq!(pool.active_workers(), 0);
        assert!(!pool.is_at_capacity());
    }

    #[test]
    fn test_pool_config() {
        let config = WorkerPoolConfig::small().with_name("test-pool");

        assert_eq!(config.max_workers, 10);
        assert_eq!(config.name, "test-pool");
    }

    #[tokio::test]
    async fn test_acquire_release() {
        let pool = WorkerPool::new(WorkerPoolConfig {
            max_workers: 2,
            ..Default::default()
        });

        assert_eq!(pool.available_permits(), 2);

        let permit1 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_permits(), 1);

        let permit2 = pool.acquire().await.unwrap();
        assert_eq!(pool.available_permits(), 0);
        assert!(pool.is_at_capacity());

        permit1.mark_success();
        assert_eq!(pool.available_permits(), 1);

        permit2.mark_failure();
        assert_eq!(pool.available_permits(), 2);
    }

    #[tokio::test]
    async fn test_try_acquire() {
        let pool = WorkerPool::new(WorkerPoolConfig {
            max_workers: 1,
            ..Default::default()
        });

        let permit1 = pool.try_acquire();
        assert!(permit1.is_some());

        let permit2 = pool.try_acquire();
        assert!(permit2.is_none());

        drop(permit1);

        let permit3 = pool.try_acquire();
        assert!(permit3.is_some());
    }

    #[tokio::test]
    async fn test_spawn() {
        let pool = WorkerPool::with_defaults();

        let result = pool
            .spawn(|| async { Ok::<i32, ApexError>(42) })
            .await
            .unwrap();

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_spawn_failure() {
        let pool = WorkerPool::with_defaults();

        let result: Result<i32> = pool
            .spawn(|| async { Err(ApexError::internal("test error")) })
            .await;

        assert!(result.is_err());

        let stats = pool.stats();
        assert_eq!(stats.tasks_failed, 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let pool = WorkerPool::new(WorkerPoolConfig {
            max_workers: 5,
            ..Default::default()
        });

        for _ in 0..3 {
            let permit = pool.acquire().await.unwrap();
            permit.mark_success();
        }

        let permit = pool.acquire().await.unwrap();
        permit.mark_failure();

        let stats = pool.stats();

        assert_eq!(stats.tasks_submitted, 4);
        assert_eq!(stats.tasks_succeeded, 3);
        assert_eq!(stats.tasks_failed, 1);
        assert_eq!(stats.success_rate(), 75.0);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
            max_workers: 10,
            ..Default::default()
        }));

        let mut handles = vec![];

        for _ in 0..20 {
            let pool_clone = pool.clone();
            let handle = tokio::spawn(async move {
                let permit = pool_clone.acquire().await.unwrap();
                tokio::time::sleep(Duration::from_millis(10)).await;
                permit.mark_success();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let stats = pool.stats();
        assert_eq!(stats.tasks_submitted, 20);
        assert_eq!(stats.tasks_succeeded, 20);
        assert!(stats.peak_concurrent <= 10);
    }

    #[tokio::test]
    async fn test_acquire_timeout() {
        let pool = WorkerPool::new(WorkerPoolConfig {
            max_workers: 1,
            acquire_timeout_ms: 50,
            ..Default::default()
        });

        // Hold the only permit
        let _permit = pool.acquire().await.unwrap();

        // Try to acquire another (should timeout)
        let result = pool.acquire().await;
        assert!(result.is_err());

        let stats = pool.stats();
        assert_eq!(stats.acquire_timeouts, 1);
    }

    #[test]
    fn test_is_healthy() {
        let pool = WorkerPool::with_defaults();
        assert!(pool.is_healthy());
    }

    #[tokio::test]
    async fn test_spawn_background() {
        let pool = Arc::new(WorkerPool::new(WorkerPoolConfig {
            max_workers: 2,
            ..Default::default()
        }));

        let counter = Arc::new(AtomicU64::new(0));
        let counter_clone = counter.clone();

        pool.spawn_background(move || async move {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        });

        pool.join_all().await;

        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_cancel_all() {
        let pool = WorkerPool::new(WorkerPoolConfig {
            max_workers: 2,
            ..Default::default()
        });

        pool.spawn_background(|| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        pool.spawn_background(|| async {
            tokio::time::sleep(Duration::from_secs(10)).await;
        });

        // Give tasks time to start
        tokio::time::sleep(Duration::from_millis(10)).await;

        pool.cancel_all();

        // Should not hang
    }

    #[test]
    fn test_worker_pool_stats_calculations() {
        let stats = WorkerPoolStats {
            name: "test".to_string(),
            max_workers: 10,
            available_permits: 3,
            active_workers: 7,
            tasks_submitted: 100,
            tasks_succeeded: 80,
            tasks_failed: 20,
            tasks_unknown: 0,
            acquire_timeouts: 5,
            peak_concurrent: 10,
            avg_wait_time_us: 1000,
            avg_exec_time_us: 5000,
            uptime_secs: 60,
        };

        assert_eq!(stats.success_rate(), 80.0);
        assert_eq!(stats.utilization(), 70.0);
        assert!((stats.throughput() - 1.667).abs() < 0.01);
    }
}
