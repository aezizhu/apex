//! Job scheduling with cron and interval support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::{JobId, JobPriority};

/// Cron-based schedule specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronSchedule {
    /// Cron expression (e.g., "0 * * * *" for hourly)
    pub expression: String,
    /// Timezone for the cron schedule
    pub timezone: Option<String>,
}

impl CronSchedule {
    pub fn new(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            timezone: None,
        }
    }
}

/// Interval-based schedule specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntervalSchedule {
    /// Duration between executions
    pub interval: Duration,
    /// Whether to run immediately on start
    pub run_immediately: bool,
}

impl IntervalSchedule {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            run_immediately: false,
        }
    }

    pub fn with_immediate(mut self) -> Self {
        self.run_immediately = true;
        self
    }
}

/// Schedule specification for a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleSpec {
    /// Run once at a specific time
    Once(DateTime<Utc>),
    /// Run on a cron schedule
    Cron(CronSchedule),
    /// Run at fixed intervals
    Interval(IntervalSchedule),
}

/// A job that has been scheduled.
#[derive(Debug)]
pub struct ScheduledJob {
    /// Unique identifier
    pub id: JobId,
    /// The job name
    pub job_name: String,
    /// Schedule specification
    pub schedule: ScheduleSpec,
    /// Priority for scheduled executions
    pub priority: JobPriority,
    /// Whether this schedule is active
    pub active: bool,
    /// Next scheduled execution time
    pub next_run: Option<DateTime<Utc>>,
    /// Last execution time
    pub last_run: Option<DateTime<Utc>>,
    /// Number of times this schedule has executed
    pub run_count: u64,
}

/// Job scheduler managing scheduled and recurring jobs.
pub struct JobScheduler {
    scheduled_jobs: Arc<RwLock<Vec<ScheduledJob>>>,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl JobScheduler {
    /// Create a new job scheduler.
    pub fn new() -> Self {
        let (shutdown, _) = tokio::sync::watch::channel(false);
        Self {
            scheduled_jobs: Arc::new(RwLock::new(Vec::new())),
            shutdown,
        }
    }

    /// Schedule a job with a cron expression.
    pub async fn schedule_cron(
        &self,
        expression: &str,
        job_name: &str,
        priority: JobPriority,
    ) -> JobId {
        let id = JobId::new();
        let scheduled = ScheduledJob {
            id,
            job_name: job_name.to_string(),
            schedule: ScheduleSpec::Cron(CronSchedule::new(expression)),
            priority,
            active: true,
            next_run: None,
            last_run: None,
            run_count: 0,
        };
        self.scheduled_jobs.write().await.push(scheduled);
        id
    }

    /// Schedule a job at fixed intervals.
    pub async fn schedule_interval(
        &self,
        interval: Duration,
        job_name: &str,
        priority: JobPriority,
    ) -> JobId {
        let id = JobId::new();
        let scheduled = ScheduledJob {
            id,
            job_name: job_name.to_string(),
            schedule: ScheduleSpec::Interval(IntervalSchedule::new(interval)),
            priority,
            active: true,
            next_run: Some(Utc::now() + chrono::Duration::from_std(interval).unwrap_or_default()),
            last_run: None,
            run_count: 0,
        };
        self.scheduled_jobs.write().await.push(scheduled);
        id
    }

    /// Schedule a one-time job.
    pub async fn schedule_once(
        &self,
        at: DateTime<Utc>,
        job_name: &str,
        priority: JobPriority,
    ) -> JobId {
        let id = JobId::new();
        let scheduled = ScheduledJob {
            id,
            job_name: job_name.to_string(),
            schedule: ScheduleSpec::Once(at),
            priority,
            active: true,
            next_run: Some(at),
            last_run: None,
            run_count: 0,
        };
        self.scheduled_jobs.write().await.push(scheduled);
        id
    }

    /// Cancel a scheduled job.
    pub async fn cancel(&self, id: JobId) -> bool {
        let mut jobs = self.scheduled_jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            job.active = false;
            true
        } else {
            false
        }
    }

    /// List all scheduled jobs.
    pub async fn list(&self) -> Vec<JobId> {
        self.scheduled_jobs
            .read()
            .await
            .iter()
            .filter(|j| j.active)
            .map(|j| j.id)
            .collect()
    }

    /// Shutdown the scheduler.
    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_schedule_interval() {
        let scheduler = JobScheduler::new();
        let id = scheduler
            .schedule_interval(Duration::from_secs(60), "test_job", JobPriority::Normal)
            .await;
        let jobs = scheduler.list().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0], id);
    }

    #[tokio::test]
    async fn test_cancel_schedule() {
        let scheduler = JobScheduler::new();
        let id = scheduler
            .schedule_interval(Duration::from_secs(60), "test_job", JobPriority::Normal)
            .await;
        assert!(scheduler.cancel(id).await);
        let jobs = scheduler.list().await;
        assert_eq!(jobs.len(), 0);
    }
}
