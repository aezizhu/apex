//! API request handlers with input validation and sanitization.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{AppState, ApiResponse};
use super::middleware::{sanitize_string, ValidationErrors};
use crate::dag::{TaskDAG, Task, TaskId, TaskInput, TaskStatus};
use crate::agents::{Agent, AgentId};
use crate::middleware::auth::AuthContext;

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
// Shared pagination
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationQuery {
    pub page_size: Option<i64>,
    pub page: Option<i64>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Task Handlers
// ═══════════════════════════════════════════════════════════════════════════════

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = pagination.page_size.unwrap_or(50).min(200).max(1);
    let page = pagination.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    match state.db.get_tasks_paginated(limit, offset).await {
        Ok(tasks) => {
            let data: Vec<serde_json::Value> = tasks.iter().map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "dagId": t.dag_id,
                    "name": t.name,
                    "status": t.status,
                    "agentId": t.agent_id,
                    "tokensUsed": t.tokens_used,
                    "costDollars": t.cost_dollars,
                    "createdAt": t.created_at.to_rfc3339(),
                    "startedAt": t.started_at.map(|ts| ts.to_rfc3339()),
                    "completedAt": t.completed_at.map(|ts| ts.to_rfc3339()),
                })
            }).collect();
            Json(ApiResponse::success(data))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub name: String,
    pub instruction: String,
    pub context: Option<serde_json::Value>,
    pub priority: Option<i32>,
    pub limits: Option<ResourceLimitsDto>,
}

impl CreateTaskRequest {
    fn sanitize(&mut self) {
        self.name = sanitize_string(&self.name);
        self.instruction = sanitize_string(&self.instruction);
    }

    fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.name.is_empty() {
            errors.add("name", "must not be empty");
        } else if self.name.len() > 255 {
            errors.add("name", "must be at most 255 characters");
        }
        if self.instruction.is_empty() {
            errors.add("instruction", "must not be empty");
        } else if self.instruction.len() > 50_000 {
            errors.add("instruction", "must be at most 50,000 characters");
        }
        if let Some(priority) = self.priority {
            if priority < 0 || priority > 100 {
                errors.add("priority", "must be between 0 and 100");
            }
        }
        if let Some(ref limits) = self.limits {
            if let Some(token_limit) = limits.token_limit {
                if token_limit == 0 {
                    errors.add("limits.tokenLimit", "must be greater than 0");
                }
            }
            if let Some(cost_limit) = limits.cost_limit {
                if cost_limit <= 0.0 {
                    errors.add("limits.costLimit", "must be greater than 0");
                }
            }
        }
        errors
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLimitsDto {
    pub token_limit: Option<u64>,
    pub cost_limit: Option<f64>,
    pub api_call_limit: Option<u64>,
    pub time_limit_seconds: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub tokens_used: u64,
    pub cost_dollars: f64,
    pub created_at: String,
}

pub async fn create_task(
    State(state): State<AppState>,
    auth: Option<AuthContext>,
    Json(mut req): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            serde_json::to_string(&errors).unwrap_or_else(|_| "Validation failed".to_string()),
            "VALIDATION_ERROR",
        ));
    }

    // Require authentication - task creation must be associated with a user
    let _auth = match auth {
        Some(a) => a,
        None => {
            return Json(ApiResponse::error_with_code(
                "Authentication required to create tasks".to_string(),
                "UNAUTHORIZED",
            ));
        }
    };

    let input = TaskInput {
        instruction: req.instruction,
        context: req.context.unwrap_or(serde_json::Value::Null),
        parameters: serde_json::Value::Null,
        artifacts: vec![],
    };

    let mut task = Task::new(req.name.clone(), input);
    if let Some(priority) = req.priority {
        task.priority = priority;
    }

    // Note: user_id and org_id should be set when the Task struct has these fields

    // Create a DAG for this mission and persist the task
    let dag_id = uuid::Uuid::new_v4();
    if let Err(e) = state.db.insert_dag(dag_id, &req.name).await {
        return Json(ApiResponse::error_with_code(
            format!("Failed to create mission: {e}"),
            "DatabaseError",
        ));
    }
    if let Err(e) = state.db.insert_task(&task, dag_id).await {
        return Json(ApiResponse::error_with_code(
            format!("Failed to persist task: {e}"),
            "DatabaseError",
        ));
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
                "tokensUsed": task.tokens_used,
                "costDollars": task.cost_dollars,
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

pub async fn retry_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_task(TaskId(id)).await {
        Ok(Some(task)) => {
            if task.status != "failed" && task.status != "cancelled" {
                return Json(ApiResponse::error_with_code(
                    format!("Cannot retry task in '{}' status; only 'failed' or 'cancelled' tasks can be retried", task.status),
                    "INVALID_STATE",
                ));
            }
            match state.db.update_task_status(TaskId(id), TaskStatus::Pending).await {
                Ok(_) => Json(ApiResponse::success(serde_json::json!({
                    "id": id,
                    "status": "pending"
                }))),
                Err(e) => Json(ApiResponse::from_apex_error(&e)),
            }
        }
        Ok(None) => Json(ApiResponse::error("Task not found")),
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

impl CreateDagRequest {
    fn sanitize(&mut self) {
        self.name = sanitize_string(&self.name);
        for task in &mut self.tasks {
            task.id = sanitize_string(&task.id);
            task.name = sanitize_string(&task.name);
            task.instruction = sanitize_string(&task.instruction);
        }
        for dep in &mut self.dependencies {
            dep.from = sanitize_string(&dep.from);
            dep.to = sanitize_string(&dep.to);
        }
    }

    fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.name.is_empty() {
            errors.add("name", "must not be empty");
        } else if self.name.len() > 255 {
            errors.add("name", "must be at most 255 characters");
        }
        if self.tasks.is_empty() {
            errors.add("tasks", "must contain at least one task");
        }
        for (i, task) in self.tasks.iter().enumerate() {
            if task.id.is_empty() {
                errors.add(format!("tasks[{}].id", i), "must not be empty");
            }
            if task.name.is_empty() {
                errors.add(format!("tasks[{}].name", i), "must not be empty");
            }
            if task.instruction.is_empty() {
                errors.add(format!("tasks[{}].instruction", i), "must not be empty");
            }
        }
        let task_ids: std::collections::HashSet<&str> = self.tasks.iter().map(|t| t.id.as_str()).collect();
        for (i, dep) in self.dependencies.iter().enumerate() {
            if !task_ids.contains(dep.from.as_str()) {
                errors.add(format!("dependencies[{}].from", i), format!("references unknown task '{}'", dep.from));
            }
            if !task_ids.contains(dep.to.as_str()) {
                errors.add(format!("dependencies[{}].to", i), format!("references unknown task '{}'", dep.to));
            }
            if dep.from == dep.to {
                errors.add(format!("dependencies[{}]", i), "task cannot depend on itself");
            }
        }
        errors
    }
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
#[serde(rename_all = "camelCase")]
pub struct DagResponse {
    pub id: Uuid,
    pub name: String,
    pub task_count: usize,
    pub status: String,
}

pub async fn list_dags(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = pagination.page_size.unwrap_or(20).min(200).max(1);
    let page = pagination.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;

    match state.db.get_dags_paginated(limit, offset).await {
        Ok(dags) => {
            let data: Vec<serde_json::Value> = dags.iter().map(|d| {
                serde_json::json!({
                    "id": d.id,
                    "name": d.name,
                    "status": d.status,
                    "createdAt": d.created_at.to_rfc3339(),
                    "startedAt": d.started_at.map(|ts| ts.to_rfc3339()),
                    "completedAt": d.completed_at.map(|ts| ts.to_rfc3339()),
                })
            }).collect();
            Json(ApiResponse::success(data))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn create_dag(
    State(state): State<AppState>,
    Json(mut req): Json<CreateDagRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            serde_json::to_string(&errors).unwrap_or_else(|_| "Validation failed".to_string()),
            "VALIDATION_ERROR",
        ));
    }

    let mut dag = TaskDAG::new(&req.name);
    let mut task_map = std::collections::HashMap::new();

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
                "createdAt": dag.created_at.to_rfc3339(),
                "startedAt": dag.started_at.map(|t| t.to_rfc3339()),
                "completedAt": dag.completed_at.map(|t| t.to_rfc3339()),
                "nodes": nodes.iter().map(|n| serde_json::json!({
                    "id": n.id,
                    "taskTemplate": n.task_template,
                    "dependsOn": n.depends_on,
                    "isEntry": n.is_entry,
                    "isExit": n.is_exit,
                })).collect::<Vec<_>>(),
                "edges": edges,
                "tasks": tasks.iter().map(|t| serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "status": t.status,
                    "tokensUsed": t.tokens_used,
                    "costDollars": t.cost_dollars,
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
            "dagId": result.dag_id,
            "status": format!("{:?}", result.status),
            "tasksCompleted": result.tasks_completed,
            "tasksFailed": result.tasks_failed,
            "totalTokens": result.total_tokens,
            "totalCost": result.total_cost,
            "durationMs": result.duration_ms,
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
                "startedAt": dag.started_at.map(|t| t.to_rfc3339()),
                "completedAt": dag.completed_at.map(|t| t.to_rfc3339()),
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

pub async fn cancel_dag(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.update_dag_status(id, "cancelled").await {
        Ok(true) => Json(ApiResponse::success(serde_json::json!({
            "id": id,
            "status": "cancelled"
        }))),
        Ok(false) => Json(ApiResponse::error("DAG not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Agent Handlers
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAgentRequest {
    pub name: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub max_load: Option<u32>,
}

impl RegisterAgentRequest {
    fn sanitize(&mut self) {
        self.name = sanitize_string(&self.name);
        self.model = sanitize_string(&self.model);
        if let Some(ref mut prompt) = self.system_prompt {
            *prompt = sanitize_string(prompt);
        }
    }

    fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        if self.name.is_empty() {
            errors.add("name", "must not be empty");
        } else if self.name.len() > 255 {
            errors.add("name", "must be at most 255 characters");
        }
        if self.model.is_empty() {
            errors.add("model", "must not be empty");
        } else if self.model.len() > 255 {
            errors.add("model", "must be at most 255 characters");
        }
        if let Some(ref prompt) = self.system_prompt {
            if prompt.len() > 100_000 {
                errors.add("systemPrompt", "must be at most 100,000 characters");
            }
        }
        if let Some(max_load) = self.max_load {
            if max_load == 0 {
                errors.add("maxLoad", "must be greater than 0");
            } else if max_load > 1000 {
                errors.add("maxLoad", "must be at most 1000");
            }
        }
        errors
    }
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
                    "currentLoad": a.current_load,
                    "maxLoad": a.max_load,
                    "successRate": if a.success_count + a.failure_count > 0 {
                        a.success_count as f64 / (a.success_count + a.failure_count) as f64
                    } else {
                        1.0
                    },
                    "reputationScore": a.reputation_score,
                })
            }).collect();
            Json(ApiResponse::success(agents))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn register_agent(
    State(state): State<AppState>,
    Json(mut req): Json<RegisterAgentRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            serde_json::to_string(&errors).unwrap_or_else(|_| "Validation failed".to_string()),
            "VALIDATION_ERROR",
        ));
    }

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
                "systemPrompt": agent.system_prompt,
                "status": agent.status,
                "currentLoad": agent.current_load,
                "maxLoad": agent.max_load,
                "successCount": agent.success_count,
                "failureCount": agent.failure_count,
                "successRate": success_rate,
                "totalTokens": agent.total_tokens,
                "totalCost": agent.total_cost,
                "reputationScore": agent.reputation_score,
                "createdAt": agent.created_at.to_rfc3339(),
                "lastActiveAt": agent.last_active_at.map(|t| t.to_rfc3339()),
            })))
        }
        Ok(None) => Json(ApiResponse::error("Agent not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn remove_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    state.orchestrator.deregister_agent(AgentId(id));
    match state.db.delete_agent(id).await {
        Ok(true) => Json(ApiResponse::success(serde_json::json!({"id": id, "status": "removed"}))),
        Ok(false) => Json(ApiResponse::error("Agent not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn pause_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.update_agent_status(id, "paused").await {
        Ok(true) => Json(ApiResponse::success(serde_json::json!({
            "id": id,
            "status": "paused"
        }))),
        Ok(false) => Json(ApiResponse::error("Agent not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn resume_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.update_agent_status(id, "idle").await {
        Ok(true) => Json(ApiResponse::success(serde_json::json!({
            "id": id,
            "status": "idle"
        }))),
        Ok(false) => Json(ApiResponse::error("Agent not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentRequest {
    pub system_prompt: Option<String>,
    pub max_load: Option<i32>,
    pub status: Option<String>,
}

pub async fn update_agent(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAgentRequest>,
) -> impl IntoResponse {
    if let Some(ref s) = req.status {
        let valid = ["idle", "busy", "paused", "error"];
        if !valid.contains(&s.as_str()) {
            return Json(ApiResponse::error_with_code(
                format!("status must be one of: {}", valid.join(", ")),
                "VALIDATION_ERROR",
            ));
        }
    }
    if let Some(ml) = req.max_load {
        if ml <= 0 {
            return Json(ApiResponse::error_with_code(
                "maxLoad must be greater than 0",
                "VALIDATION_ERROR",
            ));
        }
    }

    match state.db.update_agent(
        id,
        req.system_prompt.as_deref(),
        req.max_load,
        req.status.as_deref(),
    ).await {
        Ok(true) => {
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
                        "systemPrompt": agent.system_prompt,
                        "status": agent.status,
                        "currentLoad": agent.current_load,
                        "maxLoad": agent.max_load,
                        "successRate": success_rate,
                        "totalTokens": agent.total_tokens,
                        "totalCost": agent.total_cost,
                        "reputationScore": agent.reputation_score,
                        "createdAt": agent.created_at.to_rfc3339(),
                    })))
                }
                _ => Json(ApiResponse::success(serde_json::json!({"id": id, "status": "updated"}))),
            }
        }
        Ok(false) => Json(ApiResponse::error("Agent not found or no fields to update")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
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
                (agent.total_tokens as f64 / agent.success_count as f64) * 0.5
            } else {
                0.0
            };

            Json(ApiResponse::success(serde_json::json!({
                "id": agent.id,
                "name": agent.name,
                "tasksCompleted": tasks_completed,
                "successCount": agent.success_count,
                "failureCount": agent.failure_count,
                "successRate": success_rate,
                "totalTokens": agent.total_tokens,
                "totalCost": agent.total_cost,
                "avgLatencyMs": avg_latency_ms,
                "reputationScore": agent.reputation_score,
                "currentLoad": agent.current_load,
                "maxLoad": agent.max_load,
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
                    "agentId": c.agent_id,
                    "taskId": c.task_id,
                    "tokenLimit": c.token_limit,
                    "costLimit": c.cost_limit,
                    "timeLimitSeconds": c.time_limit_seconds,
                    "apiCallLimit": c.api_call_limit,
                    "tokenUsed": c.token_used,
                    "costUsed": c.cost_used,
                    "apiCallsUsed": c.api_calls_used,
                    "status": c.status,
                    "createdAt": c.created_at.to_rfc3339(),
                    "expiresAt": c.expires_at.map(|t| t.to_rfc3339()),
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
                "agentId": contract.agent_id,
                "taskId": contract.task_id,
                "parentContractId": contract.parent_contract_id,
                "tokenLimit": contract.token_limit,
                "costLimit": contract.cost_limit,
                "timeLimitSeconds": contract.time_limit_seconds,
                "apiCallLimit": contract.api_call_limit,
                "tokenUsed": contract.token_used,
                "costUsed": contract.cost_used,
                "apiCallsUsed": contract.api_calls_used,
                "status": contract.status,
                "createdAt": contract.created_at.to_rfc3339(),
                "expiresAt": contract.expires_at.map(|t| t.to_rfc3339()),
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
                    "activeDags": orchestrator_stats.active_dags,
                    "registeredAgents": orchestrator_stats.registered_agents,
                    "activeContracts": orchestrator_stats.active_contracts,
                    "availableWorkers": orchestrator_stats.available_workers,
                    "maxWorkers": orchestrator_stats.max_workers,
                },
                "database": {
                    "totalTasks": db_stats.total_tasks,
                    "completedTasks": db_stats.completed_tasks,
                    "failedTasks": db_stats.failed_tasks,
                    "runningTasks": db_stats.running_tasks,
                    "totalTokens": db_stats.total_tokens,
                    "totalCost": db_stats.total_cost,
                    "agentCount": db_stats.agent_count,
                }
            })))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

pub async fn get_system_metrics(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_system_stats().await {
        Ok(stats) => {
            let agents = state.db.get_agents().await.unwrap_or_default();
            let active_agents = agents.iter().filter(|a| a.status == "busy").count();
            let total_agents = agents.len();

            let success_rate = if stats.total_tasks > 0 {
                stats.completed_tasks as f64 / stats.total_tasks as f64
            } else {
                0.0
            };

            Json(ApiResponse::success(serde_json::json!({
                "totalTasks": stats.total_tasks,
                "completedTasks": stats.completed_tasks,
                "failedTasks": stats.failed_tasks,
                "runningTasks": stats.running_tasks,
                "totalAgents": total_agents,
                "activeAgents": active_agents,
                "totalTokens": stats.total_tokens,
                "totalCost": stats.total_cost,
                "avgLatencyMs": 0,
                "successRate": success_rate,
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
