# ADR-0006: Observability Architecture

## Status

Accepted

## Date

2026-01-30

## Context

Apex workflows involve complex interactions between the orchestration engine, multiple agents, external LLM providers, and downstream services. Debugging failures and optimizing performance requires comprehensive observability:

- **Tracing**: Following requests through distributed components
- **Metrics**: Quantitative performance and business measurements
- **Logging**: Detailed event records for debugging
- **LLM-Specific**: Token usage, prompt/completion pairs, latency per model

Without proper observability, debugging production issues becomes a guessing game, and cost optimization is impossible.

## Decision

We will implement a comprehensive observability stack based on OpenTelemetry with LLM-specific extensions:

### 1. Distributed Tracing (OpenTelemetry)

- Trace spans for each workflow, task, and agent execution
- Propagate trace context through all components
- Capture LLM calls as child spans with model metadata
- Export to Jaeger/Tempo for visualization

### 2. Metrics (Prometheus)

Core metrics include:
- `apex_workflow_duration_seconds` - Workflow execution time
- `apex_task_duration_seconds` - Individual task execution time
- `apex_llm_tokens_total` - Token usage by model and type (input/output)
- `apex_llm_cost_dollars` - Estimated LLM cost by model
- `apex_llm_latency_seconds` - LLM response latency by model
- `apex_routing_decisions_total` - FrugalGPT routing decisions
- `apex_contract_violations_total` - Agent contract failures

### 3. Structured Logging (JSON)

- Structured JSON logs with correlation IDs
- Log levels aligned with workflow phases
- Sensitive data (prompts, outputs) redacted by default
- Configurable verbose mode for debugging

### 4. LLM-Specific Observability

- Prompt/completion logging (opt-in, privacy-aware)
- Token usage tracking per request
- Model routing decisions and confidence scores
- Quality scores when available
- Cost attribution to workflows and agents

### 5. Alerting Integration

- Prometheus AlertManager for metric-based alerts
- Workflow failure notifications
- Cost threshold alerts
- Latency SLO violations

## Consequences

### Positive

- End-to-end visibility into workflow execution
- Precise cost attribution to workflows and agents
- Performance bottleneck identification
- Fast debugging with correlated traces/logs
- SLO monitoring and alerting
- Data for optimizing routing decisions

### Negative

- Observability infrastructure overhead (storage, processing)
- Potential performance impact from instrumentation
- Privacy concerns with prompt/completion logging
- Learning curve for observability tools
- Cost of observability backends at scale

### Neutral

- Requires OpenTelemetry SDK integration in all components
- Creates dependency on observability infrastructure
- Generates significant data volume requiring retention policies

## Alternatives Considered

### Proprietary APM (Datadog, New Relic)

Commercial APM tools offer comprehensive features but create vendor lock-in and significant cost at scale. OpenTelemetry provides vendor-neutral instrumentation with choice of backends.

### Custom Observability Solution

Building custom observability is time-consuming and reinvents solved problems. OpenTelemetry provides standardized instrumentation with extensive ecosystem support.

### Logging Only

Logs alone are insufficient for understanding distributed system behavior. Traces provide crucial context for following requests across components.

### LLM-Specific Platforms (LangSmith, Weights & Biases)

LLM-specific platforms offer prompt/completion tracking but don't integrate with general observability. We use OpenTelemetry for unified observability with LLM-specific extensions.

### Minimal Instrumentation

Reducing observability to save resources makes debugging production issues extremely difficult. The cost of observability is far outweighed by faster incident resolution.

## References

- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [Jaeger Tracing](https://www.jaegertracing.io/)
- [LangSmith Observability](https://docs.smith.langchain.com/)
- [OpenLLMetry](https://github.com/traceloop/openllmetry)
