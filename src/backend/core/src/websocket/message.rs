//! WebSocket message types and serialization.
//!
//! Defines all message types for client-server communication over WebSocket.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::room::RoomId;

// ═══════════════════════════════════════════════════════════════════════════════
// Client Messages (Client -> Server)
// ═══════════════════════════════════════════════════════════════════════════════

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Authenticate the connection
    Authenticate {
        token: String,
    },

    /// Subscribe to updates for a resource
    Subscribe {
        target: SubscriptionTarget,
    },

    /// Unsubscribe from updates
    Unsubscribe {
        target: SubscriptionTarget,
    },

    /// Ping to keep connection alive
    Ping {
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<i64>,
    },

    /// Respond to an approval request
    ApprovalResponse(ApprovalResponse),

    /// Request current state of a resource
    GetState {
        target: SubscriptionTarget,
    },

    /// Request reconnection with session ID
    Reconnect {
        session_id: String,
        last_message_id: Option<u64>,
    },

    /// Restore a previous session on a new connection
    SessionRestore {
        session_id: String,
        last_event_id: Option<i64>,
    },
}

/// Target resource for subscriptions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "resource", rename_all = "snake_case")]
pub enum SubscriptionTarget {
    /// Subscribe to a specific task's updates
    Task { id: String },

    /// Subscribe to all tasks (admin only)
    AllTasks,

    /// Subscribe to a specific agent's updates
    Agent { id: String },

    /// Subscribe to all agents
    AllAgents,

    /// Subscribe to a specific DAG's updates
    Dag { id: String },

    /// Subscribe to all DAGs
    AllDags,

    /// Subscribe to system metrics
    Metrics {
        #[serde(default = "default_metrics_interval")]
        interval_secs: u64,
    },

    /// Subscribe to approval notifications
    Approvals,

    /// Subscribe to error notifications
    Errors,
}

fn default_metrics_interval() -> u64 {
    5
}

impl From<&SubscriptionTarget> for RoomId {
    fn from(target: &SubscriptionTarget) -> Self {
        match target {
            SubscriptionTarget::Task { id } => RoomId::Task(id.clone()),
            SubscriptionTarget::AllTasks => RoomId::AllTasks,
            SubscriptionTarget::Agent { id } => RoomId::Agent(id.clone()),
            SubscriptionTarget::AllAgents => RoomId::AllAgents,
            SubscriptionTarget::Dag { id } => RoomId::Dag(id.clone()),
            SubscriptionTarget::AllDags => RoomId::AllDags,
            SubscriptionTarget::Metrics { .. } => RoomId::Metrics,
            SubscriptionTarget::Approvals => RoomId::Approvals,
            SubscriptionTarget::Errors => RoomId::Errors,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Server Messages (Server -> Client)
// ═══════════════════════════════════════════════════════════════════════════════

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Connection acknowledgment
    Connected {
        connection_id: String,
        server_time: DateTime<Utc>,
        session_id: String,
    },

    /// Authentication result
    Authenticated {
        user_id: String,
        permissions: Vec<String>,
        expires_at: DateTime<Utc>,
    },

    /// Authentication failed
    AuthenticationFailed {
        reason: String,
        code: String,
    },

    /// Subscription confirmed
    Subscribed {
        target: SubscriptionTarget,
        current_state: Option<serde_json::Value>,
    },

    /// Unsubscription confirmed
    Unsubscribed {
        target: SubscriptionTarget,
    },

    /// Pong response to ping
    Pong {
        client_timestamp: Option<i64>,
        server_timestamp: i64,
    },

    /// Task update notification
    TaskUpdate(TaskUpdate),

    /// Agent update notification
    AgentUpdate(AgentUpdate),

    /// DAG update notification
    DagUpdate(DagUpdate),

    /// Metrics snapshot
    Metrics(MetricsSnapshot),

    /// Approval required notification
    ApprovalRequired(ApprovalRequest),

    /// Approval result notification
    ApprovalResult {
        request_id: String,
        approved: bool,
        approver: Option<String>,
        comment: Option<String>,
    },

    /// Error notification
    Error(ErrorNotification),

    /// Reconnection result
    Reconnected {
        session_id: String,
        missed_messages: Vec<ServerMessage>,
    },

    /// Session successfully restored from stored state
    SessionRestored {
        session_id: String,
        missed_count: usize,
    },

    /// Batch of missed updates sent during session recovery
    MissedUpdates {
        updates: Vec<ServerMessage>,
    },

    /// Heartbeat from server
    Heartbeat {
        timestamp: i64,
    },

    /// Connection will close
    Closing {
        reason: String,
        code: u16,
    },
}

impl ServerMessage {
    /// Get a unique message ID for tracking.
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::Connected { .. } => "connected",
            Self::Authenticated { .. } => "authenticated",
            Self::AuthenticationFailed { .. } => "authentication_failed",
            Self::Subscribed { .. } => "subscribed",
            Self::Unsubscribed { .. } => "unsubscribed",
            Self::Pong { .. } => "pong",
            Self::TaskUpdate(_) => "task_update",
            Self::AgentUpdate(_) => "agent_update",
            Self::DagUpdate(_) => "dag_update",
            Self::Metrics(_) => "metrics",
            Self::ApprovalRequired(_) => "approval_required",
            Self::ApprovalResult { .. } => "approval_result",
            Self::Error(_) => "error",
            Self::Reconnected { .. } => "reconnected",
            Self::SessionRestored { .. } => "session_restored",
            Self::MissedUpdates { .. } => "missed_updates",
            Self::Heartbeat { .. } => "heartbeat",
            Self::Closing { .. } => "closing",
        }
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Update Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Task status update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUpdate {
    pub task_id: String,
    pub dag_id: Option<String>,
    pub status: TaskStatusUpdate,
    pub progress: Option<TaskProgress>,
    pub tokens_used: u64,
    pub cost_dollars: f64,
    pub duration_ms: Option<i64>,
    pub timestamp: DateTime<Utc>,
}

/// Task status values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatusUpdate {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

/// Task progress information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub current_step: Option<String>,
    pub steps_completed: u32,
    pub total_steps: Option<u32>,
    pub percentage: Option<f32>,
    pub message: Option<String>,
}

/// Agent status update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUpdate {
    pub agent_id: String,
    pub name: String,
    pub status: AgentStatusUpdate,
    pub current_load: u32,
    pub max_load: u32,
    pub success_rate: f64,
    pub current_task_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Agent status values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatusUpdate {
    Idle,
    Busy,
    Paused,
    Error,
    Offline,
}

/// DAG status update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagUpdate {
    pub dag_id: String,
    pub name: String,
    pub status: DagStatusUpdate,
    pub tasks_total: usize,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub tasks_running: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub timestamp: DateTime<Utc>,
}

/// DAG status values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DagStatusUpdate {
    Pending,
    Running,
    Completed,
    Failed,
    PartiallyCompleted,
    Cancelled,
}

/// System metrics snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: DateTime<Utc>,
    pub agents: AgentMetrics,
    pub tasks: TaskMetrics,
    pub resources: ResourceMetrics,
    pub system: SystemMetrics,
}

/// Agent-related metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub total: u64,
    pub active: u64,
    pub idle: u64,
    pub errored: u64,
    pub avg_success_rate: f64,
    pub total_tasks_completed: u64,
}

/// Task-related metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub queued: u64,
    pub running: u64,
    pub completed_last_hour: u64,
    pub failed_last_hour: u64,
    pub avg_duration_ms: f64,
    pub p99_duration_ms: f64,
}

/// Resource usage metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetrics {
    pub total_tokens_used: u64,
    pub total_cost_dollars: f64,
    pub tokens_last_hour: u64,
    pub cost_last_hour: f64,
    pub budget_remaining: Option<f64>,
}

/// System health metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub active_connections: u64,
    pub db_pool_available: u32,
    pub cache_hit_rate: f64,
    pub uptime_seconds: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approval Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Request for human approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub request_id: String,
    pub task_id: String,
    pub dag_id: Option<String>,
    pub agent_id: String,
    pub approval_type: ApprovalType,
    pub title: String,
    pub description: String,
    pub details: serde_json::Value,
    pub timeout_secs: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub required_permissions: Vec<String>,
}

/// Types of approvals that can be requested.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalType {
    /// Approval for budget/cost increase
    BudgetIncrease,
    /// Approval for sensitive tool execution
    SensitiveTool,
    /// Approval for external API call
    ExternalApi,
    /// Approval for data access
    DataAccess,
    /// Approval for code execution
    CodeExecution,
    /// Generic human-in-the-loop approval
    HumanReview,
    /// Custom approval type
    Custom { category: String },
}

/// Response to an approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    pub request_id: String,
    pub approved: bool,
    pub comment: Option<String>,
    pub modified_params: Option<serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Error Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Error notification sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorNotification {
    pub error_id: String,
    pub code: String,
    pub message: String,
    pub severity: ErrorSeverity,
    pub source: ErrorSource,
    pub related_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub recoverable: bool,
    pub suggested_action: Option<String>,
}

/// Error severity levels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Source of the error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSource {
    Task { task_id: String },
    Agent { agent_id: String },
    Dag { dag_id: String },
    System,
    Connection,
    Authentication,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Message Envelope (for message ordering and acknowledgment)
// ═══════════════════════════════════════════════════════════════════════════════

/// Wrapper for messages with sequence number for ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MessageEnvelope {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub message: ServerMessage,
    pub requires_ack: bool,
}

#[allow(dead_code)]
impl MessageEnvelope {
    pub fn new(sequence: u64, message: ServerMessage) -> Self {
        Self {
            sequence,
            timestamp: Utc::now(),
            message,
            requires_ack: false,
        }
    }

    pub fn with_ack(mut self) -> Self {
        self.requires_ack = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::Subscribe {
            target: SubscriptionTarget::Task {
                id: "task-123".to_string(),
            },
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("task-123"));

        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            ClientMessage::Subscribe { target } => match target {
                SubscriptionTarget::Task { id } => assert_eq!(id, "task-123"),
                _ => panic!("Wrong target type"),
            },
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_server_message_serialization() {
        let msg = ServerMessage::TaskUpdate(TaskUpdate {
            task_id: "task-123".to_string(),
            dag_id: Some("dag-456".to_string()),
            status: TaskStatusUpdate::Running,
            progress: Some(TaskProgress {
                current_step: Some("Processing".to_string()),
                steps_completed: 2,
                total_steps: Some(5),
                percentage: Some(40.0),
                message: None,
            }),
            tokens_used: 1000,
            cost_dollars: 0.01,
            duration_ms: Some(500),
            timestamp: Utc::now(),
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("task_update"));
        assert!(json.contains("task-123"));
    }

    #[test]
    fn test_approval_request_serialization() {
        let request = ApprovalRequest {
            request_id: Uuid::new_v4().to_string(),
            task_id: "task-123".to_string(),
            dag_id: None,
            agent_id: "agent-456".to_string(),
            approval_type: ApprovalType::SensitiveTool,
            title: "Execute shell command".to_string(),
            description: "Agent wants to run: ls -la".to_string(),
            details: serde_json::json!({"command": "ls -la"}),
            timeout_secs: 300,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(300),
            required_permissions: vec!["admin".to_string()],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("sensitive_tool"));
    }

    #[test]
    fn test_subscription_target_to_room_id() {
        let target = SubscriptionTarget::Task {
            id: "test-task".to_string(),
        };
        let room_id: RoomId = (&target).into();
        assert_eq!(room_id, RoomId::Task("test-task".to_string()));
    }
}
