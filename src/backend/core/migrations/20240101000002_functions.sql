-- ═══════════════════════════════════════════════════════════════════════════════
-- Project Apex - Stored Procedures and Triggers
-- Migration: 20240101000002_functions.sql
-- Description: Database functions, procedures, and triggers
-- ═══════════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════════
-- UTILITY FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: update_updated_at
-- Description: Automatically updates the updated_at timestamp
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_updated_at() IS 'Trigger function to automatically update updated_at timestamp';

-- ---------------------------------------------------------------------------
-- Function: notify_change
-- Description: Sends a notification on table changes (for pub/sub)
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION notify_change()
RETURNS TRIGGER AS $$
DECLARE
    payload JSONB;
BEGIN
    payload = jsonb_build_object(
        'operation', TG_OP,
        'table', TG_TABLE_NAME,
        'timestamp', NOW()
    );

    IF TG_OP = 'DELETE' THEN
        payload = payload || jsonb_build_object('id', OLD.id);
    ELSE
        payload = payload || jsonb_build_object('id', NEW.id);
    END IF;

    PERFORM pg_notify(TG_TABLE_NAME || '_changes', payload::text);

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION notify_change() IS 'Sends PostgreSQL notifications on table changes';

-- ═══════════════════════════════════════════════════════════════════════════════
-- AGENT FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: update_agent_last_active
-- Description: Updates agent's last_active_at timestamp
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_agent_last_active()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE agents
    SET last_active_at = NOW()
    WHERE id = NEW.agent_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_agent_last_active() IS 'Updates agent last_active_at when they perform actions';

-- ---------------------------------------------------------------------------
-- Function: update_agent_load
-- Description: Updates agent's current load based on assigned tasks
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_agent_load()
RETURNS TRIGGER AS $$
DECLARE
    new_load INTEGER;
BEGIN
    -- Calculate current load (tasks in assigned or running status)
    SELECT COUNT(*)
    INTO new_load
    FROM tasks
    WHERE agent_id = COALESCE(NEW.agent_id, OLD.agent_id)
    AND status IN ('assigned', 'running');

    -- Update agent
    UPDATE agents
    SET
        current_load = new_load,
        status = CASE
            WHEN new_load = 0 THEN 'idle'::agent_status
            WHEN new_load >= max_load THEN 'overloaded'::agent_status
            ELSE 'busy'::agent_status
        END
    WHERE id = COALESCE(NEW.agent_id, OLD.agent_id);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_agent_load() IS 'Recalculates agent load and status based on assigned tasks';

-- ---------------------------------------------------------------------------
-- Function: update_agent_stats
-- Description: Updates agent statistics after task completion
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_agent_stats()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status IN ('completed', 'failed') AND OLD.status NOT IN ('completed', 'failed') THEN
        UPDATE agents
        SET
            success_count = success_count + CASE WHEN NEW.status = 'completed' THEN 1 ELSE 0 END,
            failure_count = failure_count + CASE WHEN NEW.status = 'failed' THEN 1 ELSE 0 END,
            total_tasks_completed = total_tasks_completed + 1,
            total_tokens_used = total_tokens_used + NEW.tokens_used,
            total_cost = total_cost + NEW.cost_dollars,
            -- Update reputation score (simple moving average)
            reputation_score = CASE
                WHEN total_tasks_completed = 0 THEN
                    CASE WHEN NEW.status = 'completed' THEN 1.0 ELSE 0.0 END
                ELSE
                    (reputation_score * total_tasks_completed +
                     CASE WHEN NEW.status = 'completed' THEN 1.0 ELSE 0.0 END) /
                    (total_tasks_completed + 1)
            END
        WHERE id = NEW.agent_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_agent_stats() IS 'Updates agent statistics and reputation after task completion';

-- ---------------------------------------------------------------------------
-- Function: find_available_agent
-- Description: Finds the best available agent for a task
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION find_available_agent(
    p_model VARCHAR DEFAULT NULL,
    p_required_tools VARCHAR[] DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    v_agent_id UUID;
BEGIN
    SELECT id INTO v_agent_id
    FROM agents
    WHERE status IN ('idle', 'busy')
    AND current_load < max_load
    AND (p_model IS NULL OR model = p_model)
    AND (
        p_required_tools IS NULL
        OR tools @> to_jsonb(p_required_tools)
    )
    ORDER BY
        -- Prefer idle agents
        CASE WHEN status = 'idle' THEN 0 ELSE 1 END,
        -- Then by reputation
        reputation_score DESC,
        -- Then by current load (least loaded first)
        current_load ASC
    LIMIT 1;

    RETURN v_agent_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION find_available_agent(VARCHAR, VARCHAR[]) IS 'Finds the best available agent based on model, tools, and reputation';

-- ═══════════════════════════════════════════════════════════════════════════════
-- DAG FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: update_dag_status
-- Description: Updates DAG status based on task states
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_dag_status()
RETURNS TRIGGER AS $$
DECLARE
    v_total INTEGER;
    v_completed INTEGER;
    v_failed INTEGER;
    v_running INTEGER;
    v_new_status dag_status;
BEGIN
    -- Count tasks by status
    SELECT
        COUNT(*),
        COUNT(*) FILTER (WHERE status = 'completed'),
        COUNT(*) FILTER (WHERE status = 'failed'),
        COUNT(*) FILTER (WHERE status IN ('assigned', 'running'))
    INTO v_total, v_completed, v_failed, v_running
    FROM tasks
    WHERE dag_id = NEW.dag_id;

    -- Determine new DAG status
    IF v_failed > 0 THEN
        v_new_status = 'failed';
    ELSIF v_completed = v_total AND v_total > 0 THEN
        v_new_status = 'completed';
    ELSIF v_running > 0 OR v_completed > 0 THEN
        v_new_status = 'running';
    ELSE
        v_new_status = 'pending';
    END IF;

    -- Update DAG
    UPDATE dags
    SET
        status = v_new_status,
        completed_tasks = v_completed,
        failed_tasks = v_failed,
        started_at = CASE
            WHEN started_at IS NULL AND v_new_status = 'running' THEN NOW()
            ELSE started_at
        END,
        completed_at = CASE
            WHEN v_new_status IN ('completed', 'failed') AND completed_at IS NULL THEN NOW()
            ELSE completed_at
        END,
        updated_at = NOW()
    WHERE id = NEW.dag_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_dag_status() IS 'Updates DAG status and timestamps based on task state changes';

-- ---------------------------------------------------------------------------
-- Function: update_dag_costs
-- Description: Updates DAG cost and token totals
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_dag_costs()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE dags
    SET
        total_tokens_used = (
            SELECT COALESCE(SUM(tokens_used), 0)
            FROM tasks WHERE dag_id = NEW.dag_id
        ),
        total_cost = (
            SELECT COALESCE(SUM(cost_dollars), 0)
            FROM tasks WHERE dag_id = NEW.dag_id
        )
    WHERE id = NEW.dag_id;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_dag_costs() IS 'Updates DAG cost totals when task costs change';

-- ---------------------------------------------------------------------------
-- Function: check_dag_cycle
-- Description: Checks for cycles in task dependencies
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION check_dag_cycle(
    p_task_id UUID,
    p_depends_on_id UUID
)
RETURNS BOOLEAN AS $$
DECLARE
    v_has_cycle BOOLEAN;
BEGIN
    WITH RECURSIVE dependency_chain AS (
        -- Start with the task we're adding a dependency TO
        SELECT depends_on_id, 1 as depth
        FROM task_dependencies
        WHERE task_id = p_depends_on_id

        UNION ALL

        -- Recursively follow dependencies
        SELECT td.depends_on_id, dc.depth + 1
        FROM task_dependencies td
        JOIN dependency_chain dc ON td.task_id = dc.depends_on_id
        WHERE dc.depth < 100  -- Prevent infinite recursion
    )
    SELECT EXISTS (
        SELECT 1 FROM dependency_chain WHERE depends_on_id = p_task_id
    ) INTO v_has_cycle;

    RETURN v_has_cycle;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION check_dag_cycle(UUID, UUID) IS 'Checks if adding a dependency would create a cycle';

-- ---------------------------------------------------------------------------
-- Function: get_ready_tasks
-- Description: Gets tasks that are ready to execute (all dependencies met)
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION get_ready_tasks(p_dag_id UUID)
RETURNS TABLE (task_id UUID, task_name VARCHAR, priority INTEGER) AS $$
BEGIN
    RETURN QUERY
    SELECT t.id, t.name, t.priority
    FROM tasks t
    WHERE t.dag_id = p_dag_id
    AND t.status = 'pending'
    AND NOT EXISTS (
        -- Check if any dependency is not completed
        SELECT 1
        FROM task_dependencies td
        JOIN tasks dep ON td.depends_on_id = dep.id
        WHERE td.task_id = t.id
        AND dep.status != 'completed'
    )
    ORDER BY t.priority DESC, t.created_at ASC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_ready_tasks(UUID) IS 'Returns tasks ready for execution (all dependencies satisfied)';

-- ═══════════════════════════════════════════════════════════════════════════════
-- TASK FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: assign_task_to_agent
-- Description: Assigns a task to an agent with validation
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION assign_task_to_agent(
    p_task_id UUID,
    p_agent_id UUID
)
RETURNS BOOLEAN AS $$
DECLARE
    v_task_status task_status;
    v_agent_status agent_status;
    v_current_load INTEGER;
    v_max_load INTEGER;
BEGIN
    -- Lock the task and agent rows
    SELECT status INTO v_task_status
    FROM tasks WHERE id = p_task_id FOR UPDATE;

    SELECT status, current_load, max_load
    INTO v_agent_status, v_current_load, v_max_load
    FROM agents WHERE id = p_agent_id FOR UPDATE;

    -- Validate task status
    IF v_task_status IS NULL THEN
        RAISE EXCEPTION 'Task not found: %', p_task_id;
    END IF;

    IF v_task_status NOT IN ('pending', 'ready') THEN
        RAISE EXCEPTION 'Task cannot be assigned - current status: %', v_task_status;
    END IF;

    -- Validate agent
    IF v_agent_status IS NULL THEN
        RAISE EXCEPTION 'Agent not found: %', p_agent_id;
    END IF;

    IF v_agent_status NOT IN ('idle', 'busy') THEN
        RAISE EXCEPTION 'Agent not available - current status: %', v_agent_status;
    END IF;

    IF v_current_load >= v_max_load THEN
        RAISE EXCEPTION 'Agent at capacity - load: %/%', v_current_load, v_max_load;
    END IF;

    -- Assign the task
    UPDATE tasks
    SET
        agent_id = p_agent_id,
        status = 'assigned',
        updated_at = NOW()
    WHERE id = p_task_id;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION assign_task_to_agent(UUID, UUID) IS 'Safely assigns a task to an agent with validation';

-- ---------------------------------------------------------------------------
-- Function: complete_task
-- Description: Marks a task as completed and updates related entities
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION complete_task(
    p_task_id UUID,
    p_output JSONB,
    p_tokens_used BIGINT DEFAULT 0,
    p_cost_dollars DECIMAL DEFAULT 0
)
RETURNS BOOLEAN AS $$
DECLARE
    v_task_status task_status;
    v_started_at TIMESTAMPTZ;
BEGIN
    -- Lock and validate
    SELECT status, started_at INTO v_task_status, v_started_at
    FROM tasks WHERE id = p_task_id FOR UPDATE;

    IF v_task_status IS NULL THEN
        RAISE EXCEPTION 'Task not found: %', p_task_id;
    END IF;

    IF v_task_status != 'running' THEN
        RAISE EXCEPTION 'Task cannot be completed - current status: %', v_task_status;
    END IF;

    -- Update task
    UPDATE tasks
    SET
        status = 'completed',
        output = p_output,
        tokens_used = p_tokens_used,
        cost_dollars = p_cost_dollars,
        actual_duration_ms = EXTRACT(EPOCH FROM (NOW() - v_started_at)) * 1000,
        completed_at = NOW(),
        updated_at = NOW()
    WHERE id = p_task_id;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION complete_task(UUID, JSONB, BIGINT, DECIMAL) IS 'Marks a task as completed with output and metrics';

-- ---------------------------------------------------------------------------
-- Function: fail_task
-- Description: Marks a task as failed with error information
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION fail_task(
    p_task_id UUID,
    p_error TEXT,
    p_error_code VARCHAR DEFAULT NULL,
    p_error_details JSONB DEFAULT NULL
)
RETURNS BOOLEAN AS $$
DECLARE
    v_task_record RECORD;
BEGIN
    -- Lock and get task info
    SELECT * INTO v_task_record
    FROM tasks WHERE id = p_task_id FOR UPDATE;

    IF v_task_record IS NULL THEN
        RAISE EXCEPTION 'Task not found: %', p_task_id;
    END IF;

    -- Check if retries are available
    IF v_task_record.retry_count < v_task_record.max_retries THEN
        -- Schedule for retry
        UPDATE tasks
        SET
            status = 'pending',
            error = p_error,
            error_code = p_error_code,
            error_details = p_error_details,
            retry_count = retry_count + 1,
            agent_id = NULL,
            updated_at = NOW()
        WHERE id = p_task_id;
    ELSE
        -- Mark as failed (no more retries)
        UPDATE tasks
        SET
            status = 'failed',
            error = p_error,
            error_code = p_error_code,
            error_details = p_error_details,
            actual_duration_ms = CASE
                WHEN started_at IS NOT NULL
                THEN EXTRACT(EPOCH FROM (NOW() - started_at)) * 1000
                ELSE NULL
            END,
            completed_at = NOW(),
            updated_at = NOW()
        WHERE id = p_task_id;
    END IF;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION fail_task(UUID, TEXT, VARCHAR, JSONB) IS 'Marks a task as failed with optional retry';

-- ═══════════════════════════════════════════════════════════════════════════════
-- APPROVAL FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: process_approval
-- Description: Processes an approval decision
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION process_approval(
    p_approval_id UUID,
    p_status approval_status,
    p_decided_by VARCHAR,
    p_reason TEXT DEFAULT NULL
)
RETURNS BOOLEAN AS $$
DECLARE
    v_current_status approval_status;
BEGIN
    -- Lock and validate
    SELECT status INTO v_current_status
    FROM approvals WHERE id = p_approval_id FOR UPDATE;

    IF v_current_status IS NULL THEN
        RAISE EXCEPTION 'Approval not found: %', p_approval_id;
    END IF;

    IF v_current_status != 'pending' THEN
        RAISE EXCEPTION 'Approval already processed - status: %', v_current_status;
    END IF;

    -- Update approval
    UPDATE approvals
    SET
        status = p_status,
        decided_by = p_decided_by,
        decision_reason = p_reason,
        decided_at = NOW(),
        updated_at = NOW()
    WHERE id = p_approval_id;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION process_approval(UUID, approval_status, VARCHAR, TEXT) IS 'Processes an approval decision';

-- ---------------------------------------------------------------------------
-- Function: expire_pending_approvals
-- Description: Expires overdue pending approvals
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION expire_pending_approvals()
RETURNS INTEGER AS $$
DECLARE
    v_count INTEGER;
BEGIN
    UPDATE approvals
    SET
        status = 'expired',
        updated_at = NOW()
    WHERE status = 'pending'
    AND expires_at IS NOT NULL
    AND expires_at < NOW();

    GET DIAGNOSTICS v_count = ROW_COUNT;
    RETURN v_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION expire_pending_approvals() IS 'Expires all pending approvals past their expiration date';

-- ═══════════════════════════════════════════════════════════════════════════════
-- CONTRACT FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: check_contract_limits
-- Description: Checks if a contract has exceeded any limits
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION check_contract_limits(p_contract_id UUID)
RETURNS JSONB AS $$
DECLARE
    v_contract RECORD;
    v_result JSONB;
BEGIN
    SELECT * INTO v_contract
    FROM agent_contracts
    WHERE id = p_contract_id;

    IF v_contract IS NULL THEN
        RETURN jsonb_build_object('error', 'Contract not found');
    END IF;

    v_result = jsonb_build_object(
        'contract_id', p_contract_id,
        'status', v_contract.status,
        'limits_exceeded', jsonb_build_object(
            'tokens', v_contract.tokens_used >= v_contract.token_limit,
            'cost', v_contract.cost_used >= v_contract.cost_limit,
            'time', v_contract.time_used_seconds >= v_contract.time_limit_seconds,
            'api_calls', v_contract.api_calls_used >= v_contract.api_call_limit
        ),
        'usage_percent', jsonb_build_object(
            'tokens', ROUND((v_contract.tokens_used::numeric / NULLIF(v_contract.token_limit, 0)) * 100, 2),
            'cost', ROUND((v_contract.cost_used::numeric / NULLIF(v_contract.cost_limit, 0)) * 100, 2),
            'time', ROUND((v_contract.time_used_seconds::numeric / NULLIF(v_contract.time_limit_seconds, 0)) * 100, 2),
            'api_calls', ROUND((v_contract.api_calls_used::numeric / NULLIF(v_contract.api_call_limit, 0)) * 100, 2)
        )
    );

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION check_contract_limits(UUID) IS 'Returns contract limit status and usage percentages';

-- ---------------------------------------------------------------------------
-- Function: update_contract_usage
-- Description: Updates contract usage and checks for exceeded limits
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_contract_usage(
    p_contract_id UUID,
    p_tokens BIGINT DEFAULT 0,
    p_cost DECIMAL DEFAULT 0,
    p_api_calls BIGINT DEFAULT 0,
    p_tool_calls BIGINT DEFAULT 0
)
RETURNS BOOLEAN AS $$
DECLARE
    v_contract RECORD;
    v_exceeded BOOLEAN := FALSE;
BEGIN
    -- Lock and update
    SELECT * INTO v_contract
    FROM agent_contracts
    WHERE id = p_contract_id FOR UPDATE;

    IF v_contract IS NULL THEN
        RAISE EXCEPTION 'Contract not found: %', p_contract_id;
    END IF;

    IF v_contract.status != 'active' THEN
        RAISE EXCEPTION 'Contract not active - status: %', v_contract.status;
    END IF;

    -- Update usage
    UPDATE agent_contracts
    SET
        tokens_used = tokens_used + p_tokens,
        cost_used = cost_used + p_cost,
        api_calls_used = api_calls_used + p_api_calls,
        tool_calls_used = tool_calls_used + p_tool_calls,
        updated_at = NOW()
    WHERE id = p_contract_id
    RETURNING * INTO v_contract;

    -- Check if any limit exceeded
    IF v_contract.tokens_used >= v_contract.token_limit
       OR v_contract.cost_used >= v_contract.cost_limit
       OR v_contract.api_calls_used >= v_contract.api_call_limit
       OR (v_contract.tool_call_limit IS NOT NULL AND v_contract.tool_calls_used >= v_contract.tool_call_limit)
    THEN
        UPDATE agent_contracts
        SET status = 'exceeded', updated_at = NOW()
        WHERE id = p_contract_id;
        v_exceeded := TRUE;
    END IF;

    RETURN NOT v_exceeded;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_contract_usage(UUID, BIGINT, DECIMAL, BIGINT, BIGINT) IS 'Updates contract usage and returns FALSE if limits exceeded';

-- ═══════════════════════════════════════════════════════════════════════════════
-- AUDIT FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: create_audit_log
-- Description: Creates an audit log entry
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION create_audit_log(
    p_entity_type VARCHAR,
    p_entity_id UUID,
    p_action audit_action,
    p_actor_type VARCHAR,
    p_actor_id VARCHAR DEFAULT NULL,
    p_actor_name VARCHAR DEFAULT NULL,
    p_old_values JSONB DEFAULT NULL,
    p_new_values JSONB DEFAULT NULL,
    p_metadata JSONB DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    v_log_id UUID;
    v_changed_fields VARCHAR[];
BEGIN
    -- Calculate changed fields
    IF p_old_values IS NOT NULL AND p_new_values IS NOT NULL THEN
        SELECT ARRAY_AGG(key)
        INTO v_changed_fields
        FROM (
            SELECT key
            FROM jsonb_each(p_new_values)
            WHERE p_old_values->key IS DISTINCT FROM p_new_values->key
        ) changed;
    END IF;

    INSERT INTO audit_logs (
        entity_type, entity_id, action,
        actor_type, actor_id, actor_name,
        old_values, new_values, changed_fields,
        metadata, partition_key
    )
    VALUES (
        p_entity_type, p_entity_id, p_action,
        p_actor_type, p_actor_id, p_actor_name,
        p_old_values, p_new_values, v_changed_fields,
        COALESCE(p_metadata, '{}'::jsonb), CURRENT_DATE
    )
    RETURNING id INTO v_log_id;

    RETURN v_log_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION create_audit_log(VARCHAR, UUID, audit_action, VARCHAR, VARCHAR, VARCHAR, JSONB, JSONB, JSONB) IS 'Creates a standardized audit log entry';

-- ═══════════════════════════════════════════════════════════════════════════════
-- EVENT FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: append_event
-- Description: Appends an event to the event store
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION append_event(
    p_aggregate_type VARCHAR,
    p_aggregate_id UUID,
    p_event_type VARCHAR,
    p_event_data JSONB,
    p_expected_version INTEGER DEFAULT NULL,
    p_metadata JSONB DEFAULT NULL
)
RETURNS BIGINT AS $$
DECLARE
    v_current_version INTEGER;
    v_new_version INTEGER;
    v_event_id BIGINT;
BEGIN
    -- Get current version
    SELECT COALESCE(MAX(version), 0)
    INTO v_current_version
    FROM events
    WHERE aggregate_type = p_aggregate_type
    AND aggregate_id = p_aggregate_id;

    -- Check expected version for optimistic concurrency
    IF p_expected_version IS NOT NULL AND v_current_version != p_expected_version THEN
        RAISE EXCEPTION 'Concurrency conflict - expected version %, but found %',
            p_expected_version, v_current_version;
    END IF;

    v_new_version := v_current_version + 1;

    -- Insert event
    INSERT INTO events (
        aggregate_type, aggregate_id, event_type,
        event_data, version, metadata
    )
    VALUES (
        p_aggregate_type, p_aggregate_id, p_event_type,
        p_event_data, v_new_version, COALESCE(p_metadata, '{}'::jsonb)
    )
    RETURNING id INTO v_event_id;

    -- Notify listeners
    PERFORM pg_notify(
        'events',
        jsonb_build_object(
            'id', v_event_id,
            'aggregate_type', p_aggregate_type,
            'aggregate_id', p_aggregate_id,
            'event_type', p_event_type,
            'version', v_new_version
        )::text
    );

    RETURN v_event_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION append_event(VARCHAR, UUID, VARCHAR, JSONB, INTEGER, JSONB) IS 'Appends an event to the event store with optimistic concurrency';

-- ---------------------------------------------------------------------------
-- Function: get_aggregate_events
-- Description: Gets all events for an aggregate
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION get_aggregate_events(
    p_aggregate_type VARCHAR,
    p_aggregate_id UUID,
    p_from_version INTEGER DEFAULT 0
)
RETURNS TABLE (
    id BIGINT,
    event_type VARCHAR,
    event_data JSONB,
    version INTEGER,
    created_at TIMESTAMPTZ
) AS $$
BEGIN
    RETURN QUERY
    SELECT e.id, e.event_type, e.event_data, e.version, e.created_at
    FROM events e
    WHERE e.aggregate_type = p_aggregate_type
    AND e.aggregate_id = p_aggregate_id
    AND e.version > p_from_version
    ORDER BY e.version ASC;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_aggregate_events(VARCHAR, UUID, INTEGER) IS 'Retrieves events for an aggregate from a given version';

-- ═══════════════════════════════════════════════════════════════════════════════
-- ANALYTICS FUNCTIONS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- Function: get_system_stats
-- Description: Gets system-wide statistics
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION get_system_stats()
RETURNS JSONB AS $$
DECLARE
    v_stats JSONB;
BEGIN
    SELECT jsonb_build_object(
        'tasks', jsonb_build_object(
            'total', (SELECT COUNT(*) FROM tasks),
            'pending', (SELECT COUNT(*) FROM tasks WHERE status = 'pending'),
            'running', (SELECT COUNT(*) FROM tasks WHERE status IN ('assigned', 'running')),
            'completed', (SELECT COUNT(*) FROM tasks WHERE status = 'completed'),
            'failed', (SELECT COUNT(*) FROM tasks WHERE status = 'failed')
        ),
        'agents', jsonb_build_object(
            'total', (SELECT COUNT(*) FROM agents),
            'idle', (SELECT COUNT(*) FROM agents WHERE status = 'idle'),
            'busy', (SELECT COUNT(*) FROM agents WHERE status = 'busy'),
            'offline', (SELECT COUNT(*) FROM agents WHERE status = 'offline')
        ),
        'dags', jsonb_build_object(
            'total', (SELECT COUNT(*) FROM dags),
            'running', (SELECT COUNT(*) FROM dags WHERE status = 'running'),
            'completed', (SELECT COUNT(*) FROM dags WHERE status = 'completed')
        ),
        'usage', jsonb_build_object(
            'total_tokens', (SELECT COALESCE(SUM(total_tokens), 0) FROM usage_records),
            'total_cost', (SELECT COALESCE(SUM(cost_dollars), 0) FROM usage_records)
        ),
        'approvals', jsonb_build_object(
            'pending', (SELECT COUNT(*) FROM approvals WHERE status = 'pending')
        ),
        'timestamp', NOW()
    ) INTO v_stats;

    RETURN v_stats;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_system_stats() IS 'Returns comprehensive system statistics';

-- ---------------------------------------------------------------------------
-- Function: get_agent_performance
-- Description: Gets detailed performance metrics for an agent
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION get_agent_performance(p_agent_id UUID)
RETURNS JSONB AS $$
DECLARE
    v_metrics JSONB;
BEGIN
    SELECT jsonb_build_object(
        'agent_id', p_agent_id,
        'tasks', jsonb_build_object(
            'total_completed', success_count + failure_count,
            'successful', success_count,
            'failed', failure_count,
            'success_rate', CASE
                WHEN success_count + failure_count > 0
                THEN ROUND(success_count::numeric / (success_count + failure_count), 4)
                ELSE 1.0
            END
        ),
        'resources', jsonb_build_object(
            'total_tokens', total_tokens_used,
            'total_cost', total_cost,
            'avg_tokens_per_task', CASE
                WHEN total_tasks_completed > 0
                THEN ROUND(total_tokens_used::numeric / total_tasks_completed, 2)
                ELSE 0
            END
        ),
        'scores', jsonb_build_object(
            'reputation', reputation_score,
            'reliability', reliability_score
        ),
        'capacity', jsonb_build_object(
            'current_load', current_load,
            'max_load', max_load,
            'utilization', ROUND(current_load::numeric / max_load, 2)
        ),
        'timing', jsonb_build_object(
            'last_active', last_active_at,
            'created', created_at
        )
    )
    INTO v_metrics
    FROM agents
    WHERE id = p_agent_id;

    RETURN v_metrics;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_agent_performance(UUID) IS 'Returns detailed performance metrics for a specific agent';

-- ═══════════════════════════════════════════════════════════════════════════════
-- TRIGGERS
-- ═══════════════════════════════════════════════════════════════════════════════

-- Updated_at triggers
CREATE TRIGGER trg_agents_updated_at
    BEFORE UPDATE ON agents
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_dags_updated_at
    BEFORE UPDATE ON dags
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_tasks_updated_at
    BEFORE UPDATE ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_dag_nodes_updated_at
    BEFORE UPDATE ON dag_nodes
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_approvals_updated_at
    BEFORE UPDATE ON approvals
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_contracts_updated_at
    BEFORE UPDATE ON agent_contracts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER trg_config_updated_at
    BEFORE UPDATE ON system_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- Agent triggers
CREATE TRIGGER trg_tool_calls_agent_active
    AFTER INSERT ON tool_calls
    FOR EACH ROW EXECUTE FUNCTION update_agent_last_active();

CREATE TRIGGER trg_tasks_agent_load
    AFTER INSERT OR UPDATE OF agent_id, status OR DELETE ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_agent_load();

CREATE TRIGGER trg_tasks_agent_stats
    AFTER UPDATE OF status ON tasks
    FOR EACH ROW
    WHEN (NEW.agent_id IS NOT NULL)
    EXECUTE FUNCTION update_agent_stats();

-- DAG triggers
CREATE TRIGGER trg_tasks_dag_status
    AFTER INSERT OR UPDATE OF status ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_dag_status();

CREATE TRIGGER trg_tasks_dag_costs
    AFTER UPDATE OF tokens_used, cost_dollars ON tasks
    FOR EACH ROW EXECUTE FUNCTION update_dag_costs();

-- Notification triggers
CREATE TRIGGER trg_tasks_notify
    AFTER INSERT OR UPDATE OR DELETE ON tasks
    FOR EACH ROW EXECUTE FUNCTION notify_change();

CREATE TRIGGER trg_approvals_notify
    AFTER INSERT OR UPDATE OR DELETE ON approvals
    FOR EACH ROW EXECUTE FUNCTION notify_change();

CREATE TRIGGER trg_agents_notify
    AFTER INSERT OR UPDATE OR DELETE ON agents
    FOR EACH ROW EXECUTE FUNCTION notify_change();

-- ═══════════════════════════════════════════════════════════════════════════════
-- VIEWS
-- ═══════════════════════════════════════════════════════════════════════════════

-- ---------------------------------------------------------------------------
-- View: agent_summary
-- Description: Agent overview with computed metrics
-- ---------------------------------------------------------------------------
CREATE OR REPLACE VIEW agent_summary AS
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
        THEN ROUND(a.success_count::numeric / (a.success_count + a.failure_count), 4)
        ELSE 1.0
    END AS success_rate,
    a.total_tokens_used,
    a.total_cost,
    a.reputation_score,
    a.reliability_score,
    a.last_active_at,
    a.created_at,
    COUNT(t.id) FILTER (WHERE t.status IN ('assigned', 'running')) AS active_tasks
FROM agents a
LEFT JOIN tasks t ON a.id = t.agent_id
GROUP BY a.id;

COMMENT ON VIEW agent_summary IS 'Aggregated view of agent status and performance';

-- ---------------------------------------------------------------------------
-- View: dag_summary
-- Description: DAG overview with task statistics
-- ---------------------------------------------------------------------------
CREATE OR REPLACE VIEW dag_summary AS
SELECT
    d.id,
    d.name,
    d.status,
    d.total_tasks,
    d.completed_tasks,
    d.failed_tasks,
    d.total_tasks - d.completed_tasks - d.failed_tasks AS remaining_tasks,
    CASE
        WHEN d.total_tasks > 0
        THEN ROUND(d.completed_tasks::numeric / d.total_tasks * 100, 2)
        ELSE 0
    END AS completion_percent,
    d.total_tokens_used,
    d.total_cost,
    d.created_at,
    d.started_at,
    d.completed_at,
    EXTRACT(EPOCH FROM (COALESCE(d.completed_at, NOW()) - d.started_at)) AS duration_seconds
FROM dags d;

COMMENT ON VIEW dag_summary IS 'Aggregated view of DAG execution status and progress';

-- ---------------------------------------------------------------------------
-- View: pending_approvals
-- Description: Pending approvals with task and agent info
-- ---------------------------------------------------------------------------
CREATE OR REPLACE VIEW pending_approvals AS
SELECT
    ap.id,
    ap.action,
    ap.action_description,
    ap.risk_score,
    ap.created_at,
    ap.expires_at,
    t.id AS task_id,
    t.name AS task_name,
    a.id AS agent_id,
    a.name AS agent_name,
    a.model AS agent_model
FROM approvals ap
JOIN tasks t ON ap.task_id = t.id
JOIN agents a ON ap.agent_id = a.id
WHERE ap.status = 'pending'
ORDER BY ap.risk_score DESC NULLS LAST, ap.created_at ASC;

COMMENT ON VIEW pending_approvals IS 'View of pending approval requests with context';

-- ---------------------------------------------------------------------------
-- View: system_metrics
-- Description: Real-time system metrics
-- ---------------------------------------------------------------------------
CREATE OR REPLACE VIEW system_metrics AS
SELECT
    (SELECT COUNT(*) FROM tasks WHERE status IN ('assigned', 'running')) AS running_tasks,
    (SELECT COUNT(*) FROM tasks WHERE status = 'pending') AS pending_tasks,
    (SELECT COUNT(*) FROM agents WHERE status IN ('idle', 'busy')) AS available_agents,
    (SELECT AVG(current_load::numeric / NULLIF(max_load, 0)) FROM agents WHERE status != 'offline') AS avg_agent_utilization,
    (SELECT COUNT(*) FROM approvals WHERE status = 'pending') AS pending_approvals,
    (SELECT COUNT(*) FROM dags WHERE status = 'running') AS running_dags,
    (SELECT SUM(total_tokens) FROM usage_records WHERE created_at > NOW() - INTERVAL '1 hour') AS tokens_last_hour,
    (SELECT SUM(cost_dollars) FROM usage_records WHERE created_at > NOW() - INTERVAL '1 hour') AS cost_last_hour;

COMMENT ON VIEW system_metrics IS 'Real-time system health and performance metrics';
