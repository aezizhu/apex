-- ═══════════════════════════════════════════════════════════════════════════════
-- Project Apex - Performance Indexes
-- Migration: 20240101000001_indexes.sql
-- Description: Creates performance indexes for all tables
-- ═══════════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════════
-- AGENTS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Status lookup (frequently queried)
CREATE INDEX idx_agents_status ON agents(status);

-- Model-based queries
CREATE INDEX idx_agents_model ON agents(model);
CREATE INDEX idx_agents_model_status ON agents(model, status);

-- Available agents query (idle with capacity)
CREATE INDEX idx_agents_available ON agents(status, current_load, max_load)
WHERE status IN ('idle', 'busy');

-- Performance-based selection
CREATE INDEX idx_agents_reputation ON agents(reputation_score DESC);
CREATE INDEX idx_agents_reliability ON agents(reliability_score DESC);

-- Agent lookup with capacity info
CREATE INDEX idx_agents_capacity ON agents(current_load, max_load)
WHERE status != 'offline';

-- Last active for health monitoring
CREATE INDEX idx_agents_last_active ON agents(last_active_at DESC NULLS LAST);

-- Tags for categorization
CREATE INDEX idx_agents_tags ON agents USING GIN (tags);

-- Text search on name and description
CREATE INDEX idx_agents_name_trgm ON agents USING GIN (name gin_trgm_ops);

-- ═══════════════════════════════════════════════════════════════════════════════
-- DAGS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Status lookup
CREATE INDEX idx_dags_status ON dags(status);

-- Created by user
CREATE INDEX idx_dags_created_by ON dags(created_by)
WHERE created_by IS NOT NULL;

-- Active DAGs
CREATE INDEX idx_dags_active ON dags(status, started_at)
WHERE status IN ('pending', 'running', 'paused');

-- Completed DAGs for analytics
CREATE INDEX idx_dags_completed ON dags(completed_at DESC)
WHERE status = 'completed';

-- Time-based queries
CREATE INDEX idx_dags_created_at ON dags(created_at DESC);
CREATE INDEX idx_dags_started_at ON dags(started_at DESC)
WHERE started_at IS NOT NULL;

-- Tags for filtering
CREATE INDEX idx_dags_tags ON dags USING GIN (tags);

-- Metadata queries
CREATE INDEX idx_dags_metadata ON dags USING GIN (metadata jsonb_path_ops);

-- Text search
CREATE INDEX idx_dags_name_trgm ON dags USING GIN (name gin_trgm_ops);

-- ═══════════════════════════════════════════════════════════════════════════════
-- TASKS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Foreign key lookups (essential for joins)
CREATE INDEX idx_tasks_dag_id ON tasks(dag_id);
CREATE INDEX idx_tasks_parent_id ON tasks(parent_id)
WHERE parent_id IS NOT NULL;
CREATE INDEX idx_tasks_agent_id ON tasks(agent_id)
WHERE agent_id IS NOT NULL;

-- Status queries
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_dag_status ON tasks(dag_id, status);

-- Priority queue for task scheduling
CREATE INDEX idx_tasks_queue ON tasks(priority DESC, created_at ASC)
WHERE status = 'ready';

-- Pending tasks per DAG
CREATE INDEX idx_tasks_pending ON tasks(dag_id, created_at)
WHERE status = 'pending';

-- Running tasks for monitoring
CREATE INDEX idx_tasks_running ON tasks(started_at)
WHERE status = 'running';

-- Failed tasks for retry processing
CREATE INDEX idx_tasks_failed ON tasks(dag_id, retry_count)
WHERE status = 'failed' AND retry_count < max_retries;

-- Agent workload
CREATE INDEX idx_tasks_agent_running ON tasks(agent_id)
WHERE status IN ('assigned', 'running');

-- Time-based queries
CREATE INDEX idx_tasks_created_at ON tasks(created_at DESC);
CREATE INDEX idx_tasks_completed_at ON tasks(completed_at DESC)
WHERE completed_at IS NOT NULL;

-- Scheduled tasks
CREATE INDEX idx_tasks_scheduled ON tasks(scheduled_at)
WHERE scheduled_at IS NOT NULL AND status = 'pending';

-- Tracing
CREATE INDEX idx_tasks_trace_id ON tasks(trace_id)
WHERE trace_id IS NOT NULL;

-- Tags
CREATE INDEX idx_tasks_tags ON tasks USING GIN (tags);

-- JSONB indexes for flexible queries
CREATE INDEX idx_tasks_input ON tasks USING GIN (input jsonb_path_ops);
CREATE INDEX idx_tasks_output ON tasks USING GIN (output jsonb_path_ops)
WHERE output IS NOT NULL;
CREATE INDEX idx_tasks_metadata ON tasks USING GIN (metadata jsonb_path_ops);

-- Composite index for common query pattern
CREATE INDEX idx_tasks_dag_agent_status ON tasks(dag_id, agent_id, status);

-- ═══════════════════════════════════════════════════════════════════════════════
-- DAG_NODES INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- DAG lookup
CREATE INDEX idx_dag_nodes_dag_id ON dag_nodes(dag_id);

-- Task lookup
CREATE INDEX idx_dag_nodes_task_id ON dag_nodes(task_id);

-- Entry points for DAG initialization
CREATE INDEX idx_dag_nodes_entry ON dag_nodes(dag_id)
WHERE is_entry_point = TRUE;

-- Exit points for completion detection
CREATE INDEX idx_dag_nodes_exit ON dag_nodes(dag_id)
WHERE is_exit_point = TRUE;

-- Dependency array lookups
CREATE INDEX idx_dag_nodes_dependencies ON dag_nodes USING GIN (dependencies);
CREATE INDEX idx_dag_nodes_dependents ON dag_nodes USING GIN (dependents);

-- Order for execution sequence
CREATE INDEX idx_dag_nodes_order ON dag_nodes(dag_id, node_order, depth_level);

-- ═══════════════════════════════════════════════════════════════════════════════
-- TASK_DEPENDENCIES INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Reverse lookup (what tasks depend on this one)
CREATE INDEX idx_task_deps_depends_on ON task_dependencies(depends_on_id);

-- Required dependencies
CREATE INDEX idx_task_deps_required ON task_dependencies(task_id)
WHERE is_required = TRUE;

-- ═══════════════════════════════════════════════════════════════════════════════
-- APPROVALS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Foreign keys
CREATE INDEX idx_approvals_task_id ON approvals(task_id);
CREATE INDEX idx_approvals_agent_id ON approvals(agent_id);

-- Status queries
CREATE INDEX idx_approvals_status ON approvals(status);

-- Pending approvals (hot path)
CREATE INDEX idx_approvals_pending ON approvals(created_at ASC)
WHERE status = 'pending';

-- Pending with expiration check
CREATE INDEX idx_approvals_pending_expires ON approvals(expires_at)
WHERE status = 'pending' AND expires_at IS NOT NULL;

-- Cluster-based batch processing
CREATE INDEX idx_approvals_cluster ON approvals(cluster_id, status)
WHERE cluster_id IS NOT NULL;

-- Action type queries
CREATE INDEX idx_approvals_action ON approvals(action);
CREATE INDEX idx_approvals_action_pending ON approvals(action, created_at)
WHERE status = 'pending';

-- Risk-based sorting
CREATE INDEX idx_approvals_risk ON approvals(risk_score DESC NULLS LAST)
WHERE status = 'pending';

-- Decision history
CREATE INDEX idx_approvals_decided ON approvals(decided_at DESC)
WHERE status IN ('approved', 'denied');

-- Decided by user
CREATE INDEX idx_approvals_decided_by ON approvals(decided_by)
WHERE decided_by IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════════════════
-- USAGE_RECORDS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Foreign keys
CREATE INDEX idx_usage_task_id ON usage_records(task_id)
WHERE task_id IS NOT NULL;
CREATE INDEX idx_usage_agent_id ON usage_records(agent_id)
WHERE agent_id IS NOT NULL;
CREATE INDEX idx_usage_dag_id ON usage_records(dag_id)
WHERE dag_id IS NOT NULL;

-- Model analytics
CREATE INDEX idx_usage_model ON usage_records(model);
CREATE INDEX idx_usage_model_provider ON usage_records(model, provider);

-- Time-based analytics
CREATE INDEX idx_usage_created_at ON usage_records(created_at DESC);

-- Daily aggregation
CREATE INDEX idx_usage_daily ON usage_records(DATE(created_at));

-- Cost analysis
CREATE INDEX idx_usage_cost ON usage_records(cost_dollars DESC)
WHERE cost_dollars > 0;

-- High token usage
CREATE INDEX idx_usage_tokens ON usage_records(total_tokens DESC);

-- Request type analytics
CREATE INDEX idx_usage_request_type ON usage_records(request_type)
WHERE request_type IS NOT NULL;

-- Error tracking
CREATE INDEX idx_usage_errors ON usage_records(error_type, created_at)
WHERE error_type IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════════════════
-- AUDIT_LOGS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Entity queries (most common pattern)
CREATE INDEX idx_audit_entity ON audit_logs(entity_type, entity_id);

-- Action queries
CREATE INDEX idx_audit_action ON audit_logs(action);

-- Actor queries
CREATE INDEX idx_audit_actor ON audit_logs(actor_type, actor_id)
WHERE actor_id IS NOT NULL;

-- Time-based queries (most recent first)
CREATE INDEX idx_audit_created_at ON audit_logs(created_at DESC);

-- Partition key for potential partitioning
CREATE INDEX idx_audit_partition ON audit_logs(partition_key);

-- Combined time and entity
CREATE INDEX idx_audit_entity_time ON audit_logs(entity_type, entity_id, created_at DESC);

-- Tracing
CREATE INDEX idx_audit_trace ON audit_logs(trace_id)
WHERE trace_id IS NOT NULL;

-- Request correlation
CREATE INDEX idx_audit_request ON audit_logs(request_id)
WHERE request_id IS NOT NULL;

-- IP address for security queries
CREATE INDEX idx_audit_ip ON audit_logs(ip_address)
WHERE ip_address IS NOT NULL;

-- Change tracking
CREATE INDEX idx_audit_changed_fields ON audit_logs USING GIN (changed_fields)
WHERE changed_fields IS NOT NULL;

-- JSONB for flexible queries
CREATE INDEX idx_audit_old_values ON audit_logs USING GIN (old_values jsonb_path_ops)
WHERE old_values IS NOT NULL;
CREATE INDEX idx_audit_new_values ON audit_logs USING GIN (new_values jsonb_path_ops)
WHERE new_values IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════════════════
-- AGENT_CONTRACTS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Foreign keys
CREATE INDEX idx_contracts_agent ON agent_contracts(agent_id);
CREATE INDEX idx_contracts_task ON agent_contracts(task_id);
CREATE INDEX idx_contracts_parent ON agent_contracts(parent_contract_id)
WHERE parent_contract_id IS NOT NULL;

-- Status queries
CREATE INDEX idx_contracts_status ON agent_contracts(status);

-- Active contracts for an agent
CREATE INDEX idx_contracts_agent_active ON agent_contracts(agent_id, task_id)
WHERE status = 'active';

-- Expiring contracts for cleanup
CREATE INDEX idx_contracts_expires ON agent_contracts(expires_at)
WHERE status = 'active';

-- Usage monitoring (near limits)
CREATE INDEX idx_contracts_usage ON agent_contracts(
    (tokens_used::float / NULLIF(token_limit, 0)),
    (cost_used::float / NULLIF(cost_limit, 0))
)
WHERE status = 'active';

-- Tool permissions
CREATE INDEX idx_contracts_allowed_tools ON agent_contracts USING GIN (allowed_tools);
CREATE INDEX idx_contracts_denied_tools ON agent_contracts USING GIN (denied_tools);

-- ═══════════════════════════════════════════════════════════════════════════════
-- TOOL_CALLS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Foreign keys
CREATE INDEX idx_tool_calls_task ON tool_calls(task_id);
CREATE INDEX idx_tool_calls_agent ON tool_calls(agent_id);
CREATE INDEX idx_tool_calls_contract ON tool_calls(contract_id)
WHERE contract_id IS NOT NULL;

-- Tool usage analytics
CREATE INDEX idx_tool_calls_tool_name ON tool_calls(tool_name);
CREATE INDEX idx_tool_calls_tool_agent ON tool_calls(tool_name, agent_id);

-- Time-based queries
CREATE INDEX idx_tool_calls_created ON tool_calls(created_at DESC);

-- Successful vs failed calls
CREATE INDEX idx_tool_calls_success ON tool_calls(success, tool_name)
WHERE success IS NOT NULL;

-- Error analysis
CREATE INDEX idx_tool_calls_errors ON tool_calls(tool_name, error_code)
WHERE error_code IS NOT NULL;

-- Latency analysis
CREATE INDEX idx_tool_calls_latency ON tool_calls(tool_name, latency_ms)
WHERE latency_ms IS NOT NULL;

-- Tracing
CREATE INDEX idx_tool_calls_trace ON tool_calls(trace_id)
WHERE trace_id IS NOT NULL;

-- High cost calls
CREATE INDEX idx_tool_calls_cost ON tool_calls(cost_dollars DESC)
WHERE cost_dollars > 0;

-- JSONB parameters for query flexibility
CREATE INDEX idx_tool_calls_params ON tool_calls USING GIN (parameters jsonb_path_ops);

-- ═══════════════════════════════════════════════════════════════════════════════
-- EVENTS INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Aggregate lookup (most common pattern)
CREATE INDEX idx_events_aggregate ON events(aggregate_type, aggregate_id);

-- Event type queries
CREATE INDEX idx_events_type ON events(event_type);

-- Aggregate with version for replay
CREATE INDEX idx_events_aggregate_version ON events(aggregate_type, aggregate_id, version);

-- Time-based queries
CREATE INDEX idx_events_created ON events(created_at DESC);

-- Tracing
CREATE INDEX idx_events_trace ON events(trace_id)
WHERE trace_id IS NOT NULL;

-- Event data queries
CREATE INDEX idx_events_data ON events USING GIN (event_data jsonb_path_ops);

-- Combined type and time for analytics
CREATE INDEX idx_events_type_time ON events(event_type, created_at DESC);

-- Recent events per aggregate
CREATE INDEX idx_events_aggregate_recent ON events(aggregate_type, aggregate_id, created_at DESC);

-- ═══════════════════════════════════════════════════════════════════════════════
-- SYSTEM_CONFIG INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Category lookup
CREATE INDEX idx_config_category ON system_config(category)
WHERE category IS NOT NULL;

-- Sensitive config (for masking)
CREATE INDEX idx_config_sensitive ON system_config(key)
WHERE is_sensitive = TRUE;

-- ═══════════════════════════════════════════════════════════════════════════════
-- COMPOSITE & SPECIALIZED INDEXES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Task scheduling optimization
-- Find ready tasks with best available agent
CREATE INDEX idx_task_scheduling ON tasks(dag_id, priority DESC, created_at ASC)
WHERE status = 'ready';

-- Agent selection optimization
-- Find best agent for a task based on model and availability
CREATE INDEX idx_agent_selection ON agents(model, reputation_score DESC, current_load)
WHERE status IN ('idle', 'busy') AND current_load < max_load;

-- DAG completion tracking
CREATE INDEX idx_dag_completion ON tasks(dag_id, status)
INCLUDE (tokens_used, cost_dollars);

-- Cost tracking composite
CREATE INDEX idx_cost_tracking ON usage_records(created_at, model, cost_dollars)
WHERE cost_dollars > 0;

-- ═══════════════════════════════════════════════════════════════════════════════
-- STATISTICS & MAINTENANCE
-- ═══════════════════════════════════════════════════════════════════════════════

-- Analyze all tables for query planning
ANALYZE agents;
ANALYZE dags;
ANALYZE tasks;
ANALYZE dag_nodes;
ANALYZE task_dependencies;
ANALYZE approvals;
ANALYZE usage_records;
ANALYZE audit_logs;
ANALYZE agent_contracts;
ANALYZE tool_calls;
ANALYZE events;
ANALYZE system_config;

-- Add comments for documentation
COMMENT ON INDEX idx_tasks_queue IS 'Priority queue for task scheduling - ready tasks ordered by priority and creation time';
COMMENT ON INDEX idx_agents_available IS 'Fast lookup for available agents with capacity';
COMMENT ON INDEX idx_approvals_pending IS 'Hot path for pending approval lookups';
COMMENT ON INDEX idx_events_aggregate_version IS 'Event sourcing replay - events ordered by version per aggregate';
