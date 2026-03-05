//! Job queue with priority support and dead letter handling.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::JobMetadata;

/// Configuration for the job queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum queue size (0 = unlimited)
    pub max_size: usize,
    /// Maximum time a job can stay in the queue before expiring
    pub max_age_secs: Option<u64>,
    /// Whether to enable the dead letter queue
    pub enable_dead_letter: bool,
    /// Maximum items in the dead letter queue
    pub dead_letter_max_size: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            max_age_secs: Some(86400), // 24 hours
            enable_dead_letter: true,
            dead_letter_max_size: 1000,
        }
    }
}

/// A job in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedJob {
    /// Job metadata
    pub metadata: JobMetadata,
    /// Serialized job data
    pub data: serde_json::Value,
    /// When the job was enqueued
    pub enqueued_at: DateTime<Utc>,
}

impl Eq for QueuedJob {}

impl PartialEq for QueuedJob {
    fn eq(&self, other: &Self) -> bool {
        self.metadata.id == other.metadata.id
    }
}

impl PartialOrd for QueuedJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedJob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.metadata
            .priority
            .cmp(&other.metadata.priority)
            .then_with(|| other.enqueued_at.cmp(&self.enqueued_at))
    }
}

/// Queue statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    /// Number of pending jobs
    pub pending: usize,
    /// Number of running jobs
    pub running: usize,
    /// Number of completed jobs (total)
    pub completed: u64,
    /// Number of failed jobs (total)
    pub failed: u64,
    /// Number of dead letter jobs
    pub dead_letter: usize,
    /// Average wait time in seconds
    pub avg_wait_secs: f64,
}

/// Dead letter queue for failed jobs.
#[derive(Debug)]
pub struct DeadLetterQueue {
    jobs: VecDeque<QueuedJob>,
    max_size: usize,
}

impl DeadLetterQueue {
    pub fn new(max_size: usize) -> Self {
        Self {
            jobs: VecDeque::new(),
            max_size,
        }
    }

    pub fn push(&mut self, job: QueuedJob) {
        if self.jobs.len() >= self.max_size {
            self.jobs.pop_front();
        }
        self.jobs.push_back(job);
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub fn drain(&mut self) -> Vec<QueuedJob> {
        self.jobs.drain(..).collect()
    }
}

/// Trait for queue backends.
#[async_trait]
pub trait QueueBackend: Send + Sync {
    /// Enqueue a job.
    async fn enqueue(&self, job: QueuedJob) -> crate::error::Result<()>;

    /// Dequeue the highest priority job.
    async fn dequeue(&self) -> crate::error::Result<Option<QueuedJob>>;

    /// Get queue statistics.
    async fn stats(&self) -> crate::error::Result<QueueStats>;

    /// Get the current queue length.
    async fn len(&self) -> crate::error::Result<usize>;

    /// Check if the queue is empty.
    async fn is_empty(&self) -> crate::error::Result<bool> {
        Ok(self.len().await? == 0)
    }
}

/// In-memory queue backend for testing and development.
pub struct InMemoryQueueBackend {
    queue: Arc<RwLock<BinaryHeap<QueuedJob>>>,
    stats: Arc<RwLock<QueueStats>>,
}

impl InMemoryQueueBackend {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(RwLock::new(BinaryHeap::new())),
            stats: Arc::new(RwLock::new(QueueStats::default())),
        }
    }
}

impl Default for InMemoryQueueBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl QueueBackend for InMemoryQueueBackend {
    async fn enqueue(&self, job: QueuedJob) -> crate::error::Result<()> {
        let mut queue = self.queue.write().await;
        let mut stats = self.stats.write().await;
        queue.push(job);
        stats.pending = queue.len();
        Ok(())
    }

    async fn dequeue(&self) -> crate::error::Result<Option<QueuedJob>> {
        let mut queue = self.queue.write().await;
        let mut stats = self.stats.write().await;
        let job = queue.pop();
        stats.pending = queue.len();
        if job.is_some() {
            stats.running += 1;
        }
        Ok(job)
    }

    async fn stats(&self) -> crate::error::Result<QueueStats> {
        Ok(self.stats.read().await.clone())
    }

    async fn len(&self) -> crate::error::Result<usize> {
        Ok(self.queue.read().await.len())
    }
}

/// Redis-backed queue backend for production use.
pub struct RedisQueueBackend {
    client: redis::Client,
    queue_key: String,
    _config: QueueConfig,
}

impl RedisQueueBackend {
    /// Create a new Redis queue backend.
    ///
    /// # Arguments
    /// * `client` - A connected Redis client
    /// * `queue_key` - The Redis list key to use (e.g. `"apex:jobs:default"`)
    /// * `config` - Queue configuration
    pub fn new(client: redis::Client, queue_key: impl Into<String>, config: QueueConfig) -> Self {
        Self {
            client,
            queue_key: queue_key.into(),
            _config: config,
        }
    }

    /// Obtain an async multiplexed connection from the Redis client.
    async fn get_conn(&self) -> crate::error::Result<redis::aio::MultiplexedConnection> {
        self.client.get_multiplexed_async_connection().await
            .map_err(|e| crate::error::ApexError::with_internal(
                crate::error::ErrorCode::CacheConnectionFailed,
                "Failed to get Redis connection for job queue",
                e.to_string(),
            ))
    }
}

#[async_trait]
impl QueueBackend for RedisQueueBackend {
    async fn enqueue(&self, job: QueuedJob) -> crate::error::Result<()> {
        let _span = tracing::info_span!("redis_queue_enqueue", queue = %self.queue_key);
        let _guard = _span.enter();

        let serialized = serde_json::to_string(&job)?;

        let mut conn = self.get_conn().await?;
        redis::cmd("RPUSH")
            .arg(&self.queue_key)
            .arg(&serialized)
            .query_async::<_, i64>(&mut conn)
            .await
            .map_err(|e| crate::error::ApexError::with_internal(
                crate::error::ErrorCode::CacheError,
                "Failed to enqueue job to Redis",
                e.to_string(),
            ))?;

        tracing::debug!(queue = %self.queue_key, job_id = %job.metadata.id, "Job enqueued");
        Ok(())
    }

    async fn dequeue(&self) -> crate::error::Result<Option<QueuedJob>> {
        let _span = tracing::info_span!("redis_queue_dequeue", queue = %self.queue_key);
        let _guard = _span.enter();

        let mut conn = self.get_conn().await?;

        // BLPOP with a 5-second timeout so we don't block indefinitely
        let result: Option<(String, String)> = redis::cmd("BLPOP")
            .arg(&self.queue_key)
            .arg(5_u64)
            .query_async(&mut conn)
            .await
            .map_err(|e| crate::error::ApexError::with_internal(
                crate::error::ErrorCode::CacheError,
                "Failed to dequeue job from Redis",
                e.to_string(),
            ))?;

        match result {
            Some((_key, value)) => {
                let job: QueuedJob = serde_json::from_str(&value)?;
                tracing::debug!(queue = %self.queue_key, job_id = %job.metadata.id, "Job dequeued");
                Ok(Some(job))
            }
            None => Ok(None),
        }
    }

    async fn stats(&self) -> crate::error::Result<QueueStats> {
        let _span = tracing::info_span!("redis_queue_stats", queue = %self.queue_key);
        let _guard = _span.enter();

        let mut conn = self.get_conn().await?;
        let pending: usize = redis::cmd("LLEN")
            .arg(&self.queue_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| crate::error::ApexError::with_internal(
                crate::error::ErrorCode::CacheError,
                "Failed to get Redis queue stats",
                e.to_string(),
            ))?;

        Ok(QueueStats {
            pending,
            running: 0,
            completed: 0,
            failed: 0,
            dead_letter: 0,
            avg_wait_secs: 0.0,
        })
    }

    async fn len(&self) -> crate::error::Result<usize> {
        let _span = tracing::info_span!("redis_queue_len", queue = %self.queue_key);
        let _guard = _span.enter();

        let mut conn = self.get_conn().await?;
        let length: usize = redis::cmd("LLEN")
            .arg(&self.queue_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| crate::error::ApexError::with_internal(
                crate::error::ErrorCode::CacheError,
                "Failed to get Redis queue length",
                e.to_string(),
            ))?;

        Ok(length)
    }
}

/// The main job queue.
pub struct JobQueue {
    backend: Arc<dyn QueueBackend>,
    dead_letter: Arc<RwLock<DeadLetterQueue>>,
    config: QueueConfig,
}

impl JobQueue {
    /// Create a new job queue with the given backend.
    pub fn new(backend: Arc<dyn QueueBackend>, config: QueueConfig) -> Self {
        let dlq = DeadLetterQueue::new(config.dead_letter_max_size);
        Self {
            backend,
            dead_letter: Arc::new(RwLock::new(dlq)),
            config,
        }
    }

    /// Create a new in-memory job queue (for testing).
    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryQueueBackend::new()),
            QueueConfig::default(),
        )
    }

    /// Enqueue a job.
    pub async fn enqueue(&self, job: QueuedJob) -> crate::error::Result<()> {
        self.backend.enqueue(job).await
    }

    /// Dequeue the next job.
    pub async fn dequeue(&self) -> crate::error::Result<Option<QueuedJob>> {
        self.backend.dequeue().await
    }

    /// Move a job to the dead letter queue.
    pub async fn dead_letter(&self, job: QueuedJob) {
        if self.config.enable_dead_letter {
            self.dead_letter.write().await.push(job);
        }
    }

    /// Get queue statistics.
    pub async fn stats(&self) -> crate::error::Result<QueueStats> {
        let mut stats = self.backend.stats().await?;
        stats.dead_letter = self.dead_letter.read().await.len();
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobPriority;

    #[tokio::test]
    async fn test_in_memory_queue() {
        let queue = JobQueue::in_memory();

        let metadata = JobMetadata::new("test_job")
            .with_priority(JobPriority::Normal);
        let job = QueuedJob {
            metadata,
            data: serde_json::json!({}),
            enqueued_at: Utc::now(),
        };

        queue.enqueue(job).await.unwrap();
        let dequeued = queue.dequeue().await.unwrap();
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().metadata.job_type, "test_job");
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = JobQueue::in_memory();

        let low = QueuedJob {
            metadata: JobMetadata::new("low").with_priority(JobPriority::Low),
            data: serde_json::json!({}),
            enqueued_at: Utc::now(),
        };
        let high = QueuedJob {
            metadata: JobMetadata::new("high").with_priority(JobPriority::High),
            data: serde_json::json!({}),
            enqueued_at: Utc::now(),
        };

        queue.enqueue(low).await.unwrap();
        queue.enqueue(high).await.unwrap();

        let first = queue.dequeue().await.unwrap().unwrap();
        assert_eq!(first.metadata.job_type, "high");
    }
}
