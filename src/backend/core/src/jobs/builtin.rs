//! Built-in background jobs.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{Job, JobContext, JobResult, JobPriority, RetryPolicy};

/// Job: Clean up expired approval requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupExpiredApprovalsJob {
    /// Maximum age of approvals to keep (hours)
    pub max_age_hours: u64,
}

impl CleanupExpiredApprovalsJob {
    pub fn new() -> Self {
        Self {
            max_age_hours: 24,
        }
    }
}

impl Default for CleanupExpiredApprovalsJob {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Job for CleanupExpiredApprovalsJob {
    fn name(&self) -> &'static str {
        "cleanup_expired_approvals"
    }

    async fn execute(&self, ctx: &JobContext) -> JobResult {
        ctx.log_info(&format!(
            "Cleaning up approvals older than {} hours",
            self.max_age_hours
        ));
        // In production: DELETE FROM approvals WHERE expires_at < NOW()
        ctx.report_progress(100, Some("Cleanup complete".to_string())).await;
        Ok(())
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::with_retries(2)
    }
}

/// Job: Aggregate metrics from recent task executions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateMetricsJob {
    /// Time window for aggregation (minutes)
    pub window_minutes: u64,
}

impl AggregateMetricsJob {
    pub fn new() -> Self {
        Self {
            window_minutes: 60,
        }
    }
}

impl Default for AggregateMetricsJob {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Job for AggregateMetricsJob {
    fn name(&self) -> &'static str {
        "aggregate_metrics"
    }

    async fn execute(&self, ctx: &JobContext) -> JobResult {
        ctx.log_info(&format!(
            "Aggregating metrics for the last {} minutes",
            self.window_minutes
        ));
        // In production: Run aggregation queries and update summary tables
        ctx.report_progress(100, Some("Metrics aggregated".to_string())).await;
        Ok(())
    }

    fn priority(&self) -> JobPriority {
        JobPriority::Low
    }
}

/// Job: Send usage reports via email/webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendUsageReportsJob {
    /// Report period (e.g., "daily", "weekly")
    pub period: String,
}

impl SendUsageReportsJob {
    pub fn new(period: impl Into<String>) -> Self {
        Self {
            period: period.into(),
        }
    }
}

#[async_trait]
impl Job for SendUsageReportsJob {
    fn name(&self) -> &'static str {
        "send_usage_reports"
    }

    async fn execute(&self, ctx: &JobContext) -> JobResult {
        ctx.log_info(&format!("Generating {} usage report", self.period));
        // In production: Generate report, send via configured channels
        ctx.report_progress(100, Some("Report sent".to_string())).await;
        Ok(())
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::exponential_backoff(5)
    }
}

/// Job: Clean up old log entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupOldLogsJob {
    /// Maximum age of logs to keep (days)
    pub retention_days: u64,
}

impl CleanupOldLogsJob {
    pub fn new() -> Self {
        Self {
            retention_days: 30,
        }
    }
}

impl Default for CleanupOldLogsJob {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Job for CleanupOldLogsJob {
    fn name(&self) -> &'static str {
        "cleanup_old_logs"
    }

    async fn execute(&self, ctx: &JobContext) -> JobResult {
        ctx.log_info(&format!(
            "Cleaning up logs older than {} days",
            self.retention_days
        ));
        // In production: DELETE FROM logs WHERE created_at < NOW() - interval
        ctx.report_progress(100, Some("Log cleanup complete".to_string())).await;
        Ok(())
    }

    fn priority(&self) -> JobPriority {
        JobPriority::Low
    }

    fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::with_retries(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_approvals_default() {
        let job = CleanupExpiredApprovalsJob::new();
        assert_eq!(job.max_age_hours, 24);
        assert_eq!(job.name(), "cleanup_expired_approvals");
    }

    #[test]
    fn test_aggregate_metrics_default() {
        let job = AggregateMetricsJob::new();
        assert_eq!(job.window_minutes, 60);
        assert_eq!(job.priority(), JobPriority::Low);
    }

    #[test]
    fn test_cleanup_logs_default() {
        let job = CleanupOldLogsJob::new();
        assert_eq!(job.retention_days, 30);
        assert_eq!(job.name(), "cleanup_old_logs");
    }
}
