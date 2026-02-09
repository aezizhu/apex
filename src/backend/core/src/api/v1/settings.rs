//! Settings API handlers (V1).
//!
//! Provides REST endpoints for managing system settings and API keys.
//! - GET /settings - Retrieve current system settings
//! - PUT /settings - Update system settings
//! - GET /settings/api-keys - List configured API keys (masked)
//! - POST /settings/api-keys - Create / store a new API key
//! - DELETE /settings/api-keys/:id - Revoke an API key

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{ApiResponse, AppState};
use crate::api::middleware::sanitize_string;

// ═══════════════════════════════════════════════════════════════════════════════
// DTOs
// ═══════════════════════════════════════════════════════════════════════════════

/// System settings as returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    pub max_concurrent_tasks: u32,
    pub default_agent_model: String,
    pub approval_threshold: f64,
    pub auto_retry_enabled: bool,
    pub max_retries: u32,
    pub log_level: String,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 100,
            default_agent_model: "claude-sonnet-4-5".to_string(),
            approval_threshold: 1.0,
            auto_retry_enabled: true,
            max_retries: 5,
            log_level: "info".to_string(),
        }
    }
}

/// Request body for updating system settings (all fields optional).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsRequest {
    pub max_concurrent_tasks: Option<u32>,
    pub default_agent_model: Option<String>,
    pub approval_threshold: Option<f64>,
    pub auto_retry_enabled: Option<bool>,
    pub max_retries: Option<u32>,
    pub log_level: Option<String>,
}

impl UpdateSettingsRequest {
    fn sanitize(&mut self) {
        if let Some(ref mut model) = self.default_agent_model {
            *model = sanitize_string(model);
        }
        if let Some(ref mut level) = self.log_level {
            *level = sanitize_string(level);
        }
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Some(max) = self.max_concurrent_tasks {
            if max == 0 {
                errors.push("maxConcurrentTasks must be greater than 0".to_string());
            } else if max > 10_000 {
                errors.push("maxConcurrentTasks must be at most 10,000".to_string());
            }
        }
        if let Some(threshold) = self.approval_threshold {
            if threshold < 0.0 {
                errors.push("approvalThreshold must not be negative".to_string());
            }
        }
        if let Some(retries) = self.max_retries {
            if retries > 100 {
                errors.push("maxRetries must be at most 100".to_string());
            }
        }
        if let Some(ref level) = self.log_level {
            let valid = ["debug", "info", "warn", "error"];
            if !valid.contains(&level.as_str()) {
                errors.push(format!("logLevel must be one of: {}", valid.join(", ")));
            }
        }
        errors
    }
}

/// Summary of a stored API key (secret value is masked).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeySummary {
    pub id: Uuid,
    pub provider: String,
    pub masked_key: String,
    pub created_at: String,
}

/// Request body for creating a new API key.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiKeyRequest {
    pub provider: String,
    pub key: String,
}

impl CreateApiKeyRequest {
    fn sanitize(&mut self) {
        self.provider = sanitize_string(&self.provider);
        // Key value is not sanitised – it is an opaque secret.
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.provider.is_empty() {
            errors.push("provider must not be empty".to_string());
        }
        if self.key.is_empty() {
            errors.push("key must not be empty".to_string());
        } else if self.key.len() > 512 {
            errors.push("key must be at most 512 characters".to_string());
        }
        errors
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// In-memory store (placeholder until persistent storage is wired)
// ═══════════════════════════════════════════════════════════════════════════════

use std::sync::{Arc, LazyLock, Mutex};

struct SettingsStore {
    settings: SystemSettings,
    api_keys: Vec<StoredApiKey>,
}

#[derive(Clone)]
struct StoredApiKey {
    id: Uuid,
    provider: String,
    masked_key: String,
    created_at: String,
}

static STORE: LazyLock<Arc<Mutex<SettingsStore>>> = LazyLock::new(|| {
    Arc::new(Mutex::new(SettingsStore {
        settings: SystemSettings::default(),
        api_keys: Vec::new(),
    }))
});

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }
    let visible = &key[..4];
    format!("{}...{}", visible, &key[key.len() - 4..])
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// GET /settings
pub async fn get_settings(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let store = STORE.lock().unwrap();
    let settings = store.settings.clone();
    Json(ApiResponse::success(settings))
}

/// PUT /settings
pub async fn update_settings(
    State(_state): State<AppState>,
    Json(mut req): Json<UpdateSettingsRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            errors.join("; "),
            "VALIDATION_ERROR",
        ));
    }

    let mut store = STORE.lock().unwrap();
    let s = &mut store.settings;

    if let Some(v) = req.max_concurrent_tasks {
        s.max_concurrent_tasks = v;
    }
    if let Some(v) = req.default_agent_model {
        s.default_agent_model = v;
    }
    if let Some(v) = req.approval_threshold {
        s.approval_threshold = v;
    }
    if let Some(v) = req.auto_retry_enabled {
        s.auto_retry_enabled = v;
    }
    if let Some(v) = req.max_retries {
        s.max_retries = v;
    }
    if let Some(v) = req.log_level {
        s.log_level = v;
    }

    Json(ApiResponse::success(s.clone()))
}

/// GET /settings/api-keys
pub async fn get_api_keys(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let store = STORE.lock().unwrap();
    let summaries: Vec<ApiKeySummary> = store
        .api_keys
        .iter()
        .map(|k| ApiKeySummary {
            id: k.id,
            provider: k.provider.clone(),
            masked_key: k.masked_key.clone(),
            created_at: k.created_at.clone(),
        })
        .collect();
    Json(ApiResponse::success(summaries))
}

/// POST /settings/api-keys
pub async fn create_api_key(
    State(_state): State<AppState>,
    Json(mut req): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            errors.join("; "),
            "VALIDATION_ERROR",
        ));
    }

    let stored = StoredApiKey {
        id: Uuid::new_v4(),
        provider: req.provider.clone(),
        masked_key: mask_key(&req.key),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let summary = ApiKeySummary {
        id: stored.id,
        provider: stored.provider.clone(),
        masked_key: stored.masked_key.clone(),
        created_at: stored.created_at.clone(),
    };

    let mut store = STORE.lock().unwrap();
    store.api_keys.push(stored);

    Json(ApiResponse::success(summary))
}

/// DELETE /settings/api-keys/:id
pub async fn revoke_api_key(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let mut store = STORE.lock().unwrap();
    let before = store.api_keys.len();
    store.api_keys.retain(|k| k.id != id);
    let removed = store.api_keys.len() < before;

    if removed {
        Json(ApiResponse::success(serde_json::json!({
            "id": id,
            "status": "revoked"
        })))
    } else {
        Json(ApiResponse::error("API key not found"))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = SystemSettings::default();
        assert_eq!(s.max_concurrent_tasks, 100);
        assert_eq!(s.default_agent_model, "claude-sonnet-4-5");
        assert!(s.auto_retry_enabled);
        assert_eq!(s.log_level, "info");
    }

    #[test]
    fn test_mask_key_short() {
        assert_eq!(mask_key("abc"), "***");
    }

    #[test]
    fn test_mask_key_long() {
        let masked = mask_key("sk-1234567890abcdef");
        assert!(masked.starts_with("sk-1"));
        assert!(masked.ends_with("cdef"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_validate_update_request_valid() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: Some(50),
            default_agent_model: Some("gpt-4o".to_string()),
            approval_threshold: Some(0.5),
            auto_retry_enabled: Some(true),
            max_retries: Some(3),
            log_level: Some("debug".to_string()),
        };
        assert!(req.validate().is_empty());
    }

    #[test]
    fn test_validate_update_request_invalid_log_level() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: None,
            default_agent_model: None,
            approval_threshold: None,
            auto_retry_enabled: None,
            max_retries: None,
            log_level: Some("verbose".to_string()),
        };
        let errors = req.validate();
        assert!(!errors.is_empty());
        assert!(errors[0].contains("logLevel"));
    }

    #[test]
    fn test_validate_create_api_key_empty_provider() {
        let req = CreateApiKeyRequest {
            provider: "".to_string(),
            key: "sk-test".to_string(),
        };
        let errors = req.validate();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_create_api_key_empty_key() {
        let req = CreateApiKeyRequest {
            provider: "openai".to_string(),
            key: "".to_string(),
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("key")));
    }

    #[test]
    fn test_validate_create_api_key_too_long() {
        let req = CreateApiKeyRequest {
            provider: "openai".to_string(),
            key: "x".repeat(513),
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("512")));
    }

    #[test]
    fn test_validate_create_api_key_valid() {
        let req = CreateApiKeyRequest {
            provider: "anthropic".to_string(),
            key: "sk-ant-api03-valid-key".to_string(),
        };
        assert!(req.validate().is_empty());
    }

    #[test]
    fn test_validate_update_zero_concurrent_tasks() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: Some(0),
            default_agent_model: None,
            approval_threshold: None,
            auto_retry_enabled: None,
            max_retries: None,
            log_level: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("greater than 0")));
    }

    #[test]
    fn test_validate_update_max_concurrent_too_high() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: Some(10_001),
            default_agent_model: None,
            approval_threshold: None,
            auto_retry_enabled: None,
            max_retries: None,
            log_level: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("10,000")));
    }

    #[test]
    fn test_validate_update_negative_threshold() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: None,
            default_agent_model: None,
            approval_threshold: Some(-0.5),
            auto_retry_enabled: None,
            max_retries: None,
            log_level: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("negative")));
    }

    #[test]
    fn test_validate_update_retries_too_high() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: None,
            default_agent_model: None,
            approval_threshold: None,
            auto_retry_enabled: None,
            max_retries: Some(101),
            log_level: None,
        };
        let errors = req.validate();
        assert!(errors.iter().any(|e| e.contains("100")));
    }

    #[test]
    fn test_validate_update_boundary_retries() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: None,
            default_agent_model: None,
            approval_threshold: None,
            auto_retry_enabled: None,
            max_retries: Some(100),
            log_level: None,
        };
        assert!(req.validate().is_empty());
    }

    #[test]
    fn test_validate_update_all_log_levels() {
        for level in ["debug", "info", "warn", "error"] {
            let req = UpdateSettingsRequest {
                max_concurrent_tasks: None,
                default_agent_model: None,
                approval_threshold: None,
                auto_retry_enabled: None,
                max_retries: None,
                log_level: Some(level.to_string()),
            };
            assert!(req.validate().is_empty(), "level '{}' should be valid", level);
        }
    }

    #[test]
    fn test_validate_update_multiple_errors() {
        let req = UpdateSettingsRequest {
            max_concurrent_tasks: Some(0),
            default_agent_model: None,
            approval_threshold: Some(-1.0),
            auto_retry_enabled: None,
            max_retries: Some(200),
            log_level: Some("trace".to_string()),
        };
        let errors = req.validate();
        assert_eq!(errors.len(), 4, "Should have 4 validation errors");
    }

    #[test]
    fn test_mask_key_exactly_8_chars() {
        assert_eq!(mask_key("12345678"), "********");
    }

    #[test]
    fn test_mask_key_9_chars() {
        let masked = mask_key("123456789");
        assert_eq!(masked, "1234...6789");
    }

    #[test]
    fn test_sanitize_update_request() {
        let mut req = UpdateSettingsRequest {
            max_concurrent_tasks: None,
            default_agent_model: Some("<script>alert('xss')</script>".to_string()),
            approval_threshold: None,
            auto_retry_enabled: None,
            max_retries: None,
            log_level: Some("<b>info</b>".to_string()),
        };
        req.sanitize();
        assert!(!req.default_agent_model.as_ref().unwrap().contains('<'));
        assert!(!req.log_level.as_ref().unwrap().contains('<'));
    }

    #[test]
    fn test_sanitize_create_api_key() {
        let mut req = CreateApiKeyRequest {
            provider: "<img src=x onerror=alert(1)>".to_string(),
            key: "sk-secret-key".to_string(),
        };
        req.sanitize();
        assert!(!req.provider.contains('<'));
        // Key should NOT be sanitized (opaque secret)
        assert_eq!(req.key, "sk-secret-key");
    }
}
