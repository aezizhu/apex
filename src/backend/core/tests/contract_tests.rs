//! Integration tests for Agent Contracts.

use apex_core::contracts::{AgentContract, ResourceLimits, ContractStatus};
use apex_core::error::ErrorCode;
use uuid::Uuid;

fn test_agent_id() -> Uuid {
    Uuid::new_v4()
}

fn test_task_id() -> Uuid {
    Uuid::new_v4()
}

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
    let contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    assert_eq!(contract.status, ContractStatus::Active);
    assert_eq!(contract.usage.tokens_used, 0);
    assert_eq!(contract.usage.cost_used, 0.0);
}

#[test]
fn test_token_recording() {
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    // Record some tokens
    contract.record_tokens(5000).unwrap();
    assert_eq!(contract.usage.tokens_used, 5000);

    // Record more tokens
    contract.record_tokens(3000).unwrap();
    assert_eq!(contract.usage.tokens_used, 8000);

    // Still active
    assert_eq!(contract.status, ContractStatus::Active);
}

#[test]
fn test_token_limit_exceeded() {
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    // Use exactly the limit
    contract.record_tokens(10000).unwrap();

    // Try to use more - should fail
    let result = contract.record_tokens(1);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::TokenLimitExceeded);
    assert_eq!(contract.status, ContractStatus::Exceeded);
}

#[test]
fn test_cost_limit_exceeded() {
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    // Use most of the budget
    contract.record_cost(0.9).unwrap();

    // Try to exceed - should fail
    let result = contract.record_cost(0.2);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::CostLimitExceeded);
    assert_eq!(contract.status, ContractStatus::Exceeded);
}

#[test]
fn test_api_call_limit_exceeded() {
    let limits = ResourceLimits {
        api_call_limit: 5,
        ..test_limits()
    };
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), limits);

    // Make allowed calls
    for _ in 0..5 {
        contract.record_api_call().unwrap();
    }

    // Next call should fail
    let result = contract.record_api_call();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::ApiCallLimitExceeded);
}

#[test]
fn test_child_contract_conservation_law() {
    let mut parent = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    // Use some of parent's budget
    parent.record_tokens(5000).unwrap();

    // Child requesting exactly remaining should succeed
    let child_limits = ResourceLimits {
        token_limit: 5000,
        cost_limit: 1.0,
        api_call_limit: 100,
        time_limit_seconds: 300,
    };

    let child = parent.create_child(test_agent_id(), test_task_id(), child_limits);
    assert!(child.is_ok());
}

#[test]
fn test_child_contract_conservation_violation() {
    let mut parent = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    // Use some of parent's budget
    parent.record_tokens(5000).unwrap();

    // Child requesting more than remaining should fail
    let child_limits = ResourceLimits {
        token_limit: 6000, // Only 5000 remaining!
        cost_limit: 1.0,
        api_call_limit: 100,
        time_limit_seconds: 300,
    };

    let result = parent.create_child(test_agent_id(), test_task_id(), child_limits);
    assert!(result.is_err());
}

#[test]
fn test_remaining_budget() {
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    contract.record_tokens(3000).unwrap();
    contract.record_cost(0.4).unwrap();

    let remaining = contract.remaining();
    assert_eq!(remaining.tokens_used, 7000);
    assert!((remaining.cost_used - 0.6).abs() < 0.001);
}

#[test]
fn test_utilization_calculation() {
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    contract.record_tokens(5000).unwrap(); // 50%
    contract.record_cost(0.25).unwrap();   // 25%

    let util = contract.utilization();
    assert!((util.tokens - 50.0).abs() < 0.1);
    assert!((util.cost - 25.0).abs() < 0.1);
}

#[test]
fn test_contract_completion() {
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    contract.complete();
    assert_eq!(contract.status, ContractStatus::Completed);
}

#[test]
fn test_contract_cancellation() {
    let mut contract = AgentContract::new(test_agent_id(), test_task_id(), test_limits());

    contract.cancel();
    assert_eq!(contract.status, ContractStatus::Cancelled);
}

#[test]
fn test_resource_limits_presets() {
    let simple = ResourceLimits::simple();
    let medium = ResourceLimits::medium();
    let complex = ResourceLimits::complex();

    // Simple should fit within medium
    assert!(simple.fits_within(&medium));

    // Medium should fit within complex
    assert!(medium.fits_within(&complex));

    // Complex should not fit within simple
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

    // Overhead + allocatable should equal original
    assert_eq!(overhead.token_limit + allocatable.token_limit, limits.token_limit);
}
