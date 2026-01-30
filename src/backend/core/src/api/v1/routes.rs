//! V1 API routes for Apex Core.
//!
//! This module defines all V1 API routes and their handlers.
//! V1 is the current stable API version.

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::api::{handlers, AppState};

/// V1 API prefix.
pub const V1_PREFIX: &str = "/api/v1";

/// Build the V1 API router.
///
/// All routes are mounted under `/api/v1/`.
///
/// # Endpoints
///
/// ## Tasks
/// - `POST /api/v1/tasks` - Create a new task
/// - `GET /api/v1/tasks/:id` - Get task by ID
/// - `GET /api/v1/tasks/:id/status` - Get task status
/// - `POST /api/v1/tasks/:id/cancel` - Cancel a task
///
/// ## DAGs
/// - `POST /api/v1/dags` - Create a new DAG
/// - `GET /api/v1/dags/:id` - Get DAG by ID
/// - `POST /api/v1/dags/:id/execute` - Execute a DAG
/// - `GET /api/v1/dags/:id/status` - Get DAG execution status
///
/// ## Agents
/// - `GET /api/v1/agents` - List all agents
/// - `POST /api/v1/agents` - Register a new agent
/// - `GET /api/v1/agents/:id` - Get agent by ID
/// - `DELETE /api/v1/agents/:id` - Remove an agent
/// - `GET /api/v1/agents/:id/stats` - Get agent statistics
///
/// ## Contracts
/// - `GET /api/v1/contracts` - List all contracts
/// - `GET /api/v1/contracts/:id` - Get contract by ID
///
/// ## System
/// - `GET /api/v1/stats` - Get system statistics
pub fn v1_router() -> Router<AppState> {
    Router::new()
        // Task endpoints
        .route("/tasks", post(handlers::create_task))
        .route("/tasks/:id", get(handlers::get_task))
        .route("/tasks/:id/status", get(handlers::get_task_status))
        .route("/tasks/:id/cancel", post(handlers::cancel_task))
        // DAG endpoints
        .route("/dags", post(handlers::create_dag))
        .route("/dags/:id", get(handlers::get_dag))
        .route("/dags/:id/execute", post(handlers::execute_dag))
        .route("/dags/:id/status", get(handlers::get_dag_status))
        // Agent endpoints
        .route("/agents", get(handlers::list_agents))
        .route("/agents", post(handlers::register_agent))
        .route("/agents/:id", get(handlers::get_agent))
        .route("/agents/:id", delete(handlers::remove_agent))
        .route("/agents/:id/stats", get(handlers::get_agent_stats))
        // Contract endpoints
        .route("/contracts", get(handlers::list_contracts))
        .route("/contracts/:id", get(handlers::get_contract))
        // Stats
        .route("/stats", get(handlers::get_system_stats))
}

/// V1 API route constants for use in clients and documentation.
pub mod paths {
    // Task routes
    pub const TASKS: &str = "/api/v1/tasks";
    pub const TASK: &str = "/api/v1/tasks/:id";
    pub const TASK_STATUS: &str = "/api/v1/tasks/:id/status";
    pub const TASK_CANCEL: &str = "/api/v1/tasks/:id/cancel";

    // DAG routes
    pub const DAGS: &str = "/api/v1/dags";
    pub const DAG: &str = "/api/v1/dags/:id";
    pub const DAG_EXECUTE: &str = "/api/v1/dags/:id/execute";
    pub const DAG_STATUS: &str = "/api/v1/dags/:id/status";

    // Agent routes
    pub const AGENTS: &str = "/api/v1/agents";
    pub const AGENT: &str = "/api/v1/agents/:id";
    pub const AGENT_STATS: &str = "/api/v1/agents/:id/stats";

    // Contract routes
    pub const CONTRACTS: &str = "/api/v1/contracts";
    pub const CONTRACT: &str = "/api/v1/contracts/:id";

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
