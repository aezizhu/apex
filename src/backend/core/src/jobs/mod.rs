//! Background Job System for Apex Core.
//!
//! This module provides a comprehensive background job system with:
//!
//! - **Job Definitions**: Trait-based job definitions with retry and backoff support
//! - **Scheduler**: Cron-based, interval-based, and one-time job scheduling
//! - **Queue**: Redis-backed job queue with priority and dead letter support
//! - **Worker**: Concurrent job execution with graceful shutdown
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                         Background Job System                                │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐  │
//! │  │    Job      │    │  Scheduler  │    │    Queue    │    │   Worker    │  │
//! │  │ Definition  │───▶│  (Cron/     │───▶│  (Redis/    │───▶│  (Executor) │  │
//! │  │             │    │  Interval)  │    │  Priority)  │    │             │  │
//! │  └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘  │
//! │         │                  │                  │                  │         │
//! │         ▼                  ▼                  ▼                  ▼         │
//! │  ┌─────────────────────────────────────────────────────────────────────┐  │
//! │  │                        Built-in Jobs                                 │  │
//! │  │  • Cleanup Expired Approvals  • Aggregate Metrics                   │  │
//! │  │  • Send Usage Reports         • Cleanup Old Logs                    │  │
//! │  └─────────────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use apex_core::jobs::{
//!     Job, JobContext, JobResult, JobStatus, RetryPolicy,
//!     JobScheduler, ScheduleSpec, JobQueue, QueueConfig,
//!     JobWorker, WorkerConfig,
//! };
//!
//! // Define a custom job
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct MyJob {
//!     data: String,
//! }
//!
//! #[async_trait]
//! impl Job for MyJob {
//!     fn name(&self) -> &'static str { "my_job" }
//!
//!     async fn execute(&self, ctx: &JobContext) -> JobResult {
//!         // Do work...
//!         Ok(())
//!     }
//! }
//!
//! // Schedule jobs
//! let scheduler = JobScheduler::new(queue.clone());
//! scheduler.schedule_cron("0 * * * *", CleanupExpiredApprovalsJob::new()).await?;
//! scheduler.schedule_interval(Duration::from_secs(300), AggregateMetricsJob::new()).await?;
//!
//! // Start workers
//! let worker = JobWorker::new(queue, WorkerConfig::default());
//! worker.start().await?;
//! ```

pub mod job;
pub mod scheduler;
pub mod queue;
pub mod worker;

pub use job::{
    Job, JobContext, JobResult, JobStatus, JobError, JobMetadata,
    RetryPolicy, BackoffStrategy, JobPriority, JobId,
};
pub use scheduler::{
    JobScheduler, ScheduleSpec, ScheduledJob, CronSchedule, IntervalSchedule,
};
pub use queue::{
    JobQueue, QueueConfig, QueuedJob, DeadLetterQueue,
    QueueStats, QueueBackend, InMemoryQueueBackend, RedisQueueBackend,
};
pub use worker::{
    JobWorker, WorkerConfig, WorkerStats, WorkerHandle,
};

// Built-in jobs
mod builtin;
pub use builtin::{
    CleanupExpiredApprovalsJob,
    AggregateMetricsJob,
    SendUsageReportsJob,
    CleanupOldLogsJob,
};
