//! Comprehensive WebSocket module for real-time communication.
//!
//! This module provides:
//! - Connection management with heartbeat/ping-pong
//! - Room/channel-based subscriptions for task, agent, and DAG updates
//! - Real-time metrics streaming
//! - Approval notifications
//! - WebSocket authentication
//! - Efficient broadcasting to many clients
//! - Graceful disconnection and reconnection support

mod handler;
mod message;
mod room;
mod broadcast;
mod auth;
mod session;

pub use handler::{
    WebSocketHandler,
    WebSocketConnection,
    ConnectionId,
    ConnectionState,
    ws_upgrade_handler,
};
pub use message::{
    ClientMessage,
    ServerMessage,
    SubscriptionTarget,
    MetricsSnapshot,
    ApprovalRequest,
    ApprovalResponse,
    TaskUpdate,
    AgentUpdate,
    DagUpdate,
};
pub use room::{Room, RoomId, RoomManager, RoomType};
pub use broadcast::{Broadcaster, BroadcastMessage, BroadcastStats};
pub use auth::{WebSocketAuth, AuthToken, AuthError, Claims};
pub use session::{SessionManager, WebSocketSession};

use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for WebSocket connections.
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Heartbeat interval in seconds
    pub heartbeat_interval_secs: u64,
    /// Connection timeout after missed heartbeats
    pub connection_timeout_secs: u64,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// Maximum connections per IP (0 = unlimited)
    pub max_connections_per_ip: usize,
    /// Enable compression
    pub enable_compression: bool,
    /// Maximum reconnection attempts
    pub max_reconnection_attempts: u32,
    /// Reconnection backoff multiplier
    pub reconnection_backoff_ms: u64,
    /// JWT secret for authentication
    pub jwt_secret: String,
    /// Token expiration in seconds
    pub token_expiration_secs: u64,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 30,
            connection_timeout_secs: 90,
            max_message_size: 1024 * 1024, // 1MB
            max_connections_per_ip: 10,
            enable_compression: true,
            max_reconnection_attempts: 5,
            reconnection_backoff_ms: 1000,
            jwt_secret: "change-me-in-production".to_string(),
            token_expiration_secs: 3600,
        }
    }
}

/// Shared state for the WebSocket system.
pub struct WebSocketState {
    /// WebSocket handler for connection management
    pub handler: Arc<WebSocketHandler>,
    /// Room manager for subscriptions
    pub room_manager: Arc<RwLock<RoomManager>>,
    /// Broadcaster for efficient message distribution
    pub broadcaster: Arc<Broadcaster>,
    /// Authentication handler
    pub auth: Arc<WebSocketAuth>,
    /// Session manager for persistence and recovery
    pub session_manager: Option<Arc<SessionManager>>,
    /// Configuration
    pub config: WebSocketConfig,
}

impl WebSocketState {
    /// Create a new WebSocket state with the given configuration.
    pub fn new(config: WebSocketConfig) -> Self {
        let auth = Arc::new(WebSocketAuth::new(
            config.jwt_secret.clone(),
            config.token_expiration_secs,
        ));
        let broadcaster = Arc::new(Broadcaster::new(1024)); // 1024 message buffer
        let room_manager = Arc::new(RwLock::new(RoomManager::new()));
        let handler = Arc::new(WebSocketHandler::new(config.clone()));

        Self {
            handler,
            room_manager,
            broadcaster,
            auth,
            session_manager: None,
            config,
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(WebSocketConfig::default())
    }

    /// Attach a Redis-backed session manager for session persistence and recovery.
    pub fn with_session_manager(mut self, redis_client: redis::Client) -> Self {
        self.session_manager = Some(Arc::new(SessionManager::new(redis_client)));
        self
    }

    /// Broadcast a task update to all subscribed clients.
    pub async fn broadcast_task_update(&self, update: TaskUpdate) {
        let room_id = RoomId::Task(update.task_id.clone());
        let message = ServerMessage::TaskUpdate(update);
        self.broadcaster.broadcast_to_room(&room_id, message).await;
    }

    /// Broadcast an agent update to all subscribed clients.
    pub async fn broadcast_agent_update(&self, update: AgentUpdate) {
        let room_id = RoomId::Agent(update.agent_id.clone());
        let message = ServerMessage::AgentUpdate(update);
        self.broadcaster.broadcast_to_room(&room_id, message).await;
    }

    /// Broadcast a DAG update to all subscribed clients.
    pub async fn broadcast_dag_update(&self, update: DagUpdate) {
        let room_id = RoomId::Dag(update.dag_id.clone());
        let message = ServerMessage::DagUpdate(update);
        self.broadcaster.broadcast_to_room(&room_id, message).await;
    }

    /// Broadcast metrics to all subscribed clients.
    pub async fn broadcast_metrics(&self, metrics: MetricsSnapshot) {
        let room_id = RoomId::Metrics;
        let message = ServerMessage::Metrics(metrics);
        self.broadcaster.broadcast_to_room(&room_id, message).await;
    }

    /// Send an approval request to relevant clients.
    pub async fn send_approval_request(&self, request: ApprovalRequest) {
        let room_id = RoomId::Approvals;
        let message = ServerMessage::ApprovalRequired(request);
        self.broadcaster.broadcast_to_room(&room_id, message).await;
    }

    /// Get connection statistics.
    pub async fn get_stats(&self) -> WebSocketStats {
        let handler_stats = self.handler.get_stats().await;
        let broadcast_stats = self.broadcaster.get_stats();
        let room_manager = self.room_manager.read().await;
        let room_count = room_manager.room_count();

        WebSocketStats {
            active_connections: handler_stats.active_connections,
            total_connections: handler_stats.total_connections,
            total_disconnections: handler_stats.total_disconnections,
            messages_sent: handler_stats.messages_sent,
            messages_received: handler_stats.messages_received,
            active_rooms: room_count,
            broadcasts_sent: broadcast_stats.total_broadcasts,
            broadcasts_delivered: broadcast_stats.total_delivered,
            broadcasts_failed: broadcast_stats.total_failed,
            messages_in_queue: broadcast_stats.messages_in_queue,
            active_broadcast_subscribers: broadcast_stats.active_subscribers,
        }
    }
}

/// Statistics about the WebSocket system.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WebSocketStats {
    pub active_connections: usize,
    pub total_connections: u64,
    pub total_disconnections: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub active_rooms: usize,
    pub broadcasts_sent: u64,
    pub broadcasts_delivered: u64,
    pub broadcasts_failed: u64,
    pub messages_in_queue: usize,
    pub active_broadcast_subscribers: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_config_default() {
        let config = WebSocketConfig::default();
        assert_eq!(config.heartbeat_interval_secs, 30);
        assert_eq!(config.connection_timeout_secs, 90);
        assert_eq!(config.max_message_size, 1024 * 1024);
    }

    #[tokio::test]
    async fn test_websocket_state_creation() {
        let state = WebSocketState::with_defaults();
        let stats = state.get_stats().await;
        assert_eq!(stats.active_connections, 0);
    }
}
