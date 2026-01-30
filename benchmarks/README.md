# Project Apex - Performance Benchmarking Suite

Comprehensive performance testing and benchmarking tools for Project Apex.

## Overview

This benchmarking suite provides multiple tools and scenarios for testing:

- **API Endpoint Latency**: P50, P95, P99 response times
- **Task Creation Throughput**: Concurrent task creation performance
- **DAG Execution Performance**: Workflow execution timing
- **WebSocket Connection Scaling**: Real-time connection handling
- **Database Query Performance**: Query execution times

## Tools

### K6 (Recommended for CI/CD)

[K6](https://k6.io/) is a modern load testing tool built for developers.

**Installation:**
```bash
# macOS
brew install k6

# Linux
sudo apt-key adv --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6

# Docker
docker pull grafana/k6
```

**Available Tests:**
- `k6/load-test.js` - Standard load testing with ramping VUs
- `k6/stress-test.js` - Stress testing to find breaking points
- `k6/soak-test.js` - Extended duration testing for memory leaks

### Locust (Python-based)

[Locust](https://locust.io/) is a Python-based load testing framework with a web UI.

**Installation:**
```bash
pip install locust
```

**Running:**
```bash
cd benchmarks/locust
locust -f locustfile.py --host=http://localhost:8080
```

## Quick Start

### Using the Benchmark Runner Script

```bash
# Run all benchmarks
./scripts/benchmark.sh all

# Run specific test type
./scripts/benchmark.sh load    # K6 load test
./scripts/benchmark.sh stress  # K6 stress test
./scripts/benchmark.sh soak    # K6 soak test
./scripts/benchmark.sh locust  # Locust web UI

# Run with custom configuration
API_URL=http://api.example.com ./scripts/benchmark.sh load
```

### Running K6 Tests Directly

```bash
# Load test
k6 run benchmarks/k6/load-test.js

# With custom options
k6 run --vus 50 --duration 1m benchmarks/k6/load-test.js

# Output to JSON
k6 run --out json=benchmarks/results/load-test.json benchmarks/k6/load-test.js

# With InfluxDB (for Grafana dashboards)
k6 run --out influxdb=http://localhost:8086/k6 benchmarks/k6/load-test.js
```

### Running Locust Tests

```bash
# Web UI mode
locust -f benchmarks/locust/locustfile.py --host=http://localhost:8080

# Headless mode
locust -f benchmarks/locust/locustfile.py \
    --host=http://localhost:8080 \
    --headless \
    --users 100 \
    --spawn-rate 10 \
    --run-time 5m \
    --csv=benchmarks/results/locust
```

## Test Scenarios

### 1. Load Test (`k6/load-test.js`)

Standard load testing with gradual ramp-up:

| Stage | Duration | Virtual Users | Purpose |
|-------|----------|---------------|---------|
| 1     | 1m       | 0 -> 50       | Warm-up |
| 2     | 3m       | 50            | Steady load |
| 3     | 1m       | 50 -> 100     | Ramp-up |
| 4     | 3m       | 100           | Peak load |
| 5     | 2m       | 100 -> 0      | Cool-down |

**Thresholds:**
- P95 response time < 500ms
- P99 response time < 1000ms
- Error rate < 1%
- Request rate > 100 req/s

### 2. Stress Test (`k6/stress-test.js`)

Find the system's breaking point:

| Stage | Duration | Virtual Users | Purpose |
|-------|----------|---------------|---------|
| 1     | 2m       | 0 -> 100      | Normal load |
| 2     | 5m       | 100           | Steady state |
| 3     | 2m       | 100 -> 200    | Stress |
| 4     | 5m       | 200           | High load |
| 5     | 2m       | 200 -> 300    | Breaking point |
| 6     | 5m       | 300           | Max load |
| 7     | 5m       | 300 -> 0      | Recovery |

**Thresholds:**
- P95 response time < 2000ms
- Error rate < 10%

### 3. Soak Test (`k6/soak-test.js`)

Extended duration testing:

| Stage | Duration | Virtual Users | Purpose |
|-------|----------|---------------|---------|
| 1     | 5m       | 0 -> 100      | Ramp-up |
| 2     | 4h       | 100           | Sustained load |
| 3     | 5m       | 100 -> 0      | Ramp-down |

**Purpose:**
- Detect memory leaks
- Find resource exhaustion issues
- Identify connection pool problems
- Verify long-running stability

## Performance Targets

### API Endpoints

| Endpoint | P50 | P95 | P99 | Throughput |
|----------|-----|-----|-----|------------|
| GET /health | < 5ms | < 10ms | < 20ms | > 5000 req/s |
| GET /api/v1/tasks | < 20ms | < 50ms | < 100ms | > 1000 req/s |
| POST /api/v1/tasks | < 50ms | < 100ms | < 200ms | > 500 req/s |
| GET /api/v1/agents | < 20ms | < 50ms | < 100ms | > 1000 req/s |
| GET /api/v1/dags | < 30ms | < 75ms | < 150ms | > 800 req/s |
| POST /api/v1/dags/{id}/execute | < 100ms | < 200ms | < 500ms | > 200 req/s |

### WebSocket Connections

| Metric | Target |
|--------|--------|
| Connection time | < 100ms |
| Message latency | < 50ms |
| Max concurrent connections | > 10,000 |
| Connection drop rate | < 0.1% |

### Database Queries

| Query Type | Target |
|------------|--------|
| Simple SELECT | < 5ms |
| JOIN queries | < 20ms |
| Aggregations | < 50ms |
| Full-text search | < 100ms |

## Results Analysis

### Viewing K6 Results

```bash
# Generate HTML report (requires k6-reporter)
k6 run --out json=results.json benchmarks/k6/load-test.js
# Use k6-reporter to generate HTML

# View in Grafana
# 1. Run InfluxDB
# 2. Run: k6 run --out influxdb=http://localhost:8086/k6 benchmarks/k6/load-test.js
# 3. Import Grafana dashboard
```

### Viewing Locust Results

```bash
# CSV files are generated in benchmarks/results/
# - locust_stats.csv: Request statistics
# - locust_stats_history.csv: Time series data
# - locust_failures.csv: Failed requests
```

## Continuous Integration

### GitHub Actions Example

```yaml
name: Performance Tests

on:
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM
  workflow_dispatch:

jobs:
  load-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Start services
        run: docker-compose up -d

      - name: Wait for services
        run: sleep 30

      - name: Run K6 load test
        uses: grafana/k6-action@v0.3.1
        with:
          filename: benchmarks/k6/load-test.js

      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: k6-results
          path: benchmarks/results/
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `API_URL` | `http://localhost:8080` | Target API base URL |
| `WS_URL` | `ws://localhost:8080` | WebSocket URL |
| `K6_VUS` | Varies | Override virtual users |
| `K6_DURATION` | Varies | Override test duration |
| `AUTH_TOKEN` | None | Bearer token for authenticated endpoints |

## Troubleshooting

### High Error Rates

1. Check if services are running: `docker-compose ps`
2. Verify connectivity: `curl http://localhost:8080/health`
3. Check resource usage: `docker stats`
4. Review logs: `docker-compose logs -f`

### Slow Response Times

1. Check database connections: `docker-compose exec db psql -c "SELECT count(*) FROM pg_stat_activity;"`
2. Monitor CPU/Memory: `htop` or `docker stats`
3. Check for connection pool exhaustion
4. Review slow query logs

### Connection Errors

1. Increase system file limits: `ulimit -n 65535`
2. Check firewall rules
3. Verify port availability
4. Monitor TCP connections: `netstat -an | grep 8080`

## Contributing

When adding new benchmarks:

1. Add test file to appropriate directory (`k6/` or `locust/`)
2. Update this README with new scenarios
3. Add threshold assertions
4. Include cleanup logic
5. Document expected results
