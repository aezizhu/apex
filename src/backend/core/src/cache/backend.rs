//! Cache backend implementations.
//!
//! This module provides pluggable cache backends:
//! - **InMemoryBackend**: High-performance in-memory cache using Moka-like eviction
//! - **RedisBackend**: Distributed cache using Redis
//! - **MultiTierBackend**: L1 (memory) + L2 (Redis) multi-tier caching

use crate::error::{ApexError, ErrorCode, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use metrics::{counter, gauge, histogram};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Entry
// ═══════════════════════════════════════════════════════════════════════════════

/// A cached entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Serialized data
    pub data: Vec<u8>,

    /// Time-to-live
    #[serde(with = "duration_serde")]
    pub ttl: Option<Duration>,

    /// Tags for invalidation
    pub tags: Vec<String>,

    /// When this entry was created
    pub created_at: DateTime<Utc>,
}

impl CacheEntry {
    /// Check if the entry has expired.
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            let elapsed = Utc::now()
                .signed_duration_since(self.created_at)
                .to_std()
                .unwrap_or(Duration::MAX);
            elapsed >= ttl
        } else {
            false
        }
    }

    /// Get the remaining TTL.
    pub fn remaining_ttl(&self) -> Option<Duration> {
        self.ttl.and_then(|ttl| {
            let elapsed = Utc::now()
                .signed_duration_since(self.created_at)
                .to_std()
                .ok()?;
            ttl.checked_sub(elapsed)
        })
    }
}

mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => d.as_secs().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs: Option<u64> = Option::deserialize(deserializer)?;
        Ok(secs.map(Duration::from_secs))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Statistics
// ═══════════════════════════════════════════════════════════════════════════════

/// Cache statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total number of cache hits
    pub hits: u64,

    /// Total number of cache misses
    pub misses: u64,

    /// Total number of entries
    pub entries: u64,

    /// Total size in bytes
    pub size_bytes: u64,

    /// Eviction count
    pub evictions: u64,

    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f64,

    /// Average entry size in bytes
    pub avg_entry_size: f64,

    /// Backend-specific stats
    pub backend_stats: HashMap<String, String>,
}

impl CacheStats {
    /// Calculate the hit rate.
    pub fn calculate_hit_rate(&mut self) {
        let total = self.hits + self.misses;
        self.hit_rate = if total > 0 {
            self.hits as f64 / total as f64
        } else {
            0.0
        };
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cache Backend Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for cache backends.
#[async_trait]
pub trait CacheBackend: Send + Sync {
    /// Get a value from the cache.
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>>;

    /// Set a value in the cache.
    async fn set(&self, key: &str, entry: CacheEntry) -> Result<()>;

    /// Delete a value from the cache.
    async fn delete(&self, key: &str) -> Result<bool>;

    /// Check if a key exists.
    async fn exists(&self, key: &str) -> Result<bool>;

    /// Get cache statistics.
    async fn stats(&self) -> Result<CacheStats>;

    /// Clear all entries.
    async fn clear(&self) -> Result<()>;

    /// Get entries by tag.
    async fn get_by_tag(&self, tag: &str) -> Result<Vec<String>>;

    /// Delete entries by pattern.
    async fn delete_by_pattern(&self, pattern: &str) -> Result<u64>;

    /// Get the backend name.
    fn name(&self) -> &'static str;
}

// ═══════════════════════════════════════════════════════════════════════════════
// In-Memory Backend
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for in-memory cache.
#[derive(Debug, Clone)]
pub struct InMemoryConfig {
    /// Maximum number of entries
    pub max_capacity: u64,

    /// Time-to-idle: evict entries not accessed within this duration
    pub time_to_idle: Option<Duration>,

    /// Enable LRU eviction
    pub enable_lru: bool,

    /// Shard count for concurrent access (power of 2)
    pub shard_count: usize,
}

impl Default for InMemoryConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10_000,
            time_to_idle: Some(Duration::from_secs(3600)),
            enable_lru: true,
            shard_count: 16,
        }
    }
}

/// In-memory cache entry with access tracking.
struct InMemoryEntry {
    entry: CacheEntry,
    last_access: Instant,
    access_count: u64,
}

/// High-performance in-memory cache backend.
pub struct InMemoryBackend {
    /// Cached entries
    entries: DashMap<String, InMemoryEntry>,

    /// Tag to keys mapping for tag-based invalidation
    tag_index: DashMap<String, Vec<String>>,

    /// LRU order tracking
    lru_order: Mutex<VecDeque<String>>,

    /// Configuration
    config: InMemoryConfig,

    /// Statistics
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
    size_bytes: AtomicU64,
}

impl InMemoryBackend {
    /// Create a new in-memory backend.
    pub fn new(config: InMemoryConfig) -> Self {
        Self {
            entries: DashMap::with_shard_amount(config.shard_count),
            tag_index: DashMap::new(),
            lru_order: Mutex::new(VecDeque::with_capacity(config.max_capacity as usize)),
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            size_bytes: AtomicU64::new(0),
        }
    }

    /// Evict entries if over capacity.
    async fn maybe_evict(&self) {
        if self.entries.len() as u64 >= self.config.max_capacity {
            let to_evict = (self.config.max_capacity / 10).max(1) as usize;
            let mut evicted = 0;

            if self.config.enable_lru {
                let mut lru = self.lru_order.lock().await;
                while evicted < to_evict && !lru.is_empty() {
                    if let Some(key) = lru.pop_front() {
                        if let Some((_, entry)) = self.entries.remove(&key) {
                            self.remove_from_tag_index(&key, &entry.entry.tags);
                            self.size_bytes.fetch_sub(entry.entry.data.len() as u64, Ordering::Relaxed);
                            evicted += 1;
                        }
                    }
                }
            } else {
                // Random eviction
                let keys: Vec<_> = self.entries.iter().take(to_evict).map(|e| e.key().clone()).collect();
                for key in keys {
                    if let Some((_, entry)) = self.entries.remove(&key) {
                        self.remove_from_tag_index(&key, &entry.entry.tags);
                        self.size_bytes.fetch_sub(entry.entry.data.len() as u64, Ordering::Relaxed);
                        evicted += 1;
                    }
                }
            }

            self.evictions.fetch_add(evicted as u64, Ordering::Relaxed);
            debug!("Evicted {} entries from cache", evicted);
        }
    }

    /// Add key to tag index.
    fn add_to_tag_index(&self, key: &str, tags: &[String]) {
        for tag in tags {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .push(key.to_string());
        }
    }

    /// Remove key from tag index.
    fn remove_from_tag_index(&self, key: &str, tags: &[String]) {
        for tag in tags {
            if let Some(mut keys) = self.tag_index.get_mut(tag) {
                keys.retain(|k| k != key);
            }
        }
    }

    /// Update LRU tracking.
    async fn touch_lru(&self, key: &str) {
        if self.config.enable_lru {
            let mut lru = self.lru_order.lock().await;
            // Remove from current position and add to back
            lru.retain(|k| k != key);
            lru.push_back(key.to_string());
        }
    }

    /// Cleanup expired entries.
    pub async fn cleanup_expired(&self) -> u64 {
        let mut expired = 0;
        let mut keys_to_remove = Vec::new();

        for entry in self.entries.iter() {
            if entry.value().entry.is_expired() {
                keys_to_remove.push(entry.key().clone());
            } else if let Some(tti) = self.config.time_to_idle {
                if entry.value().last_access.elapsed() > tti {
                    keys_to_remove.push(entry.key().clone());
                }
            }
        }

        for key in keys_to_remove {
            if let Some((_, entry)) = self.entries.remove(&key) {
                self.remove_from_tag_index(&key, &entry.entry.tags);
                self.size_bytes.fetch_sub(entry.entry.data.len() as u64, Ordering::Relaxed);
                expired += 1;
            }
        }

        if expired > 0 {
            debug!("Cleaned up {} expired cache entries", expired);
        }

        expired
    }
}

#[async_trait]
impl CacheBackend for InMemoryBackend {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        if let Some(mut entry) = self.entries.get_mut(key) {
            if entry.entry.is_expired() {
                drop(entry);
                self.entries.remove(key);
                self.misses.fetch_add(1, Ordering::Relaxed);
                counter!("cache_misses_total", "backend" => "in_memory", "reason" => "expired").increment(1);
                return Ok(None);
            }

            entry.last_access = Instant::now();
            entry.access_count += 1;
            let result = entry.entry.clone();
            drop(entry);

            self.touch_lru(key).await;
            self.hits.fetch_add(1, Ordering::Relaxed);
            counter!("cache_hits_total", "backend" => "in_memory").increment(1);
            Ok(Some(result))
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            counter!("cache_misses_total", "backend" => "in_memory", "reason" => "not_found").increment(1);
            Ok(None)
        }
    }

    async fn set(&self, key: &str, entry: CacheEntry) -> Result<()> {
        self.maybe_evict().await;

        let size = entry.data.len();
        let tags = entry.tags.clone();

        // Update size tracking
        if let Some(existing) = self.entries.get(key) {
            self.size_bytes.fetch_sub(existing.entry.data.len() as u64, Ordering::Relaxed);
            self.remove_from_tag_index(key, &existing.entry.tags);
        }

        self.entries.insert(
            key.to_string(),
            InMemoryEntry {
                entry,
                last_access: Instant::now(),
                access_count: 0,
            },
        );

        self.size_bytes.fetch_add(size as u64, Ordering::Relaxed);
        self.add_to_tag_index(key, &tags);
        self.touch_lru(key).await;

        counter!("cache_sets_total", "backend" => "in_memory").increment(1);
        histogram!("cache_entry_size_bytes", "backend" => "in_memory").record(size as f64);

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        if let Some((_, entry)) = self.entries.remove(key) {
            self.remove_from_tag_index(key, &entry.entry.tags);
            self.size_bytes.fetch_sub(entry.entry.data.len() as u64, Ordering::Relaxed);
            counter!("cache_deletes_total", "backend" => "in_memory").increment(1);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        if let Some(entry) = self.entries.get(key) {
            Ok(!entry.entry.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn stats(&self) -> Result<CacheStats> {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let entries = self.entries.len() as u64;
        let size_bytes = self.size_bytes.load(Ordering::Relaxed);
        let evictions = self.evictions.load(Ordering::Relaxed);

        let mut stats = CacheStats {
            hits,
            misses,
            entries,
            size_bytes,
            evictions,
            hit_rate: 0.0,
            avg_entry_size: if entries > 0 { size_bytes as f64 / entries as f64 } else { 0.0 },
            backend_stats: HashMap::new(),
        };
        stats.calculate_hit_rate();

        stats.backend_stats.insert("max_capacity".to_string(), self.config.max_capacity.to_string());
        stats.backend_stats.insert("shard_count".to_string(), self.config.shard_count.to_string());

        // Record metrics
        gauge!("cache_entries", "backend" => "in_memory").set(entries as f64);
        gauge!("cache_size_bytes", "backend" => "in_memory").set(size_bytes as f64);
        gauge!("cache_hit_rate", "backend" => "in_memory").set(stats.hit_rate);

        Ok(stats)
    }

    async fn clear(&self) -> Result<()> {
        self.entries.clear();
        self.tag_index.clear();
        self.lru_order.lock().await.clear();
        self.size_bytes.store(0, Ordering::Relaxed);
        counter!("cache_clears_total", "backend" => "in_memory").increment(1);
        Ok(())
    }

    async fn get_by_tag(&self, tag: &str) -> Result<Vec<String>> {
        Ok(self.tag_index.get(tag).map(|keys| keys.clone()).unwrap_or_default())
    }

    async fn delete_by_pattern(&self, pattern: &str) -> Result<u64> {
        let regex = regex::Regex::new(pattern)
            .map_err(|e| ApexError::new(ErrorCode::InvalidInput, format!("Invalid pattern: {}", e)))?;

        let mut deleted = 0;
        let keys: Vec<_> = self.entries.iter().filter(|e| regex.is_match(e.key())).map(|e| e.key().clone()).collect();

        for key in keys {
            if let Some((_, entry)) = self.entries.remove(&key) {
                self.remove_from_tag_index(&key, &entry.entry.tags);
                self.size_bytes.fetch_sub(entry.entry.data.len() as u64, Ordering::Relaxed);
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    fn name(&self) -> &'static str {
        "in_memory"
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Redis Backend
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for Redis cache.
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,

    /// Connection pool size
    pub pool_size: usize,

    /// Key prefix
    pub key_prefix: String,

    /// Default TTL
    pub default_ttl: Duration,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// Command timeout
    pub command_timeout: Duration,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            pool_size: 10,
            key_prefix: "apex:".to_string(),
            default_ttl: Duration::from_secs(3600),
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(2),
        }
    }
}

/// Redis cache backend.
pub struct RedisBackend {
    client: redis::Client,
    config: RedisConfig,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl RedisBackend {
    /// Create a new Redis backend.
    pub async fn new(config: RedisConfig) -> Result<Self> {
        let client = redis::Client::open(config.url.as_str())
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheConnectionFailed,
                "Failed to create Redis client",
                e.to_string(),
            ))?;

        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheConnectionFailed,
                "Failed to connect to Redis",
                e.to_string(),
            ))?;

        let _: String = redis::cmd("PING").query_async(&mut conn).await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheConnectionFailed,
                "Redis ping failed",
                e.to_string(),
            ))?;

        info!("Redis cache backend connected to {}", config.url);

        Ok(Self {
            client,
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        })
    }

    /// Get a connection from the pool.
    async fn get_conn(&self) -> Result<redis::aio::MultiplexedConnection> {
        self.client.get_multiplexed_async_connection().await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheConnectionFailed,
                "Failed to get Redis connection",
                e.to_string(),
            ))
    }

    /// Build the full key with prefix.
    fn full_key(&self, key: &str) -> String {
        format!("{}{}", self.config.key_prefix, key)
    }

    /// Build tag key.
    fn tag_key(&self, tag: &str) -> String {
        format!("{}tag:{}", self.config.key_prefix, tag)
    }
}

#[async_trait]
impl CacheBackend for RedisBackend {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        let mut conn = self.get_conn().await?;
        let full_key = self.full_key(key);

        let data: Option<Vec<u8>> = conn.get(&full_key).await
            .map_err(ApexError::from)?;

        match data {
            Some(bytes) => {
                let entry: CacheEntry = serde_json::from_slice(&bytes)
                    .map_err(ApexError::from)?;

                self.hits.fetch_add(1, Ordering::Relaxed);
                counter!("cache_hits_total", "backend" => "redis").increment(1);
                Ok(Some(entry))
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                counter!("cache_misses_total", "backend" => "redis", "reason" => "not_found").increment(1);
                Ok(None)
            }
        }
    }

    async fn set(&self, key: &str, entry: CacheEntry) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let full_key = self.full_key(key);

        let data = serde_json::to_vec(&entry)
            .map_err(ApexError::from)?;

        let ttl_secs = entry.ttl.unwrap_or(self.config.default_ttl).as_secs() as i64;

        // Set with expiration
        conn.set_ex::<_, _, ()>(&full_key, &data, ttl_secs as u64).await
            .map_err(ApexError::from)?;

        // Add to tag sets
        for tag in &entry.tags {
            let tag_key = self.tag_key(tag);
            conn.sadd::<_, _, ()>(&tag_key, &full_key).await
                .map_err(ApexError::from)?;
            // Set TTL on tag set (slightly longer than entry TTL)
            conn.expire::<_, ()>(&tag_key, ttl_secs + 60).await
                .map_err(ApexError::from)?;
        }

        counter!("cache_sets_total", "backend" => "redis").increment(1);
        histogram!("cache_entry_size_bytes", "backend" => "redis").record(data.len() as f64);

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_conn().await?;
        let full_key = self.full_key(key);

        let deleted: i64 = conn.del(&full_key).await
            .map_err(ApexError::from)?;

        if deleted > 0 {
            counter!("cache_deletes_total", "backend" => "redis").increment(1);
        }

        Ok(deleted > 0)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_conn().await?;
        let full_key = self.full_key(key);

        let exists: bool = conn.exists(&full_key).await
            .map_err(ApexError::from)?;

        Ok(exists)
    }

    async fn stats(&self) -> Result<CacheStats> {
        let mut conn = self.get_conn().await?;

        // Get Redis INFO
        let info: String = redis::cmd("INFO")
            .arg("memory")
            .query_async(&mut conn)
            .await
            .map_err(ApexError::from)?;

        let mut backend_stats = HashMap::new();

        // Parse memory info
        for line in info.lines() {
            if line.starts_with("used_memory:") {
                backend_stats.insert("used_memory".to_string(), line.split(':').nth(1).unwrap_or("0").to_string());
            }
            if line.starts_with("used_memory_human:") {
                backend_stats.insert("used_memory_human".to_string(), line.split(':').nth(1).unwrap_or("0").to_string());
            }
        }

        // Get key count (approximate)
        let dbsize: u64 = redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .map_err(ApexError::from)?;

        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);

        let mut stats = CacheStats {
            hits,
            misses,
            entries: dbsize,
            size_bytes: 0, // Redis handles this internally
            evictions: 0, // Would need to track separately
            hit_rate: 0.0,
            avg_entry_size: 0.0,
            backend_stats,
        };
        stats.calculate_hit_rate();

        gauge!("cache_entries", "backend" => "redis").set(dbsize as f64);
        gauge!("cache_hit_rate", "backend" => "redis").set(stats.hit_rate);

        Ok(stats)
    }

    async fn clear(&self) -> Result<()> {
        let mut conn = self.get_conn().await?;

        // Use SCAN to find and delete keys with our prefix
        let pattern = format!("{}*", self.config.key_prefix);
        let mut cursor: u64 = 0;
        let mut total_deleted = 0;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(ApexError::from)?;

            if !keys.is_empty() {
                let deleted: i64 = conn.del(&keys).await
                    .map_err(ApexError::from)?;
                total_deleted += deleted;
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        info!("Cleared {} Redis cache entries", total_deleted);
        counter!("cache_clears_total", "backend" => "redis").increment(1);

        Ok(())
    }

    async fn get_by_tag(&self, tag: &str) -> Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let tag_key = self.tag_key(tag);

        let keys: Vec<String> = conn.smembers(&tag_key).await
            .map_err(ApexError::from)?;

        // Strip prefix from keys
        let prefix_len = self.config.key_prefix.len();
        let keys: Vec<String> = keys.into_iter()
            .filter_map(|k| {
                if k.len() > prefix_len {
                    Some(k[prefix_len..].to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(keys)
    }

    async fn delete_by_pattern(&self, pattern: &str) -> Result<u64> {
        let mut conn = self.get_conn().await?;

        let full_pattern = format!("{}{}", self.config.key_prefix, pattern);
        let mut cursor: u64 = 0;
        let mut total_deleted = 0;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&full_pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(ApexError::from)?;

            if !keys.is_empty() {
                let deleted: i64 = conn.del(&keys).await
                    .map_err(ApexError::from)?;
                total_deleted += deleted as u64;
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(total_deleted)
    }

    fn name(&self) -> &'static str {
        "redis"
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Multi-Tier Backend
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for multi-tier cache.
#[derive(Debug, Clone)]
pub struct MultiTierConfig {
    /// Promote L2 hits to L1
    pub promote_on_hit: bool,

    /// L1 TTL multiplier (relative to original TTL)
    pub l1_ttl_multiplier: f64,

    /// Write to L2 asynchronously
    pub async_l2_write: bool,
}

impl Default for MultiTierConfig {
    fn default() -> Self {
        Self {
            promote_on_hit: true,
            l1_ttl_multiplier: 0.5, // L1 entries expire faster
            async_l2_write: true,
        }
    }
}

/// Multi-tier cache backend (L1 memory + L2 Redis).
pub struct MultiTierBackend {
    l1: Arc<dyn CacheBackend>,
    l2: Arc<dyn CacheBackend>,
    config: MultiTierConfig,
}

impl MultiTierBackend {
    /// Create a new multi-tier backend.
    pub fn new(
        l1: Arc<dyn CacheBackend>,
        l2: Arc<dyn CacheBackend>,
        config: MultiTierConfig,
    ) -> Self {
        Self { l1, l2, config }
    }
}

#[async_trait]
impl CacheBackend for MultiTierBackend {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        // Try L1 first
        if let Some(entry) = self.l1.get(key).await? {
            counter!("cache_hits_total", "backend" => "multi_tier", "tier" => "l1").increment(1);
            return Ok(Some(entry));
        }

        // Try L2
        if let Some(entry) = self.l2.get(key).await? {
            counter!("cache_hits_total", "backend" => "multi_tier", "tier" => "l2").increment(1);

            // Promote to L1
            if self.config.promote_on_hit {
                let mut l1_entry = entry.clone();
                if let Some(ttl) = l1_entry.ttl {
                    l1_entry.ttl = Some(Duration::from_secs_f64(
                        ttl.as_secs_f64() * self.config.l1_ttl_multiplier
                    ));
                }
                // Best effort promotion, ignore errors
                let _ = self.l1.set(key, l1_entry).await;
            }

            return Ok(Some(entry));
        }

        counter!("cache_misses_total", "backend" => "multi_tier").increment(1);
        Ok(None)
    }

    async fn set(&self, key: &str, entry: CacheEntry) -> Result<()> {
        // Always write to L1
        let mut l1_entry = entry.clone();
        if let Some(ttl) = l1_entry.ttl {
            l1_entry.ttl = Some(Duration::from_secs_f64(
                ttl.as_secs_f64() * self.config.l1_ttl_multiplier
            ));
        }
        self.l1.set(key, l1_entry).await?;

        // Write to L2
        if self.config.async_l2_write {
            let l2 = self.l2.clone();
            let key = key.to_string();
            tokio::spawn(async move {
                if let Err(e) = l2.set(&key, entry).await {
                    warn!("Failed to write to L2 cache: {}", e);
                }
            });
        } else {
            self.l2.set(key, entry).await?;
        }

        counter!("cache_sets_total", "backend" => "multi_tier").increment(1);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let l1_deleted = self.l1.delete(key).await?;
        let l2_deleted = self.l2.delete(key).await?;
        counter!("cache_deletes_total", "backend" => "multi_tier").increment(1);
        Ok(l1_deleted || l2_deleted)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        if self.l1.exists(key).await? {
            return Ok(true);
        }
        self.l2.exists(key).await
    }

    async fn stats(&self) -> Result<CacheStats> {
        let l1_stats = self.l1.stats().await?;
        let l2_stats = self.l2.stats().await?;

        let mut backend_stats = HashMap::new();
        backend_stats.insert("l1_hits".to_string(), l1_stats.hits.to_string());
        backend_stats.insert("l1_misses".to_string(), l1_stats.misses.to_string());
        backend_stats.insert("l1_entries".to_string(), l1_stats.entries.to_string());
        backend_stats.insert("l2_hits".to_string(), l2_stats.hits.to_string());
        backend_stats.insert("l2_misses".to_string(), l2_stats.misses.to_string());
        backend_stats.insert("l2_entries".to_string(), l2_stats.entries.to_string());

        let total_hits = l1_stats.hits + l2_stats.hits;
        let total_misses = l2_stats.misses; // Only count L2 misses as true misses

        let mut stats = CacheStats {
            hits: total_hits,
            misses: total_misses,
            entries: l1_stats.entries + l2_stats.entries,
            size_bytes: l1_stats.size_bytes + l2_stats.size_bytes,
            evictions: l1_stats.evictions + l2_stats.evictions,
            hit_rate: 0.0,
            avg_entry_size: 0.0,
            backend_stats,
        };
        stats.calculate_hit_rate();

        Ok(stats)
    }

    async fn clear(&self) -> Result<()> {
        self.l1.clear().await?;
        self.l2.clear().await?;
        counter!("cache_clears_total", "backend" => "multi_tier").increment(1);
        Ok(())
    }

    async fn get_by_tag(&self, tag: &str) -> Result<Vec<String>> {
        // Get from L2 as authoritative source
        self.l2.get_by_tag(tag).await
    }

    async fn delete_by_pattern(&self, pattern: &str) -> Result<u64> {
        let l1_deleted = self.l1.delete_by_pattern(pattern).await?;
        let l2_deleted = self.l2.delete_by_pattern(pattern).await?;
        Ok(l1_deleted.max(l2_deleted))
    }

    fn name(&self) -> &'static str {
        "multi_tier"
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry {
            data: vec![1, 2, 3],
            ttl: Some(Duration::from_millis(100)),
            tags: vec!["test".to_string()],
            created_at: Utc::now() - chrono::Duration::milliseconds(200),
        };

        assert!(entry.is_expired());
    }

    #[test]
    fn test_cache_entry_not_expired() {
        let entry = CacheEntry {
            data: vec![1, 2, 3],
            ttl: Some(Duration::from_secs(3600)),
            tags: vec!["test".to_string()],
            created_at: Utc::now(),
        };

        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_no_ttl() {
        let entry = CacheEntry {
            data: vec![1, 2, 3],
            ttl: None,
            tags: vec!["test".to_string()],
            created_at: Utc::now() - chrono::Duration::days(365),
        };

        assert!(!entry.is_expired());
    }

    #[tokio::test]
    async fn test_in_memory_backend() {
        let backend = InMemoryBackend::new(InMemoryConfig {
            max_capacity: 100,
            ..Default::default()
        });

        let entry = CacheEntry {
            data: b"test data".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec!["tag1".to_string()],
            created_at: Utc::now(),
        };

        // Set
        backend.set("key1", entry.clone()).await.unwrap();

        // Get
        let retrieved = backend.get("key1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, b"test data");

        // Exists
        assert!(backend.exists("key1").await.unwrap());

        // Delete
        assert!(backend.delete("key1").await.unwrap());
        assert!(!backend.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn test_in_memory_tag_lookup() {
        let backend = InMemoryBackend::new(InMemoryConfig::default());

        let entry = CacheEntry {
            data: b"data".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec!["project-123".to_string()],
            created_at: Utc::now(),
        };

        backend.set("task-1", entry.clone()).await.unwrap();
        backend.set("task-2", entry.clone()).await.unwrap();

        let keys = backend.get_by_tag("project-123").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"task-1".to_string()));
        assert!(keys.contains(&"task-2".to_string()));
    }

    #[tokio::test]
    async fn test_in_memory_eviction() {
        let backend = InMemoryBackend::new(InMemoryConfig {
            max_capacity: 5,
            enable_lru: true,
            ..Default::default()
        });

        // Fill cache
        for i in 0..10 {
            let entry = CacheEntry {
                data: vec![i as u8],
                ttl: Some(Duration::from_secs(60)),
                tags: vec![],
                created_at: Utc::now(),
            };
            backend.set(&format!("key-{}", i), entry).await.unwrap();
        }

        // Should have evicted some entries
        let stats = backend.stats().await.unwrap();
        assert!(stats.entries <= 5);
        assert!(stats.evictions > 0);
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let backend = InMemoryBackend::new(InMemoryConfig::default());

        let entry = CacheEntry {
            data: b"test".to_vec(),
            ttl: Some(Duration::from_secs(60)),
            tags: vec![],
            created_at: Utc::now(),
        };

        backend.set("key1", entry).await.unwrap();
        backend.get("key1").await.unwrap(); // Hit
        backend.get("key2").await.unwrap(); // Miss

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
        assert!((stats.hit_rate - 0.5).abs() < 0.01);
    }
}
