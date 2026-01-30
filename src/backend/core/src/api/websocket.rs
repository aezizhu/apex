//! WebSocket handler for real-time updates.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::AppState;

/// WebSocket message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// Subscribe to updates for specific resources
    Subscribe {
        resource: String,
        id: Option<String>,
    },
    /// Unsubscribe from updates
    Unsubscribe {
        resource: String,
        id: Option<String>,
    },
    /// Ping to keep connection alive
    Ping,
    /// Pong response
    Pong,
}

/// Server-sent update messages.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum WsUpdate {
    /// Agent status update
    AgentUpdate {
        id: String,
        status: String,
        current_load: u32,
        success_rate: f64,
    },
    /// Task status update
    TaskUpdate {
        id: String,
        status: String,
        tokens_used: u64,
        cost_dollars: f64,
    },
    /// DAG status update
    DagUpdate {
        id: String,
        status: String,
        tasks_completed: usize,
        tasks_total: usize,
    },
    /// System metrics update
    MetricsUpdate {
        active_agents: u64,
        queue_depth: u64,
        total_tokens: u64,
        total_cost: f64,
    },
    /// Error message
    Error {
        message: String,
    },
}

/// Handle WebSocket upgrade.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// A subscription entry keyed by resource name and optional ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubscriptionKey {
    resource: String,
    id: Option<String>,
}

/// Handle individual WebSocket connection.
async fn handle_socket(socket: WebSocket, _state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Track active subscriptions for this connection
    let mut subscriptions = std::collections::HashSet::<SubscriptionKey>::new();

    // Send initial connection acknowledgment
    let ack = serde_json::json!({
        "type": "connected",
        "message": "Connected to Apex real-time updates"
    });

    if sender.send(Message::Text(ack.to_string())).await.is_err() {
        return;
    }

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<WsMessage>(&text) {
                    Ok(ws_msg) => {
                        match ws_msg {
                            WsMessage::Ping => {
                                let pong = serde_json::json!({"type": "pong"});
                                if sender.send(Message::Text(pong.to_string())).await.is_err() {
                                    break;
                                }
                            }
                            WsMessage::Subscribe { resource, id } => {
                                tracing::info!(resource = %resource, id = ?id, count = subscriptions.len(), "Client subscribed");
                                let key = SubscriptionKey { resource: resource.clone(), id: id.clone() };
                                subscriptions.insert(key);
                                let ack_msg = serde_json::json!({"type": "subscribed", "resource": resource, "id": id});
                                if sender.send(Message::Text(ack_msg.to_string())).await.is_err() { break; }
                            }
                            WsMessage::Unsubscribe { resource, id } => {
                                tracing::info!(resource = %resource, id = ?id, count = subscriptions.len(), "Client unsubscribed");
                                let key = SubscriptionKey { resource: resource.clone(), id: id.clone() };
                                subscriptions.remove(&key);
                                let ack_msg = serde_json::json!({"type": "unsubscribed", "resource": resource, "id": id});
                                if sender.send(Message::Text(ack_msg.to_string())).await.is_err() { break; }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        let error = WsUpdate::Error {
                            message: format!("Invalid message format: {}", e),
                        };
                        if sender.send(Message::Text(serde_json::to_string(&error).unwrap())).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                if sender.send(Message::Pong(data)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                tracing::error!(error = %e, "WebSocket error");
                break;
            }
            _ => {}
        }
    }

    tracing::info!(subscriptions = subscriptions.len(), "WebSocket connection closed");
}

/// Broadcast an update to all connected clients.
#[allow(dead_code)]
pub async fn broadcast_update(update: WsUpdate, tx: &broadcast::Sender<String>) {
    if let Ok(json) = serde_json::to_string(&update) {
        let _ = tx.send(json);
    }
}
