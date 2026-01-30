//! Comprehensive caching layer for Apex Core.
//!
//! This module provides a multi-tier caching system with:
//!
//! - **Backend Abstraction**: Pluggable backends (in-memory, Redis, multi-tier)
//! - **Type-safe Keys**: Strongly-typed cache keys with namespacing and TTL
//! - **Invalidation Strategies**: Tag-based, pattern-based, and event-driven invalidation
//! - **HTTP Middleware**: ETag support, Cache-Control headers, conditional requests
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                         Cache Layer                                  │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────┐ │
//! │  │   CacheKey  │    │  Invalidation│    │   HTTP Middleware       │ │
//! │  │  Generation │    │    Engine    │    │  (ETag, Cache-Control)  │ │
//! │  └──────┬──────┘    └──────┬───────┘    └───────────┬─────────────┘ │
//! │         │                  │                        │               │
//! │         ▼                  ▼                        ▼               │
//! │  ┌─────────────────────────────────────────────────────────────────┐│
//! │  │                    CacheBackend Trait                          ││
//! │  └──────────────────────────┬──────────────────────────────────────┘│
//! │                             │                                       │
//! │         ┌───────────────────┼───────────────────────┐              │
//! │         ▼                   ▼                       ▼              │
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────┐ │
//! │  │  In-Memory  │    │    Redis    │    │     Multi-Tier          │ │
//! │  │   (Moka)    │    │   Backend   │    │  (L1 Memory + L2 Redis) │ │
//! │  └─────────────┘    └─────────────┘    └─────────────────────────┘ │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use apex_core::cache::{
//!     Cache, CacheConfig, CacheKey, KeyType,
//!     InMemoryBackend, RedisBackend, MultiTierBackend,
//!     InvalidationEvent, CacheMiddleware,
//! };
//!
//! // Create a multi-tier cache
//! let cache = Cache::multi_tier(
//!     InMemoryBackend::new(10_000),
//!     RedisBackend::new("redis://localhost:6379").await?,
//! );
//!
//! // Type-safe cache keys
//! let key = CacheKey::new(KeyType::Task)
//!     .with_id("task-123")
//!     .with_namespace("project-456");
//!
//! // Set and get values
//! cache.set(&key, &my_data).await?;
//! let data: Option<MyData> = cache.get(&key).await?;
//!
//! // Tag-based invalidation
//! cache.invalidate_by_tag("project-456").await?;
//!
//! // HTTP caching middleware
//! let app = Router::new()
//!     .route("/api/v1/tasks/:id", get(get_task))
//!     .layer(CacheMiddlewareLayer::new(cache.clone()));
//! ```

pub mod backend;
pub mod key;
pub mod invalidation;
pub mod middleware;

pub use backend::{
    CacheBackend, CacheEntry, CacheStats,
    InMemoryBackend, InMemoryConfig,
    RedisBackend, RedisConfig,
    MultiTierBackend, MultiTierConfig,
};
pub use key::{CacheKey, KeyType, KeyBuilder};
pub use invalidation::{
    InvalidationEngine, InvalidationEvent, InvalidationStrategy,
    TagInvalidation, PatternInvalidation, EventDrivenInvalidation,
};
pub use middleware::{
    CacheMiddlewareLayer, CacheMiddleware, CacheMiddlewareConfig,
    ETagGenerator, CacheControl, CacheDirective,
};

use crate::error::{ApexError, ErrorCode, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, instrument};

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Main cache configuration.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Default TTL for cache entries
    pub default_ttl: Duration,

    /// Maximum entry size in bytes
    pub max_entry_size: usize,

    /// Enable cache metrics
    pub enable_metrics: bool,

    /// Cache namespace prefix
    pub namespace_prefix: String,

    /// Enable compression for large values
    pub enable_compression: bool,

    /// Compression threshold in bytes
    pub compression_threshold: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300), // 5 minutes
            max_entry_size: 1024 * 1024, // 1 MB
            enable_metrics: true,
            namespace_prefix: "apex:cache:".to_string(),
            enable_compression: true,
            compression_threshold: 1024, // 1 KB
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration builder.
    pub fn builder() -> CacheConfigBuilder {
        CacheConfigBuilder::default()
    }
}

/// Builder for cache configuration.
#[derive(Debug, Default)]
pub struct CacheConfigBuilder {
    config: CacheConfig,
}

impl CacheConfigBuilder {
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = ttl;
        self
    }

    pub fn max_entry_size(mut self, size: usize) -> Self {
        self.config.max_entry_size = size;
        self
    }

    pub fn enable_metrics(mut self, enabled: bool) -> Self {
        self.config.enable_metrics = enabled;
        self
    }

    pub fn namespace_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.namespace_prefix = prefix.into();
        self
    }

    pub fn enable_compression(mut self, enabled: bool) -> Self {
        self.config.enable_compression = enabled;
        self
    }

    pub fn compression_threshold(mut self, threshold: usize) -> Self {
        self.config.compression_threshold = threshold;
        self
    }

    pub fn build(self) -> CacheConfig {
        self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main Cache Interface
// ═══════════════════════════════════════════════════════════════════════════════

/// Main cache interface providing a unified API over different backends.
pub struct Cache {
    backend: Arc<dyn CacheBackend>,
    config: CacheConfig,
    invalidation: Arc<InvalidationEngine>,
}

impl Cache {
    /// Create a new cache with the given backend.
    pub fn new(backend: Arc<dyn CacheBackend>, config: CacheConfig) -> Self {
        let invalidation = Arc::new(InvalidationEngine::new(backend.clone()));
        Self {
            backend,
            config,
            invalidation,
        }
    }

    /// Create an in-memory cache.
    pub fn in_memory(max_capacity: u64) -> Self {
        let backend = Arc::new(InMemoryBackend::new(InMemoryConfig {
            max_capacity,
            ..Default::default()
        }));
        Self::new(backend, CacheConfig::default())
    }

    /// Create a Redis-backed cache.
    pub async fn redis(url: &str) -> Result<Self> {
        let backend = Arc::new(RedisBackend::new(RedisConfig {
            url: url.to_string(),
            ..Default::default()
        }).await?);
        Ok(Self::new(backend, CacheConfig::default()))
    }

    /// Create a multi-tier cache (L1 memory + L2 Redis).
    pub async fn multi_tier(
        l1_capacity: u64,
        redis_url: &str,
    ) -> Result<Self> {
        let l1 = InMemoryBackend::new(InMemoryConfig {
            max_capacity: l1_capacity,
            ..Default::default()
        });
        let l2 = RedisBackend::new(RedisConfig {
            url: redis_url.to_string(),
            ..Default::default()
        }).await?;
        let backend = Arc::new(MultiTierBackend::new(
            Arc::new(l1),
            Arc::new(l2),
            MultiTierConfig::default(),
        ));
        Ok(Self::new(backend, CacheConfig::default()))
    }

    /// Get a value from the cache.
    #[instrument(skip(self), fields(key = %key))]
    pub async fn get<T: DeserializeOwned>(&self, key: &CacheKey) -> Result<Option<T>> {
        let full_key = self.build_key(key);
        match self.backend.get(&full_key).await? {
            Some(entry) => {
                let value: T = serde_json::from_slice(&entry.data)
                    .map_err(|e| ApexError::with_internal(
                        ErrorCode::DeserializationError,
                        "Failed to deserialize cached value",
                        e.to_string(),
                    ))?;
                debug!("Cache hit for key: {}", full_key);
                Ok(Some(value))
            }
            None => {
                debug!("Cache miss for key: {}", full_key);
                Ok(None)
            }
        }
    }

    /// Set a value in the cache with the key's default TTL.
    #[instrument(skip(self, value), fields(key = %key))]
    pub async fn set<T: Serialize>(&self, key: &CacheKey, value: &T) -> Result<()> {
        let ttl = key.ttl().unwrap_or(self.config.default_ttl);
        self.set_with_ttl(key, value, ttl).await
    }

    /// Set a value in the cache with a specific TTL.
    #[instrument(skip(self, value), fields(key = %key, ttl_secs = ttl.as_secs()))]
    pub async fn set_with_ttl<T: Serialize>(
        &self,
        key: &CacheKey,
        value: &T,
        ttl: Duration,
    ) -> Result<()> {
        let data = serde_json::to_vec(value)
            .map_err(|e| ApexError::with_internal(
                ErrorCode::SerializationError,
                "Failed to serialize value for cache",
                e.to_string(),
            ))?;

        if data.len() > self.config.max_entry_size {
            return Err(ApexError::new(
                ErrorCode::ValidationError,
                format!("Cache entry size {} exceeds maximum {}", data.len(), self.config.max_entry_size),
            ));
        }

        let full_key = self.build_key(key);
        let entry = CacheEntry {
            data,
            ttl: Some(ttl),
            tags: key.tags().to_vec(),
            created_at: chrono::Utc::now(),
        };

        self.backend.set(&full_key, entry).await?;
        debug!("Cache set for key: {} with TTL: {:?}", full_key, ttl);
        Ok(())
    }

    /// Delete a value from the cache.
    #[instrument(skip(self), fields(key = %key))]
    pub async fn delete(&self, key: &CacheKey) -> Result<bool> {
        let full_key = self.build_key(key);
        let deleted = self.backend.delete(&full_key).await?;
        debug!("Cache delete for key: {} - deleted: {}", full_key, deleted);
        Ok(deleted)
    }

    /// Check if a key exists in the cache.
    #[instrument(skip(self), fields(key = %key))]
    pub async fn exists(&self, key: &CacheKey) -> Result<bool> {
        let full_key = self.build_key(key);
        self.backend.exists(&full_key).await
    }

    /// Get or set a value using a factory function.
    #[instrument(skip(self, factory), fields(key = %key))]
    pub async fn get_or_set<T, F, Fut>(
        &self,
        key: &CacheKey,
        factory: F,
    ) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        let value = factory().await?;
        self.set(key, &value).await?;
        Ok(value)
    }

    /// Invalidate entries by tag.
    #[instrument(skip(self))]
    pub async fn invalidate_by_tag(&self, tag: &str) -> Result<u64> {
        self.invalidation.invalidate_by_tag(tag).await
    }

    /// Invalidate entries by pattern.
    #[instrument(skip(self))]
    pub async fn invalidate_by_pattern(&self, pattern: &str) -> Result<u64> {
        self.invalidation.invalidate_by_pattern(pattern).await
    }

    /// Get cache statistics.
    pub async fn stats(&self) -> Result<CacheStats> {
        self.backend.stats().await
    }

    /// Clear all cache entries.
    #[instrument(skip(self))]
    pub async fn clear(&self) -> Result<()> {
        info!("Clearing all cache entries");
        self.backend.clear().await
    }

    /// Get the invalidation engine for advanced invalidation operations.
    pub fn invalidation_engine(&self) -> Arc<InvalidationEngine> {
        self.invalidation.clone()
    }

    /// Build the full cache key with namespace prefix.
    fn build_key(&self, key: &CacheKey) -> String {
        format!("{}{}", self.config.namespace_prefix, key)
    }
}

impl Clone for Cache {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            config: self.config.clone(),
            invalidation: self.invalidation.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestData {
        id: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_in_memory_cache_basic() {
        let cache = Cache::in_memory(1000);

        let key = CacheKey::new(KeyType::Task).with_id("test-1");
        let data = TestData {
            id: "test-1".to_string(),
            value: 42,
        };

        // Set value
        cache.set(&key, &data).await.unwrap();

        // Get value
        let retrieved: Option<TestData> = cache.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(data));

        // Delete value
        let deleted = cache.delete(&key).await.unwrap();
        assert!(deleted);

        // Verify deleted
        let retrieved: Option<TestData> = cache.get(&key).await.unwrap();
        assert_eq!(retrieved, None);
    }

    #[tokio::test]
    async fn test_get_or_set() {
        let cache = Cache::in_memory(1000);
        let key = CacheKey::new(KeyType::Agent).with_id("agent-1");

        let call_count = std::sync::atomic::AtomicU32::new(0);

        // First call should invoke factory
        let value1: TestData = cache
            .get_or_set(&key, || async {
                call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(TestData {
                    id: "agent-1".to_string(),
                    value: 100,
                })
            })
            .await
            .unwrap();

        assert_eq!(value1.value, 100);
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Second call should use cached value
        let value2: TestData = cache
            .get_or_set(&key, || async {
                call_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(TestData {
                    id: "agent-1".to_string(),
                    value: 200,
                })
            })
            .await
            .unwrap();

        assert_eq!(value2.value, 100); // Still the cached value
        assert_eq!(call_count.load(std::sync::atomic::Ordering::SeqCst), 1); // Factory not called again
    }

    #[test]
    fn test_config_builder() {
        let config = CacheConfig::builder()
            .default_ttl(Duration::from_secs(600))
            .max_entry_size(2 * 1024 * 1024)
            .enable_compression(false)
            .build();

        assert_eq!(config.default_ttl, Duration::from_secs(600));
        assert_eq!(config.max_entry_size, 2 * 1024 * 1024);
        assert!(!config.enable_compression);
    }

    #[test]
    fn test_config_default_values() {
        let config = CacheConfig::default();
        assert_eq!(config.default_ttl, Duration::from_secs(300));
        assert_eq!(config.max_entry_size, 1024 * 1024);
        assert!(config.enable_metrics);
        assert_eq!(config.namespace_prefix, "apex:cache:");
        assert!(config.enable_compression);
        assert_eq!(config.compression_threshold, 1024);
    }

    #[test]
    fn test_config_builder_namespace_prefix() {
        let config = CacheConfig::builder()
            .namespace_prefix("custom:prefix:")
            .build();
        assert_eq!(config.namespace_prefix, "custom:prefix:");
    }

    #[test]
    fn test_config_builder_compression_threshold() {
        let config = CacheConfig::builder()
            .compression_threshold(4096)
            .build();
        assert_eq!(config.compression_threshold, 4096);
    }

    #[test]
    fn test_config_builder_enable_metrics() {
        let config = CacheConfig::builder()
            .enable_metrics(false)
            .build();
        assert!(!config.enable_metrics);
    }

    #[test]
    fn test_build_key_with_namespace_prefix() {
        let config = CacheConfig::builder()
            .namespace_prefix("test:")
            .build();
        let cache = Cache::new(
            Arc::new(InMemoryBackend::new(InMemoryConfig {
                max_capacity: 100,
                ..Default::default()
            })),
            config,
        );
        let key = CacheKey::new(KeyType::Task).with_id("abc");
        let full_key = cache.build_key(&key);
        assert!(full_key.starts_with("test:"));
        assert!(full_key.contains("task"));
    }

    #[tokio::test]
    async fn test_cache_exists() {
        let cache = Cache::in_memory(1000);
        let key = CacheKey::new(KeyType::Task).with_id("exists-test");
        let data = TestData {
            id: "exists-test".to_string(),
            value: 1,
        };

        assert!(!cache.exists(&key).await.unwrap());
        cache.set(&key, &data).await.unwrap();
        assert!(cache.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = Cache::in_memory(1000);
        let key = CacheKey::new(KeyType::Task).with_id("clear-test");
        let data = TestData {
            id: "clear-test".to_string(),
            value: 99,
        };

        cache.set(&key, &data).await.unwrap();
        assert!(cache.exists(&key).await.unwrap());

        cache.clear().await.unwrap();
        assert!(!cache.exists(&key).await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_set_with_custom_ttl() {
        let cache = Cache::in_memory(1000);
        let key = CacheKey::new(KeyType::Task).with_id("ttl-test");
        let data = TestData {
            id: "ttl-test".to_string(),
            value: 7,
        };

        cache
            .set_with_ttl(&key, &data, Duration::from_secs(60))
            .await
            .unwrap();
        let retrieved: Option<TestData> = cache.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(data));
    }

    #[tokio::test]
    async fn test_cache_delete_nonexistent_key() {
        let cache = Cache::in_memory(1000);
        let key = CacheKey::new(KeyType::Task).with_id("nonexistent");
        let deleted = cache.delete(&key).await.unwrap();
        assert!(!deleted);
    }

    #[test]
    fn test_cache_clone() {
        let cache = Cache::in_memory(1000);
        let cloned = cache.clone();
        // Cloned cache should share the same backend (Arc)
        assert_eq!(
            cache.config.namespace_prefix,
            cloned.config.namespace_prefix
        );
    }
}
