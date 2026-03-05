//! Swarm management commands.
//!
//! Provides create, list, delete, and status operations for agent swarms.

use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use tabled::Tabled;
use uuid::Uuid;

use crate::client::ApiClient;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum SwarmCommands {
    /// Create a new agent swarm
    Create {
        /// Swarm name
        #[arg(short, long)]
        name: String,

        /// Number of agents to provision
        #[arg(short, long, default_value = "5")]
        agents: u32,

        /// Model to use for agents
        #[arg(short, long, default_value = "gpt-4o-mini")]
        model: String,
    },

    /// List all swarms
    List {
        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },

    /// Delete a swarm
    Delete {
        /// Swarm name or ID
        swarm: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Show swarm status
    Status {
        /// Swarm name or ID
        swarm: String,
    },
}

// ── API request / response types ────────────────────────────────────────────

#[derive(Serialize)]
struct CreateSwarmRequest {
    name: String,
    agent_count: u32,
    model: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SwarmInfo {
    id: Uuid,
    name: String,
    #[serde(default)]
    agent_count: u32,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Deserialize, Serialize, Tabled)]
struct SwarmRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Agents")]
    agent_count: u32,
    #[tabled(rename = "Status")]
    status: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SwarmStatus {
    id: Uuid,
    name: String,
    status: String,
    #[serde(default)]
    active_agents: u32,
    #[serde(default)]
    total_agents: u32,
    #[serde(default)]
    tasks_completed: u64,
    #[serde(default)]
    tasks_running: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct DeleteResponse {
    #[serde(default)]
    id: Uuid,
    #[serde(default)]
    status: String,
}

// ── Execution ───────────────────────────────────────────────────────────────

pub async fn execute(cmd: SwarmCommands, client: &ApiClient, format: OutputFormat) -> Result<()> {
    match cmd {
        SwarmCommands::Create {
            name,
            agents,
            model,
        } => {
            let body = CreateSwarmRequest {
                name: name.clone(),
                agent_count: agents,
                model: model.clone(),
            };

            let resp: SwarmInfo = client.post("/api/v1/swarms", &body).await?;

            match format {
                OutputFormat::Table => {
                    output::print_success(&format!("Swarm '{}' created", name));
                    output::print_detail("ID", &resp.id.to_string());
                    output::print_detail("Agents", &agents.to_string());
                    output::print_detail("Model", &model);
                }
                _ => output::print_item(&resp, format),
            }
        }

        SwarmCommands::List { limit } => {
            let swarms: Vec<SwarmInfo> =
                client.get(&format!("/api/v1/swarms?limit={}", limit)).await?;

            let rows: Vec<SwarmRow> = swarms
                .into_iter()
                .map(|s| SwarmRow {
                    id: s.id.to_string()[..8].to_string(),
                    name: s.name,
                    agent_count: s.agent_count,
                    status: s.status,
                })
                .collect();

            output::print_list(&rows, format);
        }

        SwarmCommands::Delete { swarm, force } => {
            if !force {
                output::print_info(&format!(
                    "This will permanently delete swarm '{}'. Use --force to skip confirmation.",
                    swarm
                ));
                return Ok(());
            }

            let resp: DeleteResponse =
                client.delete(&format!("/api/v1/swarms/{}", swarm)).await?;

            match format {
                OutputFormat::Table => {
                    output::print_success(&format!("Swarm '{}' deleted", swarm));
                }
                _ => output::print_item(&resp, format),
            }
        }

        SwarmCommands::Status { swarm } => {
            let status: SwarmStatus =
                client.get(&format!("/api/v1/swarms/{}/status", swarm)).await?;

            match format {
                OutputFormat::Table => {
                    output::print_header(&format!("Swarm: {}", status.name));
                    output::print_detail("ID", &status.id.to_string());
                    output::print_detail("Status", &status.status);
                    output::print_detail(
                        "Agents",
                        &format!("{}/{}", status.active_agents, status.total_agents),
                    );
                    output::print_detail("Tasks Running", &status.tasks_running.to_string());
                    output::print_detail("Tasks Completed", &status.tasks_completed.to_string());
                }
                _ => output::print_item(&status, format),
            }
        }
    }

    Ok(())
}
