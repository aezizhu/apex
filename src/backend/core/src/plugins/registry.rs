//! Plugin discovery and registration.
//!
//! The [`PluginRegistry`] scans plugin directories, loads manifests,
//! validates them, and manages the lifecycle (install, enable, disable,
//! uninstall) of every registered plugin.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::manifest::{ManifestError, PluginManifest};
use super::sandbox::SandboxPolicy;

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin State
// ═══════════════════════════════════════════════════════════════════════════════

/// Runtime state of a plugin inside the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginState {
    /// Discovered on disk but not yet loaded.
    Discovered,
    /// Successfully loaded and ready to be enabled.
    Installed,
    /// Actively running.
    Enabled,
    /// Temporarily disabled by the operator.
    Disabled,
    /// An error occurred during loading or execution.
    Error(String),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Registered Plugin
// ═══════════════════════════════════════════════════════════════════════════════

/// A plugin that has been registered with the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredPlugin {
    /// The plugin manifest.
    pub manifest: PluginManifest,
    /// Current state.
    pub state: PluginState,
    /// Filesystem path to the plugin directory.
    pub path: PathBuf,
    /// Sandbox policy applied to this plugin.
    pub sandbox_policy: SandboxPolicy,
    /// When the plugin was first discovered.
    pub discovered_at: DateTime<Utc>,
    /// When the plugin was last state-changed.
    pub updated_at: DateTime<Utc>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Registry Errors
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),

    #[error("Manifest error: {0}")]
    Manifest(#[from] ManifestError),

    #[error("Invalid state transition for plugin '{name}': {from:?} -> {to:?}")]
    InvalidStateTransition {
        name: String,
        from: PluginState,
        to: PluginState,
    },

    #[error("Dependency not satisfied: plugin '{plugin}' requires '{dependency}'")]
    DependencyNotSatisfied { plugin: String, dependency: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Plugin Registry
// ═══════════════════════════════════════════════════════════════════════════════

/// Central registry that manages all plugins.
///
/// Thread-safe via interior `RwLock`.
#[derive(Debug, Clone)]
pub struct PluginRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

#[derive(Debug)]
struct RegistryInner {
    /// Map of plugin name -> registered plugin.
    plugins: HashMap<String, RegisteredPlugin>,
    /// Base directory where plugins are stored.
    plugins_dir: PathBuf,
}

impl PluginRegistry {
    /// Create a new registry rooted at `plugins_dir`.
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                plugins: HashMap::new(),
                plugins_dir: plugins_dir.into(),
            })),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Discovery
    // ─────────────────────────────────────────────────────────────────────────

    /// Scan the plugins directory and register any new plugins found.
    pub async fn discover(&self) -> Result<Vec<String>, RegistryError> {
        let plugins_dir = {
            let inner = self.inner.read().await;
            inner.plugins_dir.clone()
        };

        if !plugins_dir.exists() {
            info!(dir = %plugins_dir.display(), "Plugins directory does not exist, skipping discovery");
            return Ok(vec![]);
        }

        let mut discovered = Vec::new();
        let mut entries = tokio::fs::read_dir(&plugins_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            match PluginManifest::load_from_dir(&path) {
                Ok(manifest) => {
                    if let Err(e) = manifest.validate() {
                        warn!(
                            plugin_dir = %path.display(),
                            error = %e,
                            "Skipping plugin with invalid manifest"
                        );
                        continue;
                    }

                    let name = manifest.name.clone();
                    let mut inner = self.inner.write().await;
                    if inner.plugins.contains_key(&name) {
                        continue; // already registered
                    }

                    let now = Utc::now();
                    inner.plugins.insert(
                        name.clone(),
                        RegisteredPlugin {
                            manifest,
                            state: PluginState::Discovered,
                            path,
                            sandbox_policy: SandboxPolicy::default(),
                            discovered_at: now,
                            updated_at: now,
                        },
                    );
                    discovered.push(name);
                }
                Err(e) => {
                    warn!(
                        plugin_dir = %path.display(),
                        error = %e,
                        "Failed to load manifest from directory"
                    );
                }
            }
        }

        info!(count = discovered.len(), "Plugin discovery complete");
        Ok(discovered)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // CRUD Operations
    // ─────────────────────────────────────────────────────────────────────────

    /// List all registered plugins.
    pub async fn list(&self) -> Vec<RegisteredPlugin> {
        let inner = self.inner.read().await;
        inner.plugins.values().cloned().collect()
    }

    /// Get a single plugin by name.
    pub async fn get(&self, name: &str) -> Result<RegisteredPlugin, RegistryError> {
        let inner = self.inner.read().await;
        inner
            .plugins
            .get(name)
            .cloned()
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))
    }

    /// Install a plugin (transition from Discovered -> Installed).
    pub async fn install(&self, name: &str) -> Result<RegisteredPlugin, RegistryError> {
        let mut inner = self.inner.write().await;
        let plugin = inner
            .plugins
            .get_mut(name)
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))?;

        if plugin.state != PluginState::Discovered {
            return Err(RegistryError::InvalidStateTransition {
                name: name.to_string(),
                from: plugin.state.clone(),
                to: PluginState::Installed,
            });
        }

        // Check dependencies are satisfied.
        for dep in &plugin.manifest.dependencies {
            if dep.optional {
                continue;
            }
            if !inner.plugins.contains_key(&dep.name) {
                return Err(RegistryError::DependencyNotSatisfied {
                    plugin: name.to_string(),
                    dependency: dep.name.clone(),
                });
            }
        }

        plugin.state = PluginState::Installed;
        plugin.updated_at = Utc::now();
        info!(plugin = name, "Plugin installed");
        Ok(plugin.clone())
    }

    /// Enable a plugin (Installed | Disabled -> Enabled).
    pub async fn enable(&self, name: &str) -> Result<RegisteredPlugin, RegistryError> {
        let mut inner = self.inner.write().await;
        let plugin = inner
            .plugins
            .get_mut(name)
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))?;

        match &plugin.state {
            PluginState::Installed | PluginState::Disabled => {}
            other => {
                return Err(RegistryError::InvalidStateTransition {
                    name: name.to_string(),
                    from: other.clone(),
                    to: PluginState::Enabled,
                });
            }
        }

        plugin.state = PluginState::Enabled;
        plugin.updated_at = Utc::now();
        info!(plugin = name, "Plugin enabled");
        Ok(plugin.clone())
    }

    /// Disable a plugin (Enabled -> Disabled).
    pub async fn disable(&self, name: &str) -> Result<RegisteredPlugin, RegistryError> {
        let mut inner = self.inner.write().await;
        let plugin = inner
            .plugins
            .get_mut(name)
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))?;

        if plugin.state != PluginState::Enabled {
            return Err(RegistryError::InvalidStateTransition {
                name: name.to_string(),
                from: plugin.state.clone(),
                to: PluginState::Disabled,
            });
        }

        plugin.state = PluginState::Disabled;
        plugin.updated_at = Utc::now();
        info!(plugin = name, "Plugin disabled");
        Ok(plugin.clone())
    }

    /// Uninstall a plugin (remove from registry). The plugin must be Disabled or Installed.
    pub async fn uninstall(&self, name: &str) -> Result<RegisteredPlugin, RegistryError> {
        let mut inner = self.inner.write().await;
        let plugin = inner
            .plugins
            .get(name)
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))?;

        match &plugin.state {
            PluginState::Installed | PluginState::Disabled | PluginState::Discovered => {}
            other => {
                return Err(RegistryError::InvalidStateTransition {
                    name: name.to_string(),
                    from: other.clone(),
                    to: PluginState::Discovered, // conceptual target
                });
            }
        }

        let removed = inner.plugins.remove(name).unwrap();
        info!(plugin = name, "Plugin uninstalled");
        Ok(removed)
    }

    /// Update the sandbox policy for a plugin.
    pub async fn set_sandbox_policy(
        &self,
        name: &str,
        policy: SandboxPolicy,
    ) -> Result<(), RegistryError> {
        let mut inner = self.inner.write().await;
        let plugin = inner
            .plugins
            .get_mut(name)
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))?;
        plugin.sandbox_policy = policy;
        plugin.updated_at = Utc::now();
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn write_example_manifest(dir: &Path) {
        let toml = r#"
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
author = "Test"
capabilities = ["tool_execution"]
"#;
        fs::write(dir.join("plugin.toml"), toml).unwrap();
    }

    #[tokio::test]
    async fn test_discover_plugins() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("test-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        write_example_manifest(&plugin_dir);

        let registry = PluginRegistry::new(tmp.path());
        let discovered = registry.discover().await.unwrap();
        assert_eq!(discovered, vec!["test-plugin"]);
    }

    #[tokio::test]
    async fn test_lifecycle() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("test-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        write_example_manifest(&plugin_dir);

        let registry = PluginRegistry::new(tmp.path());
        registry.discover().await.unwrap();

        // Install
        let p = registry.install("test-plugin").await.unwrap();
        assert_eq!(p.state, PluginState::Installed);

        // Enable
        let p = registry.enable("test-plugin").await.unwrap();
        assert_eq!(p.state, PluginState::Enabled);

        // Disable
        let p = registry.disable("test-plugin").await.unwrap();
        assert_eq!(p.state, PluginState::Disabled);

        // Uninstall
        let p = registry.uninstall("test-plugin").await.unwrap();
        assert_eq!(p.state, PluginState::Disabled);

        // Verify removed
        assert!(registry.get("test-plugin").await.is_err());
    }

    #[tokio::test]
    async fn test_invalid_transition() {
        let tmp = TempDir::new().unwrap();
        let plugin_dir = tmp.path().join("test-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        write_example_manifest(&plugin_dir);

        let registry = PluginRegistry::new(tmp.path());
        registry.discover().await.unwrap();

        // Cannot enable a Discovered plugin directly
        assert!(registry.enable("test-plugin").await.is_err());
    }

    #[tokio::test]
    async fn test_plugin_not_found() {
        let tmp = TempDir::new().unwrap();
        let registry = PluginRegistry::new(tmp.path());
        assert!(registry.get("nonexistent").await.is_err());
    }
}
