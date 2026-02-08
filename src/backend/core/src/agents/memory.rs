//! Agent memory system for multi-turn interactions.
//!
//! Provides episodic, semantic, and procedural memory types with relevance-based
//! retrieval, enabling agents to recall and leverage past experiences.

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
// Memory Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Classification of memory by cognitive type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Episodic: specific events or interactions (e.g., "user asked about X on Tuesday").
    Episodic,
    /// Semantic: factual knowledge or learned concepts (e.g., "user prefers Python").
    Semantic,
    /// Procedural: learned procedures or strategies (e.g., "when asked about code, provide examples").
    Procedural,
}

impl MemoryType {
    /// Returns a human-readable label for the memory type.
    pub fn label(&self) -> &'static str {
        match self {
            MemoryType::Episodic => "episodic",
            MemoryType::Semantic => "semantic",
            MemoryType::Procedural => "procedural",
        }
    }
}

/// Importance level for memory entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Importance {
    Low,
    Medium,
    High,
    Critical,
}

impl Importance {
    /// Returns a numeric weight for relevance scoring.
    pub fn weight(&self) -> f64 {
        match self {
            Importance::Low => 0.25,
            Importance::Medium => 0.5,
            Importance::High => 0.75,
            Importance::Critical => 1.0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Memory Entry
// ═══════════════════════════════════════════════════════════════════════════════

/// A single memory entry stored by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique memory identifier.
    pub id: Uuid,
    /// Agent that owns this memory.
    pub agent_id: AgentId,
    /// Type of memory.
    pub memory_type: MemoryType,
    /// The memory content (natural language or structured).
    pub content: String,
    /// Importance level.
    pub importance: Importance,
    /// Tags for categorization and filtering.
    pub tags: Vec<String>,
    /// Structured metadata.
    pub metadata: HashMap<String, serde_json::Value>,
    /// Optional embedding vector for similarity search.
    pub embedding: Option<Vec<f32>>,
    /// Number of times this memory has been accessed.
    pub access_count: u64,
    /// Last time this memory was accessed.
    pub last_accessed_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Expiry timestamp (if the memory should decay).
    pub expires_at: Option<DateTime<Utc>>,
}

impl MemoryEntry {
    /// Create a new memory entry.
    pub fn new(
        agent_id: AgentId,
        memory_type: MemoryType,
        content: impl Into<String>,
        importance: Importance,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id,
            memory_type,
            content: content.into(),
            importance,
            tags: Vec::new(),
            metadata: HashMap::new(),
            embedding: None,
            access_count: 0,
            last_accessed_at: None,
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set an embedding vector.
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Set an expiry time.
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Record an access (bumps count and timestamp).
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed_at = Some(Utc::now());
    }

    /// Check if the memory has expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .map(|exp| Utc::now() > exp)
            .unwrap_or(false)
    }

    /// Calculate a relevance score based on recency, importance, and access frequency.
    /// Returns a value in [0.0, 1.0].
    pub fn relevance_score(&self) -> f64 {
        let importance_score = self.importance.weight();

        // Recency score: exponential decay over 30 days
        let age_hours = (Utc::now() - self.created_at).num_hours().max(0) as f64;
        let recency_score = (-age_hours / (30.0 * 24.0)).exp();

        // Access frequency score: logarithmic scaling
        let frequency_score = (1.0 + self.access_count as f64).ln() / (1.0 + 100.0_f64).ln();
        let frequency_score = frequency_score.min(1.0);

        // Weighted combination
        0.4 * importance_score + 0.35 * recency_score + 0.25 * frequency_score
    }
}

/// A scored memory result from a search.
#[derive(Debug, Clone)]
pub struct ScoredMemory {
    pub entry: MemoryEntry,
    pub score: f64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Agent Memory (Short-Term, Long-Term, Working)
// ═══════════════════════════════════════════════════════════════════════════════

/// Aggregated memory view for an agent, split into short-term, long-term, and working memory.
#[derive(Debug)]
pub struct AgentMemory {
    /// Agent this memory belongs to.
    pub agent_id: AgentId,
    /// Short-term memory: recent, transient entries for the current session.
    pub short_term: Vec<MemoryEntry>,
    /// Long-term memory: persistent, consolidated knowledge.
    pub long_term: Vec<MemoryEntry>,
    /// Working memory: active entries currently in use for a task.
    pub working: Vec<MemoryEntry>,
}

impl AgentMemory {
    /// Create an empty memory for an agent.
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            short_term: Vec::new(),
            long_term: Vec::new(),
            working: Vec::new(),
        }
    }

    /// Add an entry to short-term memory.
    pub fn add_short_term(&mut self, entry: MemoryEntry) {
        self.short_term.push(entry);
    }

    /// Add an entry to long-term memory.
    pub fn add_long_term(&mut self, entry: MemoryEntry) {
        self.long_term.push(entry);
    }

    /// Promote the most relevant short-term memories to long-term.
    /// Returns the number of entries promoted.
    pub fn consolidate(&mut self, threshold: f64) -> usize {
        let mut promoted = 0;
        self.short_term.retain(|entry| {
            if entry.relevance_score() >= threshold {
                self.long_term.push(entry.clone());
                promoted += 1;
                false // remove from short-term
            } else {
                true
            }
        });
        promoted
    }

    /// Load entries into working memory for an active task.
    pub fn load_working(&mut self, entries: Vec<MemoryEntry>) {
        self.working = entries;
    }

    /// Clear working memory after task completion.
    pub fn clear_working(&mut self) {
        self.working.clear();
    }

    /// Get all memories sorted by relevance.
    pub fn all_sorted_by_relevance(&self) -> Vec<ScoredMemory> {
        let mut all: Vec<ScoredMemory> = self
            .short_term
            .iter()
            .chain(self.long_term.iter())
            .chain(self.working.iter())
            .map(|e| ScoredMemory {
                score: e.relevance_score(),
                entry: e.clone(),
            })
            .collect();
        all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        all
    }

    /// Total number of memories across all stores.
    pub fn total_count(&self) -> usize {
        self.short_term.len() + self.long_term.len() + self.working.len()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MemoryManager
// ═══════════════════════════════════════════════════════════════════════════════

/// Query parameters for searching memories.
#[derive(Debug, Clone, Default)]
pub struct MemoryQuery {
    /// Filter by memory type.
    pub memory_type: Option<MemoryType>,
    /// Filter by tags (any match).
    pub tags: Vec<String>,
    /// Minimum importance level.
    pub min_importance: Option<Importance>,
    /// Maximum number of results.
    pub limit: usize,
    /// Text to match against content (simple substring match).
    pub text_query: Option<String>,
}

impl MemoryQuery {
    /// Create a new query with a result limit.
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            ..Default::default()
        }
    }

    /// Filter by memory type.
    pub fn with_type(mut self, memory_type: MemoryType) -> Self {
        self.memory_type = Some(memory_type);
        self
    }

    /// Filter by tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Filter by minimum importance.
    pub fn with_min_importance(mut self, importance: Importance) -> Self {
        self.min_importance = Some(importance);
        self
    }

    /// Filter by text content.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text_query = Some(text.into());
        self
    }
}

/// Manages the full memory lifecycle for agents: storage, retrieval, consolidation, and cleanup.
pub struct MemoryManager {
    store: Arc<dyn MemoryStore>,
}

impl MemoryManager {
    /// Create a new memory manager with the given backing store.
    pub fn new(store: impl MemoryStore + 'static) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Store a new memory entry.
    pub async fn store(&self, entry: &MemoryEntry) -> Result<()> {
        self.store.save(entry).await
    }

    /// Retrieve a memory entry by ID and record the access.
    pub async fn recall(&self, memory_id: Uuid) -> Result<Option<MemoryEntry>> {
        if let Some(mut entry) = self.store.load(memory_id).await? {
            entry.record_access();
            self.store.save(&entry).await?;
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    /// Search memories for a given agent using a query.
    pub async fn search(
        &self,
        agent_id: AgentId,
        query: &MemoryQuery,
    ) -> Result<Vec<ScoredMemory>> {
        self.store.search(agent_id, query).await
    }

    /// Delete a memory entry.
    pub async fn forget(&self, memory_id: Uuid) -> Result<bool> {
        self.store.delete(memory_id).await
    }

    /// Remove all expired memories for an agent.
    pub async fn cleanup_expired(&self, agent_id: AgentId) -> Result<u64> {
        self.store.delete_expired(agent_id).await
    }

    /// Build an AgentMemory snapshot by loading recent and relevant entries.
    pub async fn build_agent_memory(
        &self,
        agent_id: AgentId,
        short_term_limit: usize,
        long_term_limit: usize,
    ) -> Result<AgentMemory> {
        let mut memory = AgentMemory::new(agent_id);

        // Short-term: recent episodic memories
        let short_term_query = MemoryQuery::new(short_term_limit).with_type(MemoryType::Episodic);
        let short_term = self.store.search(agent_id, &short_term_query).await?;
        memory.short_term = short_term.into_iter().map(|sm| sm.entry).collect();

        // Long-term: high-importance semantic and procedural memories
        let long_term_query = MemoryQuery::new(long_term_limit)
            .with_min_importance(Importance::Medium);
        let long_term = self.store.search(agent_id, &long_term_query).await?;
        memory.long_term = long_term.into_iter().map(|sm| sm.entry).collect();

        Ok(memory)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MemoryStore Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for persisting and querying agent memories.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Save or update a memory entry.
    async fn save(&self, entry: &MemoryEntry) -> Result<()>;

    /// Load a memory entry by ID.
    async fn load(&self, memory_id: Uuid) -> Result<Option<MemoryEntry>>;

    /// Delete a memory entry by ID.
    async fn delete(&self, memory_id: Uuid) -> Result<bool>;

    /// Search memories with relevance scoring.
    async fn search(&self, agent_id: AgentId, query: &MemoryQuery) -> Result<Vec<ScoredMemory>>;

    /// Delete all expired memories for an agent. Returns count deleted.
    async fn delete_expired(&self, agent_id: AgentId) -> Result<u64>;
}

// ═══════════════════════════════════════════════════════════════════════════════
// InMemoryMemoryStore
// ═══════════════════════════════════════════════════════════════════════════════

/// In-memory implementation of MemoryStore for testing and development.
pub struct InMemoryMemoryStore {
    entries: DashMap<Uuid, MemoryEntry>,
}

impl InMemoryMemoryStore {
    /// Create a new in-memory memory store.
    pub fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }
}

impl Default for InMemoryMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for InMemoryMemoryStore {
    async fn save(&self, entry: &MemoryEntry) -> Result<()> {
        self.entries.insert(entry.id, entry.clone());
        Ok(())
    }

    async fn load(&self, memory_id: Uuid) -> Result<Option<MemoryEntry>> {
        Ok(self.entries.get(&memory_id).map(|e| e.value().clone()))
    }

    async fn delete(&self, memory_id: Uuid) -> Result<bool> {
        Ok(self.entries.remove(&memory_id).is_some())
    }

    async fn search(&self, agent_id: AgentId, query: &MemoryQuery) -> Result<Vec<ScoredMemory>> {
        let mut results: Vec<ScoredMemory> = self
            .entries
            .iter()
            .filter(|e| {
                let entry = e.value();
                if entry.agent_id != agent_id {
                    return false;
                }
                if entry.is_expired() {
                    return false;
                }
                if let Some(ref mt) = query.memory_type {
                    if &entry.memory_type != mt {
                        return false;
                    }
                }
                if let Some(ref min_imp) = query.min_importance {
                    if &entry.importance < min_imp {
                        return false;
                    }
                }
                if !query.tags.is_empty()
                    && !query.tags.iter().any(|t| entry.tags.contains(t))
                {
                    return false;
                }
                if let Some(ref text) = query.text_query {
                    let lower_content = entry.content.to_lowercase();
                    let lower_query = text.to_lowercase();
                    if !lower_content.contains(&lower_query) {
                        return false;
                    }
                }
                true
            })
            .map(|e| {
                let entry = e.value().clone();
                let score = entry.relevance_score();
                ScoredMemory { entry, score }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply limit
        results.truncate(query.limit);

        Ok(results)
    }

    async fn delete_expired(&self, agent_id: AgentId) -> Result<u64> {
        let expired_ids: Vec<Uuid> = self
            .entries
            .iter()
            .filter(|e| e.value().agent_id == agent_id && e.value().is_expired())
            .map(|e| e.value().id)
            .collect();

        let count = expired_ids.len() as u64;
        for id in expired_ids {
            self.entries.remove(&id);
        }
        Ok(count)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PostgresMemoryStore
// ═══════════════════════════════════════════════════════════════════════════════

/// PostgreSQL-backed implementation of MemoryStore using sqlx.
pub struct PostgresMemoryStore {
    pool: PgPool,
}

impl PostgresMemoryStore {
    /// Create a new Postgres memory store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MemoryStore for PostgresMemoryStore {
    async fn save(&self, entry: &MemoryEntry) -> Result<()> {
        let tags_json = serde_json::to_value(&entry.tags)
            .map_err(|e| ApexError::internal(format!("Failed to serialize tags: {}", e)))?;
        let metadata_json = serde_json::to_value(&entry.metadata)
            .map_err(|e| ApexError::internal(format!("Failed to serialize metadata: {}", e)))?;
        let embedding_json = entry.embedding.as_ref().map(|e| {
            serde_json::to_value(e).unwrap_or_default()
        });

        sqlx::query(
            r#"
            INSERT INTO agent_memories (
                id, agent_id, memory_type, content, importance, tags, metadata,
                embedding, access_count, last_accessed_at, created_at, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO UPDATE SET
                content = EXCLUDED.content,
                importance = EXCLUDED.importance,
                tags = EXCLUDED.tags,
                metadata = EXCLUDED.metadata,
                embedding = EXCLUDED.embedding,
                access_count = EXCLUDED.access_count,
                last_accessed_at = EXCLUDED.last_accessed_at,
                expires_at = EXCLUDED.expires_at
            "#,
        )
        .bind(entry.id)
        .bind(entry.agent_id.0)
        .bind(entry.memory_type.label())
        .bind(&entry.content)
        .bind(serde_json::to_value(&entry.importance).unwrap_or_default().as_str().unwrap_or("medium"))
        .bind(&tags_json)
        .bind(&metadata_json)
        .bind(&embedding_json)
        .bind(entry.access_count as i64)
        .bind(entry.last_accessed_at)
        .bind(entry.created_at)
        .bind(entry.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load(&self, memory_id: Uuid) -> Result<Option<MemoryEntry>> {
        let row = sqlx::query_as::<_, MemoryRow>(
            r#"
            SELECT id, agent_id, memory_type, content, importance, tags, metadata,
                   embedding, access_count, last_accessed_at, created_at, expires_at
            FROM agent_memories
            WHERE id = $1
            "#,
        )
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.into_entry()?)),
            None => Ok(None),
        }
    }

    async fn delete(&self, memory_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM agent_memories WHERE id = $1")
            .bind(memory_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn search(&self, agent_id: AgentId, query: &MemoryQuery) -> Result<Vec<ScoredMemory>> {
        // Build a broad query and filter/score in Rust for flexibility.
        // For production at scale, this would use pgvector for embedding similarity.
        let rows = sqlx::query_as::<_, MemoryRow>(
            r#"
            SELECT id, agent_id, memory_type, content, importance, tags, metadata,
                   embedding, access_count, last_accessed_at, created_at, expires_at
            FROM agent_memories
            WHERE agent_id = $1
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            LIMIT 500
            "#,
        )
        .bind(agent_id.0)
        .fetch_all(&self.pool)
        .await?;

        let mut results: Vec<ScoredMemory> = Vec::new();

        for row in rows {
            let entry = row.into_entry()?;

            // Apply filters
            if let Some(ref mt) = query.memory_type {
                if &entry.memory_type != mt {
                    continue;
                }
            }
            if let Some(ref min_imp) = query.min_importance {
                if &entry.importance < min_imp {
                    continue;
                }
            }
            if !query.tags.is_empty() && !query.tags.iter().any(|t| entry.tags.contains(t)) {
                continue;
            }
            if let Some(ref text) = query.text_query {
                if !entry.content.to_lowercase().contains(&text.to_lowercase()) {
                    continue;
                }
            }

            let score = entry.relevance_score();
            results.push(ScoredMemory { entry, score });
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(query.limit);

        Ok(results)
    }

    async fn delete_expired(&self, agent_id: AgentId) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM agent_memories WHERE agent_id = $1 AND expires_at IS NOT NULL AND expires_at < NOW()",
        )
        .bind(agent_id.0)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SQLx Row Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(sqlx::FromRow)]
struct MemoryRow {
    id: Uuid,
    agent_id: Uuid,
    memory_type: String,
    content: String,
    importance: String,
    tags: serde_json::Value,
    metadata: serde_json::Value,
    embedding: Option<serde_json::Value>,
    access_count: i64,
    last_accessed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

impl MemoryRow {
    fn into_entry(self) -> Result<MemoryEntry> {
        let memory_type = match self.memory_type.as_str() {
            "episodic" => MemoryType::Episodic,
            "semantic" => MemoryType::Semantic,
            "procedural" => MemoryType::Procedural,
            other => {
                return Err(ApexError::internal(format!(
                    "Unknown memory type: {}",
                    other
                )));
            }
        };

        let importance = match self.importance.as_str() {
            "low" => Importance::Low,
            "medium" => Importance::Medium,
            "high" => Importance::High,
            "critical" => Importance::Critical,
            other => {
                return Err(ApexError::internal(format!(
                    "Unknown importance level: {}",
                    other
                )));
            }
        };

        let tags: Vec<String> = serde_json::from_value(self.tags)
            .map_err(|e| ApexError::internal(format!("Failed to deserialize tags: {}", e)))?;
        let metadata: HashMap<String, serde_json::Value> =
            serde_json::from_value(self.metadata).map_err(|e| {
                ApexError::internal(format!("Failed to deserialize metadata: {}", e))
            })?;
        let embedding: Option<Vec<f32>> = self
            .embedding
            .map(|v| serde_json::from_value(v))
            .transpose()
            .map_err(|e| {
                ApexError::internal(format!("Failed to deserialize embedding: {}", e))
            })?;

        Ok(MemoryEntry {
            id: self.id,
            agent_id: AgentId(self.agent_id),
            memory_type,
            content: self.content,
            importance,
            tags,
            metadata,
            embedding,
            access_count: self.access_count as u64,
            last_accessed_at: self.last_accessed_at,
            created_at: self.created_at,
            expires_at: self.expires_at,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_store_crud() {
        let store = InMemoryMemoryStore::new();
        let agent_id = AgentId::new();

        let entry = MemoryEntry::new(
            agent_id,
            MemoryType::Semantic,
            "User prefers Rust over Python",
            Importance::High,
        )
        .with_tag("preference")
        .with_tag("language");

        let mem_id = entry.id;

        // Save
        store.save(&entry).await.unwrap();

        // Load
        let loaded = store.load(mem_id).await.unwrap().unwrap();
        assert_eq!(loaded.content, "User prefers Rust over Python");
        assert_eq!(loaded.tags.len(), 2);

        // Delete
        let deleted = store.delete(mem_id).await.unwrap();
        assert!(deleted);

        let gone = store.load(mem_id).await.unwrap();
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn test_memory_search() {
        let store = InMemoryMemoryStore::new();
        let agent_id = AgentId::new();

        // Create varied memories
        let entries = vec![
            MemoryEntry::new(agent_id, MemoryType::Semantic, "Likes Rust", Importance::High)
                .with_tag("language"),
            MemoryEntry::new(agent_id, MemoryType::Episodic, "Asked about Rust closures", Importance::Medium)
                .with_tag("rust"),
            MemoryEntry::new(agent_id, MemoryType::Procedural, "When coding, use examples", Importance::Low)
                .with_tag("coding"),
            MemoryEntry::new(agent_id, MemoryType::Semantic, "Prefers dark mode", Importance::Low)
                .with_tag("preference"),
        ];

        for e in &entries {
            store.save(e).await.unwrap();
        }

        // Search by type
        let query = MemoryQuery::new(10).with_type(MemoryType::Semantic);
        let results = store.search(agent_id, &query).await.unwrap();
        assert_eq!(results.len(), 2);

        // Search by min importance
        let query = MemoryQuery::new(10).with_min_importance(Importance::Medium);
        let results = store.search(agent_id, &query).await.unwrap();
        assert_eq!(results.len(), 2);

        // Search by text
        let query = MemoryQuery::new(10).with_text("Rust");
        let results = store.search(agent_id, &query).await.unwrap();
        assert_eq!(results.len(), 2);

        // Search by tag
        let query = MemoryQuery::new(10).with_tag("language".to_string());
        let results = store.search(agent_id, &query).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_expiry() {
        let store = InMemoryMemoryStore::new();
        let agent_id = AgentId::new();

        let expired = MemoryEntry::new(
            agent_id,
            MemoryType::Episodic,
            "Old event",
            Importance::Low,
        )
        .with_expiry(Utc::now() - chrono::Duration::hours(1));

        let active = MemoryEntry::new(
            agent_id,
            MemoryType::Episodic,
            "Recent event",
            Importance::Low,
        );

        store.save(&expired).await.unwrap();
        store.save(&active).await.unwrap();

        // Cleanup should remove the expired entry
        let count = store.delete_expired(agent_id).await.unwrap();
        assert_eq!(count, 1);

        // Only active should remain in search
        let query = MemoryQuery::new(10);
        let results = store.search(agent_id, &query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.content, "Recent event");
    }

    #[tokio::test]
    async fn test_memory_manager_build_agent_memory() {
        let store = InMemoryMemoryStore::new();
        let agent_id = AgentId::new();

        // Episodic (short-term candidates)
        store
            .save(&MemoryEntry::new(
                agent_id,
                MemoryType::Episodic,
                "User asked about X",
                Importance::Medium,
            ))
            .await
            .unwrap();

        // Semantic (long-term candidates)
        store
            .save(&MemoryEntry::new(
                agent_id,
                MemoryType::Semantic,
                "User is a Rust developer",
                Importance::High,
            ))
            .await
            .unwrap();

        let manager = MemoryManager::new(store);
        let agent_memory = manager.build_agent_memory(agent_id, 10, 10).await.unwrap();

        assert_eq!(agent_memory.short_term.len(), 1);
        // Long-term query uses min_importance=Medium, so both qualify
        assert_eq!(agent_memory.long_term.len(), 2);
    }

    #[test]
    fn test_agent_memory_consolidate() {
        let agent_id = AgentId::new();
        let mut memory = AgentMemory::new(agent_id);

        memory.add_short_term(MemoryEntry::new(
            agent_id,
            MemoryType::Episodic,
            "Important event",
            Importance::Critical,
        ));
        memory.add_short_term(MemoryEntry::new(
            agent_id,
            MemoryType::Episodic,
            "Trivial event",
            Importance::Low,
        ));

        assert_eq!(memory.short_term.len(), 2);
        assert_eq!(memory.long_term.len(), 0);

        // Consolidate with a high threshold so only the critical one promotes
        let promoted = memory.consolidate(0.5);
        assert_eq!(promoted, 1);
        assert_eq!(memory.short_term.len(), 1);
        assert_eq!(memory.long_term.len(), 1);
        assert_eq!(memory.long_term[0].content, "Important event");
    }

    #[test]
    fn test_relevance_scoring() {
        let agent_id = AgentId::new();

        let critical = MemoryEntry::new(agent_id, MemoryType::Semantic, "Critical", Importance::Critical);
        let low = MemoryEntry::new(agent_id, MemoryType::Semantic, "Low", Importance::Low);

        // Critical should score higher than low
        assert!(critical.relevance_score() > low.relevance_score());
    }
}
