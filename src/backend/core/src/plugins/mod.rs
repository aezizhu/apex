//! # Plugin Marketplace
//!
//! A plugin system for extending the Apex agent swarm orchestration engine.
//!
//! ## Architecture
//!
//! - **Manifest**: Declarative metadata (name, version, capabilities, dependencies)
//!   parsed from `plugin.toml` or `plugin.json` inside each plugin directory.
//! - **Registry**: Discovery, installation, enabling/disabling, and uninstallation
//!   of plugins with full lifecycle state management.
//! - **Sandbox**: Permission and resource enforcement layer that constrains plugin
//!   execution (network access, filesystem, memory, execution time).
//! - **Plugin trait**: The interface every plugin must implement to participate in
//!   the Apex runtime.
//! - **PluginLoader**: Dynamic loading of plugin implementations from disk.
//!
//! ## Directory Layout
//!
//! ```text
//! plugins/
//! +-- example-hello/
//! |   +-- plugin.toml
//! +-- my-custom-tool/
//!     +-- plugin.toml
//! ```

pub mod manifest;
pub mod registry;
pub mod sandbox;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use manifest::PluginManifest;
use registry::PluginRegistry;
use sandbox::SandboxContext;

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin Trait
// ═══════════════════════════════════════════════════════════════════════════════

/// The core trait every Apex plugin must implement.
///
/// Plugins are loaded by the runtime and invoked through this interface.
/// The sandbox context is provided to enforce resource and permission limits.
#[async_trait]
pub trait Plugin: Send + Sync + std::fmt::Debug {
    /// Unique plugin name (must match the manifest).
    fn name(&self) -> &str;

    /// Plugin version string.
    fn version(&self) -> &str;

    /// Short description of what the plugin does.
    fn description(&self) -> &str;

    /// Called once when the plugin is loaded. Use for setup / initialisation.
    async fn on_load(&self) -> Result<(), PluginError> {
        Ok(())
    }

    /// Called once when the plugin is about to be unloaded. Use for cleanup.
    async fn on_unload(&self) -> Result<(), PluginError> {
        Ok(())
    }

    /// Execute the plugin's main logic with the provided input.
    ///
    /// The `sandbox` context must be used for any resource-consuming operations
    /// (network requests, file I/O, memory allocations) so that limits are
    /// enforced consistently.
    async fn execute(
        &self,
        input: PluginInput,
        sandbox: &mut SandboxContext,
    ) -> Result<PluginOutput, PluginError>;
}

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin I/O Types
// ═══════════════════════════════════════════════════════════════════════════════

/// Input passed to a plugin's `execute` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInput {
    /// The action the plugin should perform.
    pub action: String,
    /// Arbitrary parameters as JSON.
    pub parameters: serde_json::Value,
}

/// Output returned from a plugin's `execute` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOutput {
    /// Whether execution succeeded.
    pub success: bool,
    /// Arbitrary result payload.
    pub data: serde_json::Value,
    /// Optional human-readable message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl PluginOutput {
    /// Convenience constructor for a successful result.
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    /// Convenience constructor for a failure result.
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            message: Some(message.into()),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin Error
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors produced by plugin operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Plugin initialization failed: {0}")]
    InitFailed(String),

    #[error("Sandbox violation: {0}")]
    SandboxViolation(#[from] sandbox::SandboxViolation),

    #[error("Registry error: {0}")]
    Registry(#[from] registry::RegistryError),

    #[error("Manifest error: {0}")]
    Manifest(#[from] manifest::ManifestError),

    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin Loader
// ═══════════════════════════════════════════════════════════════════════════════

/// Loads plugin implementations from disk.
///
/// In this initial scaffolding the loader validates the manifest and returns
/// metadata. Dynamic library / WASM loading will be added in a future
/// iteration.
pub struct PluginLoader {
    plugins_dir: PathBuf,
}

impl PluginLoader {
    /// Create a loader for the given plugins directory.
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
        }
    }

    /// Validate that a plugin directory contains a well-formed manifest.
    pub fn validate(&self, plugin_name: &str) -> Result<PluginManifest, PluginError> {
        let dir = self.plugins_dir.join(plugin_name);
        let manifest = PluginManifest::load_from_dir(&dir)?;
        manifest.validate().map_err(PluginError::Manifest)?;
        Ok(manifest)
    }

    /// List all valid plugin directories.
    pub fn list_available(&self) -> Result<Vec<PluginManifest>, PluginError> {
        let mut manifests = Vec::new();

        if !self.plugins_dir.exists() {
            return Ok(manifests);
        }

        let entries = std::fs::read_dir(&self.plugins_dir)?;
        for entry in entries {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }
            match PluginManifest::load_from_dir(&entry.path()) {
                Ok(m) if m.validate().is_ok() => manifests.push(m),
                _ => continue,
            }
        }
        Ok(manifests)
    }

    /// Get the plugins base directory.
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Re-exports for convenience
// ═══════════════════════════════════════════════════════════════════════════════

pub use manifest::{PluginCapability, PluginDependency, PluginManifest, PluginPermission};
pub use registry::{PluginRegistry, PluginState, RegisteredPlugin};
pub use sandbox::{SandboxContext, SandboxPolicy, SandboxViolation};

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_output_ok() {
        let output = PluginOutput::ok(serde_json::json!({"result": 42}));
        assert!(output.success);
        assert!(output.message.is_none());
    }

    #[test]
    fn test_plugin_output_err() {
        let output = PluginOutput::err("something went wrong");
        assert!(!output.success);
        assert_eq!(output.message, Some("something went wrong".into()));
    }

    #[test]
    fn test_loader_validate_missing() {
        let loader = PluginLoader::new("/nonexistent/path");
        assert!(loader.validate("ghost-plugin").is_err());
    }
}
