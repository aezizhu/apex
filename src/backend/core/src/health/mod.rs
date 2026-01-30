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
            ],
        }
    }
}

/// Health service managing all health checks
pub struct HealthService {
    config: HealthConfig,
    checkers: Vec<Arc<dyn HealthChecker>>,
}

impl HealthService {
    pub fn new(config: HealthConfig) -> Self {
        Self {
            config,
            checkers: Vec::new(),
        }
    }

    pub fn register_checker(&mut self, checker: Arc<dyn HealthChecker>) {
        self.checkers.push(checker);
    }

    pub async fn check_health(&self) -> HealthReport {
        let mut components = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        for checker in &self.checkers {
            let result = tokio::time::timeout(
                self.config.check_timeout,
                checker.check(),
            )
            .await;

            let component_health = match result {
                Ok(health) => health,
                Err(_) => ComponentHealth::unhealthy(checker.name())
                    .with_message("Health check timed out"),
            };

            // Update overall status
            overall_status = overall_status.combine(component_health.status);
            components.push(component_health);
        }

        HealthReport::new()
            .with_service("apex-core")
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
}
