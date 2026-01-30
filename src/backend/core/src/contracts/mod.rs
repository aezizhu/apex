//! Agent Contract Framework
//!
//! Enforces resource limits and ensures agents operate within defined budgets.
//! Implements the "conservation law": parent contract budget >= sum(child budgets) + overhead.

mod limits;
mod enforcement;
mod tracker;

pub use limits::ResourceLimits;
pub use enforcement::ContractEnforcer;
pub use tracker::UsageTracker;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Duration, Utc};

use crate::error::{ApexError, Result};

/// An agent contract defining resource limits and execution bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContract {
    /// Unique identifier
    pub id: Uuid,

    /// Agent this contract is assigned to
    pub agent_id: Uuid,

    /// Task this contract governs
    pub task_id: Uuid,

    /// Parent contract (if this is a sub-contract)
    pub parent_contract_id: Option<Uuid>,

    /// Resource limits
    pub limits: ResourceLimits,

    /// Current usage tracking
    pub usage: ResourceUsage,

    /// Contract status
    pub status: ContractStatus,

    /// When the contract was created
    pub created_at: DateTime<Utc>,

    /// When the contract expires (hard deadline)
    pub expires_at: DateTime<Utc>,

    /// Child contracts spawned from this one
    pub child_contracts: Vec<Uuid>,
}

/// Current resource usage against a contract.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Tokens consumed
    pub tokens_used: u64,

    /// Cost in dollars
    pub cost_used: f64,

    /// API calls made
    pub api_calls_used: u64,

    /// Wall-clock time elapsed (seconds)
    pub time_elapsed_secs: u64,
}

/// Status of a contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    /// Contract is active and within limits
    Active,
    /// Contract completed successfully
    Completed,
    /// Contract exceeded one or more limits
    Exceeded,
    /// Contract was cancelled
    Cancelled,
}

impl AgentContract {
    /// Create a new contract for an agent-task pair.
    pub fn new(agent_id: Uuid, task_id: Uuid, limits: ResourceLimits) -> Self {
        let now = Utc::now();
        let expires_at = now + Duration::seconds(limits.time_limit_seconds as i64);

        Self {
            id: Uuid::new_v4(),
            agent_id,
            task_id,
            parent_contract_id: None,
            limits,
            usage: ResourceUsage::default(),
            status: ContractStatus::Active,
            created_at: now,
            expires_at,
            child_contracts: Vec::new(),
        }
    }

    /// Create a sub-contract from this parent contract.
    ///
    /// Enforces the conservation law: child limits must not exceed parent's remaining budget.
    pub fn create_child(&mut self, agent_id: Uuid, task_id: Uuid, child_limits: ResourceLimits) -> Result<Self> {
        // Check conservation law
        let remaining_tokens = self.limits.token_limit.saturating_sub(self.usage.tokens_used);
        let remaining_cost = self.limits.cost_limit - self.usage.cost_used;
        let remaining_time = self.limits.time_limit_seconds.saturating_sub(self.usage.time_elapsed_secs);

        if child_limits.token_limit > remaining_tokens {
            return Err(ApexError::contract_violation(
                remaining_tokens as f64,
                child_limits.token_limit as f64,
            ));
        }

        if child_limits.cost_limit > remaining_cost {
            return Err(ApexError::contract_violation(
                remaining_cost,
                child_limits.cost_limit,
            ));
        }

        if child_limits.time_limit_seconds > remaining_time {
            return Err(ApexError::time_limit_exceeded(
                child_limits.time_limit_seconds,
                remaining_time,
            ));
        }

        let mut child = AgentContract::new(agent_id, task_id, child_limits);
        child.parent_contract_id = Some(self.id);
        child.expires_at = self.expires_at.min(child.expires_at);

        self.child_contracts.push(child.id);

        Ok(child)
    }

    /// Record token usage.
    pub fn record_tokens(&mut self, tokens: u64) -> Result<()> {
        let new_total = self.usage.tokens_used + tokens;

        if new_total > self.limits.token_limit {
            self.status = ContractStatus::Exceeded;
            return Err(ApexError::token_limit_exceeded(new_total, self.limits.token_limit));
        }

        self.usage.tokens_used = new_total;
        Ok(())
    }

    /// Record cost.
    pub fn record_cost(&mut self, cost: f64) -> Result<()> {
        let new_total = self.usage.cost_used + cost;

        if new_total > self.limits.cost_limit {
            self.status = ContractStatus::Exceeded;
            return Err(ApexError::cost_limit_exceeded(new_total, self.limits.cost_limit));
        }

        self.usage.cost_used = new_total;
        Ok(())
    }

    /// Record an API call.
    pub fn record_api_call(&mut self) -> Result<()> {
        let new_total = self.usage.api_calls_used + 1;

        if new_total > self.limits.api_call_limit {
            self.status = ContractStatus::Exceeded;
            return Err(ApexError::api_call_limit_exceeded(new_total, self.limits.api_call_limit));
        }

        self.usage.api_calls_used = new_total;
        Ok(())
    }

    /// Check if contract has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Get remaining budget.
    pub fn remaining(&self) -> ResourceUsage {
        ResourceUsage {
            tokens_used: self.limits.token_limit.saturating_sub(self.usage.tokens_used),
            cost_used: (self.limits.cost_limit - self.usage.cost_used).max(0.0),
            api_calls_used: self.limits.api_call_limit.saturating_sub(self.usage.api_calls_used),
            time_elapsed_secs: self.limits.time_limit_seconds.saturating_sub(self.usage.time_elapsed_secs),
        }
    }

    /// Calculate utilization percentage.
    pub fn utilization(&self) -> ContractUtilization {
        ContractUtilization {
            tokens: (self.usage.tokens_used as f64 / self.limits.token_limit as f64 * 100.0).min(100.0),
            cost: (self.usage.cost_used / self.limits.cost_limit * 100.0).min(100.0),
            api_calls: (self.usage.api_calls_used as f64 / self.limits.api_call_limit as f64 * 100.0).min(100.0),
            time: (self.usage.time_elapsed_secs as f64 / self.limits.time_limit_seconds as f64 * 100.0).min(100.0),
        }
    }

    /// Mark contract as completed.
    pub fn complete(&mut self) {
        self.status = ContractStatus::Completed;
    }

    /// Cancel the contract.
    pub fn cancel(&mut self) {
        self.status = ContractStatus::Cancelled;
    }
}

/// Utilization percentages for contract resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractUtilization {
    pub tokens: f64,
    pub cost: f64,
    pub api_calls: f64,
    pub time: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_limits() -> ResourceLimits {
        ResourceLimits {
            token_limit: 10000,
            cost_limit: 1.0,
            api_call_limit: 100,
            time_limit_seconds: 300,
        }
    }

    #[test]
    fn test_contract_creation() {
        let contract = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), test_limits());

        assert_eq!(contract.status, ContractStatus::Active);
        assert_eq!(contract.usage.tokens_used, 0);
    }

    #[test]
    fn test_token_limit_enforcement() {
        let mut contract = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), test_limits());

        // Should succeed
        assert!(contract.record_tokens(5000).is_ok());

        // Should fail (exceeds limit)
        let result = contract.record_tokens(6000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), crate::error::ErrorCode::TokenLimitExceeded);
        assert_eq!(contract.status, ContractStatus::Exceeded);
    }

    #[test]
    fn test_child_contract_conservation() {
        let mut parent = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), test_limits());

        // Use some of parent's budget
        parent.record_tokens(5000).unwrap();

        // Child requesting more than remaining should fail
        let child_limits = ResourceLimits {
            token_limit: 6000, // Only 5000 remaining
            ..test_limits()
        };

        let result = parent.create_child(Uuid::new_v4(), Uuid::new_v4(), child_limits);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), crate::error::ErrorCode::ContractViolation);

        // Child requesting within budget should succeed
        let valid_limits = ResourceLimits {
            token_limit: 4000,
            ..test_limits()
        };

        let child = parent.create_child(Uuid::new_v4(), Uuid::new_v4(), valid_limits);
        assert!(child.is_ok());
    }
}
