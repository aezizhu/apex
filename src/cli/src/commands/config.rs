//! Configuration management commands.
//!
//! Stores CLI configuration in `~/.apex/config.toml`.

use anyhow::{Context, Result};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., api-url)
        key: String,
        /// Value to set
        value: String,
    },

    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Show all configuration
    Show,

    /// Reset configuration to defaults
    Reset {
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

/// Persistent CLI configuration stored on disk.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

/// Return the path to the configuration file (`~/.apex/config.toml`).
fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".apex").join("config.toml"))
}

/// Load the CLI configuration from disk, returning defaults if the file does
/// not exist.
fn load_config() -> Result<CliConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(CliConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let cfg: CliConfig =
        toml::from_str(&content).with_context(|| "Failed to parse config file")?;
    Ok(cfg)
}

/// Save the CLI configuration to disk, creating the directory if needed.
fn save_config(cfg: &CliConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let content =
        toml::to_string_pretty(cfg).context("Failed to serialize config")?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Load the `api-url` value from the config file, if set.
pub fn load_api_url() -> Option<String> {
    load_config()
        .ok()
        .and_then(|cfg| cfg.values.get("api-url").cloned())
}

pub async fn execute(cmd: ConfigCommands, format: OutputFormat) -> Result<()> {
    match cmd {
        ConfigCommands::Set { key, value } => {
            let mut cfg = load_config()?;
            cfg.values.insert(key.clone(), value.clone());
            save_config(&cfg)?;

            match format {
                OutputFormat::Table => {
                    output::print_success(&format!("{} = {}", key, value));
                }
                _ => {
                    output::print_item(
                        &serde_json::json!({ "key": key, "value": value }),
                        format,
                    );
                }
            }
        }

        ConfigCommands::Get { key } => {
            let cfg = load_config()?;
            match cfg.values.get(&key) {
                Some(value) => match format {
                    OutputFormat::Table => println!("{}", value),
                    _ => {
                        output::print_item(
                            &serde_json::json!({ "key": key, "value": value }),
                            format,
                        );
                    }
                },
                None => {
                    output::print_error(&format!("Key '{}' not found", key));
                }
            }
        }

        ConfigCommands::Show => {
            let cfg = load_config()?;

            if cfg.values.is_empty() {
                output::print_info("No configuration values set.");
                return Ok(());
            }

            match format {
                OutputFormat::Table => {
                    output::print_header("Configuration");
                    for (k, v) in &cfg.values {
                        output::print_detail(k, v);
                    }
                }
                _ => output::print_item(&cfg.values, format),
            }
        }

        ConfigCommands::Reset { force } => {
            if !force {
                output::print_info(
                    "This will reset all CLI configuration. Use --force to confirm.",
                );
                return Ok(());
            }

            let path = config_path()?;
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("Failed to remove {}", path.display()))?;
            }

            output::print_success("Configuration reset to defaults");
        }
    }

    Ok(())
}
