//! Prometheus Metrics for Request Durations, Connections, and Errors.
//!
//! This module provides comprehensive metrics collection with:
//!
//! - Request duration histograms with configurable buckets
//! - Active connections gauge for connection pool monitoring
//! - Error counters by type/code for observability
//! - Custom business metrics (tokens, costs, etc.)
//!
//! # Example
//!
//! ```rust,no_run
//! use apex_core::telemetry::metrics::{MetricsRegistry, RequestDurationHistogram, ErrorCounter};
//!
//! // Record request duration
//! RequestDurationHistogram::record("http", "POST", "/api/tasks", 200, 0.125);
//!
//! // Increment error counter
//! ErrorCounter::increment("validation", "invalid_input");
//! ```

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Global metrics registry.
static METRICS_REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();

/// Metrics configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    /// Whether metrics collection is enabled
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,

    /// Prometheus exporter endpoint (e.g., "0.0.0.0:9090")
    #[serde(default = "default_metrics_endpoint")]
    pub endpoint: String,

    /// Histogram buckets for request durations (in seconds)
    #[serde(default = "default_duration_buckets")]
    pub duration_buckets: Vec<f64>,

    /// Global labels to add to all metrics
    #[serde(default)]
    pub global_labels: HashMap<String, String>,

    /// Whether to enable default process metrics
    #[serde(default = "default_enable_process_metrics")]
    pub enable_process_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: default_metrics_enabled(),
            endpoint: default_metrics_endpoint(),
            duration_buckets: default_duration_buckets(),
            global_labels: HashMap::new(),
            enable_process_metrics: default_enable_process_metrics(),
        }
    }
}

// Default value functions
fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_endpoint() -> String {
    "0.0.0.0:9090".to_string()
}

fn default_duration_buckets() -> Vec<f64> {
    vec![
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ]
}

fn default_enable_process_metrics() -> bool {
    true
}

/// Central metrics registry for managing all metrics.
pub struct MetricsRegistry {
    prometheus_handle: Option<PrometheusHandle>,
}

impl std::fmt::Debug for MetricsRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsRegistry")
            .field("prometheus_handle", &self.prometheus_handle.is_some())
            .finish()
    }
}

impl MetricsRegistry {
    /// Get the global metrics registry.
    pub fn global() -> &'static MetricsRegistry {
        METRICS_REGISTRY.get_or_init(|| MetricsRegistry {
            prometheus_handle: None,
        })
    }

    /// Render all metrics in Prometheus text format.
    pub fn render(&self) -> String {
        self.prometheus_handle
            .as_ref()
            .map(|h| h.render())
            .unwrap_or_default()
    }
}

/// Prometheus exporter for serving metrics via HTTP.
pub struct PrometheusExporter {
    handle: PrometheusHandle,
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter.
    pub fn new(handle: PrometheusHandle) -> Self {
        Self { handle }
    }

    /// Render metrics in Prometheus text format.
    pub fn render(&self) -> String {
        self.handle.render()
    }
}

/// Initialize the metrics subsystem.
///
/// # Arguments
///
/// * `config` - Metrics configuration
/// * `service_name` - Name of the service for identification
///
/// # Errors
///
/// Returns an error if metrics initialization fails.
pub fn init_metrics(config: &MetricsConfig, service_name: &str) -> anyhow::Result<MetricsRegistry> {
    if !config.enabled {
        return Ok(MetricsRegistry {
            prometheus_handle: None,
        });
    }

    // Build the Prometheus recorder
    let mut builder = PrometheusBuilder::new();

    // Add global labels
    for (key, value) in &config.global_labels {
        builder = builder.add_global_label(key, value);
    }

    // Set custom buckets for histograms
    builder = builder.set_buckets(&config.duration_buckets)?;

    // Install the recorder and get the handle
    let handle = builder.install_recorder()?;

    // Register metric descriptions
    register_metric_descriptions();

    // Store the registry globally
    let registry = MetricsRegistry {
        prometheus_handle: Some(handle),
    };

    let _ = METRICS_REGISTRY.set(MetricsRegistry {
        prometheus_handle: None, // We'll use the one in the returned registry
    });

    tracing::info!(
        service_name = %service_name,
        endpoint = %config.endpoint,
        "Metrics initialized"
    );

    Ok(registry)
}

/// Register all metric descriptions.
fn register_metric_descriptions() {
    // Request metrics
    describe_histogram!(
        "http_request_duration_seconds",
        "HTTP request duration in seconds"
    );
    describe_counter!("http_requests_total", "Total number of HTTP requests");
    describe_counter!("http_request_errors_total", "Total number of HTTP errors");

    // Connection metrics
    describe_gauge!(
        "active_connections",
        "Number of currently active connections"
    );
    describe_gauge!(
        "connection_pool_size",
        "Current size of the connection pool"
    );
    describe_gauge!(
        "connection_pool_available",
        "Available connections in the pool"
    );

    // Error metrics
    describe_counter!("errors_total", "Total number of errors by type");

    // Task metrics
    describe_counter!("apex_tasks_total", "Total number of tasks processed");
    describe_counter!(
        "apex_tasks_completed",
        "Total number of tasks completed successfully"
    );
    describe_counter!("apex_tasks_failed", "Total number of tasks that failed");
    describe_histogram!(
        "apex_task_duration_seconds",
        "Task execution duration in seconds"
    );

    // Agent metrics
    describe_gauge!("apex_active_agents", "Number of currently active agents");
    describe_counter!("apex_agent_spawns_total", "Total number of agents spawned");
    describe_histogram!(
        "apex_agent_latency_seconds",
        "Agent response latency in seconds"
    );

    // Token/Cost metrics
    describe_counter!("apex_tokens_total", "Total tokens consumed");
    describe_counter!(
        "apex_tokens_input_total",
        "Total input tokens consumed"
    );
    describe_counter!(
        "apex_tokens_output_total",
        "Total output tokens consumed"
    );
    describe_counter!("apex_cost_total_microdollars", "Total cost in microdollars");

    // Tool metrics
    describe_counter!("apex_tool_calls_total", "Total tool calls made");
    describe_histogram!(
        "apex_tool_latency_seconds",
        "Tool execution latency in seconds"
    );

    // Queue metrics
    describe_gauge!("apex_queue_depth", "Number of tasks in the queue");
    describe_gauge!(
        "apex_worker_utilization",
        "Worker pool utilization (0-1)"
    );

    // Circuit breaker metrics
    describe_counter!(
        "apex_circuit_breaker_trips_total",
        "Total circuit breaker trips"
    );
    describe_gauge!(
        "apex_circuit_breaker_state",
        "Circuit breaker state (0=closed, 1=half-open, 2=open)"
    );

    // Contract metrics
    describe_counter!(
        "apex_contract_violations_total",
        "Total contract violations"
    );
}

/// Request duration histogram for HTTP requests.
pub struct RequestDurationHistogram;

impl RequestDurationHistogram {
    /// Record a request duration.
    pub fn record(
        protocol: &str,
        method: &str,
        path: &str,
        status_code: u16,
        duration_seconds: f64,
    ) {
        histogram!(
            "http_request_duration_seconds",
            "protocol" => protocol.to_string(),
            "method" => method.to_string(),
            "path" => path.to_string(),
            "status_code" => status_code.to_string(),
        )
        .record(duration_seconds);

        counter!(
            "http_requests_total",
            "protocol" => protocol.to_string(),
            "method" => method.to_string(),
            "path" => path.to_string(),
            "status_code" => status_code.to_string(),
        )
        .increment(1);

        // Track errors separately
        if status_code >= 400 {
            counter!(
                "http_request_errors_total",
                "protocol" => protocol.to_string(),
                "method" => method.to_string(),
                "path" => path.to_string(),
                "status_code" => status_code.to_string(),
            )
            .increment(1);
        }
    }

    /// Start timing a request, returns a guard that records duration on drop.
    pub fn start(protocol: &str, method: &str, path: &str) -> RequestTimer {
        RequestTimer {
            start: Instant::now(),
            protocol: protocol.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            status_code: None,
        }
    }
}

/// Timer for measuring request durations.
pub struct RequestTimer {
    start: Instant,
    protocol: String,
    method: String,
    path: String,
    status_code: Option<u16>,
}

impl RequestTimer {
    /// Set the status code for the response.
    pub fn set_status(&mut self, status_code: u16) {
        self.status_code = Some(status_code);
    }

    /// Finish timing and record the duration.
    pub fn finish(self, status_code: u16) {
        let duration = self.start.elapsed().as_secs_f64();
        RequestDurationHistogram::record(
            &self.protocol,
            &self.method,
            &self.path,
            status_code,
            duration,
        );
    }
}

impl Drop for RequestTimer {
    fn drop(&mut self) {
        if let Some(status_code) = self.status_code {
            let duration = self.start.elapsed().as_secs_f64();
            RequestDurationHistogram::record(
                &self.protocol,
                &self.method,
                &self.path,
                status_code,
                duration,
            );
        }
    }
}

/// Active connections gauge for monitoring connection pools.
pub struct ActiveConnectionsGauge;

impl ActiveConnectionsGauge {
    /// Set the number of active connections.
    pub fn set(pool_name: &str, count: u64) {
        gauge!("active_connections", "pool" => pool_name.to_string()).set(count as f64);
    }

    /// Increment the active connection count.
    pub fn increment(pool_name: &str) {
        gauge!("active_connections", "pool" => pool_name.to_string()).increment(1.0);
    }

    /// Decrement the active connection count.
    pub fn decrement(pool_name: &str) {
        gauge!("active_connections", "pool" => pool_name.to_string()).decrement(1.0);
    }

    /// Set the total pool size.
    pub fn set_pool_size(pool_name: &str, size: u64) {
        gauge!("connection_pool_size", "pool" => pool_name.to_string()).set(size as f64);
    }

    /// Set the available connections in the pool.
    pub fn set_available(pool_name: &str, available: u64) {
        gauge!("connection_pool_available", "pool" => pool_name.to_string()).set(available as f64);
    }
}

/// Error counter for tracking errors by type.
pub struct ErrorCounter;

impl ErrorCounter {
    /// Increment the error counter for a specific error type.
    pub fn increment(error_type: &str, error_code: &str) {
        counter!(
            "errors_total",
            "type" => error_type.to_string(),
            "code" => error_code.to_string(),
        )
        .increment(1);
    }

    /// Increment with additional context.
    pub fn increment_with_context(error_type: &str, error_code: &str, service: &str) {
        counter!(
            "errors_total",
            "type" => error_type.to_string(),
            "code" => error_code.to_string(),
            "service" => service.to_string(),
        )
        .increment(1);
    }
}

/// Business metrics for tracking token usage.
pub struct TokenUsageMetrics;

impl TokenUsageMetrics {
    /// Record token usage for a task.
    pub fn record(model: &str, input_tokens: u64, output_tokens: u64) {
        counter!(
            "apex_tokens_total",
            "model" => model.to_string(),
        )
        .increment(input_tokens + output_tokens);

        counter!(
            "apex_tokens_input_total",
            "model" => model.to_string(),
        )
        .increment(input_tokens);

        counter!(
            "apex_tokens_output_total",
            "model" => model.to_string(),
        )
        .increment(output_tokens);
    }
}

/// Business metrics for tracking costs.
pub struct CostMetrics;

impl CostMetrics {
    /// Record cost in dollars (stored as microdollars for precision).
    pub fn record(model: &str, cost_dollars: f64) {
        let microdollars = (cost_dollars * 1_000_000.0) as u64;
        counter!(
            "apex_cost_total_microdollars",
            "model" => model.to_string(),
        )
        .increment(microdollars);
    }

    /// Record cost in microdollars directly.
    pub fn record_microdollars(model: &str, microdollars: u64) {
        counter!(
            "apex_cost_total_microdollars",
            "model" => model.to_string(),
        )
        .increment(microdollars);
    }
}

/// Comprehensive business metrics collection.
pub struct BusinessMetrics;

impl BusinessMetrics {
    /// Record a task completion with all associated metrics.
    pub fn record_task_completed(
        task_id: &str,
        agent_id: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost_dollars: f64,
        duration_seconds: f64,
    ) {
        // Task counters
        counter!("apex_tasks_total", "status" => "completed").increment(1);
        counter!("apex_tasks_completed").increment(1);

        // Duration histogram
        histogram!("apex_task_duration_seconds", "model" => model.to_string())
            .record(duration_seconds);

        // Token metrics
        TokenUsageMetrics::record(model, input_tokens, output_tokens);

        // Cost metrics
        CostMetrics::record(model, cost_dollars);

        tracing::debug!(
            task_id = %task_id,
            agent_id = %agent_id,
            model = %model,
            input_tokens = %input_tokens,
            output_tokens = %output_tokens,
            cost_dollars = %cost_dollars,
            duration_seconds = %duration_seconds,
            "Task completed metrics recorded"
        );
    }

    /// Record a task failure.
    pub fn record_task_failed(task_id: &str, error_type: &str, model: &str) {
        counter!("apex_tasks_total", "status" => "failed").increment(1);
        counter!("apex_tasks_failed", "error_type" => error_type.to_string()).increment(1);

        tracing::debug!(
            task_id = %task_id,
            error_type = %error_type,
            model = %model,
            "Task failed metrics recorded"
        );
    }

    /// Record an agent spawn.
    pub fn record_agent_spawned(agent_id: &str, model: &str) {
        counter!("apex_agent_spawns_total", "model" => model.to_string()).increment(1);
        gauge!("apex_active_agents").increment(1.0);

        tracing::debug!(
            agent_id = %agent_id,
            model = %model,
            "Agent spawned metrics recorded"
        );
    }

    /// Record an agent termination.
    pub fn record_agent_terminated(agent_id: &str) {
        gauge!("apex_active_agents").decrement(1.0);

        tracing::debug!(
            agent_id = %agent_id,
            "Agent terminated metrics recorded"
        );
    }

    /// Set the current queue depth.
    pub fn set_queue_depth(depth: u64) {
        gauge!("apex_queue_depth").set(depth as f64);
    }

    /// Set worker utilization (0.0 to 1.0).
    pub fn set_worker_utilization(utilization: f64) {
        gauge!("apex_worker_utilization").set(utilization);
    }

    /// Record a tool call.
    pub fn record_tool_call(tool_name: &str, duration_seconds: f64, success: bool) {
        counter!(
            "apex_tool_calls_total",
            "tool" => tool_name.to_string(),
            "success" => success.to_string(),
        )
        .increment(1);

        histogram!(
            "apex_tool_latency_seconds",
            "tool" => tool_name.to_string(),
        )
        .record(duration_seconds);
    }

    /// Record a circuit breaker state change.
    pub fn record_circuit_breaker_state(service: &str, state: CircuitBreakerState) {
        let state_value = match state {
            CircuitBreakerState::Closed => 0.0,
            CircuitBreakerState::HalfOpen => 1.0,
            CircuitBreakerState::Open => 2.0,
        };

        gauge!(
            "apex_circuit_breaker_state",
            "service" => service.to_string(),
        )
        .set(state_value);
    }

    /// Record a circuit breaker trip.
    pub fn record_circuit_breaker_trip(service: &str) {
        counter!(
            "apex_circuit_breaker_trips_total",
            "service" => service.to_string(),
        )
        .increment(1);
    }

    /// Record a contract violation.
    pub fn record_contract_violation(contract_id: &str, limit_type: &str) {
        counter!(
            "apex_contract_violations_total",
            "contract_id" => contract_id.to_string(),
            "limit_type" => limit_type.to_string(),
        )
        .increment(1);
    }
}

/// Circuit breaker state for metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Circuit is closed (normal operation)
    Closed,
    /// Circuit is half-open (testing recovery)
    HalfOpen,
    /// Circuit is open (failing fast)
    Open,
}

/// Utility for tracking operation timing with automatic metric recording.
pub struct OperationTimer {
    start: Instant,
    operation_name: &'static str,
    labels: HashMap<String, String>,
}

impl OperationTimer {
    /// Start timing an operation.
    ///
    /// Note: The operation_name must be a static string since metrics names
    /// cannot be dynamically generated at runtime.
    pub fn start(operation_name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            operation_name,
            labels: HashMap::new(),
        }
    }

    /// Add a label to the timer.
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Get elapsed time without recording.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Finish and record to a histogram.
    ///
    /// Records to a generic "operation_duration_seconds" histogram with the
    /// operation name as a label.
    pub fn finish(self) -> Duration {
        let duration = self.start.elapsed();

        histogram!(
            "operation_duration_seconds",
            "operation" => self.operation_name,
        )
        .record(duration.as_secs_f64());

        duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_config_defaults() {
        let config = MetricsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.endpoint, "0.0.0.0:9090");
        assert!(!config.duration_buckets.is_empty());
    }

    #[test]
    fn test_request_timer() {
        let timer = RequestDurationHistogram::start("http", "GET", "/test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.finish(200);
    }

    #[test]
    fn test_operation_timer() {
        let timer = OperationTimer::start("test_operation").label("key", "value");

        std::thread::sleep(std::time::Duration::from_millis(10));
        let duration = timer.finish();

        assert!(duration.as_millis() >= 10);
    }

    #[test]
    fn test_circuit_breaker_state() {
        assert_eq!(CircuitBreakerState::Closed, CircuitBreakerState::Closed);
        assert_ne!(CircuitBreakerState::Closed, CircuitBreakerState::Open);
    }
}
