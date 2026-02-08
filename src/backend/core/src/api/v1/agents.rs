//! Agent collaboration API handlers (V1).
//!
//! Provides REST endpoints for agent delegation, memory, and communication.
//! - POST /agents/:id/delegate - Delegate a task to another agent
//! - GET /agents/:id/memory - List agent memories
//! - POST /agents/:id/memory - Store a new memory
//! - DELETE /agents/:id/memory/:memory_id - Delete a memory
//! - GET /agents/:id/conversations - List conversations for an agent
//! - GET /threads - List conversation threads
//! - POST /threads - Create a new thread
//! - GET /threads/:id - Get thread details
//! - POST /threads/:id/messages - Post a message to a thread
//! - POST /threads/:id/complete - Mark a thread as completed

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::LazyLock;

use crate::api::{ApiResponse, AppState};
use crate::api::middleware::sanitize_string;
use crate::agents::{
    AgentId,
    MemoryEntry, MemoryType, Importance, MemoryQuery, InMemoryMemoryStore, MemoryManager,
    AgentMessage, MessageKind, ThreadManager, ConversationThread,
};
// DelegationStrategy and DelegationPriority are used by DelegateRequest
// (currently only exercised in tests; will be wired to a handler when
// the DelegationManager is added to AppState).
#[allow(unused_imports)]
use crate::agents::{DelegationStrategy, DelegationPriority};

// ═══════════════════════════════════════════════════════════════════════════════
// In-memory stores (placeholder until wired into AppState)
// ═══════════════════════════════════════════════════════════════════════════════

// Both MemoryManager and ThreadManager are Send + Sync internally (use DashMap/Arc),
// so they don't need an outer Mutex.
static MEMORY_MANAGER: LazyLock<MemoryManager> = LazyLock::new(|| {
    MemoryManager::new(InMemoryMemoryStore::new())
});

static THREAD_MANAGER: LazyLock<ThreadManager> = LazyLock::new(|| {
    ThreadManager::new()
});

// ═══════════════════════════════════════════════════════════════════════════════
// DTOs
// ═══════════════════════════════════════════════════════════════════════════════

/// Request body for delegating a task.
/// Currently used by tests; the delegation endpoint will be wired
/// once the DelegationManager is added to AppState.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegateRequest {
    pub task: String,
    pub to_agent: Option<Uuid>,
    pub strategy: Option<String>,
    pub priority: Option<String>,
    pub required_capabilities: Option<Vec<String>>,
}

#[allow(dead_code)]
impl DelegateRequest {
    fn sanitize(&mut self) {
        self.task = sanitize_string(&self.task);
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.task.is_empty() {
            errors.push("task must not be empty".to_string());
        }
        if self.task.len() > 5000 {
            errors.push("task must be at most 5,000 characters".to_string());
        }
        if let Some(ref strategy) = self.strategy {
            let valid = ["direct", "broadcast", "round_robin", "least_busy", "capability"];
            if !valid.contains(&strategy.as_str()) {
                errors.push(format!("strategy must be one of: {}", valid.join(", ")));
            }
        }
        if let Some(ref priority) = self.priority {
            let valid = ["low", "normal", "high", "critical"];
            if !valid.contains(&priority.as_str()) {
                errors.push(format!("priority must be one of: {}", valid.join(", ")));
            }
        }
        errors
    }

    fn parse_strategy(&self) -> DelegationStrategy {
        match self.strategy.as_deref() {
            Some("direct") => DelegationStrategy::Direct,
            Some("broadcast") => DelegationStrategy::Broadcast,
            Some("round_robin") => DelegationStrategy::RoundRobin,
            Some("least_busy") => DelegationStrategy::LeastBusy,
            Some("capability") => DelegationStrategy::Capability,
            _ => DelegationStrategy::LeastBusy,
        }
    }

    fn parse_priority(&self) -> DelegationPriority {
        match self.priority.as_deref() {
            Some("low") => DelegationPriority::Low,
            Some("normal") => DelegationPriority::Normal,
            Some("high") => DelegationPriority::High,
            Some("critical") => DelegationPriority::Critical,
            _ => DelegationPriority::Normal,
        }
    }
}

/// Request body for storing a memory.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreMemoryRequest {
    pub content: String,
    pub memory_type: Option<String>,
    pub importance: Option<String>,
    pub tags: Option<Vec<String>>,
}

impl StoreMemoryRequest {
    fn sanitize(&mut self) {
        self.content = sanitize_string(&self.content);
        if let Some(ref mut tags) = self.tags {
            *tags = tags.iter().map(|t| sanitize_string(t)).collect();
        }
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.content.is_empty() {
            errors.push("content must not be empty".to_string());
        }
        if self.content.len() > 50_000 {
            errors.push("content must be at most 50,000 characters".to_string());
        }
        if let Some(ref mt) = self.memory_type {
            let valid = ["episodic", "semantic", "procedural"];
            if !valid.contains(&mt.as_str()) {
                errors.push(format!("memoryType must be one of: {}", valid.join(", ")));
            }
        }
        if let Some(ref imp) = self.importance {
            let valid = ["low", "medium", "high", "critical"];
            if !valid.contains(&imp.as_str()) {
                errors.push(format!("importance must be one of: {}", valid.join(", ")));
            }
        }
        errors
    }

    fn parse_memory_type(&self) -> MemoryType {
        match self.memory_type.as_deref() {
            Some("episodic") => MemoryType::Episodic,
            Some("semantic") => MemoryType::Semantic,
            Some("procedural") => MemoryType::Procedural,
            _ => MemoryType::Episodic,
        }
    }

    fn parse_importance(&self) -> Importance {
        match self.importance.as_deref() {
            Some("low") => Importance::Low,
            Some("medium") => Importance::Medium,
            Some("high") => Importance::High,
            Some("critical") => Importance::Critical,
            _ => Importance::Medium,
        }
    }
}

/// Serializable memory summary returned by the API.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryResponse {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub memory_type: String,
    pub content: String,
    pub importance: String,
    pub tags: Vec<String>,
    pub access_count: u64,
    pub relevance_score: f64,
    pub created_at: String,
}

impl From<MemoryEntry> for MemoryResponse {
    fn from(e: MemoryEntry) -> Self {
        Self {
            id: e.id,
            agent_id: e.agent_id.0,
            memory_type: e.memory_type.label().to_string(),
            content: e.content.clone(),
            importance: format!("{:?}", e.importance).to_lowercase(),
            tags: e.tags.clone(),
            access_count: e.access_count,
            relevance_score: e.relevance_score(),
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

/// Request body for creating a thread.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateThreadRequest {
    pub title: String,
    pub creator_agent_id: Uuid,
}

impl CreateThreadRequest {
    fn sanitize(&mut self) {
        self.title = sanitize_string(&self.title);
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.title.is_empty() {
            errors.push("title must not be empty".to_string());
        }
        if self.title.len() > 500 {
            errors.push("title must be at most 500 characters".to_string());
        }
        errors
    }
}

/// Request body for posting a message to a thread.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostMessageRequest {
    pub from_agent_id: Uuid,
    pub kind: Option<String>,
    pub content: String,
}

impl PostMessageRequest {
    fn sanitize(&mut self) {
        self.content = sanitize_string(&self.content);
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.content.is_empty() {
            errors.push("content must not be empty".to_string());
        }
        if self.content.len() > 50_000 {
            errors.push("content must be at most 50,000 characters".to_string());
        }
        if let Some(ref kind) = self.kind {
            let valid = ["text", "data", "request", "response", "status_update", "error", "complete"];
            if !valid.contains(&kind.as_str()) {
                errors.push(format!("kind must be one of: {}", valid.join(", ")));
            }
        }
        errors
    }

    fn parse_kind(&self) -> MessageKind {
        match self.kind.as_deref() {
            Some("text") => MessageKind::Text,
            Some("data") => MessageKind::Data,
            Some("request") => MessageKind::Request,
            Some("response") => MessageKind::Response,
            Some("status_update") => MessageKind::StatusUpdate,
            Some("error") => MessageKind::Error,
            Some("complete") => MessageKind::Complete,
            _ => MessageKind::Text,
        }
    }
}

/// Serializable thread summary for the API.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResponse {
    pub id: Uuid,
    pub title: String,
    pub participants: Vec<Uuid>,
    pub message_count: usize,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ConversationThread> for ThreadResponse {
    fn from(t: ConversationThread) -> Self {
        Self {
            id: t.id,
            title: t.title,
            participants: t.participants.iter().map(|p| p.0).collect(),
            message_count: t.messages.len(),
            status: format!("{:?}", t.status).to_lowercase(),
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
        }
    }
}

/// Serializable message for the API.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub id: Uuid,
    pub from: Uuid,
    pub to: Option<Uuid>,
    pub kind: String,
    pub content: String,
    pub created_at: String,
}

impl From<&AgentMessage> for MessageResponse {
    fn from(m: &AgentMessage) -> Self {
        Self {
            id: m.id.0,
            from: m.from.0,
            to: m.to.map(|a| a.0),
            kind: format!("{:?}", m.kind).to_lowercase(),
            content: m.content.clone(),
            created_at: m.created_at.to_rfc3339(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Memory Handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// GET /agents/:id/memory
pub async fn get_agent_memory(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let agent_id = AgentId(id);
    let query = MemoryQuery::new(100);

    match MEMORY_MANAGER.search(agent_id, &query).await {
        Ok(results) => {
            let memories: Vec<MemoryResponse> = results
                .into_iter()
                .map(|sm| sm.entry.into())
                .collect();
            Json(ApiResponse::success(memories))
        }
        Err(e) => Json(ApiResponse::error(format!("Failed to search memories: {}", e))),
    }
}

/// POST /agents/:id/memory
pub async fn store_agent_memory(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(mut req): Json<StoreMemoryRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            errors.join("; "),
            "VALIDATION_ERROR",
        ));
    }

    let agent_id = AgentId(id);
    let mut entry = MemoryEntry::new(
        agent_id,
        req.parse_memory_type(),
        &req.content,
        req.parse_importance(),
    );
    if let Some(tags) = req.tags {
        for tag in tags {
            entry = entry.with_tag(tag);
        }
    }

    match MEMORY_MANAGER.store(&entry).await {
        Ok(()) => {
            let response: MemoryResponse = entry.into();
            Json(ApiResponse::success(response))
        }
        Err(e) => Json(ApiResponse::error(format!("Failed to store memory: {}", e))),
    }
}

/// DELETE /agents/:id/memory/:memory_id
pub async fn delete_agent_memory(
    State(_state): State<AppState>,
    Path((id, memory_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    let _ = AgentId(id);

    match MEMORY_MANAGER.forget(memory_id).await {
        Ok(true) => Json(ApiResponse::success(serde_json::json!({
            "id": memory_id,
            "status": "deleted"
        }))),
        Ok(false) => Json(ApiResponse::error("Memory not found")),
        Err(e) => Json(ApiResponse::error(format!("Failed to delete memory: {}", e))),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Thread Handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// GET /threads
pub async fn list_threads(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let active_ids = THREAD_MANAGER.active_threads();
    let threads: Vec<ThreadResponse> = active_ids
        .iter()
        .filter_map(|id| THREAD_MANAGER.get_thread(id))
        .map(Into::into)
        .collect();

    Json(ApiResponse::success(threads))
}

/// POST /threads
pub async fn create_thread(
    State(_state): State<AppState>,
    Json(mut req): Json<CreateThreadRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            errors.join("; "),
            "VALIDATION_ERROR",
        ));
    }

    let creator = AgentId(req.creator_agent_id);
    let thread_id = THREAD_MANAGER.create_thread(&req.title, creator);

    match THREAD_MANAGER.get_thread(&thread_id) {
        Some(thread) => {
            let response: ThreadResponse = thread.into();
            Json(ApiResponse::success(response))
        }
        None => Json(ApiResponse::error("Failed to create thread")),
    }
}

/// GET /threads/:id
pub async fn get_thread(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match THREAD_MANAGER.get_thread(&id) {
        Some(thread) => {
            let messages: Vec<MessageResponse> = thread.messages.iter().map(Into::into).collect();
            Json(ApiResponse::success(serde_json::json!({
                "id": thread.id,
                "title": thread.title,
                "participants": thread.participants.iter().map(|p| p.0).collect::<Vec<_>>(),
                "messages": messages,
                "status": format!("{:?}", thread.status).to_lowercase(),
                "messageCount": thread.messages.len(),
                "createdAt": thread.created_at.to_rfc3339(),
                "updatedAt": thread.updated_at.to_rfc3339(),
            })))
        }
        None => Json(ApiResponse::error("Thread not found")),
    }
}

/// POST /threads/:id/messages
pub async fn post_thread_message(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(mut req): Json<PostMessageRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            errors.join("; "),
            "VALIDATION_ERROR",
        ));
    }

    let from = AgentId(req.from_agent_id);
    let message = AgentMessage::new(from, "thread", req.parse_kind(), &req.content);

    if THREAD_MANAGER.post_to_thread(id, message) {
        match THREAD_MANAGER.get_thread(&id) {
            Some(thread) => {
                let response: ThreadResponse = thread.into();
                Json(ApiResponse::success(response))
            }
            None => Json(ApiResponse::error("Thread not found after posting")),
        }
    } else {
        Json(ApiResponse::error("Thread not found or not active"))
    }
}

/// POST /threads/:id/complete
pub async fn complete_thread(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    if THREAD_MANAGER.complete_thread(&id) {
        match THREAD_MANAGER.get_thread(&id) {
            Some(thread) => {
                let response: ThreadResponse = thread.into();
                Json(ApiResponse::success(response))
            }
            None => Json(ApiResponse::error("Thread not found after completion")),
        }
    } else {
        Json(ApiResponse::error("Thread not found"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_delegate_request_valid() {
        let req = DelegateRequest {
            task: "Review this code".to_string(),
            to_agent: Some(Uuid::new_v4()),
            strategy: Some("direct".to_string()),
            priority: Some("high".to_string()),
            required_capabilities: None,
        };
        assert!(req.validate().is_empty());
    }

    #[test]
    fn test_validate_delegate_request_empty_task() {
        let req = DelegateRequest {
            task: "".to_string(),
            to_agent: None,
            strategy: None,
            priority: None,
            required_capabilities: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("task")));
    }

    #[test]
    fn test_validate_delegate_request_invalid_strategy() {
        let req = DelegateRequest {
            task: "Do work".to_string(),
            to_agent: None,
            strategy: Some("random".to_string()),
            priority: None,
            required_capabilities: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("strategy")));
    }

    #[test]
    fn test_validate_delegate_request_invalid_priority() {
        let req = DelegateRequest {
            task: "Do work".to_string(),
            to_agent: None,
            strategy: None,
            priority: Some("urgent".to_string()),
            required_capabilities: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("priority")));
    }

    #[test]
    fn test_parse_strategy() {
        let req = DelegateRequest {
            task: "x".to_string(),
            to_agent: None,
            strategy: Some("round_robin".to_string()),
            priority: None,
            required_capabilities: None,
        };
        assert_eq!(req.parse_strategy(), DelegationStrategy::RoundRobin);
    }

    #[test]
    fn test_parse_strategy_default() {
        let req = DelegateRequest {
            task: "x".to_string(),
            to_agent: None,
            strategy: None,
            priority: None,
            required_capabilities: None,
        };
        assert_eq!(req.parse_strategy(), DelegationStrategy::LeastBusy);
    }

    #[test]
    fn test_parse_priority() {
        let req = DelegateRequest {
            task: "x".to_string(),
            to_agent: None,
            strategy: None,
            priority: Some("critical".to_string()),
            required_capabilities: None,
        };
        assert_eq!(req.parse_priority(), DelegationPriority::Critical);
    }

    #[test]
    fn test_validate_store_memory_valid() {
        let req = StoreMemoryRequest {
            content: "User prefers Rust".to_string(),
            memory_type: Some("semantic".to_string()),
            importance: Some("high".to_string()),
            tags: Some(vec!["language".to_string()]),
        };
        assert!(req.validate().is_empty());
    }

    #[test]
    fn test_validate_store_memory_empty_content() {
        let req = StoreMemoryRequest {
            content: "".to_string(),
            memory_type: None,
            importance: None,
            tags: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("content")));
    }

    #[test]
    fn test_validate_store_memory_invalid_type() {
        let req = StoreMemoryRequest {
            content: "something".to_string(),
            memory_type: Some("invalid".to_string()),
            importance: None,
            tags: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("memoryType")));
    }

    #[test]
    fn test_validate_store_memory_invalid_importance() {
        let req = StoreMemoryRequest {
            content: "something".to_string(),
            memory_type: None,
            importance: Some("very_high".to_string()),
            tags: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("importance")));
    }

    #[test]
    fn test_parse_memory_type() {
        let req = StoreMemoryRequest {
            content: "x".to_string(),
            memory_type: Some("procedural".to_string()),
            importance: None,
            tags: None,
        };
        assert_eq!(req.parse_memory_type(), MemoryType::Procedural);
    }

    #[test]
    fn test_parse_memory_type_default() {
        let req = StoreMemoryRequest {
            content: "x".to_string(),
            memory_type: None,
            importance: None,
            tags: None,
        };
        assert_eq!(req.parse_memory_type(), MemoryType::Episodic);
    }

    #[test]
    fn test_validate_create_thread_valid() {
        let req = CreateThreadRequest {
            title: "Architecture discussion".to_string(),
            creator_agent_id: Uuid::new_v4(),
        };
        assert!(req.validate().is_empty());
    }

    #[test]
    fn test_validate_create_thread_empty_title() {
        let req = CreateThreadRequest {
            title: "".to_string(),
            creator_agent_id: Uuid::new_v4(),
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("title")));
    }

    #[test]
    fn test_validate_create_thread_long_title() {
        let req = CreateThreadRequest {
            title: "x".repeat(501),
            creator_agent_id: Uuid::new_v4(),
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("500")));
    }

    #[test]
    fn test_validate_post_message_valid() {
        let req = PostMessageRequest {
            from_agent_id: Uuid::new_v4(),
            kind: Some("text".to_string()),
            content: "Hello!".to_string(),
        };
        assert!(req.validate().is_empty());
    }

    #[test]
    fn test_validate_post_message_empty_content() {
        let req = PostMessageRequest {
            from_agent_id: Uuid::new_v4(),
            kind: None,
            content: "".to_string(),
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("content")));
    }

    #[test]
    fn test_validate_post_message_invalid_kind() {
        let req = PostMessageRequest {
            from_agent_id: Uuid::new_v4(),
            kind: Some("invalid_kind".to_string()),
            content: "Hello".to_string(),
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("kind")));
    }

    #[test]
    fn test_parse_message_kind() {
        let req = PostMessageRequest {
            from_agent_id: Uuid::new_v4(),
            kind: Some("request".to_string()),
            content: "x".to_string(),
        };
        assert_eq!(req.parse_kind(), MessageKind::Request);
    }

    #[test]
    fn test_parse_message_kind_default() {
        let req = PostMessageRequest {
            from_agent_id: Uuid::new_v4(),
            kind: None,
            content: "x".to_string(),
        };
        assert_eq!(req.parse_kind(), MessageKind::Text);
    }

    #[test]
    fn test_memory_response_from_entry() {
        let agent_id = AgentId::new();
        let entry = MemoryEntry::new(
            agent_id,
            MemoryType::Semantic,
            "User likes Rust",
            Importance::High,
        ).with_tag("preference");

        let response: MemoryResponse = entry.into();
        assert_eq!(response.memory_type, "semantic");
        assert_eq!(response.content, "User likes Rust");
        assert_eq!(response.tags, vec!["preference"]);
        assert_eq!(response.agent_id, agent_id.0);
    }

    #[test]
    fn test_validate_delegate_request_task_too_long() {
        let req = DelegateRequest {
            task: "x".repeat(5001),
            to_agent: None,
            strategy: None,
            priority: None,
            required_capabilities: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("5,000")));
    }

    #[test]
    fn test_validate_store_memory_content_too_long() {
        let req = StoreMemoryRequest {
            content: "x".repeat(50_001),
            memory_type: None,
            importance: None,
            tags: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("50,000")));
    }

    #[test]
    fn test_validate_post_message_content_too_long() {
        let req = PostMessageRequest {
            from_agent_id: Uuid::new_v4(),
            kind: None,
            content: "x".repeat(50_001),
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("50,000")));
    }

    #[test]
    fn test_all_strategies_parseable() {
        for (input, expected) in [
            ("direct", DelegationStrategy::Direct),
            ("broadcast", DelegationStrategy::Broadcast),
            ("round_robin", DelegationStrategy::RoundRobin),
            ("least_busy", DelegationStrategy::LeastBusy),
            ("capability", DelegationStrategy::Capability),
        ] {
            let req = DelegateRequest {
                task: "x".to_string(),
                to_agent: None,
                strategy: Some(input.to_string()),
                priority: None,
                required_capabilities: None,
            };
            assert_eq!(req.parse_strategy(), expected);
        }
    }

    #[test]
    fn test_all_priorities_parseable() {
        for (input, expected) in [
            ("low", DelegationPriority::Low),
            ("normal", DelegationPriority::Normal),
            ("high", DelegationPriority::High),
            ("critical", DelegationPriority::Critical),
        ] {
            let req = DelegateRequest {
                task: "x".to_string(),
                to_agent: None,
                strategy: None,
                priority: Some(input.to_string()),
                required_capabilities: None,
            };
            assert_eq!(req.parse_priority(), expected);
        }
    }

    #[test]
    fn test_all_message_kinds_parseable() {
        for (input, expected) in [
            ("text", MessageKind::Text),
            ("data", MessageKind::Data),
            ("request", MessageKind::Request),
            ("response", MessageKind::Response),
            ("status_update", MessageKind::StatusUpdate),
            ("error", MessageKind::Error),
            ("complete", MessageKind::Complete),
        ] {
            let req = PostMessageRequest {
                from_agent_id: Uuid::new_v4(),
                kind: Some(input.to_string()),
                content: "x".to_string(),
            };
            assert_eq!(req.parse_kind(), expected);
        }
    }

    #[test]
    fn test_all_memory_types_parseable() {
        for (input, expected) in [
            ("episodic", MemoryType::Episodic),
            ("semantic", MemoryType::Semantic),
            ("procedural", MemoryType::Procedural),
        ] {
            let req = StoreMemoryRequest {
                content: "x".to_string(),
                memory_type: Some(input.to_string()),
                importance: None,
                tags: None,
            };
            assert_eq!(req.parse_memory_type(), expected);
        }
    }

    #[test]
    fn test_all_importance_levels_parseable() {
        for (input, expected) in [
            ("low", Importance::Low),
            ("medium", Importance::Medium),
            ("high", Importance::High),
            ("critical", Importance::Critical),
        ] {
            let req = StoreMemoryRequest {
                content: "x".to_string(),
                memory_type: None,
                importance: Some(input.to_string()),
                tags: None,
            };
            assert_eq!(req.parse_importance(), expected);
        }
    }
}
