# Project Apex - Architecture Overview

> Detailed technical architecture of the world's No. 1 Agent Swarm Orchestration System

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              PRESENTATION LAYER                                  │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │                      Panopticon Dashboard (React)                          │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │  │
│  │  │ Agent Grid  │  │ Task Board  │  │  Metrics    │  │  Approvals  │       │  │
│  │  │  (D3.js)    │  │             │  │  (Plotly)   │  │   Queue     │       │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘       │  │
│  │                         │                                                   │  │
│  │                    Zustand Store + TanStack Query                          │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                    ┌───────────────────┼───────────────────┐
                    │ WebSocket         │ REST              │ gRPC
                    ▼                   ▼                   ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                API LAYER (Rust/Axum)                            │
│  ┌───────────────────┐  ┌───────────────────┐  ┌───────────────────┐           │
│  │   REST Handlers   │  │ WebSocket Server  │  │   gRPC Service    │           │
│  │   (Axum Routes)   │  │   (Real-time)     │  │    (Tonic)        │           │
│  └───────────────────┘  └───────────────────┘  └───────────────────┘           │
│                                     │                                           │
│  ┌──────────────────────────────────┴──────────────────────────────────────┐   │
│  │                        Middleware Layer                                  │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │   │
│  │  │   Auth   │  │   CORS   │  │  Tracing │  │  Metrics │  │   Rate   │  │   │
│  │  │          │  │          │  │  (OTEL)  │  │  (Prom)  │  │  Limit   │  │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │   │
│  └──────────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           ORCHESTRATION LAYER (Rust)                            │
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────┐     │
│  │                        Swarm Orchestrator                               │     │
│  │  ┌───────────────────┬───────────────────┬───────────────────┐         │     │
│  │  │   DAG Executor    │   Task Scheduler  │   Event Bus       │         │     │
│  │  │  (petgraph)       │  (Priority Queue) │   (broadcast)     │         │     │
│  │  └───────────────────┴───────────────────┴───────────────────┘         │     │
│  └────────────────────────────────────────────────────────────────────────┘     │
│                                     │                                            │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐               │
│  │  Contract Layer  │  │   Model Router   │  │  Worker Pool     │               │
│  │  ┌────────────┐  │  │  ┌────────────┐  │  │  ┌────────────┐  │               │
│  │  │ Enforcer   │  │  │  │ FrugalGPT  │  │  │  │ Semaphore  │  │               │
│  │  │ Tracker    │  │  │  │ Cascade    │  │  │  │ Pool       │  │               │
│  │  │ Limits     │  │  │  │ Router     │  │  │  │ Manager    │  │               │
│  │  └────────────┘  │  │  └────────────┘  │  │  └────────────┘  │               │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘               │
│                                     │                                            │
│  ┌──────────────────────────────────┴──────────────────────────────────────┐    │
│  │                        Circuit Breaker                                   │    │
│  │         CLOSED ──────▶ OPEN ──────▶ HALF_OPEN ──────▶ CLOSED            │    │
│  └──────────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        │ Redis Queue (BRPOP)
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            AGENT LAYER (Python/asyncio)                         │
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────┐     │
│  │                          Worker Pool                                    │     │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │     │
│  │  │  Worker 1   │  │  Worker 2   │  │  Worker 3   │  │  Worker N   │    │     │
│  │  │  ┌───────┐  │  │  ┌───────┐  │  │  ┌───────┐  │  │  ┌───────┐  │    │     │
│  │  │  │Agent 1│  │  │  │Agent 4│  │  │  │Agent 7│  │  │  │Agent M│  │    │     │
│  │  │  │Agent 2│  │  │  │Agent 5│  │  │  │Agent 8│  │  │  │  ...  │  │    │     │
│  │  │  │Agent 3│  │  │  │Agent 6│  │  │  │Agent 9│  │  │  │       │  │    │     │
│  │  │  └───────┘  │  │  └───────┘  │  │  └───────┘  │  │  └───────┘  │    │     │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │     │
│  └────────────────────────────────────────────────────────────────────────┘     │
│                                     │                                            │
│  ┌──────────────────────────────────┴──────────────────────────────────────┐    │
│  │                           LLM Client                                     │    │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐         │    │
│  │  │   OpenAI   │  │  Anthropic │  │   Google   │  │   Local    │         │    │
│  │  │  gpt-4o    │  │   Claude   │  │   Gemini   │  │   Ollama   │         │    │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘         │    │
│  └──────────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              DATA LAYER                                          │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐               │
│  │   PostgreSQL     │  │      Redis       │  │   Object Store   │               │
│  │  ┌────────────┐  │  │  ┌────────────┐  │  │  ┌────────────┐  │               │
│  │  │   Tasks    │  │  │  │ Task Queue │  │  │  │ Artifacts  │  │               │
│  │  │   Agents   │  │  │  │   Cache    │  │  │  │   Logs     │  │               │
│  │  │  Contracts │  │  │  │   PubSub   │  │  │  │  Reports   │  │               │
│  │  │   Events   │  │  │  │ Rate Limit │  │  │  │            │  │               │
│  │  └────────────┘  │  │  └────────────┘  │  │  └────────────┘  │               │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘               │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           OBSERVABILITY LAYER                                    │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐               │
│  │     Jaeger       │  │   Prometheus     │  │      Loki        │               │
│  │  (Dist Tracing)  │  │    (Metrics)     │  │     (Logs)       │               │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘               │
│                                     │                                            │
│                           ┌─────────┴─────────┐                                  │
│                           │      Grafana      │                                  │
│                           │   (Dashboards)    │                                  │
│                           └───────────────────┘                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Component Details

### 1. DAG Executor

The DAG (Directed Acyclic Graph) Executor manages task dependencies and parallel execution.

```rust
pub struct TaskDAG {
    graph: DiGraph<Task, ()>,         // petgraph for topology
    node_map: HashMap<TaskId, NodeIndex>,
    status_map: HashMap<TaskId, TaskStatus>,
}

impl TaskDAG {
    // Get tasks ready for execution (no pending dependencies)
    pub fn get_ready_tasks(&self) -> Vec<TaskId>;

    // Topological sort for execution order
    pub fn topological_order(&self) -> Result<Vec<TaskId>>;

    // Cycle detection (reject invalid DAGs)
    pub fn add_dependency(&mut self, from: TaskId, to: TaskId) -> Result<()>;

    // Cascading cancellation on failure
    pub fn cancel_dependents(&mut self, failed_task: TaskId) -> Vec<TaskId>;
}
```

### 2. Agent Contract Framework

Contracts enforce resource limits with a conservation law:

```
parent_budget >= sum(child_budgets) + overhead
```

```rust
pub struct AgentContract {
    pub limits: ResourceLimits,
    pub usage: ResourceUsage,
    pub status: ContractStatus,
}

pub struct ResourceLimits {
    pub token_limit: u64,
    pub cost_limit: f64,
    pub api_call_limit: u32,
    pub time_limit_seconds: u64,
}
```

### 3. FrugalGPT Model Router

Adaptive model selection cascading from cheap to expensive:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Economy   │ ──▶ │  Standard   │ ──▶ │   Premium   │
│  gpt-4o-mini│     │    gpt-4o   │     │ claude-opus │
│   $0.00015  │     │    $0.005   │     │   $0.015    │
└─────────────┘     └─────────────┘     └─────────────┘
      │                   │                   │
      ▼                   ▼                   ▼
   Confidence         Confidence         Final Answer
   < 0.85?            < 0.90?
```

### 4. Circuit Breaker Pattern

Protection against cascading failures:

```
                    failure_threshold
                    exceeded (5 failures)
        ┌──────────────────────────────────────┐
        │                                      │
        ▼                                      │
   ┌─────────┐                            ┌────┴────┐
   │ CLOSED  │──────────────────────────▶ │  OPEN   │
   └─────────┘      failures >= 5         └────┬────┘
        ▲                                      │
        │         recovery_timeout             │
        │         (30 seconds)                 │
        │                                      ▼
        │                               ┌─────────────┐
        └───────────────────────────────│  HALF_OPEN  │
                  success               └─────────────┘
```

### 5. Worker Pool Architecture

Python workers communicate via Redis queues:

```
Rust Orchestrator                 Python Workers
      │                                │
      │  LPUSH apex:tasks:queue        │
      │ ─────────────────────────────▶ │
      │                                │ BRPOP (blocking)
      │                                │
      │                                │ Execute Task
      │                                │
      │  LPUSH apex:results:queue      │
      │ ◀───────────────────────────── │
      │                                │
```

## Data Flow

### Task Submission Flow

```
1. Client ──POST /tasks──▶ API Server
                              │
2.                   Validate & Create Task
                              │
3.                   Insert into PostgreSQL
                              │
4.                   Push to Redis Queue
                              │
5.                   Return Task ID
                              │
6.            ◀── WebSocket: task_created ──
```

### Task Execution Flow

```
1. Worker ──BRPOP──▶ Redis Queue
                        │
2.              Receive Task
                        │
3.         Check Contract Limits
                        │
4.         Route to LLM (FrugalGPT)
                        │
5.         Execute Tool Calls
                        │
6.         Update Contract Usage
                        │
7.         Push Result to Redis
                        │
8. Orchestrator ◀── Result ──
                        │
9.         Update PostgreSQL
                        │
10.        Broadcast via WebSocket
```

## Deployment Architecture

### Kubernetes Topology

```
┌─────────────────────────────────────────────────────────────────┐
│                        Kubernetes Cluster                        │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                     Ingress Controller                      │ │
│  │                    (nginx / traefik)                        │ │
│  └────────────────────────────────────────────────────────────┘ │
│                              │                                   │
│         ┌────────────────────┼────────────────────┐              │
│         ▼                    ▼                    ▼              │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐      │
│  │ Dashboard   │      │  API Server │      │   Worker    │      │
│  │ Deployment  │      │ Deployment  │      │ Deployment  │      │
│  │  (2 pods)   │      │  (2-10 pods)│      │ (3-20 pods) │      │
│  │             │      │     HPA     │      │     HPA     │      │
│  └─────────────┘      └─────────────┘      └─────────────┘      │
│                              │                    │              │
│                              │                    │              │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    StatefulSets                             │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │ │
│  │  │ PostgreSQL  │  │    Redis    │  │   Jaeger    │         │ │
│  │  │  (1 pod)    │  │  (1 pod)    │  │  (1 pod)    │         │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘         │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    Monitoring Stack                         │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │ │
│  │  │ Prometheus  │  │   Grafana   │  │    Loki     │         │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘         │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Security Model

### Authentication Flow

```
┌────────┐     ┌───────────┐     ┌────────────┐
│ Client │────▶│  API Key  │────▶│  Validate  │
└────────┘     │  Header   │     │   (Redis)  │
               └───────────┘     └────────────┘
                                       │
                                       ▼
                               ┌────────────┐
                               │   Rate     │
                               │  Limiter   │
                               └────────────┘
                                       │
                                       ▼
                               ┌────────────┐
                               │  Handler   │
                               └────────────┘
```

### Contract Security

```
┌─────────────────────────────────────────────────────────────┐
│                    Contract Hierarchy                        │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Root Contract (DAG)                     │    │
│  │         tokens: 100,000  cost: $10.00               │    │
│  │  ┌─────────────────┬─────────────────┐              │    │
│  │  │  Child Contract │  Child Contract │              │    │
│  │  │  tokens: 30,000 │  tokens: 50,000 │              │    │
│  │  │  cost: $3.00    │  cost: $5.00    │              │    │
│  │  └─────────────────┴─────────────────┘              │    │
│  │                                                      │    │
│  │  Overhead: 20,000 tokens, $2.00                     │    │
│  │  Conservation: 30k + 50k + 20k = 100k ✓            │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Performance Characteristics

| Metric | Target | Architecture Support |
|--------|--------|---------------------|
| Concurrent Agents | 1000+ | Tokio async runtime, Semaphore pools |
| Task Throughput | 100/sec | Redis queues, parallel workers |
| API Latency P95 | <100ms | Axum async handlers |
| WebSocket Latency | <50ms | Broadcast channels |
| DAG Execution | <15s (3 tasks) | Topological sort, parallel execution |
| Recovery Time | <30s | Circuit breaker, health checks |
