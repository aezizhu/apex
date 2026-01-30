//! Plugin management API handlers (V1).
//!
//! Provides REST endpoints for listing, installing, enabling, disabling,
//! and uninstalling plugins via the plugin registry.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api::{ApiResponse, AppState};
use crate::plugins::{PluginRegistry, PluginState, RegisteredPlugin};

// ═══════════════════════════════════════════════════════════════════════════════
// DTOs
// ═══════════════════════════════════════════════════════════════════════════════

/// Summary view of a plugin returned in list responses.
#[derive(Debug, Serialize)]
pub struct PluginSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub state: PluginState,
    pub capabilities: Vec<String>,
}

impl From<&RegisteredPlugin> for PluginSummary {
    fn from(p: &RegisteredPlugin) -> Self {
        Self {
            name: p.manifest.name.clone(),
            version: p.manifest.version.clone(),
            description: p.manifest.description.clone(),
            author: p.manifest.author.clone(),
            state: p.state.clone(),
            capabilities: p
                .manifest
                .capabilities
                .iter()
                .map(|c| format!("{:?}", c))
                .collect(),
        }
    }
}

/// Detailed view of a single plugin.
#[derive(Debug, Serialize)]
pub struct PluginDetail {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: Option<String>,
    pub homepage: Option<String>,
    pub state: PluginState,
    pub capabilities: Vec<String>,
    pub permissions: Vec<String>,
    pub dependencies: Vec<PluginDepDto>,
    pub discovered_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PluginDepDto {
    pub name: String,
    pub version_req: String,
    pub optional: bool,
}

impl From<&RegisteredPlugin> for PluginDetail {
    fn from(p: &RegisteredPlugin) -> Self {
        Self {
            name: p.manifest.name.clone(),
            version: p.manifest.version.clone(),
            description: p.manifest.description.clone(),
            author: p.manifest.author.clone(),
            license: p.manifest.license.clone(),
            homepage: p.manifest.homepage.clone(),
            state: p.state.clone(),
            capabilities: p
                .manifest
                .capabilities
                .iter()
                .map(|c| format!("{:?}", c))
                .collect(),
            permissions: p
                .manifest
                .permissions
                .iter()
                .map(|perm| format!("{:?}", perm))
                .collect(),
            dependencies: p
                .manifest
                .dependencies
                .iter()
                .map(|d| PluginDepDto {
                    name: d.name.clone(),
                    version_req: d.version_req.clone(),
                    optional: d.optional,
                })
                .collect(),
            discovered_at: p.discovered_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// `GET /api/v1/plugins` - List all registered plugins.
pub async fn list_plugins(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let registry = state.plugin_registry();
    let plugins = registry.list().await;
    let summaries: Vec<PluginSummary> = plugins.iter().map(PluginSummary::from).collect();
    Json(ApiResponse::success(summaries))
}

/// `GET /api/v1/plugins/:name` - Get plugin details.
pub async fn get_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let registry = state.plugin_registry();
    match registry.get(&name).await {
        Ok(plugin) => Json(ApiResponse::success(PluginDetail::from(&plugin))),
        Err(_) => Json(ApiResponse::<PluginDetail>::error(format!(
            "Plugin not found: {}",
            name
        ))),
    }
}

/// `POST /api/v1/plugins/:name/install` - Install a discovered plugin.
pub async fn install_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let registry = state.plugin_registry();
    match registry.install(&name).await {
        Ok(plugin) => Json(ApiResponse::success(PluginSummary::from(&plugin))),
        Err(e) => Json(ApiResponse::<PluginSummary>::error(e.to_string())),
    }
}

/// `POST /api/v1/plugins/:name/enable` - Enable an installed plugin.
pub async fn enable_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let registry = state.plugin_registry();
    match registry.enable(&name).await {
        Ok(plugin) => Json(ApiResponse::success(PluginSummary::from(&plugin))),
        Err(e) => Json(ApiResponse::<PluginSummary>::error(e.to_string())),
    }
}

/// `POST /api/v1/plugins/:name/disable` - Disable an enabled plugin.
pub async fn disable_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let registry = state.plugin_registry();
    match registry.disable(&name).await {
        Ok(plugin) => Json(ApiResponse::success(PluginSummary::from(&plugin))),
        Err(e) => Json(ApiResponse::<PluginSummary>::error(e.to_string())),
    }
}

/// `POST /api/v1/plugins/:name/uninstall` - Uninstall a plugin.
pub async fn uninstall_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let registry = state.plugin_registry();
    match registry.uninstall(&name).await {
        Ok(plugin) => Json(ApiResponse::success(PluginSummary::from(&plugin))),
        Err(e) => Json(ApiResponse::<PluginSummary>::error(e.to_string())),
    }
}

/// `POST /api/v1/plugins/discover` - Trigger plugin discovery scan.
pub async fn discover_plugins(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let registry = state.plugin_registry();
    match registry.discover().await {
        Ok(names) => Json(ApiResponse::success(serde_json::json!({
            "discovered": names,
            "count": names.len(),
        }))),
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
    }
}
