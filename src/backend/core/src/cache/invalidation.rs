//! Cache invalidation strategies.
//!
//! This module provides:
//! - Tag-based invalidation for grouped cache entries
//! - Pattern-based invalidation using wildcards
//! - Event-driven invalidation with pub/sub support
//! - Bulk invalidation operations

use crate::cache::backend::CacheBackend;
use crate::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use metrics::counter;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Invalidation Events
// ═══════════════════════════════════════════════════════════════════════════════

/// Types of invalidation events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InvalidationEvent {
    /// Invalidate by specific key
    Key {
        key: String,
    },

    /// Invalidate by tag
    Tag {
        tag: String,
    },

    /// Invalidate by pattern (glob-style)
    Pattern {
        pattern: String,
    },

    /// Invalidate all entries
    All,

    /// Invalidate entries for a specific entity
    Entity {
        entity_type: String,
        entity_id: String,
    },

    /// Invalidate entries in a namespace
    Namespace {
        namespace: String,
    },

    /// Cascade invalidation (invalidate related entries)
    Cascade {
        source: Box<InvalidationEvent>,
        related_tags: Vec<String>,
    },
}

impl InvalidationEvent {
    /// Create a key invalidation event.
    pub fn key(key: impl Into<String>) -> Self {
        Self::Key { key: key.into() }
    }

    /// Create a tag invalidation event.
    pub fn tag(tag: impl Into<String>) -> Self {
        Self::Tag { tag: tag.into() }
    }

    /// Create a pattern invalidation event.
    pub fn pattern(pattern: impl Into<String>) -> Self {
        Self::Pattern { pattern: pattern.into() }
    }

    /// Create an entity invalidation event.
    pub fn entity(entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        Self::Entity {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
        }
    }

    /// Create a namespace invalidation event.
    pub fn namespace(namespace: impl Into<String>) -> Self {
        Self::Namespace { namespace: namespace.into() }
    }

    /// Create a cascade invalidation event.
    pub fn cascade(source: InvalidationEvent, related_tags: Vec<String>) -> Self {
        Self::Cascade {
            source: Box::new(source),
            related_tags,
        }
    }

    /// Get the event type name.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Key { .. } => "key",
            Self::Tag { .. } => "tag",
            Self::Pattern { .. } => "pattern",
            Self::All => "all",
            Self::Entity { .. } => "entity",
            Self::Namespace { .. } => "namespace",
            Self::Cascade { .. } => "cascade",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Invalidation Strategy Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for invalidation strategies.
#[async_trait]
pub trait InvalidationStrategy: Send + Sync {
    /// Process an invalidation event.
    async fn invalidate(&self, event: InvalidationEvent) -> Result<u64>;

    /// Get the strategy name.
    fn name(&self) -> &'static str;
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tag-Based Invalidation
// ═══════════════════════════════════════════════════════════════════════════════

/// Tag-based cache invalidation.
///
/// Tags allow grouping cache entries and invalidating them together.
/// For example, all entries related to a project can be tagged with the project ID.
pub struct TagInvalidation {
    backend: Arc<dyn CacheBackend>,
}

impl TagInvalidation {
    /// Create a new tag invalidation strategy.
    pub fn new(backend: Arc<dyn CacheBackend>) -> Self {
        Self { backend }
    }

    /// Invalidate all entries with the given tag.
    pub async fn invalidate_tag(&self, tag: &str) -> Result<u64> {
        let keys = self.backend.get_by_tag(tag).await?;
        let count = keys.len() as u64;

        for key in keys {
            if let Err(e) = self.backend.delete(&key).await {
                warn!("Failed to delete key {} during tag invalidation: {}", key, e);
            }
        }

        counter!("cache_invalidations_total", "strategy" => "tag").increment(count);
        debug!("Invalidated {} entries with tag: {}", count, tag);

        Ok(count)
    }

    /// Invalidate entries with any of the given tags.
    pub async fn invalidate_tags(&self, tags: &[String]) -> Result<u64> {
        let mut total = 0;
        for tag in tags {
            total += self.invalidate_tag(tag).await?;
        }
        Ok(total)
    }
}

#[async_trait]
impl InvalidationStrategy for TagInvalidation {
    async fn invalidate(&self, event: InvalidationEvent) -> Result<u64> {
        match event {
            InvalidationEvent::Tag { tag } => self.invalidate_tag(&tag).await,
            _ => Ok(0),
        }
    }

    fn name(&self) -> &'static str {
        "tag"
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pattern-Based Invalidation
// ═══════════════════════════════════════════════════════════════════════════════

/// Pattern-based cache invalidation using wildcards.
///
/// Supports glob-style patterns:
/// - `*` matches any sequence of characters
/// - `?` matches any single character
pub struct PatternInvalidation {
    backend: Arc<dyn CacheBackend>,
}

impl PatternInvalidation {
    /// Create a new pattern invalidation strategy.
    pub fn new(backend: Arc<dyn CacheBackend>) -> Self {
        Self { backend }
    }

    /// Invalidate all entries matching the pattern.
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<u64> {
        // Convert glob pattern to regex
        let regex_pattern = glob_to_regex(pattern)?;
        let count = self.backend.delete_by_pattern(&regex_pattern).await?;

        counter!("cache_invalidations_total", "strategy" => "pattern").increment(count);
        debug!("Invalidated {} entries matching pattern: {}", count, pattern);

        Ok(count)
    }
}

#[async_trait]
impl InvalidationStrategy for PatternInvalidation {
    async fn invalidate(&self, event: InvalidationEvent) -> Result<u64> {
        match event {
            InvalidationEvent::Pattern { pattern } => self.invalidate_pattern(&pattern).await,
            _ => Ok(0),
        }
    }

    fn name(&self) -> &'static str {
        "pattern"
    }
}

/// Convert a glob pattern to a regex pattern.
fn glob_to_regex(glob: &str) -> Result<String> {
    let mut regex = String::with_capacity(glob.len() * 2);
    regex.push('^');

    for c in glob.chars() {
        match c {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(c);
            }
            _ => regex.push(c),
        }
    }

    regex.push('$');
    Ok(regex)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Event-Driven Invalidation
// ═══════════════════════════════════════════════════════════════════════════════

/// Event-driven cache invalidation with pub/sub support.
///
/// Allows subscribing to invalidation events and triggering invalidation
/// based on domain events (e.g., task updated, agent deleted).
pub struct EventDrivenInvalidation {
    backend: Arc<dyn CacheBackend>,
    event_sender: broadcast::Sender<InvalidationEvent>,
    entity_tag_mappings: DashMap<String, Vec<String>>,
}

impl EventDrivenInvalidation {
    /// Create a new event-driven invalidation strategy.
    pub fn new(backend: Arc<dyn CacheBackend>, channel_capacity: usize) -> Self {
        let (event_sender, _) = broadcast::channel(channel_capacity);
        Self {
            backend,
            event_sender,
            entity_tag_mappings: DashMap::new(),
        }
    }

    /// Register entity-to-tag mappings.
    ///
    /// When an entity is invalidated, all associated tags will also be invalidated.
    pub fn register_entity_tags(&self, entity_type: &str, tags: Vec<String>) {
        self.entity_tag_mappings.insert(entity_type.to_string(), tags);
    }

    /// Subscribe to invalidation events.
    pub fn subscribe(&self) -> broadcast::Receiver<InvalidationEvent> {
        self.event_sender.subscribe()
    }

    /// Publish an invalidation event.
    pub async fn publish(&self, event: InvalidationEvent) -> Result<u64> {
        // Send to subscribers
        let _ = self.event_sender.send(event.clone());

        // Process the event
        self.process_event(event).await
    }

    /// Process an invalidation event.
    fn process_event(&self, event: InvalidationEvent) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + '_>> {
        Box::pin(async move {
            let count = match &event {
                InvalidationEvent::Key { key } => {
                    if self.backend.delete(key).await? {
                        1
                    } else {
                        0
                    }
                }
                InvalidationEvent::Tag { tag } => {
                    let tag_invalidation = TagInvalidation::new(self.backend.clone());
                    tag_invalidation.invalidate_tag(tag).await?
                }
                InvalidationEvent::Pattern { pattern } => {
                    let pattern_invalidation = PatternInvalidation::new(self.backend.clone());
                    pattern_invalidation.invalidate_pattern(pattern).await?
                }
                InvalidationEvent::All => {
                    self.backend.clear().await?;
                    counter!("cache_invalidations_total", "strategy" => "all").increment(1);
                    u64::MAX // Indicate all cleared
                }
                InvalidationEvent::Entity { entity_type, entity_id } => {
                    let mut count = 0;

                    // Invalidate entity-specific key
                    let entity_pattern = format!("{}:{}*", entity_type, entity_id);
                    count += self.backend.delete_by_pattern(&glob_to_regex(&entity_pattern)?).await?;

                    // Invalidate related tags
                    if let Some(tags) = self.entity_tag_mappings.get(entity_type) {
                        let tag_invalidation = TagInvalidation::new(self.backend.clone());
                        for tag in tags.iter() {
                            let specific_tag = format!("{}:{}", tag, entity_id);
                            count += tag_invalidation.invalidate_tag(&specific_tag).await?;
                        }
                    }

                    count
                }
                InvalidationEvent::Namespace { namespace } => {
                    let pattern = format!("{}:*", namespace);
                    self.backend.delete_by_pattern(&glob_to_regex(&pattern)?).await?
                }
                InvalidationEvent::Cascade { source, related_tags } => {
                    // Process source event
                    let mut count = self.process_event(*source.clone()).await?;

                    // Process related tags
                    let tag_invalidation = TagInvalidation::new(self.backend.clone());
                    for tag in related_tags {
                        count += tag_invalidation.invalidate_tag(tag).await?;
                    }

                    count
                }
            };

            counter!("cache_invalidations_total", "strategy" => "event", "type" => event.event_type()).increment(count);

            Ok(count)
        })
    }
}

#[async_trait]
impl InvalidationStrategy for EventDrivenInvalidation {
    async fn invalidate(&self, event: InvalidationEvent) -> Result<u64> {
        self.publish(event).await
    }

    fn name(&self) -> &'static str {
        "event_driven"
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Invalidation Engine
// ═══════════════════════════════════════════════════════════════════════════════

/// Unified invalidation engine combining all strategies.
pub struct InvalidationEngine {
    backend: Arc<dyn CacheBackend>,
    tag_invalidation: TagInvalidation,
    pattern_invalidation: PatternInvalidation,
    event_invalidation: EventDrivenInvalidation,
    invalidation_log: DashMap<String, InvalidationLogEntry>,
}

/// Log entry for invalidation operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationLogEntry {
    /// Event that triggered the invalidation
    pub event: InvalidationEvent,

    /// Number of entries invalidated
    pub count: u64,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Duration of the operation
    pub duration_ms: u64,
}

impl InvalidationEngine {
    /// Create a new invalidation engine.
    pub fn new(backend: Arc<dyn CacheBackend>) -> Self {
        Self {
            tag_invalidation: TagInvalidation::new(backend.clone()),
            pattern_invalidation: PatternInvalidation::new(backend.clone()),
            event_invalidation: EventDrivenInvalidation::new(backend.clone(), 1000),
            backend,
            invalidation_log: DashMap::new(),
        }
    }

    /// Invalidate by tag.
    pub async fn invalidate_by_tag(&self, tag: &str) -> Result<u64> {
        let start = std::time::Instant::now();
        let event = InvalidationEvent::tag(tag);
        let count = self.tag_invalidation.invalidate_tag(tag).await?;

        self.log_invalidation(event, count, start.elapsed());
        Ok(count)
    }

    /// Invalidate by pattern.
    pub async fn invalidate_by_pattern(&self, pattern: &str) -> Result<u64> {
        let start = std::time::Instant::now();
        let event = InvalidationEvent::pattern(pattern);
        let count = self.pattern_invalidation.invalidate_pattern(pattern).await?;

        self.log_invalidation(event, count, start.elapsed());
        Ok(count)
    }

    /// Invalidate by key.
    pub async fn invalidate_key(&self, key: &str) -> Result<bool> {
        let start = std::time::Instant::now();
        let event = InvalidationEvent::key(key);
        let deleted = self.backend.delete(key).await?;

        self.log_invalidation(event, if deleted { 1 } else { 0 }, start.elapsed());
        Ok(deleted)
    }

    /// Invalidate all entries.
    pub async fn invalidate_all(&self) -> Result<()> {
        let start = std::time::Instant::now();
        self.backend.clear().await?;

        self.log_invalidation(InvalidationEvent::All, u64::MAX, start.elapsed());
        info!("Invalidated all cache entries");
        Ok(())
    }

    /// Invalidate by entity.
    pub async fn invalidate_entity(&self, entity_type: &str, entity_id: &str) -> Result<u64> {
        let start = std::time::Instant::now();
        let event = InvalidationEvent::entity(entity_type, entity_id);
        let count = self.event_invalidation.process_event(event.clone()).await?;

        self.log_invalidation(event, count, start.elapsed());
        Ok(count)
    }

    /// Invalidate by namespace.
    pub async fn invalidate_namespace(&self, namespace: &str) -> Result<u64> {
        let start = std::time::Instant::now();
        let event = InvalidationEvent::namespace(namespace);
        let count = self.event_invalidation.process_event(event.clone()).await?;

        self.log_invalidation(event, count, start.elapsed());
        Ok(count)
    }

    /// Process a cascade invalidation.
    pub async fn cascade_invalidate(
        &self,
        source: InvalidationEvent,
        related_tags: Vec<String>,
    ) -> Result<u64> {
        let start = std::time::Instant::now();
        let event = InvalidationEvent::cascade(source, related_tags);
        let count = self.event_invalidation.process_event(event.clone()).await?;

        self.log_invalidation(event, count, start.elapsed());
        Ok(count)
    }

    /// Subscribe to invalidation events.
    pub fn subscribe(&self) -> broadcast::Receiver<InvalidationEvent> {
        self.event_invalidation.subscribe()
    }

    /// Register entity-to-tag mappings.
    pub fn register_entity_tags(&self, entity_type: &str, tags: Vec<String>) {
        self.event_invalidation.register_entity_tags(entity_type, tags);
    }

    /// Get recent invalidation log entries.
    pub fn get_recent_invalidations(&self, limit: usize) -> Vec<InvalidationLogEntry> {
        let mut entries: Vec<_> = self.invalidation_log
            .iter()
            .map(|e| e.value().clone())
            .collect();

        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Log an invalidation operation.
    fn log_invalidation(&self, event: InvalidationEvent, count: u64, duration: Duration) {
        let entry = InvalidationLogEntry {
            event,
            count,
            timestamp: Utc::now(),
            duration_ms: duration.as_millis() as u64,
        };

        // Use timestamp as key for uniqueness
        let key = format!("{}", entry.timestamp.timestamp_nanos_opt().unwrap_or(0));
        self.invalidation_log.insert(key, entry);

        // Keep only last 1000 entries
        if self.invalidation_log.len() > 1000 {
            let oldest_key = self.invalidation_log
                .iter()
                .min_by_key(|e| e.value().timestamp)
                .map(|e| e.key().clone());

            if let Some(key) = oldest_key {
                self.invalidation_log.remove(&key);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Invalidation Batch Operations
// ═══════════════════════════════════════════════════════════════════════════════

/// Batch invalidation builder for efficient bulk operations.
pub struct InvalidationBatch {
    events: Vec<InvalidationEvent>,
}

impl InvalidationBatch {
    /// Create a new invalidation batch.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add a key to invalidate.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.events.push(InvalidationEvent::key(key));
        self
    }

    /// Add a tag to invalidate.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.events.push(InvalidationEvent::tag(tag));
        self
    }

    /// Add a pattern to invalidate.
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.events.push(InvalidationEvent::pattern(pattern));
        self
    }

    /// Add an entity to invalidate.
    pub fn entity(mut self, entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        self.events.push(InvalidationEvent::entity(entity_type, entity_id));
        self
    }

    /// Execute the batch invalidation.
    pub async fn execute(self, engine: &InvalidationEngine) -> Result<u64> {
        let mut total = 0;

        for event in self.events {
            let count = match event {
                InvalidationEvent::Key { key } => {
                    if engine.invalidate_key(&key).await? { 1 } else { 0 }
                }
                InvalidationEvent::Tag { tag } => {
                    engine.invalidate_by_tag(&tag).await?
                }
                InvalidationEvent::Pattern { pattern } => {
                    engine.invalidate_by_pattern(&pattern).await?
                }
                InvalidationEvent::Entity { entity_type, entity_id } => {
                    engine.invalidate_entity(&entity_type, &entity_id).await?
                }
                _ => 0,
            };
            total += count;
        }

        Ok(total)
    }
}

impl Default for InvalidationBatch {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::backend::{InMemoryBackend, InMemoryConfig, CacheEntry};

    async fn create_test_backend() -> Arc<InMemoryBackend> {
        Arc::new(InMemoryBackend::new(InMemoryConfig::default()))
    }

    #[tokio::test]
    async fn test_tag_invalidation() {
        let backend = create_test_backend().await;
        let invalidation = TagInvalidation::new(backend.clone());

        // Add entries with tags
        let entry = CacheEntry {
            data: b"test".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec!["project-123".to_string()],
            created_at: Utc::now(),
        };

        backend.set("task-1", entry.clone()).await.unwrap();
        backend.set("task-2", entry.clone()).await.unwrap();

        // Invalidate by tag
        let count = invalidation.invalidate_tag("project-123").await.unwrap();
        assert_eq!(count, 2);

        // Verify entries are deleted
        assert!(!backend.exists("task-1").await.unwrap());
        assert!(!backend.exists("task-2").await.unwrap());
    }

    #[tokio::test]
    async fn test_pattern_invalidation() {
        let backend = create_test_backend().await;
        let invalidation = PatternInvalidation::new(backend.clone());

        // Add entries
        let entry = CacheEntry {
            data: b"test".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec![],
            created_at: Utc::now(),
        };

        backend.set("task:project-1:task-1", entry.clone()).await.unwrap();
        backend.set("task:project-1:task-2", entry.clone()).await.unwrap();
        backend.set("task:project-2:task-3", entry.clone()).await.unwrap();

        // Invalidate by pattern
        let count = invalidation.invalidate_pattern("task:project-1:*").await.unwrap();
        assert_eq!(count, 2);

        // Verify correct entries are deleted
        assert!(!backend.exists("task:project-1:task-1").await.unwrap());
        assert!(!backend.exists("task:project-1:task-2").await.unwrap());
        assert!(backend.exists("task:project-2:task-3").await.unwrap());
    }

    #[test]
    fn test_glob_to_regex() {
        assert_eq!(glob_to_regex("task:*").unwrap(), "^task:.*$");
        assert_eq!(glob_to_regex("task:?:detail").unwrap(), "^task:.:detail$");
        assert_eq!(glob_to_regex("task.name").unwrap(), "^task\\.name$");
    }

    #[tokio::test]
    async fn test_invalidation_engine() {
        let backend = create_test_backend().await;
        let engine = InvalidationEngine::new(backend.clone());

        // Add entry
        let entry = CacheEntry {
            data: b"test".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec!["tag1".to_string()],
            created_at: Utc::now(),
        };

        backend.set("key1", entry).await.unwrap();

        // Test key invalidation
        let deleted = engine.invalidate_key("key1").await.unwrap();
        assert!(deleted);

        // Check log
        let logs = engine.get_recent_invalidations(10);
        assert!(!logs.is_empty());
    }

    #[tokio::test]
    async fn test_invalidation_batch() {
        let backend = create_test_backend().await;
        let engine = InvalidationEngine::new(backend.clone());

        // Add entries
        let entry = CacheEntry {
            data: b"test".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec!["batch-tag".to_string()],
            created_at: Utc::now(),
        };

        backend.set("batch-key-1", entry.clone()).await.unwrap();
        backend.set("batch-key-2", entry.clone()).await.unwrap();

        // Batch invalidation
        let count = InvalidationBatch::new()
            .key("batch-key-1")
            .tag("batch-tag")
            .execute(&engine)
            .await
            .unwrap();

        assert!(count >= 1);
    }

    #[test]
    fn test_invalidation_event_types() {
        let key_event = InvalidationEvent::key("test-key");
        assert_eq!(key_event.event_type(), "key");

        let tag_event = InvalidationEvent::tag("test-tag");
        assert_eq!(tag_event.event_type(), "tag");

        let entity_event = InvalidationEvent::entity("task", "task-123");
        assert_eq!(entity_event.event_type(), "entity");
    }

    #[test]
    fn test_all_event_types() {
        assert_eq!(InvalidationEvent::key("k").event_type(), "key");
        assert_eq!(InvalidationEvent::tag("t").event_type(), "tag");
        assert_eq!(InvalidationEvent::pattern("p").event_type(), "pattern");
        assert_eq!(InvalidationEvent::entity("e", "id").event_type(), "entity");
        assert_eq!(InvalidationEvent::namespace("ns").event_type(), "namespace");
        assert_eq!(InvalidationEvent::All.event_type(), "all");
        let cascade = InvalidationEvent::cascade(InvalidationEvent::key("k"), vec!["t".into()]);
        assert_eq!(cascade.event_type(), "cascade");
    }

    #[test]
    fn test_glob_to_regex_special_chars() {
        assert_eq!(glob_to_regex("a.b").unwrap(), "^a\\.b$");
        assert_eq!(glob_to_regex("a+b").unwrap(), "^a\\+b$");
        assert_eq!(glob_to_regex("prefix*suffix").unwrap(), "^prefix.*suffix$");
    }

    #[test]
    fn test_glob_to_regex_question_mark() {
        let regex = glob_to_regex("a?c").unwrap();
        assert_eq!(regex, "^a.c$");
        let re = regex::Regex::new(&regex).unwrap();
        assert!(re.is_match("abc"));
        assert!(!re.is_match("abbc"));
    }

    #[tokio::test]
    async fn test_tag_invalidation_empty_tag() {
        let backend = create_test_backend().await;
        let invalidation = TagInvalidation::new(backend.clone());
        let count = invalidation.invalidate_tag("nonexistent").await.unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_strategy_names() {
        let backend_arc: Arc<dyn CacheBackend> = Arc::new(InMemoryBackend::new(InMemoryConfig::default()));
        let tag = TagInvalidation::new(backend_arc.clone());
        let pattern = PatternInvalidation::new(backend_arc.clone());
        let event = EventDrivenInvalidation::new(backend_arc, 100);
        assert_eq!(tag.name(), "tag");
        assert_eq!(pattern.name(), "pattern");
        assert_eq!(event.name(), "event_driven");
    }

    #[tokio::test]
    async fn test_engine_key_not_found() {
        let backend = create_test_backend().await;
        let engine = InvalidationEngine::new(backend.clone());
        let deleted = engine.invalidate_key("nope").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_engine_invalidate_all() {
        let backend = create_test_backend().await;
        let engine = InvalidationEngine::new(backend.clone());
        let entry = CacheEntry {
            data: b"data".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec![],
            created_at: Utc::now(),
        };
        backend.set("a", entry.clone()).await.unwrap();
        backend.set("b", entry).await.unwrap();
        engine.invalidate_all().await.unwrap();
        assert!(!backend.exists("a").await.unwrap());
        assert!(!backend.exists("b").await.unwrap());
    }

    #[tokio::test]
    async fn test_engine_logging() {
        let backend = create_test_backend().await;
        let engine = InvalidationEngine::new(backend.clone());
        let entry = CacheEntry {
            data: b"x".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec!["t".into()],
            created_at: Utc::now(),
        };
        backend.set("k1", entry).await.unwrap();
        engine.invalidate_key("k1").await.unwrap();
        let logs = engine.get_recent_invalidations(5);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].count, 1);
    }

    #[test]
    fn test_batch_default() {
        let batch = InvalidationBatch::default();
        assert!(batch.events.is_empty());
    }

    #[test]
    fn test_batch_builder() {
        let batch = InvalidationBatch::new()
            .key("k1")
            .tag("t1")
            .pattern("p*")
            .entity("task", "123");
        assert_eq!(batch.events.len(), 4);
    }

    #[tokio::test]
    async fn test_event_driven_subscribe() {
        let backend: Arc<dyn CacheBackend> = Arc::new(InMemoryBackend::new(InMemoryConfig::default()));
        let event_inv = EventDrivenInvalidation::new(backend, 16);
        let mut rx = event_inv.subscribe();
        event_inv.publish(InvalidationEvent::key("sub-test")).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type(), "key");
    }

    #[tokio::test]
    async fn test_engine_by_pattern() {
        let backend = create_test_backend().await;
        let engine = InvalidationEngine::new(backend.clone());
        let entry = CacheEntry {
            data: b"d".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec![],
            created_at: Utc::now(),
        };
        backend.set("pat:a", entry.clone()).await.unwrap();
        backend.set("pat:b", entry.clone()).await.unwrap();
        backend.set("other", entry).await.unwrap();
        let count = engine.invalidate_by_pattern("pat:*").await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_engine_by_tag() {
        let backend = create_test_backend().await;
        let engine = InvalidationEngine::new(backend.clone());
        let entry = CacheEntry {
            data: b"d".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec!["my-tag".into()],
            created_at: Utc::now(),
        };
        backend.set("tagged-1", entry.clone()).await.unwrap();
        backend.set("tagged-2", entry).await.unwrap();
        let count = engine.invalidate_by_tag("my-tag").await.unwrap();
        assert_eq!(count, 2);
    }
}
