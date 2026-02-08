//! Approval Policy Engine
//!
//! Defines declarative rules for when an approval is required.
//! Policies are evaluated against an action context and return whether
//! approval is needed (and if so, which `ApprovalType` applies).

use serde::{Deserialize, Serialize};
use tracing::debug;

use super::ApprovalType;

// ═══════════════════════════════════════════════════════════════════════════════
// Policy Rules
// ═══════════════════════════════════════════════════════════════════════════════

/// A single rule that can trigger an approval requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalRule {
    /// Triggers when the projected or actual cost exceeds a dollar threshold.
    CostThreshold {
        /// Maximum allowed cost before approval is required.
        max_cost_dollars: f64,
    },

    /// Triggers when the operation involves resources at or above a security level.
    SecurityLevel {
        /// Minimum security level that requires approval (0 = public, 10 = top secret).
        min_level: u8,
    },

    /// A user-defined rule identified by name; matched against action metadata.
    CustomRule {
        /// Name of the custom rule (must match the `rule_name` key in the action context).
        name: String,
        /// Human-readable description.
        description: String,
    },

    /// Always require approval (useful for high-risk action categories).
    Always,
}

impl ApprovalRule {
    /// Evaluate this rule against an action context.
    ///
    /// Returns `Some(ApprovalType)` if an approval is required, `None` otherwise.
    pub fn evaluate(&self, context: &ActionContext) -> Option<ApprovalType> {
        match self {
            Self::CostThreshold { max_cost_dollars } => {
                if context.estimated_cost > *max_cost_dollars {
                    debug!(
                        cost = context.estimated_cost,
                        threshold = max_cost_dollars,
                        "Cost threshold exceeded -- approval required"
                    );
                    Some(ApprovalType::CostOverrun)
                } else {
                    None
                }
            }
            Self::SecurityLevel { min_level } => {
                if context.security_level >= *min_level {
                    debug!(
                        level = context.security_level,
                        threshold = min_level,
                        "Security level reached -- approval required"
                    );
                    Some(ApprovalType::SecuritySensitive)
                } else {
                    None
                }
            }
            Self::CustomRule { name, .. } => {
                if context.custom_rule_matches.contains(name) {
                    debug!(
                        rule = %name,
                        "Custom rule matched -- approval required"
                    );
                    Some(ApprovalType::Custom(name.clone()))
                } else {
                    None
                }
            }
            Self::Always => {
                debug!("Always-approve rule triggered");
                Some(ApprovalType::TaskExecution)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ActionContext
// ═══════════════════════════════════════════════════════════════════════════════

/// Context about the action being evaluated; fed into policy rules.
#[derive(Debug, Clone, Default)]
pub struct ActionContext {
    /// Estimated cost in dollars for the planned action.
    pub estimated_cost: f64,

    /// Security level of the resources involved (0-10).
    pub security_level: u8,

    /// Set of custom rule names that are considered matched for this action.
    /// Typically populated by the caller based on action metadata.
    pub custom_rule_matches: Vec<String>,
}

impl ActionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.estimated_cost = cost;
        self
    }

    pub fn with_security_level(mut self, level: u8) -> Self {
        self.security_level = level;
        self
    }

    pub fn with_custom_match(mut self, rule_name: impl Into<String>) -> Self {
        self.custom_rule_matches.push(rule_name.into());
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ApprovalPolicy
// ═══════════════════════════════════════════════════════════════════════════════

/// A named collection of rules that determines when approvals are required.
///
/// Rules are evaluated in order. The first matching rule wins and its
/// `ApprovalType` is returned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    /// Human-readable name for this policy.
    pub name: String,

    /// Description of what this policy enforces.
    pub description: String,

    /// Whether this policy is currently active.
    pub enabled: bool,

    /// Ordered list of rules to evaluate.
    pub rules: Vec<ApprovalRule>,
}

impl ApprovalPolicy {
    /// Create a new empty policy.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            enabled: true,
            rules: Vec::new(),
        }
    }

    /// Add a rule.
    pub fn with_rule(mut self, rule: ApprovalRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Disable this policy.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Evaluate all rules against the given context.
    ///
    /// Returns the first matching `ApprovalType`, or `None` if no rule triggers.
    pub fn evaluate(&self, context: &ActionContext) -> Option<ApprovalType> {
        if !self.enabled {
            return None;
        }

        for rule in &self.rules {
            if let Some(approval_type) = rule.evaluate(context) {
                debug!(
                    policy = %self.name,
                    approval_type = %approval_type,
                    "Policy triggered approval requirement"
                );
                return Some(approval_type);
            }
        }

        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PolicySet (convenience for evaluating multiple policies)
// ═══════════════════════════════════════════════════════════════════════════════

/// A collection of policies. Evaluation stops at the first policy that triggers.
#[derive(Debug, Clone, Default)]
pub struct PolicySet {
    policies: Vec<ApprovalPolicy>,
}

impl PolicySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_policy(&mut self, policy: ApprovalPolicy) {
        self.policies.push(policy);
    }

    pub fn with_policy(mut self, policy: ApprovalPolicy) -> Self {
        self.policies.push(policy);
        self
    }

    /// Evaluate all policies. Returns the first triggered `ApprovalType`.
    pub fn evaluate(&self, context: &ActionContext) -> Option<ApprovalType> {
        for policy in &self.policies {
            if let Some(approval_type) = policy.evaluate(context) {
                return Some(approval_type);
            }
        }
        None
    }

    /// Returns `true` if any policy requires an approval for the given context.
    pub fn requires_approval(&self, context: &ActionContext) -> bool {
        self.evaluate(context).is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_threshold_triggers() {
        let policy = ApprovalPolicy::new("cost-guard", "Block high cost")
            .with_rule(ApprovalRule::CostThreshold {
                max_cost_dollars: 100.0,
            });

        let ctx = ActionContext::new().with_cost(150.0);
        let result = policy.evaluate(&ctx);
        assert_eq!(result, Some(ApprovalType::CostOverrun));
    }

    #[test]
    fn test_cost_threshold_passes() {
        let policy = ApprovalPolicy::new("cost-guard", "Block high cost")
            .with_rule(ApprovalRule::CostThreshold {
                max_cost_dollars: 100.0,
            });

        let ctx = ActionContext::new().with_cost(50.0);
        assert!(policy.evaluate(&ctx).is_none());
    }

    #[test]
    fn test_security_level_triggers() {
        let policy = ApprovalPolicy::new("sec-guard", "Block sensitive ops")
            .with_rule(ApprovalRule::SecurityLevel { min_level: 5 });

        let ctx = ActionContext::new().with_security_level(7);
        assert_eq!(
            policy.evaluate(&ctx),
            Some(ApprovalType::SecuritySensitive)
        );
    }

    #[test]
    fn test_security_level_passes() {
        let policy = ApprovalPolicy::new("sec-guard", "Block sensitive ops")
            .with_rule(ApprovalRule::SecurityLevel { min_level: 5 });

        let ctx = ActionContext::new().with_security_level(3);
        assert!(policy.evaluate(&ctx).is_none());
    }

    #[test]
    fn test_custom_rule_triggers() {
        let policy = ApprovalPolicy::new("export-guard", "Block data exports")
            .with_rule(ApprovalRule::CustomRule {
                name: "data_export".into(),
                description: "Any data export".into(),
            });

        let ctx = ActionContext::new().with_custom_match("data_export");
        assert_eq!(
            policy.evaluate(&ctx),
            Some(ApprovalType::Custom("data_export".into()))
        );
    }

    #[test]
    fn test_custom_rule_no_match() {
        let policy = ApprovalPolicy::new("export-guard", "Block data exports")
            .with_rule(ApprovalRule::CustomRule {
                name: "data_export".into(),
                description: "Any data export".into(),
            });

        let ctx = ActionContext::new();
        assert!(policy.evaluate(&ctx).is_none());
    }

    #[test]
    fn test_always_rule() {
        let policy = ApprovalPolicy::new("always", "Always approve")
            .with_rule(ApprovalRule::Always);

        let ctx = ActionContext::new();
        assert_eq!(policy.evaluate(&ctx), Some(ApprovalType::TaskExecution));
    }

    #[test]
    fn test_disabled_policy_skipped() {
        let policy = ApprovalPolicy::new("disabled", "Should not fire")
            .with_rule(ApprovalRule::Always)
            .disabled();

        let ctx = ActionContext::new();
        assert!(policy.evaluate(&ctx).is_none());
    }

    #[test]
    fn test_first_matching_rule_wins() {
        let policy = ApprovalPolicy::new("multi", "Multiple rules")
            .with_rule(ApprovalRule::CostThreshold {
                max_cost_dollars: 50.0,
            })
            .with_rule(ApprovalRule::SecurityLevel { min_level: 3 });

        // Both would match; cost should win (first in list).
        let ctx = ActionContext::new().with_cost(100.0).with_security_level(5);
        assert_eq!(policy.evaluate(&ctx), Some(ApprovalType::CostOverrun));
    }

    #[test]
    fn test_policy_set_evaluation() {
        let p1 = ApprovalPolicy::new("cost", "Cost guard")
            .with_rule(ApprovalRule::CostThreshold {
                max_cost_dollars: 100.0,
            });
        let p2 = ApprovalPolicy::new("sec", "Security guard")
            .with_rule(ApprovalRule::SecurityLevel { min_level: 8 });

        let set = PolicySet::new().with_policy(p1).with_policy(p2);

        // Only security triggers
        let ctx = ActionContext::new().with_cost(50.0).with_security_level(9);
        assert_eq!(set.evaluate(&ctx), Some(ApprovalType::SecuritySensitive));

        // Nothing triggers
        let ctx2 = ActionContext::new().with_cost(10.0).with_security_level(2);
        assert!(!set.requires_approval(&ctx2));
    }

    #[test]
    fn test_policy_set_first_policy_wins() {
        let p1 = ApprovalPolicy::new("always", "Always").with_rule(ApprovalRule::Always);
        let p2 = ApprovalPolicy::new("sec", "Security")
            .with_rule(ApprovalRule::SecurityLevel { min_level: 1 });

        let set = PolicySet::new().with_policy(p1).with_policy(p2);

        let ctx = ActionContext::new().with_security_level(5);
        // The "always" policy fires first.
        assert_eq!(set.evaluate(&ctx), Some(ApprovalType::TaskExecution));
    }
}
