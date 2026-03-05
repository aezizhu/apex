//! Contract Enforcement - Validates resource limits before each operation.
//!
//! The `ContractEnforcer` is responsible for:
//! - Pre-operation validation against contract limits
//! - Conservation law enforcement for parent-child contracts
//! - Soft and hard limit handling
//! - Enforcement policy configuration

use parking_lot::RwLock;

use super::{AgentContract, ContractStatus, ResourceLimits, ResourceUsage};
use crate::error::{ApexError, Result};

/// Threshold levels for limit warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThresholdLevel {
    /// Normal - under 70% usage
    Normal,
    /// Warning - 70-90% usage
    Warning,
    /// Critical - over 90% usage
    Critical,
    /// Exceeded - over 100% usage
    Exceeded,
}

impl ThresholdLevel {
    /// Get threshold level from a percentage (0.0 - 1.0+).
    pub fn from_percentage(pct: f64) -> Self {
        if pct >= 1.0 {
            ThresholdLevel::Exceeded
        } else if pct >= 0.9 {
            ThresholdLevel::Critical
        } else if pct >= 0.7 {
            ThresholdLevel::Warning
        } else {
            ThresholdLevel::Normal
        }
    }
}

/// Configuration for contract enforcement.
#[derive(Debug, Clone)]
pub struct EnforcementConfig {
    /// Whether to enforce token limits
    pub enforce_tokens: bool,
    /// Whether to enforce cost limits
    pub enforce_cost: bool,
    /// Whether to enforce API call limits
    pub enforce_api_calls: bool,
    /// Whether to enforce time limits
    pub enforce_time: bool,
    /// Warning threshold (0.0 - 1.0)
    pub warning_threshold: f64,
    /// Critical threshold (0.0 - 1.0)
    pub critical_threshold: f64,
    /// Whether to allow soft limit overruns
    pub allow_soft_overrun: bool,
    /// Maximum soft overrun percentage (e.g., 0.1 for 10%)
    pub max_soft_overrun: f64,
}

impl Default for EnforcementConfig {
    fn default() -> Self {
        Self {
            enforce_tokens: true,
            enforce_cost: true,
            enforce_api_calls: true,
            enforce_time: true,
            warning_threshold: 0.7,
            critical_threshold: 0.9,
            allow_soft_overrun: false,
            max_soft_overrun: 0.1,
        }
    }
}

/// Result of a pre-operation validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the operation is allowed
    pub allowed: bool,
    /// Resource that would be exceeded (if not allowed)
    pub exceeded_resource: Option<String>,
    /// Threshold levels for each resource
    pub token_level: ThresholdLevel,
    pub cost_level: ThresholdLevel,
    pub api_call_level: ThresholdLevel,
    pub time_level: ThresholdLevel,
    /// Warnings to emit
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a successful validation result.
    pub fn ok() -> Self {
        Self {
            allowed: true,
            exceeded_resource: None,
            token_level: ThresholdLevel::Normal,
            cost_level: ThresholdLevel::Normal,
            api_call_level: ThresholdLevel::Normal,
            time_level: ThresholdLevel::Normal,
            warnings: Vec::new(),
        }
    }

    /// Create a failed validation result.
    pub fn denied(resource: impl Into<String>) -> Self {
        Self {
            allowed: false,
            exceeded_resource: Some(resource.into()),
            token_level: ThresholdLevel::Normal,
            cost_level: ThresholdLevel::Normal,
            api_call_level: ThresholdLevel::Normal,
            time_level: ThresholdLevel::Normal,
            warnings: Vec::new(),
        }
    }

    /// Check if any resource is at warning level or above.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
            || self.token_level >= ThresholdLevel::Warning
            || self.cost_level >= ThresholdLevel::Warning
            || self.api_call_level >= ThresholdLevel::Warning
            || self.time_level >= ThresholdLevel::Warning
    }

    /// Check if any resource is at critical level.
    pub fn is_critical(&self) -> bool {
        self.token_level >= ThresholdLevel::Critical
            || self.cost_level >= ThresholdLevel::Critical
            || self.api_call_level >= ThresholdLevel::Critical
            || self.time_level >= ThresholdLevel::Critical
    }
}

/// Validates resource limits and enforces contracts.
pub struct ContractEnforcer {
    /// Enforcement configuration
    config: EnforcementConfig,
    /// Root contract (if any)
    root_contract: RwLock<Option<AgentContract>>,
    /// Total tokens enforced across all contracts
    total_tokens_enforced: RwLock<u64>,
    /// Total operations validated
    validations_performed: RwLock<u64>,
    /// Total operations denied
    validations_denied: RwLock<u64>,
}

impl ContractEnforcer {
    /// Create a new contract enforcer.
    pub fn new(root_contract: Option<AgentContract>) -> Self {
        Self {
            config: EnforcementConfig::default(),
            root_contract: RwLock::new(root_contract),
            total_tokens_enforced: RwLock::new(0),
            validations_performed: RwLock::new(0),
            validations_denied: RwLock::new(0),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: EnforcementConfig, root_contract: Option<AgentContract>) -> Self {
        Self {
            config,
            root_contract: RwLock::new(root_contract),
            total_tokens_enforced: RwLock::new(0),
            validations_performed: RwLock::new(0),
            validations_denied: RwLock::new(0),
        }
    }

    /// Set the root contract.
    pub fn set_root_contract(&self, contract: AgentContract) {
        *self.root_contract.write() = Some(contract);
    }

    /// Validate an operation against a contract.
    ///
    /// This should be called before any resource-consuming operation.
    pub fn validate(
        &self,
        contract: &AgentContract,
        estimated_tokens: u64,
        estimated_cost: f64,
    ) -> ValidationResult {
        *self.validations_performed.write() += 1;

        // Check if contract is still active
        if contract.status != ContractStatus::Active {
            *self.validations_denied.write() += 1;
            return ValidationResult::denied("contract_inactive");
        }

        // Check expiration
        if contract.is_expired() {
            *self.validations_denied.write() += 1;
            return ValidationResult::denied("time_expired");
        }

        let mut result = ValidationResult::ok();

        // Calculate projected usage
        let projected_tokens = contract.usage.tokens_used + estimated_tokens;
        let projected_cost = contract.usage.cost_used + estimated_cost;
        let projected_api_calls = contract.usage.api_calls_used + 1;

        // Check token limit
        if self.config.enforce_tokens {
            let token_pct = projected_tokens as f64 / contract.limits.token_limit as f64;
            result.token_level = ThresholdLevel::from_percentage(token_pct);

            if result.token_level == ThresholdLevel::Exceeded {
                if !self.can_soft_overrun(token_pct) {
                    *self.validations_denied.write() += 1;
                    return ValidationResult::denied("tokens");
                }
                result.warnings.push(format!(
                    "Token soft limit exceeded: {:.1}% of limit",
                    token_pct * 100.0
                ));
            } else if result.token_level >= ThresholdLevel::Warning {
                result
                    .warnings
                    .push(format!("Token usage at {:.1}% of limit", token_pct * 100.0));
            }
        }

        // Check cost limit
        if self.config.enforce_cost {
            let cost_pct = projected_cost / contract.limits.cost_limit;
            result.cost_level = ThresholdLevel::from_percentage(cost_pct);

            if result.cost_level == ThresholdLevel::Exceeded {
                if !self.can_soft_overrun(cost_pct) {
                    *self.validations_denied.write() += 1;
                    return ValidationResult::denied("cost");
                }
                result.warnings.push(format!(
                    "Cost soft limit exceeded: {:.1}% of limit",
                    cost_pct * 100.0
                ));
            } else if result.cost_level >= ThresholdLevel::Warning {
                result
                    .warnings
                    .push(format!("Cost at {:.1}% of limit", cost_pct * 100.0));
            }
        }

        // Check API call limit
        if self.config.enforce_api_calls {
            let api_pct = projected_api_calls as f64 / contract.limits.api_call_limit as f64;
            result.api_call_level = ThresholdLevel::from_percentage(api_pct);

            if result.api_call_level == ThresholdLevel::Exceeded {
                if !self.can_soft_overrun(api_pct) {
                    *self.validations_denied.write() += 1;
                    return ValidationResult::denied("api_calls");
                }
                result.warnings.push(format!(
                    "API call soft limit exceeded: {:.1}% of limit",
                    api_pct * 100.0
                ));
            } else if result.api_call_level >= ThresholdLevel::Warning {
                result
                    .warnings
                    .push(format!("API calls at {:.1}% of limit", api_pct * 100.0));
            }
        }

        // Check time limit
        if self.config.enforce_time {
            let time_pct =
                contract.usage.time_elapsed_secs as f64 / contract.limits.time_limit_seconds as f64;
            result.time_level = ThresholdLevel::from_percentage(time_pct);

            if result.time_level == ThresholdLevel::Exceeded {
                *self.validations_denied.write() += 1;
                return ValidationResult::denied("time");
            } else if result.time_level >= ThresholdLevel::Warning {
                result
                    .warnings
                    .push(format!("Time at {:.1}% of limit", time_pct * 100.0));
            }
        }

        // Log warnings
        for warning in &result.warnings {
            tracing::warn!(contract_id = %contract.id, warning = %warning, "Contract limit warning");
        }

        *self.total_tokens_enforced.write() += estimated_tokens;
        result
    }

    /// Validate that a child contract fits within parent's remaining budget.
    pub fn validate_child_contract(&self, child: &AgentContract) -> Result<()> {
        let root = self.root_contract.read();

        if let Some(parent) = root.as_ref() {
            let remaining = parent.remaining();

            // Check token conservation
            if child.limits.token_limit > remaining.tokens_used {
                return Err(ApexError::contract_violation(
                    remaining.tokens_used as f64,
                    child.limits.token_limit as f64,
                ));
            }

            // Check cost conservation
            if child.limits.cost_limit > remaining.cost_used {
                return Err(ApexError::contract_violation(
                    remaining.cost_used,
                    child.limits.cost_limit,
                ));
            }

            // Check time conservation
            if child.limits.time_limit_seconds > remaining.time_elapsed_secs {
                return Err(ApexError::time_limit_exceeded(
                    child.limits.time_limit_seconds,
                    remaining.time_elapsed_secs,
                ));
            }

            tracing::debug!(
                parent_id = %parent.id,
                child_id = %child.id,
                "Child contract validated against parent"
            );
        }

        Ok(())
    }

    /// Check if limits can be allocated from parent to child.
    pub fn can_allocate(&self, requested: &ResourceLimits) -> bool {
        let root = self.root_contract.read();

        match root.as_ref() {
            Some(parent) => {
                let remaining = parent.remaining();
                requested.token_limit <= remaining.tokens_used
                    && requested.cost_limit <= remaining.cost_used
                    && requested.api_call_limit <= remaining.api_calls_used
                    && requested.time_limit_seconds <= remaining.time_elapsed_secs
            }
            None => true, // No root contract means no limits
        }
    }

    /// Record usage against the root contract.
    pub fn record_usage(&self, tokens: u64, cost: f64, api_calls: u64) -> Result<()> {
        let mut root = self.root_contract.write();

        if let Some(contract) = root.as_mut() {
            contract.record_tokens(tokens)?;
            contract.record_cost(cost)?;
            for _ in 0..api_calls {
                contract.record_api_call()?;
            }
        }

        Ok(())
    }

    /// Get current usage from the root contract.
    pub fn current_usage(&self) -> Option<ResourceUsage> {
        self.root_contract.read().as_ref().map(|c| c.usage.clone())
    }

    /// Get remaining budget from the root contract.
    pub fn remaining_budget(&self) -> Option<ResourceUsage> {
        self.root_contract.read().as_ref().map(|c| c.remaining())
    }

    /// Get enforcement statistics.
    pub fn stats(&self) -> EnforcementStats {
        EnforcementStats {
            validations_performed: *self.validations_performed.read(),
            validations_denied: *self.validations_denied.read(),
            total_tokens_enforced: *self.total_tokens_enforced.read(),
            root_contract_active: self.root_contract.read().is_some(),
        }
    }

    /// Check if soft overrun is allowed and within limits.
    fn can_soft_overrun(&self, percentage: f64) -> bool {
        self.config.allow_soft_overrun && percentage <= 1.0 + self.config.max_soft_overrun
    }
}

/// Enforcement statistics.
#[derive(Debug, Clone)]
pub struct EnforcementStats {
    /// Total validations performed
    pub validations_performed: u64,
    /// Total validations denied
    pub validations_denied: u64,
    /// Total tokens that passed enforcement
    pub total_tokens_enforced: u64,
    /// Whether a root contract is active
    pub root_contract_active: bool,
}

impl EnforcementStats {
    /// Calculate denial rate as a percentage.
    pub fn denial_rate(&self) -> f64 {
        if self.validations_performed == 0 {
            0.0
        } else {
            (self.validations_denied as f64 / self.validations_performed as f64) * 100.0
        }
    }
}

/// Builder for creating contract enforcers with custom configuration.
#[allow(dead_code)]
pub struct ContractEnforcerBuilder {
    config: EnforcementConfig,
    root_contract: Option<AgentContract>,
}

#[allow(dead_code)]
impl ContractEnforcerBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: EnforcementConfig::default(),
            root_contract: None,
        }
    }

    /// Set the root contract.
    pub fn root_contract(mut self, contract: AgentContract) -> Self {
        self.root_contract = Some(contract);
        self
    }

    /// Enable or disable token enforcement.
    pub fn enforce_tokens(mut self, enable: bool) -> Self {
        self.config.enforce_tokens = enable;
        self
    }

    /// Enable or disable cost enforcement.
    pub fn enforce_cost(mut self, enable: bool) -> Self {
        self.config.enforce_cost = enable;
        self
    }

    /// Enable or disable API call enforcement.
    pub fn enforce_api_calls(mut self, enable: bool) -> Self {
        self.config.enforce_api_calls = enable;
        self
    }

    /// Enable or disable time enforcement.
    pub fn enforce_time(mut self, enable: bool) -> Self {
        self.config.enforce_time = enable;
        self
    }

    /// Set warning threshold.
    pub fn warning_threshold(mut self, threshold: f64) -> Self {
        self.config.warning_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set critical threshold.
    pub fn critical_threshold(mut self, threshold: f64) -> Self {
        self.config.critical_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Allow soft limit overruns.
    pub fn allow_soft_overrun(mut self, max_overrun: f64) -> Self {
        self.config.allow_soft_overrun = true;
        self.config.max_soft_overrun = max_overrun.clamp(0.0, 0.5);
        self
    }

    /// Build the contract enforcer.
    pub fn build(self) -> ContractEnforcer {
        ContractEnforcer::with_config(self.config, self.root_contract)
    }
}

impl Default for ContractEnforcerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_contract() -> AgentContract {
        AgentContract::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ResourceLimits {
                token_limit: 10000,
                cost_limit: 1.0,
                api_call_limit: 100,
                time_limit_seconds: 300,
            },
        )
    }

    #[test]
    fn test_enforcer_creation() {
        let enforcer = ContractEnforcer::new(None);
        let stats = enforcer.stats();

        assert_eq!(stats.validations_performed, 0);
        assert!(!stats.root_contract_active);
    }

    #[test]
    fn test_validation_allowed() {
        let enforcer = ContractEnforcer::new(None);
        let contract = test_contract();

        let result = enforcer.validate(&contract, 1000, 0.1);

        assert!(result.allowed);
        assert!(result.exceeded_resource.is_none());
        assert_eq!(result.token_level, ThresholdLevel::Normal);
    }

    #[test]
    fn test_validation_denied_tokens() {
        let enforcer = ContractEnforcer::new(None);
        let contract = test_contract();

        // Try to use more than limit
        let result = enforcer.validate(&contract, 15000, 0.1);

        assert!(!result.allowed);
        assert_eq!(result.exceeded_resource, Some("tokens".to_string()));
    }

    #[test]
    fn test_validation_denied_cost() {
        let enforcer = ContractEnforcer::new(None);
        let contract = test_contract();

        // Try to use more cost than limit
        let result = enforcer.validate(&contract, 1000, 1.5);

        assert!(!result.allowed);
        assert_eq!(result.exceeded_resource, Some("cost".to_string()));
    }

    #[test]
    fn test_threshold_levels() {
        let enforcer = ContractEnforcer::new(None);
        let mut contract = test_contract();

        // Use 75% of tokens
        contract.usage.tokens_used = 7500;

        let result = enforcer.validate(&contract, 0, 0.0);

        assert!(result.allowed);
        assert_eq!(result.token_level, ThresholdLevel::Warning);
        assert!(result.has_warnings());
    }

    #[test]
    fn test_critical_threshold() {
        let enforcer = ContractEnforcer::new(None);
        let mut contract = test_contract();

        // Use 95% of tokens
        contract.usage.tokens_used = 9500;

        let result = enforcer.validate(&contract, 0, 0.0);

        assert!(result.allowed);
        assert_eq!(result.token_level, ThresholdLevel::Critical);
        assert!(result.is_critical());
    }

    #[test]
    fn test_soft_overrun() {
        let enforcer = ContractEnforcerBuilder::new()
            .allow_soft_overrun(0.1)
            .build();

        let mut contract = test_contract();
        contract.usage.tokens_used = 9500;

        // 5% overrun should be allowed
        let result = enforcer.validate(&contract, 1000, 0.0);
        assert!(result.allowed);

        // 15% overrun should not be allowed
        let result = enforcer.validate(&contract, 2000, 0.0);
        assert!(!result.allowed);
    }

    #[test]
    fn test_child_contract_validation() {
        let root = test_contract();
        let enforcer = ContractEnforcer::new(Some(root));

        // Child within budget
        let child = AgentContract::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ResourceLimits {
                token_limit: 5000,
                cost_limit: 0.5,
                api_call_limit: 50,
                time_limit_seconds: 150,
            },
        );

        assert!(enforcer.validate_child_contract(&child).is_ok());

        // Child exceeding budget
        let large_child = AgentContract::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ResourceLimits {
                token_limit: 15000, // Exceeds root
                cost_limit: 0.5,
                api_call_limit: 50,
                time_limit_seconds: 150,
            },
        );

        assert!(enforcer.validate_child_contract(&large_child).is_err());
    }

    #[test]
    fn test_can_allocate() {
        let root = test_contract();
        let enforcer = ContractEnforcer::new(Some(root));

        let small_request = ResourceLimits {
            token_limit: 5000,
            cost_limit: 0.5,
            api_call_limit: 50,
            time_limit_seconds: 150,
        };

        assert!(enforcer.can_allocate(&small_request));

        let large_request = ResourceLimits {
            token_limit: 15000,
            cost_limit: 2.0,
            api_call_limit: 200,
            time_limit_seconds: 600,
        };

        assert!(!enforcer.can_allocate(&large_request));
    }

    #[test]
    fn test_record_usage() {
        let root = test_contract();
        let enforcer = ContractEnforcer::new(Some(root));

        enforcer.record_usage(1000, 0.1, 5).unwrap();

        let usage = enforcer.current_usage().unwrap();
        assert_eq!(usage.tokens_used, 1000);
        assert!((usage.cost_used - 0.1).abs() < 0.001);
        assert_eq!(usage.api_calls_used, 5);
    }

    #[test]
    fn test_builder() {
        let enforcer = ContractEnforcerBuilder::new()
            .enforce_tokens(true)
            .enforce_cost(false)
            .warning_threshold(0.8)
            .critical_threshold(0.95)
            .build();

        let stats = enforcer.stats();
        assert!(!stats.root_contract_active);
    }

    #[test]
    fn test_threshold_level_from_percentage() {
        assert_eq!(ThresholdLevel::from_percentage(0.5), ThresholdLevel::Normal);
        assert_eq!(
            ThresholdLevel::from_percentage(0.75),
            ThresholdLevel::Warning
        );
        assert_eq!(
            ThresholdLevel::from_percentage(0.95),
            ThresholdLevel::Critical
        );
        assert_eq!(
            ThresholdLevel::from_percentage(1.1),
            ThresholdLevel::Exceeded
        );
    }

    #[test]
    fn test_inactive_contract_denied() {
        let enforcer = ContractEnforcer::new(None);
        let mut contract = test_contract();
        contract.status = ContractStatus::Exceeded;

        let result = enforcer.validate(&contract, 100, 0.01);

        assert!(!result.allowed);
        assert_eq!(
            result.exceeded_resource,
            Some("contract_inactive".to_string())
        );
    }

    #[test]
    fn test_enforcement_stats() {
        let enforcer = ContractEnforcer::new(None);
        let contract = test_contract();

        // Perform some validations
        enforcer.validate(&contract, 100, 0.01);
        enforcer.validate(&contract, 100, 0.01);
        enforcer.validate(&contract, 20000, 0.01); // Should be denied

        let stats = enforcer.stats();
        assert_eq!(stats.validations_performed, 3);
        assert_eq!(stats.validations_denied, 1);
        assert!((stats.denial_rate() - 33.33).abs() < 1.0);
    }
}
