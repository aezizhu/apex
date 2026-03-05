//! Apex CLI - Command-line interface for managing Apex agent swarms.
//!
//! Provides commands for swarm, agent, task, health, and configuration management.

mod client;
mod commands;
mod output;

use anyhow::Result;
use clap::{Parser, Subcommand};

use commands::{agent, config, health, swarm, task};
use output::OutputFormat;

/// Apex - Agent Swarm Orchestration Engine CLI
#[derive(Parser)]
#[command(
    name = "apex",
    author = "Aezi <aezi.zhu@icloud.com>",
    version = "0.1.0",
    about = "Apex - Agent Swarm Orchestration Engine",
    long_about = "CLI tool for managing Apex agent swarms, tasks, and system configuration.",
    propagate_version = true
)]
pub struct Cli {
    /// Output format
    #[arg(short, long, global = true, default_value = "table")]
    output: OutputFormat,

    /// API server URL
    #[arg(long, global = true, env = "APEX_API_URL")]
    api_url: Option<String>,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Swarm management operations
    #[command(subcommand)]
    Swarm(swarm::SwarmCommands),

    /// Agent management operations
    #[command(subcommand)]
    Agent(agent::AgentCommands),

    /// Task management operations
    #[command(subcommand)]
    Task(task::TaskCommands),

    /// Check system health
    Health(health::HealthArgs),

    /// Configuration management
    #[command(subcommand)]
    Config(config::ConfigCommands),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_color {
        colored::control::set_override(false);
    }

    let api_url = cli
        .api_url
        .clone()
        .or_else(|| config::load_api_url())
        .unwrap_or_else(|| "http://localhost:8080".to_string());

    let client = client::ApiClient::new(&api_url)?;
    let format = cli.output;

    let result = match cli.command {
        Commands::Swarm(cmd) => swarm::execute(cmd, &client, format).await,
        Commands::Agent(cmd) => agent::execute(cmd, &client, format).await,
        Commands::Task(cmd) => task::execute(cmd, &client, format).await,
        Commands::Health(args) => health::execute(args, &client, format).await,
        Commands::Config(cmd) => config::execute(cmd, format).await,
    };

    if let Err(e) = result {
        output::print_error(&format!("{:#}", e));
        std::process::exit(1);
    }

    Ok(())
}
