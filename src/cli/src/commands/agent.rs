//! Agent management commands.
//!
//! Provides list, inspect, stop, and logs operations for agents.

use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use tabled::Tabled;
use uuid::Uuid;

use crate::client::ApiClient;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum AgentCommands {
    /// List agents in a swarm or across all swarms
    List {
        /// Filter by swarm name or ID
        #[arg(short, long)]
        swarm: Option<String>,

        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },

    /// Inspect a specific agent
    Inspect {
        /// Agent ID
        agent_id: Uuid,
    },

    /// Stop (remove) an agent
    Stop {
        /// Agent ID
        agent_id: Uuid,

        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Show agent logs / statistics
    Logs {
        /// Agent ID
        agent_id: Uuid,

        /// Number of recent entries to show
        #[arg(short, long, default_value = "20")]
        tail: u32,
    },
}

// ── API response types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct AgentInfo {
    id: Uuid,
    name: String,
    model: String,
    status: String,
    current_load: i32,
    max_load: i32,
    #[serde(default)]
    success_rate: f64,
    #[serde(default)]
    reputation_score: f64,
}

#[derive(Debug, Deserialize, Serialize, Tabled)]
struct AgentRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Model")]
    model: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Load")]
    load: String,
    #[tabled(rename = "Success Rate")]
    success_rate: String,
    #[tabled(rename = "Reputation")]
    reputation: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct AgentDetail {
    id: Uuid,
    name: String,
    model: String,
    #[serde(default)]
    system_prompt: Option<String>,
    status: String,
    current_load: i32,
    max_load: i32,
    #[serde(default)]
    success_count: u64,
    #[serde(default)]
    failure_count: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    total_cost: f64,
    #[serde(default)]
    reputation_score: f64,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    last_active_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AgentStats {
    id: Uuid,
    name: String,
    #[serde(default)]
    tasks_completed: u64,
    #[serde(default)]
    success_count: u64,
    #[serde(default)]
    failure_count: u64,
    #[serde(default)]
    success_rate: f64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    total_cost: f64,
    #[serde(default)]
    avg_latency_ms: f64,
    #[serde(default)]
    reputation_score: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct RemoveResponse {
    id: Uuid,
    status: String,
}

// ── Execution ───────────────────────────────────────────────────────────────

pub async fn execute(cmd: AgentCommands, client: &ApiClient, format: OutputFormat) -> Result<()> {
    match cmd {
        AgentCommands::List { swarm, limit } => {
            let path = match &swarm {
                Some(s) => format!("/api/v1/agents?swarm={}&limit={}", s, limit),
                None => format!("/api/v1/agents?limit={}", limit),
            };

            let agents: Vec<AgentInfo> = client.get(&path).await?;

            let rows: Vec<AgentRow> = agents
                .into_iter()
                .map(|a| AgentRow {
                    id: a.id.to_string()[..8].to_string(),
                    name: a.name,
                    model: a.model,
                    status: a.status,
                    load: format!("{}/{}", a.current_load, a.max_load),
                    success_rate: format!("{:.1}%", a.success_rate * 100.0),
                    reputation: format!("{:.2}", a.reputation_score),
                })
                .collect();

            output::print_list(&rows, format);
        }

        AgentCommands::Inspect { agent_id } => {
            let agent: AgentDetail =
                client.get(&format!("/api/v1/agents/{}", agent_id)).await?;

            match format {
                OutputFormat::Table => {
                    output::print_header(&format!("Agent: {}", agent.name));
                    output::print_detail("ID", &agent.id.to_string());
                    output::print_detail("Model", &agent.model);
                    output::print_detail("Status", &agent.status);
                    output::print_detail(
                        "Load",
                        &format!("{}/{}", agent.current_load, agent.max_load),
                    );
                    output::print_detail("Success", &agent.success_count.to_string());
                    output::print_detail("Failures", &agent.failure_count.to_string());
                    output::print_detail("Total Tokens", &agent.total_tokens.to_string());
                    output::print_detail("Total Cost", &format!("${:.4}", agent.total_cost));
                    output::print_detail(
                        "Reputation",
                        &format!("{:.2}", agent.reputation_score),
                    );
                    if let Some(created) = &agent.created_at {
                        output::print_detail("Created", created);
                    }
                    if let Some(active) = &agent.last_active_at {
                        output::print_detail("Last Active", active);
                    }
                }
                _ => output::print_item(&agent, format),
            }
        }

        AgentCommands::Stop { agent_id, force } => {
            if !force {
                output::print_info(
                    "This will remove the agent. Use --force to skip confirmation.",
                );
                return Ok(());
            }

            let resp: RemoveResponse = client
                .delete(&format!("/api/v1/agents/{}", agent_id))
                .await?;

            match format {
                OutputFormat::Table => {
                    output::print_success(&format!("Agent {} stopped", agent_id));
                }
                _ => output::print_item(&resp, format),
            }
        }

        AgentCommands::Logs { agent_id, tail } => {
            let stats: AgentStats = client
                .get(&format!(
                    "/api/v1/agents/{}/stats?tail={}",
                    agent_id, tail
                ))
                .await?;

            match format {
                OutputFormat::Table => {
                    output::print_header(&format!("Agent Stats: {}", stats.name));
                    output::print_detail("Tasks Completed", &stats.tasks_completed.to_string());
                    output::print_detail("Success", &stats.success_count.to_string());
                    output::print_detail("Failures", &stats.failure_count.to_string());
                    output::print_detail(
                        "Success Rate",
                        &format!("{:.1}%", stats.success_rate * 100.0),
                    );
                    output::print_detail("Total Tokens", &stats.total_tokens.to_string());
                    output::print_detail("Total Cost", &format!("${:.4}", stats.total_cost));
                    output::print_detail(
                        "Avg Latency",
                        &format!("{:.1}ms", stats.avg_latency_ms),
                    );
                    output::print_detail(
                        "Reputation",
                        &format!("{:.2}", stats.reputation_score),
                    );
                }
                _ => output::print_item(&stats, format),
            }
        }
    }

    Ok(())
}
