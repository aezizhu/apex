//! Task management commands.
//!
//! Provides submit, list, status, and cancel operations for tasks.

use anyhow::{Context, Result};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use tabled::Tabled;
use uuid::Uuid;

use crate::client::ApiClient;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum TaskCommands {
    /// Submit a new task or DAG workflow
    Submit {
        /// Path to a DAG workflow YAML file
        #[arg(short, long)]
        dag: Option<String>,

        /// Task name (for single-task submission)
        #[arg(short, long)]
        name: Option<String>,

        /// Task instruction (for single-task submission)
        #[arg(short, long)]
        instruction: Option<String>,

        /// Task priority
        #[arg(short, long, default_value = "0")]
        priority: i32,
    },

    /// List tasks
    List {
        /// Filter by status (pending, running, completed, failed)
        #[arg(short, long)]
        status: Option<String>,

        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },

    /// Get task status
    Status {
        /// Task ID
        task_id: Uuid,
    },

    /// Cancel a task
    Cancel {
        /// Task ID
        task_id: Uuid,
    },
}

// ── API types ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CreateTaskRequest {
    name: String,
    instruction: String,
    priority: Option<i32>,
}

#[derive(Serialize)]
struct CreateDagRequest {
    name: String,
    tasks: Vec<DagTaskReq>,
    dependencies: Vec<DagDep>,
}

#[derive(Serialize, Deserialize)]
struct DagTaskReq {
    id: String,
    name: String,
    instruction: String,
}

#[derive(Serialize, Deserialize)]
struct DagDep {
    from: String,
    to: String,
}

/// Workflow YAML format for DAG submission.
#[derive(Deserialize)]
struct WorkflowFile {
    name: String,
    tasks: Vec<DagTaskReq>,
    #[serde(default)]
    dependencies: Vec<DagDep>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TaskInfo {
    id: Uuid,
    name: String,
    status: String,
    #[serde(default)]
    tokens_used: u64,
    #[serde(default)]
    cost_dollars: f64,
    #[serde(default)]
    created_at: String,
}

#[derive(Debug, Deserialize, Serialize, Tabled)]
struct TaskRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Tokens")]
    tokens_used: u64,
    #[tabled(rename = "Cost ($)")]
    cost: String,
    #[tabled(rename = "Created")]
    created_at: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TaskStatus {
    id: Uuid,
    status: String,
    #[serde(default)]
    tokens_used: u64,
    #[serde(default)]
    cost_dollars: f64,
}

#[derive(Debug, Deserialize, Serialize)]
struct DagResponse {
    id: Uuid,
    name: String,
    task_count: usize,
    status: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct CancelResponse {
    id: Uuid,
    status: String,
}

// ── Execution ───────────────────────────────────────────────────────────────

pub async fn execute(cmd: TaskCommands, client: &ApiClient, format: OutputFormat) -> Result<()> {
    match cmd {
        TaskCommands::Submit {
            dag,
            name,
            instruction,
            priority,
        } => {
            if let Some(dag_path) = dag {
                // DAG workflow submission
                let content = std::fs::read_to_string(&dag_path)
                    .with_context(|| format!("Failed to read workflow file: {}", dag_path))?;

                let workflow: WorkflowFile = serde_yaml::from_str(&content)
                    .with_context(|| "Failed to parse workflow YAML")?;

                let body = CreateDagRequest {
                    name: workflow.name,
                    tasks: workflow.tasks,
                    dependencies: workflow.dependencies,
                };

                let resp: DagResponse = client.post("/api/v1/dags", &body).await?;

                match format {
                    OutputFormat::Table => {
                        output::print_success("DAG workflow submitted");
                        output::print_detail("DAG ID", &resp.id.to_string());
                        output::print_detail("Name", &resp.name);
                        output::print_detail("Tasks", &resp.task_count.to_string());
                        output::print_detail("Status", &resp.status);
                    }
                    _ => output::print_item(&resp, format),
                }
            } else {
                // Single task submission
                let task_name = name.unwrap_or_else(|| "Untitled Task".to_string());
                let task_instruction =
                    instruction.unwrap_or_else(|| "No instruction provided".to_string());

                let body = CreateTaskRequest {
                    name: task_name.clone(),
                    instruction: task_instruction,
                    priority: Some(priority),
                };

                let resp: TaskInfo = client.post("/api/v1/tasks", &body).await?;

                match format {
                    OutputFormat::Table => {
                        output::print_success("Task submitted");
                        output::print_detail("ID", &resp.id.to_string());
                        output::print_detail("Name", &resp.name);
                        output::print_detail("Status", &resp.status);
                    }
                    _ => output::print_item(&resp, format),
                }
            }
        }

        TaskCommands::List { status, limit } => {
            let path = match &status {
                Some(s) => format!("/api/v1/tasks?status={}&limit={}", s, limit),
                None => format!("/api/v1/tasks?limit={}", limit),
            };

            // The API doesn't have a list-tasks endpoint yet, so we call
            // the stats endpoint as a fallback. In a real implementation
            // this would call a proper list endpoint.
            let tasks: Vec<TaskInfo> = client.get(&path).await?;

            let rows: Vec<TaskRow> = tasks
                .into_iter()
                .map(|t| TaskRow {
                    id: t.id.to_string()[..8].to_string(),
                    name: t.name,
                    status: t.status,
                    tokens_used: t.tokens_used,
                    cost: format!("{:.4}", t.cost_dollars),
                    created_at: t.created_at,
                })
                .collect();

            output::print_list(&rows, format);
        }

        TaskCommands::Status { task_id } => {
            let status: TaskStatus = client
                .get(&format!("/api/v1/tasks/{}/status", task_id))
                .await?;

            match format {
                OutputFormat::Table => {
                    output::print_header(&format!("Task: {}", task_id));
                    output::print_detail("Status", &status.status);
                    output::print_detail("Tokens Used", &status.tokens_used.to_string());
                    output::print_detail("Cost", &format!("${:.4}", status.cost_dollars));
                }
                _ => output::print_item(&status, format),
            }
        }

        TaskCommands::Cancel { task_id } => {
            let resp: CancelResponse = client
                .post(
                    &format!("/api/v1/tasks/{}/cancel", task_id),
                    &serde_json::json!({}),
                )
                .await?;

            match format {
                OutputFormat::Table => {
                    output::print_success(&format!("Task {} cancelled", task_id));
                }
                _ => output::print_item(&resp, format),
            }
        }
    }

    Ok(())
}
