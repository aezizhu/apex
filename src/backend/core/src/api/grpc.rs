//! gRPC Service Implementation for Apex Orchestrator
//!
//! This module implements the ApexOrchestrator gRPC service defined in apex.proto.
//! It provides a high-performance, streaming-capable API for task orchestration,
//! DAG management, and agent coordination.

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures::Stream;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::agents::{AgentId, AgentStatus as RustAgentStatus, AgentStats as RustAgentStats, Tool as RustTool, Agent as RustAgent};
use crate::contracts::ResourceLimits as RustResourceLimits;
use crate::dag::{Artifact as RustArtifact, Task as RustTask, TaskDAG, TaskId, TaskInput as RustTaskInput, TaskStatus as RustTaskStatus};
use crate::error::{ApexError, ErrorCode};
use crate::orchestrator::{DagExecutionStatus, SwarmOrchestrator};

// Include the generated protobuf code
pub mod proto {
    tonic::include_proto!("apex.v1");
}

use proto::apex_orchestrator_server::ApexOrchestrator;
use proto::*;

// ═══════════════════════════════════════════════════════════════════════════════
// Type Conversions
// ═══════════════════════════════════════════════════════════════════════════════

/// Convert chrono DateTime to proto Timestamp
fn to_proto_timestamp(dt: chrono::DateTime<chrono::Utc>) -> Timestamp {
    Timestamp {
        seconds: dt.timestamp(),
        nanos: dt.timestamp_subsec_nanos() as i32,
    }
}

/// Convert proto Timestamp to chrono DateTime
#[allow(dead_code)]
fn from_proto_timestamp(ts: &Timestamp) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
        .unwrap_or_else(chrono::Utc::now)
}

/// Convert Rust TaskStatus to proto TaskStatus
fn to_proto_task_status(status: &RustTaskStatus) -> i32 {
    match status {
        RustTaskStatus::Pending => TaskStatus::Pending as i32,
        RustTaskStatus::Ready => TaskStatus::Ready as i32,
        RustTaskStatus::Running => TaskStatus::Running as i32,
        RustTaskStatus::Completed => TaskStatus::Completed as i32,
        RustTaskStatus::Failed => TaskStatus::Failed as i32,
        RustTaskStatus::Cancelled => TaskStatus::Cancelled as i32,
    }
}

/// Convert proto TaskStatus to Rust TaskStatus
fn from_proto_task_status(status: i32) -> RustTaskStatus {
    match TaskStatus::try_from(status) {
        Ok(TaskStatus::Pending) => RustTaskStatus::Pending,
        Ok(TaskStatus::Ready) => RustTaskStatus::Ready,
        Ok(TaskStatus::Running) => RustTaskStatus::Running,
        Ok(TaskStatus::Completed) => RustTaskStatus::Completed,
        Ok(TaskStatus::Failed) => RustTaskStatus::Failed,
        Ok(TaskStatus::Cancelled) => RustTaskStatus::Cancelled,
        _ => RustTaskStatus::Pending,
    }
}

/// Convert Rust AgentStatus to proto AgentStatus
fn to_proto_agent_status(status: &RustAgentStatus) -> i32 {
    match status {
        RustAgentStatus::Idle => AgentStatus::Idle as i32,
        RustAgentStatus::Busy => AgentStatus::Busy as i32,
        RustAgentStatus::Error => AgentStatus::Error as i32,
        RustAgentStatus::Paused => AgentStatus::Paused as i32,
    }
}

/// Convert proto AgentStatus to Rust AgentStatus
fn from_proto_agent_status(status: i32) -> RustAgentStatus {
    match AgentStatus::try_from(status) {
        Ok(AgentStatus::Idle) => RustAgentStatus::Idle,
        Ok(AgentStatus::Busy) => RustAgentStatus::Busy,
        Ok(AgentStatus::Error) => RustAgentStatus::Error,
        Ok(AgentStatus::Paused) => RustAgentStatus::Paused,
        _ => RustAgentStatus::Idle,
    }
}

/// Convert DagExecutionStatus to proto DagStatus
fn to_proto_dag_status(status: &DagExecutionStatus) -> i32 {
    match status {
        DagExecutionStatus::Completed => DagStatus::Completed as i32,
        DagExecutionStatus::PartialFailure => DagStatus::PartialFailure as i32,
        DagExecutionStatus::Failed => DagStatus::Failed as i32,
        DagExecutionStatus::Cancelled => DagStatus::Cancelled as i32,
    }
}

/// Convert Rust Task to proto Task
fn to_proto_task(task: &RustTask) -> Task {
    Task {
        id: task.id.0.to_string(),
        parent_id: task.parent_id.map(|id| id.0.to_string()),
        name: task.name.clone(),
        status: to_proto_task_status(&task.status),
        priority: task.priority,
        input: Some(TaskInput {
            instruction: task.input.instruction.clone(),
            context_json: task.input.context.to_string(),
            parameters_json: task.input.parameters.to_string(),
            artifacts: task.input.artifacts.iter().map(|a| Artifact {
                name: a.name.clone(),
                mime_type: a.mime_type.clone(),
                size_bytes: a.size_bytes,
                url: a.url.clone(),
                content_hash: a.content_hash.clone(),
            }).collect(),
        }),
        output: task.output.as_ref().map(|o| TaskOutput {
            result: o.result.clone(),
            data_json: o.data.to_string(),
            artifacts: o.artifacts.iter().map(|a| Artifact {
                name: a.name.clone(),
                mime_type: a.mime_type.clone(),
                size_bytes: a.size_bytes,
                url: a.url.clone(),
                content_hash: a.content_hash.clone(),
            }).collect(),
            reasoning: o.reasoning.clone(),
        }),
        error: task.error.clone(),
        agent_id: task.agent_id.map(|id| id.to_string()),
        contract_id: task.contract_id.map(|id| id.to_string()),
        retry_count: task.retry_count,
        max_retries: task.max_retries,
        tokens_used: task.tokens_used,
        cost_microdollars: (task.cost_dollars * 1_000_000.0) as i64,
        created_at: Some(to_proto_timestamp(task.created_at)),
        started_at: task.started_at.map(to_proto_timestamp),
        completed_at: task.completed_at.map(to_proto_timestamp),
        trace_id: task.trace_id.clone(),
        span_id: task.span_id.clone(),
    }
}

/// Convert proto TaskInput to Rust TaskInput
fn from_proto_task_input(input: &TaskInput) -> RustTaskInput {
    RustTaskInput {
        instruction: input.instruction.clone(),
        context: serde_json::from_str(&input.context_json).unwrap_or_default(),
        parameters: serde_json::from_str(&input.parameters_json).unwrap_or_default(),
        artifacts: input.artifacts.iter().map(|a| RustArtifact {
            name: a.name.clone(),
            mime_type: a.mime_type.clone(),
            size_bytes: a.size_bytes,
            url: a.url.clone(),
            content_hash: a.content_hash.clone(),
        }).collect(),
    }
}

/// Convert proto ResourceLimits to Rust ResourceLimits
#[allow(dead_code)]
fn from_proto_resource_limits(limits: &ResourceLimits) -> RustResourceLimits {
    RustResourceLimits {
        token_limit: limits.token_limit,
        cost_limit: limits.cost_limit_microdollars as f64 / 1_000_000.0,
        api_call_limit: limits.api_call_limit,
        time_limit_seconds: limits.time_limit_seconds,
    }
}

/// Convert Rust ResourceLimits to proto ResourceLimits
#[allow(dead_code)]
fn to_proto_resource_limits(limits: &RustResourceLimits) -> ResourceLimits {
    ResourceLimits {
        token_limit: limits.token_limit,
        cost_limit_microdollars: (limits.cost_limit * 1_000_000.0) as i64,
        api_call_limit: limits.api_call_limit,
        time_limit_seconds: limits.time_limit_seconds,
    }
}

/// Convert Rust AgentStats to proto Agent (partial conversion from stats)
#[allow(dead_code)]
fn agent_from_stats(stats: &RustAgentStats) -> Agent {
    Agent {
        id: stats.id.0.to_string(),
        name: stats.name.clone(),
        model: stats.model.clone(),
        system_prompt: String::new(),
        tools: vec![],
        status: to_proto_agent_status(&stats.status),
        current_load: stats.current_load,
        max_load: stats.max_load,
        created_at: Some(to_proto_timestamp(Utc::now())),
        last_active_at: None,
    }
}

/// Convert Rust AgentStats to proto AgentStats
#[allow(dead_code)]
fn to_proto_agent_stats(stats: &RustAgentStats) -> proto::AgentStats {
    proto::AgentStats {
        id: stats.id.0.to_string(),
        name: stats.name.clone(),
        model: stats.model.clone(),
        status: to_proto_agent_status(&stats.status),
        current_load: stats.current_load,
        max_load: stats.max_load,
        success_count: stats.success_count,
        failure_count: stats.failure_count,
        success_rate_millionths: (stats.success_rate * 1_000_000.0) as u64,
        total_tokens: stats.total_tokens,
        total_cost_microdollars: (stats.total_cost * 1_000_000.0) as i64,
        reputation_score_millionths: (stats.reputation_score * 1_000_000.0) as u64,
    }
}

/// Convert ApexError to gRPC Status
fn to_grpc_status(err: ApexError) -> Status {
    let code = match err.code() {
        ErrorCode::TaskNotFound | ErrorCode::AgentNotFound | ErrorCode::RecordNotFound | ErrorCode::ToolNotFound | ErrorCode::ContractNotFound => tonic::Code::NotFound,
        ErrorCode::TaskAlreadyExists | ErrorCode::DuplicateRecord => tonic::Code::AlreadyExists,
        ErrorCode::InvalidStateTransition | ErrorCode::DependencyNotMet => tonic::Code::FailedPrecondition,
        ErrorCode::TokenLimitExceeded | ErrorCode::CostLimitExceeded | ErrorCode::TimeLimitExceeded | ErrorCode::ApiCallLimitExceeded | ErrorCode::ContractViolation | ErrorCode::ContractExpired | ErrorCode::LlmRateLimited | ErrorCode::AgentOverloaded => tonic::Code::ResourceExhausted,
        ErrorCode::DagCycleDetected | ErrorCode::DagValidationFailed | ErrorCode::ValidationError | ErrorCode::InvalidInput | ErrorCode::MissingRequiredField | ErrorCode::InvalidFormat | ErrorCode::ConfigurationError | ErrorCode::InvalidConfiguration => tonic::Code::InvalidArgument,
        ErrorCode::Unauthorized | ErrorCode::InvalidToken | ErrorCode::TokenExpired => tonic::Code::Unauthenticated,
        ErrorCode::Forbidden => tonic::Code::PermissionDenied,
        ErrorCode::LlmTimeout | ErrorCode::AgentTimeout | ErrorCode::ToolTimeout => tonic::Code::DeadlineExceeded,
        ErrorCode::LlmUnavailable | ErrorCode::AgentUnavailable | ErrorCode::DatabaseConnectionFailed | ErrorCode::CacheConnectionFailed | ErrorCode::ExternalServiceError => tonic::Code::Unavailable,
        ErrorCode::NotImplemented => tonic::Code::Unimplemented,
        _ => tonic::Code::Internal,
    };

    Status::new(code, err.user_message().to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Update Event Types for Streaming
// ═══════════════════════════════════════════════════════════════════════════════

/// Internal task update event
#[derive(Debug, Clone)]
pub struct TaskUpdateEvent {
    pub task: RustTask,
    pub previous_status: Option<RustTaskStatus>,
    pub update_type: TaskUpdateType,
}

/// Internal agent update event
#[derive(Debug, Clone)]
pub struct AgentUpdateEvent {
    pub agent_id: AgentId,
    pub agent_stats: Option<RustAgentStats>,
    pub previous_status: Option<RustAgentStatus>,
    pub update_type: AgentUpdateType,
    pub current_task_id: Option<TaskId>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// gRPC Service Implementation
// ═══════════════════════════════════════════════════════════════════════════════

/// The Apex gRPC service implementation
pub struct ApexGrpcService {
    /// Reference to the swarm orchestrator
    orchestrator: Arc<SwarmOrchestrator>,

    /// Broadcast channel for task updates
    task_updates_tx: broadcast::Sender<TaskUpdateEvent>,

    /// Broadcast channel for agent updates
    agent_updates_tx: broadcast::Sender<AgentUpdateEvent>,

    /// Active DAGs storage (maps DAG ID to DAG)
    dags: Arc<RwLock<std::collections::HashMap<Uuid, TaskDAG>>>,

    /// Tasks storage (maps Task ID to Task)
    tasks: Arc<RwLock<std::collections::HashMap<TaskId, RustTask>>>,
}

impl ApexGrpcService {
    /// Create a new ApexGrpcService
    pub fn new(orchestrator: Arc<SwarmOrchestrator>) -> Self {
        let (task_updates_tx, _) = broadcast::channel(1024);
        let (agent_updates_tx, _) = broadcast::channel(256);

        Self {
            orchestrator,
            task_updates_tx,
            agent_updates_tx,
            dags: Arc::new(RwLock::new(std::collections::HashMap::new())),
            tasks: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Broadcast a task update event
    pub fn broadcast_task_update(&self, event: TaskUpdateEvent) {
        let _ = self.task_updates_tx.send(event);
    }

    /// Broadcast an agent update event
    pub fn broadcast_agent_update(&self, event: AgentUpdateEvent) {
        let _ = self.agent_updates_tx.send(event);
    }

    /// Subscribe to task updates
    pub fn subscribe_task_updates(&self) -> broadcast::Receiver<TaskUpdateEvent> {
        self.task_updates_tx.subscribe()
    }

    /// Subscribe to agent updates
    pub fn subscribe_agent_updates(&self) -> broadcast::Receiver<AgentUpdateEvent> {
        self.agent_updates_tx.subscribe()
    }

    /// Parse a UUID from a string, returning a gRPC error on failure
    fn parse_uuid(s: &str, field_name: &str) -> Result<Uuid, Status> {
        Uuid::parse_str(s).map_err(|_| {
            Status::invalid_argument(format!("Invalid UUID for {}: {}", field_name, s))
        })
    }
}

#[tonic::async_trait]
impl ApexOrchestrator for ApexGrpcService {
    // ─────────────────────────────────────────────────────────────────────────
    // Task Operations
    // ─────────────────────────────────────────────────────────────────────────

    async fn submit_task(
        &self,
        request: Request<SubmitTaskRequest>,
    ) -> Result<Response<SubmitTaskResponse>, Status> {
        let req = request.into_inner();

        // Validate input
        let input = req.input.ok_or_else(|| {
            Status::invalid_argument("Task input is required")
        })?;

        // Create Rust task input
        let rust_input = from_proto_task_input(&input);

        // Create the task
        let mut task = RustTask::new(&req.name, rust_input);
        task.priority = req.priority;
        task.max_retries = if req.max_retries > 0 { req.max_retries } else { 3 };

        // Set parent if provided
        if let Some(parent_id_str) = &req.parent_task_id {
            let parent_id = Self::parse_uuid(parent_id_str, "parent_task_id")?;
            task.parent_id = Some(TaskId(parent_id));
        }

        // Store the task
        let task_id = task.id;
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_id, task.clone());
        }

        // Broadcast the task creation event
        self.broadcast_task_update(TaskUpdateEvent {
            task: task.clone(),
            previous_status: None,
            update_type: TaskUpdateType::Created,
        });

        tracing::info!(
            task_id = %task_id,
            name = %req.name,
            "Task submitted via gRPC"
        );

        Ok(Response::new(SubmitTaskResponse {
            task: Some(to_proto_task(&task)),
        }))
    }

    async fn get_task(
        &self,
        request: Request<GetTaskRequest>,
    ) -> Result<Response<GetTaskResponse>, Status> {
        let req = request.into_inner();
        let task_id = Self::parse_uuid(&req.task_id, "task_id")?;

        let tasks = self.tasks.read().await;
        let task = tasks
            .get(&TaskId(task_id))
            .ok_or_else(|| Status::not_found(format!("Task not found: {}", task_id)))?;

        Ok(Response::new(GetTaskResponse {
            task: Some(to_proto_task(task)),
        }))
    }

    async fn cancel_task(
        &self,
        request: Request<CancelTaskRequest>,
    ) -> Result<Response<CancelTaskResponse>, Status> {
        let req = request.into_inner();
        let task_id = Self::parse_uuid(&req.task_id, "task_id")?;

        let mut cancelled_ids = Vec::new();

        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&TaskId(task_id)) {
                let previous_status = task.status.clone();

                // Check if task can be cancelled
                if task.status.is_terminal() {
                    return Ok(Response::new(CancelTaskResponse {
                        success: false,
                        cancelled_task_ids: vec![],
                        error: Some(format!(
                            "Task {} is already in terminal state: {:?}",
                            task_id, task.status
                        )),
                    }));
                }

                // Cancel the task
                task.status = RustTaskStatus::Cancelled;
                task.error = req.reason.clone();
                task.completed_at = Some(Utc::now());

                cancelled_ids.push(task_id.to_string());

                // Broadcast the cancellation
                self.broadcast_task_update(TaskUpdateEvent {
                    task: task.clone(),
                    previous_status: Some(previous_status),
                    update_type: TaskUpdateType::Cancelled,
                });

                // Handle cascade cancellation
                if req.cascade {
                    // Find and cancel dependent tasks
                    let dependent_ids: Vec<TaskId> = tasks
                        .iter()
                        .filter(|(_, t)| t.parent_id == Some(TaskId(task_id)))
                        .map(|(id, _)| *id)
                        .collect();

                    for dep_id in dependent_ids {
                        if let Some(dep_task) = tasks.get_mut(&dep_id) {
                            if !dep_task.status.is_terminal() {
                                let prev_status = dep_task.status.clone();
                                dep_task.status = RustTaskStatus::Cancelled;
                                dep_task.error = Some("Parent task was cancelled".to_string());
                                dep_task.completed_at = Some(Utc::now());

                                cancelled_ids.push(dep_id.0.to_string());

                                self.broadcast_task_update(TaskUpdateEvent {
                                    task: dep_task.clone(),
                                    previous_status: Some(prev_status),
                                    update_type: TaskUpdateType::Cancelled,
                                });
                            }
                        }
                    }
                }
            } else {
                return Err(Status::not_found(format!("Task not found: {}", task_id)));
            }
        }

        tracing::info!(
            task_id = %task_id,
            cancelled_count = cancelled_ids.len(),
            cascade = req.cascade,
            "Task cancelled via gRPC"
        );

        Ok(Response::new(CancelTaskResponse {
            success: true,
            cancelled_task_ids: cancelled_ids,
            error: None,
        }))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DAG Operations
    // ─────────────────────────────────────────────────────────────────────────

    async fn create_dag(
        &self,
        request: Request<CreateDagRequest>,
    ) -> Result<Response<CreateDagResponse>, Status> {
        let req = request.into_inner();

        // Create a new DAG
        let mut dag = TaskDAG::new(&req.name);

        // Map to track task definitions to their assigned IDs
        let mut task_id_map: std::collections::HashMap<String, TaskId> = std::collections::HashMap::new();

        // Add tasks to the DAG
        for task_def in &req.tasks {
            let input = task_def.input.as_ref().ok_or_else(|| {
                Status::invalid_argument("Task input is required for all tasks")
            })?;

            let rust_input = from_proto_task_input(input);
            let mut task = RustTask::new(&task_def.name, rust_input);
            task.priority = task_def.priority;
            task.max_retries = if task_def.max_retries > 0 { task_def.max_retries } else { 3 };

            // Use provided ID or generate one
            let task_id = if let Some(ref id_str) = task_def.id {
                let id = Self::parse_uuid(id_str, "task.id")?;
                task.id = TaskId(id);
                task.id
            } else {
                task.id
            };

            // Track the mapping for dependency resolution
            let def_key = task_def.id.clone().unwrap_or_else(|| task_def.name.clone());
            task_id_map.insert(def_key, task_id);

            dag.add_task(task).map_err(to_grpc_status)?;
        }

        // Add dependencies
        for dep in &req.dependencies {
            let from_id = task_id_map
                .get(&dep.from_task_id)
                .or_else(|| {
                    Uuid::parse_str(&dep.from_task_id)
                        .ok()
                        .and_then(|_| task_id_map.get(&dep.from_task_id))
                })
                .ok_or_else(|| {
                    Status::invalid_argument(format!(
                        "Unknown task in dependency: {}",
                        dep.from_task_id
                    ))
                })?;

            let to_id = task_id_map
                .get(&dep.to_task_id)
                .or_else(|| {
                    Uuid::parse_str(&dep.to_task_id)
                        .ok()
                        .and_then(|_| task_id_map.get(&dep.to_task_id))
                })
                .ok_or_else(|| {
                    Status::invalid_argument(format!(
                        "Unknown task in dependency: {}",
                        dep.to_task_id
                    ))
                })?;

            dag.add_dependency(*from_id, *to_id).map_err(to_grpc_status)?;
        }

        let dag_id = dag.id();

        // Store the DAG
        {
            let mut dags = self.dags.write().await;
            dags.insert(dag_id, dag.clone());
        }

        // Submit to orchestrator
        self.orchestrator.submit_dag(dag.clone()).await.map_err(to_grpc_status)?;

        tracing::info!(
            dag_id = %dag_id,
            name = %req.name,
            task_count = req.tasks.len(),
            "DAG created via gRPC"
        );

        // Build response DAG
        let proto_dag = Dag {
            id: dag_id.to_string(),
            name: req.name,
            tasks: vec![], // Simplified - would need to iterate the graph
            dependencies: req.dependencies,
            created_at: Some(to_proto_timestamp(dag.created_at())),
            status: DagStatus::Pending as i32,
        };

        Ok(Response::new(CreateDagResponse {
            dag: Some(proto_dag),
        }))
    }

    async fn execute_dag(
        &self,
        request: Request<ExecuteDagRequest>,
    ) -> Result<Response<ExecuteDagResponse>, Status> {
        let req = request.into_inner();
        let dag_id = Self::parse_uuid(&req.dag_id, "dag_id")?;

        // Execute the DAG
        let result = if req.wait_for_completion {
            // Synchronous execution with optional timeout
            let timeout = Duration::from_secs(req.timeout_seconds.unwrap_or(3600));

            let execution = self.orchestrator.execute_dag(dag_id);
            match tokio::time::timeout(timeout, execution).await {
                Ok(result) => result.map_err(to_grpc_status)?,
                Err(_) => {
                    return Err(Status::deadline_exceeded(format!(
                        "DAG execution timed out after {}s",
                        timeout.as_secs()
                    )));
                }
            }
        } else {
            // Asynchronous execution - spawn and return immediately
            let orchestrator = self.orchestrator.clone();
            tokio::spawn(async move {
                if let Err(e) = orchestrator.execute_dag(dag_id).await {
                    tracing::error!(dag_id = %dag_id, error = %e, "DAG execution failed");
                }
            });

            // Return a pending response
            return Ok(Response::new(ExecuteDagResponse {
                dag_id: dag_id.to_string(),
                status: DagStatus::Running as i32,
                stats: Some(DagStats::default()),
                total_tokens: 0,
                total_cost_microdollars: 0,
                duration_ms: 0,
                error: None,
            }));
        };

        tracing::info!(
            dag_id = %dag_id,
            status = ?result.status,
            tasks_completed = result.tasks_completed,
            tasks_failed = result.tasks_failed,
            "DAG executed via gRPC"
        );

        Ok(Response::new(ExecuteDagResponse {
            dag_id: dag_id.to_string(),
            status: to_proto_dag_status(&result.status),
            stats: Some(DagStats {
                total: (result.tasks_completed + result.tasks_failed) as u32,
                pending: 0,
                ready: 0,
                running: 0,
                completed: result.tasks_completed as u32,
                failed: result.tasks_failed as u32,
                cancelled: 0,
            }),
            total_tokens: result.total_tokens,
            total_cost_microdollars: (result.total_cost * 1_000_000.0) as i64,
            duration_ms: result.duration_ms,
            error: if result.status == DagExecutionStatus::Completed {
                None
            } else {
                Some(format!("{} tasks failed", result.tasks_failed))
            },
        }))
    }

    async fn get_dag_status(
        &self,
        request: Request<GetDagStatusRequest>,
    ) -> Result<Response<GetDagStatusResponse>, Status> {
        let req = request.into_inner();
        let dag_id = Self::parse_uuid(&req.dag_id, "dag_id")?;

        let dags = self.dags.read().await;
        let dag = dags
            .get(&dag_id)
            .ok_or_else(|| Status::not_found(format!("DAG not found: {}", dag_id)))?;

        let stats = dag.stats();
        let rust_stats = crate::dag::DagStats {
            total: stats.total,
            pending: stats.pending,
            ready: stats.ready,
            running: stats.running,
            completed: stats.completed,
            failed: stats.failed,
            cancelled: stats.cancelled,
        };

        // Determine overall status
        let status = if dag.is_complete() {
            if rust_stats.failed > 0 || rust_stats.cancelled > 0 {
                DagStatus::PartialFailure
            } else {
                DagStatus::Completed
            }
        } else if rust_stats.running > 0 {
            DagStatus::Running
        } else {
            DagStatus::Pending
        };

        Ok(Response::new(GetDagStatusResponse {
            dag_id: dag_id.to_string(),
            status: status as i32,
            stats: Some(DagStats {
                total: rust_stats.total as u32,
                pending: rust_stats.pending as u32,
                ready: rust_stats.ready as u32,
                running: rust_stats.running as u32,
                completed: rust_stats.completed as u32,
                failed: rust_stats.failed as u32,
                cancelled: rust_stats.cancelled as u32,
            }),
            tasks: vec![], // Would need to iterate graph
            total_tokens: 0, // Would need to aggregate
            total_cost_microdollars: 0,
            duration_ms: 0,
        }))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Agent Operations
    // ─────────────────────────────────────────────────────────────────────────

    async fn register_agent(
        &self,
        request: Request<RegisterAgentRequest>,
    ) -> Result<Response<RegisterAgentResponse>, Status> {
        let req = request.into_inner();
        let agent_def = req.agent.ok_or_else(|| {
            Status::invalid_argument("Agent definition is required")
        })?;

        // Create the Rust agent
        let mut agent = RustAgent::new(&agent_def.name, &agent_def.model);

        if !agent_def.system_prompt.is_empty() {
            agent = agent.with_system_prompt(&agent_def.system_prompt);
        }

        if agent_def.max_load > 0 {
            agent = agent.with_max_load(agent_def.max_load);
        }

        // Add tools
        for tool in &agent_def.tools {
            agent = agent.with_tool(RustTool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: serde_json::from_str(&tool.parameters_json).unwrap_or_default(),
                enabled: tool.enabled,
            });
        }

        // Register with orchestrator
        let agent_id = self.orchestrator.register_agent(agent);

        tracing::info!(
            agent_id = %agent_id.0,
            name = %agent_def.name,
            model = %agent_def.model,
            "Agent registered via gRPC"
        );

        // Broadcast agent registration event
        self.broadcast_agent_update(AgentUpdateEvent {
            agent_id,
            agent_stats: None, // Agent stats not available at registration
            previous_status: None,
            update_type: AgentUpdateType::Registered,
            current_task_id: None,
        });

        // Build response (simplified - real impl would retrieve the registered agent)
        let proto_agent = Agent {
            id: agent_id.0.to_string(),
            name: agent_def.name,
            model: agent_def.model,
            system_prompt: agent_def.system_prompt,
            tools: agent_def.tools,
            status: AgentStatus::Idle as i32,
            current_load: 0,
            max_load: agent_def.max_load,
            created_at: Some(to_proto_timestamp(Utc::now())),
            last_active_at: None,
        };

        Ok(Response::new(RegisterAgentResponse {
            agent: Some(proto_agent),
        }))
    }

    async fn list_agents(
        &self,
        request: Request<ListAgentsRequest>,
    ) -> Result<Response<ListAgentsResponse>, Status> {
        let _req = request.into_inner();

        // Get orchestrator stats (which includes agent count)
        let stats = self.orchestrator.stats();

        // In a real implementation, we'd iterate over the orchestrator's agents
        // For now, return an empty list with the count
        let agents = Vec::new();

        Ok(Response::new(ListAgentsResponse {
            agents,
            total_count: stats.registered_agents as u32,
        }))
    }

    async fn get_agent_stats(
        &self,
        request: Request<GetAgentStatsRequest>,
    ) -> Result<Response<GetAgentStatsResponse>, Status> {
        let req = request.into_inner();
        let agent_id = Self::parse_uuid(&req.agent_id, "agent_id")?;

        // In a real implementation, we'd look up the agent and get its stats
        // For now, return a not found error
        Err(Status::not_found(format!(
            "Agent not found: {}",
            agent_id
        )))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Streaming Operations
    // ─────────────────────────────────────────────────────────────────────────

    type StreamTaskUpdatesStream = Pin<Box<dyn Stream<Item = Result<TaskUpdate, Status>> + Send>>;

    async fn stream_task_updates(
        &self,
        request: Request<StreamTaskUpdatesRequest>,
    ) -> Result<Response<Self::StreamTaskUpdatesStream>, Status> {
        let req = request.into_inner();

        // Parse filter task IDs
        let filter_task_ids: Option<Vec<TaskId>> = if req.task_ids.is_empty() {
            None
        } else {
            let mut ids = Vec::new();
            for id_str in &req.task_ids {
                ids.push(TaskId(Self::parse_uuid(id_str, "task_id")?));
            }
            Some(ids)
        };

        // Parse filter DAG ID
        let filter_dag_id: Option<Uuid> = if let Some(ref dag_id_str) = req.dag_id {
            Some(Self::parse_uuid(dag_id_str, "dag_id")?)
        } else {
            None
        };

        // Parse status filter
        let filter_statuses: Option<Vec<RustTaskStatus>> = if req.status_filter.is_empty() {
            None
        } else {
            Some(req.status_filter.iter().map(|s| from_proto_task_status(*s)).collect())
        };

        // Subscribe to task updates
        let rx = self.subscribe_task_updates();

        // Create the stream with filtering
        let stream = BroadcastStream::new(rx)
            .filter_map(move |result| {
                match result {
                    Ok(event) => {
                        // Apply filters
                        if let Some(ref ids) = filter_task_ids {
                            if !ids.contains(&event.task.id) {
                                return None;
                            }
                        }

                        if let Some(ref dag_id) = filter_dag_id {
                            // Would need to check if task belongs to this DAG
                            let _ = dag_id; // Placeholder
                        }

                        if let Some(ref statuses) = filter_statuses {
                            if !statuses.contains(&event.task.status) {
                                return None;
                            }
                        }

                        // Convert to proto
                        Some(Ok(TaskUpdate {
                            task: Some(to_proto_task(&event.task)),
                            previous_status: event.previous_status.as_ref().map(to_proto_task_status),
                            timestamp: Some(to_proto_timestamp(Utc::now())),
                            update_type: event.update_type as i32,
                        }))
                    }
                    Err(_) => None, // Skip lagged messages
                }
            });

        tracing::info!(
            task_ids_filter = ?req.task_ids,
            dag_id_filter = ?req.dag_id,
            "Client subscribed to task updates stream"
        );

        Ok(Response::new(Box::pin(stream)))
    }

    type StreamAgentUpdatesStream = Pin<Box<dyn Stream<Item = Result<AgentUpdate, Status>> + Send>>;

    async fn stream_agent_updates(
        &self,
        request: Request<StreamAgentUpdatesRequest>,
    ) -> Result<Response<Self::StreamAgentUpdatesStream>, Status> {
        let req = request.into_inner();

        // Parse filter agent IDs
        let filter_agent_ids: Option<Vec<AgentId>> = if req.agent_ids.is_empty() {
            None
        } else {
            let mut ids = Vec::new();
            for id_str in &req.agent_ids {
                ids.push(AgentId(Self::parse_uuid(id_str, "agent_id")?));
            }
            Some(ids)
        };

        // Parse status filter
        let filter_statuses: Option<Vec<RustAgentStatus>> = if req.status_filter.is_empty() {
            None
        } else {
            Some(req.status_filter.iter().map(|s| from_proto_agent_status(*s)).collect())
        };

        // Subscribe to agent updates
        let rx = self.subscribe_agent_updates();

        // Create the stream with filtering
        let stream = BroadcastStream::new(rx)
            .filter_map(move |result| {
                match result {
                    Ok(event) => {
                        // Apply filters
                        if let Some(ref ids) = filter_agent_ids {
                            if !ids.contains(&event.agent_id) {
                                return None;
                            }
                        }

                        if let Some(ref statuses) = filter_statuses {
                            if let Some(ref stats) = event.agent_stats {
                                if !statuses.contains(&stats.status) {
                                    return None;
                                }
                            }
                        }

                        // Convert to proto - build agent from stats if available
                        let proto_agent = event.agent_stats.as_ref().map(|stats| Agent {
                            id: stats.id.0.to_string(),
                            name: stats.name.clone(),
                            model: stats.model.clone(),
                            system_prompt: String::new(),
                            tools: vec![],
                            status: to_proto_agent_status(&stats.status),
                            current_load: stats.current_load,
                            max_load: stats.max_load,
                            created_at: Some(to_proto_timestamp(Utc::now())),
                            last_active_at: None,
                        });

                        Some(Ok(AgentUpdate {
                            agent: proto_agent,
                            previous_status: event.previous_status.as_ref().map(to_proto_agent_status),
                            timestamp: Some(to_proto_timestamp(Utc::now())),
                            update_type: event.update_type as i32,
                            current_task_id: event.current_task_id.map(|id| id.0.to_string()),
                        }))
                    }
                    Err(_) => None, // Skip lagged messages
                }
            });

        tracing::info!(
            agent_ids_filter = ?req.agent_ids,
            "Client subscribed to agent updates stream"
        );

        Ok(Response::new(Box::pin(stream)))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Server Builder
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for the gRPC server
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
    /// Address to bind to
    pub addr: std::net::SocketAddr,

    /// Maximum concurrent streams per connection
    pub max_concurrent_streams: Option<u32>,

    /// HTTP/2 keep-alive interval
    pub http2_keepalive_interval: Option<Duration>,

    /// HTTP/2 keep-alive timeout
    pub http2_keepalive_timeout: Option<Duration>,

    /// Enable gRPC reflection (for debugging)
    pub enable_reflection: bool,

    /// Maximum message size (default: 4MB)
    pub max_message_size: usize,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            addr: "0.0.0.0:50051".parse().unwrap(),
            max_concurrent_streams: Some(1000),
            http2_keepalive_interval: Some(Duration::from_secs(10)),
            http2_keepalive_timeout: Some(Duration::from_secs(20)),
            enable_reflection: true,
            max_message_size: 4 * 1024 * 1024, // 4MB
        }
    }
}

/// Build and start the gRPC server
pub async fn start_grpc_server(
    config: GrpcServerConfig,
    orchestrator: Arc<SwarmOrchestrator>,
) -> Result<(), Box<dyn std::error::Error>> {
    use proto::apex_orchestrator_server::ApexOrchestratorServer;

    let service = ApexGrpcService::new(orchestrator);

    let mut server = tonic::transport::Server::builder();

    // Configure HTTP/2 settings
    if let Some(interval) = config.http2_keepalive_interval {
        server = server.http2_keepalive_interval(Some(interval));
    }

    if let Some(timeout) = config.http2_keepalive_timeout {
        server = server.http2_keepalive_timeout(Some(timeout));
    }

    if let Some(max_streams) = config.max_concurrent_streams {
        server = server.concurrency_limit_per_connection(max_streams as usize);
    }

    // Build the service with size limits
    let grpc_service = ApexOrchestratorServer::new(service)
        .max_decoding_message_size(config.max_message_size)
        .max_encoding_message_size(config.max_message_size);

    tracing::info!(
        addr = %config.addr,
        "Starting Apex gRPC server"
    );

    server
        .add_service(grpc_service)
        .serve(config.addr)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_timestamp_conversion() {
        let now = Utc::now();
        let proto_ts = to_proto_timestamp(now);
        let back = from_proto_timestamp(&proto_ts);

        // Should be within 1 microsecond (nanos precision loss)
        assert!((now.timestamp_millis() - back.timestamp_millis()).abs() < 1);
    }

    #[test]
    fn test_task_status_conversion() {
        let statuses = vec![
            RustTaskStatus::Pending,
            RustTaskStatus::Ready,
            RustTaskStatus::Running,
            RustTaskStatus::Completed,
            RustTaskStatus::Failed,
            RustTaskStatus::Cancelled,
        ];

        for status in statuses {
            let proto = to_proto_task_status(&status);
            let back = from_proto_task_status(proto);
            assert_eq!(status, back);
        }
    }

    #[test]
    fn test_agent_status_conversion() {
        let statuses = vec![
            RustAgentStatus::Idle,
            RustAgentStatus::Busy,
            RustAgentStatus::Error,
            RustAgentStatus::Paused,
        ];

        for status in statuses {
            let proto = to_proto_agent_status(&status);
            let back = from_proto_agent_status(proto);
            assert_eq!(status, back);
        }
    }
}
