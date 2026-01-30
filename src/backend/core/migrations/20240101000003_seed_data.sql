-- ═══════════════════════════════════════════════════════════════════════════════
-- Project Apex - Seed Data for Development
-- Migration: 20240101000003_seed_data.sql
-- Description: Initial seed data for development and testing
-- ═══════════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════════
-- SYSTEM CONFIGURATION
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO system_config (key, value, description, category, is_sensitive) VALUES
-- General settings
('system.version', '"1.0.0"', 'Current system version', 'system', FALSE),
('system.environment', '"development"', 'Current environment (development, staging, production)', 'system', FALSE),
('system.maintenance_mode', 'false', 'Enable maintenance mode', 'system', FALSE),

-- Task settings
('tasks.default_timeout_seconds', '3600', 'Default task timeout in seconds', 'tasks', FALSE),
('tasks.default_max_retries', '3', 'Default maximum retry attempts for tasks', 'tasks', FALSE),
('tasks.retry_delay_seconds', '60', 'Delay between task retries in seconds', 'tasks', FALSE),
('tasks.priority_queue_enabled', 'true', 'Enable priority-based task scheduling', 'tasks', FALSE),

-- Agent settings
('agents.default_max_load', '10', 'Default maximum concurrent tasks per agent', 'agents', FALSE),
('agents.health_check_interval_seconds', '30', 'Interval for agent health checks', 'agents', FALSE),
('agents.idle_timeout_seconds', '300', 'Time before idle agent is marked offline', 'agents', FALSE),
('agents.reputation_decay_factor', '0.95', 'Daily reputation score decay factor', 'agents', FALSE),

-- Approval settings
('approvals.default_expiry_hours', '24', 'Default approval request expiry time in hours', 'approvals', FALSE),
('approvals.high_risk_threshold', '0.7', 'Risk score threshold for mandatory approval', 'approvals', FALSE),
('approvals.auto_approve_low_risk', 'false', 'Auto-approve requests below risk threshold', 'approvals', FALSE),
('approvals.batch_processing_enabled', 'true', 'Enable batch processing of similar approvals', 'approvals', FALSE),

-- Contract settings
('contracts.default_token_limit', '100000', 'Default token limit for agent contracts', 'contracts', FALSE),
('contracts.default_cost_limit', '10.00', 'Default cost limit in dollars for agent contracts', 'contracts', FALSE),
('contracts.default_time_limit_seconds', '3600', 'Default time limit for agent contracts', 'contracts', FALSE),
('contracts.default_api_call_limit', '1000', 'Default API call limit for agent contracts', 'contracts', FALSE),

-- Cost settings
('costs.claude_opus_input_per_1k', '0.015', 'Cost per 1K input tokens for Claude Opus', 'costs', FALSE),
('costs.claude_opus_output_per_1k', '0.075', 'Cost per 1K output tokens for Claude Opus', 'costs', FALSE),
('costs.claude_sonnet_input_per_1k', '0.003', 'Cost per 1K input tokens for Claude Sonnet', 'costs', FALSE),
('costs.claude_sonnet_output_per_1k', '0.015', 'Cost per 1K output tokens for Claude Sonnet', 'costs', FALSE),
('costs.claude_haiku_input_per_1k', '0.00025', 'Cost per 1K input tokens for Claude Haiku', 'costs', FALSE),
('costs.claude_haiku_output_per_1k', '0.00125', 'Cost per 1K output tokens for Claude Haiku', 'costs', FALSE),

-- Rate limiting
('rate_limits.requests_per_minute', '60', 'Maximum API requests per minute', 'rate_limits', FALSE),
('rate_limits.tokens_per_minute', '100000', 'Maximum tokens per minute', 'rate_limits', FALSE),

-- Feature flags
('features.event_sourcing_enabled', 'true', 'Enable event sourcing', 'features', FALSE),
('features.audit_logging_enabled', 'true', 'Enable detailed audit logging', 'features', FALSE),
('features.real_time_notifications', 'true', 'Enable real-time notifications via WebSocket', 'features', FALSE),
('features.advanced_analytics', 'false', 'Enable advanced analytics features', 'features', FALSE)

ON CONFLICT (key) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE AGENTS
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO agents (id, name, description, model, model_version, system_prompt, tools, capabilities, config, status, max_load, tags) VALUES
(
    'a0000000-0000-0000-0000-000000000001',
    'apex-orchestrator',
    'Primary orchestrator agent responsible for task decomposition and delegation',
    'claude-3-opus-20240229',
    '20240229',
    'You are the primary orchestrator for Project Apex. Your role is to decompose complex tasks into subtasks, delegate work to specialized agents, and coordinate overall execution. Always prioritize efficiency, accuracy, and safety.',
    '["task_decompose", "agent_delegate", "dag_create", "status_check", "result_aggregate"]'::jsonb,
    '["orchestration", "planning", "delegation", "monitoring"]'::jsonb,
    '{"max_subtasks": 20, "delegation_strategy": "capability_match"}'::jsonb,
    'idle',
    5,
    ARRAY['orchestrator', 'primary', 'planning']
),
(
    'a0000000-0000-0000-0000-000000000002',
    'apex-coder',
    'Specialized agent for code generation, review, and debugging',
    'claude-3-sonnet-20240229',
    '20240229',
    'You are a specialized coding agent for Project Apex. Your expertise includes code generation, refactoring, debugging, and code review across multiple programming languages. Write clean, efficient, and well-documented code.',
    '["code_generate", "code_review", "debug", "refactor", "test_generate", "file_read", "file_write"]'::jsonb,
    '["coding", "debugging", "testing", "documentation"]'::jsonb,
    '{"preferred_languages": ["python", "typescript", "rust"], "style_guide": "google"}'::jsonb,
    'idle',
    10,
    ARRAY['coder', 'developer', 'specialist']
),
(
    'a0000000-0000-0000-0000-000000000003',
    'apex-researcher',
    'Research agent for information gathering, analysis, and synthesis',
    'claude-3-sonnet-20240229',
    '20240229',
    'You are a research specialist for Project Apex. Your role is to gather information, analyze data, synthesize findings, and provide comprehensive research reports. Always cite sources and maintain objectivity.',
    '["web_search", "document_analyze", "data_extract", "summarize", "cite_sources"]'::jsonb,
    '["research", "analysis", "synthesis", "reporting"]'::jsonb,
    '{"citation_style": "apa", "max_sources": 50}'::jsonb,
    'idle',
    15,
    ARRAY['researcher', 'analyst', 'specialist']
),
(
    'a0000000-0000-0000-0000-000000000004',
    'apex-reviewer',
    'Quality assurance agent for reviewing outputs and ensuring standards',
    'claude-3-haiku-20240307',
    '20240307',
    'You are a quality assurance reviewer for Project Apex. Your role is to review outputs from other agents, check for errors, verify accuracy, and ensure quality standards are met. Be thorough but constructive.',
    '["review_output", "validate_data", "check_quality", "suggest_improvements"]'::jsonb,
    '["review", "validation", "quality_assurance"]'::jsonb,
    '{"review_criteria": ["accuracy", "completeness", "clarity", "format"]}'::jsonb,
    'idle',
    20,
    ARRAY['reviewer', 'qa', 'validator']
),
(
    'a0000000-0000-0000-0000-000000000005',
    'apex-executor',
    'Execution agent for running tasks and interacting with external systems',
    'claude-3-haiku-20240307',
    '20240307',
    'You are an execution agent for Project Apex. Your role is to execute specific tasks, interact with external APIs and systems, and report results. Always handle errors gracefully and maintain security practices.',
    '["api_call", "shell_execute", "file_operations", "http_request", "data_transform"]'::jsonb,
    '["execution", "integration", "automation"]'::jsonb,
    '{"timeout_seconds": 300, "retry_on_failure": true}'::jsonb,
    'idle',
    25,
    ARRAY['executor', 'worker', 'integration']
)
ON CONFLICT (id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE DAG
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO dags (id, name, description, status, config, total_tasks, metadata, created_by, tags) VALUES
(
    'd0000000-0000-0000-0000-000000000001',
    'sample-analysis-workflow',
    'A sample workflow demonstrating task orchestration with multiple agents',
    'pending',
    '{"timeout_minutes": 60, "notify_on_complete": true}'::jsonb,
    5,
    '{"category": "sample", "version": "1.0"}'::jsonb,
    'system',
    ARRAY['sample', 'demo', 'analysis']
)
ON CONFLICT (id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE TASKS
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO tasks (id, dag_id, name, description, instruction, status, priority, input, metadata, tags) VALUES
(
    't0000000-0000-0000-0000-000000000001',
    'd0000000-0000-0000-0000-000000000001',
    'research-topic',
    'Research and gather information on the target topic',
    'Research the topic provided in the input. Gather relevant information from multiple sources, analyze findings, and prepare a structured summary. Include key facts, statistics, and notable perspectives.',
    'pending',
    100,
    '{"topic": "AI agent orchestration patterns", "depth": "comprehensive", "max_sources": 10}'::jsonb,
    '{"estimated_duration_minutes": 15}'::jsonb,
    ARRAY['research', 'initial']
),
(
    't0000000-0000-0000-0000-000000000002',
    'd0000000-0000-0000-0000-000000000001',
    'analyze-findings',
    'Analyze research findings and extract insights',
    'Analyze the research findings from the previous task. Identify patterns, key themes, and actionable insights. Create a structured analysis document with clear categorization.',
    'pending',
    90,
    '{"analysis_type": "thematic", "output_format": "structured"}'::jsonb,
    '{"estimated_duration_minutes": 10}'::jsonb,
    ARRAY['analysis', 'intermediate']
),
(
    't0000000-0000-0000-0000-000000000003',
    'd0000000-0000-0000-0000-000000000001',
    'generate-code',
    'Generate implementation code based on analysis',
    'Based on the analysis, generate sample implementation code demonstrating the identified patterns. Include proper documentation, error handling, and test cases.',
    'pending',
    80,
    '{"language": "python", "include_tests": true, "documentation_level": "detailed"}'::jsonb,
    '{"estimated_duration_minutes": 20}'::jsonb,
    ARRAY['coding', 'implementation']
),
(
    't0000000-0000-0000-0000-000000000004',
    'd0000000-0000-0000-0000-000000000001',
    'review-output',
    'Review all outputs for quality and accuracy',
    'Review all outputs from previous tasks. Check for accuracy, completeness, consistency, and quality. Provide detailed feedback and suggestions for improvement if needed.',
    'pending',
    70,
    '{"review_criteria": ["accuracy", "completeness", "quality", "consistency"]}'::jsonb,
    '{"estimated_duration_minutes": 10}'::jsonb,
    ARRAY['review', 'quality']
),
(
    't0000000-0000-0000-0000-000000000005',
    'd0000000-0000-0000-0000-000000000001',
    'compile-report',
    'Compile final report with all findings and outputs',
    'Compile a comprehensive final report incorporating research findings, analysis, code samples, and review feedback. Format the report professionally with executive summary, detailed sections, and appendices.',
    'pending',
    60,
    '{"format": "markdown", "include_executive_summary": true, "include_appendices": true}'::jsonb,
    '{"estimated_duration_minutes": 15}'::jsonb,
    ARRAY['report', 'final']
)
ON CONFLICT (id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE DAG NODES
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO dag_nodes (id, dag_id, task_id, node_order, depth_level, dependencies, is_entry_point, is_exit_point) VALUES
(
    'n0000000-0000-0000-0000-000000000001',
    'd0000000-0000-0000-0000-000000000001',
    't0000000-0000-0000-0000-000000000001',
    1,
    0,
    ARRAY[]::UUID[],
    TRUE,
    FALSE
),
(
    'n0000000-0000-0000-0000-000000000002',
    'd0000000-0000-0000-0000-000000000001',
    't0000000-0000-0000-0000-000000000002',
    2,
    1,
    ARRAY['t0000000-0000-0000-0000-000000000001']::UUID[],
    FALSE,
    FALSE
),
(
    'n0000000-0000-0000-0000-000000000003',
    'd0000000-0000-0000-0000-000000000001',
    't0000000-0000-0000-0000-000000000003',
    3,
    2,
    ARRAY['t0000000-0000-0000-0000-000000000002']::UUID[],
    FALSE,
    FALSE
),
(
    'n0000000-0000-0000-0000-000000000004',
    'd0000000-0000-0000-0000-000000000001',
    't0000000-0000-0000-0000-000000000004',
    4,
    3,
    ARRAY['t0000000-0000-0000-0000-000000000002', 't0000000-0000-0000-0000-000000000003']::UUID[],
    FALSE,
    FALSE
),
(
    'n0000000-0000-0000-0000-000000000005',
    'd0000000-0000-0000-0000-000000000001',
    't0000000-0000-0000-0000-000000000005',
    5,
    4,
    ARRAY['t0000000-0000-0000-0000-000000000004']::UUID[],
    FALSE,
    TRUE
)
ON CONFLICT (id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE TASK DEPENDENCIES
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO task_dependencies (task_id, depends_on_id, dependency_type, is_required) VALUES
-- analyze-findings depends on research-topic
('t0000000-0000-0000-0000-000000000002', 't0000000-0000-0000-0000-000000000001', 'completion', TRUE),
-- generate-code depends on analyze-findings
('t0000000-0000-0000-0000-000000000003', 't0000000-0000-0000-0000-000000000002', 'completion', TRUE),
-- review-output depends on analyze-findings and generate-code
('t0000000-0000-0000-0000-000000000004', 't0000000-0000-0000-0000-000000000002', 'completion', TRUE),
('t0000000-0000-0000-0000-000000000004', 't0000000-0000-0000-0000-000000000003', 'completion', TRUE),
-- compile-report depends on review-output
('t0000000-0000-0000-0000-000000000005', 't0000000-0000-0000-0000-000000000004', 'completion', TRUE)
ON CONFLICT (task_id, depends_on_id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE USAGE RECORDS (for analytics testing)
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO usage_records (id, agent_id, model, provider, prompt_tokens, completion_tokens, total_tokens, cost_dollars, request_type, latency_ms, response_status, created_at) VALUES
-- Historical usage data for testing analytics
('u0000000-0000-0000-0000-000000000001', 'a0000000-0000-0000-0000-000000000001', 'claude-3-opus-20240229', 'anthropic', 1500, 800, 2300, 0.0825, 'orchestration', 3200, 'success', NOW() - INTERVAL '7 days'),
('u0000000-0000-0000-0000-000000000002', 'a0000000-0000-0000-0000-000000000002', 'claude-3-sonnet-20240229', 'anthropic', 2000, 1500, 3500, 0.0285, 'code_generation', 4500, 'success', NOW() - INTERVAL '6 days'),
('u0000000-0000-0000-0000-000000000003', 'a0000000-0000-0000-0000-000000000003', 'claude-3-sonnet-20240229', 'anthropic', 3000, 2000, 5000, 0.0390, 'research', 5000, 'success', NOW() - INTERVAL '5 days'),
('u0000000-0000-0000-0000-000000000004', 'a0000000-0000-0000-0000-000000000004', 'claude-3-haiku-20240307', 'anthropic', 1000, 500, 1500, 0.000875, 'review', 1200, 'success', NOW() - INTERVAL '4 days'),
('u0000000-0000-0000-0000-000000000005', 'a0000000-0000-0000-0000-000000000005', 'claude-3-haiku-20240307', 'anthropic', 800, 400, 1200, 0.000700, 'execution', 800, 'success', NOW() - INTERVAL '3 days'),
('u0000000-0000-0000-0000-000000000006', 'a0000000-0000-0000-0000-000000000001', 'claude-3-opus-20240229', 'anthropic', 2000, 1000, 3000, 0.1050, 'orchestration', 3500, 'success', NOW() - INTERVAL '2 days'),
('u0000000-0000-0000-0000-000000000007', 'a0000000-0000-0000-0000-000000000002', 'claude-3-sonnet-20240229', 'anthropic', 2500, 2000, 4500, 0.0375, 'code_generation', 4800, 'success', NOW() - INTERVAL '1 day'),
('u0000000-0000-0000-0000-000000000008', 'a0000000-0000-0000-0000-000000000003', 'claude-3-sonnet-20240229', 'anthropic', 2800, 1800, 4600, 0.0354, 'research', 4200, 'success', NOW() - INTERVAL '12 hours'),
('u0000000-0000-0000-0000-000000000009', 'a0000000-0000-0000-0000-000000000004', 'claude-3-haiku-20240307', 'anthropic', 1200, 600, 1800, 0.001050, 'review', 1400, 'success', NOW() - INTERVAL '6 hours'),
('u0000000-0000-0000-0000-000000000010', 'a0000000-0000-0000-0000-000000000005', 'claude-3-haiku-20240307', 'anthropic', 900, 450, 1350, 0.000788, 'execution', 950, 'success', NOW() - INTERVAL '1 hour')
ON CONFLICT (id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE AUDIT LOGS
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO audit_logs (id, entity_type, entity_id, action, actor_type, actor_id, actor_name, new_values, metadata, partition_key) VALUES
(
    'l0000000-0000-0000-0000-000000000001',
    'agent',
    'a0000000-0000-0000-0000-000000000001',
    'create',
    'system',
    'system',
    'System',
    '{"name": "apex-orchestrator", "model": "claude-3-opus-20240229"}'::jsonb,
    '{"source": "seed_data"}'::jsonb,
    CURRENT_DATE
),
(
    'l0000000-0000-0000-0000-000000000002',
    'dag',
    'd0000000-0000-0000-0000-000000000001',
    'create',
    'system',
    'system',
    'System',
    '{"name": "sample-analysis-workflow", "status": "pending"}'::jsonb,
    '{"source": "seed_data"}'::jsonb,
    CURRENT_DATE
)
ON CONFLICT (id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SAMPLE EVENTS (for event sourcing testing)
-- ═══════════════════════════════════════════════════════════════════════════════

INSERT INTO events (event_id, aggregate_type, aggregate_id, event_type, event_data, version, metadata) VALUES
(
    'e0000000-0000-0000-0000-000000000001',
    'dag',
    'd0000000-0000-0000-0000-000000000001',
    'DagCreated',
    '{"name": "sample-analysis-workflow", "total_tasks": 5}'::jsonb,
    1,
    '{"source": "seed_data"}'::jsonb
),
(
    'e0000000-0000-0000-0000-000000000002',
    'agent',
    'a0000000-0000-0000-0000-000000000001',
    'AgentRegistered',
    '{"name": "apex-orchestrator", "model": "claude-3-opus-20240229", "capabilities": ["orchestration", "planning"]}'::jsonb,
    1,
    '{"source": "seed_data"}'::jsonb
)
ON CONFLICT (event_id) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- UPDATE AGENT STATISTICS
-- ═══════════════════════════════════════════════════════════════════════════════

-- Update agents with simulated historical performance data
UPDATE agents SET
    success_count = 50,
    failure_count = 2,
    total_tasks_completed = 52,
    total_tokens_used = 150000,
    total_cost = 5.25,
    reputation_score = 0.9615,
    reliability_score = 0.9800
WHERE id = 'a0000000-0000-0000-0000-000000000001';

UPDATE agents SET
    success_count = 120,
    failure_count = 5,
    total_tasks_completed = 125,
    total_tokens_used = 500000,
    total_cost = 15.75,
    reputation_score = 0.9600,
    reliability_score = 0.9700
WHERE id = 'a0000000-0000-0000-0000-000000000002';

UPDATE agents SET
    success_count = 80,
    failure_count = 3,
    total_tasks_completed = 83,
    total_tokens_used = 400000,
    total_cost = 12.50,
    reputation_score = 0.9639,
    reliability_score = 0.9750
WHERE id = 'a0000000-0000-0000-0000-000000000003';

UPDATE agents SET
    success_count = 200,
    failure_count = 8,
    total_tasks_completed = 208,
    total_tokens_used = 300000,
    total_cost = 0.35,
    reputation_score = 0.9615,
    reliability_score = 0.9650
WHERE id = 'a0000000-0000-0000-0000-000000000004';

UPDATE agents SET
    success_count = 500,
    failure_count = 15,
    total_tasks_completed = 515,
    total_tokens_used = 600000,
    total_cost = 0.75,
    reputation_score = 0.9709,
    reliability_score = 0.9800
WHERE id = 'a0000000-0000-0000-0000-000000000005';

-- ═══════════════════════════════════════════════════════════════════════════════
-- VERIFICATION QUERIES (for testing - can be commented out in production)
-- ═══════════════════════════════════════════════════════════════════════════════

-- Verify data was inserted correctly
DO $$
DECLARE
    v_agents INTEGER;
    v_dags INTEGER;
    v_tasks INTEGER;
    v_config INTEGER;
BEGIN
    SELECT COUNT(*) INTO v_agents FROM agents;
    SELECT COUNT(*) INTO v_dags FROM dags;
    SELECT COUNT(*) INTO v_tasks FROM tasks;
    SELECT COUNT(*) INTO v_config FROM system_config;

    RAISE NOTICE 'Seed data verification:';
    RAISE NOTICE '  - Agents: %', v_agents;
    RAISE NOTICE '  - DAGs: %', v_dags;
    RAISE NOTICE '  - Tasks: %', v_tasks;
    RAISE NOTICE '  - Config entries: %', v_config;
END $$;

-- ═══════════════════════════════════════════════════════════════════════════════
-- GRANT PERMISSIONS (adjust roles as needed for your setup)
-- ═══════════════════════════════════════════════════════════════════════════════

-- These are example grants - adjust based on your actual roles
-- GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO apex_app;
-- GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO apex_app;
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO apex_app;
-- GRANT SELECT ON ALL TABLES IN SCHEMA public TO apex_readonly;
