//! Job worker for concurrent job execution.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Semaphore;

use super::JobQueue;

/// Configuration for the job worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Maximum concurrent job executions
    pub concurrency: usize,
    /// Poll interval for checking the queue (milliseconds)
    pub poll_interval_ms: u64,
    /// Shutdown timeout (seconds)
    pub shutdown_timeout_secs: u64,
    /// Worker name/identifier
    pub name: String,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            poll_interval_ms: 1000,
            shutdown_timeout_secs: 30,
            name: "apex-worker".to_string(),
        }
    }
}

/// Statistics for the job worker.
#[derive(Debug, Clone, Default)]
pub struct WorkerStats {
    /// Total jobs processed
    pub processed: Arc<AtomicU64>,
    /// Total jobs succeeded
    pub succeeded: Arc<AtomicU64>,
    /// Total jobs failed
    pub failed: Arc<AtomicU64>,
    /// Currently running jobs
    pub active: Arc<AtomicU64>,
}

impl WorkerStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn processed(&self) -> u64 {
        self.processed.load(Ordering::Relaxed)
    }

    pub fn succeeded(&self) -> u64 {
        self.succeeded.load(Ordering::Relaxed)
    }

    pub fn failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    pub fn active(&self) -> u64 {
        self.active.load(Ordering::Relaxed)
    }
}

/// Handle for controlling a running worker.
pub struct WorkerHandle {
    shutdown: tokio::sync::watch::Sender<bool>,
    stats: WorkerStats,
}

impl WorkerHandle {
    /// Signal the worker to shut down gracefully.
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }

    /// Get worker statistics.
    pub fn stats(&self) -> &WorkerStats {
        &self.stats
    }
}

/// Job worker that processes jobs from a queue.
pub struct JobWorker {
    config: WorkerConfig,
    stats: WorkerStats,
}

impl JobWorker {
    /// Create a new job worker.
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            config,
            stats: WorkerStats::new(),
        }
    }

    /// Start the worker, returning a handle for control.
    pub fn start(self, _queue: Arc<JobQueue>) -> WorkerHandle {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let stats = self.stats.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let _semaphore = Arc::new(Semaphore::new(config.concurrency));
            let poll_interval = tokio::time::Duration::from_millis(config.poll_interval_ms);

            tracing::info!(
                worker = %config.name,
                concurrency = config.concurrency,
                "Job worker started"
            );

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            tracing::info!(worker = %config.name, "Worker shutting down");
                            break;
                        }
                    }
                    _ = tokio::time::sleep(poll_interval) => {
                        // Poll queue for jobs
                        // In production, this would dequeue and execute jobs
                    }
                }
            }

            tracing::info!(worker = %config.name, "Worker stopped");
        });

        WorkerHandle {
            shutdown: shutdown_tx,
            stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_config_default() {
        let config = WorkerConfig::default();
        assert_eq!(config.concurrency, 4);
        assert_eq!(config.poll_interval_ms, 1000);
    }

    #[test]
    fn test_worker_stats() {
        let stats = WorkerStats::new();
        assert_eq!(stats.processed(), 0);
        assert_eq!(stats.succeeded(), 0);
        assert_eq!(stats.failed(), 0);
        assert_eq!(stats.active(), 0);

        stats.processed.fetch_add(1, Ordering::Relaxed);
        assert_eq!(stats.processed(), 1);
    }
}
