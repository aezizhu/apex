//! Inter-agent communication primitives.
//!
//! Provides a `MessageBus` for pub/sub messaging between agents and
//! `ConversationThread` for multi-turn agent discussions. Together with
//! the delegation module, these primitives enable rich agent collaboration
//! patterns that set Apex apart from single-agent frameworks.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use super::AgentId;

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// Unique identifier for a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Kind of message being sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    /// Free-form text (chat-like).
    Text,
    /// Structured data payload.
    Data,
    /// A request for information or action.
    Request,
    /// A response to a prior request.
    Response,
    /// A status or progress update.
    StatusUpdate,
    /// An error report.
    Error,
    /// Agent is signalling it is done with the conversation.
    Complete,
}

/// A message exchanged between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique message identifier.
    pub id: MessageId,

    /// The sending agent.
    pub from: AgentId,

    /// The intended recipient (None = broadcast to topic).
    pub to: Option<AgentId>,

    /// The topic this message is published under.
    pub topic: String,

    /// The kind of message.
    pub kind: MessageKind,

    /// Human-readable content / summary.
    pub content: String,

    /// Structured payload.
    #[serde(default)]
    pub payload: serde_json::Value,

    /// Optional correlation ID to link request/response pairs.
    pub correlation_id: Option<MessageId>,

    /// Thread this message belongs to.
    pub thread_id: Option<Uuid>,

    /// Timestamp.
    pub created_at: DateTime<Utc>,

    /// Arbitrary metadata.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl AgentMessage {
    /// Create a new message.
    pub fn new(
        from: AgentId,
        topic: impl Into<String>,
        kind: MessageKind,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: MessageId::new(),
            from,
            to: None,
            topic: topic.into(),
            kind,
            content: content.into(),
            payload: serde_json::Value::Null,
            correlation_id: None,
            thread_id: None,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Builder: set a direct recipient.
    pub fn with_recipient(mut self, to: AgentId) -> Self {
        self.to = Some(to);
        self
    }

    /// Builder: set payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Builder: set correlation ID.
    pub fn with_correlation(mut self, id: MessageId) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Builder: set thread ID.
    pub fn with_thread(mut self, thread_id: Uuid) -> Self {
        self.thread_id = Some(thread_id);
        self
    }

    /// Builder: add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Create a reply to this message.
    pub fn reply(
        &self,
        from: AgentId,
        kind: MessageKind,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: MessageId::new(),
            from,
            to: Some(self.from),
            topic: self.topic.clone(),
            kind,
            content: content.into(),
            payload: serde_json::Value::Null,
            correlation_id: Some(self.id),
            thread_id: self.thread_id,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// MessageBus
// ---------------------------------------------------------------------------

/// Channel capacity for topic subscribers.
const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// A pub/sub message bus for inter-agent communication.
///
/// Agents publish `AgentMessage`s to named topics. Other agents subscribe
/// to topics of interest and receive cloned messages via `broadcast` channels.
pub struct MessageBus {
    /// Topic -> broadcast sender.
    topics: DashMap<String, broadcast::Sender<AgentMessage>>,

    /// Channel capacity for new topics.
    channel_capacity: usize,

    /// Total messages published (all topics).
    total_published: std::sync::atomic::AtomicU64,
}

impl MessageBus {
    /// Create a new `MessageBus` with the default channel capacity.
    pub fn new() -> Self {
        Self {
            topics: DashMap::new(),
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            total_published: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Create a `MessageBus` with a custom channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            topics: DashMap::new(),
            channel_capacity: capacity,
            total_published: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Publish a message to its topic. Creates the topic if it doesn't exist.
    ///
    /// Returns the number of receivers that received the message.
    pub fn publish(&self, message: AgentMessage) -> usize {
        let topic = message.topic.clone();
        let sender = self
            .topics
            .entry(topic)
            .or_insert_with(|| broadcast::channel(self.channel_capacity).0);

        // send returns Err if there are no receivers, which is fine.
        let receivers = sender.send(message).unwrap_or(0);
        self.total_published
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        receivers
    }

    /// Subscribe to a topic. Returns a receiver for incoming messages.
    ///
    /// Creates the topic if it doesn't exist yet.
    pub fn subscribe(&self, topic: impl Into<String>) -> broadcast::Receiver<AgentMessage> {
        let topic = topic.into();
        let sender = self
            .topics
            .entry(topic)
            .or_insert_with(|| broadcast::channel(self.channel_capacity).0);

        sender.subscribe()
    }

    /// Check if a topic exists and has been created.
    pub fn has_topic(&self, topic: &str) -> bool {
        self.topics.contains_key(topic)
    }

    /// Return the number of active topics.
    pub fn topic_count(&self) -> usize {
        self.topics.len()
    }

    /// Total messages published across all topics.
    pub fn total_published(&self) -> u64 {
        self.total_published
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// List all active topic names.
    pub fn topics(&self) -> Vec<String> {
        self.topics.iter().map(|e| e.key().clone()).collect()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ConversationThread
// ---------------------------------------------------------------------------

/// A multi-turn conversation between agents.
///
/// Threads collect messages in order and track which agents are participating.
/// This enables structured multi-agent discussions (e.g. a planner coordinating
/// with a coder and a reviewer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationThread {
    /// Unique thread identifier.
    pub id: Uuid,

    /// Human-readable title / topic.
    pub title: String,

    /// Agents participating in this thread.
    pub participants: Vec<AgentId>,

    /// Ordered messages in the thread.
    pub messages: Vec<AgentMessage>,

    /// Thread status.
    pub status: ThreadStatus,

    /// When the thread was created.
    pub created_at: DateTime<Utc>,

    /// When the last message was added.
    pub updated_at: DateTime<Utc>,

    /// Arbitrary metadata.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Status of a conversation thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    /// Thread is active and accepting messages.
    Active,
    /// Thread has been resolved / completed.
    Completed,
    /// Thread was abandoned or timed out.
    Abandoned,
}

impl ConversationThread {
    /// Create a new conversation thread.
    pub fn new(title: impl Into<String>, creator: AgentId) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            participants: vec![creator],
            messages: Vec::new(),
            status: ThreadStatus::Active,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Add a participant to the thread.
    pub fn add_participant(&mut self, agent_id: AgentId) {
        if !self.participants.contains(&agent_id) {
            self.participants.push(agent_id);
        }
    }

    /// Remove a participant from the thread.
    pub fn remove_participant(&mut self, agent_id: &AgentId) {
        self.participants.retain(|p| p != agent_id);
    }

    /// Post a message to the thread. Automatically tags the message with the
    /// thread ID and adds the sender as a participant.
    pub fn post(&mut self, mut message: AgentMessage) {
        message.thread_id = Some(self.id);
        self.add_participant(message.from);
        self.updated_at = Utc::now();
        self.messages.push(message);
    }

    /// Get the number of messages in the thread.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Get messages from a specific agent.
    pub fn messages_from(&self, agent_id: &AgentId) -> Vec<&AgentMessage> {
        self.messages.iter().filter(|m| &m.from == agent_id).collect()
    }

    /// Get the last N messages.
    pub fn last_messages(&self, n: usize) -> &[AgentMessage] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }

    /// Mark the thread as completed.
    pub fn complete(&mut self) {
        self.status = ThreadStatus::Completed;
        self.updated_at = Utc::now();
    }

    /// Mark the thread as abandoned.
    pub fn abandon(&mut self) {
        self.status = ThreadStatus::Abandoned;
        self.updated_at = Utc::now();
    }

    /// Check if the thread is still active.
    pub fn is_active(&self) -> bool {
        self.status == ThreadStatus::Active
    }
}

// ---------------------------------------------------------------------------
// ThreadManager
// ---------------------------------------------------------------------------

/// Manages multiple conversation threads.
pub struct ThreadManager {
    threads: DashMap<Uuid, ConversationThread>,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self {
            threads: DashMap::new(),
        }
    }

    /// Create and store a new thread.
    pub fn create_thread(
        &self,
        title: impl Into<String>,
        creator: AgentId,
    ) -> Uuid {
        let thread = ConversationThread::new(title, creator);
        let id = thread.id;
        self.threads.insert(id, thread);
        id
    }

    /// Post a message to an existing thread. Returns false if the thread
    /// doesn't exist or is not active.
    pub fn post_to_thread(&self, thread_id: Uuid, message: AgentMessage) -> bool {
        if let Some(mut thread) = self.threads.get_mut(&thread_id) {
            if thread.is_active() {
                thread.post(message);
                return true;
            }
        }
        false
    }

    /// Retrieve a snapshot of a thread.
    pub fn get_thread(&self, thread_id: &Uuid) -> Option<ConversationThread> {
        self.threads.get(thread_id).map(|t| t.clone())
    }

    /// Mark a thread as completed.
    pub fn complete_thread(&self, thread_id: &Uuid) -> bool {
        if let Some(mut thread) = self.threads.get_mut(thread_id) {
            thread.complete();
            true
        } else {
            false
        }
    }

    /// List all active thread IDs.
    pub fn active_threads(&self) -> Vec<Uuid> {
        self.threads
            .iter()
            .filter(|e| e.value().is_active())
            .map(|e| *e.key())
            .collect()
    }

    /// Total number of threads.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AgentId;

    #[test]
    fn test_message_creation() {
        let agent = AgentId::new();
        let msg = AgentMessage::new(agent, "tasks", MessageKind::Text, "Hello world");

        assert_eq!(msg.from, agent);
        assert_eq!(msg.topic, "tasks");
        assert_eq!(msg.content, "Hello world");
        assert!(msg.to.is_none());
    }

    #[test]
    fn test_message_reply() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();

        let original = AgentMessage::new(a1, "discussion", MessageKind::Request, "What do you think?");
        let reply = original.reply(a2, MessageKind::Response, "Looks good!");

        assert_eq!(reply.from, a2);
        assert_eq!(reply.to, Some(a1));
        assert_eq!(reply.correlation_id, Some(original.id));
        assert_eq!(reply.topic, "discussion");
    }

    #[test]
    fn test_message_bus_pub_sub() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe("code-review");

        let agent = AgentId::new();
        let msg = AgentMessage::new(agent, "code-review", MessageKind::Text, "Please review");

        let receivers = bus.publish(msg.clone());
        assert_eq!(receivers, 1);

        let received = rx.try_recv().unwrap();
        assert_eq!(received.content, "Please review");
        assert_eq!(bus.total_published(), 1);
    }

    #[test]
    fn test_message_bus_multiple_subscribers() {
        let bus = MessageBus::new();
        let mut rx1 = bus.subscribe("updates");
        let mut rx2 = bus.subscribe("updates");

        let agent = AgentId::new();
        let msg = AgentMessage::new(agent, "updates", MessageKind::StatusUpdate, "50% done");
        let receivers = bus.publish(msg);

        assert_eq!(receivers, 2);
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn test_message_bus_no_subscribers() {
        let bus = MessageBus::new();
        let agent = AgentId::new();
        let msg = AgentMessage::new(agent, "void", MessageKind::Text, "Hello?");

        let receivers = bus.publish(msg);
        assert_eq!(receivers, 0);
        assert_eq!(bus.total_published(), 1);
    }

    #[test]
    fn test_message_bus_topics() {
        let bus = MessageBus::new();
        let _rx1 = bus.subscribe("alpha");
        let _rx2 = bus.subscribe("beta");

        assert!(bus.has_topic("alpha"));
        assert!(bus.has_topic("beta"));
        assert!(!bus.has_topic("gamma"));
        assert_eq!(bus.topic_count(), 2);
    }

    #[test]
    fn test_conversation_thread() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();

        let mut thread = ConversationThread::new("Architecture discussion", a1);
        assert!(thread.is_active());
        assert_eq!(thread.participants.len(), 1);

        let msg1 = AgentMessage::new(a1, "arch", MessageKind::Request, "Should we use microservices?");
        thread.post(msg1);

        let msg2 = AgentMessage::new(a2, "arch", MessageKind::Response, "Yes, with event sourcing.");
        thread.post(msg2);

        assert_eq!(thread.message_count(), 2);
        assert_eq!(thread.participants.len(), 2);
        assert_eq!(thread.messages_from(&a1).len(), 1);
        assert_eq!(thread.messages_from(&a2).len(), 1);
    }

    #[test]
    fn test_conversation_thread_completion() {
        let agent = AgentId::new();
        let mut thread = ConversationThread::new("Quick chat", agent);

        assert!(thread.is_active());
        thread.complete();
        assert!(!thread.is_active());
        assert_eq!(thread.status, ThreadStatus::Completed);
    }

    #[test]
    fn test_last_messages() {
        let agent = AgentId::new();
        let mut thread = ConversationThread::new("Long discussion", agent);

        for i in 0..10 {
            let msg = AgentMessage::new(agent, "topic", MessageKind::Text, format!("Message {}", i));
            thread.post(msg);
        }

        let last_3 = thread.last_messages(3);
        assert_eq!(last_3.len(), 3);
        assert_eq!(last_3[0].content, "Message 7");
        assert_eq!(last_3[2].content, "Message 9");
    }

    #[test]
    fn test_thread_manager() {
        let mgr = ThreadManager::new();
        let a1 = AgentId::new();
        let a2 = AgentId::new();

        let thread_id = mgr.create_thread("Design review", a1);
        assert_eq!(mgr.thread_count(), 1);

        let msg = AgentMessage::new(a2, "review", MessageKind::Text, "I have feedback");
        assert!(mgr.post_to_thread(thread_id, msg));

        let thread = mgr.get_thread(&thread_id).unwrap();
        assert_eq!(thread.message_count(), 1);
        assert_eq!(thread.participants.len(), 2);

        assert!(mgr.complete_thread(&thread_id));
        assert!(mgr.active_threads().is_empty());
    }

    #[test]
    fn test_thread_manager_post_to_completed() {
        let mgr = ThreadManager::new();
        let agent = AgentId::new();

        let thread_id = mgr.create_thread("Done thread", agent);
        mgr.complete_thread(&thread_id);

        let msg = AgentMessage::new(agent, "topic", MessageKind::Text, "Late message");
        assert!(!mgr.post_to_thread(thread_id, msg));
    }

    #[test]
    fn test_add_remove_participant() {
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let a3 = AgentId::new();

        let mut thread = ConversationThread::new("Team sync", a1);
        thread.add_participant(a2);
        thread.add_participant(a3);
        assert_eq!(thread.participants.len(), 3);

        // Adding duplicate is a no-op.
        thread.add_participant(a2);
        assert_eq!(thread.participants.len(), 3);

        thread.remove_participant(&a2);
        assert_eq!(thread.participants.len(), 2);
        assert!(!thread.participants.contains(&a2));
    }
}
