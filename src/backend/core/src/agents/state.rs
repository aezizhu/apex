//! Conversation state persistence for multi-turn agent interactions.
//!
//! Provides LangGraph-style state management with checkpoint/rollback support,
//! enabling agents to maintain context across conversation turns.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::agents::AgentId;
use crate::error::{ApexError, Result};

// ═══════════════════════════════════════════════════════════════════════════════
// Core Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Role within a conversation message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// Unique message identifier.
    pub id: Uuid,
    /// Role of the message sender.
    pub role: MessageRole,
    /// Message content.
    pub content: String,
    /// Optional tool call ID (for tool messages).
    pub tool_call_id: Option<String>,
    /// Optional structured data attached to the message.
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp when this message was created.
    pub created_at: DateTime<Utc>,
}

impl ConversationMessage {
    /// Create a new message.
    pub fn new(role: MessageRole, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role,
            content: content.into(),
            tool_call_id: None,
            metadata: HashMap::new(),
            created_at: Utc::now(),
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageRole::System, content)
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    /// Create a tool response message.
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        let mut msg = Self::new(MessageRole::Tool, content);
        msg.tool_call_id = Some(tool_call_id.into());
        msg
    }

    /// Attach metadata to this message.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// The persistent state of a conversation between a user and an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationState {
    /// Unique conversation identifier.
    pub id: Uuid,
    /// Agent that owns this conversation.
    pub agent_id: AgentId,
    /// Ordered list of messages in the conversation.
    pub messages: Vec<ConversationMessage>,
    /// Arbitrary context values persisted across turns (LangGraph-style state).
    pub context: HashMap<String, serde_json::Value>,
    /// Additional metadata (tags, source, etc.).
    pub metadata: HashMap<String, serde_json::Value>,
    /// Current checkpoint version (incremented on each save).
    pub version: u64,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl ConversationState {
    /// Create a new conversation state for the given agent.
    pub fn new(agent_id: AgentId) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            agent_id,
            messages: Vec::new(),
            context: HashMap::new(),
            metadata: HashMap::new(),
            version: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a message to the conversation.
    pub fn add_message(&mut self, message: ConversationMessage) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Set a context value (LangGraph-style state channel).
    pub fn set_context(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.context.insert(key.into(), value);
        self.updated_at = Utc::now();
    }

    /// Get a context value.
    pub fn get_context(&self, key: &str) -> Option<&serde_json::Value> {
        self.context.get(key)
    }

    /// Remove a context value.
    pub fn remove_context(&mut self, key: &str) -> Option<serde_json::Value> {
        let val = self.context.remove(key);
        if val.is_some() {
            self.updated_at = Utc::now();
        }
        val
    }

    /// Get the total number of messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Get messages filtered by role.
    pub fn messages_by_role(&self, role: &MessageRole) -> Vec<&ConversationMessage> {
        self.messages.iter().filter(|m| &m.role == role).collect()
    }

    /// Get the last N messages.
    pub fn last_messages(&self, n: usize) -> &[ConversationMessage] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }

    /// Create a checkpoint snapshot of the current state.
    pub fn checkpoint(&self) -> StateCheckpoint {
        StateCheckpoint {
            id: Uuid::new_v4(),
            conversation_id: self.id,
            version: self.version,
            state_snapshot: serde_json::to_value(self).unwrap_or_default(),
            created_at: Utc::now(),
        }
    }

    /// Restore state from a checkpoint.
    pub fn restore_from_checkpoint(checkpoint: &StateCheckpoint) -> Result<Self> {
        serde_json::from_value(checkpoint.state_snapshot.clone())
            .map_err(|e| ApexError::internal(format!("Failed to restore checkpoint: {}", e)))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// State Checkpoint
// ═══════════════════════════════════════════════════════════════════════════════

/// A checkpoint snapshot of conversation state for rollback support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCheckpoint {
    /// Unique checkpoint identifier.
    pub id: Uuid,
    /// Conversation this checkpoint belongs to.
    pub conversation_id: Uuid,
    /// Version of the state at checkpoint time.
    pub version: u64,
    /// Full serialized state snapshot.
    pub state_snapshot: serde_json::Value,
    /// Timestamp when checkpoint was created.
    pub created_at: DateTime<Utc>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// StateStore Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for persisting and retrieving conversation state.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Save conversation state. Increments the version automatically.
    async fn save(&self, state: &mut ConversationState) -> Result<()>;

    /// Load conversation state by ID.
    async fn load(&self, conversation_id: Uuid) -> Result<Option<ConversationState>>;

    /// Delete conversation state by ID.
    async fn delete(&self, conversation_id: Uuid) -> Result<bool>;

    /// List all conversation states for a given agent.
    async fn list_by_agent(&self, agent_id: AgentId) -> Result<Vec<ConversationState>>;

    /// Save a checkpoint for rollback support.
    async fn save_checkpoint(&self, checkpoint: &StateCheckpoint) -> Result<()>;

    /// Load checkpoints for a conversation, ordered by version descending.
    async fn load_checkpoints(&self, conversation_id: Uuid) -> Result<Vec<StateCheckpoint>>;

    /// Load a specific checkpoint by ID.
    async fn load_checkpoint(&self, checkpoint_id: Uuid) -> Result<Option<StateCheckpoint>>;
}

// ═══════════════════════════════════════════════════════════════════════════════
// InMemoryStateStore
// ═══════════════════════════════════════════════════════════════════════════════

/// In-memory implementation of StateStore for testing and development.
pub struct InMemoryStateStore {
    states: DashMap<Uuid, ConversationState>,
    checkpoints: DashMap<Uuid, Vec<StateCheckpoint>>,
}

impl InMemoryStateStore {
    /// Create a new in-memory state store.
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
            checkpoints: DashMap::new(),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn save(&self, state: &mut ConversationState) -> Result<()> {
        state.version += 1;
        state.updated_at = Utc::now();
        self.states.insert(state.id, state.clone());
        Ok(())
    }

    async fn load(&self, conversation_id: Uuid) -> Result<Option<ConversationState>> {
        Ok(self.states.get(&conversation_id).map(|s| s.value().clone()))
    }

    async fn delete(&self, conversation_id: Uuid) -> Result<bool> {
        let removed = self.states.remove(&conversation_id).is_some();
        self.checkpoints.remove(&conversation_id);
        Ok(removed)
    }

    async fn list_by_agent(&self, agent_id: AgentId) -> Result<Vec<ConversationState>> {
        let results: Vec<ConversationState> = self
            .states
            .iter()
            .filter(|entry| entry.value().agent_id == agent_id)
            .map(|entry| entry.value().clone())
            .collect();
        Ok(results)
    }

    async fn save_checkpoint(&self, checkpoint: &StateCheckpoint) -> Result<()> {
        self.checkpoints
            .entry(checkpoint.conversation_id)
            .or_default()
            .push(checkpoint.clone());
        Ok(())
    }

    async fn load_checkpoints(&self, conversation_id: Uuid) -> Result<Vec<StateCheckpoint>> {
        Ok(self
            .checkpoints
            .get(&conversation_id)
            .map(|v| {
                let mut cps = v.value().clone();
                cps.sort_by(|a, b| b.version.cmp(&a.version));
                cps
            })
            .unwrap_or_default())
    }

    async fn load_checkpoint(&self, checkpoint_id: Uuid) -> Result<Option<StateCheckpoint>> {
        for entry in self.checkpoints.iter() {
            if let Some(cp) = entry.value().iter().find(|c| c.id == checkpoint_id) {
                return Ok(Some(cp.clone()));
            }
        }
        Ok(None)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PostgresStateStore
// ═══════════════════════════════════════════════════════════════════════════════

/// PostgreSQL-backed implementation of StateStore using sqlx.
pub struct PostgresStateStore {
    pool: PgPool,
}

impl PostgresStateStore {
    /// Create a new Postgres state store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StateStore for PostgresStateStore {
    async fn save(&self, state: &mut ConversationState) -> Result<()> {
        state.version += 1;
        state.updated_at = Utc::now();

        let messages_json = serde_json::to_value(&state.messages)
            .map_err(|e| ApexError::internal(format!("Failed to serialize messages: {}", e)))?;
        let context_json = serde_json::to_value(&state.context)
            .map_err(|e| ApexError::internal(format!("Failed to serialize context: {}", e)))?;
        let metadata_json = serde_json::to_value(&state.metadata)
            .map_err(|e| ApexError::internal(format!("Failed to serialize metadata: {}", e)))?;

        sqlx::query(
            r#"
            INSERT INTO conversation_states (id, agent_id, messages, context, metadata, version, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                messages = EXCLUDED.messages,
                context = EXCLUDED.context,
                metadata = EXCLUDED.metadata,
                version = EXCLUDED.version,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(state.id)
        .bind(state.agent_id.0)
        .bind(&messages_json)
        .bind(&context_json)
        .bind(&metadata_json)
        .bind(state.version as i64)
        .bind(state.created_at)
        .bind(state.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load(&self, conversation_id: Uuid) -> Result<Option<ConversationState>> {
        let row = sqlx::query_as::<_, ConversationStateRow>(
            r#"
            SELECT id, agent_id, messages, context, metadata, version, created_at, updated_at
            FROM conversation_states
            WHERE id = $1
            "#,
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.into_state()?)),
            None => Ok(None),
        }
    }

    async fn delete(&self, conversation_id: Uuid) -> Result<bool> {
        // Delete checkpoints first (foreign key)
        sqlx::query("DELETE FROM state_checkpoints WHERE conversation_id = $1")
            .bind(conversation_id)
            .execute(&self.pool)
            .await?;

        let result = sqlx::query("DELETE FROM conversation_states WHERE id = $1")
            .bind(conversation_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn list_by_agent(&self, agent_id: AgentId) -> Result<Vec<ConversationState>> {
        let rows = sqlx::query_as::<_, ConversationStateRow>(
            r#"
            SELECT id, agent_id, messages, context, metadata, version, created_at, updated_at
            FROM conversation_states
            WHERE agent_id = $1
            ORDER BY updated_at DESC
            "#,
        )
        .bind(agent_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.into_state()).collect()
    }

    async fn save_checkpoint(&self, checkpoint: &StateCheckpoint) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO state_checkpoints (id, conversation_id, version, state_snapshot, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(checkpoint.id)
        .bind(checkpoint.conversation_id)
        .bind(checkpoint.version as i64)
        .bind(&checkpoint.state_snapshot)
        .bind(checkpoint.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load_checkpoints(&self, conversation_id: Uuid) -> Result<Vec<StateCheckpoint>> {
        let rows = sqlx::query_as::<_, CheckpointRow>(
            r#"
            SELECT id, conversation_id, version, state_snapshot, created_at
            FROM state_checkpoints
            WHERE conversation_id = $1
            ORDER BY version DESC
            "#,
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_checkpoint()).collect())
    }

    async fn load_checkpoint(&self, checkpoint_id: Uuid) -> Result<Option<StateCheckpoint>> {
        let row = sqlx::query_as::<_, CheckpointRow>(
            r#"
            SELECT id, conversation_id, version, state_snapshot, created_at
            FROM state_checkpoints
            WHERE id = $1
            "#,
        )
        .bind(checkpoint_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into_checkpoint()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SQLx Row Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(sqlx::FromRow)]
struct ConversationStateRow {
    id: Uuid,
    agent_id: Uuid,
    messages: serde_json::Value,
    context: serde_json::Value,
    metadata: serde_json::Value,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl ConversationStateRow {
    fn into_state(self) -> Result<ConversationState> {
        let messages: Vec<ConversationMessage> = serde_json::from_value(self.messages)
            .map_err(|e| ApexError::internal(format!("Failed to deserialize messages: {}", e)))?;
        let context: HashMap<String, serde_json::Value> =
            serde_json::from_value(self.context).map_err(|e| {
                ApexError::internal(format!("Failed to deserialize context: {}", e))
            })?;
        let metadata: HashMap<String, serde_json::Value> =
            serde_json::from_value(self.metadata).map_err(|e| {
                ApexError::internal(format!("Failed to deserialize metadata: {}", e))
            })?;

        Ok(ConversationState {
            id: self.id,
            agent_id: AgentId(self.agent_id),
            messages,
            context,
            metadata,
            version: self.version as u64,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct CheckpointRow {
    id: Uuid,
    conversation_id: Uuid,
    version: i64,
    state_snapshot: serde_json::Value,
    created_at: DateTime<Utc>,
}

impl CheckpointRow {
    fn into_checkpoint(self) -> StateCheckpoint {
        StateCheckpoint {
            id: self.id,
            conversation_id: self.conversation_id,
            version: self.version as u64,
            state_snapshot: self.state_snapshot,
            created_at: self.created_at,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Thread-Safe StateStore Wrapper
// ═══════════════════════════════════════════════════════════════════════════════

/// A thread-safe, cloneable handle to any StateStore implementation.
#[derive(Clone)]
pub struct StateStoreHandle {
    inner: Arc<dyn StateStore>,
}

impl StateStoreHandle {
    /// Wrap any StateStore implementation in a thread-safe handle.
    pub fn new(store: impl StateStore + 'static) -> Self {
        Self {
            inner: Arc::new(store),
        }
    }

    /// Save conversation state.
    pub async fn save(&self, state: &mut ConversationState) -> Result<()> {
        self.inner.save(state).await
    }

    /// Load conversation state.
    pub async fn load(&self, conversation_id: Uuid) -> Result<Option<ConversationState>> {
        self.inner.load(conversation_id).await
    }

    /// Delete conversation state.
    pub async fn delete(&self, conversation_id: Uuid) -> Result<bool> {
        self.inner.delete(conversation_id).await
    }

    /// List states by agent.
    pub async fn list_by_agent(&self, agent_id: AgentId) -> Result<Vec<ConversationState>> {
        self.inner.list_by_agent(agent_id).await
    }

    /// Save a checkpoint.
    pub async fn save_checkpoint(&self, checkpoint: &StateCheckpoint) -> Result<()> {
        self.inner.save_checkpoint(checkpoint).await
    }

    /// Load checkpoints for a conversation.
    pub async fn load_checkpoints(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<StateCheckpoint>> {
        self.inner.load_checkpoints(conversation_id).await
    }

    /// Load a specific checkpoint.
    pub async fn load_checkpoint(
        &self,
        checkpoint_id: Uuid,
    ) -> Result<Option<StateCheckpoint>> {
        self.inner.load_checkpoint(checkpoint_id).await
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_conversation_state_lifecycle() {
        let store = InMemoryStateStore::new();
        let agent_id = AgentId::new();

        // Create state
        let mut state = ConversationState::new(agent_id);
        state.add_message(ConversationMessage::system("You are a helpful assistant."));
        state.add_message(ConversationMessage::user("Hello!"));
        state.set_context("turn_count", serde_json::json!(1));

        let conv_id = state.id;

        // Save
        store.save(&mut state).await.unwrap();
        assert_eq!(state.version, 1);

        // Load
        let loaded = store.load(conv_id).await.unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.get_context("turn_count"), Some(&serde_json::json!(1)));

        // Update
        state.add_message(ConversationMessage::assistant("Hi there!"));
        state.set_context("turn_count", serde_json::json!(2));
        store.save(&mut state).await.unwrap();
        assert_eq!(state.version, 2);

        // List by agent
        let agent_states = store.list_by_agent(agent_id).await.unwrap();
        assert_eq!(agent_states.len(), 1);

        // Delete
        let deleted = store.delete(conv_id).await.unwrap();
        assert!(deleted);

        let gone = store.load(conv_id).await.unwrap();
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn test_checkpoint_rollback() {
        let store = InMemoryStateStore::new();
        let agent_id = AgentId::new();

        let mut state = ConversationState::new(agent_id);
        state.add_message(ConversationMessage::user("First message"));
        store.save(&mut state).await.unwrap();

        // Checkpoint after first message
        let cp = state.checkpoint();
        store.save_checkpoint(&cp).await.unwrap();

        // Add more messages
        state.add_message(ConversationMessage::assistant("Response"));
        state.add_message(ConversationMessage::user("Second message"));
        store.save(&mut state).await.unwrap();
        assert_eq!(state.messages.len(), 3);

        // Rollback to checkpoint
        let checkpoints = store.load_checkpoints(state.id).await.unwrap();
        assert_eq!(checkpoints.len(), 1);

        let restored = ConversationState::restore_from_checkpoint(&checkpoints[0]).unwrap();
        assert_eq!(restored.messages.len(), 1);
        assert_eq!(restored.messages[0].content, "First message");
    }

    #[test]
    fn test_message_helpers() {
        let sys = ConversationMessage::system("System prompt");
        assert_eq!(sys.role, MessageRole::System);

        let user = ConversationMessage::user("Hello");
        assert_eq!(user.role, MessageRole::User);

        let asst = ConversationMessage::assistant("Hi");
        assert_eq!(asst.role, MessageRole::Assistant);

        let tool = ConversationMessage::tool("result", "call_123");
        assert_eq!(tool.role, MessageRole::Tool);
        assert_eq!(tool.tool_call_id.unwrap(), "call_123");
    }

    #[test]
    fn test_last_messages() {
        let agent_id = AgentId::new();
        let mut state = ConversationState::new(agent_id);

        for i in 0..10 {
            state.add_message(ConversationMessage::user(format!("msg {}", i)));
        }

        let last3 = state.last_messages(3);
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].content, "msg 7");
        assert_eq!(last3[2].content, "msg 9");
    }

    #[test]
    fn test_messages_by_role() {
        let agent_id = AgentId::new();
        let mut state = ConversationState::new(agent_id);
        state.add_message(ConversationMessage::user("q1"));
        state.add_message(ConversationMessage::assistant("a1"));
        state.add_message(ConversationMessage::user("q2"));

        let user_msgs = state.messages_by_role(&MessageRole::User);
        assert_eq!(user_msgs.len(), 2);
    }
}
