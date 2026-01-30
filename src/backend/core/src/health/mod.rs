//! Health Check System
//!
//! Comprehensive health checking for all system components with support for
//! Kubernetes probes (liveness, readiness) and detailed component health.

mod check;
mod checker;
mod routes;

pub use check::*;
pub use checker::*;
pub use routes::*;

use std::sync::Arc;
use std::time::Instant;

/// Health check configuration
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Timeout for individual health checks
    pub check_timeout: std::time::Duration,
    /// Whether to include detailed component info
    pub include_details: bool,
    /// Components to check
    pub components: Vec<String>,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_timeout: std::time::Duration::from_secs(5),
            include_details: true,
            components: vec![
                "database".into(),
                "redis".into(),
                "workers".into(),
                "disk_space".into(),
                "memory".into(),
                "database_backup".into(),
            ],
        }
    }
}

/// Health service managing all health checks
pub struct HealthService {
    config: HealthConfig,
    checkers: Vec<Arc<dyn HealthChecker>>,
    started_at: Instant,
}

impl HealthService {
    pub fn new(config: HealthConfig) -> Self {
        Self {
            config,
            checkers: Vec::new(),
            started_at: Instant::now(),
        }
    }

    pub fn register_checker(&mut self, checker: Arc<dyn HealthChecker>) {
        self.checkers.push(checker);
    }

    /// Run all health checks concurrently with timeout per check.
    pub async fn check_health(&self) -> HealthReport {
        let futures: Vec<_> = self
            .checkers
            .iter()
            .map(|checker| {
                let checker = checker.clone();
                let timeout = self.config.check_timeout;
                async move {
                    match tokio::time::timeout(timeout, checker.check()).await {
                        Ok(health) => health,
                        Err(_) => ComponentHealth::unhealthy(checker.name())
                            .with_message("Health check timed out"),
                    }
                }
            })
            .collect();

        let components = futures::future::join_all(futures).await;

        HealthReport::new()
            .with_service("apex-core")
            .with_uptime(self.started_at.elapsed())
            .with_components(components)
    }

    pub async fn is_ready(&self) -> bool {
        let report = self.check_health().await;
        report.is_operational()
    }

    pub async fn is_live(&self) -> bool {
        // Liveness is simpler - just check if the service is running
        true
    }

    /// Get the service uptime.
    pub fn uptime(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}
