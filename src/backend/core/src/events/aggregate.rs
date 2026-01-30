//! Aggregate trait and implementations for event-sourced state reconstruction.
//!
//! Aggregates are domain objects that can be rebuilt from a stream of events.
//! Each aggregate implements `Default` (empty state) and `apply` (fold an event).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agents::AgentId;
use crate::dag::{TaskId, TaskStatus};

use super::event::DomainEvent;

// =============================================================================
// Aggregate Trait
// =============================================================================

/// Trait for aggregates that can be reconstructed from a sequence of domain events.
///
/// An aggregate starts at its `Default` state and folds each event via `apply`.
/// This provides full temporal reconstruction -- given the same event stream,
/// the resulting state is deterministic.
pub trait Aggregate: Default {
    /// Apply a single domain event to mutate state.
    ///
    /// Implementations must be pure functions of `(self, event) -> self'`.
    /// They must not perform I/O or fail -- every persisted event is valid by definition.
    fn apply(&mut self, event: &DomainEvent);
}

// =============================================================================
// Task Aggregate
// =============================================================================

/// Reconstructed state of a task derived from its event stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskAggregate {
    pub task_id: Option<TaskId>,
    pub name: String,
    pub instruction: String,
    pub status: Option<TaskStatus>,
    pub assigned_agent: Option<Uuid>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub tokens_used: u64,
    pub cost_dollars: f64,
    pub duration_ms: Option<i64>,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancelled_by: Option<String>,
    /// Number of events applied (useful for optimistic concurrency).
    pub version: u64,
}

impl Aggregate for TaskAggregate {
    fn apply(&mut self, event: &DomainEvent) {
        self.version += 1;

        match event {
            DomainEvent::TaskCreated(e) => {
                self.task_id = Some(e.task_id);
                self.name = e.name.clone();
                self.instruction = e.instruction.clone();
                self.max_retries = e.max_retries;
                self.status = Some(TaskStatus::Pending);
            }
            DomainEvent::TaskAssigned(e) => {
                self.assigned_agent = Some(e.agent_id);
            }
            DomainEvent::TaskStarted(e) => {
                self.assigned_agent = Some(e.agent_id);
                self.started_at = Some(e.started_at);
                self.status = Some(TaskStatus::Running);
            }
            DomainEvent::TaskCompleted(e) => {
                self.tokens_used = e.tokens_used;
                self.cost_dollars = e.cost_dollars;
                self.duration_ms = Some(e.duration_ms);
                self.output = Some(serde_json::to_value(&e.output).unwrap_or_default());
                self.completed_at = Some(e.completed_at);
                self.status = Some(TaskStatus::Completed);
            }
            DomainEvent::TaskFailed(e) => {
                self.error = Some(e.error.clone());
                self.retry_count = e.retry_count;
                self.failed_at = Some(e.failed_at);
                if !e.is_retryable || e.retry_count >= self.max_retries {
                    self.status = Some(TaskStatus::Failed);
                }
            }
            DomainEvent::TaskRetried(e) => {
                self.retry_count = e.retry_count;
                // Task goes back to pending/ready for re-execution.
                self.status = Some(TaskStatus::Pending);
            }
            DomainEvent::TaskCancelled(e) => {
                self.cancelled_at = Some(e.cancelled_at);
                self.cancelled_by = e.cancelled_by.clone();
                self.status = Some(TaskStatus::Cancelled);
            }
            DomainEvent::TaskStatusChanged(e) => {
                self.status = Some(e.to_status.clone());
            }
            // Ignore events that do not pertain to tasks.
            _ => {}
        }
    }
}

// =============================================================================
// Agent Aggregate
// =============================================================================

/// Reconstructed state of an agent derived from its event stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentAggregate {
    pub agent_id: Option<AgentId>,
    pub name: String,
    pub model: String,
    pub max_load: u32,
    pub current_load: u32,
    pub status: String,
    pub total_tasks_started: u64,
    pub total_tasks_finished: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub total_tokens_used: u64,
    pub total_cost_dollars: f64,
    /// Number of events applied.
    pub version: u64,
}

impl Aggregate for AgentAggregate {
    fn apply(&mut self, event: &DomainEvent) {
        self.version += 1;

        match event {
            DomainEvent::AgentCreated(e) => {
                self.agent_id = Some(e.agent_id);
                self.name = e.name.clone();
                self.model = e.model.clone();
                self.max_load = e.max_load;
                self.status = "idle".to_string();
            }
            DomainEvent::AgentTaskStarted(e) => {
                self.current_load = e.current_load;
                self.total_tasks_started += 1;
            }
            DomainEvent::AgentTaskFinished(e) => {
                self.total_tasks_finished += 1;
                self.total_tokens_used += e.tokens_used;
                self.total_cost_dollars += e.cost_dollars;
                if e.success {
                    self.total_successes += 1;
                } else {
                    self.total_failures += 1;
                }
                // Decrement load (saturating to avoid underflow).
                self.current_load = self.current_load.saturating_sub(1);
            }
            DomainEvent::AgentStatusChanged(e) => {
                self.status = e.to_status.clone();
            }
            _ => {}
        }
    }
}

// =============================================================================
// DAG Aggregate
// =============================================================================

/// Reconstructed state of a DAG execution derived from its event stream.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DagAggregate {
    pub dag_id: Option<Uuid>,
    pub name: String,
    pub task_count: usize,
    pub tasks: Vec<TaskId>,
    pub is_running: bool,
    pub is_completed: bool,
    pub success: Option<bool>,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub duration_ms: Option<i64>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Number of events applied.
    pub version: u64,
}

impl Aggregate for DagAggregate {
    fn apply(&mut self, event: &DomainEvent) {
        self.version += 1;

        match event {
            DomainEvent::DagCreated(e) => {
                self.dag_id = Some(e.dag_id);
                self.name = e.name.clone();
                self.task_count = e.task_count;
            }
            DomainEvent::DagTaskAdded(e) => {
                self.tasks.push(e.task_id);
                self.task_count = self.tasks.len();
            }
            DomainEvent::DagExecutionStarted(e) => {
                self.is_running = true;
                self.started_at = Some(e.started_at);
            }
            DomainEvent::DagExecutionCompleted(e) => {
                self.is_running = false;
                self.is_completed = true;
                self.success = Some(e.success);
                self.completed_tasks = e.completed_tasks;
                self.failed_tasks = e.failed_tasks;
                self.total_tokens = e.total_tokens;
                self.total_cost = e.total_cost;
                self.duration_ms = Some(e.duration_ms);
                self.completed_at = Some(e.completed_at);
            }
            _ => {}
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::event::*;

    #[test]
    fn test_task_aggregate_lifecycle() {
        let task_id = TaskId::new();
        let agent_id = Uuid::new_v4();
        let now = Utc::now();

        let events = vec![
            DomainEvent::TaskCreated(TaskCreated {
                task_id,
                name: "Summarize doc".to_string(),
                instruction: "Please summarize".to_string(),
                parent_id: None,
                priority: 5,
                max_retries: 3,
            }),
            DomainEvent::TaskAssigned(TaskAssigned {
                task_id,
                agent_id,
                contract_id: None,
            }),
            DomainEvent::TaskStarted(TaskStarted {
                task_id,
                agent_id,
                started_at: now,
            }),
            DomainEvent::TaskCompleted(TaskCompleted {
                task_id,
                output: crate::dag::TaskOutput {
                    result: "Summary here".to_string(),
                    data: serde_json::Value::Null,
                    artifacts: vec![],
                    reasoning: None,
                },
                tokens_used: 1500,
                cost_dollars: 0.003,
                duration_ms: 2400,
                completed_at: now,
            }),
        ];

        let mut agg = TaskAggregate::default();
        for e in &events {
            agg.apply(e);
        }

        assert_eq!(agg.task_id, Some(task_id));
        assert_eq!(agg.name, "Summarize doc");
        assert_eq!(agg.status, Some(TaskStatus::Completed));
        assert_eq!(agg.tokens_used, 1500);
        assert_eq!(agg.version, 4);
        assert!(agg.assigned_agent.is_some());
    }

    #[test]
    fn test_task_aggregate_failure_and_retry() {
        let task_id = TaskId::new();
        let now = Utc::now();

        let events = vec![
            DomainEvent::TaskCreated(TaskCreated {
                task_id,
                name: "Risky task".to_string(),
                instruction: "Try hard".to_string(),
                parent_id: None,
                priority: 1,
                max_retries: 3,
            }),
            DomainEvent::TaskFailed(TaskFailed {
                task_id,
                error: "Timeout".to_string(),
                retry_count: 1,
                is_retryable: true,
                failed_at: now,
            }),
            DomainEvent::TaskRetried(TaskRetried {
                task_id,
                retry_count: 1,
                reason: "Auto-retry".to_string(),
            }),
        ];

        let mut agg = TaskAggregate::default();
        for e in &events {
            agg.apply(e);
        }

        assert_eq!(agg.status, Some(TaskStatus::Pending));
        assert_eq!(agg.retry_count, 1);
    }

    #[test]
    fn test_agent_aggregate() {
        let agent_id = AgentId::new();
        let task_id = TaskId::new();

        let events = vec![
            DomainEvent::AgentCreated(AgentCreated {
                agent_id,
                name: "GPT-4 worker".to_string(),
                model: "gpt-4".to_string(),
                max_load: 5,
            }),
            DomainEvent::AgentTaskStarted(AgentTaskStarted {
                agent_id,
                task_id,
                current_load: 1,
            }),
            DomainEvent::AgentTaskFinished(AgentTaskFinished {
                agent_id,
                task_id,
                success: true,
                tokens_used: 800,
                cost_dollars: 0.002,
            }),
        ];

        let mut agg = AgentAggregate::default();
        for e in &events {
            agg.apply(e);
        }

        assert_eq!(agg.agent_id, Some(agent_id));
        assert_eq!(agg.model, "gpt-4");
        assert_eq!(agg.total_tasks_started, 1);
        assert_eq!(agg.total_successes, 1);
        assert_eq!(agg.total_tokens_used, 800);
        assert_eq!(agg.current_load, 0);
    }

    #[test]
    fn test_dag_aggregate() {
        let dag_id = Uuid::new_v4();
        let task_a = TaskId::new();
        let task_b = TaskId::new();
        let now = Utc::now();

        let events = vec![
            DomainEvent::DagCreated(DagCreated {
                dag_id,
                name: "Pipeline".to_string(),
                task_count: 2,
            }),
            DomainEvent::DagTaskAdded(DagTaskAdded {
                dag_id,
                task_id: task_a,
                dependencies: vec![],
            }),
            DomainEvent::DagTaskAdded(DagTaskAdded {
                dag_id,
                task_id: task_b,
                dependencies: vec![task_a],
            }),
            DomainEvent::DagExecutionStarted(DagExecutionStarted {
                dag_id,
                started_at: now,
            }),
            DomainEvent::DagExecutionCompleted(DagExecutionCompleted {
                dag_id,
                success: true,
                completed_tasks: 2,
                failed_tasks: 0,
                total_tokens: 3000,
                total_cost: 0.006,
                duration_ms: 5000,
                completed_at: now,
            }),
        ];

        let mut agg = DagAggregate::default();
        for e in &events {
            agg.apply(e);
        }

        assert_eq!(agg.dag_id, Some(dag_id));
        assert_eq!(agg.tasks.len(), 2);
        assert!(agg.is_completed);
        assert_eq!(agg.success, Some(true));
        assert_eq!(agg.total_tokens, 3000);
    }
}
