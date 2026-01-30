//! Job definitions and traits.
//!
//! This module provides the core abstractions for defining background jobs:
//!
//! - **Job trait**: The main interface that all jobs must implement
//! - **JobStatus**: Enumeration of possible job states
//! - **JobContext**: Context passed to jobs during execution
//! - **RetryPolicy**: Configuration for retry behavior with backoff strategies

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use uuid::Uuid;

use crate::error::{ApexError, ErrorCode, Result};

// ═══════════════════════════════════════════════════════════════════════════════
// Job Identification
// ═══════════════════════════════════════════════════════════════════════════════

/// Unique identifier for a job instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    /// Create a new random job ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from an existing UUID.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for JobId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Job Status
// ═══════════════════════════════════════════════════════════════════════════════

/// Status of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job is waiting in the queue
    Pending,
    /// Job is scheduled for future execution
    Scheduled,
    /// Job is currently being executed
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed and may be retried
    Failed,
    /// Job failed after all retry attempts
    Dead,
    /// Job was cancelled
    Cancelled,
}

impl JobStatus {
    /// Check if the job is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Dead | Self::Cancelled)
    }

    /// Check if the job can be retried.
    pub fn can_retry(&self) -> bool {
        matches!(self, Self::Failed)
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Scheduled => write!(f, "scheduled"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Dead => write!(f, "dead"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Job Priority
// ═══════════════════════════════════════════════════════════════════════════════

/// Priority level for jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobPriority {
    /// Lowest priority - processed when queues are empty
    Low = 0,
    /// Normal priority - default for most jobs
    Normal = 1,
    /// High priority - processed before normal jobs
    High = 2,
    /// Critical priority - processed immediately
    Critical = 3,
}

impl Default for JobPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl JobPriority {
    /// Get the numeric value for queue ordering.
    pub fn score(&self) -> i64 {
        match self {
            Self::Low => 0,
            Self::Normal => 100,
            Self::High => 200,
            Self::Critical => 300,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Job Error
// ═══════════════════════════════════════════════════════════════════════════════

/// Error type for job execution failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobError {
    /// Error message
    pub message: String,
    /// Whether this error is retryable
    pub retryable: bool,
    /// Optional error code
    pub code: Option<String>,
    /// Additional context
    pub context: Option<serde_json::Value>,
}

impl JobError {
    /// Create a new retryable error.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
            code: None,
            context: None,
        }
    }

    /// Create a new non-retryable (fatal) error.
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
            code: None,
            context: None,
        }
    }

    /// Add an error code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Add context.
    pub fn with_context(mut self, context: impl Serialize) -> Self {
        self.context = serde_json::to_value(context).ok();
        self
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(code) = &self.code {
            write!(f, " (code: {})", code)?;
        }
        Ok(())
    }
}

impl std::error::Error for JobError {}

impl From<ApexError> for JobError {
    fn from(error: ApexError) -> Self {
        Self {
            message: error.user_message().to_string(),
            retryable: error.is_retryable(),
            code: Some(error.code().to_string()),
            context: None,
        }
    }
}

/// Result type for job execution.
pub type JobResult = std::result::Result<(), JobError>;

// ═══════════════════════════════════════════════════════════════════════════════
// Backoff Strategy
// ═══════════════════════════════════════════════════════════════════════════════

/// Strategy for calculating retry delays.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed {
        delay_secs: u64,
    },
    /// Linear increase in delay (delay * attempt)
    Linear {
        initial_delay_secs: u64,
        increment_secs: u64,
    },
    /// Exponential increase in delay (initial * 2^attempt)
    Exponential {
        initial_delay_secs: u64,
        max_delay_secs: u64,
        multiplier: f64,
    },
    /// Exponential with random jitter
    ExponentialWithJitter {
        initial_delay_secs: u64,
        max_delay_secs: u64,
        multiplier: f64,
        jitter_factor: f64,
    },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::Exponential {
            initial_delay_secs: 5,
            max_delay_secs: 3600, // 1 hour max
            multiplier: 2.0,
        }
    }
}

impl BackoffStrategy {
    /// Calculate the delay for a given attempt number (0-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let secs = match self {
            Self::Fixed { delay_secs } => *delay_secs,
            Self::Linear {
                initial_delay_secs,
                increment_secs,
            } => initial_delay_secs + (increment_secs * attempt as u64),
            Self::Exponential {
                initial_delay_secs,
                max_delay_secs,
                multiplier,
            } => {
                let delay = (*initial_delay_secs as f64) * multiplier.powi(attempt as i32);
                delay.min(*max_delay_secs as f64) as u64
            }
            Self::ExponentialWithJitter {
                initial_delay_secs,
                max_delay_secs,
                multiplier,
                jitter_factor,
            } => {
                let base_delay = (*initial_delay_secs as f64) * multiplier.powi(attempt as i32);
                let capped_delay = base_delay.min(*max_delay_secs as f64);
                // Add jitter: delay * (1 +/- jitter_factor * random)
                let jitter_range = capped_delay * jitter_factor;
                let jitter = (rand_simple() * 2.0 - 1.0) * jitter_range;
                (capped_delay + jitter).max(1.0) as u64
            }
        };

        Duration::from_secs(secs)
    }

    /// Create a fixed backoff strategy.
    pub fn fixed(delay_secs: u64) -> Self {
        Self::Fixed { delay_secs }
    }

    /// Create an exponential backoff strategy with sensible defaults.
    pub fn exponential() -> Self {
        Self::default()
    }

    /// Create an exponential backoff with jitter.
    pub fn exponential_with_jitter() -> Self {
        Self::ExponentialWithJitter {
            initial_delay_secs: 5,
            max_delay_secs: 3600,
            multiplier: 2.0,
            jitter_factor: 0.2,
        }
    }
}

/// Simple pseudo-random number generator for jitter (0.0 to 1.0).
fn rand_simple() -> f64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    hasher.write_u64(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64);
    (hasher.finish() as f64) / (u64::MAX as f64)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Retry Policy
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for job retry behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 = no retries)
    pub max_attempts: u32,
    /// Backoff strategy for calculating delays
    pub backoff: BackoffStrategy,
    /// Whether to retry on any error or only retryable errors
    pub retry_on_any_error: bool,
    /// Maximum total time for all retries (optional)
    pub max_retry_duration_secs: Option<u64>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff: BackoffStrategy::default(),
            retry_on_any_error: false,
            max_retry_duration_secs: None,
        }
    }
}

impl RetryPolicy {
    /// Create a policy with no retries.
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 0,
            ..Default::default()
        }
    }

    /// Create a policy with a specific number of retries.
    pub fn with_retries(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            ..Default::default()
        }
    }

    /// Create a policy with exponential backoff.
    pub fn exponential_backoff(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            backoff: BackoffStrategy::exponential(),
            ..Default::default()
        }
    }

    /// Create an aggressive retry policy for critical jobs.
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 10,
            backoff: BackoffStrategy::ExponentialWithJitter {
                initial_delay_secs: 1,
                max_delay_secs: 300,
                multiplier: 1.5,
                jitter_factor: 0.3,
            },
            retry_on_any_error: true,
            max_retry_duration_secs: Some(3600),
        }
    }

    /// Check if another retry should be attempted.
    pub fn should_retry(&self, attempt: u32, error: &JobError, started_at: DateTime<Utc>) -> bool {
        // Check max attempts
        if attempt >= self.max_attempts {
            return false;
        }

        // Check if error is retryable
        if !self.retry_on_any_error && !error.retryable {
            return false;
        }

        // Check max retry duration
        if let Some(max_duration) = self.max_retry_duration_secs {
            let elapsed = Utc::now().signed_duration_since(started_at);
            if elapsed.num_seconds() as u64 >= max_duration {
                return false;
            }
        }

        true
    }

    /// Get the delay before the next retry.
    pub fn next_retry_delay(&self, attempt: u32) -> Duration {
        self.backoff.delay_for_attempt(attempt)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Job Metadata
// ═══════════════════════════════════════════════════════════════════════════════

/// Metadata associated with a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    /// Unique job identifier
    pub id: JobId,
    /// Job type name
    pub job_type: String,
    /// Current status
    pub status: JobStatus,
    /// Priority level
    pub priority: JobPriority,
    /// Number of execution attempts
    pub attempts: u32,
    /// Maximum attempts allowed
    pub max_attempts: u32,
    /// When the job was created
    pub created_at: DateTime<Utc>,
    /// When the job should be executed (for scheduled jobs)
    pub scheduled_at: Option<DateTime<Utc>>,
    /// When the job started executing
    pub started_at: Option<DateTime<Utc>>,
    /// When the job finished (success or failure)
    pub finished_at: Option<DateTime<Utc>>,
    /// Last error message (if failed)
    pub last_error: Option<String>,
    /// Correlation ID for tracing
    pub correlation_id: Option<String>,
    /// Custom tags for filtering
    pub tags: Vec<String>,
    /// Timeout for job execution
    pub timeout_secs: Option<u64>,
}

impl JobMetadata {
    /// Create new metadata for a job.
    pub fn new(job_type: impl Into<String>) -> Self {
        Self {
            id: JobId::new(),
            job_type: job_type.into(),
            status: JobStatus::Pending,
            priority: JobPriority::default(),
            attempts: 0,
            max_attempts: 3,
            created_at: Utc::now(),
            scheduled_at: None,
            started_at: None,
            finished_at: None,
            last_error: None,
            correlation_id: None,
            tags: Vec::new(),
            timeout_secs: None,
        }
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: JobPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the scheduled time.
    pub fn scheduled_for(mut self, at: DateTime<Utc>) -> Self {
        self.scheduled_at = Some(at);
        self.status = JobStatus::Scheduled;
        self
    }

    /// Set the maximum attempts.
    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts = max;
        self
    }

    /// Set a correlation ID.
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set the timeout.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Mark as running.
    pub fn mark_running(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Some(Utc::now());
        self.attempts += 1;
    }

    /// Mark as completed.
    pub fn mark_completed(&mut self) {
        self.status = JobStatus::Completed;
        self.finished_at = Some(Utc::now());
    }

    /// Mark as failed.
    pub fn mark_failed(&mut self, error: &str) {
        self.status = JobStatus::Failed;
        self.last_error = Some(error.to_string());
    }

    /// Mark as dead (no more retries).
    pub fn mark_dead(&mut self, error: &str) {
        self.status = JobStatus::Dead;
        self.finished_at = Some(Utc::now());
        self.last_error = Some(error.to_string());
    }

    /// Mark as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = JobStatus::Cancelled;
        self.finished_at = Some(Utc::now());
    }

    /// Get the duration if completed.
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }

    /// Check if the job can be retried.
    pub fn can_retry(&self) -> bool {
        self.status == JobStatus::Failed && self.attempts < self.max_attempts
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Job Context
// ═══════════════════════════════════════════════════════════════════════════════

/// Context passed to jobs during execution.
pub struct JobContext {
    /// Job metadata
    pub metadata: JobMetadata,
    /// Retry policy for this job
    pub retry_policy: RetryPolicy,
    /// Cancellation token
    cancellation: tokio::sync::watch::Receiver<bool>,
    /// Progress callback
    progress_sender: Option<tokio::sync::mpsc::Sender<JobProgress>>,
}

/// Progress update from a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobProgress {
    /// Job ID
    pub job_id: JobId,
    /// Progress percentage (0-100)
    pub percent: u8,
    /// Status message
    pub message: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl JobContext {
    /// Create a new job context.
    pub fn new(
        metadata: JobMetadata,
        retry_policy: RetryPolicy,
        cancellation: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self {
            metadata,
            retry_policy,
            cancellation,
            progress_sender: None,
        }
    }

    /// Set a progress sender.
    pub fn with_progress_sender(
        mut self,
        sender: tokio::sync::mpsc::Sender<JobProgress>,
    ) -> Self {
        self.progress_sender = Some(sender);
        self
    }

    /// Get the job ID.
    pub fn job_id(&self) -> JobId {
        self.metadata.id
    }

    /// Get the job type.
    pub fn job_type(&self) -> &str {
        &self.metadata.job_type
    }

    /// Get the current attempt number (1-indexed).
    pub fn attempt(&self) -> u32 {
        self.metadata.attempts
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        *self.cancellation.borrow()
    }

    /// Wait for cancellation or complete a future.
    pub async fn cancellable<F, T>(&mut self, future: F) -> Option<T>
    where
        F: std::future::Future<Output = T>,
    {
        tokio::select! {
            result = future => Some(result),
            _ = self.cancellation.changed() => {
                if *self.cancellation.borrow() {
                    None
                } else {
                    // Spurious wakeup, but we'll return None to be safe
                    None
                }
            }
        }
    }

    /// Report progress.
    pub async fn report_progress(&self, percent: u8, message: Option<String>) {
        if let Some(ref sender) = self.progress_sender {
            let progress = JobProgress {
                job_id: self.metadata.id,
                percent: percent.min(100),
                message,
                timestamp: Utc::now(),
            };
            let _ = sender.send(progress).await;
        }
    }

    /// Log a message associated with this job.
    pub fn log_info(&self, message: &str) {
        tracing::info!(
            job_id = %self.metadata.id,
            job_type = %self.metadata.job_type,
            attempt = self.metadata.attempts,
            message
        );
    }

    /// Log a warning associated with this job.
    pub fn log_warn(&self, message: &str) {
        tracing::warn!(
            job_id = %self.metadata.id,
            job_type = %self.metadata.job_type,
            attempt = self.metadata.attempts,
            message
        );
    }

    /// Log an error associated with this job.
    pub fn log_error(&self, message: &str) {
        tracing::error!(
            job_id = %self.metadata.id,
            job_type = %self.metadata.job_type,
            attempt = self.metadata.attempts,
            message
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Job Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// The main trait that all background jobs must implement.
#[async_trait]
pub trait Job: Send + Sync {
    /// Returns the unique name/type identifier for this job.
    fn name(&self) -> &'static str;

    /// Execute the job.
    ///
    /// This method contains the main logic of the job. It receives a context
    /// that provides access to job metadata, cancellation support, and progress
    /// reporting.
    ///
    /// # Errors
    ///
    /// Return a `JobError` if the job fails. Use `JobError::retryable()` for
    /// transient failures that should be retried, and `JobError::fatal()` for
    /// permanent failures.
    async fn execute(&self, ctx: &JobContext) -> JobResult;

    /// Returns the default retry policy for this job type.
    ///
    /// Override this to customize retry behavior for specific job types.
    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::default()
    }

    /// Returns the default priority for this job type.
    fn priority(&self) -> JobPriority {
        JobPriority::Normal
    }

    /// Returns the default timeout for this job type (in seconds).
    fn timeout_secs(&self) -> Option<u64> {
        Some(300) // 5 minutes default
    }

    /// Called before the job starts executing.
    ///
    /// Override this to perform setup or validation.
    async fn before_execute(&self, _ctx: &JobContext) -> JobResult {
        Ok(())
    }

    /// Called after the job completes (success or failure).
    ///
    /// Override this to perform cleanup.
    async fn after_execute(&self, _ctx: &JobContext, _result: &JobResult) {
        // Default: do nothing
    }

    /// Serialize the job to JSON for storage.
    fn serialize(&self) -> Result<serde_json::Value>
    where
        Self: Serialize,
    {
        serde_json::to_value(self).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::SerializationError,
                "Failed to serialize job",
                e.to_string(),
            )
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id() {
        let id1 = JobId::new();
        let id2 = JobId::new();
        assert_ne!(id1, id2);

        let uuid = Uuid::new_v4();
        let id = JobId::from_uuid(uuid);
        assert_eq!(id.0, uuid);
    }

    #[test]
    fn test_job_status() {
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Dead.is_terminal());
        assert!(JobStatus::Cancelled.is_terminal());
        assert!(!JobStatus::Pending.is_terminal());
        assert!(!JobStatus::Running.is_terminal());

        assert!(JobStatus::Failed.can_retry());
        assert!(!JobStatus::Completed.can_retry());
    }

    #[test]
    fn test_job_priority_ordering() {
        assert!(JobPriority::Critical > JobPriority::High);
        assert!(JobPriority::High > JobPriority::Normal);
        assert!(JobPriority::Normal > JobPriority::Low);
    }

    #[test]
    fn test_backoff_fixed() {
        let backoff = BackoffStrategy::fixed(10);
        assert_eq!(backoff.delay_for_attempt(0), Duration::from_secs(10));
        assert_eq!(backoff.delay_for_attempt(5), Duration::from_secs(10));
    }

    #[test]
    fn test_backoff_exponential() {
        let backoff = BackoffStrategy::Exponential {
            initial_delay_secs: 1,
            max_delay_secs: 100,
            multiplier: 2.0,
        };
        assert_eq!(backoff.delay_for_attempt(0), Duration::from_secs(1));
        assert_eq!(backoff.delay_for_attempt(1), Duration::from_secs(2));
        assert_eq!(backoff.delay_for_attempt(2), Duration::from_secs(4));
        assert_eq!(backoff.delay_for_attempt(3), Duration::from_secs(8));
        // Should cap at max
        assert_eq!(backoff.delay_for_attempt(10), Duration::from_secs(100));
    }

    #[test]
    fn test_backoff_linear() {
        let backoff = BackoffStrategy::Linear {
            initial_delay_secs: 5,
            increment_secs: 3,
        };
        assert_eq!(backoff.delay_for_attempt(0), Duration::from_secs(5));
        assert_eq!(backoff.delay_for_attempt(1), Duration::from_secs(8));
        assert_eq!(backoff.delay_for_attempt(2), Duration::from_secs(11));
    }

    #[test]
    fn test_retry_policy() {
        let policy = RetryPolicy::with_retries(3);

        let retryable_error = JobError::retryable("temporary failure");
        let fatal_error = JobError::fatal("permanent failure");
        let now = Utc::now();

        // Should retry retryable errors
        assert!(policy.should_retry(0, &retryable_error, now));
        assert!(policy.should_retry(2, &retryable_error, now));
        assert!(!policy.should_retry(3, &retryable_error, now)); // max attempts reached

        // Should not retry fatal errors by default
        assert!(!policy.should_retry(0, &fatal_error, now));
    }

    #[test]
    fn test_retry_policy_any_error() {
        let mut policy = RetryPolicy::with_retries(3);
        policy.retry_on_any_error = true;

        let fatal_error = JobError::fatal("permanent failure");
        let now = Utc::now();

        // Should retry even fatal errors when retry_on_any_error is true
        assert!(policy.should_retry(0, &fatal_error, now));
    }

    #[test]
    fn test_job_metadata() {
        let mut metadata = JobMetadata::new("test_job")
            .with_priority(JobPriority::High)
            .with_max_attempts(5)
            .with_tag("important");

        assert_eq!(metadata.job_type, "test_job");
        assert_eq!(metadata.priority, JobPriority::High);
        assert_eq!(metadata.max_attempts, 5);
        assert!(metadata.tags.contains(&"important".to_string()));

        metadata.mark_running();
        assert_eq!(metadata.status, JobStatus::Running);
        assert_eq!(metadata.attempts, 1);

        metadata.mark_completed();
        assert_eq!(metadata.status, JobStatus::Completed);
        assert!(metadata.finished_at.is_some());
    }

    #[test]
    fn test_job_error() {
        let error = JobError::retryable("network timeout")
            .with_code("TIMEOUT")
            .with_context(serde_json::json!({"attempt": 3}));

        assert!(error.retryable);
        assert_eq!(error.code, Some("TIMEOUT".to_string()));
        assert!(error.context.is_some());
    }
}
