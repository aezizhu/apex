//! Swarm Orchestrator - The heart of Apex.
//!
//! Manages DAG execution, agent coordination, and resource tracking.

pub mod worker_pool;
pub mod circuit_breaker;
pub mod cnp;

pub use worker_pool::{WorkerPool, WorkerPoolConfig, WorkerPoolStats, WorkerPermit, WorkerExecution};
pub use circuit_breaker::{
    CircuitBreaker, CircuitState, CircuitBreakerMetrics,
    AgentCircuitBreakerRegistry, AgentCircuitMetrics, AgentCircuitOpenReason,
};
pub use cnp::{
    CnpManager, CnpConfig, TaskAnnouncement, AgentBid, BidScore,
    ScoreBreakdown, AwardDecision,
};

use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use dashmap::DashMap;
use uuid::Uuid;

use crate::dag::{TaskDAG, TaskId, TaskOutput};
use crate::contracts::{AgentContract, ResourceLimits};
use crate::agents::{Agent, AgentId};
use crate::routing::ModelRouter;
use crate::error::{ApexError, Result};
use crate::db::Database;
use crate::observability::Tracer;

use serde::{Deserialize, Serialize};

/// Configuration for the SwarmOrchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum concurrent agents
    pub max_concurrent_agents: usize,

    /// Default resource limits for tasks without explicit limits
    pub default_limits: ResourceLimits,

    /// Enable FrugalGPT model routing
    pub enable_model_routing: bool,

    /// Circuit breaker threshold (consecutive failures)
    pub circuit_breaker_threshold: u32,

    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,

    /// Timeout in seconds for waiting on task results from Redis
    pub task_result_timeout_secs: u64,
}

/// Payload published to the Redis pending queue for agent workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisTaskPayload {
    pub task_id: String,
    pub dag_id: String,
    pub input: serde_json::Value,
    pub contract: RedisContractPayload,
    pub trace_context: Option<RedisTraceContext>,
}

/// Resource limits sent alongside a task to the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisContractPayload {
    pub token_limit: u64,
    pub cost_limit: f64,
    pub api_call_limit: u64,
    pub time_limit_seconds: u64,
}

/// Distributed tracing context forwarded to workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisTraceContext {
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

/// Result payload returned by a Python agent worker via Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisTaskResult {
    pub output: String,
    pub tokens_used: u64,
    pub cost_dollars: f64,
    pub status: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 100,
            default_limits: ResourceLimits::medium(),
            enable_model_routing: true,
            circuit_breaker_threshold: 5,
            retry_delay_ms: 1000,
            task_result_timeout_secs: 300,
        }
    }
}

/// The main swarm orchestration engine.
#[allow(dead_code)]
pub struct SwarmOrchestrator {
    /// Configuration
    config: OrchestratorConfig,

    /// Database connection pool
    db: Arc<Database>,

    /// Redis client for task queue communication
    redis_client: redis::Client,

    /// Worker pool semaphore for concurrency control
    worker_semaphore: Arc<Semaphore>,

    /// Active DAGs being executed
    active_dags: DashMap<Uuid, Arc<RwLock<TaskDAG>>>,

    /// Registered agents
    agents: DashMap<AgentId, Arc<Agent>>,

    /// Active contracts
    contracts: DashMap<Uuid, Arc<RwLock<AgentContract>>>,

    /// Model router for FrugalGPT
    model_router: Arc<ModelRouter>,

    /// Circuit breaker for failure handling
    circuit_breaker: Arc<CircuitBreaker>,

    /// Distributed tracing
    tracer: Arc<Tracer>,
}

impl SwarmOrchestrator {
    /// Create a new orchestrator.
    pub async fn new(
        config: OrchestratorConfig,
        db: Arc<Database>,
        redis_client: redis::Client,
        tracer: Arc<Tracer>,
    ) -> Result<Self> {
        let model_router = Arc::new(ModelRouter::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(config.circuit_breaker_threshold));

        Ok(Self {
            worker_semaphore: Arc::new(Semaphore::new(config.max_concurrent_agents)),
            config,
            db,
            redis_client,
            active_dags: DashMap::new(),
            agents: DashMap::new(),
            contracts: DashMap::new(),
            model_router,
            circuit_breaker,
            tracer,
        })
    }

    /// Register an agent with the orchestrator.
    pub fn register_agent(&self, agent: Agent) -> AgentId {
        let id = agent.id;
        self.agents.insert(id, Arc::new(agent));
        id
    }

    /// Deregister an agent from the orchestrator.
    pub fn deregister_agent(&self, agent_id: AgentId) -> bool {
        self.agents.remove(&agent_id).is_some()
    }

    /// Submit a DAG for execution.
    pub async fn submit_dag(&self, dag: TaskDAG) -> Result<Uuid> {
        let dag_id = dag.id();

        // Validate DAG
        let _ = dag.topological_order()?;

        // Store in active DAGs
        self.active_dags.insert(dag_id, Arc::new(RwLock::new(dag)));

        // Persist to database
        // self.db.store_dag(&dag).await?;

        tracing::info!(dag_id = %dag_id, "DAG submitted for execution");

        Ok(dag_id)
    }

    /// Execute a DAG to completion.
    pub async fn execute_dag(&self, dag_id: Uuid) -> Result<DagExecutionResult> {
        let dag_lock = self.active_dags.get(&dag_id)
            .ok_or_else(|| ApexError::not_found("DAG", dag_id.to_string()))?
            .clone();

        let span = tracing::info_span!("execute_dag", dag_id = %dag_id);
        let _guard = span.enter();

        let start_time = std::time::Instant::now();
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0f64;
        let mut tasks_completed = 0usize;
        let mut tasks_failed = 0usize;

        loop {
            // Get ready tasks
            let ready_tasks = {
                let dag = dag_lock.read().await;
                if dag.is_complete() {
                    break;
                }
                dag.get_ready_tasks()
            };

            if ready_tasks.is_empty() {
                // No tasks ready but DAG not complete - might be waiting for running tasks
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }

            // Execute ready tasks in parallel
            let mut handles = Vec::new();

            for task_id in ready_tasks {
                let permit = self.worker_semaphore.clone().acquire_owned().await?;

                let dag_lock = dag_lock.clone();
                let db = self.db.clone();
                let redis_client = self.redis_client.clone();
                let model_router = self.model_router.clone();
                let agents = self.agents.clone();
                let circuit_breaker = self.circuit_breaker.clone();
                let default_limits = self.config.default_limits.clone();
                let task_result_timeout_secs = self.config.task_result_timeout_secs;

                let handle = tokio::spawn(async move {
                    let result = Self::execute_task(
                        task_id,
                        dag_id,
                        dag_lock,
                        db,
                        redis_client,
                        model_router,
                        agents,
                        circuit_breaker,
                        default_limits,
                        task_result_timeout_secs,
                    ).await;

                    drop(permit); // Release semaphore permit
                    result
                });

                handles.push(handle);
            }

            // Wait for all parallel tasks
            let results = futures::future::join_all(handles).await;

            for result in results {
                match result {
                    Ok(Ok(task_result)) => {
                        total_tokens += task_result.tokens_used;
                        total_cost += task_result.cost;
                        tasks_completed += 1;
                    }
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "Task execution failed");
                        tasks_failed += 1;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Task join error");
                        tasks_failed += 1;
                    }
                }
            }
        }

        let elapsed = start_time.elapsed();

        // Clean up
        self.active_dags.remove(&dag_id);

        let result = DagExecutionResult {
            dag_id,
            status: if tasks_failed == 0 {
                DagExecutionStatus::Completed
            } else {
                DagExecutionStatus::PartialFailure
            },
            tasks_completed,
            tasks_failed,
            total_tokens,
            total_cost,
            duration_ms: elapsed.as_millis() as u64,
        };

        tracing::info!(
            dag_id = %dag_id,
            tasks_completed = tasks_completed,
            tasks_failed = tasks_failed,
            total_tokens = total_tokens,
            total_cost = total_cost,
            duration_ms = result.duration_ms,
            "DAG execution completed"
        );

        Ok(result)
    }

    /// Execute a single task by publishing it to Redis and waiting for the result.
    async fn execute_task(
        task_id: TaskId,
        dag_id: Uuid,
        dag_lock: Arc<RwLock<TaskDAG>>,
        _db: Arc<Database>,
        redis_client: redis::Client,
        model_router: Arc<ModelRouter>,
        agents: DashMap<AgentId, Arc<Agent>>,
        circuit_breaker: Arc<CircuitBreaker>,
        default_limits: ResourceLimits,
        task_result_timeout_secs: u64,
    ) -> Result<TaskExecutionResult> {
        let span = tracing::info_span!("execute_task", task_id = %task_id);
        let _guard = span.enter();

        // Get task details
        let task = {
            let dag = dag_lock.read().await;
            dag.get_task(task_id)
                .ok_or_else(|| ApexError::task_not_found(task_id.0))?
                .clone()
        };

        // Check circuit breaker
        if !circuit_breaker.can_execute() {
            return Err(ApexError::internal("Circuit breaker is open"));
        }

        // Select agent (round-robin for now, CNP bidding later)
        let agent = agents.iter()
            .find(|entry| entry.value().is_available())
            .map(|entry| entry.value().clone())
            .ok_or_else(|| ApexError::internal("No available agents"))?;

        // Select model via router
        let model = if let Some(router) = Some(&model_router) {
            router.select_model(&task.input.instruction)
        } else {
            "gpt-4o-mini".to_string()
        };

        // Mark task as running
        {
            let mut dag = dag_lock.write().await;
            if let Some(t) = dag.get_task_mut(task_id) {
                t.start(agent.id.0);
            }
        }

        // Create contract for this task
        let _contract = AgentContract::new(agent.id.0, task_id.0, default_limits.clone());

        // Execute the task via Redis queue
        let execution_start = std::time::Instant::now();

        // Build the task payload for the pending queue
        let payload = RedisTaskPayload {
            task_id: task_id.0.to_string(),
            dag_id: dag_id.to_string(),
            input: serde_json::to_value(&task.input)?,
            contract: RedisContractPayload {
                token_limit: default_limits.token_limit,
                cost_limit: default_limits.cost_limit,
                api_call_limit: default_limits.api_call_limit,
                time_limit_seconds: default_limits.time_limit_seconds,
            },
            trace_context: Some(RedisTraceContext {
                trace_id: task.trace_id.clone(),
                span_id: task.span_id.clone(),
            }),
        };

        let payload_json = serde_json::to_string(&payload)?;

        // Publish task to the pending queue
        {
            let _redis_span = tracing::info_span!("redis_publish_task", task_id = %task_id);
            let _redis_guard = _redis_span.enter();

            let mut conn = redis_client.get_multiplexed_async_connection().await
                .map_err(|e| ApexError::with_internal(
                    crate::error::ErrorCode::CacheConnectionFailed,
                    "Failed to connect to Redis for task publishing",
                    e.to_string(),
                ))?;

            redis::cmd("RPUSH")
                .arg("apex:tasks:pending")
                .arg(&payload_json)
                .query_async::<_, i64>(&mut conn)
                .await
                .map_err(|e| ApexError::with_internal(
                    crate::error::ErrorCode::CacheError,
                    "Failed to publish task to Redis queue",
                    e.to_string(),
                ))?;

            tracing::debug!(task_id = %task_id, "Task published to apex:tasks:pending");
        }

        // Wait for the result on the per-task result queue
        let result_key = format!("apex:tasks:result:{}", task_id.0);
        let redis_result: RedisTaskResult = {
            let _redis_span = tracing::info_span!("redis_await_result", task_id = %task_id, result_key = %result_key);
            let _redis_guard = _redis_span.enter();

            let mut conn = redis_client.get_multiplexed_async_connection().await
                .map_err(|e| ApexError::with_internal(
                    crate::error::ErrorCode::CacheConnectionFailed,
                    "Failed to connect to Redis for result polling",
                    e.to_string(),
                ))?;

            // BLPOP blocks until a result is available or the timeout expires
            let blpop_result: Option<(String, String)> = redis::cmd("BLPOP")
                .arg(&result_key)
                .arg(task_result_timeout_secs)
                .query_async(&mut conn)
                .await
                .map_err(|e| ApexError::with_internal(
                    crate::error::ErrorCode::CacheError,
                    "Failed to read task result from Redis",
                    e.to_string(),
                ))?;

            match blpop_result {
                Some((_key, value)) => {
                    serde_json::from_str::<RedisTaskResult>(&value).map_err(|e| {
                        ApexError::with_internal(
                            crate::error::ErrorCode::DeserializationError,
                            "Failed to deserialize task result from Redis",
                            e.to_string(),
                        )
                    })?
                }
                None => {
                    // Timeout: no result received within the configured window
                    circuit_breaker.record_failure();
                    return Err(ApexError::with_internal(
                        crate::error::ErrorCode::AgentTimeout,
                        "Task execution timed out waiting for agent result",
                        format!(
                            "No result on {} within {}s",
                            result_key, task_result_timeout_secs
                        ),
                    ));
                }
            }
        };

        let elapsed = execution_start.elapsed();

        // Check if the worker reported a failure
        if redis_result.status == "failed" {
            circuit_breaker.record_failure();
            let error_msg = redis_result
                .error
                .unwrap_or_else(|| "Agent worker reported failure".to_string());
            // Update task as failed
            {
                let mut dag = dag_lock.write().await;
                if let Some(t) = dag.get_task_mut(task_id) {
                    t.fail(&error_msg);
                }
            }
            return Err(ApexError::agent_execution_failed(error_msg));
        }

        // Build the TaskOutput from the Redis result
        let output = TaskOutput {
            result: redis_result.output,
            data: redis_result.data.unwrap_or(serde_json::json!({})),
            artifacts: vec![],
            reasoning: redis_result.reasoning,
        };

        let tokens_used = redis_result.tokens_used;
        let cost = redis_result.cost_dollars;

        // Update task as completed
        {
            let mut dag = dag_lock.write().await;
            if let Some(t) = dag.get_task_mut(task_id) {
                t.complete(output, tokens_used, cost);
            }
        }

        circuit_breaker.record_success();

        tracing::info!(
            task_id = %task_id,
            agent_id = %agent.id.0,
            model = %model,
            tokens = tokens_used,
            cost = cost,
            duration_ms = elapsed.as_millis(),
            "Task completed"
        );

        Ok(TaskExecutionResult {
            task_id,
            agent_id: agent.id,
            model,
            tokens_used,
            cost,
            duration_ms: elapsed.as_millis() as u64,
        })
    }

    /// Get current orchestrator statistics.
    pub fn stats(&self) -> OrchestratorStats {
        OrchestratorStats {
            active_dags: self.active_dags.len(),
            registered_agents: self.agents.len(),
            active_contracts: self.contracts.len(),
            available_workers: self.worker_semaphore.available_permits(),
            max_workers: self.config.max_concurrent_agents,
        }
    }
}

/// Result of DAG execution.
#[derive(Debug, Clone)]
pub struct DagExecutionResult {
    pub dag_id: Uuid,
    pub status: DagExecutionStatus,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub duration_ms: u64,
}

/// Status of DAG execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagExecutionStatus {
    Completed,
    PartialFailure,
    Failed,
    Cancelled,
}

/// Result of task execution.
#[derive(Debug, Clone)]
pub struct TaskExecutionResult {
    pub task_id: TaskId,
    pub agent_id: AgentId,
    pub model: String,
    pub tokens_used: u64,
    pub cost: f64,
    pub duration_ms: u64,
}

/// Orchestrator statistics.
#[derive(Debug, Clone)]
pub struct OrchestratorStats {
    pub active_dags: usize,
    pub registered_agents: usize,
    pub active_contracts: usize,
    pub available_workers: usize,
    pub max_workers: usize,
}

#[cfg(test)]
mod tests {
    // Tests would go here
}
