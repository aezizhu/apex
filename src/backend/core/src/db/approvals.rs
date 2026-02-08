//! Database operations for approvals.

use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::error::Result;
use super::Database;

// ═══════════════════════════════════════════════════════════════════════════════
// Row Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ApprovalRow {
    pub id: Uuid,
    pub task_id: Uuid,
    pub agent_id: Uuid,
    pub action: String,
    pub action_description: Option<String>,
    pub action_data: serde_json::Value,
    pub risk_score: Option<f64>,
    pub risk_factors: Option<serde_json::Value>,
    pub cluster_id: Option<Uuid>,
    pub status: String,
    pub decided_by: Option<String>,
    pub decision_reason: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Database Operations
// ═══════════════════════════════════════════════════════════════════════════════

impl Database {
    /// Get pending approvals with pagination.
    pub async fn get_pending_approvals(&self, limit: i64, offset: i64) -> Result<Vec<ApprovalRow>> {
        let rows = sqlx::query_as::<_, ApprovalRow>(
            r#"
            SELECT id, task_id, agent_id, action::text, action_description, action_data,
                   risk_score::float8, risk_factors, cluster_id, status::text,
                   decided_by, decision_reason, decided_at, expires_at,
                   metadata, created_at, updated_at
            FROM approvals
            WHERE status = 'pending'
            ORDER BY
                risk_score DESC NULLS LAST,
                created_at ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get total count of pending approvals.
    pub async fn get_pending_approval_count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM approvals WHERE status = 'pending'"
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    /// Get a single approval by ID.
    pub async fn get_approval(&self, approval_id: Uuid) -> Result<Option<ApprovalRow>> {
        let row = sqlx::query_as::<_, ApprovalRow>(
            r#"
            SELECT id, task_id, agent_id, action::text, action_description, action_data,
                   risk_score::float8, risk_factors, cluster_id, status::text,
                   decided_by, decision_reason, decided_at, expires_at,
                   metadata, created_at, updated_at
            FROM approvals
            WHERE id = $1
            "#,
        )
        .bind(approval_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    /// Process an approval decision (approve or deny).
    ///
    /// Uses the `process_approval` stored function which handles
    /// concurrency checks and status validation.
    pub async fn decide_approval(
        &self,
        approval_id: Uuid,
        status: &str,
        decided_by: &str,
        reason: Option<&str>,
    ) -> Result<Option<ApprovalRow>> {
        // Call the stored function for atomic processing
        sqlx::query(
            r#"
            SELECT process_approval($1, $2::approval_status, $3, $4)
            "#,
        )
        .bind(approval_id)
        .bind(status)
        .bind(decided_by)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        // Return the updated row
        self.get_approval(approval_id).await
    }

    /// Get approval history (decided approvals) with pagination.
    pub async fn get_approval_history(&self, limit: i64, offset: i64) -> Result<Vec<ApprovalRow>> {
        let rows = sqlx::query_as::<_, ApprovalRow>(
            r#"
            SELECT id, task_id, agent_id, action::text, action_description, action_data,
                   risk_score::float8, risk_factors, cluster_id, status::text,
                   decided_by, decision_reason, decided_at, expires_at,
                   metadata, created_at, updated_at
            FROM approvals
            WHERE status != 'pending'
            ORDER BY decided_at DESC NULLS LAST, updated_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get total count of decided (non-pending) approvals.
    pub async fn get_approval_history_count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM approvals WHERE status != 'pending'"
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }
}
