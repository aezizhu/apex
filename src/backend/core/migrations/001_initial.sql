-- Project Apex - Initial Database Schema
-- Version: 1.0.0

-- ═══════════════════════════════════════════════════════════════════════════════
-- EXTENSIONS
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ═══════════════════════════════════════════════════════════════════════════════
-- ENUMS
-- ═══════════════════════════════════════════════════════════════════════════════

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

-- ═══════════════════════════════════════════════════════════════════════════════
-- TABLES
-- ═══════════════════════════════════════════════════════════════════════════════

-- DAGs (Directed Acyclic Graphs of tasks)
CREATE TABLE dags (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

-- Agents
CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL,
    model VARCHAR(100) NOT NULL,
    system_prompt TEXT,
    tools JSONB DEFAULT '[]',
    status agent_status NOT NULL DEFAULT 'idle',
    current_load INTEGER NOT NULL DEFAULT 0,
    max_load INTEGER NOT NULL DEFAULT 10,
    success_count BIGINT NOT NULL DEFAULT 0,
    failure_count BIGINT NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    total_cost DECIMAL(12, 6) NOT NULL DEFAULT 0,
    reputation_score DECIMAL(5, 4) NOT NULL DEFAULT 1.0,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ,

    CONSTRAINT agents_reputation_range CHECK (reputation_score >= 0 AND reputation_score <= 1)
);

-- Tasks
CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    dag_id UUID NOT NULL REFERENCES dags(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES tasks(id),
    agent_id UUID REFERENCES agents(id),
    name VARCHAR(255) NOT NULL,
    status task_status NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,
    input JSONB NOT NULL,
    output JSONB,
    error TEXT,
    tokens_used BIGINT NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

-- Task Dependencies (for DAG structure)
CREATE TABLE task_dependencies (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on_id),

    CONSTRAINT no_self_dependency CHECK (task_id != depends_on_id)
);

-- Agent Contracts
CREATE TABLE agent_contracts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_id UUID NOT NULL REFERENCES agents(id),
    task_id UUID NOT NULL REFERENCES tasks(id),
    parent_contract_id UUID REFERENCES agent_contracts(id),

    -- Limits
    token_limit BIGINT NOT NULL,
    cost_limit DECIMAL(10, 6) NOT NULL,
    time_limit_seconds BIGINT NOT NULL,
    api_call_limit BIGINT NOT NULL,

    -- Usage tracking
    token_used BIGINT NOT NULL DEFAULT 0,
    cost_used DECIMAL(10, 6) NOT NULL DEFAULT 0,
    api_calls_used BIGINT NOT NULL DEFAULT 0,

    status contract_status NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

-- Tool Calls
CREATE TABLE tool_calls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    tool_name VARCHAR(255) NOT NULL,
    parameters JSONB,
    result JSONB,
    error TEXT,
    tokens_used BIGINT NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,
    latency_ms INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Events (Event Sourcing)
CREATE TABLE events (
    id BIGSERIAL PRIMARY KEY,
    event_id UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4(),
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    aggregate_type VARCHAR(50) NOT NULL,
    aggregate_id UUID NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    event_data JSONB NOT NULL,
    metadata JSONB DEFAULT '{}',
    version INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Approval Requests
CREATE TABLE approval_requests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id UUID NOT NULL REFERENCES tasks(id),
    agent_id UUID NOT NULL REFERENCES agents(id),
    action_type VARCHAR(100) NOT NULL,
    action_data JSONB NOT NULL,
    risk_score DECIMAL(3, 2),
    cluster_id UUID,
    status approval_status NOT NULL DEFAULT 'pending',
    decided_by VARCHAR(255),
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

-- ═══════════════════════════════════════════════════════════════════════════════
-- INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Tasks
CREATE INDEX idx_tasks_dag_id ON tasks(dag_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_agent_id ON tasks(agent_id);
CREATE INDEX idx_tasks_created_at ON tasks(created_at DESC);
CREATE INDEX idx_tasks_dag_status ON tasks(dag_id, status);
CREATE INDEX idx_tasks_pending ON tasks(dag_id) WHERE status = 'pending';
CREATE INDEX idx_tasks_running ON tasks(dag_id) WHERE status = 'running';

-- Agents
CREATE INDEX idx_agents_status ON agents(status);
CREATE INDEX idx_agents_model ON agents(model);
CREATE INDEX idx_agents_available ON agents(status, current_load) WHERE status = 'idle';

-- Contracts
CREATE INDEX idx_contracts_agent ON agent_contracts(agent_id);
CREATE INDEX idx_contracts_task ON agent_contracts(task_id);
CREATE INDEX idx_contracts_status ON agent_contracts(status);
CREATE INDEX idx_contracts_active ON agent_contracts(agent_id) WHERE status = 'active';

-- Tool Calls
CREATE INDEX idx_tool_calls_task ON tool_calls(task_id);
CREATE INDEX idx_tool_calls_agent ON tool_calls(agent_id);
CREATE INDEX idx_tool_calls_name ON tool_calls(tool_name);
CREATE INDEX idx_tool_calls_created ON tool_calls(created_at DESC);

-- Events
CREATE INDEX idx_events_aggregate ON events(aggregate_type, aggregate_id);
CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_trace ON events(trace_id);
CREATE INDEX idx_events_created ON events(created_at DESC);

-- Approvals
CREATE INDEX idx_approvals_status ON approval_requests(status);
CREATE INDEX idx_approvals_pending ON approval_requests(created_at) WHERE status = 'pending';
CREATE INDEX idx_approvals_cluster ON approval_requests(cluster_id) WHERE cluster_id IS NOT NULL;

-- GIN indexes for JSONB
CREATE INDEX idx_tasks_input_gin ON tasks USING GIN (input);
CREATE INDEX idx_events_data_gin ON events USING GIN (event_data);

-- ═══════════════════════════════════════════════════════════════════════════════
-- FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- Function to update agent's last_active_at
CREATE OR REPLACE FUNCTION update_agent_last_active()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE agents SET last_active_at = NOW() WHERE id = NEW.agent_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Function to update DAG status based on tasks
CREATE OR REPLACE FUNCTION update_dag_status()
RETURNS TRIGGER AS $$
DECLARE
    all_completed BOOLEAN;
    any_failed BOOLEAN;
    any_running BOOLEAN;
BEGIN
    SELECT
        BOOL_AND(status IN ('completed', 'cancelled')),
        BOOL_OR(status = 'failed'),
        BOOL_OR(status = 'running')
    INTO all_completed, any_failed, any_running
    FROM tasks WHERE dag_id = NEW.dag_id;

    IF any_failed THEN
        UPDATE dags SET status = 'failed', completed_at = NOW() WHERE id = NEW.dag_id;
    ELSIF all_completed THEN
        UPDATE dags SET status = 'completed', completed_at = NOW() WHERE id = NEW.dag_id;
    ELSIF any_running THEN
        UPDATE dags SET status = 'running', started_at = COALESCE(started_at, NOW()) WHERE id = NEW.dag_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- ═══════════════════════════════════════════════════════════════════════════════
-- TRIGGERS
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TRIGGER trg_update_agent_activity
AFTER INSERT ON tool_calls
FOR EACH ROW
EXECUTE FUNCTION update_agent_last_active();

CREATE TRIGGER trg_update_dag_on_task_change
AFTER UPDATE OF status ON tasks
FOR EACH ROW
EXECUTE FUNCTION update_dag_status();

-- ═══════════════════════════════════════════════════════════════════════════════
-- VIEWS
-- ═══════════════════════════════════════════════════════════════════════════════

-- Agent summary view
CREATE VIEW agent_summary AS
SELECT
    a.id,
    a.name,
    a.model,
    a.status,
    a.current_load,
    a.max_load,
    a.success_count,
    a.failure_count,
    CASE
        WHEN a.success_count + a.failure_count > 0
        THEN a.success_count::DECIMAL / (a.success_count + a.failure_count)
        ELSE 1.0
    END AS success_rate,
    a.total_tokens,
    a.total_cost,
    a.reputation_score,
    a.last_active_at,
    COUNT(t.id) FILTER (WHERE t.status = 'running') AS active_tasks
FROM agents a
LEFT JOIN tasks t ON a.id = t.agent_id
GROUP BY a.id;

-- DAG summary view
CREATE VIEW dag_summary AS
SELECT
    d.id,
    d.name,
    d.status,
    COUNT(t.id) AS total_tasks,
    COUNT(t.id) FILTER (WHERE t.status = 'completed') AS completed_tasks,
    COUNT(t.id) FILTER (WHERE t.status = 'failed') AS failed_tasks,
    COUNT(t.id) FILTER (WHERE t.status = 'running') AS running_tasks,
    SUM(t.tokens_used) AS total_tokens,
    SUM(t.cost_dollars) AS total_cost,
    d.created_at,
    d.started_at,
    d.completed_at,
    EXTRACT(EPOCH FROM (COALESCE(d.completed_at, NOW()) - d.started_at)) AS duration_seconds
FROM dags d
LEFT JOIN tasks t ON d.id = t.dag_id
GROUP BY d.id;

-- System stats view
CREATE VIEW system_stats AS
SELECT
    (SELECT COUNT(*) FROM tasks) AS total_tasks,
    (SELECT COUNT(*) FROM tasks WHERE status = 'completed') AS completed_tasks,
    (SELECT COUNT(*) FROM tasks WHERE status = 'failed') AS failed_tasks,
    (SELECT COUNT(*) FROM tasks WHERE status = 'running') AS running_tasks,
    (SELECT COUNT(*) FROM agents) AS total_agents,
    (SELECT COUNT(*) FROM agents WHERE status = 'idle') AS idle_agents,
    (SELECT COALESCE(SUM(tokens_used), 0) FROM tasks) AS total_tokens,
    (SELECT COALESCE(SUM(cost_dollars), 0) FROM tasks) AS total_cost;
