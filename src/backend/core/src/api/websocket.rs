//! WebSocket handler for real-time updates.
//!
//! Bridges the Axum HTTP layer to the core WebSocket module, providing the
//! `/ws` endpoint with full connection lifecycle, heartbeat/ping-pong,
//! subscription management, message filtering, backpressure handling,
//! JWT authentication, and reconnection support.
//!
//! ## Connection Lifecycle
//!
//! 1. Client connects to `/ws?token=<JWT>&session_id=<optional>`
//! 2. Server validates JWT and registers the connection
//! 3. Server sends `Connected` with connection ID and session ID
//! 4. If `session_id` was provided, session is restored (subs, auth, missed msgs)
//! 5. Client sends `Subscribe`/`Unsubscribe` for room-based filtering
//! 6. Server sends periodic heartbeats
//! 7. On disconnect, session is persisted for future reconnection
//!
//! ## Reconnection with Exponential Backoff (Client-Side)
//!
//! ```text
//! base_delay = 1000ms, max_delay = 30000ms, max_attempts = 5
//! for attempt in 0..max_attempts:
//!     delay = min(base_delay * 2^attempt, max_delay) + random_jitter
//!     connect to /ws?session_id=<prev>&token=<jwt>
//! ```
//!
//! ## Backpressure Handling
//!
//! Bounded outgoing channel (100 msgs). >80% full = warning. Full = disconnect.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::AppState;

use crate::websocket::{
    WebSocketConfig, WebSocketState,
    handler::{ConnectionId, ConnectionState, WebSocketConnection},
    message::{
        ClientMessage, ServerMessage,
        ErrorNotification, ErrorSeverity, ErrorSource,
    },
    room::RoomId,
    session,
};

/// Query parameters for WebSocket upgrade requests.
#[derive(Debug, Deserialize)]
pub struct WsQueryParams {
    pub token: Option<String>,
    pub session_id: Option<String>,
}

const CHANNEL_CAPACITY: usize = 100;
const BACKPRESSURE_WARN_THRESHOLD: usize = 80;

/// Handle WebSocket upgrade with authentication and session recovery.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsQueryParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, params, state))
}

/// Full lifecycle WebSocket connection handler.
async fn handle_socket(socket: WebSocket, params: WsQueryParams, _app_state: AppState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<ServerMessage>(CHANNEL_CAPACITY);

    let ws_config = WebSocketConfig::default();
    let ws_state = Arc::new(WebSocketState::new(ws_config.clone()));

    let mut connection = WebSocketConnection::new(tx.clone());
    let conn_id = connection.id;

    // --- Session recovery ---
    let mut recovered_session = None;
    if let Some(ref old_session_id) = params.session_id {
        debug!(session_id = %old_session_id, "Reconnection attempt via query parameter");
        if let Some(ref sm) = ws_state.session_manager {
            match sm.load_session(old_session_id).await {
                Ok(Some(sd)) => {
                    connection.session_id = sd.session_id.clone();
                    if let Some(ref cj) = sd.user_claims_json {
                        if let Ok(claims) = serde_json::from_str::<crate::websocket::auth::Claims>(cj) {
                            connection.state = ConnectionState::Authenticated;
                            connection.user_id = Some(claims.sub.clone());
                            connection.claims = Some(claims);
                        }
                    }
                    for rid in session::strings_to_room_ids(&sd.subscribed_rooms) {
                        connection.add_subscription(rid);
                    }
                    recovered_session = Some(sd);
                }
                Ok(None) => warn!(session_id = %old_session_id, "No stored session found"),
                Err(e) => error!(session_id = %old_session_id, error = %e, "Failed to load session"),
            }
        }
    }

    let session_id = connection.session_id.clone();

    // --- JWT authentication on connect ---
    if let Some(token) = params.token {
        if connection.state != ConnectionState::Authenticated {
            match ws_state.auth.validate_token(&token) {
                Ok(claims) => {
                    connection.state = ConnectionState::Authenticated;
                    connection.user_id = Some(claims.sub.clone());
                    connection.claims = Some(claims);
                }
                Err(e) => warn!(connection_id = %conn_id, error = %e, "Invalid token on connect"),
            }
        }
    }

    // --- Register connection ---
    if let Err(e) = ws_state.handler.register_connection(connection, None).await {
        error!(error = %e, "Failed to register connection");
        return;
    }

    // Send connected acknowledgment
    let connected_msg = ServerMessage::Connected {
        connection_id: conn_id.to_string(),
        server_time: Utc::now(),
        session_id: session_id.clone(),
    };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        if ws_sender.send(Message::Text(json)).await.is_err() {
            ws_state.handler.unregister_connection(conn_id).await;
            return;
        }
    }

    // --- Restore session subscriptions and replay missed messages ---
    if let Some(ref sd) = recovered_session {
        let room_ids = session::strings_to_room_ids(&sd.subscribed_rooms);
        {
            let mut rm = ws_state.room_manager.write().await;
            for r in &room_ids {
                rm.join_room(conn_id, r.clone());
            }
        }
        for r in &room_ids {
            let _ = ws_state.handler.add_subscription(conn_id, r.clone()).await;
        }
        let last_eid = sd.last_seen_event_id.unwrap_or(0);
        let mut total_missed: usize = 0;
        if let Some(ref sm) = ws_state.session_manager {
            for r in &room_ids {
                if let Ok(missed) = sm.get_missed_messages(&r.as_str(), last_eid).await {
                    if !missed.is_empty() {
                        total_missed += missed.len();
                        let _ = tx.send(ServerMessage::MissedUpdates { updates: missed }).await;
                    }
                }
            }
        }
        let _ = tx.send(ServerMessage::SessionRestored {
            session_id: session_id.clone(),
            missed_count: total_missed,
        }).await;
        info!(
            session_id = %session_id,
            rooms_restored = room_ids.len(),
            missed_messages = total_missed,
            "Session restored"
        );
    }

    // --- Main event loop with heartbeat and backpressure ---
    let mut heartbeat_timer = interval(Duration::from_secs(ws_config.heartbeat_interval_secs));

    // Forward outgoing messages to the WebSocket sender
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

    let mut last_activity = Instant::now();

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        last_activity = Instant::now();
                        ws_state.handler.record_message_received();
                        handle_client_message(&text, conn_id, &ws_state, &tx).await;
                    }
                    Some(Ok(Message::Ping(_))) => {
                        last_activity = Instant::now();
                        let _ = tx.send(ServerMessage::Pong {
                            client_timestamp: None,
                            server_timestamp: Utc::now().timestamp_millis(),
                        }).await;
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
            _ = heartbeat_timer.tick() => {
                // Backpressure detection
                let remaining = tx.capacity();
                if remaining < (CHANNEL_CAPACITY - BACKPRESSURE_WARN_THRESHOLD) {
                    warn!(
                        connection_id = %conn_id,
                        remaining_capacity = remaining,
                        "Backpressure warning: outgoing channel filling up"
                    );
                }
                if remaining == 0 {
                    warn!(connection_id = %conn_id, "Channel full, disconnecting slow client");
                    let _ = tx.try_send(ServerMessage::Closing {
                        reason: "Client too slow".to_string(),
                        code: 1008,
                    });
                    break;
                }
                // Send heartbeat
                if tx.send(ServerMessage::Heartbeat {
                    timestamp: Utc::now().timestamp_millis(),
                }).await.is_err() {
                    break;
                }
                // Connection timeout detection
                if last_activity.elapsed() > Duration::from_secs(ws_config.connection_timeout_secs) {
                    warn!(connection_id = %conn_id, "Connection timed out due to inactivity");
                    break;
                }
            }
        }
    }

    // --- Persist session for reconnection ---
    if let Some(ref sm) = ws_state.session_manager {
        if let Some(conn) = ws_state.handler.get_connection(conn_id).await {
            let ws_session = session::WebSocketSession {
                session_id: conn.session_id.clone(),
                user_id: conn.user_id.clone(),
                subscribed_rooms: session::room_ids_to_strings(&conn.subscriptions),
                last_seen_event_id: None,
                user_claims_json: conn.claims.as_ref().and_then(|c| serde_json::to_string(c).ok()),
                created_at_ms: conn.connected_at.timestamp_millis(),
                last_active_ms: Utc::now().timestamp_millis(),
            };
            let _ = sm.save_session(&ws_session).await;
        }
    }

    // --- Cleanup ---
    forward_handle.abort();
    ws_state.handler.unregister_connection(conn_id).await;
    ws_state.room_manager.write().await.remove_connection_from_all(conn_id);
    info!(connection_id = %conn_id, "WebSocket connection closed and cleaned up");
}

/// Handle client messages with full subscription and auth support.
async fn handle_client_message(
    text: &str,
    conn_id: ConnectionId,
    state: &Arc<WebSocketState>,
    tx: &mpsc::Sender<ServerMessage>,
) {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            let _ = tx.send(ServerMessage::Error(ErrorNotification {
                error_id: Uuid::new_v4().to_string(),
                code: "INVALID_MESSAGE".to_string(),
                message: format!("Invalid message format: {}", e),
                severity: ErrorSeverity::Warning,
                source: ErrorSource::Connection,
                related_id: None,
                details: None,
                timestamp: Utc::now(),
                recoverable: true,
                suggested_action: Some("Check message format".to_string()),
            })).await;
            return;
        }
    };

    match msg {
        ClientMessage::Authenticate { token } => {
            match state.auth.validate_token(&token) {
                Ok(claims) => {
                    let _ = state.handler.authenticate_connection(conn_id, claims.clone()).await;
                    let _ = tx.send(ServerMessage::Authenticated {
                        user_id: claims.sub,
                        permissions: claims.permissions,
                        expires_at: claims.exp,
                    }).await;
                }
                Err(e) => {
                    let code = match e {
                        crate::websocket::auth::AuthError::Expired => "TOKEN_EXPIRED",
                        crate::websocket::auth::AuthError::Invalid(_) => "INVALID_TOKEN",
                        crate::websocket::auth::AuthError::MissingClaims => "MISSING_CLAIMS",
                    };
                    let _ = tx.send(ServerMessage::AuthenticationFailed {
                        reason: e.to_string(),
                        code: code.to_string(),
                    }).await;
                }
            }
        }

        ClientMessage::Subscribe { target } => {
            let room_id: RoomId = (&target).into();
            {
                state.room_manager.write().await.join_room(conn_id, room_id.clone());
            }
            let _ = state.handler.add_subscription(conn_id, room_id.clone()).await;
            let _ = tx.send(ServerMessage::Subscribed {
                target,
                current_state: None,
            }).await;
        }

        ClientMessage::Unsubscribe { target } => {
            let room_id: RoomId = (&target).into();
            {
                state.room_manager.write().await.leave_room(conn_id, &room_id);
            }
            let _ = state.handler.remove_subscription(conn_id, &room_id).await;
            let _ = tx.send(ServerMessage::Unsubscribed { target }).await;
        }

        ClientMessage::Ping { timestamp } => {
            let _ = tx.send(ServerMessage::Pong {
                client_timestamp: timestamp,
                server_timestamp: Utc::now().timestamp_millis(),
            }).await;
        }

        ClientMessage::ApprovalResponse(response) => {
            let approver = state.handler.get_connection(conn_id).await
                .and_then(|c| c.user_id.clone());
            let result_msg = ServerMessage::ApprovalResult {
                request_id: response.request_id.clone(),
                approved: response.approved,
                approver,
                comment: response.comment,
            };
            state.broadcaster.broadcast_to_room(&RoomId::Approvals, result_msg).await;
        }

        ClientMessage::GetState { target } => {
            let room_id: RoomId = (&target).into();
            if let Some(ref sm) = state.session_manager {
                if let Ok(messages) = sm.get_missed_messages(&room_id.as_str(), 0).await {
                    if !messages.is_empty() {
                        let _ = tx.send(ServerMessage::MissedUpdates { updates: messages }).await;
                    } else {
                        let _ = tx.send(ServerMessage::Subscribed {
                            target,
                            current_state: None,
                        }).await;
                    }
                }
            }
        }

        ClientMessage::Reconnect { session_id, last_message_id } => {
            if let Some(ref sm) = state.session_manager {
                match sm.load_session(&session_id).await {
                    Ok(Some(sd)) => {
                        let last_id = last_message_id
                            .map(|id| id as i64)
                            .or(sd.last_seen_event_id)
                            .unwrap_or(0);
                        let room_ids = session::strings_to_room_ids(&sd.subscribed_rooms);
                        let mut all_missed = Vec::new();
                        for r in &room_ids {
                            {
                                state.room_manager.write().await.join_room(conn_id, r.clone());
                            }
                            let _ = state.handler.add_subscription(conn_id, r.clone()).await;
                            if let Ok(m) = sm.get_missed_messages(&r.as_str(), last_id).await {
                                all_missed.extend(m);
                            }
                        }
                        let _ = tx.send(ServerMessage::Reconnected {
                            session_id,
                            missed_messages: all_missed,
                        }).await;
                    }
                    Ok(None) => {
                        let _ = tx.send(ServerMessage::Error(ErrorNotification {
                            error_id: Uuid::new_v4().to_string(),
                            code: "SESSION_NOT_FOUND".to_string(),
                            message: format!("Session {} not found", session_id),
                            severity: ErrorSeverity::Warning,
                            source: ErrorSource::Connection,
                            related_id: Some(session_id),
                            details: None,
                            timestamp: Utc::now(),
                            recoverable: true,
                            suggested_action: Some("Create a new connection".to_string()),
                        })).await;
                    }
                    Err(e) => error!(error = %e, "Failed to load session for reconnect"),
                }
            }
        }

        ClientMessage::SessionRestore { session_id, last_event_id } => {
            if let Some(ref sm) = state.session_manager {
                match sm.load_session(&session_id).await {
                    Ok(Some(sd)) => {
                        let since_id = last_event_id.or(sd.last_seen_event_id).unwrap_or(0);
                        if let Some(ref cj) = sd.user_claims_json {
                            if let Ok(claims) = serde_json::from_str::<crate::websocket::auth::Claims>(cj) {
                                let _ = state.handler.authenticate_connection(conn_id, claims).await;
                            }
                        }
                        let room_ids = session::strings_to_room_ids(&sd.subscribed_rooms);
                        let mut total_missed: usize = 0;
                        for r in &room_ids {
                            {
                                state.room_manager.write().await.join_room(conn_id, r.clone());
                            }
                            let _ = state.handler.add_subscription(conn_id, r.clone()).await;
                            if let Ok(missed) = sm.get_missed_messages(&r.as_str(), since_id).await {
                                if !missed.is_empty() {
                                    total_missed += missed.len();
                                    let _ = tx.send(ServerMessage::MissedUpdates { updates: missed }).await;
                                }
                            }
                        }
                        let _ = tx.send(ServerMessage::SessionRestored {
                            session_id,
                            missed_count: total_missed,
                        }).await;
                    }
                    Ok(None) => {
                        let _ = tx.send(ServerMessage::Error(ErrorNotification {
                            error_id: Uuid::new_v4().to_string(),
                            code: "SESSION_NOT_FOUND".to_string(),
                            message: format!("Session {} not found", session_id),
                            severity: ErrorSeverity::Warning,
                            source: ErrorSource::Connection,
                            related_id: Some(session_id),
                            details: None,
                            timestamp: Utc::now(),
                            recoverable: true,
                            suggested_action: Some("Create a new connection".to_string()),
                        })).await;
                    }
                    Err(e) => error!(error = %e, "Failed to load session for restore"),
                }
            }
        }
    }
}

/// Legacy broadcast compatibility shim. Use `WebSocketState::broadcast_*` for new code.
#[allow(dead_code)]
pub async fn broadcast_update(update: ServerMessage, tx: &broadcast::Sender<String>) {
    if let Ok(json) = serde_json::to_string(&update) {
        let _ = tx.send(json);
    }
}
