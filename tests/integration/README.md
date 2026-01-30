# Apex Integration Tests

This directory contains comprehensive integration tests for the Apex platform. These tests verify the correct behavior of the entire system including API endpoints, database operations, WebSocket communications, and end-to-end workflows.

## Overview

Integration tests differ from unit tests in that they test the system as a whole, with real infrastructure dependencies (PostgreSQL, Redis) running in Docker containers.

## Test Structure

```
tests/integration/
├── README.md                    # This file
├── docker-compose.test.yml      # Test infrastructure configuration
├── conftest.py                  # Pytest fixtures and setup
├── test_api_tasks.py            # Task API integration tests
├── test_api_agents.py           # Agent API integration tests
├── test_api_dags.py             # DAG API integration tests
├── test_websocket.py            # WebSocket integration tests
├── test_workflow.py             # End-to-end workflow tests
└── test_contracts.py            # Contract enforcement tests
```

## Prerequisites

- Docker and Docker Compose installed
- Python 3.11+
- The Apex Python SDK (`apex-sdk`)

## Running Tests

### Quick Start

Use the integration test runner script:

```bash
./scripts/run-integration-tests.sh
```

### Manual Execution

1. Start the test infrastructure:

```bash
docker compose -f tests/integration/docker-compose.test.yml up -d
```

2. Wait for services to be healthy:

```bash
docker compose -f tests/integration/docker-compose.test.yml ps
```

3. Run the tests:

```bash
pytest tests/integration/ -v
```

4. Tear down the infrastructure:

```bash
docker compose -f tests/integration/docker-compose.test.yml down -v
```

### Running Specific Tests

```bash
# Run only Task API tests
pytest tests/integration/test_api_tasks.py -v

# Run only WebSocket tests
pytest tests/integration/test_websocket.py -v

# Run tests matching a pattern
pytest tests/integration/ -v -k "create"

# Run with coverage
pytest tests/integration/ -v --cov=apex_sdk --cov-report=html
```

## Test Categories

### API Tests

These tests verify the REST API endpoints:

- **test_api_tasks.py**: CRUD operations for tasks, task lifecycle management
- **test_api_agents.py**: CRUD operations for agents, agent status management
- **test_api_dags.py**: DAG creation, execution, and lifecycle management

### WebSocket Tests

Tests for real-time communication:

- Connection and authentication
- Event subscription and filtering
- Message delivery and ordering
- Reconnection handling

### Workflow Tests

End-to-end tests that simulate real-world usage:

- Complete task execution workflows
- DAG orchestration scenarios
- Approval flows
- Error handling and recovery

### Contract Tests

Tests that verify API contracts and data integrity:

- Request/response schema validation
- Error response formats
- Pagination behavior
- Rate limiting

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `TEST_API_URL` | Apex API URL | `http://localhost:8081` |
| `TEST_WS_URL` | WebSocket URL | `ws://localhost:8081/ws` |
| `TEST_API_KEY` | API key for tests | `test-api-key` |
| `TEST_DB_URL` | PostgreSQL connection | `postgres://apex:apex_test@localhost:5433/apex_test` |
| `TEST_REDIS_URL` | Redis connection | `redis://localhost:6380` |
| `TEST_TIMEOUT` | Default test timeout (seconds) | `30` |

### Pytest Configuration

The `conftest.py` file contains shared fixtures:

- `api_client`: Async API client for making requests
- `sync_api_client`: Sync API client
- `ws_client`: WebSocket client for real-time tests
- `db_connection`: Direct database connection for state verification
- `redis_client`: Redis client for cache/queue verification
- `cleanup_tasks`: Automatic cleanup of created resources

## Writing New Tests

### Basic Test Structure

```python
import pytest
from apex_sdk import AsyncApexClient
from apex_sdk.models import TaskCreate, TaskStatus

@pytest.mark.asyncio
async def test_example(api_client: AsyncApexClient):
    """Test description."""
    # Arrange
    task_data = TaskCreate(
        name="test-task",
        description="Test description"
    )

    # Act
    task = await api_client.create_task(task_data)

    # Assert
    assert task.name == "test-task"
    assert task.status == TaskStatus.PENDING
```

### Testing Database State

```python
@pytest.mark.asyncio
async def test_database_state(api_client: AsyncApexClient, db_connection):
    """Verify database state after API operation."""
    task = await api_client.create_task(TaskCreate(name="test"))

    # Verify directly in database
    result = await db_connection.fetchrow(
        "SELECT * FROM tasks WHERE id = $1",
        task.id
    )
    assert result is not None
    assert result["name"] == "test"
```

### Testing WebSocket Events

```python
@pytest.mark.asyncio
async def test_websocket_events(api_client: AsyncApexClient, ws_client):
    """Test WebSocket event delivery."""
    events = []

    async def handler(message):
        events.append(message)

    ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, handler)
    await ws_client.connect()
    await ws_client.subscribe([WebSocketEventType.TASK_CREATED])

    # Trigger event
    await api_client.create_task(TaskCreate(name="test"))

    # Wait for event
    await asyncio.sleep(1)
    assert len(events) == 1
```

## Troubleshooting

### Tests Failing to Connect

1. Ensure Docker containers are running:
   ```bash
   docker compose -f tests/integration/docker-compose.test.yml ps
   ```

2. Check container logs:
   ```bash
   docker compose -f tests/integration/docker-compose.test.yml logs apex-api-test
   ```

3. Verify ports are not in use:
   ```bash
   lsof -i :8081  # API port
   lsof -i :5433  # PostgreSQL port
   lsof -i :6380  # Redis port
   ```

### Flaky Tests

If tests are intermittently failing:

1. Increase timeouts in environment variables
2. Check for resource cleanup between tests
3. Verify database state is properly reset

### Database State Issues

If database state is causing issues:

```bash
# Reset the test database
docker compose -f tests/integration/docker-compose.test.yml down -v
docker compose -f tests/integration/docker-compose.test.yml up -d
```

## CI/CD Integration

These tests are designed to run in CI/CD pipelines. See `.github/workflows/integration-tests.yml` for the GitHub Actions configuration.

Key considerations for CI:
- Use service containers for dependencies
- Set appropriate timeouts
- Clean up resources after tests
- Capture test artifacts and logs

## Best Practices

1. **Isolation**: Each test should be independent and not rely on state from other tests
2. **Cleanup**: Always clean up created resources, even on test failure
3. **Timeouts**: Use appropriate timeouts for async operations
4. **Assertions**: Make specific assertions about expected behavior
5. **Documentation**: Document what each test is verifying
6. **Determinism**: Avoid tests that depend on timing or random values
