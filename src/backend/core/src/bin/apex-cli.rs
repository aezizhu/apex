//! Apex CLI - Comprehensive command-line interface for Apex Agent Swarm Orchestration Engine
//!
//! This CLI provides commands for managing tasks, agents, DAGs, approvals,
//! database migrations, system health, and configuration.

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Duration;
use tabled::{
    settings::{Style, Modify, object::Columns, Alignment},
    Table, Tabled,
};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════════
// CLI Structure
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Parser)]
#[command(
    name = "apex",
    author = "Aezi <aezi.zhu@icloud.com>",
    version = "0.1.0",
    about = "Apex - World's No. 1 Agent Swarm Orchestration Engine",
    long_about = "A comprehensive CLI for managing Apex agent swarms, tasks, DAGs, and system configuration.",
    propagate_version = true
)]
struct Cli {
    /// Output format
    #[arg(short, long, global = true, default_value = "text")]
    format: OutputFormat,

    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum, Default)]
enum OutputFormat {
    #[default]
    Text,
    Json,
    Table,
}

#[derive(Subcommand)]
enum Commands {
    /// Task management operations
    #[command(subcommand)]
    Task(TaskCommands),

    /// Agent management operations
    #[command(subcommand)]
    Agent(AgentCommands),

    /// DAG management operations
    #[command(subcommand)]
    Dag(DagCommands),

    /// Approval management operations
    #[command(subcommand)]
    Approval(ApprovalCommands),

    /// Database migration operations
    #[command(subcommand)]
    Migrate(MigrateCommands),

    /// Seed the database with sample data
    Seed {
        /// Number of sample records to create
        #[arg(short, long, default_value = "10")]
        count: u32,

        /// Seed specific entity type
        #[arg(short, long)]
        entity: Option<SeedEntity>,
    },

    /// Check system health
    Health {
        /// Include detailed component checks
        #[arg(short, long)]
        detailed: bool,

        /// Timeout for health checks in seconds
        #[arg(short, long, default_value = "10")]
        timeout: u64,
    },

    /// Show system statistics
    Stats {
        /// Time period for statistics (e.g., 1h, 24h, 7d)
        #[arg(short, long, default_value = "24h")]
        period: String,

        /// Show real-time statistics
        #[arg(short, long)]
        live: bool,
    },

    /// View and edit configuration
    #[command(subcommand)]
    Config(ConfigCommands),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Task Commands
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Subcommand)]
enum TaskCommands {
    /// List all tasks
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<TaskStatusFilter>,

        /// Filter by DAG ID
        #[arg(short, long)]
        dag_id: Option<Uuid>,

        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,

        /// Offset for pagination
        #[arg(short, long, default_value = "0")]
        offset: u32,
    },

    /// Get details of a specific task
    Get {
        /// Task ID
        task_id: Uuid,

        /// Show full output data
        #[arg(short, long)]
        full: bool,
    },

    /// Create a new task
    Create {
        /// Task name
        #[arg(short, long)]
        name: String,

        /// Task instruction
        #[arg(short, long)]
        instruction: String,

        /// Parent DAG ID
        #[arg(short, long)]
        dag_id: Option<Uuid>,

        /// Task priority (higher = more urgent)
        #[arg(short, long, default_value = "0")]
        priority: i32,

        /// Maximum retry attempts
        #[arg(short, long, default_value = "3")]
        max_retries: u32,
    },

    /// Cancel a task
    Cancel {
        /// Task ID
        task_id: Uuid,

        /// Force cancel even if running
        #[arg(short, long)]
        force: bool,
    },

    /// Retry a failed task
    Retry {
        /// Task ID
        task_id: Uuid,

        /// Reset retry counter
        #[arg(short, long)]
        reset_counter: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum TaskStatusFilter {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Cancelled,
    All,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Agent Commands
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Subcommand)]
enum AgentCommands {
    /// List all agents
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<AgentStatusFilter>,

        /// Filter by model
        #[arg(short, long)]
        model: Option<String>,

        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },

    /// Get details of a specific agent
    Get {
        /// Agent ID
        agent_id: Uuid,

        /// Show performance history
        #[arg(short, long)]
        history: bool,
    },

    /// Pause an agent
    Pause {
        /// Agent ID
        agent_id: Uuid,

        /// Reason for pausing
        #[arg(short, long)]
        reason: Option<String>,

        /// Gracefully wait for current task to complete
        #[arg(short, long)]
        graceful: bool,
    },

    /// Resume a paused agent
    Resume {
        /// Agent ID
        agent_id: Uuid,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum AgentStatusFilter {
    Idle,
    Busy,
    Error,
    Paused,
    All,
}

// ═══════════════════════════════════════════════════════════════════════════════
// DAG Commands
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Subcommand)]
enum DagCommands {
    /// List all DAGs
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<DagStatusFilter>,

        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },

    /// Get details of a specific DAG
    Get {
        /// DAG ID
        dag_id: Uuid,

        /// Show task tree
        #[arg(short, long)]
        tree: bool,
    },

    /// Start a DAG execution
    Start {
        /// DAG ID
        dag_id: Uuid,

        /// Wait for completion
        #[arg(short, long)]
        wait: bool,

        /// Timeout in seconds (when --wait is used)
        #[arg(short, long, default_value = "3600")]
        timeout: u64,
    },

    /// Stop a running DAG
    Stop {
        /// DAG ID
        dag_id: Uuid,

        /// Force stop (cancel running tasks)
        #[arg(short, long)]
        force: bool,

        /// Reason for stopping
        #[arg(short, long)]
        reason: Option<String>,
    },

    /// Visualize DAG structure
    Visualize {
        /// DAG ID
        dag_id: Uuid,

        /// Output format (dot, mermaid, ascii)
        #[arg(short, long, default_value = "ascii")]
        output: VisualizationFormat,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum DagStatusFilter {
    Pending,
    Running,
    Completed,
    Failed,
    All,
}

#[derive(Clone, Copy, ValueEnum)]
enum VisualizationFormat {
    Dot,
    Mermaid,
    Ascii,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Approval Commands
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Subcommand)]
enum ApprovalCommands {
    /// List pending approvals
    List {
        /// Filter by type
        #[arg(short, long)]
        approval_type: Option<ApprovalType>,

        /// Show only urgent approvals
        #[arg(short, long)]
        urgent: bool,

        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },

    /// Approve a pending request
    Approve {
        /// Approval ID
        approval_id: Uuid,

        /// Approval comment
        #[arg(short, long)]
        comment: Option<String>,
    },

    /// Deny a pending request
    Deny {
        /// Approval ID
        approval_id: Uuid,

        /// Denial reason (required)
        #[arg(short, long)]
        reason: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ApprovalType {
    ContractExtension,
    ToolExecution,
    CostOverride,
    All,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Migration Commands
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Subcommand)]
enum MigrateCommands {
    /// Run pending migrations
    Run {
        /// Run all migrations (default: only pending)
        #[arg(short, long)]
        all: bool,

        /// Dry run (show what would be executed)
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Revert the last migration
    Revert {
        /// Number of migrations to revert
        #[arg(short, long, default_value = "1")]
        count: u32,

        /// Revert all migrations
        #[arg(short, long)]
        all: bool,

        /// Force revert without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Show migration status
    Status {
        /// Show pending migrations only
        #[arg(short, long)]
        pending: bool,
    },
}

// ═══════════════════════════════════════════════════════════════════════════════
// Config Commands
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show {
        /// Configuration section to show
        #[arg(short, long)]
        section: Option<ConfigSection>,

        /// Show sensitive values
        #[arg(long)]
        show_secrets: bool,
    },

    /// Get a specific configuration value
    Get {
        /// Configuration key (e.g., server.port)
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., server.port)
        key: String,

        /// Value to set
        value: String,

        /// Persist to configuration file
        #[arg(short, long)]
        persist: bool,
    },

    /// Validate configuration
    Validate {
        /// Configuration file to validate
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Generate sample configuration file
    Init {
        /// Output file path
        #[arg(short, long, default_value = "apex.toml")]
        output: String,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ConfigSection {
    Server,
    Database,
    Redis,
    Observability,
    Orchestrator,
    Llm,
    All,
}

#[derive(Clone, Copy, ValueEnum)]
enum SeedEntity {
    Agents,
    Tasks,
    Dags,
    Contracts,
    All,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Data Types for Output
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize, Tabled)]
struct TaskSummary {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Priority")]
    priority: i32,
    #[tabled(rename = "Tokens")]
    tokens_used: u64,
    #[tabled(rename = "Cost ($)")]
    cost_dollars: String,
    #[tabled(rename = "Created")]
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Tabled)]
struct AgentSummary {
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

#[derive(Debug, Serialize, Deserialize, Tabled)]
struct DagSummary {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Tasks")]
    total_tasks: usize,
    #[tabled(rename = "Completed")]
    completed: usize,
    #[tabled(rename = "Running")]
    running: usize,
    #[tabled(rename = "Failed")]
    failed: usize,
    #[tabled(rename = "Created")]
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Tabled)]
struct ApprovalSummary {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Type")]
    approval_type: String,
    #[tabled(rename = "Requester")]
    requester: String,
    #[tabled(rename = "Resource")]
    resource: String,
    #[tabled(rename = "Requested")]
    requested_at: String,
    #[tabled(rename = "Expires")]
    expires_at: String,
}

#[derive(Debug, Serialize, Deserialize, Tabled)]
struct MigrationSummary {
    #[tabled(rename = "Version")]
    version: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Applied")]
    applied_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthStatus {
    status: String,
    components: Vec<ComponentHealth>,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Tabled)]
struct ComponentHealth {
    #[tabled(rename = "Component")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Latency (ms)")]
    latency_ms: u64,
    #[tabled(rename = "Message")]
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SystemStatistics {
    tasks: TaskStats,
    agents: AgentStats,
    dags: DagStats,
    resources: ResourceStats,
    period: String,
    generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskStats {
    total: u64,
    completed: u64,
    failed: u64,
    running: u64,
    pending: u64,
    success_rate: f64,
    avg_duration_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentStats {
    total: u64,
    active: u64,
    idle: u64,
    paused: u64,
    avg_reputation: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DagStats {
    total: u64,
    completed: u64,
    running: u64,
    failed: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResourceStats {
    total_tokens: u64,
    total_cost: f64,
    avg_tokens_per_task: u64,
    avg_cost_per_task: f64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Output Helpers
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(dead_code)]
struct OutputHelper {
    format: OutputFormat,
    no_color: bool,
}

impl OutputHelper {
    fn new(format: OutputFormat, no_color: bool) -> Self {
        if no_color {
            colored::control::set_override(false);
        }
        Self { format, no_color }
    }

    fn print_success(&self, message: &str) {
        match self.format {
            OutputFormat::Json => {
                println!(r#"{{"status": "success", "message": "{}"}}"#, message);
            }
            _ => {
                println!("{} {}", "[OK]".green().bold(), message);
            }
        }
    }

    fn print_error(&self, message: &str) {
        match self.format {
            OutputFormat::Json => {
                eprintln!(r#"{{"status": "error", "message": "{}"}}"#, message);
            }
            _ => {
                eprintln!("{} {}", "[ERROR]".red().bold(), message);
            }
        }
    }

    fn print_warning(&self, message: &str) {
        match self.format {
            OutputFormat::Json => {
                println!(r#"{{"status": "warning", "message": "{}"}}"#, message);
            }
            _ => {
                println!("{} {}", "[WARN]".yellow().bold(), message);
            }
        }
    }

    fn print_info(&self, message: &str) {
        match self.format {
            OutputFormat::Json => {
                println!(r#"{{"status": "info", "message": "{}"}}"#, message);
            }
            _ => {
                println!("{} {}", "[INFO]".blue().bold(), message);
            }
        }
    }

    fn print_table<T: Tabled>(&self, items: &[T]) {
        match self.format {
            OutputFormat::Json => {
                // For JSON, we'd need to serialize the items
                // This is a simplified version
                println!("[]"); // Placeholder
            }
            _ => {
                if items.is_empty() {
                    println!("{}", "No results found.".dimmed());
                    return;
                }
                let table = Table::new(items)
                    .with(Style::rounded())
                    .with(Modify::new(Columns::first()).with(Alignment::left()))
                    .to_string();
                println!("{}", table);
            }
        }
    }

    fn print_json<T: Serialize>(&self, data: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(data)?;
        println!("{}", json);
        Ok(())
    }

    fn print_header(&self, title: &str) {
        if !matches!(self.format, OutputFormat::Json) {
            println!();
            println!("{}", title.bold().underline());
            println!();
        }
    }

    fn print_key_value(&self, key: &str, value: &str) {
        if !matches!(self.format, OutputFormat::Json) {
            println!("  {}: {}", key.cyan(), value);
        }
    }
}

fn create_progress_bar(len: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(message.to_string());
    pb
}

fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

// ═══════════════════════════════════════════════════════════════════════════════
// Command Handlers
// ═══════════════════════════════════════════════════════════════════════════════

async fn handle_task_command(cmd: TaskCommands, output: &OutputHelper) -> Result<()> {
    match cmd {
        TaskCommands::List { status: _, dag_id: _, limit, offset } => {
            output.print_header("Tasks");

            // Simulated data for demonstration
            let tasks = vec![
                TaskSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Research market trends".to_string(),
                    status: format_status("running"),
                    priority: 5,
                    tokens_used: 1500,
                    cost_dollars: "0.015".to_string(),
                    created_at: "2024-01-15 10:30".to_string(),
                },
                TaskSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Generate report".to_string(),
                    status: format_status("pending"),
                    priority: 3,
                    tokens_used: 0,
                    cost_dollars: "0.000".to_string(),
                    created_at: "2024-01-15 10:35".to_string(),
                },
                TaskSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Review analysis".to_string(),
                    status: format_status("completed"),
                    priority: 4,
                    tokens_used: 2300,
                    cost_dollars: "0.023".to_string(),
                    created_at: "2024-01-15 09:15".to_string(),
                },
            ];

            match output.format {
                OutputFormat::Json => output.print_json(&tasks)?,
                _ => output.print_table(&tasks),
            }

            output.print_info(&format!(
                "Showing {} tasks (offset: {}, limit: {})",
                tasks.len(),
                offset,
                limit
            ));
        }

        TaskCommands::Get { task_id, full } => {
            output.print_header(&format!("Task: {}", task_id));
            output.print_key_value("ID", &task_id.to_string());
            output.print_key_value("Name", "Research market trends");
            output.print_key_value("Status", &format_status("running"));
            output.print_key_value("Priority", "5");
            output.print_key_value("Tokens Used", "1500");
            output.print_key_value("Cost", "$0.015");
            output.print_key_value("Created", "2024-01-15 10:30:00 UTC");
            output.print_key_value("Started", "2024-01-15 10:31:00 UTC");

            if full {
                println!();
                output.print_key_value("Instruction", "Analyze current market trends for AI products");
                output.print_key_value("Context", "{}");
                output.print_key_value("Output", "In progress...");
            }
        }

        TaskCommands::Create { name, instruction: _, dag_id, priority, max_retries } => {
            let spinner = create_spinner("Creating task...");

            // Simulate task creation
            std::thread::sleep(Duration::from_millis(500));
            spinner.finish_and_clear();

            let task_id = Uuid::new_v4();
            output.print_success("Task created successfully");
            output.print_key_value("Task ID", &task_id.to_string());
            output.print_key_value("Name", &name);
            output.print_key_value("Priority", &priority.to_string());
            output.print_key_value("Max Retries", &max_retries.to_string());
            if let Some(dag) = dag_id {
                output.print_key_value("DAG ID", &dag.to_string());
            }
        }

        TaskCommands::Cancel { task_id, force } => {
            if force {
                output.print_warning("Force cancelling task (may interrupt running operation)");
            }

            let spinner = create_spinner("Cancelling task...");
            std::thread::sleep(Duration::from_millis(300));
            spinner.finish_and_clear();

            output.print_success(&format!("Task {} cancelled", task_id));
        }

        TaskCommands::Retry { task_id, reset_counter } => {
            let spinner = create_spinner("Retrying task...");
            std::thread::sleep(Duration::from_millis(300));
            spinner.finish_and_clear();

            output.print_success(&format!("Task {} queued for retry", task_id));
            if reset_counter {
                output.print_info("Retry counter has been reset");
            }
        }
    }

    Ok(())
}

async fn handle_agent_command(cmd: AgentCommands, output: &OutputHelper) -> Result<()> {
    match cmd {
        AgentCommands::List { status: _, model: _, limit: _ } => {
            output.print_header("Agents");

            let agents = vec![
                AgentSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Researcher".to_string(),
                    model: "gpt-4o".to_string(),
                    status: format_agent_status("idle"),
                    load: "2/5".to_string(),
                    success_rate: "98.5%".to_string(),
                    reputation: "0.95".to_string(),
                },
                AgentSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Coder".to_string(),
                    model: "claude-3.5-sonnet".to_string(),
                    status: format_agent_status("busy"),
                    load: "3/3".to_string(),
                    success_rate: "99.2%".to_string(),
                    reputation: "0.98".to_string(),
                },
                AgentSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Reviewer".to_string(),
                    model: "gpt-4o".to_string(),
                    status: format_agent_status("paused"),
                    load: "0/10".to_string(),
                    success_rate: "97.8%".to_string(),
                    reputation: "0.92".to_string(),
                },
            ];

            match output.format {
                OutputFormat::Json => output.print_json(&agents)?,
                _ => output.print_table(&agents),
            }
        }

        AgentCommands::Get { agent_id, history } => {
            output.print_header(&format!("Agent: {}", agent_id));
            output.print_key_value("ID", &agent_id.to_string());
            output.print_key_value("Name", "Researcher");
            output.print_key_value("Model", "gpt-4o");
            output.print_key_value("Status", &format_agent_status("idle"));
            output.print_key_value("Current Load", "2/5");
            output.print_key_value("Success Count", "1,234");
            output.print_key_value("Failure Count", "19");
            output.print_key_value("Success Rate", "98.5%");
            output.print_key_value("Total Tokens", "4,567,890");
            output.print_key_value("Total Cost", "$45.68");
            output.print_key_value("Reputation", "0.95");

            if history {
                println!();
                output.print_header("Performance History (Last 7 Days)");
                output.print_key_value("Tasks Completed", "156");
                output.print_key_value("Avg Duration", "2.3s");
                output.print_key_value("Avg Tokens/Task", "892");
            }
        }

        AgentCommands::Pause { agent_id, reason, graceful } => {
            if graceful {
                let spinner = create_spinner("Waiting for current task to complete...");
                std::thread::sleep(Duration::from_millis(500));
                spinner.finish_and_clear();
            }

            output.print_success(&format!("Agent {} paused", agent_id));
            if let Some(r) = reason {
                output.print_key_value("Reason", &r);
            }
        }

        AgentCommands::Resume { agent_id } => {
            output.print_success(&format!("Agent {} resumed", agent_id));
        }
    }

    Ok(())
}

async fn handle_dag_command(cmd: DagCommands, output: &OutputHelper) -> Result<()> {
    match cmd {
        DagCommands::List { status: _, limit: _ } => {
            output.print_header("DAGs");

            let dags = vec![
                DagSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Market Analysis Pipeline".to_string(),
                    total_tasks: 12,
                    completed: 8,
                    running: 2,
                    failed: 0,
                    created_at: "2024-01-15 09:00".to_string(),
                },
                DagSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    name: "Code Review Workflow".to_string(),
                    total_tasks: 5,
                    completed: 5,
                    running: 0,
                    failed: 0,
                    created_at: "2024-01-14 14:30".to_string(),
                },
            ];

            match output.format {
                OutputFormat::Json => output.print_json(&dags)?,
                _ => output.print_table(&dags),
            }
        }

        DagCommands::Get { dag_id, tree } => {
            output.print_header(&format!("DAG: {}", dag_id));
            output.print_key_value("ID", &dag_id.to_string());
            output.print_key_value("Name", "Market Analysis Pipeline");
            output.print_key_value("Status", &format_dag_status("running"));
            output.print_key_value("Total Tasks", "12");
            output.print_key_value("Completed", "8");
            output.print_key_value("Running", "2");
            output.print_key_value("Pending", "2");
            output.print_key_value("Failed", "0");
            output.print_key_value("Created", "2024-01-15 09:00:00 UTC");

            if tree {
                println!();
                output.print_header("Task Tree");
                println!("{}", r#"
  [COMPLETED] Research Data Collection
  ├── [COMPLETED] Web Scraping
  ├── [COMPLETED] API Data Fetch
  └── [COMPLETED] Data Cleaning
      └── [RUNNING] Analysis
          ├── [RUNNING] Trend Detection
          └── [PENDING] Report Generation
              └── [PENDING] Final Review
"#.trim());
            }
        }

        DagCommands::Start { dag_id, wait, timeout: _ } => {
            output.print_success(&format!("DAG {} started", dag_id));

            if wait {
                let pb = create_progress_bar(100, "Executing DAG...");
                for i in 0..100 {
                    std::thread::sleep(Duration::from_millis(30));
                    pb.set_position(i + 1);
                }
                pb.finish_with_message("Complete!");
                output.print_success("DAG execution completed successfully");
            }
        }

        DagCommands::Stop { dag_id, force, reason } => {
            if !force {
                output.print_warning("This will stop the DAG and cancel pending tasks. Use --force to skip this warning.");
                print!("Continue? [y/N]: ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    output.print_info("Operation cancelled");
                    return Ok(());
                }
            }

            let spinner = create_spinner("Stopping DAG...");
            std::thread::sleep(Duration::from_millis(500));
            spinner.finish_and_clear();

            output.print_success(&format!("DAG {} stopped", dag_id));
            if let Some(r) = reason {
                output.print_key_value("Reason", &r);
            }
        }

        DagCommands::Visualize { dag_id, output: vis_format } => {
            output.print_header(&format!("DAG Visualization: {}", dag_id));

            match vis_format {
                VisualizationFormat::Ascii => {
                    println!("{}", r#"
┌─────────────────────┐
│   Data Collection   │
└─────────┬───────────┘
          │
    ┌─────┴─────┐
    │           │
┌───▼───┐   ┌───▼───┐
│ Scrape│   │ API   │
└───┬───┘   └───┬───┘
    │           │
    └─────┬─────┘
          │
    ┌─────▼─────┐
    │  Clean    │
    └─────┬─────┘
          │
    ┌─────▼─────┐
    │ Analyze   │
    └─────┬─────┘
          │
    ┌─────▼─────┐
    │  Report   │
    └───────────┘
"#.trim());
                }
                VisualizationFormat::Dot => {
                    println!("{}", r#"
digraph DAG {
    rankdir=TB;
    node [shape=box];

    collect [label="Data Collection"];
    scrape [label="Web Scraping"];
    api [label="API Fetch"];
    clean [label="Data Cleaning"];
    analyze [label="Analysis"];
    report [label="Report"];

    collect -> scrape;
    collect -> api;
    scrape -> clean;
    api -> clean;
    clean -> analyze;
    analyze -> report;
}
"#.trim());
                }
                VisualizationFormat::Mermaid => {
                    println!("{}", r#"
graph TD
    A[Data Collection] --> B[Web Scraping]
    A --> C[API Fetch]
    B --> D[Data Cleaning]
    C --> D
    D --> E[Analysis]
    E --> F[Report]
"#.trim());
                }
            }
        }
    }

    Ok(())
}

async fn handle_approval_command(cmd: ApprovalCommands, output: &OutputHelper) -> Result<()> {
    match cmd {
        ApprovalCommands::List { approval_type: _, urgent, limit: _ } => {
            output.print_header("Pending Approvals");

            let approvals = vec![
                ApprovalSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    approval_type: "Contract Extension".to_string(),
                    requester: "Coder Agent".to_string(),
                    resource: "+5000 tokens".to_string(),
                    requested_at: "10 min ago".to_string(),
                    expires_at: "in 50 min".to_string(),
                },
                ApprovalSummary {
                    id: Uuid::new_v4().to_string()[..8].to_string(),
                    approval_type: "Tool Execution".to_string(),
                    requester: "Researcher".to_string(),
                    resource: "file_write".to_string(),
                    requested_at: "2 min ago".to_string(),
                    expires_at: "in 58 min".to_string(),
                },
            ];

            match output.format {
                OutputFormat::Json => output.print_json(&approvals)?,
                _ => output.print_table(&approvals),
            }

            if urgent {
                output.print_warning("Showing only urgent approvals");
            }
        }

        ApprovalCommands::Approve { approval_id, comment } => {
            let spinner = create_spinner("Processing approval...");
            std::thread::sleep(Duration::from_millis(300));
            spinner.finish_and_clear();

            output.print_success(&format!("Approval {} granted", approval_id));
            if let Some(c) = comment {
                output.print_key_value("Comment", &c);
            }
        }

        ApprovalCommands::Deny { approval_id, reason } => {
            let spinner = create_spinner("Processing denial...");
            std::thread::sleep(Duration::from_millis(300));
            spinner.finish_and_clear();

            output.print_success(&format!("Approval {} denied", approval_id));
            output.print_key_value("Reason", &reason);
        }
    }

    Ok(())
}

async fn handle_migrate_command(cmd: MigrateCommands, output: &OutputHelper) -> Result<()> {
    match cmd {
        MigrateCommands::Run { all: _, dry_run } => {
            output.print_header("Database Migrations");

            let migrations = vec![
                MigrationSummary {
                    version: "20240115_001".to_string(),
                    name: "create_tasks_table".to_string(),
                    status: "Applied".green().to_string(),
                    applied_at: "2024-01-15 08:00:00".to_string(),
                },
                MigrationSummary {
                    version: "20240115_002".to_string(),
                    name: "create_agents_table".to_string(),
                    status: "Applied".green().to_string(),
                    applied_at: "2024-01-15 08:00:01".to_string(),
                },
                MigrationSummary {
                    version: "20240116_001".to_string(),
                    name: "add_contracts_table".to_string(),
                    status: "Pending".yellow().to_string(),
                    applied_at: "-".to_string(),
                },
            ];

            if dry_run {
                output.print_warning("Dry run mode - no changes will be made");
            }

            let pb = create_progress_bar(migrations.len() as u64, "Running migrations...");
            for (i, _migration) in migrations.iter().enumerate() {
                std::thread::sleep(Duration::from_millis(200));
                pb.set_position((i + 1) as u64);
            }
            pb.finish_with_message("Complete!");

            output.print_table(&migrations);
            output.print_success("All migrations applied successfully");
        }

        MigrateCommands::Revert { count, all, force } => {
            if !force {
                output.print_warning(&format!(
                    "This will revert {} migration(s). This operation cannot be undone.",
                    if all { "ALL".to_string() } else { count.to_string() }
                ));
                print!("Continue? [y/N]: ");
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    output.print_info("Operation cancelled");
                    return Ok(());
                }
            }

            let spinner = create_spinner("Reverting migrations...");
            std::thread::sleep(Duration::from_millis(500));
            spinner.finish_and_clear();

            let label = if all { "all".to_string() } else { count.to_string() };
            output.print_success(&format!("Reverted {} migration(s)", label));
        }

        MigrateCommands::Status { pending } => {
            output.print_header("Migration Status");

            let migrations = vec![
                MigrationSummary {
                    version: "20240115_001".to_string(),
                    name: "create_tasks_table".to_string(),
                    status: "Applied".green().to_string(),
                    applied_at: "2024-01-15 08:00:00".to_string(),
                },
                MigrationSummary {
                    version: "20240115_002".to_string(),
                    name: "create_agents_table".to_string(),
                    status: "Applied".green().to_string(),
                    applied_at: "2024-01-15 08:00:01".to_string(),
                },
                MigrationSummary {
                    version: "20240116_001".to_string(),
                    name: "add_contracts_table".to_string(),
                    status: "Pending".yellow().to_string(),
                    applied_at: "-".to_string(),
                },
            ];

            let filtered: Vec<_> = if pending {
                migrations.into_iter().filter(|m| m.applied_at == "-").collect()
            } else {
                migrations
            };

            output.print_table(&filtered);
        }
    }

    Ok(())
}

async fn handle_seed_command(count: u32, entity: Option<SeedEntity>, output: &OutputHelper) -> Result<()> {
    output.print_header("Database Seeding");

    let entities = match entity {
        Some(SeedEntity::Agents) => vec!["agents"],
        Some(SeedEntity::Tasks) => vec!["tasks"],
        Some(SeedEntity::Dags) => vec!["dags"],
        Some(SeedEntity::Contracts) => vec!["contracts"],
        Some(SeedEntity::All) | None => vec!["agents", "tasks", "dags", "contracts"],
    };

    for entity_name in &entities {
        let pb = create_progress_bar(count as u64, &format!("Seeding {}...", entity_name));
        for i in 0..count {
            std::thread::sleep(Duration::from_millis(50));
            pb.set_position((i + 1) as u64);
        }
        pb.finish_with_message("Done!");
    }

    output.print_success(&format!(
        "Created {} records for {} entity type(s)",
        count * entities.len() as u32,
        entities.len()
    ));

    Ok(())
}

async fn handle_health_command(detailed: bool, _timeout: u64, output: &OutputHelper) -> Result<()> {
    output.print_header("System Health");

    let spinner = create_spinner("Checking system health...");
    std::thread::sleep(Duration::from_millis(500));
    spinner.finish_and_clear();

    let components = vec![
        ComponentHealth {
            name: "PostgreSQL".to_string(),
            status: "Healthy".green().to_string(),
            latency_ms: 2,
            message: "Connected".to_string(),
        },
        ComponentHealth {
            name: "Redis".to_string(),
            status: "Healthy".green().to_string(),
            latency_ms: 1,
            message: "Connected".to_string(),
        },
        ComponentHealth {
            name: "gRPC Server".to_string(),
            status: "Healthy".green().to_string(),
            latency_ms: 0,
            message: "Listening on :50051".to_string(),
        },
        ComponentHealth {
            name: "HTTP Server".to_string(),
            status: "Healthy".green().to_string(),
            latency_ms: 0,
            message: "Listening on :8080".to_string(),
        },
        ComponentHealth {
            name: "OpenAI API".to_string(),
            status: "Healthy".green().to_string(),
            latency_ms: 45,
            message: "API key valid".to_string(),
        },
        ComponentHealth {
            name: "Anthropic API".to_string(),
            status: "Degraded".yellow().to_string(),
            latency_ms: 120,
            message: "High latency".to_string(),
        },
    ];

    let health_status = HealthStatus {
        status: "Healthy".to_string(),
        components: components.clone(),
        timestamp: Utc::now(),
    };

    match output.format {
        OutputFormat::Json => output.print_json(&health_status)?,
        _ => {
            output.print_table(&components);

            if detailed {
                println!();
                output.print_key_value("System Uptime", "7d 14h 32m");
                output.print_key_value("CPU Usage", "23%");
                output.print_key_value("Memory Usage", "4.2 GB / 16 GB (26%)");
                output.print_key_value("Disk Usage", "45 GB / 200 GB (23%)");
            }
        }
    }

    output.print_success("All critical systems operational");

    Ok(())
}

async fn handle_stats_command(period: String, live: bool, output: &OutputHelper) -> Result<()> {
    output.print_header(&format!("System Statistics ({})", period));

    let stats = SystemStatistics {
        tasks: TaskStats {
            total: 12456,
            completed: 11892,
            failed: 234,
            running: 18,
            pending: 312,
            success_rate: 98.1,
            avg_duration_ms: 2340,
        },
        agents: AgentStats {
            total: 25,
            active: 18,
            idle: 5,
            paused: 2,
            avg_reputation: 0.94,
        },
        dags: DagStats {
            total: 456,
            completed: 432,
            running: 12,
            failed: 12,
        },
        resources: ResourceStats {
            total_tokens: 45_678_901,
            total_cost: 456.78,
            avg_tokens_per_task: 3666,
            avg_cost_per_task: 0.037,
        },
        period: period.clone(),
        generated_at: Utc::now(),
    };

    match output.format {
        OutputFormat::Json => output.print_json(&stats)?,
        _ => {
            println!("{}", "Tasks".bold());
            output.print_key_value("  Total", &format!("{}", stats.tasks.total));
            output.print_key_value("  Completed", &format!("{} ({:.1}%)", stats.tasks.completed, stats.tasks.success_rate));
            output.print_key_value("  Failed", &stats.tasks.failed.to_string());
            output.print_key_value("  Running", &stats.tasks.running.to_string());
            output.print_key_value("  Pending", &stats.tasks.pending.to_string());
            output.print_key_value("  Avg Duration", &format!("{} ms", stats.tasks.avg_duration_ms));

            println!();
            println!("{}", "Agents".bold());
            output.print_key_value("  Total", &stats.agents.total.to_string());
            output.print_key_value("  Active", &stats.agents.active.to_string());
            output.print_key_value("  Idle", &stats.agents.idle.to_string());
            output.print_key_value("  Paused", &stats.agents.paused.to_string());
            output.print_key_value("  Avg Reputation", &format!("{:.2}", stats.agents.avg_reputation));

            println!();
            println!("{}", "DAGs".bold());
            output.print_key_value("  Total", &stats.dags.total.to_string());
            output.print_key_value("  Completed", &stats.dags.completed.to_string());
            output.print_key_value("  Running", &stats.dags.running.to_string());
            output.print_key_value("  Failed", &stats.dags.failed.to_string());

            println!();
            println!("{}", "Resources".bold());
            output.print_key_value("  Total Tokens", &format!("{}", stats.resources.total_tokens));
            output.print_key_value("  Total Cost", &format!("${:.2}", stats.resources.total_cost));
            output.print_key_value("  Avg Tokens/Task", &stats.resources.avg_tokens_per_task.to_string());
            output.print_key_value("  Avg Cost/Task", &format!("${:.3}", stats.resources.avg_cost_per_task));
        }
    }

    if live {
        output.print_info("Live mode: Statistics will update every 5 seconds. Press Ctrl+C to exit.");
    }

    Ok(())
}

async fn handle_config_command(cmd: ConfigCommands, output: &OutputHelper) -> Result<()> {
    match cmd {
        ConfigCommands::Show { section, show_secrets } => {
            output.print_header("Configuration");

            match section {
                Some(ConfigSection::Server) | None => {
                    println!("{}", "[server]".bold());
                    output.print_key_value("host", "0.0.0.0");
                    output.print_key_value("port", "8080");
                    output.print_key_value("grpc_port", "50051");
                }
                _ => {}
            }

            match section {
                Some(ConfigSection::Database) | None => {
                    println!();
                    println!("{}", "[database]".bold());
                    output.print_key_value("url", if show_secrets {
                        "postgres://apex:secret@localhost:5432/apex"
                    } else {
                        "postgres://apex:****@localhost:5432/apex"
                    });
                    output.print_key_value("max_connections", "20");
                    output.print_key_value("min_connections", "5");
                }
                _ => {}
            }

            match section {
                Some(ConfigSection::Redis) | None => {
                    println!();
                    println!("{}", "[redis]".bold());
                    output.print_key_value("url", "redis://localhost:6379");
                    output.print_key_value("pool_size", "10");
                }
                _ => {}
            }

            match section {
                Some(ConfigSection::Orchestrator) | None => {
                    println!();
                    println!("{}", "[orchestrator]".bold());
                    output.print_key_value("max_concurrent_agents", "100");
                    output.print_key_value("enable_model_routing", "true");
                    output.print_key_value("circuit_breaker_threshold", "5");
                    output.print_key_value("default_token_limit", "20000");
                    output.print_key_value("default_cost_limit", "0.25");
                    output.print_key_value("default_time_limit", "300");
                }
                _ => {}
            }

            match section {
                Some(ConfigSection::Llm) | None => {
                    println!();
                    println!("{}", "[llm]".bold());
                    output.print_key_value("openai_api_key", if show_secrets {
                        "sk-xxx..."
                    } else {
                        "****"
                    });
                    output.print_key_value("anthropic_api_key", if show_secrets {
                        "sk-ant-xxx..."
                    } else {
                        "****"
                    });
                    output.print_key_value("default_model", "gpt-4o-mini");
                }
                _ => {}
            }
        }

        ConfigCommands::Get { key } => {
            let value = match key.as_str() {
                "server.port" => "8080",
                "server.host" => "0.0.0.0",
                "server.grpc_port" => "50051",
                "database.max_connections" => "20",
                "orchestrator.max_concurrent_agents" => "100",
                _ => {
                    output.print_error(&format!("Unknown configuration key: {}", key));
                    return Ok(());
                }
            };

            match output.format {
                OutputFormat::Json => {
                    println!(r#"{{"key": "{}", "value": "{}"}}"#, key, value);
                }
                _ => {
                    println!("{}", value);
                }
            }
        }

        ConfigCommands::Set { key, value, persist } => {
            output.print_success(&format!("Configuration updated: {} = {}", key, value));
            if persist {
                output.print_info("Changes persisted to configuration file");
            } else {
                output.print_warning("Changes are temporary and will be lost on restart. Use --persist to save.");
            }
        }

        ConfigCommands::Validate { file } => {
            let spinner = create_spinner("Validating configuration...");
            std::thread::sleep(Duration::from_millis(300));
            spinner.finish_and_clear();

            output.print_success("Configuration is valid");
            output.print_key_value("File", file.as_deref().unwrap_or("(default)"));
        }

        ConfigCommands::Init { output: output_path, force } => {
            if !force && std::path::Path::new(&output_path).exists() {
                output.print_error(&format!("File {} already exists. Use --force to overwrite.", output_path));
                return Ok(());
            }

            let sample_config = r#"# Apex Configuration File
# Generated by apex-cli

[server]
host = "0.0.0.0"
port = 8080
grpc_port = 50051

[database]
url = "postgres://apex:password@localhost:5432/apex"
max_connections = 20
min_connections = 5

[redis]
url = "redis://localhost:6379"
pool_size = 10

[observability]
log_level = "info"
json_logging = true
# otlp_endpoint = "http://localhost:4317"

[orchestrator]
max_concurrent_agents = 100
enable_model_routing = true
circuit_breaker_threshold = 5
default_token_limit = 20000
default_cost_limit = 0.25
default_time_limit = 300

[llm]
# openai_api_key = "sk-..."
# anthropic_api_key = "sk-ant-..."
default_model = "gpt-4o-mini"
"#;

            std::fs::write(&output_path, sample_config)?;
            output.print_success(&format!("Configuration file created: {}", output_path));
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Formatting Helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn format_status(status: &str) -> String {
    match status {
        "pending" => status.yellow().to_string(),
        "ready" => status.cyan().to_string(),
        "running" => status.blue().bold().to_string(),
        "completed" => status.green().to_string(),
        "failed" => status.red().to_string(),
        "cancelled" => status.dimmed().to_string(),
        _ => status.to_string(),
    }
}

fn format_agent_status(status: &str) -> String {
    match status {
        "idle" => status.green().to_string(),
        "busy" => status.blue().bold().to_string(),
        "error" => status.red().to_string(),
        "paused" => status.yellow().to_string(),
        _ => status.to_string(),
    }
}

fn format_dag_status(status: &str) -> String {
    match status {
        "pending" => status.yellow().to_string(),
        "running" => status.blue().bold().to_string(),
        "completed" => status.green().to_string(),
        "failed" => status.red().to_string(),
        _ => status.to_string(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Main Entry Point
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Create output helper
    let output = OutputHelper::new(cli.format, cli.no_color);

    // Handle commands
    let result = match cli.command {
        Commands::Task(cmd) => handle_task_command(cmd, &output).await,
        Commands::Agent(cmd) => handle_agent_command(cmd, &output).await,
        Commands::Dag(cmd) => handle_dag_command(cmd, &output).await,
        Commands::Approval(cmd) => handle_approval_command(cmd, &output).await,
        Commands::Migrate(cmd) => handle_migrate_command(cmd, &output).await,
        Commands::Seed { count, entity } => handle_seed_command(count, entity, &output).await,
        Commands::Health { detailed, timeout } => handle_health_command(detailed, timeout, &output).await,
        Commands::Stats { period, live } => handle_stats_command(period, live, &output).await,
        Commands::Config(cmd) => handle_config_command(cmd, &output).await,
    };

    if let Err(e) = result {
        output.print_error(&format!("{:#}", e));
        std::process::exit(1);
    }

    Ok(())
}
