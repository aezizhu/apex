//! Approval Decision Tracking and History
//!
//! This module provides a structured approval workflow for the Apex orchestration
//! engine. Agents and tasks that require human sign-off (cost overruns, security-
//! sensitive operations, etc.) go through the approval pipeline defined here.
//!
//! - **Models** (`mod.rs`): Core data types -- `ApprovalRequest`, `ApprovalDecision`,
//!   `ApprovalStatus`, `ApprovalType`.
//! - **Manager** (`manager.rs`): `ApprovalManager` for creating, tracking,
//!   auto-expiring, and escalating approval requests.
//! - **Policy** (`policy.rs`): `ApprovalPolicy` for declaratively defining when
//!   approvals are required (cost thresholds, security levels, custom rules).

pub mod manager;
pub mod policy;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════════
// Identifiers
// ═══════════════════════════════════════════════════════════════════════════════

/// Strongly-typed approval request identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalRequestId(pub Uuid);

impl ApprovalRequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ApprovalRequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ApprovalRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed approval decision identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalDecisionId(pub Uuid);

impl ApprovalDecisionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ApprovalDecisionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ApprovalDecisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Enums
// ═══════════════════════════════════════════════════════════════════════════════

/// The lifecycle status of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Awaiting a decision.
    Pending,
    /// Approved by an authorised approver.
    Approved,
    /// Denied by an authorised approver.
    Denied,
    /// No decision was made before the deadline.
    Expired,
    /// Automatically escalated to a higher-level approver.
    Escalated,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::Escalated => "escalated",
        }
    }

    /// Parse from a string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "denied" => Some(Self::Denied),
            "expired" => Some(Self::Expired),
            "escalated" => Some(Self::Escalated),
            _ => None,
        }
    }

    /// Returns `true` if the status represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Approved | Self::Denied | Self::Expired)
    }
}

impl fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The category of action that triggered the approval requirement.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalType {
    /// Standard task execution requiring sign-off.
    TaskExecution,
    /// The projected or actual cost exceeds a threshold.
    CostOverrun,
    /// The operation touches security-sensitive resources.
    SecuritySensitive,
    /// A user-defined rule triggered the approval.
    Custom(String),
}

impl ApprovalType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::TaskExecution => "task_execution",
            Self::CostOverrun => "cost_overrun",
            Self::SecuritySensitive => "security_sensitive",
            Self::Custom(name) => name.as_str(),
        }
    }
}

impl fmt::Display for ApprovalType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ApprovalRequest
// ═══════════════════════════════════════════════════════════════════════════════

/// A request for human approval before an action may proceed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique identifier for this request.
    pub id: ApprovalRequestId,
    /// The task that requires approval.
    pub task_id: Uuid,
    /// The agent that initiated the action.
    pub agent_id: Uuid,
    /// Why approval is required.
    pub approval_type: ApprovalType,
    /// Freeform context to help the approver decide (e.g., cost details, risk factors).
    pub context: serde_json::Value,
    /// Current lifecycle status.
    pub status: ApprovalStatus,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
    /// Deadline after which the request expires automatically.
    pub expires_at: Option<DateTime<Utc>>,
    /// If escalated, the escalation target (e.g., a role or user id).
    pub escalated_to: Option<String>,
}

impl ApprovalRequest {
    /// Create a new pending approval request.
    pub fn new(
        task_id: Uuid,
        agent_id: Uuid,
        approval_type: ApprovalType,
        context: serde_json::Value,
    ) -> Self {
        Self {
            id: ApprovalRequestId::new(),
            task_id,
            agent_id,
            approval_type,
            context,
            status: ApprovalStatus::Pending,
            created_at: Utc::now(),
            expires_at: None,
            escalated_to: None,
        }
    }

    /// Set a deadline for auto-expiration.
    pub fn with_expiry(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Returns `true` if the request has passed its deadline.
    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |exp| Utc::now() >= exp)
    }

    /// Returns `true` if the request is still awaiting a decision.
    pub fn is_pending(&self) -> bool {
        self.status == ApprovalStatus::Pending
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ApprovalDecision
// ═══════════════════════════════════════════════════════════════════════════════

/// A recorded decision against an `ApprovalRequest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    /// Unique identifier for this decision.
    pub id: ApprovalDecisionId,
    /// The request this decision applies to.
    pub request_id: ApprovalRequestId,
    /// Who made the decision (user id, role, or system).
    pub approver_id: String,
    /// The outcome.
    pub decision: ApprovalStatus,
    /// Optional human-readable reason.
    pub reason: Option<String>,
    /// When the decision was made.
    pub decided_at: DateTime<Utc>,
}

impl ApprovalDecision {
    /// Record an approval.
    pub fn approve(
        request_id: ApprovalRequestId,
        approver_id: impl Into<String>,
        reason: Option<String>,
    ) -> Self {
        Self {
            id: ApprovalDecisionId::new(),
            request_id,
            approver_id: approver_id.into(),
            decision: ApprovalStatus::Approved,
            reason,
            decided_at: Utc::now(),
        }
    }

    /// Record a denial.
    pub fn deny(
        request_id: ApprovalRequestId,
        approver_id: impl Into<String>,
        reason: Option<String>,
    ) -> Self {
        Self {
            id: ApprovalDecisionId::new(),
            request_id,
            approver_id: approver_id.into(),
            decision: ApprovalStatus::Denied,
            reason,
            decided_at: Utc::now(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_status_roundtrip() {
        for status in &[
            ApprovalStatus::Pending,
            ApprovalStatus::Approved,
            ApprovalStatus::Denied,
            ApprovalStatus::Expired,
            ApprovalStatus::Escalated,
        ] {
            let parsed = ApprovalStatus::from_str_loose(status.as_str()).unwrap();
            assert_eq!(*status, parsed);
        }
    }

    #[test]
    fn test_approval_status_terminal() {
        assert!(!ApprovalStatus::Pending.is_terminal());
        assert!(ApprovalStatus::Approved.is_terminal());
        assert!(ApprovalStatus::Denied.is_terminal());
        assert!(ApprovalStatus::Expired.is_terminal());
        assert!(!ApprovalStatus::Escalated.is_terminal());
    }

    #[test]
    fn test_approval_request_defaults_to_pending() {
        let req = ApprovalRequest::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::CostOverrun,
            serde_json::json!({"projected_cost": 150.0}),
        );
        assert!(req.is_pending());
        assert!(!req.is_expired());
    }

    #[test]
    fn test_approval_request_expiry() {
        let req = ApprovalRequest::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        )
        .with_expiry(Utc::now() - chrono::Duration::hours(1));

        assert!(req.is_expired());
    }

    #[test]
    fn test_approval_decision_approve() {
        let req_id = ApprovalRequestId::new();
        let decision = ApprovalDecision::approve(req_id, "admin-user", Some("Looks good".into()));
        assert_eq!(decision.request_id, req_id);
        assert_eq!(decision.decision, ApprovalStatus::Approved);
        assert_eq!(decision.approver_id, "admin-user");
    }

    #[test]
    fn test_approval_decision_deny() {
        let req_id = ApprovalRequestId::new();
        let decision = ApprovalDecision::deny(req_id, "security-team", Some("Too risky".into()));
        assert_eq!(decision.decision, ApprovalStatus::Denied);
    }

    #[test]
    fn test_approval_type_display() {
        assert_eq!(ApprovalType::TaskExecution.as_str(), "task_execution");
        assert_eq!(ApprovalType::CostOverrun.as_str(), "cost_overrun");
        assert_eq!(ApprovalType::SecuritySensitive.as_str(), "security_sensitive");
        assert_eq!(
            ApprovalType::Custom("data_export".into()).as_str(),
            "data_export"
        );
    }
}
