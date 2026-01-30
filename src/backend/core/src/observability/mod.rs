//! Observability: Distributed Tracing, Metrics, and Logging.

use opentelemetry::trace::TraceContextExt;
use opentelemetry::Context;
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the observability stack.
pub fn init(service_name: &str, otlp_endpoint: Option<&str>) -> anyhow::Result<()> {
    // Set up OpenTelemetry tracing if endpoint is provided
    if let Some(endpoint) = otlp_endpoint {
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint),
            )
            .with_trace_config(
                opentelemetry_sdk::trace::config()
                    .with_resource(opentelemetry_sdk::Resource::new(vec![
                        opentelemetry::KeyValue::new("service.name", service_name.to_string()),
                    ])),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)?;

        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(telemetry_layer)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        // Just use local logging
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    }

    Ok(())
}

/// Shutdown OpenTelemetry.
pub fn shutdown() {
    opentelemetry::global::shutdown_tracer_provider();
}

/// Distributed tracer wrapper.
#[allow(dead_code)]
pub struct Tracer {
    service_name: String,
}

impl Tracer {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Get current trace ID.
    pub fn current_trace_id() -> Option<String> {
        let ctx = Context::current();
        let span = ctx.span();
        let span_ctx = span.span_context();

        if span_ctx.is_valid() {
            Some(span_ctx.trace_id().to_string())
        } else {
            None
        }
    }

    /// Get current span ID.
    pub fn current_span_id() -> Option<String> {
        let ctx = Context::current();
        let span = ctx.span();
        let span_ctx = span.span_context();

        if span_ctx.is_valid() {
            Some(span_ctx.span_id().to_string())
        } else {
            None
        }
    }
}

/// Metrics registry and helpers.
pub mod metrics {
    use metrics::{counter, gauge, histogram, describe_counter, describe_gauge, describe_histogram};

    /// Register all metric descriptions.
    pub fn register_metrics() {
        // Counters
        describe_counter!(
            "apex_tasks_total",
            "Total number of tasks processed"
        );
        describe_counter!(
            "apex_tasks_completed",
            "Total number of tasks completed successfully"
        );
        describe_counter!(
            "apex_tasks_failed",
            "Total number of tasks that failed"
        );
        describe_counter!(
            "apex_tokens_total",
            "Total tokens consumed"
        );
        describe_counter!(
            "apex_cost_total",
            "Total cost in dollars"
        );
        describe_counter!(
            "apex_tool_calls_total",
            "Total tool calls made"
        );

        // Gauges
        describe_gauge!(
            "apex_active_agents",
            "Number of currently active agents"
        );
        describe_gauge!(
            "apex_queue_depth",
            "Number of tasks in the queue"
        );
        describe_gauge!(
            "apex_worker_utilization",
            "Worker pool utilization (0-1)"
        );

        // Histograms
        describe_histogram!(
            "apex_task_duration_seconds",
            "Task execution duration in seconds"
        );
        describe_histogram!(
            "apex_agent_latency_seconds",
            "Agent response latency in seconds"
        );
        describe_histogram!(
            "apex_tool_latency_seconds",
            "Tool execution latency in seconds"
        );
    }

    /// Record a task completion.
    pub fn record_task_completed(tokens: u64, cost: f64, duration_secs: f64) {
        counter!("apex_tasks_total").increment(1);
        counter!("apex_tasks_completed").increment(1);
        counter!("apex_tokens_total").increment(tokens);
        counter!("apex_cost_total").increment((cost * 1_000_000.0) as u64);
        histogram!("apex_task_duration_seconds").record(duration_secs);
    }

    /// Record a task failure.
    pub fn record_task_failed() {
        counter!("apex_tasks_total").increment(1);
        counter!("apex_tasks_failed").increment(1);
    }

    /// Update active agent count.
    pub fn set_active_agents(count: u64) {
        gauge!("apex_active_agents").set(count as f64);
    }

    /// Update queue depth.
    pub fn set_queue_depth(depth: u64) {
        gauge!("apex_queue_depth").set(depth as f64);
    }

    /// Record tool call latency.
    pub fn record_tool_latency(tool: &str, latency_secs: f64) {
        histogram!("apex_tool_latency_seconds", "tool" => tool.to_string()).record(latency_secs);
        counter!("apex_tool_calls_total", "tool" => tool.to_string()).increment(1);
    }
}

/// Structured event types for logging.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event_type")]
pub enum ApexEvent {
    TaskCreated {
        task_id: String,
        dag_id: String,
        name: String,
    },
    TaskStarted {
        task_id: String,
        agent_id: String,
        model: String,
    },
    TaskCompleted {
        task_id: String,
        tokens_used: u64,
        cost: f64,
        duration_ms: u64,
    },
    TaskFailed {
        task_id: String,
        error: String,
        retry_count: u32,
    },
    AgentSpawned {
        agent_id: String,
        name: String,
        model: String,
    },
    ToolCalled {
        task_id: String,
        agent_id: String,
        tool_name: String,
        latency_ms: u64,
    },
    ContractExceeded {
        contract_id: String,
        limit_type: String,
        used: f64,
        limit: f64,
    },
    LoopDetected {
        agent_id: String,
        similarity: f64,
    },
    CircuitBreakerTripped {
        service: String,
        failure_count: u32,
    },
}

impl ApexEvent {
    /// Log this event.
    pub fn log(&self) {
        match self {
            ApexEvent::TaskCreated { task_id, dag_id, name } => {
                tracing::info!(
                    task_id = %task_id,
                    dag_id = %dag_id,
                    name = %name,
                    "Task created"
                );
            }
            ApexEvent::TaskStarted { task_id, agent_id, model } => {
                tracing::info!(
                    task_id = %task_id,
                    agent_id = %agent_id,
                    model = %model,
                    "Task started"
                );
            }
            ApexEvent::TaskCompleted { task_id, tokens_used, cost, duration_ms } => {
                tracing::info!(
                    task_id = %task_id,
                    tokens_used = %tokens_used,
                    cost = %cost,
                    duration_ms = %duration_ms,
                    "Task completed"
                );
            }
            ApexEvent::TaskFailed { task_id, error, retry_count } => {
                tracing::error!(
                    task_id = %task_id,
                    error = %error,
                    retry_count = %retry_count,
                    "Task failed"
                );
            }
            ApexEvent::AgentSpawned { agent_id, name, model } => {
                tracing::info!(
                    agent_id = %agent_id,
                    name = %name,
                    model = %model,
                    "Agent spawned"
                );
            }
            ApexEvent::ToolCalled { task_id, agent_id, tool_name, latency_ms } => {
                tracing::debug!(
                    task_id = %task_id,
                    agent_id = %agent_id,
                    tool_name = %tool_name,
                    latency_ms = %latency_ms,
                    "Tool called"
                );
            }
            ApexEvent::ContractExceeded { contract_id, limit_type, used, limit } => {
                tracing::warn!(
                    contract_id = %contract_id,
                    limit_type = %limit_type,
                    used = %used,
                    limit = %limit,
                    "Contract limit exceeded"
                );
            }
            ApexEvent::LoopDetected { agent_id, similarity } => {
                tracing::warn!(
                    agent_id = %agent_id,
                    similarity = %similarity,
                    "Loop detected"
                );
            }
            ApexEvent::CircuitBreakerTripped { service, failure_count } => {
                tracing::error!(
                    service = %service,
                    failure_count = %failure_count,
                    "Circuit breaker tripped"
                );
            }
        }
    }
}
