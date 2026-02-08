//! Approval API handlers for V1.
//!
//! Provides endpoints for listing, viewing, and deciding on approval requests
//! that require human oversight before an agent action proceeds.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{AppState, ApiResponse};
use crate::api::middleware::{sanitize_string, ValidationErrors};
use crate::middleware::auth::AuthContext;

// ═══════════════════════════════════════════════════════════════════════════════
// Query / Request / Response types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DecideApprovalRequest {
    pub status: String,
    pub reason: Option<String>,
}

impl DecideApprovalRequest {
    fn sanitize(&mut self) {
        self.status = sanitize_string(&self.status);
        if let Some(ref mut reason) = self.reason {
            *reason = sanitize_string(reason);
        }
    }

    fn validate(&self) -> ValidationErrors {
        let mut errors = ValidationErrors::new();
        match self.status.as_str() {
            "approved" | "denied" => {}
            _ => {
                errors.add("status", "must be either 'approved' or 'denied'");
            }
        }
        if let Some(ref reason) = self.reason {
            if reason.len() > 5000 {
                errors.add("reason", "must be at most 5,000 characters");
            }
        }
        errors
    }
}

#[derive(Serialize)]
pub struct ApprovalResponse {
    pub id: Uuid,
    pub task_id: Uuid,
    pub agent_id: Uuid,
    pub action_type: String,
    pub action_data: serde_json::Value,
    pub risk_score: Option<f64>,
    pub status: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
}

impl From<crate::db::approvals::ApprovalRow> for ApprovalResponse {
    fn from(row: crate::db::approvals::ApprovalRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            agent_id: row.agent_id,
            action_type: row.action,
            action_data: row.action_data,
            risk_score: row.risk_score,
            status: row.status,
            created_at: row.created_at.to_rfc3339(),
            expires_at: row.expires_at.map(|t| t.to_rfc3339()),
            decided_at: row.decided_at.map(|t| t.to_rfc3339()),
            decided_by: row.decided_by,
            decision_reason: row.decision_reason,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// GET /api/v1/approvals
///
/// List pending approval requests with pagination.
pub async fn list_approvals(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = pagination.limit.unwrap_or(50).min(200).max(1);
    let offset = pagination.offset.unwrap_or(0).max(0);

    let total = match state.db.get_pending_approval_count().await {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::from_apex_error(&e)),
    };

    match state.db.get_pending_approvals(limit, offset).await {
        Ok(rows) => {
            let approvals: Vec<ApprovalResponse> = rows.into_iter().map(Into::into).collect();
            Json(ApiResponse::success(serde_json::json!({
                "items": approvals,
                "total": total,
                "limit": limit,
                "offset": offset,
            })))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

/// GET /api/v1/approvals/history
///
/// List past approval decisions with pagination.
pub async fn list_approval_history(
    State(state): State<AppState>,
    Query(pagination): Query<PaginationQuery>,
) -> impl IntoResponse {
    let limit = pagination.limit.unwrap_or(50).min(200).max(1);
    let offset = pagination.offset.unwrap_or(0).max(0);

    let total = match state.db.get_approval_history_count().await {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::from_apex_error(&e)),
    };

    match state.db.get_approval_history(limit, offset).await {
        Ok(rows) => {
            let approvals: Vec<ApprovalResponse> = rows.into_iter().map(Into::into).collect();
            Json(ApiResponse::success(serde_json::json!({
                "items": approvals,
                "total": total,
                "limit": limit,
                "offset": offset,
            })))
        }
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

/// GET /api/v1/approvals/:id
///
/// Get a single approval request by ID.
pub async fn get_approval(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state.db.get_approval(id).await {
        Ok(Some(row)) => {
            let response: ApprovalResponse = row.into();
            Json(ApiResponse::success(response))
        }
        Ok(None) => Json(ApiResponse::error("Approval not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}

/// POST /api/v1/approvals/:id/decide
///
/// Approve or deny a pending approval request.
///
/// Request body:
/// ```json
/// {
///   "status": "approved" | "denied",
///   "reason": "optional reason string"
/// }
/// ```
pub async fn decide_approval(
    State(state): State<AppState>,
    auth: Option<AuthContext>,
    Path(id): Path<Uuid>,
    Json(mut req): Json<DecideApprovalRequest>,
) -> impl IntoResponse {
    req.sanitize();
    let errors = req.validate();
    if !errors.is_empty() {
        return Json(ApiResponse::error_with_code(
            serde_json::to_string(&errors).unwrap_or_else(|_| "Validation failed".to_string()),
            "VALIDATION_ERROR",
        ));
    }

    let decided_by = auth
        .map(|ctx| ctx.name.unwrap_or(ctx.user_id))
        .unwrap_or_else(|| "system".to_string());
    let decided_by = decided_by.as_str();

    match state.db.decide_approval(id, &req.status, decided_by, req.reason.as_deref()).await {
        Ok(Some(row)) => {
            let response: ApprovalResponse = row.into();
            Json(ApiResponse::success(response))
        }
        Ok(None) => Json(ApiResponse::error("Approval not found")),
        Err(e) => Json(ApiResponse::from_apex_error(&e)),
    }
}
