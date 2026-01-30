//! Task definitions and state management.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Unique identifier for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a task in the execution lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is waiting for dependencies to complete
    Pending,
    /// All dependencies are complete, task is ready to execute
    Ready,
    /// Task is currently being executed by an agent
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed after all retry attempts
    Failed,
    /// Task was cancelled (either manually or due to parent failure)
    Cancelled,
}

impl TaskStatus {
    /// Check if transition to another status is valid.
    pub fn can_transition_to(&self, target: &TaskStatus) -> bool {
        use TaskStatus::*;
        matches!(
            (self, target),
            (Pending, Ready)
                | (Pending, Cancelled)
                | (Ready, Running)
                | (Ready, Cancelled)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, Cancelled)
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled)
    }
}

/// Input data for a task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskInput {
    /// The main instruction/prompt for the task
    pub instruction: String,

    /// Additional context (e.g., from parent tasks)
    #[serde(default)]
    pub context: serde_json::Value,

    /// Input parameters
    #[serde(default)]
    pub parameters: serde_json::Value,

    /// Files or artifacts to process
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

/// Output data from a completed task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskOutput {
    /// The main result/response
    pub result: String,

    /// Structured data output
    #[serde(default)]
    pub data: serde_json::Value,

    /// Generated artifacts
    #[serde(default)]
    pub artifacts: Vec<Artifact>,

    /// Reasoning/thought process (for debugging)
    #[serde(default)]
    pub reasoning: Option<String>,
}

/// An artifact (file, image, etc.) associated with a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub url: Option<String>,
    pub content_hash: Option<String>,
}

/// A task in the DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier
    pub id: TaskId,

    /// Parent task ID (if this is a subtask)
    pub parent_id: Option<TaskId>,

    /// Human-readable name
    pub name: String,

    /// Current status
    pub status: TaskStatus,

    /// Task priority (higher = more urgent)
    pub priority: i32,

    /// Input data
    pub input: TaskInput,

    /// Output data (populated on completion)
    pub output: Option<TaskOutput>,

    /// Error message (populated on failure)
    pub error: Option<String>,

    /// ID of the agent assigned to this task
    pub agent_id: Option<Uuid>,

    /// Contract ID governing this task
    pub contract_id: Option<Uuid>,

    /// Number of retry attempts made
    pub retry_count: u32,

    /// Maximum retry attempts allowed
    pub max_retries: u32,

    /// Tokens consumed by this task
    pub tokens_used: u64,

    /// Cost in dollars
    pub cost_dollars: f64,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// When execution started
    pub started_at: Option<DateTime<Utc>>,

    /// When execution completed
    pub completed_at: Option<DateTime<Utc>>,

    /// Distributed tracing context
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

impl Task {
    /// Create a new task with the given name and input.
    pub fn new(name: impl Into<String>, input: TaskInput) -> Self {
        Self {
            id: TaskId::new(),
            parent_id: None,
            name: name.into(),
            status: TaskStatus::Pending,
            priority: 0,
            input,
            output: None,
            error: None,
            agent_id: None,
            contract_id: None,
            retry_count: 0,
            max_retries: 3,
            tokens_used: 0,
            cost_dollars: 0.0,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            trace_id: None,
            span_id: None,
        }
    }

    /// Create a subtask of this task.
    pub fn create_subtask(&self, name: impl Into<String>, input: TaskInput) -> Self {
        let mut subtask = Self::new(name, input);
        subtask.parent_id = Some(self.id);
        subtask.trace_id = self.trace_id.clone();
        subtask
    }

    /// Mark task as started.
    pub fn start(&mut self, agent_id: Uuid) {
        self.status = TaskStatus::Running;
        self.agent_id = Some(agent_id);
        self.started_at = Some(Utc::now());
    }

    /// Mark task as completed with output.
    pub fn complete(&mut self, output: TaskOutput, tokens: u64, cost: f64) {
        self.status = TaskStatus::Completed;
        self.output = Some(output);
        self.tokens_used = tokens;
        self.cost_dollars = cost;
        self.completed_at = Some(Utc::now());
    }

    /// Mark task as failed with error.
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = TaskStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(Utc::now());
    }

    /// Check if task should be retried.
    pub fn should_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry counter and reset for retry.
    pub fn prepare_retry(&mut self) {
        self.retry_count += 1;
        self.status = TaskStatus::Pending;
        self.error = None;
        self.started_at = None;
        self.completed_at = None;
    }

    /// Get task duration in milliseconds (if started).
    pub fn duration_ms(&self) -> Option<i64> {
        let started = self.started_at?;
        let ended = self.completed_at.unwrap_or_else(Utc::now);
        Some((ended - started).num_milliseconds())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_lifecycle() {
        let mut task = Task::new("Test Task", TaskInput::default());

        assert_eq!(task.status, TaskStatus::Pending);

        let agent_id = Uuid::new_v4();
        task.start(agent_id);
        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());

        task.complete(TaskOutput::default(), 100, 0.01);
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.completed_at.is_some());
        assert_eq!(task.tokens_used, 100);
    }

    #[test]
    fn test_retry_logic() {
        let mut task = Task::new("Test Task", TaskInput::default());
        task.max_retries = 2;

        assert!(task.should_retry()); // 0 < 2

        task.prepare_retry();
        assert!(task.should_retry()); // 1 < 2

        task.prepare_retry();
        assert!(!task.should_retry()); // 2 < 2 is false
    }
}
