# Apex API Examples

This directory contains comprehensive examples demonstrating how to use the Apex Agent Swarm Orchestration System API.

## Directory Structure

```
examples/
├── README.md                          # This file
├── typescript/                        # TypeScript SDK examples
│   ├── basic-usage.ts                 # Basic SDK operations
│   ├── dag-workflow.ts                # DAG workflow orchestration
│   └── real-time-monitoring.ts        # WebSocket real-time monitoring
├── python/                            # Python SDK examples
│   ├── basic_usage.py                 # Basic SDK operations
│   ├── dag_workflow.py                # DAG workflow orchestration
│   └── async_streaming.py             # Async streaming with WebSocket
└── curl/                              # curl command examples
    ├── api-examples.sh                # REST API curl commands
    └── websocket-test.sh              # WebSocket testing with wscat
```

## Prerequisites

### TypeScript Examples

```bash
# Install the Apex TypeScript SDK
npm install @apex-swarm/sdk

# Or using yarn
yarn add @apex-swarm/sdk

# For running examples directly with ts-node
npm install -g ts-node typescript
```

### Python Examples

```bash
# Install the Apex Python SDK
pip install apex-swarm

# Or using poetry
poetry add apex-swarm
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

### TypeScript

```bash
# Navigate to the typescript examples directory
cd examples/typescript

# Run with ts-node
npx ts-node basic-usage.ts
npx ts-node dag-workflow.ts
npx ts-node real-time-monitoring.ts
```

### Python

```bash
# Navigate to the python examples directory
cd examples/python

# Run with Python
python basic_usage.py
python dag_workflow.py
python async_streaming.py
```

### curl

```bash
# Navigate to the curl examples directory
cd examples/curl

# Make scripts executable
chmod +x *.sh

# Run the examples
./api-examples.sh
./websocket-test.sh
```

## Example Categories

### Basic Usage
- Client initialization and configuration
- Health check operations
- Creating, listing, and managing tasks
- Creating and managing agents
- Error handling patterns

### DAG Workflow
- Creating complex task dependencies
- Building multi-step workflows
- Parallel task execution
- Conditional branching
- Workflow monitoring and control

### Real-Time Monitoring
- WebSocket connection setup
- Subscribing to events
- Handling task updates
- Agent status monitoring
- DAG execution tracking

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

## Common Patterns

### Error Handling

All SDKs provide typed exceptions for different error scenarios:
- `ApexAuthenticationError` - Invalid credentials
- `ApexAuthorizationError` - Insufficient permissions
- `ApexNotFoundError` - Resource not found
- `ApexValidationError` - Invalid request data
- `ApexRateLimitError` - Rate limit exceeded
- `ApexServerError` - Server-side errors
- `ApexTimeoutError` - Request timeout

### Retry Logic

The SDKs include built-in retry logic with exponential backoff for transient errors. You can customize:
- Maximum retry attempts
- Initial retry delay
- Maximum retry delay
- Retryable error types

### WebSocket Reconnection

WebSocket clients automatically reconnect on disconnection with:
- Exponential backoff
- Configurable max reconnection attempts
- Automatic resubscription after reconnection

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
