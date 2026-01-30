//! API request handlers.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AppState, ApiResponse};
use crate::dag::{TaskDAG, Task, TaskId, TaskInput, TaskStatus};
use crate::agents::Agent;

// ═══════════════════════════════════════════════════════════════════════════════
// Health Check
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Task Handlers
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub instruction: String,
    pub context: Option<serde_json::Value>,
    pub priority: Option<i32>,
    pub limits: Option<ResourceLimitsDto>,
}

#[derive(Deserialize, Serialize)]
pub struct ResourceLimitsDto {
    pub token_limit: Option<u64>,
    pub cost_limit: Option<f64>,
    pub api_call_limit: Option<u64>,
    pub time_limit_seconds: Option<u64>,
}

#[derive(Serialize)]
pub struct TaskResponse {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub tokens_used: u64,
    pub cost_dollars: f64,
    pub created_at: String,
}

pub async fn create_task(
    State(_state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    let input = TaskInput {
        instruction: req.instruction,
        context: req.context.unwrap_or(serde_json::Value::Null),
        parameters: serde_json::Value::Null,
        artifacts: vec![],
    };

    let mut task = Task::new(req.name, input);
    if let Some(priority) = req.priority {
        task.priority = priority;
    }

    let response = TaskResponse {
        id: task.id.0,
        name: task.name.clone(),
        status: task.status.as_str().to_string(),
        tokens_used: task.tokens_used,
        cost_dollars: task.cost_dollars,
        created_at: task.created_at.to_rfc3339(),
    };

    Json(ApiResponse::success(response))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_task(TaskId(id)).await {
        Ok(Some(task)) => {
            let response = TaskResponse {
                id: task.id,
                name: task.name,
                status: task.status,
                tokens_used: task.tokens_used as u64,
                cost_dollars: task.cost_dollars,
                created_at: task.created_at.to_rfc3339(),
            };
            Json(ApiResponse::success(response))
        }
        Ok(None) => Json(ApiResponse::error("Task not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn get_task_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_task(TaskId(id)).await {
        Ok(Some(task)) => {
            Json(ApiResponse::success(serde_json::json!({
                "id": task.id,
                "status": task.status,
                "tokens_used": task.tokens_used,
                "cost_dollars": task.cost_dollars,
            })))
        }
        Ok(None) => Json(ApiResponse::error("Task not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.update_task_status(TaskId(id), TaskStatus::Cancelled).await {
        Ok(_) => Json(ApiResponse::success(serde_json::json!({
            "id": id,
            "status": "cancelled"
        }))),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DAG Handlers
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct CreateDagRequest {
    pub name: String,
    pub tasks: Vec<DagTaskRequest>,
    pub dependencies: Vec<DependencyRequest>,
}

#[derive(Deserialize)]
pub struct DagTaskRequest {
    pub id: String,
    pub name: String,
    pub instruction: String,
}

#[derive(Deserialize)]
pub struct DependencyRequest {
    pub from: String,
    pub to: String,
}

#[derive(Serialize)]
pub struct DagResponse {
    pub id: Uuid,
    pub name: String,
    pub task_count: usize,
    pub status: String,
}

pub async fn create_dag(
    State(state): State<AppState>,
    Json(req): Json<CreateDagRequest>,
) -> impl IntoResponse {
    let mut dag = TaskDAG::new(&req.name);

    // Track task IDs for dependency resolution
    let mut task_map = std::collections::HashMap::new();

    // Add tasks
    for task_req in &req.tasks {
        let input = TaskInput {
            instruction: task_req.instruction.clone(),
            context: serde_json::Value::Null,
            parameters: serde_json::Value::Null,
            artifacts: vec![],
        };
        let task = Task::new(&task_req.name, input);
        let task_id = task.id;

        if let Err(e) = dag.add_task(task) {
            return Json(ApiResponse::from_apex_error(&e));
        }

        task_map.insert(task_req.id.clone(), task_id);
    }

    // Add dependencies
    for dep in &req.dependencies {
        let from_id = match task_map.get(&dep.from) {
            Some(id) => *id,
            None => return Json(ApiResponse::error(format!("Task not found: {}", dep.from))),
        };
        let to_id = match task_map.get(&dep.to) {
            Some(id) => *id,
            None => return Json(ApiResponse::error(format!("Task not found: {}", dep.to))),
        };

        if let Err(e) = dag.add_dependency(from_id, to_id) {
            return Json(ApiResponse::from_apex_error(&e));
        }
    }

    let response = DagResponse {
        id: dag.id(),
        name: dag.name().to_string(),
        task_count: req.tasks.len(),
        status: "created".to_string(),
    };

    // Submit to orchestrator
    match state.orchestrator.submit_dag(dag).await {
        Ok(_) => Json(ApiResponse::success(response)),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn get_dag(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_dag(id).await {
        Ok(Some(dag)) => {
            // Fetch nodes and tasks for this DAG
            let nodes = state.db.get_dag_nodes(id).await.unwrap_or_default();
            let tasks = state.db.get_dag_tasks(id).await.unwrap_or_default();

            let edges: Vec<serde_json::Value> = nodes
                .iter()
                .filter_map(|node| {
                    node.depends_on.as_ref().map(|deps| {
                        deps.iter()
                            .map(|dep| {
                                serde_json::json!({
                                    "from": dep,
                                    "to": node.id,
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .flatten()
                .collect();

            Json(ApiResponse::success(serde_json::json!({
                "id": dag.id,
                "name": dag.name,
                "status": dag.status,
                "metadata": dag.metadata,
                "created_at": dag.created_at.to_rfc3339(),
                "started_at": dag.started_at.map(|t| t.to_rfc3339()),
                "completed_at": dag.completed_at.map(|t| t.to_rfc3339()),
                "nodes": nodes.iter().map(|n| serde_json::json!({
                    "id": n.id,
                    "task_template": n.task_template,
                    "depends_on": n.depends_on,
                    "is_entry": n.is_entry,
                    "is_exit": n.is_exit,
                })).collect::<Vec<_>>(),
                "edges": edges,
                "tasks": tasks.iter().map(|t| serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "status": t.status,
                    "tokens_used": t.tokens_used,
                    "cost_dollars": t.cost_dollars,
                })).collect::<Vec<_>>(),
            })))
        }
        Ok(None) => Json(ApiResponse::error("DAG not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn execute_dag(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.orchestrator.execute_dag(id).await {
        Ok(result) => Json(ApiResponse::success(serde_json::json!({
            "dag_id": result.dag_id,
            "status": format!("{:?}", result.status),
            "tasks_completed": result.tasks_completed,
            "tasks_failed": result.tasks_failed,
            "total_tokens": result.total_tokens,
            "total_cost": result.total_cost,
            "duration_ms": result.duration_ms,
        }))),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn get_dag_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_dag(id).await {
        Ok(Some(dag)) => {
            let tasks = state.db.get_dag_tasks(id).await.unwrap_or_default();
            let total = tasks.len();
            let completed = tasks.iter().filter(|t| t.status == "completed").count();
            let failed = tasks.iter().filter(|t| t.status == "failed").count();
            let running = tasks.iter().filter(|t| t.status == "running").count();
            let pending = tasks.iter().filter(|t| t.status == "pending" || t.status == "ready").count();

            Json(ApiResponse::success(serde_json::json!({
                "id": dag.id,
                "name": dag.name,
                "status": dag.status,
                "started_at": dag.started_at.map(|t| t.to_rfc3339()),
                "completed_at": dag.completed_at.map(|t| t.to_rfc3339()),
                "tasks": {
                    "total": total,
                    "completed": completed,
                    "failed": failed,
                    "running": running,
                    "pending": pending,
                }
            })))
        }
        Ok(None) => Json(ApiResponse::error("DAG not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Agent Handlers
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct RegisterAgentRequest {
    pub name: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub max_load: Option<u32>,
}

pub async fn list_agents(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_agents().await {
        Ok(agents) => {
            let agents: Vec<serde_json::Value> = agents.iter().map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "name": a.name,
                    "model": a.model,
                    "status": a.status,
                    "current_load": a.current_load,
                    "max_load": a.max_load,
                    "success_rate": if a.success_count + a.failure_count > 0 {
                        a.success_count as f64 / (a.success_count + a.failure_count) as f64
                    } else {
                        1.0
                    },
                    "reputation_score": a.reputation_score,
                })
            }).collect();
            Json(ApiResponse::success(agents))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn register_agent(
    State(state): State<AppState>,
    Json(req): Json<RegisterAgentRequest>,
) -> impl IntoResponse {
    let mut agent = Agent::new(&req.name, &req.model);

    if let Some(prompt) = req.system_prompt {
        agent = agent.with_system_prompt(prompt);
    }
    if let Some(max) = req.max_load {
        agent = agent.with_max_load(max);
    }

    let stats = agent.stats();
    let agent_id = state.orchestrator.register_agent(agent);

    Json(ApiResponse::success(serde_json::json!({
        "id": agent_id.0,
        "name": stats.name,
        "model": stats.model,
        "status": "registered"
    })))
}

pub async fn get_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_agent(id).await {
        Ok(Some(agent)) => {
            let success_rate = if agent.success_count + agent.failure_count > 0 {
                agent.success_count as f64 / (agent.success_count + agent.failure_count) as f64
            } else {
                1.0
            };

            Json(ApiResponse::success(serde_json::json!({
                "id": agent.id,
                "name": agent.name,
                "model": agent.model,
                "system_prompt": agent.system_prompt,
                "status": agent.status,
                "current_load": agent.current_load,
                "max_load": agent.max_load,
                "success_count": agent.success_count,
                "failure_count": agent.failure_count,
                "success_rate": success_rate,
                "total_tokens": agent.total_tokens,
                "total_cost": agent.total_cost,
                "reputation_score": agent.reputation_score,
                "created_at": agent.created_at.to_rfc3339(),
                "last_active_at": agent.last_active_at.map(|t| t.to_rfc3339()),
            })))
        }
        Ok(None) => Json(ApiResponse::error("Agent not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn remove_agent(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    // TODO: Implement agent removal
    Json(ApiResponse::success(serde_json::json!({
        "id": id,
        "status": "removed"
    })))
}

pub async fn get_agent_stats(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_agent(id).await {
        Ok(Some(agent)) => {
            let tasks_completed = agent.success_count + agent.failure_count;
            let success_rate = if tasks_completed > 0 {
                agent.success_count as f64 / tasks_completed as f64
            } else {
                1.0
            };
            let avg_latency_ms = if agent.success_count > 0 {
                // Approximate from total tokens (rough heuristic)
                (agent.total_tokens as f64 / agent.success_count as f64) * 0.5
            } else {
                0.0
            };

            Json(ApiResponse::success(serde_json::json!({
                "id": agent.id,
                "name": agent.name,
                "tasks_completed": tasks_completed,
                "success_count": agent.success_count,
                "failure_count": agent.failure_count,
                "success_rate": success_rate,
                "total_tokens": agent.total_tokens,
                "total_cost": agent.total_cost,
                "avg_latency_ms": avg_latency_ms,
                "reputation_score": agent.reputation_score,
                "current_load": agent.current_load,
                "max_load": agent.max_load,
            })))
        }
        Ok(None) => Json(ApiResponse::error("Agent not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contract Handlers
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn list_contracts(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_contracts(100, 0).await {
        Ok(contracts) => {
            let contracts: Vec<serde_json::Value> = contracts.iter().map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "agent_id": c.agent_id,
                    "task_id": c.task_id,
                    "token_limit": c.token_limit,
                    "cost_limit": c.cost_limit,
                    "time_limit_seconds": c.time_limit_seconds,
                    "api_call_limit": c.api_call_limit,
                    "token_used": c.token_used,
                    "cost_used": c.cost_used,
                    "api_calls_used": c.api_calls_used,
                    "status": c.status,
                    "created_at": c.created_at.to_rfc3339(),
                    "expires_at": c.expires_at.map(|t| t.to_rfc3339()),
                })
            }).collect();
            Json(ApiResponse::success(contracts))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn get_contract(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_contract(id).await {
        Ok(Some(contract)) => {
            Json(ApiResponse::success(serde_json::json!({
                "id": contract.id,
                "agent_id": contract.agent_id,
                "task_id": contract.task_id,
                "parent_contract_id": contract.parent_contract_id,
                "token_limit": contract.token_limit,
                "cost_limit": contract.cost_limit,
                "time_limit_seconds": contract.time_limit_seconds,
                "api_call_limit": contract.api_call_limit,
                "token_used": contract.token_used,
                "cost_used": contract.cost_used,
                "api_calls_used": contract.api_calls_used,
                "status": contract.status,
                "created_at": contract.created_at.to_rfc3339(),
                "expires_at": contract.expires_at.map(|t| t.to_rfc3339()),
            })))
        }
        Ok(None) => Json(ApiResponse::error("Contract not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Stats and Metrics
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn get_system_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let orchestrator_stats = state.orchestrator.stats();

    match state.db.get_system_stats().await {
        Ok(db_stats) => {
            Json(ApiResponse::success(serde_json::json!({
                "orchestrator": {
                    "active_dags": orchestrator_stats.active_dags,
                    "registered_agents": orchestrator_stats.registered_agents,
                    "active_contracts": orchestrator_stats.active_contracts,
                    "available_workers": orchestrator_stats.available_workers,
                    "max_workers": orchestrator_stats.max_workers,
                },
                "database": {
                    "total_tasks": db_stats.total_tasks,
                    "completed_tasks": db_stats.completed_tasks,
                    "failed_tasks": db_stats.failed_tasks,
                    "running_tasks": db_stats.running_tasks,
                    "total_tokens": db_stats.total_tokens,
                    "total_cost": db_stats.total_cost,
                    "agent_count": db_stats.agent_count,
                }
            })))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn prometheus_metrics() -> impl IntoResponse {
    let registry = crate::telemetry::metrics::MetricsRegistry::global();
    let body = registry.render();

    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}
