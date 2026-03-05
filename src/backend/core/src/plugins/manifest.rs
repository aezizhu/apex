//! Plugin manifest parsing and validation.
//!
//! Supports both TOML and JSON manifest formats for plugin metadata.
//! Every plugin must include a manifest that declares its name, version,
//! author, capabilities, and any dependencies on other plugins.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ═══════════════════════════════════════════════════════════════════════════════
// Capability
// ═══════════════════════════════════════════════════════════════════════════════

/// A capability that a plugin provides or requires.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    /// Plugin can execute tool calls.
    ToolExecution,
    /// Plugin provides a new model routing strategy.
    ModelRouting,
    /// Plugin provides observability / tracing hooks.
    Observability,
    /// Plugin adds API endpoints.
    ApiExtension,
    /// Plugin provides agent behaviors.
    AgentBehavior,
    /// Plugin provides data transformation.
    DataTransform,
    /// Plugin provides authentication / authorization.
    Auth,
    /// Custom capability described by a string.
    Custom(String),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Dependency
// ═══════════════════════════════════════════════════════════════════════════════

/// A dependency on another plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    /// Name of the required plugin.
    pub name: String,
    /// Semantic version requirement (e.g. ">=1.0.0, <2.0.0").
    pub version_req: String,
    /// Whether this dependency is optional.
    #[serde(default)]
    pub optional: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Permission
// ═══════════════════════════════════════════════════════════════════════════════

/// Permissions a plugin requests from the host runtime.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    /// Access the network.
    Network,
    /// Read from the filesystem.
    FileRead,
    /// Write to the filesystem.
    FileWrite,
    /// Access environment variables.
    Environment,
    /// Spawn sub-processes.
    Process,
    /// Access the database.
    Database,
}

// ═══════════════════════════════════════════════════════════════════════════════
// PluginManifest
// ═══════════════════════════════════════════════════════════════════════════════

/// Full manifest for a plugin, typically stored as `plugin.toml` or `plugin.json`
/// at the root of the plugin directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin name (e.g. "apex-hello-world").
    pub name: String,

    /// Semantic version string.
    pub version: String,

    /// Human-readable description.
    #[serde(default)]
    pub description: String,

    /// Author name or organisation.
    #[serde(default)]
    pub author: String,

    /// SPDX license identifier.
    #[serde(default)]
    pub license: Option<String>,

    /// Plugin homepage / repository URL.
    #[serde(default)]
    pub homepage: Option<String>,

    /// Capabilities this plugin provides.
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,

    /// Dependencies on other plugins.
    #[serde(default)]
    pub dependencies: Vec<PluginDependency>,

    /// Permissions the plugin requests.
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,

    /// Minimum Apex core version required.
    #[serde(default)]
    pub min_apex_version: Option<String>,

    /// Entry point (e.g. shared library filename or WASM module).
    #[serde(default)]
    pub entry_point: Option<String>,

    /// Arbitrary metadata.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl PluginManifest {
    // ─────────────────────────────────────────────────────────────────────────
    // Parsing helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Parse a manifest from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, ManifestError> {
        toml::from_str(toml_str).map_err(|e| ManifestError::ParseError {
            format: "TOML".into(),
            details: e.to_string(),
        })
    }

    /// Parse a manifest from a JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(json_str).map_err(|e| ManifestError::ParseError {
            format: "JSON".into(),
            details: e.to_string(),
        })
    }

    /// Load a manifest from a directory, looking for `plugin.toml` then `plugin.json`.
    pub fn load_from_dir(dir: &Path) -> Result<Self, ManifestError> {
        let toml_path = dir.join("plugin.toml");
        if toml_path.exists() {
            let content = std::fs::read_to_string(&toml_path).map_err(|e| {
                ManifestError::IoError(format!("Failed to read {}: {}", toml_path.display(), e))
            })?;
            return Self::from_toml(&content);
        }

        let json_path = dir.join("plugin.json");
        if json_path.exists() {
            let content = std::fs::read_to_string(&json_path).map_err(|e| {
                ManifestError::IoError(format!("Failed to read {}: {}", json_path.display(), e))
            })?;
            return Self::from_json(&content);
        }

        Err(ManifestError::NotFound {
            dir: dir.display().to_string(),
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Validation
    // ─────────────────────────────────────────────────────────────────────────

    /// Validate the manifest fields.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.name.is_empty() {
            return Err(ManifestError::ValidationError("name must not be empty".into()));
        }

        // Enforce naming convention: lowercase alphanumeric + hyphens.
        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ManifestError::ValidationError(
                "name must contain only lowercase alphanumeric characters and hyphens".into(),
            ));
        }

        if self.version.is_empty() {
            return Err(ManifestError::ValidationError("version must not be empty".into()));
        }

        // Basic semver check (major.minor.patch).
        let parts: Vec<&str> = self.version.split('.').collect();
        if parts.len() != 3 || !parts.iter().all(|p| p.parse::<u32>().is_ok()) {
            return Err(ManifestError::ValidationError(
                "version must follow semver (e.g. 1.0.0)".into(),
            ));
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors that can occur when working with plugin manifests.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("Manifest not found in directory: {dir}")]
    NotFound { dir: String },

    #[error("Failed to parse {format} manifest: {details}")]
    ParseError { format: String, details: String },

    #[error("Manifest validation error: {0}")]
    ValidationError(String),

    #[error("IO error: {0}")]
    IoError(String),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_toml_manifest() {
        let toml = r#"
name = "example-plugin"
version = "1.0.0"
description = "An example plugin"
author = "Apex Team"
capabilities = ["tool_execution"]
"#;
        let manifest = PluginManifest::from_toml(toml).unwrap();
        assert_eq!(manifest.name, "example-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.capabilities.len(), 1);
    }

    #[test]
    fn test_parse_json_manifest() {
        let json = r#"{
            "name": "json-plugin",
            "version": "0.1.0",
            "description": "JSON manifest test",
            "author": "Test"
        }"#;
        let manifest = PluginManifest::from_json(json).unwrap();
        assert_eq!(manifest.name, "json-plugin");
    }

    #[test]
    fn test_validate_empty_name() {
        let manifest = PluginManifest {
            name: "".into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: String::new(),
            license: None,
            homepage: None,
            capabilities: vec![],
            dependencies: vec![],
            permissions: vec![],
            min_apex_version: None,
            entry_point: None,
            metadata: HashMap::new(),
        };
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_validate_bad_version() {
        let manifest = PluginManifest {
            name: "good-name".into(),
            version: "not-semver".into(),
            description: String::new(),
            author: String::new(),
            license: None,
            homepage: None,
            capabilities: vec![],
            dependencies: vec![],
            permissions: vec![],
            min_apex_version: None,
            entry_point: None,
            metadata: HashMap::new(),
        };
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_validate_valid_manifest() {
        let manifest = PluginManifest {
            name: "my-plugin".into(),
            version: "2.1.0".into(),
            description: "desc".into(),
            author: "author".into(),
            license: None,
            homepage: None,
            capabilities: vec![PluginCapability::ToolExecution],
            dependencies: vec![],
            permissions: vec![],
            min_apex_version: None,
            entry_point: None,
            metadata: HashMap::new(),
        };
        assert!(manifest.validate().is_ok());
    }
}
