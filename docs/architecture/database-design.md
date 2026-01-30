# Project Apex: Database and State Management Design

## Overview

This document defines the complete database architecture for Project Apex, an AI agent orchestration system. The design emphasizes:

- **Event Sourcing** for complete audit trails and state reconstruction
- **CRDT-based consistency** for parallel agent operations
- **Optimistic concurrency** for high-throughput task processing
- **Vector storage** for semantic agent context retrieval

---

## Table of Contents

1. [PostgreSQL Schema](#1-postgresql-schema)
2. [Indexes](#2-indexes)
3. [Event Types](#3-event-types)
4. [State Consistency Model](#4-state-consistency-model)
5. [Vector Database](#5-vector-database)
6. [Cache Layer](#6-cache-layer)
7. [Migration Strategy](#7-migration-strategy)
8. [Backup Strategy](#8-backup-strategy)

---

## 1. PostgreSQL Schema

### 1.1 Database Initialization

```sql
-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";  -- For text search
CREATE EXTENSION IF NOT EXISTS "vector";   -- pgvector for embeddings

-- Create custom types (ENUMs)
CREATE TYPE task_status AS ENUM (
    'pending',
    'ready',
    'running',
    'completed',
    'failed',
    'cancelled'
);

CREATE TYPE agent_status AS ENUM (
    'idle',
    'busy',
    'error',
    'paused'
);

CREATE TYPE contract_status AS ENUM (
    'active',
    'completed',
    'exceeded',
    'cancelled'
);

CREATE TYPE approval_status AS ENUM (
    'pending',
    'approved',
    'denied',
    'expired'
);
```

### 1.2 Tasks Table

The central entity representing work units in the DAG-based orchestration system.

```sql
CREATE TABLE tasks (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    parent_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    dag_id UUID NOT NULL,
    agent_id UUID,  -- FK added after agents table creation

    -- Status and priority
    status task_status NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,

    -- Input/Output data
    input JSONB NOT NULL,
    output JSONB,
    error TEXT,

    -- Resource tracking
    tokens_used INTEGER NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Retry configuration
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,

    -- Optimistic concurrency control
    version INTEGER NOT NULL DEFAULT 1,

    -- Constraints
    CONSTRAINT valid_retry_count CHECK (retry_count >= 0 AND retry_count <= max_retries),
    CONSTRAINT valid_tokens CHECK (tokens_used >= 0),
    CONSTRAINT valid_cost CHECK (cost_dollars >= 0),
    CONSTRAINT valid_priority CHECK (priority >= -100 AND priority <= 100)
);

-- Add comments for documentation
COMMENT ON TABLE tasks IS 'Core task entity representing work units in the DAG orchestration';
COMMENT ON COLUMN tasks.dag_id IS 'Identifier grouping tasks belonging to the same directed acyclic graph';
COMMENT ON COLUMN tasks.parent_id IS 'Self-referential FK for task hierarchy (subtask spawning)';
COMMENT ON COLUMN tasks.version IS 'Optimistic concurrency control version number';
```

### 1.3 Agents Table

Represents AI agents with their capabilities, load state, and performance metrics.

```sql
CREATE TABLE agents (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,

    -- Model configuration
    model VARCHAR(100) NOT NULL,
    system_prompt TEXT,
    tools JSONB NOT NULL DEFAULT '[]'::JSONB,

    -- Operational state
    status agent_status NOT NULL DEFAULT 'idle',
    current_load INTEGER NOT NULL DEFAULT 0,
    max_load INTEGER NOT NULL DEFAULT 10,

    -- Performance metrics
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    total_cost DECIMAL(12, 6) NOT NULL DEFAULT 0,

    -- Reputation system (0.0 to 1.0, higher is better)
    reputation_score DECIMAL(5, 4) NOT NULL DEFAULT 1.0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ,

    -- Optimistic concurrency control
    version INTEGER NOT NULL DEFAULT 1,

    -- Constraints
    CONSTRAINT valid_load CHECK (current_load >= 0 AND current_load <= max_load),
    CONSTRAINT valid_max_load CHECK (max_load > 0 AND max_load <= 1000),
    CONSTRAINT valid_reputation CHECK (reputation_score >= 0 AND reputation_score <= 1),
    CONSTRAINT valid_counts CHECK (success_count >= 0 AND failure_count >= 0),
    CONSTRAINT valid_tools_array CHECK (jsonb_typeof(tools) = 'array')
);

-- Add FK constraint to tasks table
ALTER TABLE tasks
ADD CONSTRAINT fk_tasks_agent
FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE SET NULL;

COMMENT ON TABLE agents IS 'AI agent entities with capabilities and performance tracking';
COMMENT ON COLUMN agents.reputation_score IS 'Dynamic score based on success/failure ratio and task complexity';
COMMENT ON COLUMN agents.tools IS 'JSON array of tool definitions available to this agent';
```

### 1.4 Agent Contracts Table

Implements resource budgeting and limits for agent operations.

```sql
CREATE TABLE agent_contracts (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    parent_contract_id UUID REFERENCES agent_contracts(id) ON DELETE SET NULL,

    -- Token limits
    token_limit INTEGER NOT NULL,
    token_used INTEGER NOT NULL DEFAULT 0,

    -- Cost limits
    cost_limit DECIMAL(10, 6) NOT NULL,
    cost_used DECIMAL(10, 6) NOT NULL DEFAULT 0,

    -- Time limits
    time_limit_seconds INTEGER NOT NULL,

    -- API call limits
    api_call_limit INTEGER NOT NULL,
    api_calls_used INTEGER NOT NULL DEFAULT 0,

    -- Status tracking
    status contract_status NOT NULL DEFAULT 'active',

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,

    -- Optimistic concurrency control
    version INTEGER NOT NULL DEFAULT 1,

    -- Constraints
    CONSTRAINT valid_token_usage CHECK (token_used >= 0 AND token_used <= token_limit * 1.1),
    CONSTRAINT valid_cost_usage CHECK (cost_used >= 0),
    CONSTRAINT valid_api_usage CHECK (api_calls_used >= 0),
    CONSTRAINT valid_limits CHECK (
        token_limit > 0 AND
        cost_limit > 0 AND
        time_limit_seconds > 0 AND
        api_call_limit > 0
    )
);

COMMENT ON TABLE agent_contracts IS 'Resource budget contracts governing agent operations';
COMMENT ON COLUMN agent_contracts.parent_contract_id IS 'Links to parent contract for hierarchical budget inheritance';
COMMENT ON COLUMN agent_contracts.token_limit IS 'Maximum tokens this contract allows (soft limit with 10% grace)';
```

### 1.5 Tool Calls Table

Records all tool invocations for auditing and debugging.

```sql
CREATE TABLE tool_calls (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    -- Distributed tracing
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    parent_span_id VARCHAR(16),

    -- Tool execution details
    tool_name VARCHAR(255) NOT NULL,
    parameters JSONB,
    result JSONB,
    error TEXT,

    -- Resource usage
    tokens_used INTEGER NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,
    latency_ms INTEGER,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT valid_latency CHECK (latency_ms IS NULL OR latency_ms >= 0),
    CONSTRAINT valid_tool_tokens CHECK (tokens_used >= 0),
    CONSTRAINT valid_tool_cost CHECK (cost_dollars >= 0)
);

COMMENT ON TABLE tool_calls IS 'Audit log of all tool invocations by agents';
COMMENT ON COLUMN tool_calls.trace_id IS 'OpenTelemetry trace ID for distributed tracing';
COMMENT ON COLUMN tool_calls.span_id IS 'OpenTelemetry span ID for this specific call';
```

### 1.6 Events Table (Event Sourcing)

Immutable event log serving as the source of truth for state reconstruction.

```sql
CREATE TABLE events (
    -- Primary identification (BIGSERIAL for high-throughput append)
    id BIGSERIAL PRIMARY KEY,
    event_id UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4(),

    -- Distributed tracing
    trace_id VARCHAR(32),
    span_id VARCHAR(16),

    -- Aggregate identification
    aggregate_type VARCHAR(50) NOT NULL,
    aggregate_id UUID NOT NULL,

    -- Event details
    event_type VARCHAR(100) NOT NULL,
    event_data JSONB NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- Ordering and consistency
    version INTEGER NOT NULL,
    sequence_number BIGINT,  -- Global ordering

    -- Timestamp (immutable)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_version CHECK (version > 0),
    CONSTRAINT unique_aggregate_version UNIQUE (aggregate_type, aggregate_id, version)
);

-- Sequence for global ordering
CREATE SEQUENCE IF NOT EXISTS event_sequence_seq;

-- Trigger to auto-assign sequence number
CREATE OR REPLACE FUNCTION assign_event_sequence()
RETURNS TRIGGER AS $$
BEGIN
    NEW.sequence_number := nextval('event_sequence_seq');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER event_sequence_trigger
    BEFORE INSERT ON events
    FOR EACH ROW
    EXECUTE FUNCTION assign_event_sequence();

COMMENT ON TABLE events IS 'Immutable event log for event sourcing - source of truth';
COMMENT ON COLUMN events.version IS 'Aggregate version for optimistic concurrency control';
COMMENT ON COLUMN events.sequence_number IS 'Global sequence for total ordering of events';
```

### 1.7 Task Dependencies Table

Junction table for DAG edges representing task dependencies.

```sql
CREATE TABLE task_dependencies (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

    -- Dependency metadata
    dependency_type VARCHAR(50) NOT NULL DEFAULT 'strict',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Primary key prevents duplicate edges
    PRIMARY KEY (task_id, depends_on_id),

    -- Prevent self-dependency
    CONSTRAINT no_self_dependency CHECK (task_id != depends_on_id)
);

-- Function to detect cycles (called by trigger)
CREATE OR REPLACE FUNCTION check_dependency_cycle()
RETURNS TRIGGER AS $$
DECLARE
    cycle_exists BOOLEAN;
BEGIN
    -- Use recursive CTE to detect cycles
    WITH RECURSIVE dep_chain AS (
        -- Start from the new dependency target
        SELECT depends_on_id AS task_id, 1 AS depth
        FROM task_dependencies
        WHERE task_id = NEW.depends_on_id

        UNION ALL

        -- Follow the chain
        SELECT td.depends_on_id, dc.depth + 1
        FROM task_dependencies td
        JOIN dep_chain dc ON td.task_id = dc.task_id
        WHERE dc.depth < 100  -- Prevent infinite loops
    )
    SELECT EXISTS (
        SELECT 1 FROM dep_chain WHERE task_id = NEW.task_id
    ) INTO cycle_exists;

    IF cycle_exists THEN
        RAISE EXCEPTION 'Dependency cycle detected: task % cannot depend on task %',
            NEW.task_id, NEW.depends_on_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER prevent_dependency_cycle
    BEFORE INSERT ON task_dependencies
    FOR EACH ROW
    EXECUTE FUNCTION check_dependency_cycle();

COMMENT ON TABLE task_dependencies IS 'DAG edges representing task execution dependencies';
COMMENT ON COLUMN task_dependencies.dependency_type IS 'Type: strict (must complete), soft (optional), data (output required)';
```

### 1.8 Approval Requests Table

Human-in-the-loop approval workflow for high-risk agent actions.

```sql
CREATE TABLE approval_requests (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    -- Action details
    action_type VARCHAR(100) NOT NULL,
    action_data JSONB NOT NULL,

    -- Risk assessment
    risk_score DECIMAL(3, 2),
    risk_factors JSONB DEFAULT '[]'::JSONB,

    -- Clustering for batch approvals
    cluster_id UUID,

    -- Status tracking
    status approval_status NOT NULL DEFAULT 'pending',

    -- Decision details
    decided_by VARCHAR(255),
    decision_reason TEXT,
    decided_at TIMESTAMPTZ,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT valid_risk_score CHECK (risk_score IS NULL OR (risk_score >= 0 AND risk_score <= 1)),
    CONSTRAINT decision_requires_status CHECK (
        (status IN ('approved', 'denied') AND decided_at IS NOT NULL) OR
        (status IN ('pending', 'expired'))
    )
);

COMMENT ON TABLE approval_requests IS 'Human-in-the-loop approval queue for high-risk actions';
COMMENT ON COLUMN approval_requests.cluster_id IS 'Groups similar requests for batch approval';
COMMENT ON COLUMN approval_requests.risk_score IS 'ML-computed risk score (0.0 = safe, 1.0 = dangerous)';
```

### 1.9 Event Snapshots Table

Performance optimization for event sourcing via periodic state snapshots.

```sql
CREATE TABLE event_snapshots (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Aggregate identification
    aggregate_type VARCHAR(50) NOT NULL,
    aggregate_id UUID NOT NULL,

    -- Snapshot data
    state_data JSONB NOT NULL,
    version INTEGER NOT NULL,
    event_sequence BIGINT NOT NULL,

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT unique_snapshot_version UNIQUE (aggregate_type, aggregate_id, version)
);

CREATE INDEX idx_snapshots_lookup
ON event_snapshots(aggregate_type, aggregate_id, version DESC);

COMMENT ON TABLE event_snapshots IS 'Periodic state snapshots for faster event replay';
COMMENT ON COLUMN event_snapshots.event_sequence IS 'Last event sequence number included in snapshot';
```

### 1.10 DAGs Table

Top-level DAG metadata and configuration.

```sql
CREATE TABLE dags (
    -- Primary identification
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Metadata
    name VARCHAR(255) NOT NULL,
    description TEXT,

    -- Configuration
    config JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending',

    -- Resource tracking (aggregated)
    total_tasks INTEGER NOT NULL DEFAULT 0,
    completed_tasks INTEGER NOT NULL DEFAULT 0,
    failed_tasks INTEGER NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    total_cost DECIMAL(12, 6) NOT NULL DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Version control
    version INTEGER NOT NULL DEFAULT 1
);

-- Add FK to tasks
ALTER TABLE tasks
ADD CONSTRAINT fk_tasks_dag
FOREIGN KEY (dag_id) REFERENCES dags(id) ON DELETE CASCADE;

COMMENT ON TABLE dags IS 'Top-level DAG definitions grouping related tasks';
```

---

## 2. Indexes

### 2.1 Tasks Table Indexes

```sql
-- Primary query patterns: Find tasks by status and DAG
CREATE INDEX idx_tasks_dag_status
ON tasks(dag_id, status);

-- Find ready tasks (for scheduler)
CREATE INDEX idx_tasks_ready_priority
ON tasks(priority DESC, created_at ASC)
WHERE status = 'ready';

-- Find running tasks (for monitoring)
CREATE INDEX idx_tasks_running
ON tasks(agent_id, started_at)
WHERE status = 'running';

-- Find failed tasks (for retry logic)
CREATE INDEX idx_tasks_failed_retryable
ON tasks(dag_id, created_at)
WHERE status = 'failed' AND retry_count < max_retries;

-- Parent-child relationships
CREATE INDEX idx_tasks_parent
ON tasks(parent_id)
WHERE parent_id IS NOT NULL;

-- JSONB index for input queries
CREATE INDEX idx_tasks_input_gin
ON tasks USING GIN(input jsonb_path_ops);

-- JSONB index for output queries
CREATE INDEX idx_tasks_output_gin
ON tasks USING GIN(output jsonb_path_ops)
WHERE output IS NOT NULL;

-- Time-based queries
CREATE INDEX idx_tasks_created_at
ON tasks(created_at DESC);

CREATE INDEX idx_tasks_completed_at
ON tasks(completed_at DESC)
WHERE completed_at IS NOT NULL;
```

### 2.2 Agents Table Indexes

```sql
-- Find available agents
CREATE INDEX idx_agents_available
ON agents(model, current_load, reputation_score DESC)
WHERE status = 'idle';

-- Agent lookup by name (fuzzy search support)
CREATE INDEX idx_agents_name_trgm
ON agents USING GIN(name gin_trgm_ops);

-- Performance tracking queries
CREATE INDEX idx_agents_performance
ON agents(reputation_score DESC, success_count DESC);

-- JSONB index for tools queries
CREATE INDEX idx_agents_tools_gin
ON agents USING GIN(tools jsonb_path_ops);

-- Last active tracking
CREATE INDEX idx_agents_last_active
ON agents(last_active_at DESC)
WHERE status != 'paused';
```

### 2.3 Agent Contracts Table Indexes

```sql
-- Find active contracts for an agent
CREATE INDEX idx_contracts_agent_active
ON agent_contracts(agent_id, created_at DESC)
WHERE status = 'active';

-- Find contracts by task
CREATE INDEX idx_contracts_task
ON agent_contracts(task_id);

-- Expiring contracts (for cleanup)
CREATE INDEX idx_contracts_expiring
ON agent_contracts(expires_at)
WHERE status = 'active' AND expires_at IS NOT NULL;

-- Contract hierarchy
CREATE INDEX idx_contracts_parent
ON agent_contracts(parent_contract_id)
WHERE parent_contract_id IS NOT NULL;
```

### 2.4 Tool Calls Table Indexes

```sql
-- Query by task
CREATE INDEX idx_tool_calls_task
ON tool_calls(task_id, created_at DESC);

-- Query by agent
CREATE INDEX idx_tool_calls_agent
ON tool_calls(agent_id, created_at DESC);

-- Distributed tracing lookup
CREATE INDEX idx_tool_calls_trace
ON tool_calls(trace_id, span_id)
WHERE trace_id IS NOT NULL;

-- Tool performance analysis
CREATE INDEX idx_tool_calls_tool_name
ON tool_calls(tool_name, created_at DESC);

-- Error analysis
CREATE INDEX idx_tool_calls_errors
ON tool_calls(tool_name, created_at DESC)
WHERE error IS NOT NULL;

-- JSONB indexes for parameter/result queries
CREATE INDEX idx_tool_calls_params_gin
ON tool_calls USING GIN(parameters jsonb_path_ops)
WHERE parameters IS NOT NULL;
```

### 2.5 Events Table Indexes

```sql
-- Primary event replay query
CREATE INDEX idx_events_aggregate
ON events(aggregate_type, aggregate_id, version ASC);

-- Global sequence ordering
CREATE INDEX idx_events_sequence
ON events(sequence_number ASC);

-- Event type filtering
CREATE INDEX idx_events_type
ON events(event_type, created_at DESC);

-- Distributed tracing
CREATE INDEX idx_events_trace
ON events(trace_id)
WHERE trace_id IS NOT NULL;

-- Time-range queries
CREATE INDEX idx_events_created
ON events(created_at DESC);

-- BRIN index for time-series optimization (large tables)
CREATE INDEX idx_events_created_brin
ON events USING BRIN(created_at);

-- JSONB index for event data queries
CREATE INDEX idx_events_data_gin
ON events USING GIN(event_data jsonb_path_ops);
```

### 2.6 Task Dependencies Table Indexes

```sql
-- Find all dependencies for a task
CREATE INDEX idx_deps_task
ON task_dependencies(task_id);

-- Find all dependents (tasks waiting on this one)
CREATE INDEX idx_deps_depends_on
ON task_dependencies(depends_on_id);
```

### 2.7 Approval Requests Table Indexes

```sql
-- Pending approvals queue
CREATE INDEX idx_approvals_pending
ON approval_requests(created_at ASC, risk_score DESC)
WHERE status = 'pending';

-- Clustered approvals
CREATE INDEX idx_approvals_cluster
ON approval_requests(cluster_id, status)
WHERE cluster_id IS NOT NULL;

-- Agent approval history
CREATE INDEX idx_approvals_agent
ON approval_requests(agent_id, created_at DESC);

-- Expiring approvals (for auto-deny)
CREATE INDEX idx_approvals_expiring
ON approval_requests(expires_at)
WHERE status = 'pending' AND expires_at IS NOT NULL;
```

---

## 3. Event Types

### 3.1 Task Events

| Event Type | Description | Event Data Schema |
|------------|-------------|-------------------|
| `TASK_CREATED` | New task added to DAG | `{task_id, dag_id, parent_id?, input, priority}` |
| `TASK_QUEUED` | Task moved to ready queue | `{task_id, queue_position}` |
| `TASK_ASSIGNED` | Task assigned to agent | `{task_id, agent_id, contract_id}` |
| `TASK_STARTED` | Agent began execution | `{task_id, agent_id, started_at}` |
| `TASK_PROGRESS` | Intermediate progress update | `{task_id, progress_pct, message?}` |
| `TASK_COMPLETED` | Successful completion | `{task_id, output, tokens_used, cost, duration_ms}` |
| `TASK_FAILED` | Execution failed | `{task_id, error, retry_count, will_retry}` |
| `TASK_RETRYING` | Retry scheduled | `{task_id, retry_count, backoff_ms}` |
| `TASK_CANCELLED` | Manually or auto cancelled | `{task_id, reason, cancelled_by?}` |
| `TASK_TIMEOUT` | Exceeded time limit | `{task_id, elapsed_ms, limit_ms}` |

### 3.2 Agent Events

| Event Type | Description | Event Data Schema |
|------------|-------------|-------------------|
| `AGENT_SPAWNED` | New agent created | `{agent_id, name, model, tools}` |
| `AGENT_CONFIGURED` | Agent config updated | `{agent_id, changes}` |
| `AGENT_IDLE` | Agent finished work | `{agent_id, tasks_completed}` |
| `AGENT_BUSY` | Agent at capacity | `{agent_id, current_load, max_load}` |
| `AGENT_ERROR` | Agent entered error state | `{agent_id, error, last_task_id?}` |
| `AGENT_PAUSED` | Agent manually paused | `{agent_id, reason, paused_by?}` |
| `AGENT_RESUMED` | Agent unpaused | `{agent_id, resumed_by?}` |
| `AGENT_REPUTATION_UPDATED` | Score recalculated | `{agent_id, old_score, new_score, reason}` |

### 3.3 Contract Events

| Event Type | Description | Event Data Schema |
|------------|-------------|-------------------|
| `CONTRACT_CREATED` | New resource contract | `{contract_id, agent_id, task_id, limits}` |
| `CONTRACT_UPDATED` | Usage updated | `{contract_id, tokens_used, cost_used, api_calls_used}` |
| `CONTRACT_WARNING` | Approaching limits | `{contract_id, resource, used, limit, pct}` |
| `CONTRACT_EXCEEDED` | Limits breached | `{contract_id, resource, used, limit}` |
| `CONTRACT_COMPLETED` | Normal completion | `{contract_id, final_usage}` |
| `CONTRACT_CANCELLED` | Forcibly terminated | `{contract_id, reason}` |

### 3.4 Tool Events

| Event Type | Description | Event Data Schema |
|------------|-------------|-------------------|
| `TOOL_CALLED` | Tool invocation started | `{call_id, task_id, agent_id, tool_name, parameters}` |
| `TOOL_COMPLETED` | Tool returned result | `{call_id, result, latency_ms, tokens_used}` |
| `TOOL_FAILED` | Tool execution failed | `{call_id, error, latency_ms}` |
| `TOOL_TIMEOUT` | Tool exceeded timeout | `{call_id, timeout_ms}` |
| `TOOL_RETRIED` | Tool call retried | `{call_id, retry_count, reason}` |

### 3.5 Approval Events

| Event Type | Description | Event Data Schema |
|------------|-------------|-------------------|
| `APPROVAL_REQUESTED` | New approval needed | `{request_id, task_id, action_type, risk_score}` |
| `APPROVAL_CLUSTERED` | Added to batch | `{request_id, cluster_id}` |
| `APPROVAL_GRANTED` | Human approved | `{request_id, decided_by, reason?}` |
| `APPROVAL_DENIED` | Human rejected | `{request_id, decided_by, reason}` |
| `APPROVAL_EXPIRED` | Timed out | `{request_id, expires_at}` |
| `APPROVAL_AUTO_APPROVED` | Policy auto-approved | `{request_id, policy_id}` |

### 3.6 DAG Events

| Event Type | Description | Event Data Schema |
|------------|-------------|-------------------|
| `DAG_CREATED` | New DAG instantiated | `{dag_id, name, config, task_count}` |
| `DAG_STARTED` | Execution began | `{dag_id, started_at}` |
| `DAG_COMPLETED` | All tasks done | `{dag_id, stats}` |
| `DAG_FAILED` | Unrecoverable failure | `{dag_id, failed_task_id, error}` |
| `DAG_CANCELLED` | Manually cancelled | `{dag_id, reason, cancelled_by?}` |

### 3.7 Event Schema Definition (JSON Schema)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "ApexEvent",
  "type": "object",
  "required": ["event_id", "aggregate_type", "aggregate_id", "event_type", "event_data", "version"],
  "properties": {
    "event_id": { "type": "string", "format": "uuid" },
    "trace_id": { "type": "string", "maxLength": 32 },
    "span_id": { "type": "string", "maxLength": 16 },
    "aggregate_type": {
      "type": "string",
      "enum": ["task", "agent", "contract", "tool_call", "approval", "dag"]
    },
    "aggregate_id": { "type": "string", "format": "uuid" },
    "event_type": { "type": "string" },
    "event_data": { "type": "object" },
    "metadata": {
      "type": "object",
      "properties": {
        "correlation_id": { "type": "string" },
        "causation_id": { "type": "string" },
        "user_id": { "type": "string" },
        "source": { "type": "string" }
      }
    },
    "version": { "type": "integer", "minimum": 1 }
  }
}
```

---

## 4. State Consistency Model

### 4.1 Event Sourcing Architecture

```
+------------------+     +------------------+     +------------------+
|   Command        |     |   Event Store    |     |   Projections    |
|   Handlers       | --> |   (PostgreSQL)   | --> |   (Read Models)  |
+------------------+     +------------------+     +------------------+
        |                        |                        |
        v                        v                        v
+------------------+     +------------------+     +------------------+
|   Domain         |     |   Snapshots      |     |   Query          |
|   Aggregates     |     |   (Performance)  |     |   Handlers       |
+------------------+     +------------------+     +------------------+
```

#### Event Store Implementation

```sql
-- Append event with optimistic concurrency
CREATE OR REPLACE FUNCTION append_event(
    p_aggregate_type VARCHAR(50),
    p_aggregate_id UUID,
    p_event_type VARCHAR(100),
    p_event_data JSONB,
    p_expected_version INTEGER,
    p_trace_id VARCHAR(32) DEFAULT NULL,
    p_span_id VARCHAR(16) DEFAULT NULL,
    p_metadata JSONB DEFAULT '{}'
) RETURNS UUID AS $$
DECLARE
    v_current_version INTEGER;
    v_event_id UUID;
BEGIN
    -- Get current version with lock
    SELECT COALESCE(MAX(version), 0) INTO v_current_version
    FROM events
    WHERE aggregate_type = p_aggregate_type
      AND aggregate_id = p_aggregate_id
    FOR UPDATE;

    -- Optimistic concurrency check
    IF v_current_version != p_expected_version THEN
        RAISE EXCEPTION 'Concurrency conflict: expected version %, found %',
            p_expected_version, v_current_version
        USING ERRCODE = 'serialization_failure';
    END IF;

    -- Generate event ID
    v_event_id := uuid_generate_v4();

    -- Insert event
    INSERT INTO events (
        event_id, trace_id, span_id,
        aggregate_type, aggregate_id,
        event_type, event_data, metadata, version
    ) VALUES (
        v_event_id, p_trace_id, p_span_id,
        p_aggregate_type, p_aggregate_id,
        p_event_type, p_event_data, p_metadata, p_expected_version + 1
    );

    RETURN v_event_id;
END;
$$ LANGUAGE plpgsql;
```

#### State Reconstruction

```sql
-- Reconstruct aggregate state from events
CREATE OR REPLACE FUNCTION get_aggregate_events(
    p_aggregate_type VARCHAR(50),
    p_aggregate_id UUID,
    p_from_version INTEGER DEFAULT 0
) RETURNS TABLE (
    event_id UUID,
    event_type VARCHAR(100),
    event_data JSONB,
    metadata JSONB,
    version INTEGER,
    created_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        e.event_id,
        e.event_type,
        e.event_data,
        e.metadata,
        e.version,
        e.created_at
    FROM events e
    WHERE e.aggregate_type = p_aggregate_type
      AND e.aggregate_id = p_aggregate_id
      AND e.version > p_from_version
    ORDER BY e.version ASC;
END;
$$ LANGUAGE plpgsql;
```

#### Snapshot Management

```sql
-- Create snapshot at current state
CREATE OR REPLACE FUNCTION create_snapshot(
    p_aggregate_type VARCHAR(50),
    p_aggregate_id UUID,
    p_state_data JSONB
) RETURNS UUID AS $$
DECLARE
    v_version INTEGER;
    v_sequence BIGINT;
    v_snapshot_id UUID;
BEGIN
    -- Get latest version
    SELECT MAX(version), MAX(sequence_number)
    INTO v_version, v_sequence
    FROM events
    WHERE aggregate_type = p_aggregate_type
      AND aggregate_id = p_aggregate_id;

    IF v_version IS NULL THEN
        RAISE EXCEPTION 'No events found for aggregate';
    END IF;

    v_snapshot_id := uuid_generate_v4();

    INSERT INTO event_snapshots (
        id, aggregate_type, aggregate_id,
        state_data, version, event_sequence
    ) VALUES (
        v_snapshot_id, p_aggregate_type, p_aggregate_id,
        p_state_data, v_version, v_sequence
    );

    RETURN v_snapshot_id;
END;
$$ LANGUAGE plpgsql;

-- Get latest snapshot
CREATE OR REPLACE FUNCTION get_latest_snapshot(
    p_aggregate_type VARCHAR(50),
    p_aggregate_id UUID
) RETURNS TABLE (
    state_data JSONB,
    version INTEGER,
    event_sequence BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        s.state_data,
        s.version,
        s.event_sequence
    FROM event_snapshots s
    WHERE s.aggregate_type = p_aggregate_type
      AND s.aggregate_id = p_aggregate_id
    ORDER BY s.version DESC
    LIMIT 1;
END;
$$ LANGUAGE plpgsql;
```

### 4.2 CRDT for Parallel Writes

#### LWW-Register (Last-Writer-Wins)

For simple values where latest update wins:

```sql
-- LWW Register table for distributed state
CREATE TABLE lww_registers (
    key VARCHAR(255) PRIMARY KEY,
    value JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    node_id VARCHAR(50) NOT NULL
);

-- Merge function: higher timestamp wins, tie-break on node_id
CREATE OR REPLACE FUNCTION lww_merge(
    p_key VARCHAR(255),
    p_value JSONB,
    p_timestamp TIMESTAMPTZ,
    p_node_id VARCHAR(50)
) RETURNS BOOLEAN AS $$
DECLARE
    v_current_ts TIMESTAMPTZ;
    v_current_node VARCHAR(50);
BEGIN
    SELECT timestamp, node_id INTO v_current_ts, v_current_node
    FROM lww_registers WHERE key = p_key;

    IF NOT FOUND OR
       p_timestamp > v_current_ts OR
       (p_timestamp = v_current_ts AND p_node_id > v_current_node) THEN
        INSERT INTO lww_registers (key, value, timestamp, node_id)
        VALUES (p_key, p_value, p_timestamp, p_node_id)
        ON CONFLICT (key) DO UPDATE
        SET value = p_value, timestamp = p_timestamp, node_id = p_node_id;
        RETURN TRUE;
    END IF;

    RETURN FALSE;
END;
$$ LANGUAGE plpgsql;
```

#### G-Counter (Grow-Only Counter)

For token and cost accumulation across parallel agents:

```sql
-- G-Counter for distributed counting
CREATE TABLE g_counters (
    counter_id VARCHAR(255) NOT NULL,
    node_id VARCHAR(50) NOT NULL,
    value BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (counter_id, node_id)
);

-- Increment counter for a node
CREATE OR REPLACE FUNCTION g_counter_increment(
    p_counter_id VARCHAR(255),
    p_node_id VARCHAR(50),
    p_delta BIGINT
) RETURNS BIGINT AS $$
DECLARE
    v_new_value BIGINT;
BEGIN
    INSERT INTO g_counters (counter_id, node_id, value)
    VALUES (p_counter_id, p_node_id, p_delta)
    ON CONFLICT (counter_id, node_id) DO UPDATE
    SET value = g_counters.value + p_delta
    RETURNING value INTO v_new_value;

    RETURN v_new_value;
END;
$$ LANGUAGE plpgsql;

-- Get total counter value (sum across all nodes)
CREATE OR REPLACE FUNCTION g_counter_value(
    p_counter_id VARCHAR(255)
) RETURNS BIGINT AS $$
BEGIN
    RETURN COALESCE(
        (SELECT SUM(value) FROM g_counters WHERE counter_id = p_counter_id),
        0
    );
END;
$$ LANGUAGE plpgsql;
```

#### PN-Counter (Positive-Negative Counter)

For values that can increment and decrement:

```sql
-- PN-Counter using two G-Counters
CREATE TABLE pn_counters (
    counter_id VARCHAR(255) NOT NULL,
    node_id VARCHAR(50) NOT NULL,
    positive BIGINT NOT NULL DEFAULT 0,
    negative BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (counter_id, node_id)
);

-- Increment/decrement PN counter
CREATE OR REPLACE FUNCTION pn_counter_update(
    p_counter_id VARCHAR(255),
    p_node_id VARCHAR(50),
    p_delta BIGINT
) RETURNS BIGINT AS $$
BEGIN
    IF p_delta >= 0 THEN
        INSERT INTO pn_counters (counter_id, node_id, positive)
        VALUES (p_counter_id, p_node_id, p_delta)
        ON CONFLICT (counter_id, node_id) DO UPDATE
        SET positive = pn_counters.positive + p_delta;
    ELSE
        INSERT INTO pn_counters (counter_id, node_id, negative)
        VALUES (p_counter_id, p_node_id, ABS(p_delta))
        ON CONFLICT (counter_id, node_id) DO UPDATE
        SET negative = pn_counters.negative + ABS(p_delta);
    END IF;

    RETURN (
        SELECT SUM(positive) - SUM(negative)
        FROM pn_counters WHERE counter_id = p_counter_id
    );
END;
$$ LANGUAGE plpgsql;
```

#### OR-Set (Observed-Remove Set)

For merging tool results from parallel agents:

```sql
-- OR-Set for conflict-free set operations
CREATE TABLE or_sets (
    set_id VARCHAR(255) NOT NULL,
    element JSONB NOT NULL,
    unique_tag UUID NOT NULL,
    tombstone BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (set_id, unique_tag)
);

-- Add element to OR-Set
CREATE OR REPLACE FUNCTION or_set_add(
    p_set_id VARCHAR(255),
    p_element JSONB
) RETURNS UUID AS $$
DECLARE
    v_tag UUID;
BEGIN
    v_tag := uuid_generate_v4();
    INSERT INTO or_sets (set_id, element, unique_tag)
    VALUES (p_set_id, p_element, v_tag);
    RETURN v_tag;
END;
$$ LANGUAGE plpgsql;

-- Remove element from OR-Set (tombstone all matching)
CREATE OR REPLACE FUNCTION or_set_remove(
    p_set_id VARCHAR(255),
    p_element JSONB
) RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER;
BEGIN
    UPDATE or_sets
    SET tombstone = TRUE
    WHERE set_id = p_set_id
      AND element = p_element
      AND tombstone = FALSE;
    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

-- Get current set elements
CREATE OR REPLACE FUNCTION or_set_elements(
    p_set_id VARCHAR(255)
) RETURNS TABLE (element JSONB) AS $$
BEGIN
    RETURN QUERY
    SELECT DISTINCT o.element
    FROM or_sets o
    WHERE o.set_id = p_set_id
      AND o.tombstone = FALSE;
END;
$$ LANGUAGE plpgsql;
```

### 4.3 Optimistic Concurrency Control

```sql
-- Update with version check
CREATE OR REPLACE FUNCTION update_task_with_version(
    p_task_id UUID,
    p_status task_status,
    p_output JSONB,
    p_expected_version INTEGER
) RETURNS BOOLEAN AS $$
DECLARE
    v_updated BOOLEAN;
BEGIN
    UPDATE tasks
    SET
        status = p_status,
        output = p_output,
        version = version + 1,
        completed_at = CASE WHEN p_status IN ('completed', 'failed') THEN NOW() ELSE completed_at END
    WHERE id = p_task_id
      AND version = p_expected_version;

    GET DIAGNOSTICS v_updated = ROW_COUNT;

    IF NOT v_updated THEN
        RAISE EXCEPTION 'Optimistic concurrency violation on task %', p_task_id
        USING ERRCODE = 'serialization_failure';
    END IF;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Retry wrapper for optimistic concurrency
CREATE OR REPLACE FUNCTION with_retry(
    p_max_retries INTEGER DEFAULT 3
) RETURNS VOID AS $$
BEGIN
    -- This is a placeholder showing the pattern
    -- Actual retry logic implemented in application layer
    NULL;
END;
$$ LANGUAGE plpgsql;
```

---

## 5. Vector Database

### 5.1 pgvector Schema

Using PostgreSQL's pgvector extension for embedding storage:

```sql
-- Ensure vector extension is enabled
CREATE EXTENSION IF NOT EXISTS vector;

-- Agent context embeddings
CREATE TABLE agent_context_embeddings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    -- Content
    content_type VARCHAR(50) NOT NULL,  -- 'task_output', 'tool_result', 'conversation', 'document'
    content TEXT NOT NULL,
    content_hash VARCHAR(64) NOT NULL,  -- SHA-256 for deduplication

    -- Embedding (1536 dimensions for OpenAI ada-002, 3072 for text-embedding-3-large)
    embedding vector(1536) NOT NULL,

    -- Metadata
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- References
    source_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    source_tool_call_id UUID REFERENCES tool_calls(id) ON DELETE SET NULL,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,  -- For automatic cleanup

    -- Constraints
    CONSTRAINT unique_content_hash UNIQUE (agent_id, content_hash)
);

-- IVFFlat index for approximate nearest neighbor search
CREATE INDEX idx_embeddings_ivfflat
ON agent_context_embeddings
USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 100);

-- Alternative: HNSW index for better recall (slower inserts)
-- CREATE INDEX idx_embeddings_hnsw
-- ON agent_context_embeddings
-- USING hnsw (embedding vector_cosine_ops)
-- WITH (m = 16, ef_construction = 64);

-- Query support indexes
CREATE INDEX idx_embeddings_agent
ON agent_context_embeddings(agent_id, created_at DESC);

CREATE INDEX idx_embeddings_type
ON agent_context_embeddings(content_type, agent_id);

COMMENT ON TABLE agent_context_embeddings IS 'Vector embeddings for semantic search over agent context';
COMMENT ON COLUMN agent_context_embeddings.embedding IS '1536-dim vector from OpenAI text-embedding-ada-002';
```

### 5.2 Semantic Search Functions

```sql
-- Find similar context for an agent
CREATE OR REPLACE FUNCTION find_similar_context(
    p_agent_id UUID,
    p_query_embedding vector(1536),
    p_limit INTEGER DEFAULT 10,
    p_content_types VARCHAR(50)[] DEFAULT NULL,
    p_min_similarity FLOAT DEFAULT 0.7
) RETURNS TABLE (
    id UUID,
    content TEXT,
    content_type VARCHAR(50),
    metadata JSONB,
    similarity FLOAT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        e.id,
        e.content,
        e.content_type,
        e.metadata,
        1 - (e.embedding <=> p_query_embedding) AS similarity
    FROM agent_context_embeddings e
    WHERE e.agent_id = p_agent_id
      AND (p_content_types IS NULL OR e.content_type = ANY(p_content_types))
      AND (e.expires_at IS NULL OR e.expires_at > NOW())
      AND 1 - (e.embedding <=> p_query_embedding) >= p_min_similarity
    ORDER BY e.embedding <=> p_query_embedding
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;

-- Cross-agent context search (for knowledge sharing)
CREATE OR REPLACE FUNCTION find_global_context(
    p_query_embedding vector(1536),
    p_limit INTEGER DEFAULT 20,
    p_exclude_agent_id UUID DEFAULT NULL
) RETURNS TABLE (
    id UUID,
    agent_id UUID,
    content TEXT,
    content_type VARCHAR(50),
    similarity FLOAT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        e.id,
        e.agent_id,
        e.content,
        e.content_type,
        1 - (e.embedding <=> p_query_embedding) AS similarity
    FROM agent_context_embeddings e
    WHERE (p_exclude_agent_id IS NULL OR e.agent_id != p_exclude_agent_id)
      AND (e.expires_at IS NULL OR e.expires_at > NOW())
    ORDER BY e.embedding <=> p_query_embedding
    LIMIT p_limit;
END;
$$ LANGUAGE plpgsql;
```

### 5.3 LanceDB Integration (Alternative)

For scenarios requiring higher performance or columnar storage:

```yaml
# LanceDB schema definition (applied via Rust/Python SDK)
schema:
  name: agent_context
  columns:
    - name: id
      type: string
      primary_key: true
    - name: agent_id
      type: string
      index: true
    - name: content
      type: string
    - name: content_type
      type: string
      index: true
    - name: embedding
      type: vector
      dimension: 1536
      index:
        type: IVF_PQ
        num_partitions: 256
        num_sub_vectors: 96
    - name: metadata
      type: json
    - name: created_at
      type: timestamp
      index: true
```

```rust
// Rust integration example
use lancedb::{connect, Table};

pub struct ContextStore {
    table: Table,
}

impl ContextStore {
    pub async fn search(
        &self,
        query_embedding: Vec<f32>,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ContextRecord>> {
        let mut query = self.table
            .search(&query_embedding)
            .limit(limit)
            .metric_type(MetricType::Cosine);

        if let Some(aid) = agent_id {
            query = query.filter(format!("agent_id = '{}'", aid));
        }

        query.execute().await
    }
}
```

---

## 6. Cache Layer

### 6.1 Redis Schema

```redis
# Key naming conventions
# {entity}:{id}:{field} for single values
# {entity}:{id}:* for hash fields
# {entity}:index:{field}:{value} for lookups

# ============================================
# Agent State Cache (TTL: 5 minutes)
# ============================================

# Agent hash
HSET agent:{agent_id} \
    status "idle" \
    current_load 3 \
    reputation_score 0.95 \
    last_active_at "2024-01-15T10:30:00Z"
EXPIRE agent:{agent_id} 300

# Available agents sorted set (score = reputation * available_capacity)
ZADD agents:available 0.95 {agent_id_1} 0.87 {agent_id_2}

# Agents by model index
SADD agents:model:gpt-4 {agent_id_1} {agent_id_2}
SADD agents:model:claude-3-opus {agent_id_3}

# ============================================
# Task State Cache (TTL: 1 hour)
# ============================================

# Task hash
HSET task:{task_id} \
    status "running" \
    agent_id {agent_id} \
    dag_id {dag_id} \
    started_at "2024-01-15T10:25:00Z" \
    priority 5
EXPIRE task:{task_id} 3600

# Task output cache (larger TTL for expensive computations)
SET task:{task_id}:output '{"result": "..."}' EX 7200

# Ready tasks priority queue
ZADD tasks:ready:{dag_id} {priority} {task_id}

# Running tasks set (for monitoring)
SADD tasks:running {task_id_1} {task_id_2}

# ============================================
# Rate Limiting (Sliding window)
# ============================================

# API calls per agent (window: 1 minute)
# Using sorted set with timestamps as scores
ZADD ratelimit:agent:{agent_id}:api {timestamp} {request_id}
ZREMRANGEBYSCORE ratelimit:agent:{agent_id}:api 0 {timestamp - 60000}
ZCARD ratelimit:agent:{agent_id}:api  # Current count

# Token usage per contract (window: contract duration)
INCRBY contract:{contract_id}:tokens {tokens_used}
EXPIRE contract:{contract_id}:tokens {time_limit_seconds}

# Global rate limits
INCR ratelimit:global:minute
EXPIRE ratelimit:global:minute 60

# ============================================
# Distributed Locks
# ============================================

# Task assignment lock
SET lock:task:{task_id} {agent_id} NX EX 30

# Agent exclusive operation lock
SET lock:agent:{agent_id}:exclusive {operation_id} NX EX 60

# ============================================
# Pub/Sub Channels
# ============================================

# Real-time updates
PUBLISH apex:events '{"type": "TASK_COMPLETED", "task_id": "...", "output": {...}}'
PUBLISH apex:agent:{agent_id} '{"type": "NEW_TASK", "task_id": "..."}'
PUBLISH apex:dag:{dag_id} '{"type": "PROGRESS", "completed": 15, "total": 20}'

# Pattern subscriptions
PSUBSCRIBE apex:events:*
PSUBSCRIBE apex:agent:*
```

### 6.2 Redis Data Structures

```lua
-- Lua script: Atomic task assignment
-- KEYS[1] = ready queue key
-- KEYS[2] = running set key
-- KEYS[3] = task hash key
-- ARGV[1] = agent_id
-- ARGV[2] = current timestamp

local task_id = redis.call('ZPOPMIN', KEYS[1])
if not task_id or #task_id == 0 then
    return nil
end

task_id = task_id[1]

-- Add to running set
redis.call('SADD', KEYS[2], task_id)

-- Update task hash
redis.call('HSET', KEYS[3] .. task_id,
    'status', 'running',
    'agent_id', ARGV[1],
    'started_at', ARGV[2])

return task_id
```

```lua
-- Lua script: Rate limiting with sliding window
-- KEYS[1] = rate limit key
-- ARGV[1] = current timestamp
-- ARGV[2] = window size (ms)
-- ARGV[3] = max requests
-- ARGV[4] = request id

local key = KEYS[1]
local now = tonumber(ARGV[1])
local window = tonumber(ARGV[2])
local max_requests = tonumber(ARGV[3])
local request_id = ARGV[4]

-- Remove old entries
redis.call('ZREMRANGEBYSCORE', key, 0, now - window)

-- Check current count
local count = redis.call('ZCARD', key)
if count >= max_requests then
    return {0, count, max_requests}  -- Rejected
end

-- Add new request
redis.call('ZADD', key, now, request_id)
redis.call('EXPIRE', key, math.ceil(window / 1000))

return {1, count + 1, max_requests}  -- Accepted
```

### 6.3 Cache Invalidation Strategy

```sql
-- PostgreSQL trigger for cache invalidation via NOTIFY
CREATE OR REPLACE FUNCTION notify_cache_invalidation()
RETURNS TRIGGER AS $$
DECLARE
    payload JSONB;
BEGIN
    payload := jsonb_build_object(
        'table', TG_TABLE_NAME,
        'operation', TG_OP,
        'id', COALESCE(NEW.id, OLD.id)
    );

    -- Add relevant fields based on table
    IF TG_TABLE_NAME = 'tasks' THEN
        payload := payload || jsonb_build_object(
            'dag_id', COALESCE(NEW.dag_id, OLD.dag_id),
            'status', NEW.status
        );
    ELSIF TG_TABLE_NAME = 'agents' THEN
        payload := payload || jsonb_build_object(
            'status', NEW.status,
            'current_load', NEW.current_load
        );
    END IF;

    PERFORM pg_notify('cache_invalidation', payload::TEXT);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER tasks_cache_invalidation
    AFTER INSERT OR UPDATE OR DELETE ON tasks
    FOR EACH ROW EXECUTE FUNCTION notify_cache_invalidation();

CREATE TRIGGER agents_cache_invalidation
    AFTER INSERT OR UPDATE OR DELETE ON agents
    FOR EACH ROW EXECUTE FUNCTION notify_cache_invalidation();
```

---

## 7. Migration Strategy

### 7.1 Migration Tool: sqlx

Using sqlx for type-safe, compile-time verified migrations:

```
migrations/
├── 20240115000001_initial_schema.sql
├── 20240115000002_create_indexes.sql
├── 20240115000003_create_functions.sql
├── 20240115000004_add_vector_support.sql
└── 20240115000005_add_crdt_tables.sql
```

### 7.2 Migration Files

**20240115000001_initial_schema.sql**
```sql
-- Initial schema creation
-- This migration creates all core tables

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- Create enums
CREATE TYPE task_status AS ENUM (
    'pending', 'ready', 'running', 'completed', 'failed', 'cancelled'
);
CREATE TYPE agent_status AS ENUM ('idle', 'busy', 'error', 'paused');
CREATE TYPE contract_status AS ENUM ('active', 'completed', 'exceeded', 'cancelled');
CREATE TYPE approval_status AS ENUM ('pending', 'approved', 'denied', 'expired');

-- Create tables (as defined in Section 1)
-- [Full DDL statements here]

-- Down migration
-- DROP TYPE IF EXISTS task_status CASCADE;
-- DROP TYPE IF EXISTS agent_status CASCADE;
-- DROP TYPE IF EXISTS contract_status CASCADE;
-- DROP TYPE IF EXISTS approval_status CASCADE;
```

**20240115000002_create_indexes.sql**
```sql
-- Create all indexes
-- [Full index statements from Section 2]
```

**20240115000003_create_functions.sql**
```sql
-- Create stored procedures and functions
-- [Event sourcing functions, CRDT functions, etc.]
```

### 7.3 Rollback Procedures

```sql
-- Rollback template
BEGIN;

-- Record rollback
INSERT INTO schema_migrations_log (version, direction, executed_at)
VALUES ('20240115000001', 'down', NOW());

-- Rollback statements
DROP TABLE IF EXISTS approval_requests CASCADE;
DROP TABLE IF EXISTS tool_calls CASCADE;
DROP TABLE IF EXISTS agent_contracts CASCADE;
DROP TABLE IF EXISTS task_dependencies CASCADE;
DROP TABLE IF EXISTS events CASCADE;
DROP TABLE IF EXISTS event_snapshots CASCADE;
DROP TABLE IF EXISTS tasks CASCADE;
DROP TABLE IF EXISTS agents CASCADE;
DROP TABLE IF EXISTS dags CASCADE;

DROP TYPE IF EXISTS approval_status CASCADE;
DROP TYPE IF EXISTS contract_status CASCADE;
DROP TYPE IF EXISTS agent_status CASCADE;
DROP TYPE IF EXISTS task_status CASCADE;

COMMIT;
```

### 7.4 Migration Verification

```sql
-- Verification queries after migration
SELECT
    schemaname,
    tablename,
    tableowner
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY tablename;

-- Check indexes
SELECT
    indexname,
    tablename,
    indexdef
FROM pg_indexes
WHERE schemaname = 'public'
ORDER BY tablename, indexname;

-- Check constraints
SELECT
    conname,
    conrelid::regclass AS table_name,
    contype,
    pg_get_constraintdef(oid) AS definition
FROM pg_constraint
WHERE connamespace = 'public'::regnamespace
ORDER BY conrelid::regclass::text, conname;

-- Check functions
SELECT
    proname,
    pronargs,
    prorettype::regtype
FROM pg_proc
WHERE pronamespace = 'public'::regnamespace
ORDER BY proname;
```

---

## 8. Backup Strategy

### 8.1 WAL Archiving Configuration

```ini
# postgresql.conf

# Enable WAL archiving
wal_level = replica
archive_mode = on
archive_command = 'aws s3 cp %p s3://apex-wal-archive/%f --sse AES256'
archive_timeout = 300  # Archive every 5 minutes minimum

# WAL retention
max_wal_size = 4GB
min_wal_size = 1GB
wal_keep_size = 2GB

# Checkpoint settings
checkpoint_timeout = 15min
checkpoint_completion_target = 0.9
```

### 8.2 Backup Scripts

**Daily Full Backup**
```bash
#!/bin/bash
# daily_backup.sh

set -euo pipefail

BACKUP_DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/var/backups/apex"
S3_BUCKET="s3://apex-backups"
RETENTION_DAYS=30

# Create backup directory
mkdir -p "${BACKUP_DIR}"

# Full backup using pg_basebackup
pg_basebackup \
    -h localhost \
    -U replication_user \
    -D "${BACKUP_DIR}/base_${BACKUP_DATE}" \
    -Ft \
    -z \
    -Xs \
    -P

# Upload to S3
aws s3 cp \
    "${BACKUP_DIR}/base_${BACKUP_DATE}" \
    "${S3_BUCKET}/daily/${BACKUP_DATE}/" \
    --recursive \
    --sse AES256

# Clean up local backups older than 7 days
find "${BACKUP_DIR}" -type d -name "base_*" -mtime +7 -exec rm -rf {} +

# Clean up S3 backups older than retention period
aws s3 ls "${S3_BUCKET}/daily/" | while read -r line; do
    backup_date=$(echo "$line" | awk '{print $2}' | tr -d '/')
    backup_epoch=$(date -d "${backup_date:0:8}" +%s 2>/dev/null || echo 0)
    cutoff_epoch=$(date -d "-${RETENTION_DAYS} days" +%s)

    if [[ $backup_epoch -lt $cutoff_epoch && $backup_epoch -gt 0 ]]; then
        aws s3 rm "${S3_BUCKET}/daily/${backup_date}/" --recursive
    fi
done

echo "Backup completed: ${BACKUP_DATE}"
```

**Point-in-Time Recovery**
```bash
#!/bin/bash
# pitr_restore.sh

set -euo pipefail

TARGET_TIME="${1:-}"  # ISO 8601 format: 2024-01-15T10:30:00Z
RESTORE_DIR="/var/lib/postgresql/restore"
S3_BUCKET="s3://apex-backups"

if [[ -z "$TARGET_TIME" ]]; then
    echo "Usage: $0 <target_time>"
    echo "Example: $0 '2024-01-15T10:30:00Z'"
    exit 1
fi

# Find the base backup before target time
BASE_BACKUP=$(aws s3 ls "${S3_BUCKET}/daily/" | \
    awk '{print $2}' | tr -d '/' | \
    while read -r backup; do
        backup_time="${backup:0:8}T${backup:9:6}"
        if [[ "$backup_time" < "$TARGET_TIME" ]]; then
            echo "$backup"
        fi
    done | tail -1)

if [[ -z "$BASE_BACKUP" ]]; then
    echo "No suitable base backup found"
    exit 1
fi

echo "Using base backup: ${BASE_BACKUP}"

# Stop PostgreSQL
sudo systemctl stop postgresql

# Clear data directory
sudo rm -rf /var/lib/postgresql/data/*

# Restore base backup
mkdir -p "${RESTORE_DIR}"
aws s3 cp "${S3_BUCKET}/daily/${BASE_BACKUP}/" "${RESTORE_DIR}/" --recursive
tar -xzf "${RESTORE_DIR}/base.tar.gz" -C /var/lib/postgresql/data/

# Create recovery.conf
cat > /var/lib/postgresql/data/recovery.conf << EOF
restore_command = 'aws s3 cp s3://apex-wal-archive/%f %p'
recovery_target_time = '${TARGET_TIME}'
recovery_target_action = 'promote'
EOF

# Set permissions
sudo chown -R postgres:postgres /var/lib/postgresql/data

# Start PostgreSQL
sudo systemctl start postgresql

echo "PITR restore initiated to ${TARGET_TIME}"
echo "Monitor recovery with: SELECT pg_is_in_recovery();"
```

### 8.3 Cross-Region Replication

```sql
-- Primary server: Create replication slot
SELECT pg_create_physical_replication_slot('replica_us_west_2');

-- Replica server: recovery.conf / postgresql.auto.conf
primary_conninfo = 'host=primary.apex.internal port=5432 user=replication_user password=xxx sslmode=require'
primary_slot_name = 'replica_us_west_2'
```

```yaml
# Kubernetes StatefulSet for replica (example)
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: apex-postgres-replica
  namespace: apex
spec:
  serviceName: apex-postgres-replica
  replicas: 1
  selector:
    matchLabels:
      app: apex-postgres-replica
  template:
    metadata:
      labels:
        app: apex-postgres-replica
    spec:
      containers:
      - name: postgres
        image: postgres:16
        env:
        - name: POSTGRES_PRIMARY_HOST
          value: "apex-postgres-primary.apex.svc.cluster.local"
        - name: POSTGRES_REPLICATION_MODE
          value: "slave"
        volumeMounts:
        - name: data
          mountPath: /var/lib/postgresql/data
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: gp3
      resources:
        requests:
          storage: 500Gi
```

### 8.4 Backup Verification

```bash
#!/bin/bash
# verify_backup.sh

set -euo pipefail

BACKUP_PATH="${1}"
VERIFY_DIR="/tmp/backup_verify_$$"

# Create isolated verification environment
mkdir -p "${VERIFY_DIR}"
trap "rm -rf ${VERIFY_DIR}" EXIT

# Extract backup
tar -xzf "${BACKUP_PATH}/base.tar.gz" -C "${VERIFY_DIR}"

# Start temporary PostgreSQL instance
pg_ctl -D "${VERIFY_DIR}" -o "-p 5433" -w start

# Run verification queries
psql -p 5433 -d apex << 'EOF'
-- Verify table existence
SELECT COUNT(*) AS table_count FROM information_schema.tables
WHERE table_schema = 'public';

-- Verify row counts
SELECT
    'tasks' AS table_name, COUNT(*) AS row_count FROM tasks
UNION ALL
SELECT 'agents', COUNT(*) FROM agents
UNION ALL
SELECT 'events', COUNT(*) FROM events;

-- Verify latest data
SELECT MAX(created_at) AS latest_event FROM events;

-- Verify index health
SELECT
    indexrelname,
    idx_scan,
    idx_tup_read
FROM pg_stat_user_indexes
ORDER BY idx_scan DESC
LIMIT 10;
EOF

# Stop verification instance
pg_ctl -D "${VERIFY_DIR}" -m fast stop

echo "Backup verification completed successfully"
```

### 8.5 Disaster Recovery Runbook

| Step | Action | RTO Impact | Command |
|------|--------|------------|---------|
| 1 | Identify failure | 0-5 min | Monitor alerts |
| 2 | Failover to replica | 1-2 min | `pg_ctl promote` |
| 3 | Update DNS/connection strings | 2-5 min | Update ConfigMap |
| 4 | Verify application connectivity | 5-10 min | Health checks |
| 5 | Begin new replica setup | Background | Run restore script |
| 6 | Post-incident review | N/A | Document learnings |

**Total RTO Target: < 15 minutes**
**RPO Target: < 5 minutes (WAL archive interval)**

---

## Appendix A: Complete DDL Script

```sql
-- ============================================
-- Project Apex: Complete Database Schema
-- Version: 1.0.0
-- Generated: 2024-01-15
-- ============================================

-- Enable extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
CREATE EXTENSION IF NOT EXISTS "pg_trgm";
CREATE EXTENSION IF NOT EXISTS "vector";

-- Create custom types
CREATE TYPE task_status AS ENUM (
    'pending', 'ready', 'running', 'completed', 'failed', 'cancelled'
);

CREATE TYPE agent_status AS ENUM (
    'idle', 'busy', 'error', 'paused'
);

CREATE TYPE contract_status AS ENUM (
    'active', 'completed', 'exceeded', 'cancelled'
);

CREATE TYPE approval_status AS ENUM (
    'pending', 'approved', 'denied', 'expired'
);

-- DAGs table
CREATE TABLE dags (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    config JSONB NOT NULL DEFAULT '{}'::JSONB,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    total_tasks INTEGER NOT NULL DEFAULT 0,
    completed_tasks INTEGER NOT NULL DEFAULT 0,
    failed_tasks INTEGER NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    total_cost DECIMAL(12, 6) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1
);

-- Agents table
CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    model VARCHAR(100) NOT NULL,
    system_prompt TEXT,
    tools JSONB NOT NULL DEFAULT '[]'::JSONB,
    status agent_status NOT NULL DEFAULT 'idle',
    current_load INTEGER NOT NULL DEFAULT 0,
    max_load INTEGER NOT NULL DEFAULT 10,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    total_cost DECIMAL(12, 6) NOT NULL DEFAULT 0,
    reputation_score DECIMAL(5, 4) NOT NULL DEFAULT 1.0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1,
    CONSTRAINT valid_load CHECK (current_load >= 0 AND current_load <= max_load),
    CONSTRAINT valid_max_load CHECK (max_load > 0 AND max_load <= 1000),
    CONSTRAINT valid_reputation CHECK (reputation_score >= 0 AND reputation_score <= 1),
    CONSTRAINT valid_counts CHECK (success_count >= 0 AND failure_count >= 0),
    CONSTRAINT valid_tools_array CHECK (jsonb_typeof(tools) = 'array')
);

-- Tasks table
CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    parent_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    dag_id UUID NOT NULL REFERENCES dags(id) ON DELETE CASCADE,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    status task_status NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,
    input JSONB NOT NULL,
    output JSONB,
    error TEXT,
    tokens_used INTEGER NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    version INTEGER NOT NULL DEFAULT 1,
    CONSTRAINT valid_retry_count CHECK (retry_count >= 0 AND retry_count <= max_retries),
    CONSTRAINT valid_tokens CHECK (tokens_used >= 0),
    CONSTRAINT valid_cost CHECK (cost_dollars >= 0),
    CONSTRAINT valid_priority CHECK (priority >= -100 AND priority <= 100)
);

-- Agent contracts table
CREATE TABLE agent_contracts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    parent_contract_id UUID REFERENCES agent_contracts(id) ON DELETE SET NULL,
    token_limit INTEGER NOT NULL,
    token_used INTEGER NOT NULL DEFAULT 0,
    cost_limit DECIMAL(10, 6) NOT NULL,
    cost_used DECIMAL(10, 6) NOT NULL DEFAULT 0,
    time_limit_seconds INTEGER NOT NULL,
    api_call_limit INTEGER NOT NULL,
    api_calls_used INTEGER NOT NULL DEFAULT 0,
    status contract_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    version INTEGER NOT NULL DEFAULT 1,
    CONSTRAINT valid_token_usage CHECK (token_used >= 0 AND token_used <= token_limit * 1.1),
    CONSTRAINT valid_cost_usage CHECK (cost_used >= 0),
    CONSTRAINT valid_api_usage CHECK (api_calls_used >= 0),
    CONSTRAINT valid_limits CHECK (
        token_limit > 0 AND
        cost_limit > 0 AND
        time_limit_seconds > 0 AND
        api_call_limit > 0
    )
);

-- Tool calls table
CREATE TABLE tool_calls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    parent_span_id VARCHAR(16),
    tool_name VARCHAR(255) NOT NULL,
    parameters JSONB,
    result JSONB,
    error TEXT,
    tokens_used INTEGER NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,
    latency_ms INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    CONSTRAINT valid_latency CHECK (latency_ms IS NULL OR latency_ms >= 0),
    CONSTRAINT valid_tool_tokens CHECK (tokens_used >= 0),
    CONSTRAINT valid_tool_cost CHECK (cost_dollars >= 0)
);

-- Events table
CREATE TABLE events (
    id BIGSERIAL PRIMARY KEY,
    event_id UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4(),
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    aggregate_type VARCHAR(50) NOT NULL,
    aggregate_id UUID NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    event_data JSONB NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    version INTEGER NOT NULL,
    sequence_number BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT valid_version CHECK (version > 0),
    CONSTRAINT unique_aggregate_version UNIQUE (aggregate_type, aggregate_id, version)
);

-- Event snapshots table
CREATE TABLE event_snapshots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    aggregate_type VARCHAR(50) NOT NULL,
    aggregate_id UUID NOT NULL,
    state_data JSONB NOT NULL,
    version INTEGER NOT NULL,
    event_sequence BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_snapshot_version UNIQUE (aggregate_type, aggregate_id, version)
);

-- Task dependencies table
CREATE TABLE task_dependencies (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    dependency_type VARCHAR(50) NOT NULL DEFAULT 'strict',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (task_id, depends_on_id),
    CONSTRAINT no_self_dependency CHECK (task_id != depends_on_id)
);

-- Approval requests table
CREATE TABLE approval_requests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    action_type VARCHAR(100) NOT NULL,
    action_data JSONB NOT NULL,
    risk_score DECIMAL(3, 2),
    risk_factors JSONB DEFAULT '[]'::JSONB,
    cluster_id UUID,
    status approval_status NOT NULL DEFAULT 'pending',
    decided_by VARCHAR(255),
    decision_reason TEXT,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    CONSTRAINT valid_risk_score CHECK (risk_score IS NULL OR (risk_score >= 0 AND risk_score <= 1)),
    CONSTRAINT decision_requires_status CHECK (
        (status IN ('approved', 'denied') AND decided_at IS NOT NULL) OR
        (status IN ('pending', 'expired'))
    )
);

-- Vector embeddings table
CREATE TABLE agent_context_embeddings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    content_type VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    content_hash VARCHAR(64) NOT NULL,
    embedding vector(1536) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    source_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    source_tool_call_id UUID REFERENCES tool_calls(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    CONSTRAINT unique_content_hash UNIQUE (agent_id, content_hash)
);

-- CRDT tables
CREATE TABLE lww_registers (
    key VARCHAR(255) PRIMARY KEY,
    value JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    node_id VARCHAR(50) NOT NULL
);

CREATE TABLE g_counters (
    counter_id VARCHAR(255) NOT NULL,
    node_id VARCHAR(50) NOT NULL,
    value BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (counter_id, node_id)
);

CREATE TABLE pn_counters (
    counter_id VARCHAR(255) NOT NULL,
    node_id VARCHAR(50) NOT NULL,
    positive BIGINT NOT NULL DEFAULT 0,
    negative BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (counter_id, node_id)
);

CREATE TABLE or_sets (
    set_id VARCHAR(255) NOT NULL,
    element JSONB NOT NULL,
    unique_tag UUID NOT NULL,
    tombstone BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (set_id, unique_tag)
);

-- Create sequence for events
CREATE SEQUENCE IF NOT EXISTS event_sequence_seq;

-- ============================================
-- Create all indexes
-- ============================================

-- Tasks indexes
CREATE INDEX idx_tasks_dag_status ON tasks(dag_id, status);
CREATE INDEX idx_tasks_ready_priority ON tasks(priority DESC, created_at ASC) WHERE status = 'ready';
CREATE INDEX idx_tasks_running ON tasks(agent_id, started_at) WHERE status = 'running';
CREATE INDEX idx_tasks_failed_retryable ON tasks(dag_id, created_at) WHERE status = 'failed' AND retry_count < max_retries;
CREATE INDEX idx_tasks_parent ON tasks(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_tasks_input_gin ON tasks USING GIN(input jsonb_path_ops);
CREATE INDEX idx_tasks_output_gin ON tasks USING GIN(output jsonb_path_ops) WHERE output IS NOT NULL;
CREATE INDEX idx_tasks_created_at ON tasks(created_at DESC);
CREATE INDEX idx_tasks_completed_at ON tasks(completed_at DESC) WHERE completed_at IS NOT NULL;

-- Agents indexes
CREATE INDEX idx_agents_available ON agents(model, current_load, reputation_score DESC) WHERE status = 'idle';
CREATE INDEX idx_agents_name_trgm ON agents USING GIN(name gin_trgm_ops);
CREATE INDEX idx_agents_performance ON agents(reputation_score DESC, success_count DESC);
CREATE INDEX idx_agents_tools_gin ON agents USING GIN(tools jsonb_path_ops);
CREATE INDEX idx_agents_last_active ON agents(last_active_at DESC) WHERE status != 'paused';

-- Contracts indexes
CREATE INDEX idx_contracts_agent_active ON agent_contracts(agent_id, created_at DESC) WHERE status = 'active';
CREATE INDEX idx_contracts_task ON agent_contracts(task_id);
CREATE INDEX idx_contracts_expiring ON agent_contracts(expires_at) WHERE status = 'active' AND expires_at IS NOT NULL;
CREATE INDEX idx_contracts_parent ON agent_contracts(parent_contract_id) WHERE parent_contract_id IS NOT NULL;

-- Tool calls indexes
CREATE INDEX idx_tool_calls_task ON tool_calls(task_id, created_at DESC);
CREATE INDEX idx_tool_calls_agent ON tool_calls(agent_id, created_at DESC);
CREATE INDEX idx_tool_calls_trace ON tool_calls(trace_id, span_id) WHERE trace_id IS NOT NULL;
CREATE INDEX idx_tool_calls_tool_name ON tool_calls(tool_name, created_at DESC);
CREATE INDEX idx_tool_calls_errors ON tool_calls(tool_name, created_at DESC) WHERE error IS NOT NULL;
CREATE INDEX idx_tool_calls_params_gin ON tool_calls USING GIN(parameters jsonb_path_ops) WHERE parameters IS NOT NULL;

-- Events indexes
CREATE INDEX idx_events_aggregate ON events(aggregate_type, aggregate_id, version ASC);
CREATE INDEX idx_events_sequence ON events(sequence_number ASC);
CREATE INDEX idx_events_type ON events(event_type, created_at DESC);
CREATE INDEX idx_events_trace ON events(trace_id) WHERE trace_id IS NOT NULL;
CREATE INDEX idx_events_created ON events(created_at DESC);
CREATE INDEX idx_events_created_brin ON events USING BRIN(created_at);
CREATE INDEX idx_events_data_gin ON events USING GIN(event_data jsonb_path_ops);

-- Snapshots index
CREATE INDEX idx_snapshots_lookup ON event_snapshots(aggregate_type, aggregate_id, version DESC);

-- Dependencies indexes
CREATE INDEX idx_deps_task ON task_dependencies(task_id);
CREATE INDEX idx_deps_depends_on ON task_dependencies(depends_on_id);

-- Approvals indexes
CREATE INDEX idx_approvals_pending ON approval_requests(created_at ASC, risk_score DESC) WHERE status = 'pending';
CREATE INDEX idx_approvals_cluster ON approval_requests(cluster_id, status) WHERE cluster_id IS NOT NULL;
CREATE INDEX idx_approvals_agent ON approval_requests(agent_id, created_at DESC);
CREATE INDEX idx_approvals_expiring ON approval_requests(expires_at) WHERE status = 'pending' AND expires_at IS NOT NULL;

-- Embeddings indexes
CREATE INDEX idx_embeddings_ivfflat ON agent_context_embeddings USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);
CREATE INDEX idx_embeddings_agent ON agent_context_embeddings(agent_id, created_at DESC);
CREATE INDEX idx_embeddings_type ON agent_context_embeddings(content_type, agent_id);

-- ============================================
-- Create triggers and functions
-- ============================================

-- Event sequence trigger
CREATE OR REPLACE FUNCTION assign_event_sequence()
RETURNS TRIGGER AS $$
BEGIN
    NEW.sequence_number := nextval('event_sequence_seq');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER event_sequence_trigger
    BEFORE INSERT ON events
    FOR EACH ROW
    EXECUTE FUNCTION assign_event_sequence();

-- Dependency cycle detection
CREATE OR REPLACE FUNCTION check_dependency_cycle()
RETURNS TRIGGER AS $$
DECLARE
    cycle_exists BOOLEAN;
BEGIN
    WITH RECURSIVE dep_chain AS (
        SELECT depends_on_id AS task_id, 1 AS depth
        FROM task_dependencies
        WHERE task_id = NEW.depends_on_id
        UNION ALL
        SELECT td.depends_on_id, dc.depth + 1
        FROM task_dependencies td
        JOIN dep_chain dc ON td.task_id = dc.task_id
        WHERE dc.depth < 100
    )
    SELECT EXISTS (
        SELECT 1 FROM dep_chain WHERE task_id = NEW.task_id
    ) INTO cycle_exists;

    IF cycle_exists THEN
        RAISE EXCEPTION 'Dependency cycle detected: task % cannot depend on task %',
            NEW.task_id, NEW.depends_on_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER prevent_dependency_cycle
    BEFORE INSERT ON task_dependencies
    FOR EACH ROW
    EXECUTE FUNCTION check_dependency_cycle();

-- Cache invalidation notification
CREATE OR REPLACE FUNCTION notify_cache_invalidation()
RETURNS TRIGGER AS $$
DECLARE
    payload JSONB;
BEGIN
    payload := jsonb_build_object(
        'table', TG_TABLE_NAME,
        'operation', TG_OP,
        'id', COALESCE(NEW.id, OLD.id)
    );

    IF TG_TABLE_NAME = 'tasks' THEN
        payload := payload || jsonb_build_object(
            'dag_id', COALESCE(NEW.dag_id, OLD.dag_id),
            'status', NEW.status
        );
    ELSIF TG_TABLE_NAME = 'agents' THEN
        payload := payload || jsonb_build_object(
            'status', NEW.status,
            'current_load', NEW.current_load
        );
    END IF;

    PERFORM pg_notify('cache_invalidation', payload::TEXT);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER tasks_cache_invalidation
    AFTER INSERT OR UPDATE OR DELETE ON tasks
    FOR EACH ROW EXECUTE FUNCTION notify_cache_invalidation();

CREATE TRIGGER agents_cache_invalidation
    AFTER INSERT OR UPDATE OR DELETE ON agents
    FOR EACH ROW EXECUTE FUNCTION notify_cache_invalidation();

-- Grant schema completed
SELECT 'Schema creation completed successfully' AS status;
```

---

## Appendix B: Performance Tuning

### B.1 PostgreSQL Configuration

```ini
# Memory settings
shared_buffers = 4GB                # 25% of RAM
effective_cache_size = 12GB         # 75% of RAM
work_mem = 64MB                     # Per-operation memory
maintenance_work_mem = 1GB          # For VACUUM, CREATE INDEX

# Write performance
wal_buffers = 64MB
checkpoint_timeout = 15min
max_wal_size = 8GB

# Query planner
random_page_cost = 1.1              # SSD storage
effective_io_concurrency = 200      # SSD storage
default_statistics_target = 200     # More accurate plans

# Parallelism
max_parallel_workers_per_gather = 4
max_parallel_workers = 8
max_parallel_maintenance_workers = 4
parallel_tuple_cost = 0.01
parallel_setup_cost = 100

# Connection pooling (with PgBouncer)
max_connections = 200
```

### B.2 Query Optimization Guidelines

1. **Use covering indexes** for frequently accessed columns
2. **Partition events table** by month for large deployments
3. **Archive old events** to cold storage after 90 days
4. **Use connection pooling** (PgBouncer) for high concurrency
5. **Monitor slow queries** with `pg_stat_statements`

---

*Document generated for Project Apex - AI Agent Orchestration System*
*Version 1.0.0 | Last Updated: 2024-01-15*
