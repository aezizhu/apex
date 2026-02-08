//! Approval Manager -- creation, tracking, expiration, and escalation of
//! approval requests.

use chrono::{Duration, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info};
use uuid::Uuid;

use super::{
    ApprovalDecision, ApprovalRequest, ApprovalRequestId,
    ApprovalStatus, ApprovalType,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Errors
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors that may occur while managing approvals.
#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("Approval request not found: {0}")]
    RequestNotFound(ApprovalRequestId),

    #[error("Approval request {0} is not in a decidable state (current: {1})")]
    NotPending(ApprovalRequestId, ApprovalStatus),

    #[error("Approval request {0} has expired")]
    Expired(ApprovalRequestId),

    #[error("Duplicate decision for request {0}")]
    DuplicateDecision(ApprovalRequestId),
}

// ═══════════════════════════════════════════════════════════════════════════════
// Notification Hook
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for receiving notifications about approval lifecycle events.
///
/// Implementations can send emails, WebSocket messages, Slack notifications, etc.
pub trait ApprovalNotifier: Send + Sync + 'static {
    /// Called when a new approval request is created.
    fn on_request_created(&self, request: &ApprovalRequest);

    /// Called when a decision is recorded.
    fn on_decision_made(&self, request: &ApprovalRequest, decision: &ApprovalDecision);

    /// Called when a request expires without a decision.
    fn on_request_expired(&self, request: &ApprovalRequest);

    /// Called when a request is escalated to a higher role.
    fn on_request_escalated(&self, request: &ApprovalRequest, escalated_to: &str);
}

/// A no-op notifier for when notifications are not needed.
#[derive(Debug, Clone)]
pub struct NoOpNotifier;

impl ApprovalNotifier for NoOpNotifier {
    fn on_request_created(&self, _request: &ApprovalRequest) {}
    fn on_decision_made(&self, _request: &ApprovalRequest, _decision: &ApprovalDecision) {}
    fn on_request_expired(&self, _request: &ApprovalRequest) {}
    fn on_request_escalated(&self, _request: &ApprovalRequest, _escalated_to: &str) {}
}

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for the approval manager.
#[derive(Debug, Clone)]
pub struct ApprovalManagerConfig {
    /// How long an approval request lives before auto-expiring.
    /// Defaults to 24 hours.
    pub default_ttl: Duration,

    /// How long to wait before escalating an undecided request.
    /// Defaults to 4 hours. Must be less than `default_ttl`.
    pub escalation_timeout: Duration,

    /// The role/user to escalate to when the timeout fires.
    /// Defaults to `"admin"`.
    pub escalation_target: String,
}

impl Default for ApprovalManagerConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::hours(24),
            escalation_timeout: Duration::hours(4),
            escalation_target: "admin".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ApprovalManager
// ═══════════════════════════════════════════════════════════════════════════════

/// Central manager for the approval workflow.
///
/// Thread-safe via `DashMap`. Stores approval requests and decisions in memory
/// and delegates persistence and side-effects through notification hooks.
#[derive(Clone)]
pub struct ApprovalManager {
    /// In-flight approval requests keyed by their id.
    requests: Arc<DashMap<ApprovalRequestId, ApprovalRequest>>,

    /// Recorded decisions keyed by request id (supports history lookup).
    decisions: Arc<DashMap<ApprovalRequestId, Vec<ApprovalDecision>>>,

    /// Configuration.
    config: ApprovalManagerConfig,

    /// Notification hooks.
    notifier: Arc<dyn ApprovalNotifier>,
}

impl ApprovalManager {
    /// Create a new manager with default configuration and no-op notifier.
    pub fn new() -> Self {
        Self::with_config_and_notifier(ApprovalManagerConfig::default(), NoOpNotifier)
    }

    /// Create a new manager with custom configuration and notifier.
    pub fn with_config_and_notifier(
        config: ApprovalManagerConfig,
        notifier: impl ApprovalNotifier,
    ) -> Self {
        Self {
            requests: Arc::new(DashMap::new()),
            decisions: Arc::new(DashMap::new()),
            config,
            notifier: Arc::new(notifier),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Request lifecycle
    // ─────────────────────────────────────────────────────────────────────────

    /// Create and track a new approval request.
    ///
    /// The request is initialised as `Pending` and assigned an expiry based on
    /// the configured `default_ttl`.
    pub fn create_request(
        &self,
        task_id: Uuid,
        agent_id: Uuid,
        approval_type: ApprovalType,
        context: serde_json::Value,
    ) -> ApprovalRequest {
        let expires_at = Utc::now() + self.config.default_ttl;

        let request = ApprovalRequest::new(task_id, agent_id, approval_type, context)
            .with_expiry(expires_at);

        info!(
            request_id = %request.id,
            task_id = %task_id,
            agent_id = %agent_id,
            approval_type = %request.approval_type,
            "Approval request created"
        );

        self.requests.insert(request.id, request.clone());
        self.notifier.on_request_created(&request);

        request
    }

    /// Record a decision (approve or deny) for an existing request.
    pub fn decide(
        &self,
        request_id: ApprovalRequestId,
        approver_id: impl Into<String>,
        approved: bool,
        reason: Option<String>,
    ) -> Result<ApprovalDecision, ApprovalError> {
        let approver: String = approver_id.into();

        let mut request = self
            .requests
            .get_mut(&request_id)
            .ok_or(ApprovalError::RequestNotFound(request_id))?;

        // Must be in a decidable state (Pending or Escalated).
        if request.status != ApprovalStatus::Pending
            && request.status != ApprovalStatus::Escalated
        {
            return Err(ApprovalError::NotPending(request_id, request.status));
        }

        // Check expiry before accepting the decision.
        if request.is_expired() {
            request.status = ApprovalStatus::Expired;
            self.notifier.on_request_expired(&request);
            return Err(ApprovalError::Expired(request_id));
        }

        let decision = if approved {
            request.status = ApprovalStatus::Approved;
            ApprovalDecision::approve(request_id, &approver, reason)
        } else {
            request.status = ApprovalStatus::Denied;
            ApprovalDecision::deny(request_id, &approver, reason)
        };

        info!(
            request_id = %request_id,
            approver = %approver,
            decision = %decision.decision,
            "Approval decision recorded"
        );

        self.notifier.on_decision_made(&request, &decision);

        // Store the decision for history.
        self.decisions
            .entry(request_id)
            .or_insert_with(Vec::new)
            .push(decision.clone());

        Ok(decision)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Query
    // ─────────────────────────────────────────────────────────────────────────

    /// Get a request by id.
    pub fn get_request(&self, id: &ApprovalRequestId) -> Option<ApprovalRequest> {
        self.requests.get(id).map(|r| r.clone())
    }

    /// Get all decisions for a request.
    pub fn get_decisions(&self, request_id: &ApprovalRequestId) -> Vec<ApprovalDecision> {
        self.decisions
            .get(request_id)
            .map(|d| d.clone())
            .unwrap_or_default()
    }

    /// List all pending requests.
    pub fn pending_requests(&self) -> Vec<ApprovalRequest> {
        self.requests
            .iter()
            .filter(|entry| entry.value().status == ApprovalStatus::Pending)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// List all requests for a specific task.
    pub fn requests_for_task(&self, task_id: Uuid) -> Vec<ApprovalRequest> {
        self.requests
            .iter()
            .filter(|entry| entry.value().task_id == task_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// List all requests for a specific agent.
    pub fn requests_for_agent(&self, agent_id: Uuid) -> Vec<ApprovalRequest> {
        self.requests
            .iter()
            .filter(|entry| entry.value().agent_id == agent_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Expiration & Escalation (call periodically)
    // ─────────────────────────────────────────────────────────────────────────

    /// Sweep all pending requests and expire or escalate them as appropriate.
    ///
    /// Call this periodically (e.g., every minute from a background task).
    /// Returns the number of requests that were expired or escalated.
    pub fn sweep_stale_requests(&self) -> SweepResult {
        let now = Utc::now();
        let mut expired_count = 0u64;
        let mut escalated_count = 0u64;

        for mut entry in self.requests.iter_mut() {
            let request = entry.value_mut();

            if request.status != ApprovalStatus::Pending
                && request.status != ApprovalStatus::Escalated
            {
                continue;
            }

            // Check full expiry first.
            if let Some(exp) = request.expires_at {
                if now >= exp {
                    debug!(request_id = %request.id, "Expiring stale approval request");
                    request.status = ApprovalStatus::Expired;
                    self.notifier.on_request_expired(request);
                    expired_count += 1;
                    continue;
                }
            }

            // Check escalation timeout (only for Pending, not already Escalated).
            if request.status == ApprovalStatus::Pending {
                let age = now - request.created_at;
                if age >= self.config.escalation_timeout {
                    debug!(
                        request_id = %request.id,
                        target = %self.config.escalation_target,
                        "Escalating approval request"
                    );
                    request.status = ApprovalStatus::Escalated;
                    request.escalated_to = Some(self.config.escalation_target.clone());
                    self.notifier
                        .on_request_escalated(request, &self.config.escalation_target);
                    escalated_count += 1;
                }
            }
        }

        if expired_count > 0 || escalated_count > 0 {
            info!(
                expired = expired_count,
                escalated = escalated_count,
                "Approval sweep completed"
            );
        }

        SweepResult {
            expired: expired_count,
            escalated: escalated_count,
        }
    }
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ApprovalManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovalManager")
            .field("pending_count", &self.pending_requests().len())
            .field("config", &self.config)
            .finish()
    }
}

/// Result of a `sweep_stale_requests` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepResult {
    /// Number of requests moved to `Expired`.
    pub expired: u64,
    /// Number of requests moved to `Escalated`.
    pub escalated: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Counting notifier for testing.
    struct CountingNotifier {
        created: AtomicU64,
        decided: AtomicU64,
        expired: AtomicU64,
        escalated: AtomicU64,
    }

    impl CountingNotifier {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                created: AtomicU64::new(0),
                decided: AtomicU64::new(0),
                expired: AtomicU64::new(0),
                escalated: AtomicU64::new(0),
            })
        }
    }

    impl ApprovalNotifier for Arc<CountingNotifier> {
        fn on_request_created(&self, _request: &ApprovalRequest) {
            self.created.fetch_add(1, Ordering::Relaxed);
        }
        fn on_decision_made(&self, _request: &ApprovalRequest, _decision: &ApprovalDecision) {
            self.decided.fetch_add(1, Ordering::Relaxed);
        }
        fn on_request_expired(&self, _request: &ApprovalRequest) {
            self.expired.fetch_add(1, Ordering::Relaxed);
        }
        fn on_request_escalated(&self, _request: &ApprovalRequest, _escalated_to: &str) {
            self.escalated.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn make_manager() -> ApprovalManager {
        ApprovalManager::new()
    }

    fn make_manager_with_notifier(
        config: ApprovalManagerConfig,
        notifier: Arc<CountingNotifier>,
    ) -> ApprovalManager {
        ApprovalManager::with_config_and_notifier(config, notifier)
    }

    #[test]
    fn test_create_and_get_request() {
        let mgr = make_manager();
        let task_id = Uuid::new_v4();
        let agent_id = Uuid::new_v4();

        let req = mgr.create_request(
            task_id,
            agent_id,
            ApprovalType::CostOverrun,
            serde_json::json!({"cost": 200}),
        );

        assert!(req.is_pending());
        assert_eq!(req.task_id, task_id);

        let fetched = mgr.get_request(&req.id).unwrap();
        assert_eq!(fetched.id, req.id);
    }

    #[test]
    fn test_approve_request() {
        let mgr = make_manager();
        let req = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );

        let decision = mgr
            .decide(req.id, "alice", true, Some("LGTM".into()))
            .unwrap();
        assert_eq!(decision.decision, ApprovalStatus::Approved);

        let updated = mgr.get_request(&req.id).unwrap();
        assert_eq!(updated.status, ApprovalStatus::Approved);
    }

    #[test]
    fn test_deny_request() {
        let mgr = make_manager();
        let req = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::SecuritySensitive,
            serde_json::json!({}),
        );

        let decision = mgr.decide(req.id, "bob", false, None).unwrap();
        assert_eq!(decision.decision, ApprovalStatus::Denied);
    }

    #[test]
    fn test_cannot_decide_twice() {
        let mgr = make_manager();
        let req = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );

        mgr.decide(req.id, "alice", true, None).unwrap();
        let err = mgr.decide(req.id, "bob", false, None).unwrap_err();
        assert!(matches!(err, ApprovalError::NotPending(_, _)));
    }

    #[test]
    fn test_pending_requests_filter() {
        let mgr = make_manager();

        let req1 = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );
        let _req2 = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::CostOverrun,
            serde_json::json!({}),
        );

        // Approve one
        mgr.decide(req1.id, "admin", true, None).unwrap();

        let pending = mgr.pending_requests();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn test_decision_history() {
        let mgr = make_manager();
        let req = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );

        mgr.decide(req.id, "admin", true, Some("ok".into())).unwrap();

        let history = mgr.get_decisions(&req.id);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].approver_id, "admin");
    }

    #[test]
    fn test_notification_hooks_fire() {
        let notifier = CountingNotifier::new();
        let config = ApprovalManagerConfig::default();
        let mgr = make_manager_with_notifier(config, notifier.clone());

        let req = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );
        assert_eq!(notifier.created.load(Ordering::Relaxed), 1);

        mgr.decide(req.id, "admin", true, None).unwrap();
        assert_eq!(notifier.decided.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_sweep_expires_old_requests() {
        let config = ApprovalManagerConfig {
            default_ttl: Duration::seconds(0), // expire immediately
            escalation_timeout: Duration::seconds(0),
            escalation_target: "admin".into(),
        };
        let notifier = CountingNotifier::new();
        let mgr = make_manager_with_notifier(config, notifier.clone());

        mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );

        // Small sleep to ensure the expiry passes.
        std::thread::sleep(std::time::Duration::from_millis(10));

        let result = mgr.sweep_stale_requests();
        // With a 0-second TTL and 0-second escalation, the sweep should
        // first try to escalate (Pending -> Escalated) or expire.
        // Since expiry check comes first and expires_at <= now, it expires.
        assert!(result.expired > 0 || result.escalated > 0);
    }

    #[test]
    fn test_escalation() {
        let config = ApprovalManagerConfig {
            default_ttl: Duration::hours(24),
            escalation_timeout: Duration::seconds(0), // escalate immediately
            escalation_target: "cto".into(),
        };
        let notifier = CountingNotifier::new();
        let mgr = make_manager_with_notifier(config, notifier.clone());

        let req = mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ApprovalType::SecuritySensitive,
            serde_json::json!({}),
        );

        std::thread::sleep(std::time::Duration::from_millis(10));

        let result = mgr.sweep_stale_requests();
        assert_eq!(result.escalated, 1);

        let updated = mgr.get_request(&req.id).unwrap();
        assert_eq!(updated.status, ApprovalStatus::Escalated);
        assert_eq!(updated.escalated_to, Some("cto".into()));

        assert_eq!(notifier.escalated.load(Ordering::Relaxed), 1);

        // Escalated requests can still be decided.
        let decision = mgr.decide(req.id, "cto", true, None).unwrap();
        assert_eq!(decision.decision, ApprovalStatus::Approved);
    }

    #[test]
    fn test_requests_for_agent() {
        let mgr = make_manager();
        let agent_id = Uuid::new_v4();

        mgr.create_request(
            Uuid::new_v4(),
            agent_id,
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );
        mgr.create_request(
            Uuid::new_v4(),
            agent_id,
            ApprovalType::CostOverrun,
            serde_json::json!({}),
        );
        mgr.create_request(
            Uuid::new_v4(),
            Uuid::new_v4(), // different agent
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );

        let results = mgr.requests_for_agent(agent_id);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_request_not_found() {
        let mgr = make_manager();
        assert!(mgr.get_request(&ApprovalRequestId::new()).is_none());
    }

    #[test]
    fn test_get_decisions_empty_for_unknown_request() {
        let mgr = make_manager();
        let decisions = mgr.get_decisions(&ApprovalRequestId::new());
        assert!(decisions.is_empty());
    }

    #[test]
    fn test_decide_nonexistent_request() {
        let mgr = make_manager();
        let err = mgr
            .decide(ApprovalRequestId::new(), "admin", true, None)
            .unwrap_err();
        assert!(matches!(err, ApprovalError::RequestNotFound(_)));
    }

    #[test]
    fn test_default_manager() {
        let mgr = ApprovalManager::default();
        assert!(mgr.pending_requests().is_empty());
    }

    #[test]
    fn test_debug_impl() {
        let mgr = make_manager();
        let debug_str = format!("{:?}", mgr);
        assert!(debug_str.contains("ApprovalManager"));
        assert!(debug_str.contains("pending_count"));
    }

    #[test]
    fn test_sweep_no_stale_requests() {
        let mgr = make_manager();
        let result = mgr.sweep_stale_requests();
        assert_eq!(result.expired, 0);
        assert_eq!(result.escalated, 0);
    }

    #[test]
    fn test_sweep_result_eq() {
        let a = SweepResult { expired: 1, escalated: 2 };
        let b = SweepResult { expired: 1, escalated: 2 };
        let c = SweepResult { expired: 0, escalated: 0 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_requests_for_task() {
        let mgr = make_manager();
        let task_id = Uuid::new_v4();

        mgr.create_request(
            task_id,
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );
        mgr.create_request(
            task_id,
            Uuid::new_v4(),
            ApprovalType::CostOverrun,
            serde_json::json!({}),
        );
        mgr.create_request(
            Uuid::new_v4(), // different task
            Uuid::new_v4(),
            ApprovalType::TaskExecution,
            serde_json::json!({}),
        );

        let results = mgr.requests_for_task(task_id);
        assert_eq!(results.len(), 2);
    }
}
