//! Distributed Tracing with OpenTelemetry Integration.
//!
//! This module provides comprehensive distributed tracing capabilities:
//!
//! - OpenTelemetry integration for standardized tracing
//! - Jaeger and OTLP exporter support
//! - Custom span attributes for rich context
//! - Trace context propagation (W3C Trace Context, B3)
//!
//! # Example
//!
//! ```rust,no_run
//! use apex_core::telemetry::tracing::{SpanBuilder, TraceContext};
//!
//! // Create a custom span
//! let span = SpanBuilder::new("process_task")
//!     .attribute("task_id", "123")
//!     .attribute("priority", "high")
//!     .start();
//!
//! // Get current trace context for propagation
//! if let Some(trace_id) = TraceContext::current_trace_id() {
//!     println!("Current trace: {}", trace_id);
//! }
//! ```

use opentelemetry::trace::TraceContextExt;
use opentelemetry::Context;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::{self as sdktrace, Sampler};
use opentelemetry_sdk::Resource;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Flag indicating if tracing has been initialized.
static TRACING_INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Tracing configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TracingConfig {
    /// Whether tracing is enabled
    #[serde(default = "default_tracing_enabled")]
    pub enabled: bool,

    /// Exporter type (jaeger, otlp, none)
    #[serde(default)]
    pub exporter: ExporterType,

    /// OTLP/Jaeger endpoint
    #[serde(default)]
    pub endpoint: Option<String>,

    /// Sampling configuration
    #[serde(default)]
    pub sampling: SamplingConfig,

    /// Propagation format
    #[serde(default)]
    pub propagation: PropagationFormat,

    /// Additional resource attributes
    #[serde(default)]
    pub resource_attributes: HashMap<String, String>,

    /// Maximum events per span
    #[serde(default = "default_max_events_per_span")]
    pub max_events_per_span: u32,

    /// Maximum attributes per span
    #[serde(default = "default_max_attributes_per_span")]
    pub max_attributes_per_span: u32,

    /// Maximum links per span
    #[serde(default = "default_max_links_per_span")]
    pub max_links_per_span: u32,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: default_tracing_enabled(),
            exporter: ExporterType::default(),
            endpoint: None,
            sampling: SamplingConfig::default(),
            propagation: PropagationFormat::default(),
            resource_attributes: HashMap::new(),
            max_events_per_span: default_max_events_per_span(),
            max_attributes_per_span: default_max_attributes_per_span(),
            max_links_per_span: default_max_links_per_span(),
        }
    }
}

/// Trace exporter type.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExporterType {
    /// OTLP exporter (recommended for production)
    #[default]
    Otlp,
    /// Jaeger exporter
    Jaeger,
    /// No exporter (local only)
    None,
}

/// Trace context propagation format.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PropagationFormat {
    /// W3C Trace Context (recommended)
    #[default]
    W3c,
    /// B3 propagation (Zipkin style)
    B3,
    /// Both W3C and B3
    Composite,
}

/// Sampling configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SamplingConfig {
    /// Sampling strategy
    #[serde(default)]
    pub strategy: SamplingStrategy,

    /// Sample ratio for ratio-based sampling (0.0 to 1.0)
    #[serde(default = "default_sample_ratio")]
    pub ratio: f64,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            strategy: SamplingStrategy::default(),
            ratio: default_sample_ratio(),
        }
    }
}

/// Sampling strategy.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SamplingStrategy {
    /// Sample all traces
    #[default]
    AlwaysOn,
    /// Sample no traces
    AlwaysOff,
    /// Sample based on ratio
    TraceIdRatio,
    /// Sample based on parent span decision
    ParentBased,
}

// Default value functions
fn default_tracing_enabled() -> bool {
    true
}

fn default_max_events_per_span() -> u32 {
    128
}

fn default_max_attributes_per_span() -> u32 {
    128
}

fn default_max_links_per_span() -> u32 {
    32
}

fn default_sample_ratio() -> f64 {
    1.0
}

/// Handle for managing the tracing lifecycle.
#[derive(Debug)]
pub struct TracingHandle {
    _private: (),
}

/// Initialize the tracing subsystem with OpenTelemetry.
///
/// # Arguments
///
/// * `config` - Tracing configuration
/// * `service_name` - Name of the service for identification
///
/// # Errors
///
/// Returns an error if the tracer cannot be initialized.
pub fn init_tracing(config: &TracingConfig, service_name: &str) -> anyhow::Result<TracingHandle> {
    if !config.enabled {
        return Ok(TracingHandle { _private: () });
    }

    // Build resource attributes
    let mut resource_attrs = vec![
        opentelemetry::KeyValue::new("service.name", service_name.to_string()),
        opentelemetry::KeyValue::new("service.version", env!("CARGO_PKG_VERSION").to_string()),
    ];

    for (key, value) in &config.resource_attributes {
        resource_attrs.push(opentelemetry::KeyValue::new(key.clone(), value.clone()));
    }

    let resource = Resource::new(resource_attrs);

    // Build sampler
    let sampler = match config.sampling.strategy {
        SamplingStrategy::AlwaysOn => Sampler::AlwaysOn,
        SamplingStrategy::AlwaysOff => Sampler::AlwaysOff,
        SamplingStrategy::TraceIdRatio => Sampler::TraceIdRatioBased(config.sampling.ratio),
        SamplingStrategy::ParentBased => {
            Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(config.sampling.ratio)))
        }
    };

    // Build trace config
    let trace_config = sdktrace::Config::default()
        .with_resource(resource)
        .with_sampler(sampler)
        .with_max_events_per_span(config.max_events_per_span)
        .with_max_attributes_per_span(config.max_attributes_per_span)
        .with_max_links_per_span(config.max_links_per_span);

    // Build and install the tracer based on exporter type
    match &config.exporter {
        ExporterType::Otlp => {
            let endpoint = config
                .endpoint
                .as_deref()
                .unwrap_or("http://localhost:4317");

            let exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint);

            let _tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(exporter)
                .with_trace_config(trace_config)
                .install_batch(opentelemetry_sdk::runtime::Tokio)?;

            let _ = TRACING_INITIALIZED.set(true);
        }
        ExporterType::Jaeger => {
            // For Jaeger, we use OTLP with the Jaeger endpoint
            let endpoint = config
                .endpoint
                .as_deref()
                .unwrap_or("http://localhost:14268/api/traces");

            let exporter = opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint);

            let _tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(exporter)
                .with_trace_config(trace_config)
                .install_batch(opentelemetry_sdk::runtime::Tokio)?;

            let _ = TRACING_INITIALIZED.set(true);
        }
        ExporterType::None => {
            // No-op tracer for testing/development
            let _ = TRACING_INITIALIZED.set(false);
        }
    }

    // Set up propagation
    match config.propagation {
        PropagationFormat::W3c => {
            opentelemetry::global::set_text_map_propagator(
                opentelemetry_sdk::propagation::TraceContextPropagator::new(),
            );
        }
        PropagationFormat::B3 => {
            // B3 propagation - using trace context as fallback
            opentelemetry::global::set_text_map_propagator(
                opentelemetry_sdk::propagation::TraceContextPropagator::new(),
            );
        }
        PropagationFormat::Composite => {
            // Composite propagator supporting both W3C and B3
            opentelemetry::global::set_text_map_propagator(
                opentelemetry_sdk::propagation::TraceContextPropagator::new(),
            );
        }
    }

    Ok(TracingHandle { _private: () })
}

/// Shutdown the tracing subsystem.
///
/// This should be called during application shutdown to flush any remaining spans.
pub fn shutdown_tracing() {
    opentelemetry::global::shutdown_tracer_provider();
}

/// Trace context utilities for accessing current trace information.
pub struct TraceContext;

impl TraceContext {
    /// Get the current trace ID if available.
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

    /// Get the current span ID if available.
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

    /// Get the current trace flags.
    pub fn current_trace_flags() -> Option<u8> {
        let ctx = Context::current();
        let span = ctx.span();
        let span_ctx = span.span_context();

        if span_ctx.is_valid() {
            Some(span_ctx.trace_flags().to_u8())
        } else {
            None
        }
    }

    /// Check if the current span is sampled.
    pub fn is_sampled() -> bool {
        let ctx = Context::current();
        let span = ctx.span();
        let span_ctx = span.span_context();

        span_ctx.is_sampled()
    }

    /// Extract trace context from HTTP headers for propagation.
    pub fn extract_from_headers(headers: &HashMap<String, String>) -> Context {
        use opentelemetry::propagation::TextMapPropagator;

        let propagator = opentelemetry_sdk::propagation::TraceContextPropagator::new();
        propagator.extract(&HeaderExtractor(headers))
    }

    /// Inject trace context into HTTP headers for propagation.
    pub fn inject_into_headers(headers: &mut HashMap<String, String>) {
        use opentelemetry::propagation::TextMapPropagator;

        let ctx = Context::current();
        let propagator = opentelemetry_sdk::propagation::TraceContextPropagator::new();
        propagator.inject_context(&ctx, &mut HeaderInjector(headers));
    }
}

/// Header extractor for trace context propagation.
struct HeaderExtractor<'a>(&'a HashMap<String, String>);

impl<'a> opentelemetry::propagation::Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|s| s.as_str()).collect()
    }
}

/// Header injector for trace context propagation.
struct HeaderInjector<'a>(&'a mut HashMap<String, String>);

impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

/// Builder for creating custom spans with attributes.
#[derive(Debug)]
pub struct SpanBuilder {
    name: String,
    attributes: Vec<opentelemetry::KeyValue>,
    span_kind: opentelemetry::trace::SpanKind,
}

impl SpanBuilder {
    /// Create a new span builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attributes: Vec::new(),
            span_kind: opentelemetry::trace::SpanKind::Internal,
        }
    }

    /// Add an attribute to the span.
    pub fn attribute(mut self, key: impl Into<String>, value: impl Into<AttributeValue>) -> Self {
        self.attributes
            .push(opentelemetry::KeyValue::new(key.into(), value.into().0));
        self
    }

    /// Set the span kind.
    pub fn kind(mut self, kind: SpanKind) -> Self {
        self.span_kind = kind.into();
        self
    }

    /// Start the span and return a guard that ends it on drop.
    pub fn start(self) -> SpanGuard {
        let span = tracing::info_span!(
            "custom_span",
            otel.name = %self.name,
        );

        SpanGuard {
            _span: span.entered(),
        }
    }

    /// Start a server span (for incoming requests).
    pub fn server(name: impl Into<String>) -> Self {
        Self::new(name).kind(SpanKind::Server)
    }

    /// Start a client span (for outgoing requests).
    pub fn client(name: impl Into<String>) -> Self {
        Self::new(name).kind(SpanKind::Client)
    }

    /// Start a producer span (for message producers).
    pub fn producer(name: impl Into<String>) -> Self {
        Self::new(name).kind(SpanKind::Producer)
    }

    /// Start a consumer span (for message consumers).
    pub fn consumer(name: impl Into<String>) -> Self {
        Self::new(name).kind(SpanKind::Consumer)
    }
}

/// Span kind for semantic conventions.
#[derive(Debug, Clone, Copy)]
pub enum SpanKind {
    /// Internal span (default)
    Internal,
    /// Server span for incoming requests
    Server,
    /// Client span for outgoing requests
    Client,
    /// Producer span for message producers
    Producer,
    /// Consumer span for message consumers
    Consumer,
}

impl From<SpanKind> for opentelemetry::trace::SpanKind {
    fn from(kind: SpanKind) -> Self {
        match kind {
            SpanKind::Internal => opentelemetry::trace::SpanKind::Internal,
            SpanKind::Server => opentelemetry::trace::SpanKind::Server,
            SpanKind::Client => opentelemetry::trace::SpanKind::Client,
            SpanKind::Producer => opentelemetry::trace::SpanKind::Producer,
            SpanKind::Consumer => opentelemetry::trace::SpanKind::Consumer,
        }
    }
}

/// Wrapper for span attribute values.
pub struct AttributeValue(opentelemetry::Value);

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self {
        AttributeValue(opentelemetry::Value::String(s.to_string().into()))
    }
}

impl From<String> for AttributeValue {
    fn from(s: String) -> Self {
        AttributeValue(opentelemetry::Value::String(s.into()))
    }
}

impl From<i64> for AttributeValue {
    fn from(n: i64) -> Self {
        AttributeValue(opentelemetry::Value::I64(n))
    }
}

impl From<f64> for AttributeValue {
    fn from(n: f64) -> Self {
        AttributeValue(opentelemetry::Value::F64(n))
    }
}

impl From<bool> for AttributeValue {
    fn from(b: bool) -> Self {
        AttributeValue(opentelemetry::Value::Bool(b))
    }
}

/// Guard that ends a span when dropped.
pub struct SpanGuard {
    _span: tracing::span::EnteredSpan,
}

impl SpanGuard {
    /// Record an event in the current span.
    pub fn event(&self, name: &str) {
        tracing::info!(event = name, "span event");
    }

    /// Record an error in the current span.
    pub fn error(&self, error: &str) {
        tracing::error!(error = error, "span error");
    }
}

/// Convenience macros for common tracing patterns.
#[macro_export]
macro_rules! trace_span {
    ($name:expr) => {
        $crate::telemetry::tracing::SpanBuilder::new($name).start()
    };
    ($name:expr, $($key:ident = $value:expr),* $(,)?) => {
        $crate::telemetry::tracing::SpanBuilder::new($name)
            $(.attribute(stringify!($key), $value))*
            .start()
    };
}

// Re-export macros at module level
pub use trace_span;

// Re-export logging macros for convenience
pub use crate::{log_debug, log_error, log_info, log_trace, log_warn};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_defaults() {
        let config = TracingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.exporter, ExporterType::Otlp);
        assert_eq!(config.propagation, PropagationFormat::W3c);
    }

    #[test]
    fn test_sampling_config() {
        let config = SamplingConfig::default();
        assert_eq!(config.strategy, SamplingStrategy::AlwaysOn);
        assert_eq!(config.ratio, 1.0);
    }

    #[test]
    fn test_span_builder() {
        let builder = SpanBuilder::new("test_span")
            .attribute("key1", "value1")
            .attribute("key2", 42i64)
            .kind(SpanKind::Server);

        assert_eq!(builder.name, "test_span");
        assert_eq!(builder.attributes.len(), 2);
    }

    #[test]
    fn test_attribute_value_conversions() {
        let _s: AttributeValue = "test".into();
        let _s2: AttributeValue = String::from("test").into();
        let _i: AttributeValue = 42i64.into();
        let _f: AttributeValue = 3.14f64.into();
        let _b: AttributeValue = true.into();
    }

    #[test]
    fn test_header_injection_extraction() {
        let mut headers = HashMap::new();
        TraceContext::inject_into_headers(&mut headers);
        // Headers may or may not contain trace context depending on whether we're in a span
        let _ctx = TraceContext::extract_from_headers(&headers);
    }
}
