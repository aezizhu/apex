//! WebSocket connection handler with heartbeat and state management.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::auth::{AuthError, Claims};
use super::message::{ClientMessage, ServerMessage};
use super::room::RoomId;
use super::session::{self, WebSocketSession};
use super::{WebSocketConfig, WebSocketState};

/// Unique identifier for a WebSocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// State of a WebSocket connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection established, awaiting authentication
    Connected,
    /// Connection authenticated
    Authenticated,
    /// Connection is closing
    Closing,
    /// Connection closed
    Closed,
}

/// Represents an active WebSocket connection.
pub struct WebSocketConnection {
    pub id: ConnectionId,
    pub session_id: String,
    pub state: ConnectionState,
    pub user_id: Option<String>,
    pub claims: Option<Claims>,
    pub subscriptions: Vec<RoomId>,
    pub connected_at: DateTime<Utc>,
    pub last_activity: Instant,
    pub last_ping: Option<Instant>,
    pub ip_address: Option<IpAddr>,
    pub user_agent: Option<String>,
    pub messages_sent: u64,
    pub messages_received: u64,
    /// Channel to send messages to this connection
    pub sender: mpsc::Sender<ServerMessage>,
}

impl WebSocketConnection {
    pub fn new(sender: mpsc::Sender<ServerMessage>) -> Self {
        Self {
            id: ConnectionId::new(),
            session_id: Uuid::new_v4().to_string(),
            state: ConnectionState::Connected,
            user_id: None,
            claims: None,
            subscriptions: Vec::new(),
            connected_at: Utc::now(),
            last_activity: Instant::now(),
            last_ping: None,
            ip_address: None,
            user_agent: None,
            messages_sent: 0,
            messages_received: 0,
            sender,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.state == ConnectionState::Authenticated
    }

    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn add_subscription(&mut self, room_id: RoomId) {
        if !self.subscriptions.contains(&room_id) {
            self.subscriptions.push(room_id);
        }
    }

    pub fn remove_subscription(&mut self, room_id: &RoomId) {
        self.subscriptions.retain(|r| r != room_id);
    }
}

/// Handler statistics.
#[derive(Debug, Clone, Default)]
pub struct HandlerStats {
    pub active_connections: usize,
    pub total_connections: u64,
    pub total_disconnections: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// Main WebSocket handler managing all connections.
pub struct WebSocketHandler {
    /// Active connections indexed by connection ID
    connections: RwLock<HashMap<ConnectionId, WebSocketConnection>>,
    /// Connection count by IP for rate limiting
    connections_by_ip: RwLock<HashMap<IpAddr, usize>>,
    /// Configuration
    config: WebSocketConfig,
    /// Statistics
    total_connections: AtomicU64,
    total_disconnections: AtomicU64,
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
}

impl WebSocketHandler {
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            connections_by_ip: RwLock::new(HashMap::new()),
            config,
            total_connections: AtomicU64::new(0),
            total_disconnections: AtomicU64::new(0),
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
        }
    }

    /// Register a new connection.
    pub async fn register_connection(
        &self,
        conn: WebSocketConnection,
        ip: Option<IpAddr>,
    ) -> Result<(), &'static str> {
        // Check IP rate limit
        if let Some(ip) = ip {
            let mut by_ip = self.connections_by_ip.write().await;
            let count = by_ip.entry(ip).or_insert(0);

            if self.config.max_connections_per_ip > 0
                && *count >= self.config.max_connections_per_ip
            {
                return Err("Too many connections from this IP");
            }
            *count += 1;
        }

        let conn_id = conn.id;
        self.connections.write().await.insert(conn_id, conn);
        self.total_connections.fetch_add(1, Ordering::Relaxed);

        info!(connection_id = %conn_id, "WebSocket connection registered");
        Ok(())
    }

    /// Unregister a connection.
    pub async fn unregister_connection(&self, conn_id: ConnectionId) {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.remove(&conn_id) {
            // Update IP count
            if let Some(ip) = conn.ip_address {
                let mut by_ip = self.connections_by_ip.write().await;
                if let Some(count) = by_ip.get_mut(&ip) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        by_ip.remove(&ip);
                    }
                }
            }

            self.total_disconnections.fetch_add(1, Ordering::Relaxed);
            info!(connection_id = %conn_id, "WebSocket connection unregistered");
        }
    }

    /// Get a connection by ID.
    pub async fn get_connection(&self, conn_id: ConnectionId) -> Option<WebSocketConnection> {
        // Note: This returns a clone for thread safety
        // For actual send operations, use send_to_connection
        self.connections.read().await.get(&conn_id).map(|c| {
            WebSocketConnection {
                id: c.id,
                session_id: c.session_id.clone(),
                state: c.state.clone(),
                user_id: c.user_id.clone(),
                claims: c.claims.clone(),
                subscriptions: c.subscriptions.clone(),
                connected_at: c.connected_at,
                last_activity: c.last_activity,
                last_ping: c.last_ping,
                ip_address: c.ip_address,
                user_agent: c.user_agent.clone(),
                messages_sent: c.messages_sent,
                messages_received: c.messages_received,
                sender: c.sender.clone(),
            }
        })
    }

    /// Send a message to a specific connection.
    pub async fn send_to_connection(
        &self,
        conn_id: ConnectionId,
        message: ServerMessage,
    ) -> Result<(), &'static str> {
        let connections = self.connections.read().await;

        if let Some(conn) = connections.get(&conn_id) {
            conn.sender
                .send(message)
                .await
                .map_err(|_| "Failed to send message")?;
            self.messages_sent.fetch_add(1, Ordering::Relaxed);
            Ok(())
        } else {
            Err("Connection not found")
        }
    }

    /// Send a message to all connections subscribed to a room.
    pub async fn send_to_room(&self, room_id: &RoomId, message: ServerMessage) {
        let connections = self.connections.read().await;

        for conn in connections.values() {
            if conn.subscriptions.contains(room_id) {
                let _ = conn.sender.send(message.clone()).await;
                self.messages_sent.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Update connection state after authentication.
    pub async fn authenticate_connection(
        &self,
        conn_id: ConnectionId,
        claims: Claims,
    ) -> Result<(), &'static str> {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.get_mut(&conn_id) {
            conn.state = ConnectionState::Authenticated;
            conn.user_id = Some(claims.sub.clone());
            conn.claims = Some(claims);
            Ok(())
        } else {
            Err("Connection not found")
        }
    }

    /// Add subscription to a connection.
    pub async fn add_subscription(
        &self,
        conn_id: ConnectionId,
        room_id: RoomId,
    ) -> Result<(), &'static str> {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.get_mut(&conn_id) {
            conn.add_subscription(room_id);
            Ok(())
        } else {
            Err("Connection not found")
        }
    }

    /// Remove subscription from a connection.
    pub async fn remove_subscription(
        &self,
        conn_id: ConnectionId,
        room_id: &RoomId,
    ) -> Result<(), &'static str> {
        let mut connections = self.connections.write().await;

        if let Some(conn) = connections.get_mut(&conn_id) {
            conn.remove_subscription(room_id);
            Ok(())
        } else {
            Err("Connection not found")
        }
    }

    /// Record received message.
    pub fn record_message_received(&self) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Get handler statistics.
    pub async fn get_stats(&self) -> HandlerStats {
        let connections = self.connections.read().await;

        HandlerStats {
            active_connections: connections.len(),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            total_disconnections: self.total_disconnections.load(Ordering::Relaxed),
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
        }
    }

    /// Check for stale connections and remove them.
    pub async fn cleanup_stale_connections(&self) {
        let timeout = Duration::from_secs(self.config.connection_timeout_secs);
        let mut to_remove = Vec::new();

        {
            let connections = self.connections.read().await;
            for (id, conn) in connections.iter() {
                if conn.last_activity.elapsed() > timeout {
                    to_remove.push(*id);
                }
            }
        }

        for conn_id in to_remove {
            warn!(connection_id = %conn_id, "Removing stale connection");
            self.unregister_connection(conn_id).await;
        }
    }
}

/// Query parameters for WebSocket upgrade.
#[derive(Debug, Deserialize)]
pub struct WsQueryParams {
    pub token: Option<String>,
    pub session_id: Option<String>,
}

/// Handle WebSocket upgrade request.
pub async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsQueryParams>,
    State(state): State<Arc<WebSocketState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, params, state))
}

/// Handle an individual WebSocket connection.
async fn handle_websocket(
    socket: WebSocket,
    params: WsQueryParams,
    state: Arc<WebSocketState>,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Create channel for outgoing messages
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(100);

    // Create connection
    let mut connection = WebSocketConnection::new(tx.clone());
    let conn_id = connection.id;

    // Check for reconnection via session_id query parameter
    let mut recovered_session: Option<WebSocketSession> = None;
    if let Some(ref old_session_id) = params.session_id {
        debug!(session_id = %old_session_id, "Reconnection attempt via query parameter");

        if let Some(ref sm) = state.session_manager {
            match sm.load_session(old_session_id).await {
                Ok(Some(session)) => {
                    info!(
                        session_id = %old_session_id,
                        user_id = ?session.user_id,
                        rooms = ?session.subscribed_rooms,
                        "Session recovered from stored state"
                    );

                    // Restore session ID
                    connection.session_id = session.session_id.clone();

                    // Restore user context if claims were saved
                    if let Some(ref claims_json) = session.user_claims_json {
                        if let Ok(claims) = serde_json::from_str::<Claims>(claims_json) {
                            connection.state = ConnectionState::Authenticated;
                            connection.user_id = Some(claims.sub.clone());
                            connection.claims = Some(claims);
                        }
                    }

                    // Restore subscriptions
                    let room_ids = session::strings_to_room_ids(&session.subscribed_rooms);
                    for room_id in &room_ids {
                        connection.add_subscription(room_id.clone());
                    }

                    recovered_session = Some(session);
                }
                Ok(None) => {
                    warn!(session_id = %old_session_id, "No stored session found for recovery");
                }
                Err(e) => {
                    error!(session_id = %old_session_id, error = %e, "Failed to load session for recovery");
                }
            }
        } else {
            debug!("Session manager not configured, skipping session recovery");
        }
    }

    // Capture session ID after potential recovery
    let session_id = connection.session_id.clone();

    // Pre-authenticate if token provided in query (only if not already authenticated from recovery)
    if let Some(token) = params.token {
        match state.auth.validate_token(&token) {
            Ok(claims) => {
                connection.state = ConnectionState::Authenticated;
                connection.user_id = Some(claims.sub.clone());
                connection.claims = Some(claims);
            }
            Err(e) => {
                warn!(error = %e, "Invalid token in WebSocket query");
            }
        }
    }

    // Register connection
    if let Err(e) = state.handler.register_connection(connection, None).await {
        error!(error = %e, "Failed to register connection");
        return;
    }

    // Send connection acknowledgment
    let connected_msg = ServerMessage::Connected {
        connection_id: conn_id.to_string(),
        server_time: Utc::now(),
        session_id: session_id.clone(),
    };

    if let Ok(json) = serde_json::to_string(&connected_msg) {
        if ws_sender.send(Message::Text(json)).await.is_err() {
            state.handler.unregister_connection(conn_id).await;
            return;
        }
    }

    // If we recovered a session, re-join rooms and replay missed messages
    if let Some(ref session) = recovered_session {
        let room_ids = session::strings_to_room_ids(&session.subscribed_rooms);

        // Re-join rooms in the room manager
        {
            let mut room_manager = state.room_manager.write().await;
            for room_id in &room_ids {
                room_manager.join_room(conn_id, room_id.clone());
            }
        }

        // Re-add subscriptions on the handler side
        for room_id in &room_ids {
            let _ = state.handler.add_subscription(conn_id, room_id.clone()).await;
        }

        // Replay missed messages for each subscribed room
        let last_event_id = session.last_seen_event_id.unwrap_or(0);
        let mut total_missed: usize = 0;

        if let Some(ref sm) = state.session_manager {
            for room_id in &room_ids {
                match sm.get_missed_messages(&room_id.as_str(), last_event_id).await {
                    Ok(missed) => {
                        if !missed.is_empty() {
                            total_missed += missed.len();
                            let replay_msg = ServerMessage::MissedUpdates { updates: missed };
                            let _ = tx.send(replay_msg).await;
                        }
                    }
                    Err(e) => {
                        warn!(
                            room = %room_id.as_str(),
                            error = %e,
                            "Failed to fetch missed messages for room"
                        );
                    }
                }
            }
        }

        // Notify client that session was restored
        let restored_msg = ServerMessage::SessionRestored {
            session_id: session_id.clone(),
            missed_count: total_missed,
        };
        let _ = tx.send(restored_msg).await;

        info!(
            session_id = %session_id,
            rooms_restored = room_ids.len(),
            missed_messages = total_missed,
            "Session fully restored with message replay"
        );
    }

    // Heartbeat interval
    let heartbeat_interval = Duration::from_secs(state.config.heartbeat_interval_secs);
    let mut heartbeat_timer = interval(heartbeat_interval);

    // Message forwarding task (outgoing)
    let forward_handle = {
        let mut ws_sender = ws_sender;
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if ws_sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        })
    };

    // Main message loop
    loop {
        tokio::select! {
            // Incoming message from client
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        state.handler.record_message_received();
                        handle_client_message(&text, conn_id, &state, &tx).await;
                    }
                    Some(Ok(Message::Ping(_data))) => {
                        // Respond with pong
                        let pong_msg = ServerMessage::Pong {
                            client_timestamp: None,
                            server_timestamp: Utc::now().timestamp_millis(),
                        };
                        let _ = tx.send(pong_msg).await;
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!(connection_id = %conn_id, "Client requested close");
                        break;
                    }
                    Some(Err(e)) => {
                        error!(connection_id = %conn_id, error = %e, "WebSocket error");
                        break;
                    }
                    None => {
                        info!(connection_id = %conn_id, "WebSocket stream ended");
                        break;
                    }
                    _ => {}
                }
            }

            // Heartbeat timer
            _ = heartbeat_timer.tick() => {
                let heartbeat = ServerMessage::Heartbeat {
                    timestamp: Utc::now().timestamp_millis(),
                };
                if tx.send(heartbeat).await.is_err() {
                    break;
                }
            }
        }
    }

    // Cleanup: persist session before tearing down so the client can reconnect
    if let Some(ref sm) = state.session_manager {
        // Gather current connection state for persistence
        if let Some(conn) = state.handler.get_connection(conn_id).await {
            let claims_json = conn
                .claims
                .as_ref()
                .and_then(|c| serde_json::to_string(c).ok());

            let ws_session = WebSocketSession {
                session_id: conn.session_id.clone(),
                user_id: conn.user_id.clone(),
                subscribed_rooms: session::room_ids_to_strings(&conn.subscriptions),
                last_seen_event_id: None, // Client should provide via protocol
                user_claims_json: claims_json,
                created_at_ms: conn.connected_at.timestamp_millis(),
                last_active_ms: Utc::now().timestamp_millis(),
            };

            if let Err(e) = sm.save_session(&ws_session).await {
                error!(
                    session_id = %conn.session_id,
                    error = %e,
                    "Failed to persist session on disconnect"
                );
            } else {
                debug!(session_id = %conn.session_id, "Session persisted for future reconnection");
            }
        }
    }

    forward_handle.abort();
    state.handler.unregister_connection(conn_id).await;

    // Remove from all rooms
    let mut room_manager = state.room_manager.write().await;
    room_manager.remove_connection_from_all(conn_id);

    info!(connection_id = %conn_id, "WebSocket connection closed");
}

/// Handle a message from the client.
async fn handle_client_message(
    text: &str,
    conn_id: ConnectionId,
    state: &Arc<WebSocketState>,
    tx: &mpsc::Sender<ServerMessage>,
) {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            let error_msg = ServerMessage::Error(super::message::ErrorNotification {
                error_id: Uuid::new_v4().to_string(),
                code: "INVALID_MESSAGE".to_string(),
                message: format!("Invalid message format: {}", e),
                severity: super::message::ErrorSeverity::Warning,
                source: super::message::ErrorSource::Connection,
                related_id: None,
                details: None,
                timestamp: Utc::now(),
                recoverable: true,
                suggested_action: Some("Check message format".to_string()),
            });
            let _ = tx.send(error_msg).await;
            return;
        }
    };

    match msg {
        ClientMessage::Authenticate { token } => {
            match state.auth.validate_token(&token) {
                Ok(claims) => {
                    if let Err(e) = state.handler.authenticate_connection(conn_id, claims.clone()).await {
                        error!(error = %e, "Failed to authenticate connection");
                        return;
                    }

                    let response = ServerMessage::Authenticated {
                        user_id: claims.sub,
                        permissions: claims.permissions,
                        expires_at: claims.exp,
                    };
                    let _ = tx.send(response).await;
                }
                Err(e) => {
                    let response = ServerMessage::AuthenticationFailed {
                        reason: e.to_string(),
                        code: match e {
                            AuthError::Expired => "TOKEN_EXPIRED",
                            AuthError::Invalid(_) => "INVALID_TOKEN",
                            AuthError::MissingClaims => "MISSING_CLAIMS",
                        }.to_string(),
                    };
                    let _ = tx.send(response).await;
                }
            }
        }

        ClientMessage::Subscribe { target } => {
            let room_id: RoomId = (&target).into();

            // Add to room
            {
                let mut room_manager = state.room_manager.write().await;
                room_manager.join_room(conn_id, room_id.clone());
            }

            // Add subscription to connection
            if let Err(e) = state.handler.add_subscription(conn_id, room_id.clone()).await {
                error!(error = %e, "Failed to add subscription");
                return;
            }

            // Send confirmation with current state
            let response = ServerMessage::Subscribed {
                target,
                current_state: None, // Clients should send GetState after subscribing
            };
            let _ = tx.send(response).await;

            debug!(connection_id = %conn_id, room = ?room_id, "Client subscribed");
        }

        ClientMessage::Unsubscribe { target } => {
            let room_id: RoomId = (&target).into();

            // Remove from room
            {
                let mut room_manager = state.room_manager.write().await;
                room_manager.leave_room(conn_id, &room_id);
            }

            // Remove subscription from connection
            if let Err(e) = state.handler.remove_subscription(conn_id, &room_id).await {
                error!(error = %e, "Failed to remove subscription");
                return;
            }

            let response = ServerMessage::Unsubscribed { target };
            let _ = tx.send(response).await;

            debug!(connection_id = %conn_id, room = ?room_id, "Client unsubscribed");
        }

        ClientMessage::Ping { timestamp } => {
            let response = ServerMessage::Pong {
                client_timestamp: timestamp,
                server_timestamp: Utc::now().timestamp_millis(),
            };
            let _ = tx.send(response).await;
        }

        ClientMessage::ApprovalResponse(response) => {
            // Process approval response
            let result_msg = ServerMessage::ApprovalResult {
                request_id: response.request_id.clone(),
                approved: response.approved,
                approver: state.handler.get_connection(conn_id).await.and_then(|c| c.user_id.clone()),
                comment: response.comment,
            };

            // Broadcast to approval room
            state.broadcaster.broadcast_to_room(&RoomId::Approvals, result_msg).await;

            info!(
                request_id = %response.request_id,
                approved = response.approved,
                "Approval response received"
            );
        }

        ClientMessage::GetState { target } => {
            debug!(connection_id = %conn_id, target = ?target, "State request received");

            // Fetch recent messages for the requested room from the session store
            let room_id: RoomId = (&target).into();

            if let Some(ref sm) = state.session_manager {
                match sm.get_missed_messages(&room_id.as_str(), 0).await {
                    Ok(messages) => {
                        if !messages.is_empty() {
                            let state_msg = ServerMessage::MissedUpdates { updates: messages };
                            let _ = tx.send(state_msg).await;
                        } else {
                            // Send subscribed with no state
                            let response = ServerMessage::Subscribed {
                                target,
                                current_state: None,
                            };
                            let _ = tx.send(response).await;
                        }
                    }
                    Err(e) => {
                        warn!(
                            connection_id = %conn_id,
                            error = %e,
                            "Failed to fetch state for target"
                        );
                    }
                }
            } else {
                debug!(connection_id = %conn_id, "Session manager not configured, cannot fetch state");
            }
        }

        ClientMessage::Reconnect { session_id, last_message_id } => {
            debug!(
                connection_id = %conn_id,
                session_id = %session_id,
                last_message_id = ?last_message_id,
                "Reconnection request via message"
            );

            if let Some(ref sm) = state.session_manager {
                match sm.load_session(&session_id).await {
                    Ok(Some(session)) => {
                        let last_id = last_message_id
                            .map(|id| id as i64)
                            .or(session.last_seen_event_id)
                            .unwrap_or(0);

                        let room_ids = session::strings_to_room_ids(&session.subscribed_rooms);
                        let mut all_missed = Vec::new();

                        for room_id in &room_ids {
                            // Re-join room
                            {
                                let mut room_manager = state.room_manager.write().await;
                                room_manager.join_room(conn_id, room_id.clone());
                            }
                            let _ = state.handler.add_subscription(conn_id, room_id.clone()).await;

                            // Gather missed messages
                            match sm.get_missed_messages(&room_id.as_str(), last_id).await {
                                Ok(missed) => all_missed.extend(missed),
                                Err(e) => {
                                    warn!(room = %room_id.as_str(), error = %e, "Failed to get missed messages");
                                }
                            }
                        }

                        let reconnected_msg = ServerMessage::Reconnected {
                            session_id: session_id.clone(),
                            missed_messages: all_missed,
                        };
                        let _ = tx.send(reconnected_msg).await;

                        info!(
                            connection_id = %conn_id,
                            session_id = %session_id,
                            rooms = room_ids.len(),
                            "Reconnection completed via message"
                        );
                    }
                    Ok(None) => {
                        warn!(session_id = %session_id, "Session not found for reconnection");
                        let error_msg = ServerMessage::Error(super::message::ErrorNotification {
                            error_id: Uuid::new_v4().to_string(),
                            code: "SESSION_NOT_FOUND".to_string(),
                            message: format!("Session {} not found or expired", session_id),
                            severity: super::message::ErrorSeverity::Warning,
                            source: super::message::ErrorSource::Connection,
                            related_id: Some(session_id),
                            details: None,
                            timestamp: Utc::now(),
                            recoverable: true,
                            suggested_action: Some("Create a new connection".to_string()),
                        });
                        let _ = tx.send(error_msg).await;
                    }
                    Err(e) => {
                        error!(session_id = %session_id, error = %e, "Failed to load session for reconnection");
                    }
                }
            } else {
                debug!("Session manager not configured, cannot reconnect");
            }
        }

        ClientMessage::SessionRestore { session_id, last_event_id } => {
            debug!(
                connection_id = %conn_id,
                session_id = %session_id,
                last_event_id = ?last_event_id,
                "Session restore request"
            );

            if let Some(ref sm) = state.session_manager {
                match sm.load_session(&session_id).await {
                    Ok(Some(session)) => {
                        let since_id = last_event_id
                            .or(session.last_seen_event_id)
                            .unwrap_or(0);

                        // Restore user context
                        if let Some(ref claims_json) = session.user_claims_json {
                            if let Ok(claims) = serde_json::from_str::<Claims>(claims_json) {
                                let _ = state.handler.authenticate_connection(conn_id, claims).await;
                            }
                        }

                        let room_ids = session::strings_to_room_ids(&session.subscribed_rooms);
                        let mut total_missed: usize = 0;

                        for room_id in &room_ids {
                            {
                                let mut room_manager = state.room_manager.write().await;
                                room_manager.join_room(conn_id, room_id.clone());
                            }
                            let _ = state.handler.add_subscription(conn_id, room_id.clone()).await;

                            match sm.get_missed_messages(&room_id.as_str(), since_id).await {
                                Ok(missed) => {
                                    if !missed.is_empty() {
                                        total_missed += missed.len();
                                        let replay = ServerMessage::MissedUpdates { updates: missed };
                                        let _ = tx.send(replay).await;
                                    }
                                }
                                Err(e) => {
                                    warn!(room = %room_id.as_str(), error = %e, "Failed to replay messages");
                                }
                            }
                        }

                        let restored = ServerMessage::SessionRestored {
                            session_id: session_id.clone(),
                            missed_count: total_missed,
                        };
                        let _ = tx.send(restored).await;

                        info!(
                            connection_id = %conn_id,
                            session_id = %session_id,
                            missed_count = total_missed,
                            "Session restored via SessionRestore message"
                        );
                    }
                    Ok(None) => {
                        warn!(session_id = %session_id, "Session not found for restore");
                        let error_msg = ServerMessage::Error(super::message::ErrorNotification {
                            error_id: Uuid::new_v4().to_string(),
                            code: "SESSION_NOT_FOUND".to_string(),
                            message: format!("Session {} not found or expired", session_id),
                            severity: super::message::ErrorSeverity::Warning,
                            source: super::message::ErrorSource::Connection,
                            related_id: Some(session_id),
                            details: None,
                            timestamp: Utc::now(),
                            recoverable: true,
                            suggested_action: Some("Create a new connection".to_string()),
                        });
                        let _ = tx.send(error_msg).await;
                    }
                    Err(e) => {
                        error!(session_id = %session_id, error = %e, "Failed to load session for restore");
                    }
                }
            } else {
                debug!("Session manager not configured, cannot restore session");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_id() {
        let id1 = ConnectionId::new();
        let id2 = ConnectionId::new();
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_handler_creation() {
        let config = WebSocketConfig::default();
        let handler = WebSocketHandler::new(config);

        let stats = handler.get_stats().await;
        assert_eq!(stats.active_connections, 0);
    }
}
