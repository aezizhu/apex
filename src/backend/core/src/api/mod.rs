//! API layer for Apex Core.
//!
//! This module provides both REST (via Axum) and gRPC (via Tonic) interfaces
//! to the Apex orchestration engine.
//!
//! # API Versioning
//!
//! The API supports multiple versioning strategies:
//!
//! 1. **URL Path Versioning** (recommended):
//!    - `/api/v1/tasks` - V1 API
//!    - `/api/v2/tasks` - V2 API
//!
//! 2. **Header Versioning**:
//!    - `Accept: application/vnd.apex.v1+json`
//!    - `X-API-Version: 1`
//!
//! # Deprecation Policy
//!
//! When an API version is deprecated:
//! - Responses include `Deprecation: true` header
//! - Responses include `Sunset: <date>` header with sunset date
//! - Responses include `X-API-Warn` header with migration guidance
//!
//! # Current Versions
//!
//! - **V1** (Current/Stable): Full production support
//! - **V2** (Preview): New features, may change without notice

mod handlers;
pub mod middleware;
mod websocket;
pub mod grpc;
pub mod versioning;
pub mod v1;
pub mod v2;

use axum::{
    middleware as axum_middleware,
    routing::get,
    Router,
};
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
    compression::CompressionLayer,
};
use std::sync::Arc;

use crate::orchestrator::SwarmOrchestrator;
use crate::db::Database;
use crate::middleware::{
    SecurityHeadersLayer, SecurityHeadersConfig,
    RequestSizeLayer, RequestSizeConfig,
    AuditLayer, AuditConfig,
    CsrfLayer, CsrfConfig,
    InputSanitizerLayer, SanitizeConfig,
};
use crate::plugins::PluginRegistry;

pub use versioning::{
    ApiVersion, ExtractedVersion, Version, VersionConfig, VersionError,
    VersionInfo, VersionSource, VersionStatus, VersionedRouter, VersioningLayer,
};

/// Application state shared across handlers.
#[derive(Clone)]
pub struct AppState {
    pub orchestrator: Arc<SwarmOrchestrator>,
    pub db: Arc<Database>,
    pub plugin_registry: Option<PluginRegistry>,
}

impl AppState {
    /// Get the plugin registry, creating a default one if not configured.
    pub fn plugin_registry(&self) -> PluginRegistry {
        self.plugin_registry
            .clone()
            .unwrap_or_else(|| PluginRegistry::new("plugins"))
    }
}

/// Build the API router with versioning support.
///
/// This creates a router with:
/// - Health check endpoint (unversioned)
/// - Metrics endpoint (unversioned)
/// - WebSocket endpoint (unversioned)
/// - V1 API routes under `/api/v1/`
/// - V2 API routes under `/api/v2/`
/// - Versioning middleware with deprecation headers
///
/// # Example
///
/// ```rust,ignore
/// let state = AppState { orchestrator, db };
/// let app = build_router(state);
/// ```
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Version configuration
    let version_config = VersionConfig::default();

    Router::new()
        // Unversioned endpoints (health, metrics, websocket)
        .route("/health", get(handlers::health_check))
        .route("/metrics", get(handlers::prometheus_metrics))
        .route("/ws", get(websocket::ws_handler))
        // API version info endpoint
        .route("/api/versions", get(api_versions_handler))
        // V1 API (stable)
        .nest("/api/v1", v1::routes::v1_router())
        // V2 API (preview)
        .nest("/api/v2", v2::v2_router())
        // Middleware
        .layer(SecurityHeadersLayer::new(SecurityHeadersConfig::default()))
        .layer(AuditLayer::new(AuditConfig::default()))
        .layer(CsrfLayer::new(CsrfConfig::default()))
        .layer(InputSanitizerLayer::new(SanitizeConfig::default()))
        .layer(RequestSizeLayer::new(RequestSizeConfig::default()))
        .layer(axum_middleware::from_fn(middleware::api_version_headers))
        .layer(axum_middleware::from_fn(middleware::content_type_validation))
        .layer(VersioningLayer::new(version_config))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(cors)
        .with_state(state)
}

/// Build the API router with custom version configuration.
pub fn build_router_with_config(state: AppState, version_config: VersionConfig) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Unversioned endpoints
        .route("/health", get(handlers::health_check))
        .route("/metrics", get(handlers::prometheus_metrics))
        .route("/ws", get(websocket::ws_handler))
        // API version info endpoint
        .route("/api/versions", get(api_versions_handler))
        // V1 API (stable)
        .nest("/api/v1", v1::routes::v1_router())
        // V2 API (preview)
        .nest("/api/v2", v2::v2_router())
        // Middleware
        .layer(SecurityHeadersLayer::new(SecurityHeadersConfig::default()))
        .layer(AuditLayer::new(AuditConfig::default()))
        .layer(CsrfLayer::new(CsrfConfig::default()))
        .layer(InputSanitizerLayer::new(SanitizeConfig::default()))
        .layer(RequestSizeLayer::new(RequestSizeConfig::default()))
        .layer(VersioningLayer::new(version_config))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(cors)
        .with_state(state)
}

/// Handler for API versions endpoint.
async fn api_versions_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "success": true,
        "data": {
            "versions": [
                {
                    "version": "1.0",
                    "status": "current",
                    "path": "/api/v1",
                    "media_type": "application/vnd.apex.v1+json",
                    "description": "Stable API version with full production support"
                },
                {
                    "version": "2.0",
                    "status": "preview",
                    "path": "/api/v2",
                    "media_type": "application/vnd.apex.v2+json",
                    "description": "Preview API version with new features (may change without notice)",
                    "features": [
                        "batch_operations",
                        "cursor_pagination",
                        "enhanced_errors"
                    ]
                }
            ],
            "default_version": "1.0",
            "latest_stable": "1.0",
            "versioning": {
                "strategies": [
                    {
                        "type": "url_path",
                        "example": "/api/v1/tasks",
                        "description": "Version in URL path (recommended)"
                    },
                    {
                        "type": "accept_header",
                        "example": "Accept: application/vnd.apex.v1+json",
                        "description": "Version in Accept header"
                    },
                    {
                        "type": "custom_header",
                        "example": "X-API-Version: 1",
                        "description": "Version in custom header"
                    }
                ],
                "priority": ["url_path", "custom_header", "accept_header", "default"]
            },
            "deprecation_policy": {
                "notice_period_days": 90,
                "sunset_header": "Sunset",
                "deprecation_header": "Deprecation",
                "warning_header": "X-API-Warn"
            }
        }
    }))
}

/// API response wrapper.
#[derive(serde::Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl<T: serde::Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            error_code: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            error_code: None,
        }
    }

    pub fn error_with_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
            error_code: Some(code.into()),
        }
    }

    pub fn from_apex_error(err: &crate::error::ApexError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(err.user_message().to_string()),
            error_code: Some(err.code().to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<()> = ApiResponse::error("test error");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_versioned_router_paths() {
        assert_eq!(VersionedRouter::v1("tasks"), "/api/v1/tasks");
        assert_eq!(VersionedRouter::v2("agents"), "/api/v2/agents");
    }
}
