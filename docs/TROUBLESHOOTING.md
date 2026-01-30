# Project Apex - Troubleshooting Guide

> Comprehensive guide for diagnosing and resolving common issues in Apex deployments

## Table of Contents

1. [Common Issues and Solutions](#common-issues-and-solutions)
   - [API Not Starting](#api-not-starting)
   - [Database Connection Issues](#database-connection-issues)
   - [Redis Connection Issues](#redis-connection-issues)
   - [Worker Not Processing Tasks](#worker-not-processing-tasks)
   - [WebSocket Disconnections](#websocket-disconnections)
   - [High Memory Usage](#high-memory-usage)
   - [Slow Response Times](#slow-response-times)
2. [Debugging Techniques](#debugging-techniques)
   - [Reading Logs](#reading-logs)
   - [Using Jaeger Traces](#using-jaeger-traces)
   - [Prometheus Queries](#prometheus-queries)
   - [Database Debugging](#database-debugging)
3. [Health Check Interpretation](#health-check-interpretation)
4. [Error Code Reference](#error-code-reference)
5. [FAQ](#faq)
6. [Getting Help](#getting-help)

---

## Common Issues and Solutions

### API Not Starting

#### Symptoms
- API container exits immediately or crashes
- `curl http://localhost:8080/health` returns connection refused
- Logs show startup errors

#### Diagnostic Steps

```bash
# Check container status
docker-compose ps

# View API logs
docker-compose logs apex-api

# For Kubernetes
kubectl logs -f deployment/apex-api -n apex
kubectl describe pod -l app=apex-api -n apex
```

#### Common Causes and Solutions

**1. Database not ready**

The API depends on PostgreSQL being healthy before starting.

```bash
# Check if PostgreSQL is ready
docker-compose exec postgres pg_isready -U apex -d apex

# Wait for database and restart API
docker-compose restart apex-api
```

**2. Missing or invalid migrations**

```bash
# Check migration status
docker-compose exec postgres psql -U apex -d apex -c "SELECT * FROM _sqlx_migrations;"

# Re-run migrations (if using sqlx)
docker-compose exec apex-api ./apex-core migrate

# Or apply migrations manually
docker-compose exec postgres psql -U apex -d apex -f /docker-entrypoint-initdb.d/001_initial.sql
```

**3. Invalid configuration / environment variables**

```bash
# Verify environment variables are set
docker-compose exec apex-api env | grep -E "(DATABASE|REDIS|RUST_LOG)"

# Common missing variables:
# - DATABASE_URL: postgres://apex:apex_secret@postgres:5432/apex
# - REDIS_URL: redis://redis:6379
# - RUST_LOG: info,apex_core=debug
```

**4. Port already in use**

```bash
# Check what's using port 8080
lsof -i :8080

# Kill the process or change the port in docker-compose.yml
kill -9 <PID>
```

**5. Insufficient memory**

```bash
# Check container resource usage
docker stats apex-api

# Increase memory limits in docker-compose.yml
deploy:
  resources:
    limits:
      memory: 4G
```

---

### Database Connection Issues

#### Symptoms
- API logs show "connection refused" or "connection timed out"
- Tasks stuck in pending state
- `pg_isready` fails

#### Diagnostic Steps

```bash
# Check PostgreSQL container health
docker-compose ps postgres
docker-compose logs postgres

# Test connection from API container
docker-compose exec apex-api nc -zv postgres 5432

# Check PostgreSQL connections
docker-compose exec postgres psql -U apex -d apex -c "SELECT count(*) FROM pg_stat_activity;"
```

#### Common Causes and Solutions

**1. PostgreSQL container not running**

```bash
# Restart PostgreSQL
docker-compose up -d postgres

# Wait for health check to pass
docker-compose exec postgres pg_isready -U apex -d apex
```

**2. Connection pool exhausted**

```bash
# Check active connections
docker-compose exec postgres psql -U apex -d apex -c "
SELECT
    state,
    count(*)
FROM pg_stat_activity
WHERE datname = 'apex'
GROUP BY state;
"

# Kill idle connections
docker-compose exec postgres psql -U apex -d apex -c "
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE datname = 'apex'
  AND state = 'idle'
  AND query_start < now() - interval '5 minutes';
"

# Increase max_connections in postgresql.conf or use pgbouncer
```

**3. Authentication failure**

```bash
# Verify credentials
docker-compose exec postgres psql -U apex -d apex -c "SELECT 1;"

# Reset password if needed
docker-compose exec postgres psql -U postgres -c "ALTER USER apex WITH PASSWORD 'apex_secret';"

# Update DATABASE_URL in .env
DATABASE_URL=postgres://apex:apex_secret@postgres:5432/apex
```

**4. Network connectivity**

```bash
# Verify network exists
docker network ls | grep apex

# Ensure containers are on the same network
docker network inspect apex_apex-internal

# Recreate network if needed
docker-compose down
docker-compose up -d
```

**5. Data corruption or disk full**

```bash
# Check disk space
docker-compose exec postgres df -h /var/lib/postgresql/data

# Check PostgreSQL logs for corruption
docker-compose logs postgres | grep -i "error\|corrupt\|invalid"

# If corrupted, restore from backup or recreate
docker-compose down -v
docker-compose up -d postgres
```

---

### Redis Connection Issues

#### Symptoms
- Workers can't pull tasks from queue
- Real-time updates not working
- API logs show Redis connection errors

#### Diagnostic Steps

```bash
# Check Redis container
docker-compose ps redis
docker-compose logs redis

# Test Redis connection
docker-compose exec redis redis-cli ping
# Expected: PONG

# Check Redis info
docker-compose exec redis redis-cli info
```

#### Common Causes and Solutions

**1. Redis container not running**

```bash
docker-compose up -d redis

# Verify it's accepting connections
docker-compose exec redis redis-cli ping
```

**2. Memory limit reached**

Redis is configured with `maxmemory 256mb` by default.

```bash
# Check memory usage
docker-compose exec redis redis-cli info memory | grep -E "used_memory|maxmemory"

# Check eviction policy
docker-compose exec redis redis-cli config get maxmemory-policy

# Clear old data if needed (be careful in production!)
docker-compose exec redis redis-cli FLUSHDB

# Or increase memory limit in docker-compose.yml
command: redis-server --maxmemory 512mb --maxmemory-policy allkeys-lru
```

**3. Connection timeout**

```bash
# Check number of connected clients
docker-compose exec redis redis-cli info clients

# Check for slow commands
docker-compose exec redis redis-cli slowlog get 10

# Increase timeout in client configuration
APEX_REDIS_TIMEOUT=30
```

**4. Wrong URL/port**

```bash
# Verify REDIS_URL environment variable
echo $REDIS_URL

# Should be: redis://redis:6379 (for Docker)
# Or: redis://localhost:6379 (for local development)
```

**5. Persistence issues**

```bash
# Check AOF/RDB status
docker-compose exec redis redis-cli info persistence

# Force save
docker-compose exec redis redis-cli BGSAVE
```

---

### Worker Not Processing Tasks

#### Symptoms
- Tasks remain in "pending" status indefinitely
- Worker logs show no activity
- Queue depth increasing

#### Diagnostic Steps

```bash
# Check worker status
docker-compose ps apex-worker
docker-compose logs -f apex-worker

# Check queue depth
docker-compose exec redis redis-cli LLEN apex:tasks:queue

# Check for worker heartbeats
docker-compose exec redis redis-cli KEYS "apex:workers:heartbeat:*"
```

#### Common Causes and Solutions

**1. Missing LLM API keys**

```bash
# Check if API keys are set
docker-compose exec apex-worker env | grep -E "(OPENAI|ANTHROPIC)_API_KEY"

# Set in .env file
OPENAI_API_KEY=sk-your-key-here
ANTHROPIC_API_KEY=sk-ant-your-key-here

# Restart workers
docker-compose restart apex-worker
```

**2. Redis connection failed**

```bash
# Test from worker container
docker-compose exec apex-worker python -c "
import redis
r = redis.from_url('redis://redis:6379')
print(r.ping())
"
```

**3. Worker crashed / OOM killed**

```bash
# Check for OOM events
docker-compose logs apex-worker | grep -i "killed\|oom"

# Check resource usage
docker stats apex-worker

# Increase memory limit
deploy:
  resources:
    limits:
      memory: 2G
```

**4. LLM provider rate limiting**

```bash
# Check for rate limit errors
docker-compose logs apex-worker | grep -i "rate limit\|429"

# Solutions:
# - Reduce number of concurrent agents: APEX_WORKER_NUM_AGENTS=3
# - Add retry delay: APEX_LLM_MAX_RETRIES=5
# - Use different API keys with higher limits
```

**5. Task deserialization error**

```bash
# Check for JSON parsing errors
docker-compose logs apex-worker | grep -i "json\|parse\|deserialize"

# Inspect a task from the queue
docker-compose exec redis redis-cli LRANGE apex:tasks:queue 0 0
```

**6. Wrong number of workers/agents**

```bash
# Check current configuration
docker-compose exec apex-worker env | grep APEX_WORKER

# Adjust settings
APEX_WORKER_NUM_AGENTS=5
APEX_WORKER_POLL_INTERVAL_SECONDS=1.0
```

---

### WebSocket Disconnections

#### Symptoms
- Dashboard shows "Disconnected" status
- Real-time updates stop working
- Frequent reconnection attempts in browser console

#### Diagnostic Steps

```bash
# Test WebSocket endpoint
wscat -c ws://localhost:8080/ws

# Check API logs for WebSocket errors
docker-compose logs apex-api | grep -i "websocket\|ws\|upgrade"

# Check connection count
docker-compose exec redis redis-cli PUBSUB NUMSUB apex:events
```

#### Common Causes and Solutions

**1. Proxy/load balancer not configured for WebSocket**

For nginx:
```nginx
location /ws {
    proxy_pass http://apex-api:8080;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_read_timeout 86400;
}
```

For Kubernetes ingress (nginx-ingress):
```yaml
annotations:
  nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
  nginx.ingress.kubernetes.io/proxy-send-timeout: "3600"
  nginx.org/websocket-services: "apex-api"
```

**2. Connection timeout**

```bash
# Increase client ping interval
# In Python SDK:
client = ApexWebSocketClient(
    base_url="http://localhost:8080",
    ping_interval=30.0,
    ping_timeout=10.0
)
```

**3. Authentication issues**

```bash
# Verify API key is being sent
curl -X GET http://localhost:8080/ws \
  -H "Upgrade: websocket" \
  -H "X-API-Key: your-api-key" \
  -v
```

**4. Too many concurrent connections**

```bash
# Check current connections
docker-compose exec apex-api netstat -an | grep 8080 | wc -l

# Limit connections per client in API config
APEX__API__MAX_WS_CONNECTIONS_PER_CLIENT=10
```

**5. Memory pressure causing disconnects**

```bash
# Monitor API memory during connections
docker stats apex-api

# Consider using connection pooling or reducing message frequency
```

---

### High Memory Usage

#### Symptoms
- Containers being OOM killed
- System becoming unresponsive
- Prometheus alerts for high memory usage

#### Diagnostic Steps

```bash
# Check all container memory usage
docker stats --no-stream

# For Kubernetes
kubectl top pods -n apex

# Check for memory leaks in logs
docker-compose logs apex-api | grep -i "memory\|heap\|oom"
```

#### Common Causes and Solutions

**1. API server memory growth**

```bash
# Enable memory profiling (Rust)
RUST_LOG=apex_core=debug,memory=trace

# Check for connection leaks
docker-compose exec apex-api netstat -an | grep ESTABLISHED | wc -l

# Restart to reclaim memory
docker-compose restart apex-api
```

**2. Worker memory accumulation**

```bash
# Profile Python memory
docker-compose exec apex-worker python -c "
import tracemalloc
tracemalloc.start()
# Run some tasks...
snapshot = tracemalloc.take_snapshot()
for stat in snapshot.statistics('lineno')[:10]:
    print(stat)
"

# Reduce concurrent agents
APEX_WORKER_NUM_AGENTS=3
```

**3. Redis memory growth**

```bash
# Analyze key sizes
docker-compose exec redis redis-cli --bigkeys

# Set TTL on temporary keys
docker-compose exec redis redis-cli CONFIG SET maxmemory-policy allkeys-lru

# Clear old data
docker-compose exec redis redis-cli FLUSHDB
```

**4. PostgreSQL memory**

```bash
# Check PostgreSQL memory settings
docker-compose exec postgres psql -U apex -c "SHOW shared_buffers;"
docker-compose exec postgres psql -U apex -c "SHOW work_mem;"

# Tune settings in postgresql.conf
shared_buffers = 256MB
work_mem = 4MB
```

**5. Large task payloads**

```bash
# Check average task size
docker-compose exec redis redis-cli DEBUG OBJECT apex:tasks:queue

# Limit task payload size in API
APEX__ORCHESTRATOR__MAX_TASK_PAYLOAD_SIZE=1048576  # 1MB
```

---

### Slow Response Times

#### Symptoms
- API requests taking > 2 seconds
- Dashboard feels sluggish
- High P95 latency in metrics

#### Diagnostic Steps

```bash
# Check API latency metrics
curl -s http://localhost:9090/api/v1/query?query=histogram_quantile\(0.95,rate\(apex_api_request_duration_seconds_bucket[5m]\)\) | jq

# Check database query times
docker-compose exec postgres psql -U apex -d apex -c "
SELECT
    query,
    calls,
    mean_exec_time,
    max_exec_time
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 10;
"
```

#### Common Causes and Solutions

**1. Database slow queries**

```bash
# Enable slow query logging
docker-compose exec postgres psql -U apex -c "ALTER SYSTEM SET log_min_duration_statement = 1000;"
docker-compose exec postgres psql -U apex -c "SELECT pg_reload_conf();"

# Add missing indexes
docker-compose exec postgres psql -U apex -d apex -c "
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
"

# Analyze tables
docker-compose exec postgres psql -U apex -d apex -c "ANALYZE;"
```

**2. Redis latency**

```bash
# Check Redis latency
docker-compose exec redis redis-cli --latency

# Check slow log
docker-compose exec redis redis-cli SLOWLOG GET 10

# Enable pipelining in client
```

**3. LLM API latency**

```bash
# Check model routing config
APEX__ROUTING__ECONOMY_MODEL=gpt-4o-mini  # Faster/cheaper
APEX__ROUTING__CONFIDENCE_THRESHOLD=0.80  # Use economy model more

# Add caching for similar requests
```

**4. Network latency between services**

```bash
# Test network latency
docker-compose exec apex-api ping -c 10 postgres
docker-compose exec apex-api ping -c 10 redis

# Ensure services are on the same Docker network
docker network inspect apex_apex-internal
```

**5. Insufficient resources**

```bash
# Scale horizontally
docker-compose up -d --scale apex-api=3 --scale apex-worker=5

# For Kubernetes, increase HPA limits
kubectl patch hpa apex-api -n apex -p '{"spec":{"maxReplicas":20}}'
```

---

## Debugging Techniques

### Reading Logs

#### Log Locations

| Service | Docker | Kubernetes |
|---------|--------|------------|
| API | `docker-compose logs apex-api` | `kubectl logs -l app=apex-api -n apex` |
| Worker | `docker-compose logs apex-worker` | `kubectl logs -l app=apex-worker -n apex` |
| PostgreSQL | `docker-compose logs postgres` | `kubectl logs -l app=postgresql -n apex` |
| Redis | `docker-compose logs redis` | `kubectl logs -l app=redis -n apex` |

#### Log Level Configuration

```bash
# API (Rust) - via RUST_LOG
RUST_LOG=info,apex_core=debug,tower_http=trace

# Worker (Python) - via APEX_LOG_LEVEL
APEX_LOG_LEVEL=DEBUG
APEX_DEBUG=true

# Enable JSON logging for production
APEX_LOG_JSON=true
```

#### Useful Log Filtering

```bash
# Filter by log level
docker-compose logs apex-api 2>&1 | grep -E "ERROR|WARN"

# Filter by task ID
docker-compose logs apex-worker 2>&1 | grep "task_id=550e8400"

# Filter by time range
docker-compose logs --since="2024-01-15T10:00:00" apex-api

# Follow logs with timestamp
docker-compose logs -f --timestamps apex-api
```

#### Structured Log Queries (with Loki)

```logql
# Find errors in last hour
{container="apex-api"} |= "ERROR" | json | level="error"

# Find slow tasks
{container="apex-worker"} | json | duration_ms > 10000

# Find specific task
{container=~"apex-.*"} | json | task_id="550e8400-e29b-41d4-a716-446655440000"
```

---

### Using Jaeger Traces

Access Jaeger UI at: http://localhost:16686

#### Finding Traces

1. **By Service**: Select `apex-api` or `apex-agents` from the Service dropdown
2. **By Operation**: Filter by operation name like `POST /api/v1/tasks`
3. **By Tag**: Search for `task.id=<uuid>` or `error=true`
4. **By Duration**: Set min/max duration to find slow traces

#### Useful Queries

```
# Find all failed tasks
service=apex-agents AND error=true

# Find slow database queries
service=apex-api AND db.statement

# Find tasks by ID
service=apex-agents AND task.id="550e8400-e29b-41d4-a716-446655440000"

# Find high-cost LLM calls
service=apex-agents AND llm.cost>0.01
```

#### Understanding Trace Structure

```
apex-api: POST /api/v1/tasks
├── validate_request
├── create_task (PostgreSQL)
│   └── INSERT INTO tasks...
├── enqueue_task (Redis)
│   └── LPUSH apex:tasks:queue
└── broadcast_event (WebSocket)

apex-agents: execute_task
├── pull_task (Redis)
│   └── BRPOP apex:tasks:queue
├── agent_run
│   ├── llm_request (OpenAI/Anthropic)
│   ├── tool_execution [optional]
│   │   └── specific tool spans
│   └── llm_request [if multi-turn]
├── report_result (Redis)
└── update_backend (HTTP)
```

---

### Prometheus Queries

Access Prometheus at: http://localhost:9090

#### Essential Queries

**System Health**
```promql
# Active agents
apex_active_agents

# Task queue depth
apex_queue_depth

# Tasks per second
rate(apex_tasks_total[5m])

# Error rate
sum(rate(apex_tasks_failed[5m])) / sum(rate(apex_tasks_total[5m])) * 100
```

**Performance**
```promql
# P95 task latency
histogram_quantile(0.95, rate(apex_task_duration_seconds_bucket[5m]))

# P99 API latency
histogram_quantile(0.99, rate(apex_api_request_duration_seconds_bucket[5m]))

# Average tokens per task
rate(apex_tokens_used_total[5m]) / rate(apex_tasks_completed_total[5m])
```

**Costs**
```promql
# Cost per hour
increase(apex_cost_total[1h])

# Cost by model
sum by (model) (increase(apex_cost_total[1h]))

# Average cost per task
rate(apex_cost_total[5m]) / rate(apex_tasks_completed_total[5m])
```

**Resources**
```promql
# Memory usage by container
container_memory_usage_bytes{name=~"apex-.*"}

# CPU usage by container
rate(container_cpu_usage_seconds_total{name=~"apex-.*"}[5m])

# Redis memory
redis_memory_used_bytes
```

#### Creating Alerts

Add to `/infra/observability/prometheus/rules/custom-alerts.yml`:

```yaml
groups:
  - name: custom-apex-alerts
    rules:
      - alert: TaskQueueBacklog
        expr: apex_queue_depth > 50
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Task queue backlog growing"
          description: "Queue has {{ $value }} tasks pending"
```

---

### Database Debugging

#### Useful Queries

**Check active queries**
```sql
SELECT
    pid,
    now() - pg_stat_activity.query_start AS duration,
    query,
    state
FROM pg_stat_activity
WHERE datname = 'apex'
  AND state != 'idle'
ORDER BY duration DESC;
```

**Find blocking queries**
```sql
SELECT
    blocked_locks.pid AS blocked_pid,
    blocked_activity.usename AS blocked_user,
    blocking_locks.pid AS blocking_pid,
    blocking_activity.usename AS blocking_user,
    blocked_activity.query AS blocked_statement,
    blocking_activity.query AS blocking_statement
FROM pg_catalog.pg_locks blocked_locks
JOIN pg_catalog.pg_stat_activity blocked_activity
    ON blocked_activity.pid = blocked_locks.pid
JOIN pg_catalog.pg_locks blocking_locks
    ON blocking_locks.locktype = blocked_locks.locktype
    AND blocking_locks.database IS NOT DISTINCT FROM blocked_locks.database
    AND blocking_locks.relation IS NOT DISTINCT FROM blocked_locks.relation
    AND blocking_locks.page IS NOT DISTINCT FROM blocked_locks.page
    AND blocking_locks.tuple IS NOT DISTINCT FROM blocked_locks.tuple
    AND blocking_locks.virtualxid IS NOT DISTINCT FROM blocked_locks.virtualxid
    AND blocking_locks.transactionid IS NOT DISTINCT FROM blocked_locks.transactionid
    AND blocking_locks.classid IS NOT DISTINCT FROM blocked_locks.classid
    AND blocking_locks.objid IS NOT DISTINCT FROM blocked_locks.objid
    AND blocking_locks.objsubid IS NOT DISTINCT FROM blocked_locks.objsubid
    AND blocking_locks.pid != blocked_locks.pid
JOIN pg_catalog.pg_stat_activity blocking_activity
    ON blocking_activity.pid = blocking_locks.pid
WHERE NOT blocked_locks.granted;
```

**Table sizes**
```sql
SELECT
    relname AS table,
    pg_size_pretty(pg_total_relation_size(relid)) AS size
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC;
```

**Index usage**
```sql
SELECT
    schemaname,
    relname,
    indexrelname,
    idx_scan,
    idx_tup_read,
    idx_tup_fetch
FROM pg_stat_user_indexes
ORDER BY idx_scan DESC;
```

**Kill a stuck query**
```sql
SELECT pg_terminate_backend(<pid>);
```

---

## Health Check Interpretation

### API Health Endpoints

| Endpoint | Purpose | Response |
|----------|---------|----------|
| `/health` | Basic liveness | `{"status": "healthy", "version": "0.1.0"}` |
| `/ready` | Readiness (dependencies) | `{"ready": true, "checks": {...}}` |
| `/live` | Kubernetes liveness | `{"alive": true}` |

### Readiness Check Details

```json
{
  "ready": true,
  "checks": {
    "database": "ok",       // PostgreSQL connection
    "redis": "ok",          // Redis connection
    "agents": "ok"          // At least one worker healthy
  }
}
```

#### Status Meanings

| Check | Status | Meaning |
|-------|--------|---------|
| database | `ok` | PostgreSQL responding to queries |
| database | `error` | Cannot connect or query failed |
| redis | `ok` | Redis PING successful |
| redis | `error` | Cannot connect to Redis |
| agents | `ok` | At least one worker heartbeat recent |
| agents | `degraded` | Fewer workers than expected |
| agents | `error` | No workers responding |

### Kubernetes Probe Configuration

```yaml
livenessProbe:
  httpGet:
    path: /live
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 10
  failureThreshold: 3

readinessProbe:
  httpGet:
    path: /ready
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 5
  failureThreshold: 3
```

---

## Error Code Reference

### HTTP Error Codes

| Code | Error | Description | Resolution |
|------|-------|-------------|------------|
| 400 | Bad Request | Invalid request body or parameters | Check request format against API docs |
| 401 | Unauthorized | Missing or invalid API key | Verify `Authorization` header |
| 403 | Forbidden | Insufficient permissions | Check API key permissions |
| 404 | Not Found | Resource doesn't exist | Verify resource ID |
| 409 | Conflict | Resource conflict (e.g., duplicate) | Check for existing resource |
| 422 | Validation Error | Request validation failed | Check field constraints |
| 429 | Rate Limited | Too many requests | Implement backoff, upgrade tier |
| 500 | Internal Error | Server-side error | Check API logs |
| 503 | Service Unavailable | Service temporarily down | Retry with backoff |

### Task Status Codes

| Status | Description | Next Steps |
|--------|-------------|------------|
| `pending` | Task created, waiting in queue | Wait or check queue depth |
| `ready` | Dependencies met, ready to execute | Worker should pick up soon |
| `running` | Currently being executed | Monitor progress |
| `completed` | Finished successfully | Retrieve results |
| `failed` | Execution failed | Check error message, retry |
| `cancelled` | Manually cancelled | N/A |

### Contract Violation Errors

| Error | Description | Resolution |
|-------|-------------|------------|
| `TOKEN_LIMIT_EXCEEDED` | Task exceeded token budget | Increase limit or simplify task |
| `COST_LIMIT_EXCEEDED` | Task exceeded cost budget | Increase limit or use cheaper model |
| `API_CALL_LIMIT_EXCEEDED` | Too many LLM API calls | Increase limit or simplify task |
| `TIME_LIMIT_EXCEEDED` | Task timed out | Increase time limit or break into subtasks |

### WebSocket Error Codes

| Code | Description | Resolution |
|------|-------------|------------|
| 1000 | Normal closure | N/A (expected) |
| 1001 | Going away | Server shutting down, reconnect |
| 1006 | Abnormal closure | Network issue, reconnect |
| 1008 | Policy violation | Check authentication |
| 1011 | Server error | Check server logs |

---

## FAQ

### General

**Q: How do I check if Apex is running correctly?**

```bash
# Quick health check
curl http://localhost:8080/health

# Full readiness check
curl http://localhost:8080/ready

# Check all services
make health
# Or
docker-compose ps
```

**Q: How do I reset everything and start fresh?**

```bash
# Stop all services and remove volumes
docker-compose down -v

# Remove any cached images (optional)
docker-compose build --no-cache

# Start fresh
docker-compose up -d
```

**Q: Where are the logs stored?**

- Docker: Use `docker-compose logs <service>`
- Kubernetes: Check Loki at http://localhost:3100 or use `kubectl logs`
- Local files: Not persisted by default; configure Loki for retention

### Tasks

**Q: Why is my task stuck in "pending"?**

1. Check if workers are running: `docker-compose ps apex-worker`
2. Check queue depth: `docker-compose exec redis redis-cli LLEN apex:tasks:queue`
3. Check worker logs: `docker-compose logs apex-worker`
4. Verify LLM API keys are set

**Q: How do I cancel a running task?**

```bash
curl -X POST http://localhost:8080/api/v1/tasks/<task_id>/cancel \
  -H "Authorization: Bearer <api-key>"
```

**Q: Why did my task fail?**

1. Get task details: `curl http://localhost:8080/api/v1/tasks/<task_id>`
2. Check the `error` field in the response
3. Look at Jaeger trace for the task ID
4. Check worker logs around the failure time

### Performance

**Q: How do I scale for more throughput?**

```bash
# Docker Compose
docker-compose up -d --scale apex-worker=10

# Kubernetes
kubectl scale deployment apex-worker --replicas=10 -n apex
```

**Q: How do I reduce costs?**

1. Use economy models: Set `APEX__ROUTING__ECONOMY_MODEL=gpt-4o-mini`
2. Lower confidence threshold: `APEX__ROUTING__CONFIDENCE_THRESHOLD=0.75`
3. Set tighter task limits
4. Enable caching for similar requests

**Q: Why are tasks slow?**

1. Check Jaeger for trace breakdown
2. Verify LLM provider isn't rate limiting
3. Check database query performance
4. Monitor resource utilization

### Integrations

**Q: How do I add a new LLM provider?**

Currently supported: OpenAI, Anthropic. To add:
1. Set API key: `APEX_LLM_<PROVIDER>_API_KEY`
2. Configure model in routing: `APEX__ROUTING__<TIER>_MODEL`

**Q: Can I use a local LLM?**

Yes, with Ollama:
```bash
APEX_LLM_OLLAMA_BASE_URL=http://localhost:11434
APEX__ROUTING__ECONOMY_MODEL=ollama/llama2
```

---

## Getting Help

### Self-Service Resources

1. **Documentation**
   - [Architecture Guide](/docs/ARCHITECTURE.md)
   - [API Reference](/docs/API.md)
   - [Deployment Guide](/docs/DEPLOYMENT.md)

2. **Observability**
   - Grafana Dashboards: http://localhost:3001
   - Jaeger Traces: http://localhost:16686
   - Prometheus Metrics: http://localhost:9090

### GitHub

**Reporting Issues**

1. Search [existing issues](https://github.com/apex-swarm/apex/issues) first
2. Use the issue template
3. Include:
   - Apex version (`docker-compose exec apex-api ./apex-core --version`)
   - Environment (Docker/Kubernetes, OS)
   - Steps to reproduce
   - Relevant logs (sanitized of secrets)
   - Expected vs actual behavior

**Starting a Discussion**

For questions, ideas, or general help:
- [GitHub Discussions](https://github.com/apex-swarm/apex/discussions)

### Collecting Debug Information

When reporting issues, collect this information:

```bash
# System info
uname -a
docker --version
docker-compose --version

# Service status
docker-compose ps

# Recent logs (sanitize secrets!)
docker-compose logs --tail=100 apex-api > api-logs.txt
docker-compose logs --tail=100 apex-worker > worker-logs.txt

# Configuration (remove sensitive values)
docker-compose config | grep -v "KEY\|SECRET\|PASSWORD" > config.txt

# Resource usage
docker stats --no-stream > resources.txt

# Database info
docker-compose exec postgres psql -U apex -d apex -c "SELECT version();"
docker-compose exec postgres psql -U apex -d apex -c "SELECT count(*) FROM tasks;"
```

### Emergency Contacts

For production emergencies (if applicable):
- Check your organization's on-call procedures
- Escalate through your incident management system

---

## Appendix: Quick Reference Commands

```bash
# Start all services
docker-compose up -d

# Stop all services
docker-compose down

# View logs
docker-compose logs -f <service>

# Restart a service
docker-compose restart <service>

# Check service health
curl http://localhost:8080/health

# Check queue depth
docker-compose exec redis redis-cli LLEN apex:tasks:queue

# Check database connections
docker-compose exec postgres psql -U apex -d apex -c "SELECT count(*) FROM pg_stat_activity;"

# Scale workers
docker-compose up -d --scale apex-worker=5

# Enter container shell
docker-compose exec apex-api /bin/sh
docker-compose exec apex-worker /bin/bash

# View resource usage
docker stats

# Clean up old containers/images
docker system prune -a
```
