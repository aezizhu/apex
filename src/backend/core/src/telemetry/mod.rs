//! Telemetry: Comprehensive Logging, Tracing, and Metrics Infrastructure.
//!
//! This module provides a unified telemetry stack for the Apex orchestration engine:
//!
//! - **Logging**: Structured JSON/pretty logging with sensitive data redaction
//! - **Tracing**: Distributed tracing with OpenTelemetry/Jaeger/OTLP support
//! - **Metrics**: Prometheus metrics for request durations, connections, and errors
//!
//! # Example
//!
//! ```rust,no_run
//! use apex_core::telemetry::{TelemetryConfig, init_telemetry};
//!
//! let config = TelemetryConfig::default();
//! init_telemetry(&config).expect("Failed to initialize telemetry");
//! ```

pub mod logging;
pub mod metrics;
pub mod tracing;

pub use logging::{
    init_logging, LogFormat, LoggingConfig, RedactionConfig, RedactionPattern,
    SensitiveFieldRedactor,
};
pub use metrics::{
    init_metrics, MetricsConfig, MetricsRegistry, PrometheusExporter,
    // Metric types
    ActiveConnectionsGauge, ErrorCounter, RequestDurationHistogram,
    // Business metrics
    BusinessMetrics, TokenUsageMetrics, CostMetrics,
};
pub use tracing::{
    init_tracing, shutdown_tracing, TracingConfig, SpanBuilder, TraceContext,
    PropagationFormat, ExporterType,
};

use serde::Deserialize;

/// Unified telemetry configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TelemetryConfig {
    /// Service name for identification in traces and metrics
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Service version
    #[serde(default = "default_service_version")]
    pub service_version: String,

    /// Environment (development, staging, production)
    #[serde(default = "default_environment")]
    pub environment: String,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Tracing configuration
    #[serde(default)]
    pub tracing: TracingConfig,

    /// Metrics configuration
    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: default_service_name(),
            service_version: default_service_version(),
            environment: default_environment(),
            logging: LoggingConfig::default(),
            tracing: TracingConfig::default(),
            metrics: MetricsConfig::default(),
        }
    }
}

fn default_service_name() -> String {
    "apex-core".to_string()
}

fn default_service_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn default_environment() -> String {
    std::env::var("APEX_ENVIRONMENT").unwrap_or_else(|_| "development".to_string())
}

/// Initialize the complete telemetry stack.
///
/// This function sets up logging, tracing, and metrics based on the provided configuration.
/// It should be called once at application startup.
///
/// # Errors
///
/// Returns an error if any component fails to initialize.
pub fn init_telemetry(config: &TelemetryConfig) -> anyhow::Result<TelemetryHandle> {
    // Initialize metrics first (doesn't depend on anything)
    let metrics_handle = init_metrics(&config.metrics, &config.service_name)?;

    // Initialize tracing with OpenTelemetry
    let tracing_handle = init_tracing(&config.tracing, &config.service_name)?;

    // Initialize logging (integrates with tracing)
    init_logging(&config.logging, &config.environment)?;

    Ok(TelemetryHandle {
        metrics: metrics_handle,
        tracing: tracing_handle,
    })
}

/// Handle for managing telemetry lifecycle.
pub struct TelemetryHandle {
    /// Metrics handle for accessing the registry
    pub metrics: MetricsRegistry,
    /// Tracing handle for shutdown
    pub tracing: tracing::TracingHandle,
}

impl TelemetryHandle {
    /// Gracefully shutdown all telemetry components.
    pub fn shutdown(self) {
        // Shutdown tracing to flush remaining spans
        shutdown_tracing();

        // Metrics don't need explicit shutdown
        // Note: Using ::tracing to refer to the crate, not our local module
        ::tracing::info!("Telemetry shutdown complete");
    }
}

/// Request context for propagating telemetry data through the request lifecycle.
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique request identifier
    pub request_id: String,
    /// Trace ID for distributed tracing
    pub trace_id: Option<String>,
    /// Span ID for the current span
    pub span_id: Option<String>,
    /// User ID if authenticated
    pub user_id: Option<String>,
    /// Additional context fields
    pub extra: std::collections::HashMap<String, String>,
}

impl RequestContext {
    /// Create a new request context with a generated request ID.
    pub fn new() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            trace_id: TraceContext::current_trace_id(),
            span_id: TraceContext::current_span_id(),
            user_id: None,
            extra: std::collections::HashMap::new(),
        }
    }

    /// Create a request context with a specific request ID.
    pub fn with_request_id(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            trace_id: TraceContext::current_trace_id(),
            span_id: TraceContext::current_span_id(),
            user_id: None,
            extra: std::collections::HashMap::new(),
        }
    }

    /// Set the user ID.
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Add an extra context field.
    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_config_defaults() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "apex-core");
        assert!(!config.service_version.is_empty());
    }

    #[test]
    fn test_request_context_creation() {
        let ctx = RequestContext::new();
        assert!(!ctx.request_id.is_empty());
        assert!(ctx.user_id.is_none());
    }

    #[test]
    fn test_request_context_with_user() {
        let ctx = RequestContext::new()
            .with_user_id("user-123")
            .with_extra("tenant", "acme");

        assert_eq!(ctx.user_id, Some("user-123".to_string()));
        assert_eq!(ctx.extra.get("tenant"), Some(&"acme".to_string()));
    }
}
