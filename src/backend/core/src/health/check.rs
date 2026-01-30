//! Health check definitions and status types.
//!
//! This module provides:
//! - `HealthStatus` enum representing component health states
//! - `ComponentHealth` struct for individual component health reports
//! - `HealthReport` struct for aggregated system health
//!
//! # Health Status Semantics
//!
//! - **Healthy**: Component is fully operational
//! - **Degraded**: Component is operational but with issues (e.g., high latency)
//! - **Unhealthy**: Component is not operational
//!
//! # Example
//!
//! ```rust,ignore
//! use apex_core::health::{HealthStatus, ComponentHealth, HealthReport};
//!
//! let db_health = ComponentHealth::healthy("database")
//!     .with_message("Connected to PostgreSQL")
//!     .with_latency_ms(5);
//!
//! let report = HealthReport::new()
//!     .with_component(db_health);
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════════
// Health Status
// ═══════════════════════════════════════════════════════════════════════════════

/// Health status of a component or the entire system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Component is fully operational
    Healthy,
    /// Component is operational but with degraded performance or partial functionality
    Degraded,
    /// Component is not operational
    Unhealthy,
}

impl HealthStatus {
    /// Check if the status is healthy.
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if the status is at least partially operational.
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Combine two statuses, returning the worse one.
    pub fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unhealthy, _) | (_, Self::Unhealthy) => Self::Unhealthy,
            (Self::Degraded, _) | (_, Self::Degraded) => Self::Degraded,
            _ => Self::Healthy,
        }
    }

    /// Convert to HTTP status code.
    pub fn to_http_status(&self) -> u16 {
        match self {
            Self::Healthy => 200,
            Self::Degraded => 200, // Still operational
            Self::Unhealthy => 503,
        }
    }

    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Healthy => "Component is fully operational",
            Self::Degraded => "Component is operational with degraded performance",
            Self::Unhealthy => "Component is not operational",
        }
    }
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Healthy
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Component Health
// ═══════════════════════════════════════════════════════════════════════════════

/// Health information for a single component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name
    pub name: String,

    /// Health status
    pub status: HealthStatus,

    /// Optional message describing the current state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Latency of health check in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,

    /// Last check timestamp
    pub checked_at: DateTime<Utc>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Error details (only present if unhealthy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ComponentHealth {
    /// Create a new healthy component.
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            message: None,
            latency_ms: None,
            checked_at: Utc::now(),
            metadata: HashMap::new(),
            error: None,
        }
    }

    /// Create a new degraded component.
    pub fn degraded(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Degraded,
            message: None,
            latency_ms: None,
            checked_at: Utc::now(),
            metadata: HashMap::new(),
            error: None,
        }
    }

    /// Create a new unhealthy component.
    pub fn unhealthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            message: None,
            latency_ms: None,
            checked_at: Utc::now(),
            metadata: HashMap::new(),
            error: None,
        }
    }

    /// Create from a check result.
    pub fn from_result<E: std::fmt::Display>(
        name: impl Into<String>,
        result: Result<(), E>,
        latency: Duration,
    ) -> Self {
        match result {
            Ok(()) => Self::healthy(name).with_latency(latency),
            Err(e) => Self::unhealthy(name)
                .with_error(e.to_string())
                .with_latency(latency),
        }
    }

    /// Add a message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Add latency from Duration.
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency_ms = Some(latency.as_millis() as u64);
        self
    }

    /// Add latency in milliseconds.
    pub fn with_latency_ms(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    /// Add an error message (sets status to Unhealthy).
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.status = HealthStatus::Unhealthy;
        self.error = Some(error.into());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), v);
        }
        self
    }

    /// Set the status.
    pub fn with_status(mut self, status: HealthStatus) -> Self {
        self.status = status;
        self
    }

    /// Check if the component is healthy.
    pub fn is_healthy(&self) -> bool {
        self.status.is_healthy()
    }

    /// Check if latency exceeds a threshold (and should be considered degraded).
    pub fn check_latency_threshold(&mut self, threshold_ms: u64) {
        if let Some(latency) = self.latency_ms {
            if latency > threshold_ms && self.status == HealthStatus::Healthy {
                self.status = HealthStatus::Degraded;
                self.message = Some(format!(
                    "High latency detected: {}ms (threshold: {}ms)",
                    latency, threshold_ms
                ));
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Health Report
// ═══════════════════════════════════════════════════════════════════════════════

/// Aggregated health report for the entire system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall system status
    pub status: HealthStatus,

    /// Service name
    pub service: String,

    /// Service version
    pub version: String,

    /// When this report was generated
    pub timestamp: DateTime<Utc>,

    /// Uptime in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_secs: Option<u64>,

    /// Individual component health reports
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentHealth>,

    /// Summary counts
    pub summary: HealthSummary,

    /// Additional system metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl HealthReport {
    /// Create a new health report.
    pub fn new() -> Self {
        Self {
            status: HealthStatus::Healthy,
            service: "apex-core".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: Utc::now(),
            uptime_secs: None,
            components: Vec::new(),
            summary: HealthSummary::default(),
            metadata: HashMap::new(),
        }
    }

    /// Add a component health report.
    pub fn with_component(mut self, component: ComponentHealth) -> Self {
        self.status = self.status.combine(component.status);
        self.components.push(component);
        self.update_summary();
        self
    }

    /// Add multiple component health reports.
    pub fn with_components(mut self, components: Vec<ComponentHealth>) -> Self {
        for component in components {
            self.status = self.status.combine(component.status);
            self.components.push(component);
        }
        self.update_summary();
        self
    }

    /// Set the service name.
    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = service.into();
        self
    }

    /// Set the uptime.
    pub fn with_uptime(mut self, uptime: Duration) -> Self {
        self.uptime_secs = Some(uptime.as_secs());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), v);
        }
        self
    }

    /// Get the overall status.
    pub fn status(&self) -> HealthStatus {
        self.status
    }

    /// Check if the system is healthy.
    pub fn is_healthy(&self) -> bool {
        self.status.is_healthy()
    }

    /// Check if the system is operational (healthy or degraded).
    pub fn is_operational(&self) -> bool {
        self.status.is_operational()
    }

    /// Get the HTTP status code for this report.
    pub fn http_status(&self) -> u16 {
        self.status.to_http_status()
    }

    /// Get a specific component by name.
    pub fn get_component(&self, name: &str) -> Option<&ComponentHealth> {
        self.components.iter().find(|c| c.name == name)
    }

    /// Update the summary counts.
    fn update_summary(&mut self) {
        let mut healthy = 0;
        let mut degraded = 0;
        let mut unhealthy = 0;

        for component in &self.components {
            match component.status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Degraded => degraded += 1,
                HealthStatus::Unhealthy => unhealthy += 1,
            }
        }

        self.summary = HealthSummary {
            total: self.components.len(),
            healthy,
            degraded,
            unhealthy,
        };
    }
}

impl Default for HealthReport {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Health Summary
// ═══════════════════════════════════════════════════════════════════════════════

/// Summary counts for health report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthSummary {
    /// Total number of components
    pub total: usize,
    /// Number of healthy components
    pub healthy: usize,
    /// Number of degraded components
    pub degraded: usize,
    /// Number of unhealthy components
    pub unhealthy: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Readiness and Liveness
// ═══════════════════════════════════════════════════════════════════════════════

/// Liveness probe response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivenessResponse {
    /// Whether the service is alive
    pub alive: bool,
    /// Service name
    pub service: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl LivenessResponse {
    /// Create a new liveness response.
    pub fn new(alive: bool) -> Self {
        Self {
            alive,
            service: "apex-core".to_string(),
            timestamp: Utc::now(),
        }
    }

    /// Create a healthy liveness response.
    pub fn alive() -> Self {
        Self::new(true)
    }
}

impl Default for LivenessResponse {
    fn default() -> Self {
        Self::alive()
    }
}

/// Readiness probe response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    /// Whether the service is ready to accept traffic
    pub ready: bool,
    /// Service name
    pub service: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Optional reason if not ready
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// List of unready components
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unready_components: Vec<String>,
}

impl ReadinessResponse {
    /// Create a new readiness response.
    pub fn new(ready: bool) -> Self {
        Self {
            ready,
            service: "apex-core".to_string(),
            timestamp: Utc::now(),
            reason: None,
            unready_components: Vec::new(),
        }
    }

    /// Create a ready response.
    pub fn ready() -> Self {
        Self::new(true)
    }

    /// Create a not ready response.
    pub fn not_ready(reason: impl Into<String>) -> Self {
        Self {
            ready: false,
            service: "apex-core".to_string(),
            timestamp: Utc::now(),
            reason: Some(reason.into()),
            unready_components: Vec::new(),
        }
    }

    /// Add unready component.
    pub fn with_unready_component(mut self, component: impl Into<String>) -> Self {
        self.unready_components.push(component.into());
        self
    }

    /// Build from health report.
    pub fn from_health_report(report: &HealthReport) -> Self {
        if report.is_operational() {
            Self::ready()
        } else {
            let unready: Vec<String> = report
                .components
                .iter()
                .filter(|c| !c.status.is_operational())
                .map(|c| c.name.clone())
                .collect();

            Self {
                ready: false,
                service: report.service.clone(),
                timestamp: Utc::now(),
                reason: Some("One or more components are unhealthy".to_string()),
                unready_components: unready,
            }
        }
    }
}

impl Default for ReadinessResponse {
    fn default() -> Self {
        Self::ready()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_combine() {
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Healthy),
            HealthStatus::Healthy
        );
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Degraded),
            HealthStatus::Degraded
        );
        assert_eq!(
            HealthStatus::Healthy.combine(HealthStatus::Unhealthy),
            HealthStatus::Unhealthy
        );
        assert_eq!(
            HealthStatus::Degraded.combine(HealthStatus::Degraded),
            HealthStatus::Degraded
        );
        assert_eq!(
            HealthStatus::Degraded.combine(HealthStatus::Unhealthy),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn test_component_health_creation() {
        let healthy = ComponentHealth::healthy("db")
            .with_message("Connected")
            .with_latency_ms(5);

        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert_eq!(healthy.message, Some("Connected".to_string()));
        assert_eq!(healthy.latency_ms, Some(5));
    }

    #[test]
    fn test_component_health_degraded() {
        let degraded = ComponentHealth::degraded("cache")
            .with_message("High latency");

        assert_eq!(degraded.status, HealthStatus::Degraded);
        assert!(!degraded.is_healthy());
    }

    #[test]
    fn test_component_health_unhealthy() {
        let unhealthy = ComponentHealth::unhealthy("external-api")
            .with_error("Connection refused");

        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
        assert!(unhealthy.error.is_some());
    }

    #[test]
    fn test_health_report_aggregation() {
        let report = HealthReport::new()
            .with_component(ComponentHealth::healthy("db"))
            .with_component(ComponentHealth::healthy("cache"))
            .with_component(ComponentHealth::degraded("external"));

        assert_eq!(report.status, HealthStatus::Degraded);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.healthy, 2);
        assert_eq!(report.summary.degraded, 1);
        assert!(report.is_operational());
    }

    #[test]
    fn test_health_report_unhealthy() {
        let report = HealthReport::new()
            .with_component(ComponentHealth::healthy("db"))
            .with_component(ComponentHealth::unhealthy("cache"));

        assert_eq!(report.status, HealthStatus::Unhealthy);
        assert!(!report.is_operational());
        assert_eq!(report.http_status(), 503);
    }

    #[test]
    fn test_readiness_from_report() {
        let healthy_report = HealthReport::new()
            .with_component(ComponentHealth::healthy("db"));

        let readiness = ReadinessResponse::from_health_report(&healthy_report);
        assert!(readiness.ready);

        let unhealthy_report = HealthReport::new()
            .with_component(ComponentHealth::unhealthy("db"));

        let readiness = ReadinessResponse::from_health_report(&unhealthy_report);
        assert!(!readiness.ready);
        assert!(readiness.unready_components.contains(&"db".to_string()));
    }

    #[test]
    fn test_latency_threshold() {
        let mut component = ComponentHealth::healthy("db").with_latency_ms(500);

        // Check against 100ms threshold
        component.check_latency_threshold(100);

        assert_eq!(component.status, HealthStatus::Degraded);
        assert!(component.message.unwrap().contains("High latency"));
    }

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus::Healthy;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"healthy\"");

        let status: HealthStatus = serde_json::from_str("\"degraded\"").unwrap();
        assert_eq!(status, HealthStatus::Degraded);
    }

    #[test]
    fn test_health_status_is_healthy() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Degraded.is_healthy());
        assert!(!HealthStatus::Unhealthy.is_healthy());
    }

    #[test]
    fn test_health_status_is_operational() {
        assert!(HealthStatus::Healthy.is_operational());
        assert!(HealthStatus::Degraded.is_operational());
        assert!(!HealthStatus::Unhealthy.is_operational());
    }

    #[test]
    fn test_health_status_http_codes() {
        assert_eq!(HealthStatus::Healthy.http_status_code(), 200);
        assert_eq!(HealthStatus::Degraded.http_status_code(), 200);
        assert_eq!(HealthStatus::Unhealthy.http_status_code(), 503);
    }

    #[test]
    fn test_health_status_description() {
        assert!(!HealthStatus::Healthy.description().is_empty());
        assert!(!HealthStatus::Degraded.description().is_empty());
        assert!(!HealthStatus::Unhealthy.description().is_empty());
    }

    #[test]
    fn test_health_status_display() {
        assert_eq!(format!("{}", HealthStatus::Healthy), "healthy");
        assert_eq!(format!("{}", HealthStatus::Degraded), "degraded");
        assert_eq!(format!("{}", HealthStatus::Unhealthy), "unhealthy");
    }

    #[test]
    fn test_health_status_default() {
        assert_eq!(HealthStatus::default(), HealthStatus::Healthy);
    }

    #[test]
    fn test_health_status_combine_symmetric() {
        assert_eq!(
            HealthStatus::Unhealthy.combine(HealthStatus::Healthy),
            HealthStatus::Unhealthy
        );
        assert_eq!(
            HealthStatus::Unhealthy.combine(HealthStatus::Degraded),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn test_component_health_with_metadata() {
        let health = ComponentHealth::healthy("db")
            .with_metadata("version", "15.2")
            .with_metadata("connections", 10u64);
        assert_eq!(health.metadata.len(), 2);
    }

    #[test]
    fn test_component_health_with_latency() {
        let health = ComponentHealth::healthy("cache")
            .with_latency(std::time::Duration::from_millis(42));
        assert_eq!(health.latency_ms, Some(42));
    }

    #[test]
    fn test_component_health_with_error_sets_unhealthy() {
        let health = ComponentHealth::healthy("svc")
            .with_error("connection refused");
        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert!(health.error.is_some());
    }

    #[test]
    fn test_component_health_with_status() {
        let health = ComponentHealth::healthy("svc")
            .with_status(HealthStatus::Degraded);
        assert_eq!(health.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_component_health_from_result_ok() {
        let result: std::result::Result<(), String> = Ok(());
        let health = ComponentHealth::from_result("db", result);
        assert!(health.is_healthy());
    }

    #[test]
    fn test_component_health_from_result_err() {
        let result: std::result::Result<(), String> = Err("conn failed".into());
        let health = ComponentHealth::from_result("db", result);
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_latency_threshold_below() {
        let mut health = ComponentHealth::healthy("db").with_latency_ms(50);
        health.check_latency_threshold(100);
        assert_eq!(health.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_latency_threshold_no_latency() {
        let mut health = ComponentHealth::healthy("db");
        health.check_latency_threshold(100);
        assert_eq!(health.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_health_report_default() {
        let report = HealthReport::new();
        assert_eq!(report.status, HealthStatus::Healthy);
        assert!(report.components.is_empty());
    }

    #[test]
    fn test_health_report_with_service() {
        let report = HealthReport::new().with_service("my-service");
        assert_eq!(report.service, "my-service");
    }

    #[test]
    fn test_health_report_with_uptime() {
        let report = HealthReport::new().with_uptime(std::time::Duration::from_secs(3600));
        assert_eq!(report.uptime_seconds, Some(3600));
    }

    #[test]
    fn test_health_report_with_metadata() {
        let report = HealthReport::new().with_metadata("env", "prod");
        assert!(report.metadata.contains_key("env"));
    }

    #[test]
    fn test_health_report_get_component() {
        let report = HealthReport::new()
            .with_component(ComponentHealth::healthy("db"))
            .with_component(ComponentHealth::healthy("cache"));
        assert!(report.get_component("db").is_some());
        assert!(report.get_component("cache").is_some());
        assert!(report.get_component("nonexistent").is_none());
    }

    #[test]
    fn test_health_report_with_components() {
        let components = vec![
            ComponentHealth::healthy("a"),
            ComponentHealth::degraded("b"),
            ComponentHealth::unhealthy("c"),
        ];
        let report = HealthReport::new().with_components(components);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.healthy, 1);
        assert_eq!(report.summary.degraded, 1);
        assert_eq!(report.summary.unhealthy, 1);
        assert_eq!(report.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_liveness_response_alive() {
        let resp = LivenessResponse::alive();
        assert!(resp.alive);
    }

    #[test]
    fn test_liveness_response_default() {
        let resp = LivenessResponse::default();
        assert!(resp.alive);
    }

    #[test]
    fn test_liveness_response_dead() {
        let resp = LivenessResponse::dead();
        assert!(!resp.alive);
    }

    #[test]
    fn test_readiness_response_ready() {
        let resp = ReadinessResponse::ready();
        assert!(resp.ready);
        assert!(resp.reason.is_none());
    }

    #[test]
    fn test_readiness_response_not_ready() {
        let resp = ReadinessResponse::not_ready("Still starting up");
        assert!(!resp.ready);
        assert_eq!(resp.reason, Some("Still starting up".to_string()));
    }

    #[test]
    fn test_readiness_with_unready_component() {
        let resp = ReadinessResponse::not_ready("Deps unhealthy")
            .with_unready_component("db")
            .with_unready_component("cache");
        assert_eq!(resp.unready_components.len(), 2);
    }

    #[test]
    fn test_readiness_response_default() {
        let resp = ReadinessResponse::default();
        assert!(resp.ready);
    }

    #[test]
    fn test_readiness_from_degraded_report() {
        let report = HealthReport::new()
            .with_component(ComponentHealth::degraded("cache"));
        let resp = ReadinessResponse::from_health_report(&report);
        // Degraded is still operational, so should be ready
        assert!(resp.ready);
    }
}
