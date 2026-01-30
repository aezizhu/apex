# Apex Infrastructure Design

## 1. Observability Architecture

### Distributed Tracing (OpenTelemetry)

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│  Apex API   │───▶│   Jaeger    │───▶│   Grafana   │
│  (OTLP SDK) │    │  Collector  │    │  Tempo UI   │
└─────────────┘    └─────────────┘    └─────────────┘
       │
       ▼
┌─────────────┐
│ Agent Worker│
│ (OTLP SDK)  │
└─────────────┘
```

**Trace Context Propagation:**
- Trace ID: 32-character hex string
- Span ID: 16-character hex string
- Baggage: semantic context (agent_id, task_id, dag_id)

**Key Spans:**
- `dag_execution` - Root span for DAG
- `task_execution` - Per-task span
- `llm_call` - LLM API call
- `tool_execution` - Tool invocation

### Structured Logging (Loki)

```json
{
  "timestamp": "2026-01-29T15:30:00Z",
  "level": "INFO",
  "message": "Task completed",
  "trace_id": "abc123...",
  "span_id": "def456",
  "agent_id": "uuid",
  "task_id": "uuid",
  "tokens_used": 1500,
  "cost": 0.003,
  "duration_ms": 2500
}
```

### Metrics (Prometheus)

**Counters:**
- `apex_tasks_total{status}` - Total tasks by status
- `apex_tokens_total{model}` - Tokens consumed
- `apex_cost_total{model}` - Cost in dollars

**Gauges:**
- `apex_active_agents` - Currently active agents
- `apex_queue_depth` - Tasks waiting in queue
- `apex_worker_utilization` - Worker pool usage

**Histograms:**
- `apex_task_duration_seconds` - Task execution time
- `apex_llm_latency_seconds` - LLM response time

## 2. Deployment Architecture

### Docker Compose (Development)

```yaml
services:
  apex-api:
    build: ./src/backend/core
    ports: ["8080:8080", "50051:50051"]
    depends_on: [postgres, redis]

  apex-worker:
    build: ./src/backend/agents
    deploy:
      replicas: 3
    depends_on: [apex-api]

  apex-dashboard:
    build: ./src/frontend
    ports: ["3000:80"]

  postgres:
    image: postgres:16-alpine
    volumes: [postgres-data:/var/lib/postgresql/data]

  redis:
    image: redis:7-alpine

  jaeger:
    image: jaegertracing/all-in-one:1.53
    ports: ["16686:16686", "4317:4317"]

  prometheus:
    image: prom/prometheus:v2.48.1
    volumes: [./infra/observability/prometheus:/etc/prometheus]

  grafana:
    image: grafana/grafana:10.2.3
    ports: ["3001:3000"]
```

### Kubernetes (Production)

```yaml
# Deployment
apiVersion: apps/v1
kind: Deployment
metadata:
  name: apex-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: apex-api
  template:
    spec:
      containers:
      - name: apex-api
        image: apex/api:latest
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2"

# HPA
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: apex-api-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: apex-api
  minReplicas: 2
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
```

## 3. Security & Sandboxing

### Tool Execution Sandbox

```dockerfile
FROM python:3.11-slim
USER 1000:1000
WORKDIR /sandbox
COPY --chown=1000:1000 . .
```

**Container Constraints:**
- CPU: 1 core
- Memory: 512MB
- PIDs: 100
- Network: none (or allowlist)
- Filesystem: read-only with tmpfs

### Secret Management

```yaml
# HashiCorp Vault
path "secret/apex/*" {
  capabilities = ["read"]
}

# Kubernetes Secrets
apiVersion: v1
kind: Secret
metadata:
  name: apex-secrets
type: Opaque
data:
  OPENAI_API_KEY: <base64>
  ANTHROPIC_API_KEY: <base64>
```

### RBAC

| Role | Permissions |
|------|------------|
| admin | Full access |
| operator | Manage agents, approve actions, view all |
| viewer | Read-only access |
| agent | Execute tasks, call tools (scoped) |

## 4. CI/CD Pipeline

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Rust lint
        run: cargo clippy --all-features
      - name: Python lint
        run: ruff check .
      - name: Frontend lint
        run: npm run lint

  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: test
    steps:
      - name: Rust tests
        run: cargo test
      - name: Python tests
        run: pytest
      - name: Frontend tests
        run: npm test

  build:
    runs-on: ubuntu-latest
    steps:
      - name: Build Docker images
        run: docker-compose build
      - name: Push to registry
        run: docker push apex/api:${{ github.sha }}

  deploy-staging:
    needs: [test, build]
    runs-on: ubuntu-latest
    steps:
      - name: Deploy to staging
        run: kubectl apply -f k8s/staging/
```

## 5. Monitoring Dashboards

### Grafana Dashboard Panels

1. **Overview**
   - Active agents (gauge)
   - Running tasks (gauge)
   - Total cost (counter)
   - Success rate (percentage)

2. **Performance**
   - Task latency P50/P95/P99 (timeseries)
   - LLM latency by model (timeseries)
   - Queue depth (timeseries)

3. **Cost**
   - Cost per hour (timeseries)
   - Cost by model (pie chart)
   - Cost by agent (bar chart)

4. **Errors**
   - Error rate (timeseries)
   - Errors by type (bar chart)
   - Circuit breaker status (state)

## 6. Alerting Rules

| Alert | Condition | Severity |
|-------|-----------|----------|
| HighTaskFailureRate | failure_rate > 5% for 5m | warning |
| CostOverspend | cost > $10/hour | critical |
| NoActiveAgents | active_agents == 0 | critical |
| HighLatency | P95 > 30s for 5m | warning |
| CircuitBreakerOpen | state == open | critical |
| DatabaseDown | pg_up == 0 | critical |
