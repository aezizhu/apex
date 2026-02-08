//! API key rotation and lifecycle management.

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Error)]
pub enum ApiKeyError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already revoked: {0}")]
    AlreadyRevoked(String),

    #[error("Max keys exceeded ({0})")]
    MaxKeysExceeded(usize),

    #[error("Expired: {0}")]
    Expired(String),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStatus {
    Active,
    Rotating,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyEntry {
    pub id: String,
    pub key_hash: String,
    pub owner: String,
    pub label: String,
    pub status: KeyStatus,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub previous_key_hash: Option<String>,
    pub rotation_grace_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct GeneratedKey {
    pub raw_key: String,
    pub key_id: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ApiKeyConfig {
    pub key_prefix: String,
    pub key_length: usize,
    pub rotation_grace_period: Duration,
    pub max_keys_per_user: usize,
    pub default_expiration: Option<Duration>,
}

impl Default for ApiKeyConfig {
    fn default() -> Self {
        Self {
            key_prefix: "apex_".into(),
            key_length: 48,
            rotation_grace_period: Duration::hours(24),
            max_keys_per_user: 5,
            default_expiration: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// API Key Manager
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ApiKeyManager {
    config: ApiKeyConfig,
    keys_by_hash: Arc<DashMap<String, ApiKeyEntry>>,
    keys_by_id: Arc<DashMap<String, String>>,
    keys_by_owner: Arc<DashMap<String, Vec<String>>>,
}

impl ApiKeyManager {
    pub fn new(config: ApiKeyConfig) -> Self {
        Self {
            config,
            keys_by_hash: Arc::new(DashMap::new()),
            keys_by_id: Arc::new(DashMap::new()),
            keys_by_owner: Arc::new(DashMap::new()),
        }
    }

    /// Generate a new API key for the given owner.
    pub fn generate_key(&self, owner: &str, label: &str) -> Result<GeneratedKey, ApiKeyError> {
        let count = self
            .keys_by_owner
            .get(owner)
            .map(|v| v.len())
            .unwrap_or(0);

        if count >= self.config.max_keys_per_user {
            return Err(ApiKeyError::MaxKeysExceeded(self.config.max_keys_per_user));
        }

        let raw = self.gen_raw();
        let hash = self.hash(&raw);
        let id = Uuid::new_v4().to_string();
        let exp = self.config.default_expiration.map(|d| Utc::now() + d);

        let entry = ApiKeyEntry {
            id: id.clone(),
            key_hash: hash.clone(),
            owner: owner.into(),
            label: label.into(),
            status: KeyStatus::Active,
            created_at: Utc::now(),
            expires_at: exp,
            last_used_at: None,
            previous_key_hash: None,
            rotation_grace_until: None,
        };

        self.keys_by_hash.insert(hash.clone(), entry);
        self.keys_by_id.insert(id.clone(), hash.clone());
        self.keys_by_owner
            .entry(owner.into())
            .or_default()
            .push(hash);

        info!(key_id = %id, owner = %owner, "API key generated");
        Ok(GeneratedKey {
            raw_key: raw,
            key_id: id,
        })
    }

    /// Revoke a key by its ID.
    pub fn revoke_key(&self, id: &str) -> Result<(), ApiKeyError> {
        let hash = self
            .keys_by_id
            .get(id)
            .map(|v| v.clone())
            .ok_or_else(|| ApiKeyError::NotFound(id.into()))?;

        self.keys_by_hash
            .get_mut(&hash)
            .ok_or_else(|| ApiKeyError::NotFound(id.into()))?
            .status = KeyStatus::Revoked;

        info!(key_id = %id, "API key revoked");
        Ok(())
    }

    /// Validate a raw API key and return its entry if valid.
    ///
    /// Also checks the previous key hash during the rotation grace period.
    pub fn validate_key(&self, raw: &str) -> Option<ApiKeyEntry> {
        let hash = self.hash(raw);

        // Check direct hash match.
        if let Some(mut entry) = self.keys_by_hash.get_mut(&hash) {
            match &entry.status {
                KeyStatus::Active | KeyStatus::Rotating => {
                    if let Some(exp) = entry.expires_at {
                        if Utc::now() > exp {
                            entry.status = KeyStatus::Expired;
                            return None;
                        }
                    }
                    entry.last_used_at = Some(Utc::now());
                    return Some(entry.clone());
                }
                _ => return None,
            }
        }

        // Check rotation grace period: the old key hash may still be accepted
        // if the new key has a `previous_key_hash` matching and the grace
        // window hasn't closed.
        for entry in self.keys_by_hash.iter() {
            if let Some(ref prev) = entry.previous_key_hash {
                if *prev == hash {
                    if let Some(grace) = entry.rotation_grace_until {
                        if Utc::now() < grace {
                            return Some(entry.clone());
                        }
                    }
                }
            }
        }

        None
    }

    /// List all keys belonging to `owner`.
    pub fn list_keys(&self, owner: &str) -> Vec<ApiKeyEntry> {
        self.keys_by_owner
            .get(owner)
            .map(|hashes| {
                hashes
                    .iter()
                    .filter_map(|h| self.keys_by_hash.get(h).map(|e| e.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Generate a random key string with the configured prefix.
    fn gen_raw(&self) -> String {
        let mut rng = rand::thread_rng();
        let chars: Vec<char> =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                .chars()
                .collect();
        let suffix: String = (0..self.config.key_length)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect();
        format!("{}{}", self.config.key_prefix, suffix)
    }

    /// SHA-256 hash of a key string.
    fn hash(&self, key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    }
}
