//! DAG Executor - Manages the execution lifecycle of task DAGs.
//!
//! The `DagExecutor` is responsible for:
//! - Coordinating task execution across multiple workers
//! - Managing task state transitions
//! - Handling failures and triggering retries
//! - Enforcing contracts during execution
//! - Emitting execution events for observability

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

use super::{DagStats, Task, TaskDAG, TaskId, TaskOutput, TaskStatus};
use crate::contracts::{AgentContract, ContractEnforcer, ResourceLimits, UsageTracker};
use crate::error::Result;

/// Events emitted during DAG execution.
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    /// DAG execution started
    DagStarted { dag_id: Uuid },
    /// DAG execution completed
    DagCompleted {
        dag_id: Uuid,
        stats: DagStats,
        duration_ms: u64,
    },
    /// DAG execution failed
    DagFailed { dag_id: Uuid, error: String },
    /// Task started execution
    TaskStarted { dag_id: Uuid, task_id: TaskId },
    /// Task completed successfully
    TaskCompleted {
        dag_id: Uuid,
        task_id: TaskId,
        tokens: u64,
        cost: f64,
        duration_ms: u64,
    },
    /// Task failed
    TaskFailed {
        dag_id: Uuid,
        task_id: TaskId,
        error: String,
        will_retry: bool,
    },
    /// Task was cancelled
    TaskCancelled { dag_id: Uuid, task_id: TaskId },
    /// Contract limit warning (approaching limit)
    ContractWarning {
        dag_id: Uuid,
        task_id: TaskId,
        resource: String,
        usage_percent: f64,
    },
}

/// Configuration for the DAG executor.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum concurrent tasks per DAG
    pub max_concurrent_tasks: usize,
    /// Default resource limits for tasks without explicit limits
    pub default_limits: ResourceLimits,
    /// Whether to cancel dependents on task failure
    pub cancel_dependents_on_failure: bool,
    /// Polling interval for ready tasks (milliseconds)
    pub poll_interval_ms: u64,
    /// Event channel buffer size
    pub event_buffer_size: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 50,
            default_limits: ResourceLimits::medium(),
            cancel_dependents_on_failure: true,
            poll_interval_ms: 50,
            event_buffer_size: 1000,
        }
    }
}

/// Result of executing a single task.
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// The task ID
    pub task_id: TaskId,
    /// Output data (if successful)
    pub output: Option<TaskOutput>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Tokens consumed
    pub tokens_used: u64,
    /// Cost in dollars
    pub cost: f64,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Whether the task should be retried
    pub should_retry: bool,
}

/// Manages the execution lifecycle of a DAG.
pub struct DagExecutor {
    /// Executor configuration
    config: ExecutorConfig,
    /// The DAG being executed
    dag: Arc<RwLock<TaskDAG>>,
    /// Contract enforcer for limit validation
    contract_enforcer: Arc<ContractEnforcer>,
    /// Usage tracker for real-time monitoring
    usage_tracker: Arc<UsageTracker>,
    /// Active task contracts
    task_contracts: DashMap<TaskId, AgentContract>,
    /// Event broadcaster
    event_sender: broadcast::Sender<ExecutionEvent>,
    /// Execution start time
    start_time: Option<Instant>,
    /// Unique execution ID
    execution_id: Uuid,
}

impl DagExecutor {
    /// Create a new DAG executor.
    pub fn new(dag: TaskDAG, config: ExecutorConfig, root_contract: Option<AgentContract>) -> Self {
        let (event_sender, _) = broadcast::channel(config.event_buffer_size);

        let contract_enforcer = Arc::new(ContractEnforcer::new(root_contract.clone()));
        let usage_tracker = Arc::new(UsageTracker::new());

        Self {
            config,
            dag: Arc::new(RwLock::new(dag)),
            contract_enforcer,
            usage_tracker,
            task_contracts: DashMap::new(),
            event_sender,
            start_time: None,
            execution_id: Uuid::new_v4(),
        }
    }

    /// Subscribe to execution events.
    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.event_sender.subscribe()
    }

    /// Get the execution ID.
    pub fn execution_id(&self) -> Uuid {
        self.execution_id
    }

    /// Get the DAG ID.
    pub async fn dag_id(&self) -> Uuid {
        self.dag.read().await.id()
    }

    /// Execute the DAG to completion.
    ///
    /// This method orchestrates the execution of all tasks in the DAG,
    /// respecting dependencies and concurrency limits.
    pub async fn execute<F, Fut>(&mut self, task_executor: F) -> Result<DagExecutionSummary>
    where
        F: Fn(Task) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<TaskResult>> + Send,
    {
        let dag_id = self.dag.read().await.id();
        self.start_time = Some(Instant::now());

        // Emit start event
        self.emit_event(ExecutionEvent::DagStarted { dag_id });

        tracing::info!(
            dag_id = %dag_id,
            execution_id = %self.execution_id,
            "Starting DAG execution"
        );

        let mut total_tokens = 0u64;
        let mut total_cost = 0.0f64;
        let mut tasks_completed = 0usize;
        let mut tasks_failed = 0usize;

        // Create task result channel
        let (result_sender, mut result_receiver) =
            mpsc::channel::<TaskResult>(self.config.max_concurrent_tasks);

        // Track running tasks
        let running_tasks: Arc<DashMap<TaskId, tokio::task::JoinHandle<()>>> =
            Arc::new(DashMap::new());

        loop {
            // Check if DAG is complete
            {
                let dag = self.dag.read().await;
                if dag.is_complete() {
                    break;
                }
            }

            // Get ready tasks
            let ready_tasks = {
                let dag = self.dag.read().await;
                dag.get_ready_tasks()
            };

            // Launch ready tasks (respecting concurrency limit)
            let available_slots = self
                .config
                .max_concurrent_tasks
                .saturating_sub(running_tasks.len());
            let tasks_to_launch: Vec<_> = ready_tasks.into_iter().take(available_slots).collect();

            for task_id in tasks_to_launch {
                // Get task and mark as running
                let task = {
                    let mut dag = self.dag.write().await;
                    dag.update_task_status(task_id, TaskStatus::Ready)?;
                    dag.get_task(task_id).cloned()
                };

                let Some(task) = task else {
                    continue;
                };

                // Create contract for task
                let contract = AgentContract::new(
                    Uuid::new_v4(), // Agent will be assigned later
                    task_id.0,
                    self.config.default_limits.clone(),
                );

                // Validate against parent contract
                self.contract_enforcer.validate_child_contract(&contract)?;

                self.task_contracts.insert(task_id, contract);

                // Emit task started event
                self.emit_event(ExecutionEvent::TaskStarted { dag_id, task_id });

                // Spawn task execution
                let executor = task_executor.clone();
                let sender = result_sender.clone();
                let dag_lock = self.dag.clone();

                let handle = tokio::spawn(async move {
                    // Update status to running
                    {
                        let mut dag = dag_lock.write().await;
                        if let Some(t) = dag.get_task_mut(task_id) {
                            t.status = TaskStatus::Running;
                            t.started_at = Some(chrono::Utc::now());
                        }
                    }

                    // Execute the task
                    let result = executor(task).await;

                    let task_result = match result {
                        Ok(r) => r,
                        Err(e) => TaskResult {
                            task_id,
                            output: None,
                            error: Some(e.to_string()),
                            tokens_used: 0,
                            cost: 0.0,
                            duration_ms: 0,
                            should_retry: e.is_retryable(),
                        },
                    };

                    // Send result
                    let _ = sender.send(task_result).await;
                });

                running_tasks.insert(task_id, handle);
            }

            // Process completed tasks
            tokio::select! {
                Some(result) = result_receiver.recv() => {
                    running_tasks.remove(&result.task_id);

                    // Update task state
                    let mut dag = self.dag.write().await;

                    if let Some(error) = &result.error {
                        if let Some(task) = dag.get_task_mut(result.task_id) {
                            if result.should_retry && task.should_retry() {
                                // Retry the task
                                task.prepare_retry();
                                self.emit_event(ExecutionEvent::TaskFailed {
                                    dag_id,
                                    task_id: result.task_id,
                                    error: error.clone(),
                                    will_retry: true,
                                });
                            } else {
                                // Mark as failed
                                task.fail(error);
                                tasks_failed += 1;

                                self.emit_event(ExecutionEvent::TaskFailed {
                                    dag_id,
                                    task_id: result.task_id,
                                    error: error.clone(),
                                    will_retry: false,
                                });

                                // Cancel dependents if configured
                                if self.config.cancel_dependents_on_failure {
                                    if let Ok(cancelled) = dag.cancel_dependents(result.task_id) {
                                        for cancelled_id in cancelled {
                                            self.emit_event(ExecutionEvent::TaskCancelled {
                                                dag_id,
                                                task_id: cancelled_id,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some(output) = result.output {
                        if let Some(task) = dag.get_task_mut(result.task_id) {
                            task.complete(output, result.tokens_used, result.cost);
                            total_tokens += result.tokens_used;
                            total_cost += result.cost;
                            tasks_completed += 1;

                            // Update usage tracker
                            self.usage_tracker.record_tokens(result.tokens_used);
                            self.usage_tracker.record_cost(result.cost);
                            self.usage_tracker.record_api_call();

                            self.emit_event(ExecutionEvent::TaskCompleted {
                                dag_id,
                                task_id: result.task_id,
                                tokens: result.tokens_used,
                                cost: result.cost,
                                duration_ms: result.duration_ms,
                            });
                        }
                    }

                    // Remove task contract
                    self.task_contracts.remove(&result.task_id);
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.config.poll_interval_ms)) => {
                    // Continue polling for ready tasks
                }
            }
        }

        // Wait for any remaining tasks
        for entry in running_tasks.iter() {
            let _ = entry.value().is_finished();
        }

        let duration_ms = self
            .start_time
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let stats = self.dag.read().await.stats();

        // Emit completion event
        self.emit_event(ExecutionEvent::DagCompleted {
            dag_id,
            stats: stats.clone(),
            duration_ms,
        });

        tracing::info!(
            dag_id = %dag_id,
            execution_id = %self.execution_id,
            tasks_completed = tasks_completed,
            tasks_failed = tasks_failed,
            total_tokens = total_tokens,
            total_cost = total_cost,
            duration_ms = duration_ms,
            "DAG execution completed"
        );

        Ok(DagExecutionSummary {
            dag_id,
            execution_id: self.execution_id,
            stats,
            total_tokens,
            total_cost,
            duration_ms,
            tasks_completed,
            tasks_failed,
        })
    }

    /// Get current execution statistics.
    pub async fn stats(&self) -> DagStats {
        self.dag.read().await.stats()
    }

    /// Get the usage tracker.
    pub fn usage_tracker(&self) -> &Arc<UsageTracker> {
        &self.usage_tracker
    }

    /// Cancel the DAG execution.
    pub async fn cancel(&self) -> Result<()> {
        let dag_id = self.dag.read().await.id();

        let mut dag = self.dag.write().await;

        // Cancel all pending tasks
        for task_id in dag.get_ready_tasks() {
            dag.update_task_status(task_id, TaskStatus::Cancelled)?;
            self.emit_event(ExecutionEvent::TaskCancelled { dag_id, task_id });
        }

        tracing::info!(dag_id = %dag_id, "DAG execution cancelled");
        Ok(())
    }

    /// Emit an execution event.
    fn emit_event(&self, event: ExecutionEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.event_sender.send(event);
    }
}

/// Summary of a completed DAG execution.
#[derive(Debug, Clone)]
pub struct DagExecutionSummary {
    /// The DAG ID
    pub dag_id: Uuid,
    /// Unique execution ID
    pub execution_id: Uuid,
    /// Final statistics
    pub stats: DagStats,
    /// Total tokens consumed
    pub total_tokens: u64,
    /// Total cost in dollars
    pub total_cost: f64,
    /// Total duration in milliseconds
    pub duration_ms: u64,
    /// Number of tasks completed successfully
    pub tasks_completed: usize,
    /// Number of tasks that failed
    pub tasks_failed: usize,
}

impl DagExecutionSummary {
    /// Check if execution was fully successful.
    pub fn is_success(&self) -> bool {
        self.tasks_failed == 0 && self.stats.cancelled == 0
    }

    /// Get success rate as a percentage.
    pub fn success_rate(&self) -> f64 {
        let total = self.tasks_completed + self.tasks_failed;
        if total == 0 {
            100.0
        } else {
            (self.tasks_completed as f64 / total as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::TaskInput;

    fn create_test_dag() -> TaskDAG {
        let mut dag = TaskDAG::new("test-dag");

        let task_a = Task::new("Task A", TaskInput::default());
        let task_b = Task::new("Task B", TaskInput::default());
        let task_c = Task::new("Task C", TaskInput::default());

        let id_a = dag.add_task(task_a).unwrap();
        let id_b = dag.add_task(task_b).unwrap();
        let id_c = dag.add_task(task_c).unwrap();

        dag.add_dependency(id_a, id_b).unwrap();
        dag.add_dependency(id_b, id_c).unwrap();

        dag
    }

    #[test]
    fn test_executor_creation() {
        let dag = create_test_dag();
        let config = ExecutorConfig::default();
        let executor = DagExecutor::new(dag, config, None);

        assert!(executor.execution_id() != Uuid::nil());
    }

    #[test]
    fn test_executor_config_defaults() {
        let config = ExecutorConfig::default();

        assert_eq!(config.max_concurrent_tasks, 50);
        assert!(config.cancel_dependents_on_failure);
        assert_eq!(config.poll_interval_ms, 50);
    }

    #[tokio::test]
    async fn test_executor_subscription() {
        let dag = create_test_dag();
        let config = ExecutorConfig::default();
        let executor = DagExecutor::new(dag, config, None);

        let _receiver = executor.subscribe();
        // Should be able to subscribe multiple times
        let _receiver2 = executor.subscribe();
    }

    #[tokio::test]
    async fn test_executor_stats() {
        let dag = create_test_dag();
        let config = ExecutorConfig::default();
        let executor = DagExecutor::new(dag, config, None);

        let stats = executor.stats().await;
        assert_eq!(stats.total, 3);
        assert_eq!(stats.pending, 3);
        assert_eq!(stats.completed, 0);
    }

    #[test]
    fn test_execution_summary_success_rate() {
        let summary = DagExecutionSummary {
            dag_id: Uuid::new_v4(),
            execution_id: Uuid::new_v4(),
            stats: DagStats {
                total: 10,
                pending: 0,
                ready: 0,
                running: 0,
                completed: 8,
                failed: 2,
                cancelled: 0,
            },
            total_tokens: 1000,
            total_cost: 0.1,
            duration_ms: 5000,
            tasks_completed: 8,
            tasks_failed: 2,
        };

        assert_eq!(summary.success_rate(), 80.0);
        assert!(!summary.is_success());
    }

    #[test]
    fn test_task_result() {
        let result = TaskResult {
            task_id: TaskId::new(),
            output: Some(TaskOutput::default()),
            error: None,
            tokens_used: 100,
            cost: 0.01,
            duration_ms: 500,
            should_retry: false,
        };

        assert!(result.error.is_none());
        assert!(result.output.is_some());
        assert!(!result.should_retry);
    }
}
