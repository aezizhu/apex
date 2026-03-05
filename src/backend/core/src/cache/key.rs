//! Type-safe cache key generation.
//!
//! This module provides:
//! - Type-safe cache keys with compile-time validation
//! - Key namespacing for multi-tenant environments
//! - TTL configuration per key type
//! - Key versioning for cache invalidation

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════════
// Key Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Enumeration of cache key types with associated default TTLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyType {
    /// Task cache (short TTL, frequently updated)
    Task,

    /// Agent cache (medium TTL)
    Agent,

    /// DAG cache (longer TTL, less frequently modified)
    Dag,

    /// Contract cache (medium TTL)
    Contract,

    /// User cache
    User,

    /// Session cache (short TTL)
    Session,

    /// API response cache
    ApiResponse,

    /// Configuration cache (long TTL)
    Config,

    /// Metrics/Stats cache (very short TTL)
    Metrics,

    /// Model routing cache
    Routing,

    /// Tool execution results cache
    ToolResult,

    /// Rate limit state
    RateLimit,

    /// Custom key type
    Custom,
}

impl KeyType {
    /// Get the default TTL for this key type.
    pub fn default_ttl(&self) -> Duration {
        match self {
            Self::Task => Duration::from_secs(60),        // 1 minute
            Self::Agent => Duration::from_secs(300),     // 5 minutes
            Self::Dag => Duration::from_secs(600),       // 10 minutes
            Self::Contract => Duration::from_secs(300),  // 5 minutes
            Self::User => Duration::from_secs(900),      // 15 minutes
            Self::Session => Duration::from_secs(3600),  // 1 hour
            Self::ApiResponse => Duration::from_secs(60), // 1 minute
            Self::Config => Duration::from_secs(3600),   // 1 hour
            Self::Metrics => Duration::from_secs(10),    // 10 seconds
            Self::Routing => Duration::from_secs(300),   // 5 minutes
            Self::ToolResult => Duration::from_secs(600), // 10 minutes
            Self::RateLimit => Duration::from_secs(60),  // 1 minute
            Self::Custom => Duration::from_secs(300),    // 5 minutes
        }
    }

    /// Get the key type prefix for namespacing.
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Agent => "agent",
            Self::Dag => "dag",
            Self::Contract => "contract",
            Self::User => "user",
            Self::Session => "session",
            Self::ApiResponse => "api",
            Self::Config => "config",
            Self::Metrics => "metrics",
            Self::Routing => "routing",
            Self::ToolResult => "tool",
            Self::RateLimit => "rate",
            Self::Custom => "custom",
        }
    }
}

impl fmt::Display for KeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.prefix())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Key
// ═══════════════════════════════════════════════════════════════════════════════

/// A type-safe cache key with namespace support.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    /// Key type
    key_type: KeyType,

    /// Primary identifier
    id: Option<String>,

    /// Namespace (e.g., tenant ID, project ID)
    namespace: Option<String>,

    /// Additional key segments
    segments: Vec<String>,

    /// Tags for invalidation
    tags: Vec<String>,

    /// Custom TTL override
    ttl: Option<Duration>,

    /// Key version for cache busting
    version: Option<u32>,
}

impl CacheKey {
    /// Create a new cache key with the given type.
    pub fn new(key_type: KeyType) -> Self {
        Self {
            key_type,
            id: None,
            namespace: None,
            segments: Vec::new(),
            tags: Vec::new(),
            ttl: None,
            version: None,
        }
    }

    /// Set the primary ID.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the namespace.
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Add a key segment.
    pub fn with_segment(mut self, segment: impl Into<String>) -> Self {
        self.segments.push(segment.into());
        self
    }

    /// Add multiple key segments.
    pub fn with_segments(mut self, segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.segments.extend(segments.into_iter().map(|s| s.into()));
        self
    }

    /// Add a tag for invalidation.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(|t| t.into()));
        self
    }

    /// Set custom TTL override.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set key version.
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = Some(version);
        self
    }

    /// Get the key type.
    pub fn key_type(&self) -> KeyType {
        self.key_type
    }

    /// Get the ID.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Get the namespace.
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// Get the tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Get the TTL (custom or default for key type).
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl.or_else(|| Some(self.key_type.default_ttl()))
    }

    /// Build the cache key string.
    pub fn build(&self) -> String {
        let mut parts = Vec::new();

        // Add namespace if present
        if let Some(ref ns) = self.namespace {
            parts.push(ns.clone());
        }

        // Add key type prefix
        parts.push(self.key_type.prefix().to_string());

        // Add version if present
        if let Some(version) = self.version {
            parts.push(format!("v{}", version));
        }

        // Add ID if present
        if let Some(ref id) = self.id {
            parts.push(id.clone());
        }

        // Add additional segments
        for segment in &self.segments {
            parts.push(segment.clone());
        }

        parts.join(":")
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.build())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Key Builder
// ═══════════════════════════════════════════════════════════════════════════════

/// Builder for creating cache keys with fluent API.
pub struct KeyBuilder {
    key: CacheKey,
}

impl KeyBuilder {
    /// Create a new key builder.
    pub fn new(key_type: KeyType) -> Self {
        Self {
            key: CacheKey::new(key_type),
        }
    }

    /// Create a task key builder.
    pub fn task() -> Self {
        Self::new(KeyType::Task)
    }

    /// Create an agent key builder.
    pub fn agent() -> Self {
        Self::new(KeyType::Agent)
    }

    /// Create a DAG key builder.
    pub fn dag() -> Self {
        Self::new(KeyType::Dag)
    }

    /// Create a contract key builder.
    pub fn contract() -> Self {
        Self::new(KeyType::Contract)
    }

    /// Create a user key builder.
    pub fn user() -> Self {
        Self::new(KeyType::User)
    }

    /// Create a session key builder.
    pub fn session() -> Self {
        Self::new(KeyType::Session)
    }

    /// Create an API response key builder.
    pub fn api_response() -> Self {
        Self::new(KeyType::ApiResponse)
    }

    /// Create a config key builder.
    pub fn config() -> Self {
        Self::new(KeyType::Config)
    }

    /// Create a metrics key builder.
    pub fn metrics() -> Self {
        Self::new(KeyType::Metrics)
    }

    /// Set the primary ID.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.key = self.key.with_id(id);
        self
    }

    /// Set the namespace.
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.key = self.key.with_namespace(namespace);
        self
    }

    /// Add a key segment.
    pub fn segment(mut self, segment: impl Into<String>) -> Self {
        self.key = self.key.with_segment(segment);
        self
    }

    /// Add a tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.key = self.key.with_tag(tag);
        self
    }

    /// Set custom TTL.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.key = self.key.with_ttl(ttl);
        self
    }

    /// Set version.
    pub fn version(mut self, version: u32) -> Self {
        self.key = self.key.with_version(version);
        self
    }

    /// Build the cache key.
    pub fn build(self) -> CacheKey {
        self.key
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Convenience Constructors
// ═══════════════════════════════════════════════════════════════════════════════

impl CacheKey {
    /// Create a task cache key.
    pub fn task(task_id: impl Into<String>) -> Self {
        Self::new(KeyType::Task)
            .with_id(task_id)
            .with_tag("tasks")
    }

    /// Create a task cache key with project namespace.
    pub fn task_in_project(task_id: impl Into<String>, project_id: impl Into<String>) -> Self {
        let project = project_id.into();
        Self::new(KeyType::Task)
            .with_namespace(&project)
            .with_id(task_id)
            .with_tag("tasks")
            .with_tag(format!("project:{}", project))
    }

    /// Create an agent cache key.
    pub fn agent(agent_id: impl Into<String>) -> Self {
        Self::new(KeyType::Agent)
            .with_id(agent_id)
            .with_tag("agents")
    }

    /// Create a DAG cache key.
    pub fn dag(dag_id: impl Into<String>) -> Self {
        Self::new(KeyType::Dag)
            .with_id(dag_id)
            .with_tag("dags")
    }

    /// Create a contract cache key.
    pub fn contract(contract_id: impl Into<String>) -> Self {
        Self::new(KeyType::Contract)
            .with_id(contract_id)
            .with_tag("contracts")
    }

    /// Create a user cache key.
    pub fn user(user_id: impl Into<String>) -> Self {
        Self::new(KeyType::User)
            .with_id(user_id)
            .with_tag("users")
    }

    /// Create a session cache key.
    pub fn session(session_id: impl Into<String>) -> Self {
        Self::new(KeyType::Session)
            .with_id(session_id)
            .with_tag("sessions")
    }

    /// Create an API response cache key.
    pub fn api_response(endpoint: impl Into<String>, params_hash: impl Into<String>) -> Self {
        Self::new(KeyType::ApiResponse)
            .with_segment(endpoint)
            .with_id(params_hash)
            .with_tag("api_responses")
    }

    /// Create a config cache key.
    pub fn config(config_key: impl Into<String>) -> Self {
        Self::new(KeyType::Config)
            .with_id(config_key)
            .with_tag("config")
    }

    /// Create a metrics cache key.
    pub fn metrics(metric_name: impl Into<String>) -> Self {
        Self::new(KeyType::Metrics)
            .with_id(metric_name)
    }

    /// Create a routing decision cache key.
    pub fn routing(model: impl Into<String>, context_hash: impl Into<String>) -> Self {
        Self::new(KeyType::Routing)
            .with_segment(model)
            .with_id(context_hash)
            .with_tag("routing")
    }

    /// Create a tool result cache key.
    pub fn tool_result(tool_name: impl Into<String>, input_hash: impl Into<String>) -> Self {
        Self::new(KeyType::ToolResult)
            .with_segment(tool_name)
            .with_id(input_hash)
            .with_tag("tool_results")
    }

    /// Create a rate limit state key.
    pub fn rate_limit(client_id: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self::new(KeyType::RateLimit)
            .with_segment(endpoint)
            .with_id(client_id)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Key Hashing Utilities
// ═══════════════════════════════════════════════════════════════════════════════

/// Hash a value for use in cache keys.
pub fn hash_for_key<T: std::hash::Hash>(value: &T) -> String {
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Hash multiple values for use in cache keys.
pub fn hash_composite_key<I, T>(values: I) -> String
where
    I: IntoIterator<Item = T>,
    T: std::hash::Hash,
{
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
    for value in values {
        value.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

/// Create a cache key from request parameters.
pub fn key_from_request_params(
    endpoint: &str,
    method: &str,
    params: &impl Serialize,
) -> CacheKey {
    let params_json = serde_json::to_string(params).unwrap_or_default();
    let hash = hash_composite_key([endpoint, method, &params_json]);

    CacheKey::api_response(endpoint, hash)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_key() {
        let key = CacheKey::new(KeyType::Task).with_id("abc123");
        assert_eq!(key.build(), "task:abc123");
    }

    #[test]
    fn test_namespaced_key() {
        let key = CacheKey::new(KeyType::Task)
            .with_namespace("project-456")
            .with_id("task-789");
        assert_eq!(key.build(), "project-456:task:task-789");
    }

    #[test]
    fn test_versioned_key() {
        let key = CacheKey::new(KeyType::Config)
            .with_id("settings")
            .with_version(2);
        assert_eq!(key.build(), "config:v2:settings");
    }

    #[test]
    fn test_multi_segment_key() {
        let key = CacheKey::new(KeyType::ApiResponse)
            .with_segment("users")
            .with_segment("profile")
            .with_id("hash123");
        assert_eq!(key.build(), "api:hash123:users:profile");
    }

    #[test]
    fn test_key_with_tags() {
        let key = CacheKey::new(KeyType::Task)
            .with_id("task-1")
            .with_tag("project-abc")
            .with_tag("urgent");

        assert_eq!(key.tags().len(), 2);
        assert!(key.tags().contains(&"project-abc".to_string()));
        assert!(key.tags().contains(&"urgent".to_string()));
    }

    #[test]
    fn test_key_builder() {
        let key = KeyBuilder::task()
            .namespace("tenant-123")
            .id("task-456")
            .tag("high-priority")
            .version(1)
            .build();

        assert_eq!(key.build(), "tenant-123:task:v1:task-456");
        assert!(key.tags().contains(&"high-priority".to_string()));
    }

    #[test]
    fn test_convenience_constructors() {
        let task_key = CacheKey::task("task-1");
        assert_eq!(task_key.build(), "task:task-1");
        assert!(task_key.tags().contains(&"tasks".to_string()));

        let project_task_key = CacheKey::task_in_project("task-2", "project-abc");
        assert_eq!(project_task_key.build(), "project-abc:task:task-2");
        assert!(project_task_key.tags().contains(&"project:project-abc".to_string()));
    }

    #[test]
    fn test_key_ttl() {
        let key = CacheKey::new(KeyType::Metrics);
        assert_eq!(key.ttl(), Some(Duration::from_secs(10)));

        let key_custom = CacheKey::new(KeyType::Metrics)
            .with_ttl(Duration::from_secs(30));
        assert_eq!(key_custom.ttl(), Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_hash_for_key() {
        let hash1 = hash_for_key(&"test value");
        let hash2 = hash_for_key(&"test value");
        let hash3 = hash_for_key(&"different value");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16); // Hex representation of u64
    }

    #[test]
    fn test_hash_composite_key() {
        let hash = hash_composite_key(["endpoint", "GET", "params"]);
        assert_eq!(hash.len(), 16);
    }

    #[test]
    fn test_key_display() {
        let key = CacheKey::agent("agent-1");
        let display = format!("{}", key);
        assert_eq!(display, "agent:agent-1");
    }

    #[test]
    fn test_key_type_default_ttl() {
        assert_eq!(KeyType::Task.default_ttl(), Duration::from_secs(60));
        assert_eq!(KeyType::Session.default_ttl(), Duration::from_secs(3600));
        assert_eq!(KeyType::Metrics.default_ttl(), Duration::from_secs(10));
    }
}
