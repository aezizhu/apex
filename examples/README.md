# Apex API Examples

This directory contains comprehensive examples demonstrating how to use the Apex Agent Swarm Orchestration System API with both the Python and TypeScript SDKs, as well as raw curl commands.

## Directory Structure

```
examples/
├── README.md                              # This file
├── python/                                # Python SDK examples
│   ├── basic_usage.py                     # Basic SDK operations (CRUD, health, errors)
│   ├── dag_workflow.py                    # DAG workflow orchestration
│   ├── async_streaming.py                 # Async streaming with WebSocket
│   ├── advanced_dag.py                    # ML pipeline with conditional branches
│   ├── monitoring.py                      # Real-time WebSocket monitoring dashboard
│   └── cost_optimization.py              # FrugalGPT cascading model selection
├── typescript/                            # TypeScript SDK examples
│   ├── basic-usage.ts                     # Basic SDK operations
│   ├── dag-workflow.ts                    # DAG workflow orchestration
│   ├── real-time-monitoring.ts            # WebSocket real-time monitoring
│   ├── advanced-dag.ts                    # Content moderation pipeline DAG
│   ├── realtime-dashboard.ts              # Terminal monitoring dashboard
│   └── batch-processing.ts               # Batch task processing with concurrency
└── curl/                                  # curl command examples
    ├── api-examples.sh                    # REST API curl commands
    └── websocket-test.sh                  # WebSocket testing with wscat
```

## Example Index

### Python Examples

| File | Description | Key Concepts |
|------|-------------|-------------|
| `basic_usage.py` | Client init, CRUD for tasks/agents/DAGs, error handling | Sync client, context managers, typed exceptions |
| `dag_workflow.py` | Build and execute a multi-step DAG | Node dependencies, edge definitions, DAG lifecycle |
| `async_streaming.py` | Async client with WebSocket event streaming | `AsyncApexClient`, `asyncio`, WebSocket subscriptions |
| `advanced_dag.py` | ML model evaluation pipeline with conditional branches | Fan-out/fan-in, conditional edges, cron scheduling |
| `monitoring.py` | Five monitoring patterns (simple listener to dashboard) | Decorator handlers, filtered subscriptions, REST+WS |
| `cost_optimization.py` | FrugalGPT cascading model selection strategy | Model cascading, cost tracking, quality thresholds |

### TypeScript Examples

| File | Description | Key Concepts |
|------|-------------|-------------|
| `basic-usage.ts` | Client init, CRUD for tasks/agents/DAGs | `ApexClient`, typed responses, error handling |
| `dag-workflow.ts` | Build and execute a multi-step DAG | `CreateDAGRequest`, node wiring, execution polling |
| `real-time-monitoring.ts` | WebSocket event monitoring | `ApexWebSocket`, event subscriptions, reconnection |
| `advanced-dag.ts` | Content moderation pipeline with approval gates | Parallel stages, human-in-the-loop, conditional routing |
| `realtime-dashboard.ts` | Terminal-based monitoring dashboard | ANSI rendering, concurrent WS + REST, counters |
| `batch-processing.ts` | Batch task submission with concurrency control | Rate-limit handling, semaphore patterns, progress tracking |

### curl Examples

| File | Description |
|------|-------------|
| `api-examples.sh` | Complete REST API walkthrough using curl |
| `websocket-test.sh` | WebSocket connection testing with wscat |

## Prerequisites

### Python Examples

```bash
# Install the Apex Python SDK
pip install apex-swarm

# Or using poetry
poetry add apex-swarm
```

### TypeScript Examples

```bash
# Install the Apex TypeScript SDK
npm install @apex-swarm/sdk

# Or using yarn
yarn add @apex-swarm/sdk

# For running examples directly with ts-node
npm install -g ts-node typescript
```

### curl Examples

```bash
# No special installation required for curl
# For WebSocket testing, install wscat
npm install -g wscat
```

## Environment Setup

Before running any examples, set up your environment:

```bash
# Copy the example environment file
cp ../.env.example ../.env

# Edit the .env file with your settings
# Required variables:
# - APEX_API_URL (default: http://localhost:8080)
# - APEX_API_KEY (your API key)
# - OPENAI_API_KEY (for AI model access)
# - ANTHROPIC_API_KEY (optional, for Claude models)
```

## Running the Examples

### Python

```bash
cd examples/python

# Basic operations
python basic_usage.py

# DAG workflows
python dag_workflow.py
python advanced_dag.py

# Real-time monitoring
python monitoring.py

# Cost optimization
python cost_optimization.py

# Async streaming
python async_streaming.py
```

### TypeScript

```bash
cd examples/typescript

# Basic operations
npx ts-node basic-usage.ts

# DAG workflows
npx ts-node dag-workflow.ts
npx ts-node advanced-dag.ts

# Real-time monitoring
npx ts-node real-time-monitoring.ts
npx ts-node realtime-dashboard.ts

# Batch processing
npx ts-node batch-processing.ts
```

### curl

```bash
cd examples/curl

chmod +x *.sh
./api-examples.sh
./websocket-test.sh
```

## Key Patterns Demonstrated

### DAG Workflows

- **Sequential stages** -- nodes that depend on one another execute in order.
- **Fan-out / fan-in** -- a single node fans out to multiple parallel nodes, which converge at a downstream join node.
- **Conditional branches** -- edges with condition expressions that route execution based on upstream output.
- **Approval gates** -- human-in-the-loop steps embedded within a DAG.
- **Cron scheduling** -- DAGs that run on a recurring schedule.

### Real-Time Monitoring

- **Simple listener** -- subscribe and iterate over WebSocket messages.
- **Decorator handlers** -- register callbacks with `@ws.on_event(...)`.
- **Filtered subscriptions** -- subscribe to events for specific task IDs.
- **Terminal dashboard** -- auto-refreshing ANSI display combining WS push with REST polling.
- **Combined REST + WebSocket** -- create resources via REST, track them via WebSocket.

### Cost Optimization

- **Model cascading** -- start with the cheapest model and escalate only when quality is insufficient.
- **Quality thresholds** -- configurable confidence gates that trigger model upgrades.
- **Cost tracking** -- per-request and aggregate cost accounting.

### Batch Processing

- **Concurrency control** -- limit the number of in-flight tasks using semaphore patterns.
- **Rate-limit handling** -- automatic back-off when the API returns HTTP 429.
- **Progress tracking** -- real-time progress bars and completion callbacks.

## Error Handling Reference

All SDKs provide typed exceptions mapped to HTTP status codes:

| Exception | HTTP Status | Description |
|-----------|------------|-------------|
| `ApexAuthenticationError` | 401 | Invalid or missing credentials |
| `ApexAuthorizationError` | 403 | Insufficient permissions |
| `ApexNotFoundError` | 404 | Resource not found |
| `ApexValidationError` | 422 | Invalid request payload |
| `ApexRateLimitError` | 429 | Rate limit exceeded (includes `Retry-After`) |
| `ApexServerError` | 5xx | Server-side error |
| `ApexTimeoutError` | -- | Request timeout |
| `ApexConnectionError` | -- | Unable to reach the server |

## Retry Logic

The SDKs include built-in retry logic with exponential back-off for transient errors. You can customise:

- Maximum retry attempts (default: 3)
- Initial retry delay (default: 1 second)
- Maximum retry delay (default: 60 seconds)
- Retryable error types (server errors, connection errors, timeouts)

## WebSocket Reconnection

WebSocket clients automatically reconnect on disconnection with:

- Exponential back-off (configurable initial and max delay)
- Configurable maximum reconnection attempts
- Automatic resubscription after reconnection

## API Endpoints Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/api/v1/tasks` | GET, POST | List/create tasks |
| `/api/v1/tasks/{id}` | GET, PATCH, DELETE | Task operations |
| `/api/v1/tasks/{id}/cancel` | POST | Cancel task |
| `/api/v1/tasks/{id}/retry` | POST | Retry task |
| `/api/v1/agents` | GET, POST | List/create agents |
| `/api/v1/agents/{id}` | GET, PATCH, DELETE | Agent operations |
| `/api/v1/dags` | GET, POST | List/create DAGs |
| `/api/v1/dags/{id}` | GET, PATCH, DELETE | DAG operations |
| `/api/v1/dags/{id}/start` | POST | Start DAG execution |
| `/api/v1/dags/{id}/pause` | POST | Pause DAG |
| `/api/v1/dags/{id}/resume` | POST | Resume DAG |
| `/api/v1/approvals` | GET, POST | List/create approvals |
| `/api/v1/approvals/{id}/respond` | POST | Respond to approval |
| `/ws` | WebSocket | Real-time updates |

## Additional Resources

- [Full API Documentation](https://apex-swarm.github.io/api)
- [TypeScript SDK Reference](https://github.com/apex-swarm/apex-sdk-typescript)
- [Python SDK Reference](https://github.com/apex-swarm/apex-sdk-python)
- [Architecture Documentation](../docs/architecture/backend-architecture.md)
- [Contributing Guide](../CONTRIBUTING.md)

## Support

If you encounter any issues with these examples:

1. Check that all prerequisites are installed
2. Verify your environment variables are set correctly
3. Ensure the Apex server is running
4. Check the [Troubleshooting Guide](https://apex-swarm.github.io/docs/troubleshooting)
5. Open an issue on [GitHub](https://github.com/apex-swarm/apex/issues)
