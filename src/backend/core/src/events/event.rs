//! Event definitions and domain events for event sourcing.
//!
//! This module provides:
//! - Event trait for defining domain events
//! - EventEnvelope for metadata-wrapped events
//! - Domain events for tasks, agents, and DAGs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::any::Any;
use tracing::instrument;
use uuid::Uuid;

use crate::agents::AgentId;
use crate::dag::{TaskId, TaskOutput, TaskStatus};
use crate::error::ApexError;

// =============================================================================
// Event IDs
// =============================================================================

/// Unique identifier for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a stream (aggregate).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamId(pub String);

impl StreamId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Create a stream ID for a task.
    pub fn task(task_id: TaskId) -> Self {
        Self(format!("task-{}", task_id.0))
    }

    /// Create a stream ID for an agent.
    pub fn agent(agent_id: AgentId) -> Self {
        Self(format!("agent-{}", agent_id.0))
    }

    /// Create a stream ID for a DAG.
    pub fn dag(dag_id: Uuid) -> Self {
        Self(format!("dag-{}", dag_id))
    }
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Event Metadata
// =============================================================================

/// Metadata associated with an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique event identifier
    pub event_id: EventId,

    /// Event type name
    pub event_type: String,

    /// Stream/aggregate this event belongs to
    pub stream_id: StreamId,

    /// Version of this event in the stream (for optimistic concurrency)
    pub version: u64,

    /// When the event occurred
    pub timestamp: DateTime<Utc>,

    /// Correlation ID for distributed tracing
    pub correlation_id: Option<String>,

    /// Causation ID (the event that caused this event)
    pub causation_id: Option<EventId>,

    /// User/agent that triggered this event
    pub actor_id: Option<String>,

    /// Additional context as JSON
    #[serde(default)]
    pub context: serde_json::Value,
}

impl EventMetadata {
    /// Create new metadata for an event.
    pub fn new(event_type: impl Into<String>, stream_id: StreamId, version: u64) -> Self {
        Self {
            event_id: EventId::new(),
            event_type: event_type.into(),
            stream_id,
            version,
            timestamp: Utc::now(),
            correlation_id: None,
            causation_id: None,
            actor_id: None,
            context: serde_json::Value::Null,
        }
    }

    /// Set the correlation ID.
    pub fn with_correlation(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Set the causation ID.
    pub fn with_causation(mut self, id: EventId) -> Self {
        self.causation_id = Some(id);
        self
    }

    /// Set the actor ID.
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor_id = Some(actor.into());
        self
    }

    /// Set additional context.
    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = context;
        self
    }
}

// =============================================================================
// Event Trait
// =============================================================================

/// Trait for domain events.
///
/// Events are immutable facts that have occurred in the system.
/// They should be named in past tense (e.g., TaskCreated, TaskCompleted).
pub trait Event: Send + Sync + std::fmt::Debug {
    /// Get the event type name.
    fn event_type(&self) -> &'static str;

    /// Get the stream ID this event belongs to.
    fn stream_id(&self) -> StreamId;

    /// Convert the event to a boxed Any for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Serialize the event to JSON.
    fn to_json(&self) -> serde_json::Result<serde_json::Value>;
}

/// Type-erased event for storage.
pub trait StorableEvent: Event {
    /// Clone the event into a box.
    fn clone_box(&self) -> Box<dyn StorableEvent>;
}

impl<T: Event + Clone + 'static> StorableEvent for T {
    fn clone_box(&self) -> Box<dyn StorableEvent> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn StorableEvent> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// =============================================================================
// Event Envelope
// =============================================================================

/// An event wrapped with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<E> {
    /// The event data
    pub event: E,

    /// Event metadata
    pub metadata: EventMetadata,
}

impl<E: Event + Serialize> EventEnvelope<E> {
    /// Create a new envelope for an event.
    pub fn new(event: E, version: u64) -> Self {
        let metadata = EventMetadata::new(event.event_type(), event.stream_id(), version);
        Self { event, metadata }
    }

    /// Create with custom metadata.
    pub fn with_metadata(event: E, metadata: EventMetadata) -> Self {
        Self { event, metadata }
    }

    /// Get the event ID.
    pub fn id(&self) -> EventId {
        self.metadata.event_id
    }

    /// Get the event type.
    pub fn event_type(&self) -> &str {
        &self.metadata.event_type
    }

    /// Get the stream ID.
    pub fn stream_id(&self) -> &StreamId {
        &self.metadata.stream_id
    }

    /// Get the version.
    pub fn version(&self) -> u64 {
        self.metadata.version
    }

    /// Get the timestamp.
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.metadata.timestamp
    }
}

/// Type-erased event envelope for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Event metadata
    pub metadata: EventMetadata,

    /// Serialized event data
    pub data: serde_json::Value,
}

impl StoredEvent {
    /// Create from an event envelope.
    pub fn from_envelope<E: Event + Serialize>(envelope: &EventEnvelope<E>) -> serde_json::Result<Self> {
        Ok(Self {
            metadata: envelope.metadata.clone(),
            data: serde_json::to_value(&envelope.event)?,
        })
    }

    /// Deserialize into a typed envelope.
    pub fn into_envelope<E: Event + Serialize + for<'de> Deserialize<'de>>(
        self,
    ) -> serde_json::Result<EventEnvelope<E>> {
        let event: E = serde_json::from_value(self.data)?;
        Ok(EventEnvelope::with_metadata(event, self.metadata))
    }
}

// =============================================================================
// Domain Events - Task Events
// =============================================================================

/// Event: A task was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreated {
    pub task_id: TaskId,
    pub name: String,
    pub instruction: String,
    pub parent_id: Option<TaskId>,
    pub priority: i32,
    pub max_retries: u32,
}

impl Event for TaskCreated {
    fn event_type(&self) -> &'static str {
        "TaskCreated"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task was assigned to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssigned {
    pub task_id: TaskId,
    pub agent_id: Uuid,
    pub contract_id: Option<Uuid>,
}

impl Event for TaskAssigned {
    fn event_type(&self) -> &'static str {
        "TaskAssigned"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task started execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStarted {
    pub task_id: TaskId,
    pub agent_id: Uuid,
    pub started_at: DateTime<Utc>,
}

impl Event for TaskStarted {
    fn event_type(&self) -> &'static str {
        "TaskStarted"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task completed successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompleted {
    pub task_id: TaskId,
    pub output: TaskOutput,
    pub tokens_used: u64,
    pub cost_dollars: f64,
    pub duration_ms: i64,
    pub completed_at: DateTime<Utc>,
}

impl Event for TaskCompleted {
    fn event_type(&self) -> &'static str {
        "TaskCompleted"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFailed {
    pub task_id: TaskId,
    pub error: String,
    pub retry_count: u32,
    pub is_retryable: bool,
    pub failed_at: DateTime<Utc>,
}

impl Event for TaskFailed {
    fn event_type(&self) -> &'static str {
        "TaskFailed"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task was retried.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRetried {
    pub task_id: TaskId,
    pub retry_count: u32,
    pub reason: String,
}

impl Event for TaskRetried {
    fn event_type(&self) -> &'static str {
        "TaskRetried"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task was cancelled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCancelled {
    pub task_id: TaskId,
    pub reason: String,
    pub cancelled_by: Option<String>,
    pub cancelled_at: DateTime<Utc>,
}

impl Event for TaskCancelled {
    fn event_type(&self) -> &'static str {
        "TaskCancelled"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task's status changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusChanged {
    pub task_id: TaskId,
    pub from_status: TaskStatus,
    pub to_status: TaskStatus,
    pub changed_at: DateTime<Utc>,
}

impl Event for TaskStatusChanged {
    fn event_type(&self) -> &'static str {
        "TaskStatusChanged"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Domain Events - Agent Events
// =============================================================================

/// Event: An agent was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCreated {
    pub agent_id: AgentId,
    pub name: String,
    pub model: String,
    pub max_load: u32,
}

impl Event for AgentCreated {
    fn event_type(&self) -> &'static str {
        "AgentCreated"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::agent(self.agent_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: An agent started working on a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskStarted {
    pub agent_id: AgentId,
    pub task_id: TaskId,
    pub current_load: u32,
}

impl Event for AgentTaskStarted {
    fn event_type(&self) -> &'static str {
        "AgentTaskStarted"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::agent(self.agent_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: An agent finished a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskFinished {
    pub agent_id: AgentId,
    pub task_id: TaskId,
    pub success: bool,
    pub tokens_used: u64,
    pub cost_dollars: f64,
}

impl Event for AgentTaskFinished {
    fn event_type(&self) -> &'static str {
        "AgentTaskFinished"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::agent(self.agent_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: An agent's status changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusChanged {
    pub agent_id: AgentId,
    pub from_status: String,
    pub to_status: String,
}

impl Event for AgentStatusChanged {
    fn event_type(&self) -> &'static str {
        "AgentStatusChanged"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::agent(self.agent_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Domain Events - DAG Events
// =============================================================================

/// Event: A DAG was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagCreated {
    pub dag_id: Uuid,
    pub name: String,
    pub task_count: usize,
}

impl Event for DagCreated {
    fn event_type(&self) -> &'static str {
        "DagCreated"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::dag(self.dag_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A task was added to a DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskAdded {
    pub dag_id: Uuid,
    pub task_id: TaskId,
    pub dependencies: Vec<TaskId>,
}

impl Event for DagTaskAdded {
    fn event_type(&self) -> &'static str {
        "DagTaskAdded"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::dag(self.dag_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A DAG execution started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagExecutionStarted {
    pub dag_id: Uuid,
    pub started_at: DateTime<Utc>,
}

impl Event for DagExecutionStarted {
    fn event_type(&self) -> &'static str {
        "DagExecutionStarted"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::dag(self.dag_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A DAG execution completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagExecutionCompleted {
    pub dag_id: Uuid,
    pub success: bool,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub duration_ms: i64,
    pub completed_at: DateTime<Utc>,
}

impl Event for DagExecutionCompleted {
    fn event_type(&self) -> &'static str {
        "DagExecutionCompleted"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::dag(self.dag_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Domain Events - Contract Events
// =============================================================================

/// Event: A resource limit was reached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitReached {
    pub contract_id: Uuid,
    pub task_id: Option<TaskId>,
    pub limit_type: String, // "tokens", "cost", "time", "api_calls"
    pub used: f64,
    pub limit: f64,
}

impl Event for ResourceLimitReached {
    fn event_type(&self) -> &'static str {
        "ResourceLimitReached"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::new(format!("contract-{}", self.contract_id))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Domain Events - Tool Events
// =============================================================================

/// Event: A tool was called during task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCalled {
    pub task_id: TaskId,
    pub tool_name: String,
    pub params: serde_json::Value,
}

impl Event for ToolCalled {
    fn event_type(&self) -> &'static str {
        "ToolCalled"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: A tool returned a result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub task_id: TaskId,
    pub tool_name: String,
    pub success: bool,
    pub latency_ms: i64,
}

impl Event for ToolResult {
    fn event_type(&self) -> &'static str {
        "ToolResult"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::task(self.task_id)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Domain Events - Approval Events
// =============================================================================

/// Event: An approval was requested for a risky action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequested {
    pub approval_id: Uuid,
    pub action: String,
    pub risk_score: f64,
}

impl Event for ApprovalRequested {
    fn event_type(&self) -> &'static str {
        "ApprovalRequested"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::new(format!("approval-{}", self.approval_id))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

/// Event: An approval decision was made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecided {
    pub approval_id: Uuid,
    pub decision: String,
    pub decided_by: String,
}

impl Event for ApprovalDecided {
    fn event_type(&self) -> &'static str {
        "ApprovalDecided"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::new(format!("approval-{}", self.approval_id))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Domain Events - Contract Creation Event
// =============================================================================

/// Event: A contract was created for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractCreated {
    pub contract_id: Uuid,
    pub agent_id: Uuid,
    pub limits: serde_json::Value,
}

impl Event for ContractCreated {
    fn event_type(&self) -> &'static str {
        "ContractCreated"
    }

    fn stream_id(&self) -> StreamId {
        StreamId::new(format!("contract-{}", self.contract_id))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Event Enumeration
// =============================================================================

/// All domain events in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    // Task events
    TaskCreated(TaskCreated),
    TaskAssigned(TaskAssigned),
    TaskStarted(TaskStarted),
    TaskCompleted(TaskCompleted),
    TaskFailed(TaskFailed),
    TaskRetried(TaskRetried),
    TaskCancelled(TaskCancelled),
    TaskStatusChanged(TaskStatusChanged),

    // Agent events
    AgentCreated(AgentCreated),
    AgentTaskStarted(AgentTaskStarted),
    AgentTaskFinished(AgentTaskFinished),
    AgentStatusChanged(AgentStatusChanged),

    // DAG events
    DagCreated(DagCreated),
    DagTaskAdded(DagTaskAdded),
    DagExecutionStarted(DagExecutionStarted),
    DagExecutionCompleted(DagExecutionCompleted),

    // Contract events
    ResourceLimitReached(ResourceLimitReached),
    ContractCreated(ContractCreated),

    // Tool events
    ToolCalled(ToolCalled),
    ToolResult(ToolResult),

    // Approval events
    ApprovalRequested(ApprovalRequested),
    ApprovalDecided(ApprovalDecided),
}

impl Event for DomainEvent {
    fn event_type(&self) -> &'static str {
        match self {
            DomainEvent::TaskCreated(_) => "TaskCreated",
            DomainEvent::TaskAssigned(_) => "TaskAssigned",
            DomainEvent::TaskStarted(_) => "TaskStarted",
            DomainEvent::TaskCompleted(_) => "TaskCompleted",
            DomainEvent::TaskFailed(_) => "TaskFailed",
            DomainEvent::TaskRetried(_) => "TaskRetried",
            DomainEvent::TaskCancelled(_) => "TaskCancelled",
            DomainEvent::TaskStatusChanged(_) => "TaskStatusChanged",
            DomainEvent::AgentCreated(_) => "AgentCreated",
            DomainEvent::AgentTaskStarted(_) => "AgentTaskStarted",
            DomainEvent::AgentTaskFinished(_) => "AgentTaskFinished",
            DomainEvent::AgentStatusChanged(_) => "AgentStatusChanged",
            DomainEvent::DagCreated(_) => "DagCreated",
            DomainEvent::DagTaskAdded(_) => "DagTaskAdded",
            DomainEvent::DagExecutionStarted(_) => "DagExecutionStarted",
            DomainEvent::DagExecutionCompleted(_) => "DagExecutionCompleted",
            DomainEvent::ResourceLimitReached(_) => "ResourceLimitReached",
            DomainEvent::ContractCreated(_) => "ContractCreated",
            DomainEvent::ToolCalled(_) => "ToolCalled",
            DomainEvent::ToolResult(_) => "ToolResult",
            DomainEvent::ApprovalRequested(_) => "ApprovalRequested",
            DomainEvent::ApprovalDecided(_) => "ApprovalDecided",
        }
    }

    fn stream_id(&self) -> StreamId {
        match self {
            DomainEvent::TaskCreated(e) => e.stream_id(),
            DomainEvent::TaskAssigned(e) => e.stream_id(),
            DomainEvent::TaskStarted(e) => e.stream_id(),
            DomainEvent::TaskCompleted(e) => e.stream_id(),
            DomainEvent::TaskFailed(e) => e.stream_id(),
            DomainEvent::TaskRetried(e) => e.stream_id(),
            DomainEvent::TaskCancelled(e) => e.stream_id(),
            DomainEvent::TaskStatusChanged(e) => e.stream_id(),
            DomainEvent::AgentCreated(e) => e.stream_id(),
            DomainEvent::AgentTaskStarted(e) => e.stream_id(),
            DomainEvent::AgentTaskFinished(e) => e.stream_id(),
            DomainEvent::AgentStatusChanged(e) => e.stream_id(),
            DomainEvent::DagCreated(e) => e.stream_id(),
            DomainEvent::DagTaskAdded(e) => e.stream_id(),
            DomainEvent::DagExecutionStarted(e) => e.stream_id(),
            DomainEvent::DagExecutionCompleted(e) => e.stream_id(),
            DomainEvent::ResourceLimitReached(e) => e.stream_id(),
            DomainEvent::ContractCreated(e) => e.stream_id(),
            DomainEvent::ToolCalled(e) => e.stream_id(),
            DomainEvent::ToolResult(e) => e.stream_id(),
            DomainEvent::ApprovalRequested(e) => e.stream_id(),
            DomainEvent::ApprovalDecided(e) => e.stream_id(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn to_json(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }
}

// =============================================================================
// Event Store
// =============================================================================

/// Persistent event store backed by PostgreSQL.
///
/// All domain events are appended to an immutable log. The store supports:
/// - Appending events with full metadata
/// - Replaying events for a specific entity
/// - Replaying events in a time range (debugging / auditing)
/// - Querying by trace_id (distributed tracing correlation)
/// - Reconstructing aggregate state at a point in time
pub struct EventStore {
    pool: PgPool,
}

impl EventStore {
    /// Create a new EventStore with the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Append a domain event to the immutable log.
    ///
    /// The event is serialized to JSONB and stored alongside trace/entity metadata.
    /// Returns the generated `EventId`.
    #[instrument(skip(self, event), fields(event_type = %event.event_type()))]
    pub async fn append(&self, event: DomainEvent) -> Result<EventId, ApexError> {
        let event_id = EventId::new();
        let event_type = event.event_type().to_string();
        let stream_id = event.stream_id();

        // Derive entity_type and entity_id from the stream_id format "type-uuid".
        let (entity_type, entity_id) = parse_stream_id(&stream_id.0);

        let payload = serde_json::to_value(&event)?;
        let metadata = serde_json::json!({
            "stream_id": stream_id.0,
        });

        sqlx::query(
            r#"
            INSERT INTO events (event_id, event_type, entity_type, entity_id, payload, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
        )
        .bind(event_id.0)
        .bind(&event_type)
        .bind(&entity_type)
        .bind(entity_id)
        .bind(&payload)
        .bind(&metadata)
        .execute(&self.pool)
        .await?;

        tracing::debug!(event_id = %event_id, event_type = %event_type, "Event appended");

        Ok(event_id)
    }

    /// Replay all events for a specific entity, ordered by creation time.
    #[instrument(skip(self))]
    pub async fn replay(
        &self,
        entity_id: Uuid,
        entity_type: &str,
    ) -> Result<Vec<DomainEvent>, ApexError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event_id, trace_id, span_id, event_type, entity_type, entity_id,
                   payload, metadata, created_at
            FROM events
            WHERE entity_id = $1 AND entity_type = $2
            ORDER BY created_at ASC
            "#,
        )
        .bind(entity_id)
        .bind(entity_type)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| deserialize_domain_event(&row.payload))
            .collect()
    }

    /// Replay events created within a time range, ordered by creation time.
    ///
    /// Useful for debugging and auditing.
    #[instrument(skip(self))]
    pub async fn replay_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<DomainEvent>, ApexError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event_id, trace_id, span_id, event_type, entity_type, entity_id,
                   payload, metadata, created_at
            FROM events
            WHERE created_at >= $1 AND created_at <= $2
            ORDER BY created_at ASC
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| deserialize_domain_event(&row.payload))
            .collect()
    }

    /// Get all events associated with a distributed trace ID.
    #[instrument(skip(self))]
    pub async fn get_by_trace(&self, trace_id: &str) -> Result<Vec<DomainEvent>, ApexError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event_id, trace_id, span_id, event_type, entity_type, entity_id,
                   payload, metadata, created_at
            FROM events
            WHERE trace_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(trace_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| deserialize_domain_event(&row.payload))
            .collect()
    }

    /// Reconstruct an aggregate's state at a specific point in time.
    ///
    /// Replays all events for the entity up to (and including) the given timestamp,
    /// folding them through the `Aggregate::apply` method.
    #[instrument(skip(self))]
    pub async fn reconstruct_state<S: super::aggregate::Aggregate>(
        &self,
        entity_id: Uuid,
        at: DateTime<Utc>,
    ) -> Result<S, ApexError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event_id, trace_id, span_id, event_type, entity_type, entity_id,
                   payload, metadata, created_at
            FROM events
            WHERE entity_id = $1 AND created_at <= $2
            ORDER BY created_at ASC
            "#,
        )
        .bind(entity_id)
        .bind(at)
        .fetch_all(&self.pool)
        .await?;

        let mut aggregate = S::default();
        for row in &rows {
            let event = deserialize_domain_event(&row.payload)?;
            aggregate.apply(&event);
        }

        Ok(aggregate)
    }
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Row type for reading events from the database.
#[derive(Debug, sqlx::FromRow)]
struct EventRow {
    #[allow(dead_code)]
    event_id: Uuid,
    #[allow(dead_code)]
    trace_id: Option<String>,
    #[allow(dead_code)]
    span_id: Option<String>,
    #[allow(dead_code)]
    event_type: String,
    #[allow(dead_code)]
    entity_type: String,
    #[allow(dead_code)]
    entity_id: Uuid,
    payload: serde_json::Value,
    #[allow(dead_code)]
    metadata: Option<serde_json::Value>,
    #[allow(dead_code)]
    created_at: DateTime<Utc>,
}

/// Deserialize a `DomainEvent` from a JSONB payload.
fn deserialize_domain_event(payload: &serde_json::Value) -> Result<DomainEvent, ApexError> {
    serde_json::from_value(payload.clone()).map_err(ApexError::from)
}

/// Parse a stream ID string (e.g. "task-<uuid>") into (entity_type, entity_id).
/// Falls back to ("unknown", nil UUID) if parsing fails.
fn parse_stream_id(stream_id: &str) -> (String, Uuid) {
    if let Some(idx) = stream_id.find('-') {
        let entity_type = &stream_id[..idx];
        let id_str = &stream_id[idx + 1..];
        match Uuid::parse_str(id_str) {
            Ok(id) => (entity_type.to_string(), id),
            Err(_) => (entity_type.to_string(), Uuid::nil()),
        }
    } else {
        ("unknown".to_string(), Uuid::nil())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_id_generation() {
        let id1 = EventId::new();
        let id2 = EventId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_stream_id_creation() {
        let task_id = TaskId::new();
        let stream = StreamId::task(task_id);
        assert!(stream.0.starts_with("task-"));
    }

    #[test]
    fn test_event_metadata() {
        let stream_id = StreamId::new("test-stream");
        let metadata = EventMetadata::new("TestEvent", stream_id, 1)
            .with_correlation("corr-123")
            .with_actor("user-1");

        assert_eq!(metadata.event_type, "TestEvent");
        assert_eq!(metadata.version, 1);
        assert_eq!(metadata.correlation_id, Some("corr-123".to_string()));
        assert_eq!(metadata.actor_id, Some("user-1".to_string()));
    }

    #[test]
    fn test_task_created_event() {
        let task_id = TaskId::new();
        let event = TaskCreated {
            task_id,
            name: "Test Task".to_string(),
            instruction: "Do something".to_string(),
            parent_id: None,
            priority: 5,
            max_retries: 3,
        };

        assert_eq!(event.event_type(), "TaskCreated");
        assert!(event.stream_id().0.starts_with("task-"));
    }

    #[test]
    fn test_event_envelope() {
        let task_id = TaskId::new();
        let event = TaskCreated {
            task_id,
            name: "Test".to_string(),
            instruction: "Test".to_string(),
            parent_id: None,
            priority: 0,
            max_retries: 3,
        };

        let envelope = EventEnvelope::new(event.clone(), 1);
        assert_eq!(envelope.version(), 1);
        assert_eq!(envelope.event_type(), "TaskCreated");
    }

    #[test]
    fn test_stored_event_roundtrip() {
        let task_id = TaskId::new();
        let event = TaskCreated {
            task_id,
            name: "Test".to_string(),
            instruction: "Test".to_string(),
            parent_id: None,
            priority: 0,
            max_retries: 3,
        };

        let envelope = EventEnvelope::new(event, 1);
        let stored = StoredEvent::from_envelope(&envelope).unwrap();
        let restored: EventEnvelope<TaskCreated> = stored.into_envelope().unwrap();

        assert_eq!(restored.event.name, "Test");
        assert_eq!(restored.version(), 1);
    }

    #[test]
    fn test_domain_event_serialization() {
        let task_id = TaskId::new();
        let event = DomainEvent::TaskCreated(TaskCreated {
            task_id,
            name: "Test".to_string(),
            instruction: "Test".to_string(),
            parent_id: None,
            priority: 0,
            max_retries: 3,
        });

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TaskCreated"));

        let restored: DomainEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.event_type(), "TaskCreated");
    }
}
