//! WebSocket session management with Redis-backed persistence.
//!
//! Provides session storage, recovery, and message replay for reconnecting clients.
//! Sessions are stored in Redis with a 1-hour TTL and include subscribed rooms,
//! last seen event ID, and user context.

use crate::error::{ApexError, ErrorCode};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use super::message::ServerMessage;
use super::room::RoomId;

/// Session expiration time in seconds (1 hour).
const SESSION_TTL_SECS: u64 = 3600;

/// Maximum number of recent messages stored per room.
const MAX_MESSAGES_PER_ROOM: isize = 1000;

/// A serializable representation of a WebSocket session for Redis storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketSession {
    /// Unique session identifier
    pub session_id: String,
    /// Authenticated user ID (if any)
    pub user_id: Option<String>,
    /// Rooms the session was subscribed to
    pub subscribed_rooms: Vec<String>,
    /// The last event ID the client acknowledged seeing
    pub last_seen_event_id: Option<i64>,
    /// Serialized user claims (JWT payload) for restoring auth state
    pub user_claims_json: Option<String>,
    /// Timestamp when the session was created (epoch millis)
    pub created_at_ms: i64,
    /// Timestamp when the session was last active (epoch millis)
    pub last_active_ms: i64,
}

/// Manages WebSocket session persistence in Redis.
///
/// Supports saving, loading, and deleting sessions, as well as storing
/// and retrieving per-room message buffers for replay on reconnection.
pub struct SessionManager {
    redis: redis::Client,
}

impl SessionManager {
    /// Create a new session manager backed by the given Redis client.
    pub fn new(redis: redis::Client) -> Self {
        Self { redis }
    }

    /// Get a multiplexed async connection from the Redis client.
    async fn get_conn(
        &self,
    ) -> Result<redis::aio::MultiplexedConnection, ApexError> {
        self.redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to get Redis connection for session manager");
                ApexError::with_internal(
                    ErrorCode::CacheConnectionFailed,
                    "Failed to connect to session store",
                    e.to_string(),
                )
            })
    }

    /// Redis key for a session.
    fn session_key(session_id: &str) -> String {
        format!("apex:ws:session:{}", session_id)
    }

    /// Redis key for a room's message buffer.
    fn room_messages_key(room_id: &str) -> String {
        format!("apex:ws:room:{}:messages", room_id)
    }

    /// Save a session to Redis with a 1-hour TTL.
    pub async fn save_session(&self, session: &WebSocketSession) -> Result<(), ApexError> {
        let mut conn = self.get_conn().await?;
        let key = Self::session_key(&session.session_id);

        let json = serde_json::to_string(session).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::SerializationError,
                "Failed to serialize session",
                e.to_string(),
            )
        })?;

        conn.set_ex::<_, _, ()>(&key, &json, SESSION_TTL_SECS)
            .await
            .map_err(|e| {
                error!(session_id = %session.session_id, error = %e, "Failed to save session to Redis");
                ApexError::from(e)
            })?;

        debug!(session_id = %session.session_id, "Session saved to Redis");
        Ok(())
    }

    /// Load a session from Redis by session ID.
    ///
    /// Returns `None` if the session does not exist or has expired.
    pub async fn load_session(
        &self,
        session_id: &str,
    ) -> Result<Option<WebSocketSession>, ApexError> {
        let mut conn = self.get_conn().await?;
        let key = Self::session_key(session_id);

        let result: Option<String> = conn.get(&key).await.map_err(|e| {
            error!(session_id = %session_id, error = %e, "Failed to load session from Redis");
            ApexError::from(e)
        })?;

        match result {
            Some(json) => {
                let session: WebSocketSession =
                    serde_json::from_str(&json).map_err(|e| {
                        warn!(session_id = %session_id, error = %e, "Failed to deserialize session");
                        ApexError::with_internal(
                            ErrorCode::DeserializationError,
                            "Failed to deserialize session",
                            e.to_string(),
                        )
                    })?;
                debug!(session_id = %session_id, "Session loaded from Redis");
                Ok(Some(session))
            }
            None => {
                debug!(session_id = %session_id, "No session found in Redis");
                Ok(None)
            }
        }
    }

    /// Delete a session from Redis.
    pub async fn delete_session(&self, session_id: &str) -> Result<(), ApexError> {
        let mut conn = self.get_conn().await?;
        let key = Self::session_key(session_id);

        conn.del::<_, ()>(&key).await.map_err(|e| {
            error!(session_id = %session_id, error = %e, "Failed to delete session from Redis");
            ApexError::from(e)
        })?;

        info!(session_id = %session_id, "Session deleted from Redis");
        Ok(())
    }

    /// Store a message in the room's bounded message buffer.
    ///
    /// Uses LPUSH + LTRIM to maintain a rolling window of the last 1000 messages.
    pub async fn store_message(
        &self,
        room_id: &str,
        message: &ServerMessage,
    ) -> Result<(), ApexError> {
        let mut conn = self.get_conn().await?;
        let key = Self::room_messages_key(room_id);

        let json = serde_json::to_string(message).map_err(|e| {
            ApexError::with_internal(
                ErrorCode::SerializationError,
                "Failed to serialize message for room buffer",
                e.to_string(),
            )
        })?;

        // LPUSH the message, then LTRIM to keep only the latest MAX_MESSAGES_PER_ROOM
        redis::pipe()
            .lpush(&key, &json)
            .ltrim(&key, 0, MAX_MESSAGES_PER_ROOM - 1)
            // Set a TTL on the list so stale rooms don't accumulate forever
            .expire(&key, SESSION_TTL_SECS as i64)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| {
                warn!(room_id = %room_id, error = %e, "Failed to store message in room buffer");
                ApexError::from(e)
            })?;

        Ok(())
    }

    /// Retrieve messages missed by a client since a given event ID.
    ///
    /// Messages are stored most-recent-first in Redis (LPUSH). This method
    /// fetches all buffered messages and filters to those with a sequence/ID
    /// greater than `since_id`. The returned list is in chronological order
    /// (oldest first) for proper replay.
    pub async fn get_missed_messages(
        &self,
        room_id: &str,
        since_id: i64,
    ) -> Result<Vec<ServerMessage>, ApexError> {
        let mut conn = self.get_conn().await?;
        let key = Self::room_messages_key(room_id);

        // Fetch all buffered messages (newest first)
        let raw: Vec<String> = conn
            .lrange(&key, 0, MAX_MESSAGES_PER_ROOM - 1)
            .await
            .map_err(|e| {
                error!(room_id = %room_id, error = %e, "Failed to fetch room messages from Redis");
                ApexError::from(e)
            })?;

        let mut messages: Vec<ServerMessage> = Vec::new();

        let total = raw.len() as i64;
        for (idx, json) in raw.iter().enumerate() {
            match serde_json::from_str::<ServerMessage>(json) {
                Ok(msg) => {
                    // Use list index as a proxy event ID: index 0 is newest,
                    // so the effective event_id is (total - index).
                    // We include messages whose effective ID > since_id.
                    let effective_id = total - (idx as i64);
                    if effective_id > since_id {
                        messages.push(msg);
                    }
                }
                Err(e) => {
                    warn!(room_id = %room_id, error = %e, "Skipping malformed message in room buffer");
                }
            }
        }

        // Reverse so the client receives messages in chronological order (oldest first)
        messages.reverse();

        debug!(
            room_id = %room_id,
            since_id = since_id,
            missed_count = messages.len(),
            "Retrieved missed messages for replay"
        );

        Ok(messages)
    }

    /// Update the last_seen_event_id for an existing session without replacing the whole object.
    pub async fn update_last_seen_event(
        &self,
        session_id: &str,
        event_id: i64,
    ) -> Result<(), ApexError> {
        // Load, update, save
        if let Some(mut session) = self.load_session(session_id).await? {
            session.last_seen_event_id = Some(event_id);
            session.last_active_ms = chrono::Utc::now().timestamp_millis();
            self.save_session(&session).await?;
            debug!(session_id = %session_id, event_id = event_id, "Updated last_seen_event_id");
        }
        Ok(())
    }
}

/// Convert a list of `RoomId` values to their string representations for serialization.
pub fn room_ids_to_strings(rooms: &[RoomId]) -> Vec<String> {
    rooms.iter().map(|r| r.as_str()).collect()
}

/// Parse room ID strings back into `RoomId` values.
///
/// Unrecognized formats are stored as `RoomId::Custom`.
pub fn strings_to_room_ids(strings: &[String]) -> Vec<RoomId> {
    strings
        .iter()
        .map(|s| {
            if let Some(id) = s.strip_prefix("task:") {
                RoomId::Task(id.to_string())
            } else if s == "tasks:all" {
                RoomId::AllTasks
            } else if let Some(id) = s.strip_prefix("agent:") {
                RoomId::Agent(id.to_string())
            } else if s == "agents:all" {
                RoomId::AllAgents
            } else if let Some(id) = s.strip_prefix("dag:") {
                RoomId::Dag(id.to_string())
            } else if s == "dags:all" {
                RoomId::AllDags
            } else if s == "metrics" {
                RoomId::Metrics
            } else if s == "approvals" {
                RoomId::Approvals
            } else if s == "errors" {
                RoomId::Errors
            } else if let Some(name) = s.strip_prefix("custom:") {
                RoomId::Custom(name.to_string())
            } else {
                RoomId::Custom(s.clone())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_id_round_trip() {
        let rooms = vec![
            RoomId::Task("task-1".to_string()),
            RoomId::AllTasks,
            RoomId::Agent("agent-1".to_string()),
            RoomId::AllAgents,
            RoomId::Dag("dag-1".to_string()),
            RoomId::AllDags,
            RoomId::Metrics,
            RoomId::Approvals,
            RoomId::Errors,
            RoomId::Custom("my-room".to_string()),
        ];

        let strings = room_ids_to_strings(&rooms);
        let restored = strings_to_room_ids(&strings);

        assert_eq!(rooms, restored);
    }

    #[test]
    fn test_websocket_session_serialization() {
        let session = WebSocketSession {
            session_id: "test-session".to_string(),
            user_id: Some("user-123".to_string()),
            subscribed_rooms: vec!["task:abc".to_string(), "metrics".to_string()],
            last_seen_event_id: Some(42),
            user_claims_json: None,
            created_at_ms: 1700000000000,
            last_active_ms: 1700000001000,
        };

        let json = serde_json::to_string(&session).unwrap();
        let restored: WebSocketSession = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.session_id, "test-session");
        assert_eq!(restored.user_id, Some("user-123".to_string()));
        assert_eq!(restored.last_seen_event_id, Some(42));
        assert_eq!(restored.subscribed_rooms.len(), 2);
    }
}
