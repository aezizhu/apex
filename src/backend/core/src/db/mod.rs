//! Database layer for Apex.
//!
//! Uses PostgreSQL for persistent storage with sqlx.

use sqlx::{PgPool, postgres::PgPoolOptions, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::error::{ApexError, Result};
use crate::dag::{Task, TaskId, TaskStatus, TaskOutput};
use crate::agents::AgentStats;
use crate::contracts::{AgentContract, ResourceUsage};

/// Database connection and operations.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection pool.
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(5)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Run migrations.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| ApexError::from(sqlx::Error::Migrate(Box::new(e))))?;
        Ok(())
    }

    /// Get the connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Task Operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Insert a new task.
    pub async fn insert_task(&self, task: &Task, dag_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tasks (id, dag_id, parent_id, name, status, priority, input, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(task.id.0)
        .bind(dag_id)
        .bind(task.parent_id.map(|id| id.0))
        .bind(&task.name)
        .bind(task.status.as_str())
        .bind(task.priority)
        .bind(serde_json::to_value(&task.input)?)
        .bind(task.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update task status.
    pub async fn update_task_status(&self, task_id: TaskId, status: TaskStatus) -> Result<()> {
        let now = Utc::now();

        let (started_at, completed_at): (Option<DateTime<Utc>>, Option<DateTime<Utc>>) = match &status {
            TaskStatus::Running => (Some(now), None),
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => (None, Some(now)),
            _ => (None, None),
        };

        sqlx::query(
            r#"
            UPDATE tasks
            SET status = $2, started_at = COALESCE($3, started_at), completed_at = COALESCE($4, completed_at)
            WHERE id = $1
            "#,
        )
        .bind(task_id.0)
        .bind(status.as_str())
        .bind(started_at)
        .bind(completed_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update task with completion data.
    pub async fn complete_task(
        &self,
        task_id: TaskId,
        output: &TaskOutput,
        tokens: u64,
        cost: f64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'completed',
                output = $2,
                tokens_used = $3,
                cost_dollars = $4,
                completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(task_id.0)
        .bind(serde_json::to_value(output)?)
        .bind(tokens as i64)
        .bind(cost)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get task by ID.
    pub async fn get_task(&self, task_id: TaskId) -> Result<Option<TaskRow>> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, dag_id, parent_id, agent_id, name, status, priority,
                   input, output, error, tokens_used, cost_dollars,
                   retry_count, created_at, started_at, completed_at
            FROM tasks
            WHERE id = $1
            "#,
        )
        .bind(task_id.0)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get paginated tasks ordered by created_at descending.
    pub async fn get_tasks_paginated(&self, limit: i64, offset: i64) -> Result<Vec<TaskRow>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, dag_id, parent_id, agent_id, name, status, priority,
                   input, output, error, tokens_used, cost_dollars,
                   retry_count, created_at, started_at, completed_at
            FROM tasks
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get total task count.
    pub async fn get_task_count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Get all tasks for a DAG.
    pub async fn get_dag_tasks(&self, dag_id: Uuid) -> Result<Vec<TaskRow>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, dag_id, parent_id, agent_id, name, status, priority,
                   input, output, error, tokens_used, cost_dollars,
                   retry_count, created_at, started_at, completed_at
            FROM tasks
            WHERE dag_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(dag_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Agent Operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Insert or update agent.
    pub async fn upsert_agent(&self, agent: &AgentStats) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO agents (id, name, model, status, current_load, max_load,
                               success_count, failure_count, total_tokens, total_cost, reputation_score)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                current_load = EXCLUDED.current_load,
                success_count = EXCLUDED.success_count,
                failure_count = EXCLUDED.failure_count,
                total_tokens = EXCLUDED.total_tokens,
                total_cost = EXCLUDED.total_cost,
                reputation_score = EXCLUDED.reputation_score,
                last_active_at = NOW()
            "#,
        )
        .bind(agent.id.0)
        .bind(&agent.name)
        .bind(&agent.model)
        .bind(agent.status.as_str())
        .bind(agent.current_load as i32)
        .bind(agent.max_load as i32)
        .bind(agent.success_count as i64)
        .bind(agent.failure_count as i64)
        .bind(agent.total_tokens as i64)
        .bind(agent.total_cost)
        .bind(agent.reputation_score)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get agent by ID.
    pub async fn get_agent(&self, agent_id: Uuid) -> Result<Option<AgentRow>> {
        let row = sqlx::query_as::<_, AgentRow>(
            r#"
            SELECT id, name, model, system_prompt, status, current_load, max_load,
                   success_count, failure_count, total_tokens, total_cost, reputation_score,
                   created_at, last_active_at
            FROM agents
            WHERE id = $1
            "#,
        )
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get all agents.
    pub async fn get_agents(&self) -> Result<Vec<AgentRow>> {
        let rows = sqlx::query_as::<_, AgentRow>(
            r#"
            SELECT id, name, model, system_prompt, status, current_load, max_load,
                   success_count, failure_count, total_tokens, total_cost, reputation_score,
                   created_at, last_active_at
            FROM agents
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // DAG Operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get DAG by ID.
    pub async fn get_dag(&self, dag_id: Uuid) -> Result<Option<DagRow>> {
        let row = sqlx::query_as::<_, DagRow>(
            r#"
            SELECT id, name, status, metadata, created_at, started_at, completed_at
            FROM dags
            WHERE id = $1
            "#,
        )
        .bind(dag_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get DAG nodes for a DAG.
    pub async fn get_dag_nodes(&self, dag_id: Uuid) -> Result<Vec<DagNodeRow>> {
        let rows = sqlx::query_as::<_, DagNodeRow>(
            r#"
            SELECT id, dag_id, task_template, depends_on, is_entry, is_exit
            FROM dag_nodes
            WHERE dag_id = $1
            "#,
        )
        .bind(dag_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Contract Operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get all contracts with pagination.
    pub async fn get_contracts(&self, limit: i64, offset: i64) -> Result<Vec<ContractRow>> {
        let rows = sqlx::query_as::<_, ContractRow>(
            r#"
            SELECT id, agent_id, task_id, parent_contract_id,
                   token_limit, cost_limit, time_limit_seconds, api_call_limit,
                   token_used, cost_used, api_calls_used,
                   status, created_at, expires_at
            FROM agent_contracts
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get contract count.
    pub async fn get_contract_count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_contracts")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Get contract by ID.
    pub async fn get_contract(&self, contract_id: Uuid) -> Result<Option<ContractRow>> {
        let row = sqlx::query_as::<_, ContractRow>(
            r#"
            SELECT id, agent_id, task_id, parent_contract_id,
                   token_limit, cost_limit, time_limit_seconds, api_call_limit,
                   token_used, cost_used, api_calls_used,
                   status, created_at, expires_at
            FROM agent_contracts
            WHERE id = $1
            "#,
        )
        .bind(contract_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Insert a contract.
    pub async fn insert_contract(&self, contract: &AgentContract) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO agent_contracts (id, agent_id, task_id, parent_contract_id,
                                        token_limit, cost_limit, time_limit_seconds, api_call_limit,
                                        status, created_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(contract.id)
        .bind(contract.agent_id)
        .bind(contract.task_id)
        .bind(contract.parent_contract_id)
        .bind(contract.limits.token_limit as i64)
        .bind(contract.limits.cost_limit)
        .bind(contract.limits.time_limit_seconds as i64)
        .bind(contract.limits.api_call_limit as i64)
        .bind(contract.status.as_str())
        .bind(contract.created_at)
        .bind(contract.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update contract usage.
    pub async fn update_contract_usage(&self, contract_id: Uuid, usage: &ResourceUsage) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE agent_contracts
            SET token_used = $2, cost_used = $3, api_calls_used = $4
            WHERE id = $1
            "#,
        )
        .bind(contract_id)
        .bind(usage.tokens_used as i64)
        .bind(usage.cost_used)
        .bind(usage.api_calls_used as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Event Operations (Event Sourcing)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Insert an event.
    pub async fn insert_event(&self, event: &Event) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO events (event_id, trace_id, span_id, aggregate_type, aggregate_id,
                               event_type, event_data, metadata, version)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(event.event_id)
        .bind(&event.trace_id)
        .bind(&event.span_id)
        .bind(&event.aggregate_type)
        .bind(event.aggregate_id)
        .bind(&event.event_type)
        .bind(&event.event_data)
        .bind(&event.metadata)
        .bind(event.version)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get events for an aggregate.
    pub async fn get_events(&self, aggregate_type: &str, aggregate_id: Uuid) -> Result<Vec<EventRow>> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT id, event_id, trace_id, span_id, aggregate_type, aggregate_id,
                   event_type, event_data, metadata, version, created_at
            FROM events
            WHERE aggregate_type = $1 AND aggregate_id = $2
            ORDER BY version
            "#,
        )
        .bind(aggregate_type)
        .bind(aggregate_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Metrics / Aggregations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Get system-wide statistics.
    pub async fn get_system_stats(&self) -> Result<SystemStats> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_tasks,
                COUNT(*) FILTER (WHERE status = 'completed') as completed_tasks,
                COUNT(*) FILTER (WHERE status = 'failed') as failed_tasks,
                COUNT(*) FILTER (WHERE status = 'running') as running_tasks,
                COALESCE(SUM(tokens_used), 0) as total_tokens,
                COALESCE(SUM(cost_dollars), 0.0) as total_cost
            FROM tasks
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let agent_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agents")
            .fetch_one(&self.pool)
            .await?;

        Ok(SystemStats {
            total_tasks: row.get::<i64, _>("total_tasks") as u64,
            completed_tasks: row.get::<i64, _>("completed_tasks") as u64,
            failed_tasks: row.get::<i64, _>("failed_tasks") as u64,
            running_tasks: row.get::<i64, _>("running_tasks") as u64,
            total_tokens: row.get::<i64, _>("total_tokens") as u64,
            total_cost: row.get::<f64, _>("total_cost"),
            agent_count: agent_count as u64,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Row Types (for sqlx queries)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, sqlx::FromRow)]
pub struct TaskRow {
    pub id: Uuid,
    pub dag_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub name: String,
    pub status: String,
    pub priority: i32,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub tokens_used: i64,
    pub cost_dollars: f64,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AgentRow {
    pub id: Uuid,
    pub name: String,
    pub model: String,
    pub system_prompt: Option<String>,
    pub status: String,
    pub current_load: i32,
    pub max_load: i32,
    pub success_count: i64,
    pub failure_count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub reputation_score: f64,
    pub created_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct DagRow {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct DagNodeRow {
    pub id: Uuid,
    pub dag_id: Uuid,
    pub task_template: serde_json::Value,
    pub depends_on: Option<Vec<String>>,
    pub is_entry: bool,
    pub is_exit: bool,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ContractRow {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub task_id: Option<Uuid>,
    pub parent_contract_id: Option<Uuid>,
    pub token_limit: i64,
    pub cost_limit: f64,
    pub time_limit_seconds: i64,
    pub api_call_limit: i64,
    pub token_used: i64,
    pub cost_used: f64,
    pub api_calls_used: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EventRow {
    pub id: i64,
    pub event_id: Uuid,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub metadata: serde_json::Value,
    pub version: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct Event {
    pub event_id: Uuid,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub aggregate_type: String,
    pub aggregate_id: Uuid,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub metadata: serde_json::Value,
    pub version: i32,
}

#[derive(Debug, Clone)]
pub struct SystemStats {
    pub total_tasks: u64,
    pub completed_tasks: u64,
    pub failed_tasks: u64,
    pub running_tasks: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub agent_count: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helper trait implementations
// ═══════════════════════════════════════════════════════════════════════════════

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Ready => "ready",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

impl crate::agents::AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            crate::agents::AgentStatus::Idle => "idle",
            crate::agents::AgentStatus::Busy => "busy",
            crate::agents::AgentStatus::Error => "error",
            crate::agents::AgentStatus::Paused => "paused",
        }
    }
}

impl crate::contracts::ContractStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            crate::contracts::ContractStatus::Active => "active",
            crate::contracts::ContractStatus::Completed => "completed",
            crate::contracts::ContractStatus::Exceeded => "exceeded",
            crate::contracts::ContractStatus::Cancelled => "cancelled",
        }
    }
}
