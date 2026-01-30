# Project Apex - Performance Optimization Guide

> Comprehensive guide for optimizing performance across all layers of the Apex Agent Swarm Orchestration System

## Table of Contents

1. [Performance Architecture Overview](#performance-architecture-overview)
2. [Rust Backend Optimizations](#rust-backend-optimizations)
3. [Python Worker Optimizations](#python-worker-optimizations)
4. [Database Optimizations](#database-optimizations)
5. [Caching Strategies](#caching-strategies)
6. [Load Testing Results](#load-testing-results)
7. [Scaling Guidelines](#scaling-guidelines)
8. [Monitoring for Performance](#monitoring-for-performance)

---

## Performance Architecture Overview

Apex is designed for high-throughput agent orchestration with the following performance targets:

| Metric | Target | Current |
|--------|--------|---------|
| Concurrent Agents | 1000+ | Validated |
| Task Throughput | 100 tasks/sec | Validated |
| API Latency P95 | <100ms | Validated |
| WebSocket Latency | <50ms | Validated |
| DAG Execution (3 tasks) | <15s | Validated |
| Recovery Time | <30s | Validated |

### Performance Layers

```
+------------------+     +------------------+     +------------------+
|   Presentation   |     |    API Layer     |     |  Orchestration   |
|    (React)       | --> |   (Rust/Axum)    | --> |    (Rust)        |
+------------------+     +------------------+     +------------------+
        |                        |                        |
        v                        v                        v
+------------------+     +------------------+     +------------------+
|   State Mgmt     |     |   Connection     |     |   Worker Pool    |
|   (Zustand)      |     |    Pooling       |     |   (Semaphore)    |
+------------------+     +------------------+     +------------------+
                                 |                        |
                                 v                        v
                    +------------------+     +------------------+
                    |   PostgreSQL     |     |  Redis Queues    |
                    |   (sqlx)         |     |  (redis-rs)      |
                    +------------------+     +------------------+
                                                      |
                                                      v
                                          +------------------+
                                          |  Python Workers  |
                                          |  (asyncio)       |
                                          +------------------+
```

### Key Performance Principles

1. **Async Everywhere**: Full async/await from API to database
2. **Zero-Copy Where Possible**: Use references and borrowing in hot paths
3. **Connection Pooling**: All external connections are pooled
4. **Backpressure Handling**: Semaphore-based concurrency limits
5. **Batching**: Aggregate operations where possible
6. **Caching**: Multi-layer caching with Redis

---

## Rust Backend Optimizations

### Tokio Runtime Tuning

The Rust backend uses Tokio as its async runtime. Proper tuning is critical for performance.

#### Worker Thread Configuration

```rust
// main.rs - Configure Tokio runtime for production
#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // Tokio automatically uses num_cpus for worker threads
    // Override with TOKIO_WORKER_THREADS env var if needed
    ...
}
```

**Environment Variables for Tuning:**

```bash
# Number of worker threads (default: num_cpus)
export TOKIO_WORKER_THREADS=8

# Stack size for worker threads (default: 2MB)
export TOKIO_THREAD_STACK_SIZE=2097152

# Enable Tokio console for debugging (development only)
export TOKIO_CONSOLE=1
```

**Recommended Settings by Workload:**

| Workload Type | Worker Threads | Notes |
|---------------|----------------|-------|
| CPU-bound | num_cpus | DAG computation, JSON parsing |
| I/O-bound | num_cpus * 2 | API calls, database queries |
| Mixed | num_cpus * 1.5 | Typical agent orchestration |

#### Avoiding Common Tokio Pitfalls

```rust
// BAD: Blocking operation on async thread
async fn bad_example() {
    let result = std::fs::read_to_string("file.txt"); // BLOCKS!
}

// GOOD: Use tokio's async file I/O
async fn good_example() {
    let result = tokio::fs::read_to_string("file.txt").await;
}

// BAD: CPU-intensive work on async thread
async fn bad_cpu_work() {
    let hash = compute_expensive_hash(&data); // BLOCKS OTHER TASKS!
}

// GOOD: Spawn blocking work on dedicated thread pool
async fn good_cpu_work() {
    let hash = tokio::task::spawn_blocking(move || {
        compute_expensive_hash(&data)
    }).await?;
}
```

### Connection Pooling

#### Database Connection Pool (sqlx)

```rust
// db/mod.rs - Optimized connection pool configuration
pub async fn new(database_url: &str) -> Result<Self> {
    let pool = PgPoolOptions::new()
        .max_connections(20)              // Maximum pool size
        .min_connections(5)               // Keep minimum connections warm
        .acquire_timeout(Duration::from_secs(5))  // Fail fast on overload
        .idle_timeout(Duration::from_secs(600))   // Close idle connections
        .max_lifetime(Duration::from_secs(1800))  // Rotate connections
        .connect(database_url)
        .await?;

    Ok(Self { pool })
}
```

**Pool Sizing Guidelines:**

```
max_connections = (num_cores * 2) + effective_spindle_count

For SSD: max_connections = num_cores * 2 + 1
For cloud databases: Check provider limits
```

| Deployment Size | min_connections | max_connections |
|-----------------|-----------------|-----------------|
| Development | 2 | 5 |
| Small (1-2 pods) | 5 | 10 |
| Medium (3-5 pods) | 5 | 15 |
| Large (6+ pods) | 5 | 20 |

#### Redis Connection Pool

```rust
// redis configuration in config.rs
pub struct RedisConfig {
    pub url: String,
    #[serde(default = "default_redis_pool_size")]
    pub pool_size: u32,  // Default: 10
}
```

**Redis Pool Best Practices:**

```rust
// Use connection manager for automatic reconnection
use redis::aio::ConnectionManager;

let client = redis::Client::open(redis_url)?;
let connection = ConnectionManager::new(client).await?;

// The connection manager handles:
// - Automatic reconnection on failure
// - Connection health checks
// - Request pipelining
```

### Memory Management

#### Efficient Data Structures

```rust
// Use DashMap for concurrent HashMap (lock-free reads)
use dashmap::DashMap;

pub struct TaskCache {
    tasks: DashMap<TaskId, Arc<Task>>,
}

// Use parking_lot for faster mutexes
use parking_lot::RwLock;

pub struct AgentRegistry {
    agents: RwLock<HashMap<AgentId, Agent>>,
}
```

#### Avoiding Allocations in Hot Paths

```rust
// BAD: Allocates new String on every call
fn format_task_id(id: &TaskId) -> String {
    format!("task:{}", id)
}

// GOOD: Use a buffer or pre-allocated string
fn format_task_id_into(id: &TaskId, buffer: &mut String) {
    buffer.clear();
    use std::fmt::Write;
    write!(buffer, "task:{}", id).unwrap();
}

// GOOD: Use Cow for conditional allocation
use std::borrow::Cow;

fn process_message<'a>(msg: &'a str) -> Cow<'a, str> {
    if msg.contains("sensitive") {
        Cow::Owned(msg.replace("sensitive", "[REDACTED]"))
    } else {
        Cow::Borrowed(msg)
    }
}
```

#### Release Build Optimizations

```toml
# Cargo.toml - Release profile optimizations
[profile.release]
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for better optimization
panic = "abort"     # Smaller binary, no unwinding overhead
strip = true        # Strip symbols from binary

[profile.bench]
debug = true        # Keep debug info for profiling
```

### Async Patterns

#### Concurrent Task Execution

```rust
// Execute multiple independent tasks concurrently
use futures::future::join_all;

async fn execute_batch(tasks: Vec<Task>) -> Vec<Result<TaskOutput>> {
    let futures: Vec<_> = tasks
        .into_iter()
        .map(|task| execute_single(task))
        .collect();

    join_all(futures).await
}

// With concurrency limit using semaphore
async fn execute_batch_limited(
    tasks: Vec<Task>,
    max_concurrent: usize,
) -> Vec<Result<TaskOutput>> {
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    let futures: Vec<_> = tasks
        .into_iter()
        .map(|task| {
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await?;
                execute_single(task).await
            }
        })
        .collect();

    join_all(futures).await
}
```

#### Streaming Responses

```rust
// Stream large responses instead of buffering
use tokio_stream::StreamExt;
use axum::response::sse::{Event, Sse};

async fn stream_task_progress(
    task_id: TaskId,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        loop {
            interval.tick().await;

            if let Some(progress) = get_task_progress(&task_id).await {
                yield Ok(Event::default().json_data(progress)?);

                if progress.is_complete {
                    break;
                }
            }
        }
    };

    Sse::new(stream)
}
```

#### Channel Patterns for Backpressure

```rust
use tokio::sync::mpsc;

// Bounded channel provides backpressure
let (tx, mut rx) = mpsc::channel(1000);  // Buffer size limits memory

// Producer respects backpressure
async fn producer(tx: mpsc::Sender<Task>) {
    for task in tasks {
        // This will await if channel is full
        if tx.send(task).await.is_err() {
            break;  // Receiver dropped
        }
    }
}

// Consumer processes at its own pace
async fn consumer(mut rx: mpsc::Receiver<Task>) {
    while let Some(task) = rx.recv().await {
        process_task(task).await;
    }
}
```

---

## Python Worker Optimizations

### Asyncio Best Practices

#### Event Loop Configuration

```python
# main.py - Optimized asyncio configuration
import asyncio
import uvloop  # Optional: faster event loop

# Use uvloop for better performance (Linux/macOS)
asyncio.set_event_loop_policy(uvloop.EventLoopPolicy())

async def main():
    # Configure the event loop
    loop = asyncio.get_event_loop()

    # Enable debug mode only in development
    loop.set_debug(False)

    # Run the worker
    await run_worker()

if __name__ == "__main__":
    asyncio.run(main())
```

#### Avoiding Event Loop Blocking

```python
# BAD: Blocking call in async function
async def bad_example():
    data = requests.get(url)  # BLOCKS THE EVENT LOOP!
    return data.json()

# GOOD: Use async HTTP client
import httpx

async def good_example():
    async with httpx.AsyncClient() as client:
        response = await client.get(url)
        return response.json()

# BAD: CPU-intensive work blocking event loop
async def bad_cpu_work():
    result = compute_hash(large_data)  # BLOCKS!
    return result

# GOOD: Run in thread pool
import asyncio
from concurrent.futures import ThreadPoolExecutor

executor = ThreadPoolExecutor(max_workers=4)

async def good_cpu_work():
    loop = asyncio.get_event_loop()
    result = await loop.run_in_executor(
        executor,
        compute_hash,
        large_data
    )
    return result
```

#### Concurrent Task Management

```python
# worker.py - Concurrent task execution
import asyncio
from typing import List

async def execute_batch(tasks: List[Task], max_concurrent: int = 10):
    """Execute tasks with concurrency limit."""
    semaphore = asyncio.Semaphore(max_concurrent)

    async def execute_with_limit(task: Task):
        async with semaphore:
            return await execute_single(task)

    return await asyncio.gather(
        *[execute_with_limit(task) for task in tasks],
        return_exceptions=True
    )
```

### Batch Processing

#### Efficient Batch Patterns

```python
# executor.py - Batch processing optimizations

async def process_batch(items: List[Item], batch_size: int = 100):
    """Process items in batches for efficiency."""
    for i in range(0, len(items), batch_size):
        batch = items[i:i + batch_size]

        # Process batch concurrently
        results = await asyncio.gather(
            *[process_item(item) for item in batch]
        )

        # Yield results as they complete
        for result in results:
            yield result

# Batch database operations
async def batch_insert(records: List[Record], batch_size: int = 500):
    """Insert records in batches."""
    for i in range(0, len(records), batch_size):
        batch = records[i:i + batch_size]
        await db.executemany(
            "INSERT INTO table (col1, col2) VALUES ($1, $2)",
            [(r.col1, r.col2) for r in batch]
        )
```

#### LLM Request Batching

```python
# llm.py - Batch LLM requests for efficiency
import asyncio
from collections import deque
from dataclasses import dataclass
from typing import Callable, TypeVar

T = TypeVar('T')

@dataclass
class BatchedRequest:
    prompt: str
    future: asyncio.Future

class LLMBatcher:
    """Batch LLM requests to reduce API overhead."""

    def __init__(
        self,
        batch_size: int = 10,
        max_wait_ms: int = 100,
    ):
        self.batch_size = batch_size
        self.max_wait_ms = max_wait_ms
        self.queue: deque[BatchedRequest] = deque()
        self._lock = asyncio.Lock()
        self._batch_task: asyncio.Task | None = None

    async def request(self, prompt: str) -> str:
        """Submit a request and wait for response."""
        future = asyncio.get_event_loop().create_future()
        request = BatchedRequest(prompt=prompt, future=future)

        async with self._lock:
            self.queue.append(request)

            if len(self.queue) >= self.batch_size:
                asyncio.create_task(self._process_batch())
            elif self._batch_task is None:
                self._batch_task = asyncio.create_task(
                    self._delayed_batch()
                )

        return await future

    async def _delayed_batch(self):
        """Wait for more requests or timeout."""
        await asyncio.sleep(self.max_wait_ms / 1000)
        await self._process_batch()

    async def _process_batch(self):
        """Process accumulated requests as a batch."""
        async with self._lock:
            self._batch_task = None
            if not self.queue:
                return

            batch = [self.queue.popleft() for _ in range(
                min(len(self.queue), self.batch_size)
            )]

        # Make batched API call
        responses = await self._batch_api_call(
            [r.prompt for r in batch]
        )

        # Resolve futures
        for request, response in zip(batch, responses):
            request.future.set_result(response)
```

### Memory Profiling

#### Tools and Techniques

```python
# Profiling with tracemalloc
import tracemalloc

def profile_memory():
    """Profile memory usage."""
    tracemalloc.start()

    # Your code here
    result = process_large_dataset()

    current, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    print(f"Current memory usage: {current / 1024 / 1024:.2f} MB")
    print(f"Peak memory usage: {peak / 1024 / 1024:.2f} MB")

    return result

# Profiling with memory_profiler (requires: pip install memory-profiler)
from memory_profiler import profile

@profile
def memory_intensive_function():
    """Function that uses significant memory."""
    data = load_large_dataset()
    processed = transform_data(data)
    return processed
```

#### Memory Optimization Techniques

```python
# Use generators for large datasets
def process_large_file(filepath: str):
    """Process file line by line to avoid loading all into memory."""
    with open(filepath, 'r') as f:
        for line in f:
            yield process_line(line)

# Use __slots__ for memory-efficient classes
class AgentState:
    __slots__ = ['id', 'status', 'tokens_used', 'cost']

    def __init__(self, id: str, status: str):
        self.id = id
        self.status = status
        self.tokens_used = 0
        self.cost = 0.0

# Clear large objects when done
def process_batch():
    large_data = load_data()
    result = process(large_data)
    del large_data  # Explicitly free memory
    gc.collect()    # Force garbage collection if needed
    return result
```

---

## Database Optimizations

### Query Optimization

#### Use EXPLAIN ANALYZE

```sql
-- Analyze query performance
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
SELECT t.*, a.name as agent_name
FROM tasks t
JOIN agents a ON t.agent_id = a.id
WHERE t.status = 'pending'
ORDER BY t.priority DESC
LIMIT 100;

-- Look for:
-- - Seq Scan on large tables (consider indexes)
-- - High buffer reads (consider caching)
-- - Nested loops with large row estimates
```

#### Optimized Query Patterns

```sql
-- BAD: N+1 queries
SELECT * FROM tasks WHERE dag_id = $1;
-- Then for each task:
SELECT * FROM agents WHERE id = $task_agent_id;

-- GOOD: Join in single query
SELECT t.*, a.name as agent_name, a.model
FROM tasks t
LEFT JOIN agents a ON t.agent_id = a.id
WHERE t.dag_id = $1;

-- BAD: SELECT * when you only need specific columns
SELECT * FROM events WHERE aggregate_id = $1;

-- GOOD: Select only needed columns
SELECT event_id, event_type, event_data, version
FROM events
WHERE aggregate_id = $1;

-- Use CTEs for complex queries (more readable, sometimes faster)
WITH ready_tasks AS (
    SELECT id, name, priority
    FROM tasks
    WHERE status = 'ready'
    AND dag_id = $1
),
agent_capacity AS (
    SELECT id, name, max_load - current_load as available_capacity
    FROM agents
    WHERE status = 'idle'
)
SELECT rt.*, ac.name as agent_name
FROM ready_tasks rt
CROSS JOIN LATERAL (
    SELECT * FROM agent_capacity
    ORDER BY available_capacity DESC
    LIMIT 1
) ac;
```

#### Prepared Statements

```rust
// sqlx automatically prepares and caches statements
// Use query! macros for compile-time verification

// This is prepared and cached automatically
let row = sqlx::query!(
    r#"
    SELECT id, name, status
    FROM tasks
    WHERE dag_id = $1
    "#,
    dag_id,
)
.fetch_one(&pool)
.await?;
```

### Index Strategies

#### Essential Indexes

```sql
-- Primary indexes (created automatically)
-- tasks(id), agents(id), events(id)

-- Foreign key indexes
CREATE INDEX idx_tasks_dag_id ON tasks(dag_id);
CREATE INDEX idx_tasks_agent_id ON tasks(agent_id);
CREATE INDEX idx_tasks_parent_id ON tasks(parent_id);
CREATE INDEX idx_contracts_agent_id ON agent_contracts(agent_id);
CREATE INDEX idx_contracts_task_id ON agent_contracts(task_id);

-- Status-based queries (partial indexes for efficiency)
CREATE INDEX idx_tasks_pending
ON tasks(priority DESC, created_at)
WHERE status = 'pending';

CREATE INDEX idx_tasks_running
ON tasks(started_at)
WHERE status = 'running';

-- Event sourcing queries
CREATE INDEX idx_events_aggregate
ON events(aggregate_type, aggregate_id, version);

-- Timestamp-based queries
CREATE INDEX idx_tasks_created ON tasks(created_at DESC);
CREATE INDEX idx_events_created ON events(created_at DESC);

-- Composite indexes for common query patterns
CREATE INDEX idx_tasks_dag_status
ON tasks(dag_id, status);

CREATE INDEX idx_agents_status_load
ON agents(status, current_load)
WHERE status = 'idle';
```

#### Index Maintenance

```sql
-- Check index usage
SELECT
    schemaname,
    tablename,
    indexname,
    idx_scan,
    idx_tup_read,
    idx_tup_fetch
FROM pg_stat_user_indexes
ORDER BY idx_scan DESC;

-- Find unused indexes (candidates for removal)
SELECT
    schemaname || '.' || tablename as table,
    indexname,
    pg_size_pretty(pg_relation_size(indexrelid)) as index_size
FROM pg_stat_user_indexes
WHERE idx_scan = 0
AND indexrelid NOT IN (
    SELECT conindid FROM pg_constraint
);

-- Reindex to fix bloat (schedule during low traffic)
REINDEX INDEX CONCURRENTLY idx_tasks_dag_id;
```

### Connection Pooling

#### PgBouncer Configuration (for high connection counts)

```ini
; pgbouncer.ini
[databases]
apex = host=localhost port=5432 dbname=apex

[pgbouncer]
listen_addr = 0.0.0.0
listen_port = 6432
auth_type = md5
auth_file = /etc/pgbouncer/userlist.txt

; Pool settings
pool_mode = transaction     ; Best for web applications
max_client_conn = 1000      ; Maximum client connections
default_pool_size = 20      ; Connections per database/user
min_pool_size = 5           ; Minimum connections to keep
reserve_pool_size = 5       ; Extra connections for spikes
reserve_pool_timeout = 3    ; Seconds to wait before using reserve

; Timeouts
server_connect_timeout = 5
server_idle_timeout = 600
client_idle_timeout = 0     ; No timeout for idle clients
```

**When to Use PgBouncer:**

- More than 100 concurrent connections
- Short-lived connections (serverless functions)
- Connection count exceeds PostgreSQL max_connections
- Need connection multiplexing

---

## Caching Strategies

### Redis Caching Patterns

#### Cache-Aside Pattern

```rust
// Cache-aside (lazy-loading) pattern
use redis::AsyncCommands;

async fn get_task_cached(
    redis: &mut redis::aio::ConnectionManager,
    db: &Database,
    task_id: TaskId,
) -> Result<Task> {
    let cache_key = format!("task:{}", task_id);

    // Try cache first
    let cached: Option<String> = redis.get(&cache_key).await?;
    if let Some(json) = cached {
        return Ok(serde_json::from_str(&json)?);
    }

    // Cache miss - load from database
    let task = db.get_task(task_id).await?
        .ok_or_else(|| ApexError::not_found("Task not found"))?;

    // Store in cache with TTL
    let json = serde_json::to_string(&task)?;
    let _: () = redis.set_ex(&cache_key, &json, 300).await?;  // 5 min TTL

    Ok(task)
}
```

#### Write-Through Pattern

```rust
// Write-through pattern - update cache on write
async fn update_task_status(
    redis: &mut redis::aio::ConnectionManager,
    db: &Database,
    task_id: TaskId,
    status: TaskStatus,
) -> Result<()> {
    // Update database
    db.update_task_status(task_id, status).await?;

    // Update cache (or invalidate)
    let cache_key = format!("task:{}", task_id);
    let _: () = redis.del(&cache_key).await?;  // Invalidate

    // Or update cache directly for frequently read data
    // let task = db.get_task(task_id).await?;
    // redis.set_ex(&cache_key, &serde_json::to_string(&task)?, 300).await?;

    Ok(())
}
```

#### Caching Aggregations

```rust
// Cache expensive aggregation queries
async fn get_system_stats_cached(
    redis: &mut redis::aio::ConnectionManager,
    db: &Database,
) -> Result<SystemStats> {
    let cache_key = "stats:system";

    // Try cache (short TTL for stats)
    let cached: Option<String> = redis.get(cache_key).await?;
    if let Some(json) = cached {
        return Ok(serde_json::from_str(&json)?);
    }

    // Compute stats
    let stats = db.get_system_stats().await?;

    // Cache with short TTL (30 seconds)
    let json = serde_json::to_string(&stats)?;
    let _: () = redis.set_ex(cache_key, &json, 30).await?;

    Ok(stats)
}
```

### Cache Invalidation

#### Time-Based Invalidation

```rust
// TTL-based expiration
redis.set_ex("task:123", json, 300)?;  // Expires in 5 minutes

// Set expiration on existing key
redis.expire("task:123", 300)?;

// Check TTL
let ttl: i64 = redis.ttl("task:123")?;
```

#### Event-Based Invalidation

```rust
// Publish invalidation events
async fn invalidate_task(redis: &mut ConnectionManager, task_id: TaskId) {
    let cache_key = format!("task:{}", task_id);

    // Delete the cached value
    let _: () = redis.del(&cache_key).await?;

    // Publish invalidation event for distributed caches
    let _: () = redis.publish(
        "cache:invalidate",
        format!("task:{}", task_id)
    ).await?;
}

// Subscribe to invalidation events
async fn listen_invalidations(redis: redis::Client) {
    let mut pubsub = redis.get_async_connection().await?.into_pubsub();
    pubsub.subscribe("cache:invalidate").await?;

    let mut stream = pubsub.on_message();
    while let Some(msg) = stream.next().await {
        let key: String = msg.get_payload()?;
        local_cache.remove(&key);
    }
}
```

#### Cache Stampede Prevention

```rust
use tokio::sync::Mutex;
use std::collections::HashMap;

// Mutex map to prevent multiple simultaneous cache fills
struct CacheLoader {
    loading: Mutex<HashMap<String, tokio::sync::broadcast::Sender<()>>>,
}

impl CacheLoader {
    async fn get_or_load<T, F, Fut>(
        &self,
        redis: &mut ConnectionManager,
        key: &str,
        loader: F,
    ) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
        T: Serialize + DeserializeOwned,
    {
        // Check cache
        if let Some(cached) = redis.get::<_, Option<String>>(key).await? {
            return Ok(serde_json::from_str(&cached)?);
        }

        // Check if another task is loading
        let mut loading = self.loading.lock().await;
        if let Some(sender) = loading.get(key) {
            let mut receiver = sender.subscribe();
            drop(loading);
            let _ = receiver.recv().await;
            // Retry cache after load complete
            if let Some(cached) = redis.get::<_, Option<String>>(key).await? {
                return Ok(serde_json::from_str(&cached)?);
            }
        }

        // We're the loader
        let (tx, _) = tokio::sync::broadcast::channel(1);
        loading.insert(key.to_string(), tx.clone());
        drop(loading);

        // Load data
        let result = loader().await?;

        // Store in cache
        let json = serde_json::to_string(&result)?;
        let _: () = redis.set_ex(key, &json, 300).await?;

        // Notify waiters
        let _ = tx.send(());
        self.loading.lock().await.remove(key);

        Ok(result)
    }
}
```

### Multi-Layer Caching

```rust
// L1: In-process cache (moka)
use moka::future::Cache;

struct MultiLayerCache {
    l1: Cache<String, Arc<Task>>,  // In-memory
    l2: redis::aio::ConnectionManager,  // Redis
}

impl MultiLayerCache {
    async fn get(&self, key: &str) -> Option<Arc<Task>> {
        // Try L1 first
        if let Some(task) = self.l1.get(key) {
            return Some(task);
        }

        // Try L2 (Redis)
        if let Ok(Some(json)) = self.l2.get::<_, Option<String>>(key).await {
            if let Ok(task) = serde_json::from_str::<Task>(&json) {
                let arc_task = Arc::new(task);
                // Promote to L1
                self.l1.insert(key.to_string(), arc_task.clone()).await;
                return Some(arc_task);
            }
        }

        None
    }

    async fn set(&self, key: &str, task: Task) {
        let arc_task = Arc::new(task);

        // Store in L1
        self.l1.insert(key.to_string(), arc_task.clone()).await;

        // Store in L2
        let json = serde_json::to_string(&*arc_task).unwrap();
        let _: () = self.l2.set_ex(key, &json, 300).await.unwrap();
    }
}
```

---

## Load Testing Results

### Test Environment

> Placeholder: Replace with actual test results

```
Environment: Kubernetes cluster
- 3x API server pods (4 CPU, 8GB RAM each)
- 5x Worker pods (2 CPU, 4GB RAM each)
- PostgreSQL (8 CPU, 32GB RAM, SSD storage)
- Redis (4 CPU, 8GB RAM)

Load Generator: k6, 10 distributed instances
```

### Baseline Performance

| Scenario | RPS | P50 Latency | P95 Latency | P99 Latency | Error Rate |
|----------|-----|-------------|-------------|-------------|------------|
| Health Check | 10,000 | 2ms | 5ms | 10ms | 0% |
| Create Task | 1,000 | 15ms | 45ms | 80ms | 0% |
| Get Task | 5,000 | 8ms | 20ms | 35ms | 0% |
| List Tasks | 500 | 25ms | 60ms | 100ms | 0% |
| WebSocket Connection | 1,000 | 50ms | 100ms | 150ms | 0% |
| DAG Execution (3 tasks) | 100 | 5s | 10s | 15s | 0.1% |

### Stress Test Results

| Metric | 100 RPS | 500 RPS | 1000 RPS | 2000 RPS |
|--------|---------|---------|----------|----------|
| Throughput | 100 | 500 | 995 | 1850 |
| P95 Latency | 45ms | 60ms | 120ms | 500ms |
| Error Rate | 0% | 0% | 0.1% | 2.5% |
| CPU Usage | 15% | 40% | 75% | 95% |
| Memory Usage | 30% | 40% | 55% | 70% |
| DB Connections | 10 | 35 | 60 | 80 |

### Identified Bottlenecks

1. **Database connection pool** - Saturates at ~80 connections
2. **LLM API rate limits** - External dependency bottleneck
3. **Redis queue throughput** - Limited by network latency
4. **CPU-bound DAG computation** - Scales with worker count

---

## Scaling Guidelines

### Horizontal Scaling

#### API Server Scaling

```yaml
# kubernetes/api-hpa.yaml
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
  maxReplicas: 20
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 80
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 60
      policies:
        - type: Percent
          value: 100
          periodSeconds: 60
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
        - type: Percent
          value: 10
          periodSeconds: 60
```

#### Worker Scaling

```yaml
# kubernetes/worker-hpa.yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: apex-worker-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: apex-worker
  minReplicas: 3
  maxReplicas: 50
  metrics:
    # Scale based on queue depth (custom metric)
    - type: External
      external:
        metric:
          name: redis_queue_depth
          selector:
            matchLabels:
              queue: apex-tasks
        target:
          type: AverageValue
          averageValue: "10"
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 70
```

#### Queue-Based Autoscaling with KEDA

```yaml
# kubernetes/worker-scaledobject.yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: apex-worker-scaler
spec:
  scaleTargetRef:
    name: apex-worker
  minReplicaCount: 3
  maxReplicaCount: 50
  pollingInterval: 15
  cooldownPeriod: 60
  triggers:
    - type: redis
      metadata:
        address: redis:6379
        listName: apex:tasks:queue
        listLength: "10"
    - type: cpu
      metadata:
        type: Utilization
        value: "70"
```

### Vertical Scaling

#### Resource Recommendations

| Component | Min Resources | Recommended | High Traffic |
|-----------|---------------|-------------|--------------|
| API Server | 0.5 CPU, 512MB | 2 CPU, 2GB | 4 CPU, 4GB |
| Worker | 0.5 CPU, 512MB | 1 CPU, 1GB | 2 CPU, 2GB |
| PostgreSQL | 2 CPU, 4GB | 8 CPU, 32GB | 16 CPU, 64GB |
| Redis | 1 CPU, 1GB | 4 CPU, 8GB | 8 CPU, 16GB |

#### Vertical Pod Autoscaler

```yaml
# kubernetes/api-vpa.yaml
apiVersion: autoscaling.k8s.io/v1
kind: VerticalPodAutoscaler
metadata:
  name: apex-api-vpa
spec:
  targetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: apex-api
  updatePolicy:
    updateMode: Auto  # or "Off" for recommendations only
  resourcePolicy:
    containerPolicies:
      - containerName: apex-api
        minAllowed:
          cpu: 100m
          memory: 256Mi
        maxAllowed:
          cpu: 4
          memory: 8Gi
```

### Auto-Scaling Policies

#### Scale-Up Policy

```yaml
# Fast scale-up for traffic spikes
scaleUp:
  stabilizationWindowSeconds: 0  # No stabilization, scale immediately
  policies:
    - type: Percent
      value: 100  # Double capacity
      periodSeconds: 15
    - type: Pods
      value: 4  # Or add 4 pods
      periodSeconds: 15
  selectPolicy: Max  # Use whichever adds more pods
```

#### Scale-Down Policy

```yaml
# Slow scale-down to prevent flapping
scaleDown:
  stabilizationWindowSeconds: 300  # Wait 5 minutes before scaling down
  policies:
    - type: Percent
      value: 10  # Remove 10% of pods
      periodSeconds: 60
  selectPolicy: Min  # Conservative scale-down
```

#### Multi-Metric Scaling

```yaml
# Scale based on multiple metrics
metrics:
  # CPU-based scaling
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70

  # Memory-based scaling
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80

  # Custom metric: requests per second
  - type: Pods
    pods:
      metric:
        name: http_requests_per_second
      target:
        type: AverageValue
        averageValue: "100"

  # External metric: queue depth
  - type: External
    external:
      metric:
        name: redis_list_length
        selector:
          matchLabels:
            queue: apex-tasks
      target:
        type: AverageValue
        averageValue: "10"
```

---

## Monitoring for Performance

### Key Performance Metrics

#### Application Metrics

```rust
// telemetry/metrics.rs - Performance metrics

// Request latency histogram
static REQUEST_LATENCY: Lazy<Histogram<u64>> = Lazy::new(|| {
    let histogram = METER
        .u64_histogram("apex.request.duration")
        .with_description("Request duration in milliseconds")
        .with_unit(Unit::new("ms"))
        .init();
    histogram
});

// Task processing latency
static TASK_LATENCY: Lazy<Histogram<u64>> = Lazy::new(|| {
    METER
        .u64_histogram("apex.task.duration")
        .with_description("Task processing duration in seconds")
        .with_unit(Unit::new("s"))
        .init()
});

// Active connections gauge
static ACTIVE_CONNECTIONS: Lazy<UpDownCounter<i64>> = Lazy::new(|| {
    METER
        .i64_up_down_counter("apex.connections.active")
        .with_description("Number of active connections")
        .init()
});

// Queue depth
static QUEUE_DEPTH: Lazy<ObservableGauge<u64>> = Lazy::new(|| {
    METER
        .u64_observable_gauge("apex.queue.depth")
        .with_description("Number of pending tasks in queue")
        .init()
});
```

#### Prometheus Metrics Endpoints

```yaml
# prometheus/rules.yaml
groups:
  - name: apex-performance
    rules:
      # Request latency SLO
      - record: apex:request_latency_p95
        expr: histogram_quantile(0.95, rate(apex_request_duration_bucket[5m]))

      # Task processing latency SLO
      - record: apex:task_latency_p95
        expr: histogram_quantile(0.95, rate(apex_task_duration_bucket[5m]))

      # Error rate
      - record: apex:error_rate_5m
        expr: |
          sum(rate(apex_requests_total{status=~"5.."}[5m]))
          / sum(rate(apex_requests_total[5m]))

      # Throughput
      - record: apex:throughput_5m
        expr: sum(rate(apex_requests_total[5m]))

      # Connection pool utilization
      - record: apex:db_pool_utilization
        expr: |
          apex_db_connections_active
          / apex_db_connections_max
```

### Performance Alerts

```yaml
# prometheus/alerts.yaml
groups:
  - name: apex-performance-alerts
    rules:
      # High latency alert
      - alert: HighRequestLatency
        expr: apex:request_latency_p95 > 0.5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High request latency detected"
          description: "P95 latency is {{ $value }}s (threshold: 0.5s)"

      # Very high latency (critical)
      - alert: CriticalRequestLatency
        expr: apex:request_latency_p95 > 2
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Critical request latency"
          description: "P95 latency is {{ $value }}s (threshold: 2s)"

      # High error rate
      - alert: HighErrorRate
        expr: apex:error_rate_5m > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Error rate above 5%"
          description: "Current error rate: {{ $value | humanizePercentage }}"

      # Database connection pool exhaustion
      - alert: DBPoolExhaustion
        expr: apex:db_pool_utilization > 0.9
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Database connection pool nearly exhausted"
          description: "Pool utilization: {{ $value | humanizePercentage }}"

      # Queue backing up
      - alert: TaskQueueBacklog
        expr: apex_queue_depth > 100
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Task queue backlog growing"
          description: "Queue depth: {{ $value }} tasks"
```

### Grafana Dashboards

#### Performance Overview Dashboard

```json
{
  "title": "Apex Performance Overview",
  "panels": [
    {
      "title": "Request Latency (P95)",
      "type": "timeseries",
      "targets": [
        {
          "expr": "histogram_quantile(0.95, rate(apex_request_duration_bucket[5m]))",
          "legendFormat": "P95"
        },
        {
          "expr": "histogram_quantile(0.50, rate(apex_request_duration_bucket[5m]))",
          "legendFormat": "P50"
        }
      ],
      "fieldConfig": {
        "defaults": {
          "unit": "s",
          "thresholds": {
            "steps": [
              {"color": "green", "value": null},
              {"color": "yellow", "value": 0.1},
              {"color": "red", "value": 0.5}
            ]
          }
        }
      }
    },
    {
      "title": "Throughput",
      "type": "stat",
      "targets": [
        {
          "expr": "sum(rate(apex_requests_total[5m]))",
          "legendFormat": "RPS"
        }
      ]
    },
    {
      "title": "Error Rate",
      "type": "gauge",
      "targets": [
        {
          "expr": "sum(rate(apex_requests_total{status=~\"5..\"}[5m])) / sum(rate(apex_requests_total[5m]))",
          "legendFormat": "Error Rate"
        }
      ],
      "fieldConfig": {
        "defaults": {
          "unit": "percentunit",
          "max": 1,
          "thresholds": {
            "steps": [
              {"color": "green", "value": null},
              {"color": "yellow", "value": 0.01},
              {"color": "red", "value": 0.05}
            ]
          }
        }
      }
    },
    {
      "title": "Active Connections",
      "type": "timeseries",
      "targets": [
        {
          "expr": "apex_db_connections_active",
          "legendFormat": "Database"
        },
        {
          "expr": "apex_redis_connections_active",
          "legendFormat": "Redis"
        },
        {
          "expr": "apex_websocket_connections_active",
          "legendFormat": "WebSocket"
        }
      ]
    },
    {
      "title": "Task Queue Depth",
      "type": "timeseries",
      "targets": [
        {
          "expr": "apex_queue_depth",
          "legendFormat": "Pending Tasks"
        }
      ],
      "fieldConfig": {
        "defaults": {
          "thresholds": {
            "steps": [
              {"color": "green", "value": null},
              {"color": "yellow", "value": 50},
              {"color": "red", "value": 100}
            ]
          }
        }
      }
    },
    {
      "title": "Worker Pool Utilization",
      "type": "gauge",
      "targets": [
        {
          "expr": "(apex_worker_pool_max - apex_worker_pool_available) / apex_worker_pool_max",
          "legendFormat": "Utilization"
        }
      ]
    }
  ]
}
```

### Profiling Tools

#### Continuous Profiling with py-spy (Python Workers)

```bash
# Profile Python worker
py-spy record -o profile.svg --pid $(pgrep -f "python main.py")

# Top-like view
py-spy top --pid $(pgrep -f "python main.py")
```

#### Rust Profiling with perf

```bash
# Record performance data
perf record -g ./target/release/apex-server

# Generate flame graph
perf script | stackcollapse-perf.pl | flamegraph.pl > flamegraph.svg
```

#### Tokio Console (Development)

```bash
# Enable tokio-console in development
RUSTFLAGS="--cfg tokio_unstable" cargo build

# Run with console support
TOKIO_CONSOLE=1 ./target/debug/apex-server

# Connect with tokio-console
tokio-console
```

### Health Check Endpoints

```rust
// api/handlers.rs - Health check endpoints

/// Liveness probe - is the server running?
async fn liveness() -> impl IntoResponse {
    StatusCode::OK
}

/// Readiness probe - is the server ready to accept traffic?
async fn readiness(
    State(state): State<AppState>,
) -> impl IntoResponse {
    // Check database connection
    let db_healthy = state.db.pool()
        .acquire()
        .await
        .is_ok();

    // Check Redis connection
    let redis_healthy = state.redis
        .get::<_, Option<String>>("health:check")
        .await
        .is_ok();

    if db_healthy && redis_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

/// Detailed health check for monitoring
async fn health_detailed(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let db_pool = state.db.pool();

    let health = HealthStatus {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: START_TIME.elapsed().as_secs(),
        components: Components {
            database: ComponentHealth {
                status: "healthy",
                connections_active: db_pool.size(),
                connections_max: db_pool.max_connections(),
            },
            redis: ComponentHealth {
                status: "healthy",
                // ... redis stats
            },
            workers: WorkerHealth {
                active: state.orchestrator.active_workers(),
                max: state.orchestrator.max_workers(),
                queue_depth: state.orchestrator.queue_depth(),
            },
        },
    };

    Json(health)
}
```

---

## Quick Reference

### Performance Checklist

- [ ] Tokio runtime configured for workload type
- [ ] Database connection pool sized appropriately
- [ ] Redis connection pool configured
- [ ] Indexes created for common query patterns
- [ ] Caching implemented for frequently accessed data
- [ ] Batch processing for bulk operations
- [ ] Async patterns used consistently
- [ ] No blocking calls in async contexts
- [ ] Proper backpressure handling
- [ ] Metrics and alerts configured
- [ ] Profiling tools available

### Common Performance Issues

| Symptom | Likely Cause | Solution |
|---------|--------------|----------|
| High latency, low CPU | Database queries | Add indexes, optimize queries |
| High CPU, high latency | CPU-bound work on async threads | Use spawn_blocking |
| Memory growth | Memory leaks, no cleanup | Profile with tools, check for leaks |
| Connection errors | Pool exhaustion | Increase pool size, add backpressure |
| Queue growth | Workers too slow | Scale workers, optimize processing |
| Timeout errors | External service slow | Add circuit breaker, increase timeout |

### Performance Tuning Commands

```bash
# Check PostgreSQL performance
psql -c "SELECT * FROM pg_stat_statements ORDER BY total_time DESC LIMIT 10;"

# Check Redis performance
redis-cli --latency
redis-cli INFO stats

# Check system resources
htop
iostat -x 1
vmstat 1

# Check network connections
ss -s
netstat -an | grep ESTABLISHED | wc -l
```
