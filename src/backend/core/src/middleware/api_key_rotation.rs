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
#[derive(Debug, Error)]
pub enum ApiKeyError { #[error("Not found: {0}")] NotFound(String), #[error("Already revoked: {0}")] AlreadyRevoked(String), #[error("Max keys exceeded ({0})")] MaxKeysExceeded(usize), #[error("Expired: {0}")] Expired(String) }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStatus { Active, Rotating, Revoked, Expired }
#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyEntry { pub id: String, pub key_hash: String, pub owner: String, pub label: String, pub status: KeyStatus, pub created_at: DateTime<Utc>, pub expires_at: Option<DateTime<Utc>>, pub last_used_at: Option<DateTime<Utc>>, pub previous_key_hash: Option<String>, pub rotation_grace_until: Option<DateTime<Utc>> }
#[derive(Debug, Clone)] pub struct GeneratedKey { pub raw_key: String, pub key_id: String }
#[derive(Debug, Clone)]
pub struct ApiKeyConfig { pub key_prefix: String, pub key_length: usize, pub rotation_grace_period: Duration, pub max_keys_per_user: usize, pub default_expiration: Option<Duration> }
impl Default for ApiKeyConfig { fn default() -> Self { Self { key_prefix: "apex_".into(), key_length: 48, rotation_grace_period: Duration::hours(24), max_keys_per_user: 5, default_expiration: None } } }
#[derive(Debug, Clone)]
pub struct ApiKeyManager { config: ApiKeyConfig, keys_by_hash: Arc<DashMap<String, ApiKeyEntry>>, keys_by_id: Arc<DashMap<String, String>>, keys_by_owner: Arc<DashMap<String, Vec<String>>> }
impl ApiKeyManager {
    pub fn new(config: ApiKeyConfig) -> Self { Self { config, keys_by_hash: Arc::new(DashMap::new()), keys_by_id: Arc::new(DashMap::new()), keys_by_owner: Arc::new(DashMap::new()) } }
    pub fn generate_key(&self, owner: &str, label: &str) -> Result<GeneratedKey, ApiKeyError> {
        let c = self.keys_by_owner.get(owner).map(|v| v.len()).unwrap_or(0);
        if c >= self.config.max_keys_per_user { return Err(ApiKeyError::MaxKeysExceeded(self.config.max_keys_per_user)); }
        let raw = self.gen_raw(); let hash = self.hash(&raw); let id = Uuid::new_v4().to_string();
        let exp = self.config.default_expiration.map(|d| Utc::now() + d);
        let entry = ApiKeyEntry { id: id.clone(), key_hash: hash.clone(), owner: owner.into(), label: label.into(), status: KeyStatus::Active, created_at: Utc::now(), expires_at: exp, last_used_at: None, previous_key_hash: None, rotation_grace_until: None };
        self.keys_by_hash.insert(hash.clone(), entry); self.keys_by_id.insert(id.clone(), hash.clone()); self.keys_by_owner.entry(owner.into()).or_default().push(hash);
        info!(key_id=%id, owner=%owner, "API key generated"); Ok(GeneratedKey { raw_key: raw, key_id: id })
    }
    pub fn revoke_key(&self, id: &str) -> Result<(), ApiKeyError> { let h = self.keys_by_id.get(id).map(|v| v.clone()).ok_or_else(|| ApiKeyError::NotFound(id.into()))?; self.keys_by_hash.get_mut(&h).ok_or_else(|| ApiKeyError::NotFound(id.into()))?.status = KeyStatus::Revoked; info!(key_id=%id, "API key revoked"); Ok(()) }
    pub fn validate_key(&self, raw: &str) -> Option<ApiKeyEntry> { let h = self.hash(raw); if let Some(mut e) = self.keys_by_hash.get_mut(&h) { match &e.status { KeyStatus::Active | KeyStatus::Rotating => { if let Some(exp) = e.expires_at { if Utc::now() > exp { e.status = KeyStatus::Expired; return None; } } e.last_used_at = Some(Utc::now()); return Some(e.clone()); } _ => return None } } for e in self.keys_by_hash.iter() { if let Some(ref p) = e.previous_key_hash { if *p == h { if let Some(g) = e.rotation_grace_until { if Utc::now() < g { return Some(e.clone()); } } } } } None }
    pub fn list_keys(&self, owner: &str) -> Vec<ApiKeyEntry> { self.keys_by_owner.get(owner).map(|hs| hs.iter().filter_map(|h| self.keys_by_hash.get(h).map(|e| e.clone())).collect()).unwrap_or_default() }
    fn gen_raw(&self) -> String { let mut r = rand::rng(); let c: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect(); let s: String = (0..self.config.key_length).map(|_| c[r.random_range(0..c.len())]).collect(); format!("{}{}", self.config.key_prefix, s) }
    fn hash(&self, key: &str) -> String { let mut h = Sha256::new(); h.update(key.as_bytes()); hex::encode(h.finalize()) }
}
