//! V1 API routes for Apex Core.
//!
//! This module defines all V1 API routes and their handlers.
//! V1 is the current stable API version.

use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};

use crate::api::{handlers, AppState};
use super::agents as agent_collab;
use super::approvals;
use super::plugins;
use super::settings;

/// V1 API prefix.
pub const V1_PREFIX: &str = "/api/v1";

/// Build the V1 API router.
///
/// All routes are mounted under `/api/v1/`.
///
/// # Endpoints
///
/// ## Tasks
/// - `GET /api/v1/tasks` - List tasks (paginated)
/// - `POST /api/v1/tasks` - Create a new task
/// - `GET /api/v1/tasks/:id` - Get task by ID
/// - `GET /api/v1/tasks/:id/status` - Get task status
/// - `POST /api/v1/tasks/:id/cancel` - Cancel a task
/// - `POST /api/v1/tasks/:id/retry` - Retry a failed/cancelled task
///
/// ## DAGs
/// - `GET /api/v1/dags` - List DAGs (paginated)
/// - `POST /api/v1/dags` - Create a new DAG
/// - `GET /api/v1/dags/:id` - Get DAG by ID
/// - `POST /api/v1/dags/:id/execute` - Execute a DAG
/// - `GET /api/v1/dags/:id/status` - Get DAG execution status
/// - `POST /api/v1/dags/:id/cancel` - Cancel a DAG
///
/// ## Agents
/// - `GET /api/v1/agents` - List all agents
/// - `POST /api/v1/agents` - Register a new agent
/// - `GET /api/v1/agents/:id` - Get agent by ID
/// - `DELETE /api/v1/agents/:id` - Remove an agent
/// - `PATCH /api/v1/agents/:id` - Update agent fields (system prompt, max load, status)
/// - `GET /api/v1/agents/:id/stats` - Get agent statistics
/// - `POST /api/v1/agents/:id/pause` - Pause an agent
/// - `POST /api/v1/agents/:id/resume` - Resume a paused agent
///
/// ## Contracts
/// - `GET /api/v1/contracts` - List all contracts
/// - `GET /api/v1/contracts/:id` - Get contract by ID
///
/// ## Plugins
/// - `GET /api/v1/plugins` - List all plugins
/// - `GET /api/v1/plugins/:name` - Get plugin details
/// - `POST /api/v1/plugins/discover` - Trigger plugin discovery
/// - `POST /api/v1/plugins/:name/install` - Install a plugin
/// - `POST /api/v1/plugins/:name/enable` - Enable a plugin
/// - `POST /api/v1/plugins/:name/disable` - Disable a plugin
/// - `POST /api/v1/plugins/:name/uninstall` - Uninstall a plugin
///
/// ## Settings
/// - `GET /api/v1/settings` - Get system settings
/// - `PUT /api/v1/settings` - Update system settings
/// - `GET /api/v1/settings/api-keys` - List API keys (masked)
/// - `POST /api/v1/settings/api-keys` - Create an API key
/// - `DELETE /api/v1/settings/api-keys/:id` - Revoke an API key
///
/// ## Approvals
/// - `GET /api/v1/approvals` - List pending approvals
/// - `GET /api/v1/approvals/history` - List approval history
/// - `GET /api/v1/approvals/:id` - Get approval details
/// - `POST /api/v1/approvals/:id/decide` - Approve or deny
///
/// ## Agent Delegation
/// - `POST /api/v1/agents/:id/delegate` - Delegate a task to another agent
///
/// ## Agent Collaboration
/// - `GET /api/v1/agents/:id/memory` - List agent memories
/// - `POST /api/v1/agents/:id/memory` - Store a new memory
/// - `DELETE /api/v1/agents/:id/memory/:memory_id` - Delete a memory
///
/// ## Threads
/// - `GET /api/v1/threads` - List conversation threads
/// - `POST /api/v1/threads` - Create a new thread
/// - `GET /api/v1/threads/:id` - Get thread details
/// - `POST /api/v1/threads/:id/messages` - Post a message to a thread
/// - `POST /api/v1/threads/:id/complete` - Mark a thread as completed
///
/// ## System
/// - `GET /api/v1/stats` - Get system statistics
pub fn v1_router() -> Router<AppState> {
    Router::new()
        // Task endpoints
        .route("/tasks", get(handlers::list_tasks))
        .route("/tasks", post(handlers::create_task))
        .route("/tasks/:id", get(handlers::get_task))
        .route("/tasks/:id/status", get(handlers::get_task_status))
        .route("/tasks/:id/cancel", post(handlers::cancel_task))
        .route("/tasks/:id/retry", post(handlers::retry_task))
        // DAG endpoints
        .route("/dags", get(handlers::list_dags))
        .route("/dags", post(handlers::create_dag))
        .route("/dags/:id", get(handlers::get_dag))
        .route("/dags/:id/execute", post(handlers::execute_dag))
        .route("/dags/:id/status", get(handlers::get_dag_status))
        .route("/dags/:id/cancel", post(handlers::cancel_dag))
        // Agent endpoints
        .route("/agents", get(handlers::list_agents))
        .route("/agents", post(handlers::register_agent))
        .route("/agents/:id", get(handlers::get_agent))
        .route("/agents/:id", delete(handlers::remove_agent))
        .route("/agents/:id", patch(handlers::update_agent))
        .route("/agents/:id/stats", get(handlers::get_agent_stats))
        .route("/agents/:id/pause", post(handlers::pause_agent))
        .route("/agents/:id/resume", post(handlers::resume_agent))
        // Contract endpoints
        .route("/contracts", get(handlers::list_contracts))
        .route("/contracts/:id", get(handlers::get_contract))
        // Plugin endpoints
        .route("/plugins", get(plugins::list_plugins))
        .route("/plugins/discover", post(plugins::discover_plugins))
        .route("/plugins/:name", get(plugins::get_plugin))
        .route("/plugins/:name/install", post(plugins::install_plugin))
        .route("/plugins/:name/enable", post(plugins::enable_plugin))
        .route("/plugins/:name/disable", post(plugins::disable_plugin))
        .route("/plugins/:name/uninstall", post(plugins::uninstall_plugin))
        // Settings endpoints
        .route("/settings", get(settings::get_settings))
        .route("/settings", put(settings::update_settings))
        .route("/settings/api-keys", get(settings::get_api_keys))
        .route("/settings/api-keys", post(settings::create_api_key))
        .route("/settings/api-keys/:id", delete(settings::revoke_api_key))
        // Approval endpoints
        .route("/approvals", get(approvals::list_approvals))
        .route("/approvals/history", get(approvals::list_approval_history))
        .route("/approvals/:id", get(approvals::get_approval))
        .route("/approvals/:id/decide", post(approvals::decide_approval))
        // Agent delegation endpoint
        .route("/agents/:id/delegate", post(agent_collab::delegate_task))
        // Agent collaboration endpoints
        .route("/agents/:id/memory", get(agent_collab::get_agent_memory))
        .route("/agents/:id/memory", post(agent_collab::store_agent_memory))
        .route("/agents/:id/memory/:memory_id", delete(agent_collab::delete_agent_memory))
        // Thread endpoints
        .route("/threads", get(agent_collab::list_threads))
        .route("/threads", post(agent_collab::create_thread))
        .route("/threads/:id", get(agent_collab::get_thread))
        .route("/threads/:id/messages", post(agent_collab::post_thread_message))
        .route("/threads/:id/complete", post(agent_collab::complete_thread))
        // Metrics & Stats
        .route("/metrics", get(handlers::get_system_metrics))
        .route("/stats", get(handlers::get_system_stats))
}

/// V1 API route constants for use in clients and documentation.
pub mod paths {
    // Task routes
    pub const TASKS: &str = "/api/v1/tasks";
    pub const TASK: &str = "/api/v1/tasks/:id";
    pub const TASK_STATUS: &str = "/api/v1/tasks/:id/status";
    pub const TASK_CANCEL: &str = "/api/v1/tasks/:id/cancel";
    pub const TASK_RETRY: &str = "/api/v1/tasks/:id/retry";

    // DAG routes
    pub const DAGS: &str = "/api/v1/dags";
    pub const DAG: &str = "/api/v1/dags/:id";
    pub const DAG_EXECUTE: &str = "/api/v1/dags/:id/execute";
    pub const DAG_STATUS: &str = "/api/v1/dags/:id/status";
    pub const DAG_CANCEL: &str = "/api/v1/dags/:id/cancel";

    // Agent routes
    pub const AGENTS: &str = "/api/v1/agents";
    pub const AGENT: &str = "/api/v1/agents/:id";
    pub const AGENT_STATS: &str = "/api/v1/agents/:id/stats";
    pub const AGENT_PAUSE: &str = "/api/v1/agents/:id/pause";
    pub const AGENT_RESUME: &str = "/api/v1/agents/:id/resume";

    // Contract routes
    pub const CONTRACTS: &str = "/api/v1/contracts";
    pub const CONTRACT: &str = "/api/v1/contracts/:id";

    // Plugin routes
    pub const PLUGINS: &str = "/api/v1/plugins";
    pub const PLUGIN: &str = "/api/v1/plugins/:name";
    pub const PLUGIN_DISCOVER: &str = "/api/v1/plugins/discover";
    pub const PLUGIN_INSTALL: &str = "/api/v1/plugins/:name/install";
    pub const PLUGIN_ENABLE: &str = "/api/v1/plugins/:name/enable";
    pub const PLUGIN_DISABLE: &str = "/api/v1/plugins/:name/disable";
    pub const PLUGIN_UNINSTALL: &str = "/api/v1/plugins/:name/uninstall";

    // Settings routes
    pub const SETTINGS: &str = "/api/v1/settings";
    pub const SETTINGS_API_KEYS: &str = "/api/v1/settings/api-keys";
    pub const SETTINGS_API_KEY: &str = "/api/v1/settings/api-keys/:id";

    // Approval routes
    pub const APPROVALS: &str = "/api/v1/approvals";
    pub const APPROVALS_HISTORY: &str = "/api/v1/approvals/history";
    pub const APPROVAL: &str = "/api/v1/approvals/:id";
    pub const APPROVAL_DECIDE: &str = "/api/v1/approvals/:id/decide";

    // Agent delegation routes
    pub const AGENT_DELEGATE: &str = "/api/v1/agents/:id/delegate";

    // Agent collaboration routes
    pub const AGENT_MEMORY: &str = "/api/v1/agents/:id/memory";
    pub const AGENT_MEMORY_ITEM: &str = "/api/v1/agents/:id/memory/:memory_id";

    // Thread routes
    pub const THREADS: &str = "/api/v1/threads";
    pub const THREAD: &str = "/api/v1/threads/:id";
    pub const THREAD_MESSAGES: &str = "/api/v1/threads/:id/messages";
    pub const THREAD_COMPLETE: &str = "/api/v1/threads/:id/complete";

    // System routes
    pub const STATS: &str = "/api/v1/stats";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_constants() {
        assert!(paths::TASKS.starts_with("/api/v1"));
        assert!(paths::DAGS.starts_with("/api/v1"));
        assert!(paths::AGENTS.starts_with("/api/v1"));
    }
}
