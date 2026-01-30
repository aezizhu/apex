-- ═══════════════════════════════════════════════════════════════════════════════
-- Project Apex - Initial Database Schema
-- Migration: 20240101000000_initial_schema.sql
-- Description: Creates all core tables with proper constraints and relationships
-- ═══════════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════════
-- EXTENSIONS
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";      -- UUID generation
CREATE EXTENSION IF NOT EXISTS "pgcrypto";       -- Cryptographic functions
CREATE EXTENSION IF NOT EXISTS "pg_trgm";        -- Trigram similarity for text search

-- ═══════════════════════════════════════════════════════════════════════════════
-- CUSTOM TYPES (ENUMS)
-- ═══════════════════════════════════════════════════════════════════════════════

-- Task lifecycle states
COMMENT ON TYPE task_status IS 'Represents the lifecycle state of a task';
CREATE TYPE task_status AS ENUM (
    'pending',      -- Task created but not yet ready to execute
    'ready',        -- All dependencies satisfied, ready for assignment
    'assigned',     -- Assigned to an agent but not started
    'running',      -- Currently being executed
    'paused',       -- Temporarily paused
    'completed',    -- Successfully completed
    'failed',       -- Failed with error
    'cancelled',    -- Cancelled by user or system
    'timeout'       -- Exceeded time limit
);

-- Agent operational states
CREATE TYPE agent_status AS ENUM (
    'idle',         -- Available for work
    'busy',         -- Currently processing tasks
    'overloaded',   -- At or near capacity
    'error',        -- In error state
    'paused',       -- Administratively paused
    'offline'       -- Not available
);

-- DAG execution states
CREATE TYPE dag_status AS ENUM (
    'pending',      -- Created but not started
    'running',      -- Currently executing
    'paused',       -- Temporarily paused
    'completed',    -- All tasks completed successfully
    'failed',       -- One or more tasks failed
    'cancelled'     -- Cancelled by user
);

-- Approval workflow states
CREATE TYPE approval_status AS ENUM (
    'pending',      -- Awaiting decision
    'approved',     -- Approved by approver
    'denied',       -- Denied by approver
    'expired',      -- Approval request expired
    'auto_approved' -- Automatically approved by policy
);

-- Approval action types
CREATE TYPE approval_action AS ENUM (
    'tool_execution',   -- Execute a tool/function
    'resource_access',  -- Access external resource
    'cost_threshold',   -- Exceeded cost threshold
    'data_mutation',    -- Modify data
    'external_api',     -- Call external API
    'file_operation',   -- File system operation
    'network_request',  -- Network request
    'escalation'        -- Task escalation
);

-- Audit log action types
CREATE TYPE audit_action AS ENUM (
    'create',
    'update',
    'delete',
    'read',
    'execute',
    'approve',
    'deny',
    'assign',
    'complete',
    'fail',
    'cancel',
    'retry'
);

-- Contract status
CREATE TYPE contract_status AS ENUM (
    'active',       -- Contract is active
    'completed',    -- Contract completed within limits
    'exceeded',     -- Contract limits exceeded
    'cancelled',    -- Contract cancelled
    'expired'       -- Contract time limit expired
);

-- ═══════════════════════════════════════════════════════════════════════════════
-- CORE TABLES
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Table: agents
-- Description: AI agents that can execute tasks
-- ---------------------------------------------------------------------------
CREATE TABLE agents (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Identity
    name VARCHAR(255) NOT NULL,
    description TEXT,
    model VARCHAR(100) NOT NULL,
    model_version VARCHAR(50),

    -- Configuration
    system_prompt TEXT,
    tools JSONB DEFAULT '[]'::jsonb,
    capabilities JSONB DEFAULT '[]'::jsonb,
    config JSONB DEFAULT '{}'::jsonb,

    -- Status and capacity
    status agent_status NOT NULL DEFAULT 'idle',
    current_load INTEGER NOT NULL DEFAULT 0,
    max_load INTEGER NOT NULL DEFAULT 10,

    -- Performance metrics
    success_count BIGINT NOT NULL DEFAULT 0,
    failure_count BIGINT NOT NULL DEFAULT 0,
    total_tasks_completed BIGINT NOT NULL DEFAULT 0,
    total_tokens_used BIGINT NOT NULL DEFAULT 0,
    total_cost DECIMAL(12, 6) NOT NULL DEFAULT 0,
    avg_task_duration_ms INTEGER,

    -- Reputation and scoring
    reputation_score DECIMAL(5, 4) NOT NULL DEFAULT 1.0,
    reliability_score DECIMAL(5, 4) NOT NULL DEFAULT 1.0,

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,
    tags VARCHAR(100)[] DEFAULT ARRAY[]::VARCHAR(100)[],

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ,
    last_error_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT agents_name_unique UNIQUE (name),
    CONSTRAINT agents_reputation_range CHECK (reputation_score >= 0 AND reputation_score <= 1),
    CONSTRAINT agents_reliability_range CHECK (reliability_score >= 0 AND reliability_score <= 1),
    CONSTRAINT agents_load_valid CHECK (current_load >= 0 AND current_load <= max_load),
    CONSTRAINT agents_max_load_positive CHECK (max_load > 0)
);

COMMENT ON TABLE agents IS 'AI agents capable of executing tasks within the system';
COMMENT ON COLUMN agents.model IS 'The LLM model identifier (e.g., claude-3-opus, gpt-4)';
COMMENT ON COLUMN agents.system_prompt IS 'The system prompt used to configure the agent behavior';
COMMENT ON COLUMN agents.tools IS 'JSON array of available tools/functions for this agent';
COMMENT ON COLUMN agents.reputation_score IS 'Calculated reputation score based on task outcomes (0-1)';
COMMENT ON COLUMN agents.current_load IS 'Number of currently assigned tasks';
COMMENT ON COLUMN agents.max_load IS 'Maximum concurrent tasks this agent can handle';

-- ---------------------------------------------------------------------------
-- Table: dags
-- Description: Directed Acyclic Graphs representing task workflows
-- ---------------------------------------------------------------------------
CREATE TABLE dags (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Identity
    name VARCHAR(255) NOT NULL,
    description TEXT,
    version INTEGER NOT NULL DEFAULT 1,

    -- Status
    status dag_status NOT NULL DEFAULT 'pending',

    -- Configuration
    config JSONB DEFAULT '{}'::jsonb,
    timeout_seconds INTEGER,
    max_retries INTEGER DEFAULT 3,
    retry_delay_seconds INTEGER DEFAULT 60,

    -- Execution tracking
    total_tasks INTEGER NOT NULL DEFAULT 0,
    completed_tasks INTEGER NOT NULL DEFAULT 0,
    failed_tasks INTEGER NOT NULL DEFAULT 0,

    -- Resource tracking
    total_tokens_used BIGINT NOT NULL DEFAULT 0,
    total_cost DECIMAL(12, 6) NOT NULL DEFAULT 0,

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,
    tags VARCHAR(100)[] DEFAULT ARRAY[]::VARCHAR(100)[],

    -- User context
    created_by VARCHAR(255),

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT dags_version_positive CHECK (version > 0),
    CONSTRAINT dags_tasks_non_negative CHECK (
        total_tasks >= 0 AND
        completed_tasks >= 0 AND
        failed_tasks >= 0
    )
);

COMMENT ON TABLE dags IS 'Directed Acyclic Graphs representing task execution workflows';
COMMENT ON COLUMN dags.status IS 'Current execution status of the DAG';
COMMENT ON COLUMN dags.total_tasks IS 'Total number of tasks in this DAG';

-- ---------------------------------------------------------------------------
-- Table: tasks
-- Description: Individual units of work to be executed by agents
-- ---------------------------------------------------------------------------
CREATE TABLE tasks (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Relationships
    dag_id UUID NOT NULL REFERENCES dags(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,

    -- Identity
    name VARCHAR(255) NOT NULL,
    description TEXT,
    instruction TEXT NOT NULL,

    -- Status
    status task_status NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 0,

    -- Input/Output
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    output JSONB,
    context JSONB DEFAULT '{}'::jsonb,

    -- Error handling
    error TEXT,
    error_code VARCHAR(50),
    error_details JSONB,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,

    -- Resource tracking
    tokens_used BIGINT NOT NULL DEFAULT 0,
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,

    -- Execution timing
    timeout_seconds INTEGER,
    estimated_duration_ms INTEGER,
    actual_duration_ms INTEGER,

    -- Tracing
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    parent_span_id VARCHAR(16),

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,
    tags VARCHAR(100)[] DEFAULT ARRAY[]::VARCHAR(100)[],

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scheduled_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT tasks_retry_valid CHECK (retry_count >= 0 AND retry_count <= max_retries),
    CONSTRAINT tasks_tokens_non_negative CHECK (
        tokens_used >= 0 AND
        prompt_tokens >= 0 AND
        completion_tokens >= 0
    ),
    CONSTRAINT tasks_cost_non_negative CHECK (cost_dollars >= 0)
);

COMMENT ON TABLE tasks IS 'Individual units of work executed by agents within a DAG';
COMMENT ON COLUMN tasks.instruction IS 'The specific instruction for the agent to execute';
COMMENT ON COLUMN tasks.priority IS 'Task priority (higher values = higher priority)';
COMMENT ON COLUMN tasks.trace_id IS 'Distributed tracing ID for observability';

-- ---------------------------------------------------------------------------
-- Table: dag_nodes
-- Description: Nodes in the DAG with dependency information
-- ---------------------------------------------------------------------------
CREATE TABLE dag_nodes (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Relationships
    dag_id UUID NOT NULL REFERENCES dags(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

    -- Node position in DAG
    node_order INTEGER NOT NULL DEFAULT 0,
    depth_level INTEGER NOT NULL DEFAULT 0,

    -- Dependencies (stored as JSON array of task IDs)
    dependencies UUID[] DEFAULT ARRAY[]::UUID[],
    dependents UUID[] DEFAULT ARRAY[]::UUID[],

    -- Execution tracking
    is_entry_point BOOLEAN NOT NULL DEFAULT FALSE,
    is_exit_point BOOLEAN NOT NULL DEFAULT FALSE,

    -- Conditional execution
    condition_expression TEXT,
    skip_on_failure BOOLEAN NOT NULL DEFAULT FALSE,

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT dag_nodes_unique_task UNIQUE (dag_id, task_id),
    CONSTRAINT dag_nodes_order_non_negative CHECK (node_order >= 0),
    CONSTRAINT dag_nodes_depth_non_negative CHECK (depth_level >= 0)
);

COMMENT ON TABLE dag_nodes IS 'Represents nodes in the DAG with their dependency relationships';
COMMENT ON COLUMN dag_nodes.dependencies IS 'Array of task IDs that must complete before this node';
COMMENT ON COLUMN dag_nodes.depth_level IS 'The depth level in the DAG (0 = root nodes)';

-- ---------------------------------------------------------------------------
-- Table: task_dependencies
-- Description: Many-to-many relationship for task dependencies
-- ---------------------------------------------------------------------------
CREATE TABLE task_dependencies (
    -- Composite primary key
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

    -- Dependency metadata
    dependency_type VARCHAR(50) NOT NULL DEFAULT 'completion',
    is_required BOOLEAN NOT NULL DEFAULT TRUE,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (task_id, depends_on_id),

    -- Constraints
    CONSTRAINT task_deps_no_self_dependency CHECK (task_id != depends_on_id)
);

COMMENT ON TABLE task_dependencies IS 'Defines dependency relationships between tasks';
COMMENT ON COLUMN task_dependencies.dependency_type IS 'Type of dependency (completion, data, resource)';

-- ---------------------------------------------------------------------------
-- Table: approvals
-- Description: Approval requests for sensitive operations
-- ---------------------------------------------------------------------------
CREATE TABLE approvals (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Relationships
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    -- Request details
    action approval_action NOT NULL,
    action_description TEXT,
    action_data JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Risk assessment
    risk_score DECIMAL(3, 2),
    risk_factors JSONB DEFAULT '[]'::jsonb,

    -- Clustering (for batch approvals)
    cluster_id UUID,

    -- Status
    status approval_status NOT NULL DEFAULT 'pending',

    -- Decision details
    decided_by VARCHAR(255),
    decision_reason TEXT,
    decided_at TIMESTAMPTZ,

    -- Expiration
    expires_at TIMESTAMPTZ,

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT approvals_risk_score_range CHECK (risk_score IS NULL OR (risk_score >= 0 AND risk_score <= 1))
);

COMMENT ON TABLE approvals IS 'Approval requests for operations requiring human oversight';
COMMENT ON COLUMN approvals.action IS 'The type of action requiring approval';
COMMENT ON COLUMN approvals.risk_score IS 'Calculated risk score for this action (0-1)';
COMMENT ON COLUMN approvals.cluster_id IS 'ID for grouping similar approvals for batch processing';

-- ---------------------------------------------------------------------------
-- Table: usage_records
-- Description: Detailed usage tracking for billing and analytics
-- ---------------------------------------------------------------------------
CREATE TABLE usage_records (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Relationships
    task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
    dag_id UUID REFERENCES dags(id) ON DELETE SET NULL,

    -- Model information
    model VARCHAR(100) NOT NULL,
    model_version VARCHAR(50),
    provider VARCHAR(100),

    -- Token usage
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    cached_tokens BIGINT NOT NULL DEFAULT 0,

    -- Cost
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,
    cost_breakdown JSONB DEFAULT '{}'::jsonb,

    -- Request details
    request_type VARCHAR(50),
    endpoint VARCHAR(255),
    latency_ms INTEGER,

    -- Response details
    response_status VARCHAR(50),
    error_type VARCHAR(100),

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT usage_tokens_non_negative CHECK (
        prompt_tokens >= 0 AND
        completion_tokens >= 0 AND
        total_tokens >= 0 AND
        cached_tokens >= 0
    ),
    CONSTRAINT usage_cost_non_negative CHECK (cost_dollars >= 0)
);

COMMENT ON TABLE usage_records IS 'Detailed tracking of API usage for billing and analytics';
COMMENT ON COLUMN usage_records.cost_breakdown IS 'Detailed breakdown of costs by category';
COMMENT ON COLUMN usage_records.cached_tokens IS 'Number of tokens served from cache';

-- ---------------------------------------------------------------------------
-- Table: audit_logs
-- Description: Comprehensive audit trail for all system actions
-- ---------------------------------------------------------------------------
CREATE TABLE audit_logs (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Entity reference
    entity_type VARCHAR(100) NOT NULL,
    entity_id UUID NOT NULL,

    -- Action details
    action audit_action NOT NULL,
    action_description TEXT,

    -- Actor information
    actor_type VARCHAR(50) NOT NULL,  -- 'user', 'agent', 'system'
    actor_id VARCHAR(255),
    actor_name VARCHAR(255),

    -- Change tracking
    old_values JSONB,
    new_values JSONB,
    changed_fields VARCHAR(255)[],

    -- Context
    ip_address INET,
    user_agent TEXT,
    request_id VARCHAR(64),

    -- Tracing
    trace_id VARCHAR(32),
    span_id VARCHAR(16),

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Partitioning support
    partition_key DATE NOT NULL DEFAULT CURRENT_DATE
);

COMMENT ON TABLE audit_logs IS 'Immutable audit trail of all system actions';
COMMENT ON COLUMN audit_logs.old_values IS 'Previous values before the change';
COMMENT ON COLUMN audit_logs.new_values IS 'New values after the change';
COMMENT ON COLUMN audit_logs.partition_key IS 'Date key for table partitioning';

-- ---------------------------------------------------------------------------
-- Table: agent_contracts
-- Description: Resource contracts limiting agent capabilities
-- ---------------------------------------------------------------------------
CREATE TABLE agent_contracts (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Relationships
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    parent_contract_id UUID REFERENCES agent_contracts(id) ON DELETE SET NULL,

    -- Limits
    token_limit BIGINT NOT NULL,
    cost_limit DECIMAL(10, 6) NOT NULL,
    time_limit_seconds BIGINT NOT NULL,
    api_call_limit BIGINT NOT NULL,
    tool_call_limit BIGINT,

    -- Current usage
    tokens_used BIGINT NOT NULL DEFAULT 0,
    cost_used DECIMAL(10, 6) NOT NULL DEFAULT 0,
    time_used_seconds BIGINT NOT NULL DEFAULT 0,
    api_calls_used BIGINT NOT NULL DEFAULT 0,
    tool_calls_used BIGINT NOT NULL DEFAULT 0,

    -- Status
    status contract_status NOT NULL DEFAULT 'active',

    -- Permissions
    allowed_tools VARCHAR(255)[] DEFAULT ARRAY[]::VARCHAR(255)[],
    denied_tools VARCHAR(255)[] DEFAULT ARRAY[]::VARCHAR(255)[],
    allowed_resources JSONB DEFAULT '[]'::jsonb,

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,

    -- Constraints
    CONSTRAINT contracts_limits_positive CHECK (
        token_limit > 0 AND
        cost_limit > 0 AND
        time_limit_seconds > 0 AND
        api_call_limit > 0
    ),
    CONSTRAINT contracts_usage_non_negative CHECK (
        tokens_used >= 0 AND
        cost_used >= 0 AND
        time_used_seconds >= 0 AND
        api_calls_used >= 0 AND
        tool_calls_used >= 0
    )
);

COMMENT ON TABLE agent_contracts IS 'Resource contracts that limit agent capabilities during task execution';
COMMENT ON COLUMN agent_contracts.token_limit IS 'Maximum tokens the agent can use for this task';
COMMENT ON COLUMN agent_contracts.allowed_tools IS 'List of tool names the agent is allowed to use';

-- ---------------------------------------------------------------------------
-- Table: tool_calls
-- Description: Record of all tool/function calls made by agents
-- ---------------------------------------------------------------------------
CREATE TABLE tool_calls (
    -- Primary key
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),

    -- Relationships
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    contract_id UUID REFERENCES agent_contracts(id) ON DELETE SET NULL,

    -- Tool information
    tool_name VARCHAR(255) NOT NULL,
    tool_version VARCHAR(50),

    -- Call details
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB,
    error TEXT,
    error_code VARCHAR(50),

    -- Status
    success BOOLEAN,

    -- Resource usage
    tokens_used BIGINT NOT NULL DEFAULT 0,
    cost_dollars DECIMAL(10, 6) NOT NULL DEFAULT 0,
    latency_ms INTEGER,

    -- Tracing
    trace_id VARCHAR(32),
    span_id VARCHAR(16),
    parent_span_id VARCHAR(16),

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT tool_calls_tokens_non_negative CHECK (tokens_used >= 0),
    CONSTRAINT tool_calls_cost_non_negative CHECK (cost_dollars >= 0),
    CONSTRAINT tool_calls_latency_non_negative CHECK (latency_ms IS NULL OR latency_ms >= 0)
);

COMMENT ON TABLE tool_calls IS 'Detailed log of all tool/function calls made by agents';
COMMENT ON COLUMN tool_calls.parameters IS 'The parameters passed to the tool';
COMMENT ON COLUMN tool_calls.result IS 'The result returned by the tool';

-- ---------------------------------------------------------------------------
-- Table: events
-- Description: Event sourcing table for system events
-- ---------------------------------------------------------------------------
CREATE TABLE events (
    -- Primary key (serial for ordering guarantee)
    id BIGSERIAL PRIMARY KEY,

    -- Event identity
    event_id UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4(),

    -- Aggregate reference
    aggregate_type VARCHAR(50) NOT NULL,
    aggregate_id UUID NOT NULL,

    -- Event details
    event_type VARCHAR(100) NOT NULL,
    event_data JSONB NOT NULL DEFAULT '{}'::jsonb,

    -- Versioning for optimistic concurrency
    version INTEGER NOT NULL,

    -- Tracing
    trace_id VARCHAR(32),
    span_id VARCHAR(16),

    -- Metadata
    metadata JSONB DEFAULT '{}'::jsonb,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT events_version_positive CHECK (version > 0),
    CONSTRAINT events_unique_version UNIQUE (aggregate_type, aggregate_id, version)
);

COMMENT ON TABLE events IS 'Event sourcing table capturing all state changes';
COMMENT ON COLUMN events.aggregate_type IS 'Type of aggregate (task, agent, dag, etc.)';
COMMENT ON COLUMN events.version IS 'Sequential version number for optimistic concurrency';

-- ---------------------------------------------------------------------------
-- Table: system_config
-- Description: System-wide configuration settings
-- ---------------------------------------------------------------------------
CREATE TABLE system_config (
    -- Primary key
    key VARCHAR(255) PRIMARY KEY,

    -- Value
    value JSONB NOT NULL,

    -- Metadata
    description TEXT,
    category VARCHAR(100),
    is_sensitive BOOLEAN NOT NULL DEFAULT FALSE,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by VARCHAR(255)
);

COMMENT ON TABLE system_config IS 'System-wide configuration key-value store';
COMMENT ON COLUMN system_config.is_sensitive IS 'Whether this config contains sensitive data';
