//! Resource limit definitions.

use serde::{Deserialize, Serialize};

/// Resource limits for an agent contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum tokens that can be consumed
    pub token_limit: u64,

    /// Maximum cost in dollars
    pub cost_limit: f64,

    /// Maximum number of API calls
    pub api_call_limit: u64,

    /// Maximum execution time in seconds
    pub time_limit_seconds: u64,
}

impl ResourceLimits {
    /// Create limits suitable for a simple task.
    pub fn simple() -> Self {
        Self {
            token_limit: 4_000,
            cost_limit: 0.05,
            api_call_limit: 10,
            time_limit_seconds: 60,
        }
    }

    /// Create limits suitable for a medium complexity task.
    pub fn medium() -> Self {
        Self {
            token_limit: 20_000,
            cost_limit: 0.25,
            api_call_limit: 50,
            time_limit_seconds: 300,
        }
    }

    /// Create limits suitable for a complex task.
    pub fn complex() -> Self {
        Self {
            token_limit: 100_000,
            cost_limit: 2.00,
            api_call_limit: 200,
            time_limit_seconds: 900,
        }
    }

    /// Create limits suitable for a long-running task.
    pub fn long_running() -> Self {
        Self {
            token_limit: 500_000,
            cost_limit: 10.00,
            api_call_limit: 1000,
            time_limit_seconds: 3600,
        }
    }

    /// Calculate overhead for spawning sub-contracts.
    /// Returns 10% of limits reserved for orchestration overhead.
    pub fn overhead(&self) -> Self {
        Self {
            token_limit: self.token_limit / 10,
            cost_limit: self.cost_limit / 10.0,
            api_call_limit: self.api_call_limit / 10,
            time_limit_seconds: self.time_limit_seconds / 10,
        }
    }

    /// Calculate maximum allocatable to children (excluding overhead).
    pub fn allocatable(&self) -> Self {
        Self {
            token_limit: self.token_limit * 9 / 10,
            cost_limit: self.cost_limit * 0.9,
            api_call_limit: self.api_call_limit * 9 / 10,
            time_limit_seconds: self.time_limit_seconds * 9 / 10,
        }
    }

    /// Check if these limits are within another set of limits.
    pub fn fits_within(&self, other: &ResourceLimits) -> bool {
        self.token_limit <= other.token_limit
            && self.cost_limit <= other.cost_limit
            && self.api_call_limit <= other.api_call_limit
            && self.time_limit_seconds <= other.time_limit_seconds
    }

    /// Calculate estimated cost per token based on model pricing.
    pub fn estimated_cost_per_token(model: &str) -> f64 {
        match model {
            // OpenAI
            "gpt-4" | "gpt-4-turbo" => 0.00003,
            "gpt-4o" => 0.000005,
            "gpt-4o-mini" => 0.00000015,
            "gpt-3.5-turbo" => 0.0000005,

            // Anthropic
            "claude-3-opus" => 0.000015,
            "claude-3-sonnet" | "claude-3.5-sonnet" => 0.000003,
            "claude-3-haiku" | "claude-3.5-haiku" => 0.00000025,

            // Default (conservative estimate)
            _ => 0.00001,
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self::medium()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limits_hierarchy() {
        let simple = ResourceLimits::simple();
        let medium = ResourceLimits::medium();
        let complex = ResourceLimits::complex();

        assert!(simple.fits_within(&medium));
        assert!(medium.fits_within(&complex));
        assert!(!complex.fits_within(&simple));
    }

    #[test]
    fn test_overhead_calculation() {
        let limits = ResourceLimits {
            token_limit: 10000,
            cost_limit: 1.0,
            api_call_limit: 100,
            time_limit_seconds: 300,
        };

        let overhead = limits.overhead();
        let allocatable = limits.allocatable();

        assert_eq!(overhead.token_limit + allocatable.token_limit, limits.token_limit);
    }
}
