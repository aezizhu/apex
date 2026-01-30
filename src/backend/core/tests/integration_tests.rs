//! Integration tests for the Apex Orchestrator.
//!
//! These tests verify end-to-end functionality across modules.

use apex_core::contracts::{AgentContract, ContractStatus, ResourceLimits};
use apex_core::dag::{Task, TaskDAG, TaskInput, TaskStatus};
use apex_core::error::ErrorCode;
use uuid::Uuid;

// ============================================================================
// Test Utilities
// ============================================================================

fn create_test_limits(tokens: u64, cost: f64, api_calls: u64) -> ResourceLimits {
    ResourceLimits {
        token_limit: tokens,
        cost_limit: cost,
        api_call_limit: api_calls,
        time_limit_seconds: 300,
    }
}

fn create_test_task(name: &str) -> Task {
    Task::new(name, TaskInput::default())
}

// ============================================================================
// DAG + Contract Integration Tests
// ============================================================================

#[test]
fn test_dag_execution_with_contracts() {
    // Create a DAG with multiple tasks
    let mut dag = TaskDAG::new("integration-test");

    let task_a = create_test_task("Task A");
    let task_b = create_test_task("Task B");
    let task_c = create_test_task("Task C");

    let id_a = dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();

    // A -> B -> C dependency chain
    dag.add_dependency(id_a, id_b).unwrap();
    dag.add_dependency(id_b, id_c).unwrap();

    // Create a contract for the entire DAG execution
    let limits = create_test_limits(30000, 3.0, 300);
    let mut contract = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), limits);

    // Simulate execution: Task A
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_a);

    // Record resource usage for Task A
    contract.record_tokens(5000).unwrap();
    contract.record_cost(0.5).unwrap();
    contract.record_api_call().unwrap();

    dag.update_task_status(id_a, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_a, TaskStatus::Running).unwrap();
    dag.update_task_status(id_a, TaskStatus::Completed).unwrap();

    // Simulate execution: Task B
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_b);

    contract.record_tokens(8000).unwrap();
    contract.record_cost(0.8).unwrap();
    contract.record_api_call().unwrap();

    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Completed).unwrap();

    // Simulate execution: Task C
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_c);

    contract.record_tokens(3000).unwrap();
    contract.record_cost(0.3).unwrap();
    contract.record_api_call().unwrap();

    dag.update_task_status(id_c, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_c, TaskStatus::Running).unwrap();
    dag.update_task_status(id_c, TaskStatus::Completed).unwrap();

    // Verify final state
    assert!(dag.is_complete());
    assert_eq!(contract.status, ContractStatus::Active);
    assert_eq!(contract.usage.tokens_used, 16000);
    assert!((contract.usage.cost_used - 1.6).abs() < 0.001);
    assert_eq!(contract.usage.api_calls_used, 3);

    // Mark contract as completed
    contract.complete();
    assert_eq!(contract.status, ContractStatus::Completed);
}

#[test]
fn test_dag_failure_cascading_with_contracts() {
    let mut dag = TaskDAG::new("failure-test");

    let task_a = create_test_task("Task A");
    let task_b = create_test_task("Task B");
    let task_c = create_test_task("Task C");
    let task_d = create_test_task("Task D");

    let id_a = dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();
    let id_d = dag.add_task(task_d).unwrap();

    // A -> B -> C -> D
    dag.add_dependency(id_a, id_b).unwrap();
    dag.add_dependency(id_b, id_c).unwrap();
    dag.add_dependency(id_c, id_d).unwrap();

    // Create contract
    let limits = create_test_limits(10000, 1.0, 100);
    let mut contract = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), limits);

    // Execute Task A successfully
    dag.update_task_status(id_a, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_a, TaskStatus::Running).unwrap();
    dag.update_task_status(id_a, TaskStatus::Completed).unwrap();
    contract.record_tokens(3000).unwrap();

    // Task B fails
    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Failed).unwrap();

    // Cancel all dependent tasks (C and D)
    let cancelled = dag.cancel_dependents(id_b).unwrap();
    assert_eq!(cancelled.len(), 2);
    assert!(cancelled.contains(&id_c));
    assert!(cancelled.contains(&id_d));

    // Verify DAG state
    assert_eq!(dag.get_task(id_c).unwrap().status, TaskStatus::Cancelled);
    assert_eq!(dag.get_task(id_d).unwrap().status, TaskStatus::Cancelled);

    // Contract should still be active (failure doesn't auto-exceed)
    assert_eq!(contract.status, ContractStatus::Active);

    // Mark contract as cancelled due to failure
    contract.cancel();
    assert_eq!(contract.status, ContractStatus::Cancelled);
}

#[test]
fn test_contract_exceeded_stops_dag_execution() {
    let mut dag = TaskDAG::new("exceeded-test");

    let task_a = create_test_task("Task A");
    let task_b = create_test_task("Task B");

    let id_a = dag.add_task(task_a).unwrap();
    let _id_b = dag.add_task(task_b).unwrap();

    dag.add_dependency(id_a, _id_b).unwrap();

    // Create a very limited contract
    let limits = create_test_limits(1000, 0.1, 5);
    let mut contract = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), limits);

    // Task A uses all the budget
    contract.record_tokens(1000).unwrap();
    dag.update_task_status(id_a, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_a, TaskStatus::Running).unwrap();
    dag.update_task_status(id_a, TaskStatus::Completed).unwrap();

    // Trying to record more should fail
    let result = contract.record_tokens(1);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), ErrorCode::TokenLimitExceeded);
    assert_eq!(contract.status, ContractStatus::Exceeded);

    // Task B cannot execute - contract exceeded
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1); // B is ready but contract is exceeded
}

#[test]
fn test_child_contracts_for_subtasks() {
    // Parent contract for main DAG
    let parent_limits = create_test_limits(20000, 2.0, 200);
    let mut parent = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), parent_limits);

    // Use some parent budget
    parent.record_tokens(5000).unwrap();
    parent.record_cost(0.5).unwrap();

    // Create child contract for a subtask with subset of remaining budget
    let child_limits = create_test_limits(10000, 1.0, 100);
    let child = parent
        .create_child(Uuid::new_v4(), Uuid::new_v4(), child_limits)
        .unwrap();

    assert_eq!(child.status, ContractStatus::Active);
    assert_eq!(child.limits.token_limit, 10000);

    // Verify remaining budget on parent allows the child
    let remaining = parent.remaining();
    assert_eq!(remaining.tokens_used, 15000); // 20000 - 5000
    assert!((remaining.cost_used - 1.5).abs() < 0.001);
}

#[test]
fn test_parallel_dag_execution() {
    let mut dag = TaskDAG::new("parallel-test");

    //     A
    //    / \
    //   B   C
    //    \ /
    //     D

    let task_a = create_test_task("Task A");
    let task_b = create_test_task("Task B");
    let task_c = create_test_task("Task C");
    let task_d = create_test_task("Task D");

    let id_a = dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();
    let id_d = dag.add_task(task_d).unwrap();

    dag.add_dependency(id_a, id_b).unwrap();
    dag.add_dependency(id_a, id_c).unwrap();
    dag.add_dependency(id_b, id_d).unwrap();
    dag.add_dependency(id_c, id_d).unwrap();

    // Initially only A is ready
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_a);

    // Complete A
    dag.update_task_status(id_a, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_a, TaskStatus::Running).unwrap();
    dag.update_task_status(id_a, TaskStatus::Completed).unwrap();

    // Now B and C are ready (parallel execution)
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 2);
    assert!(ready.contains(&id_b));
    assert!(ready.contains(&id_c));

    // Complete B only
    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Completed).unwrap();

    // D is not ready yet (waiting for C)
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_c);

    // Complete C
    dag.update_task_status(id_c, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_c, TaskStatus::Running).unwrap();
    dag.update_task_status(id_c, TaskStatus::Completed).unwrap();

    // Now D is ready
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_d);

    // Complete D
    dag.update_task_status(id_d, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_d, TaskStatus::Running).unwrap();
    dag.update_task_status(id_d, TaskStatus::Completed).unwrap();

    // DAG is complete
    assert!(dag.is_complete());
}

// ============================================================================
// Resource Limit Preset Tests
// ============================================================================

#[test]
fn test_resource_limit_presets() {
    let simple = ResourceLimits::simple();
    let medium = ResourceLimits::medium();
    let complex = ResourceLimits::complex();

    // Simple fits within medium
    assert!(simple.fits_within(&medium));
    assert!(simple.fits_within(&complex));

    // Medium fits within complex
    assert!(medium.fits_within(&complex));

    // Complex doesn't fit within simpler presets
    assert!(!complex.fits_within(&medium));
    assert!(!complex.fits_within(&simple));
    assert!(!medium.fits_within(&simple));
}

#[test]
fn test_overhead_and_allocatable() {
    let limits = create_test_limits(10000, 1.0, 100);

    let overhead = limits.overhead();
    let allocatable = limits.allocatable();

    // Overhead + allocatable should equal original (conservation law)
    assert_eq!(
        overhead.token_limit + allocatable.token_limit,
        limits.token_limit
    );
    assert!(
        (overhead.cost_limit + allocatable.cost_limit - limits.cost_limit).abs() < 0.001
    );
    assert_eq!(
        overhead.api_call_limit + allocatable.api_call_limit,
        limits.api_call_limit
    );
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_cycle_detection_in_dag() {
    let mut dag = TaskDAG::new("cycle-test");

    let task_a = create_test_task("Task A");
    let task_b = create_test_task("Task B");
    let task_c = create_test_task("Task C");

    let id_a = dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();

    // A -> B -> C (valid)
    dag.add_dependency(id_a, id_b).unwrap();
    dag.add_dependency(id_b, id_c).unwrap();

    // C -> A would create a cycle (should fail)
    let result = dag.add_dependency(id_c, id_a);
    assert!(result.is_err());

    // DAG should still be valid after failed cycle addition
    let order = dag.topological_order().unwrap();
    assert_eq!(order.len(), 3);
}

#[test]
fn test_contract_conservation_violation() {
    let parent_limits = create_test_limits(10000, 1.0, 100);
    let mut parent = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), parent_limits);

    // Use most of the parent's budget
    parent.record_tokens(8000).unwrap();
    parent.record_cost(0.9).unwrap();

    // Try to create a child with more than remaining budget
    let excessive_limits = create_test_limits(5000, 0.5, 50); // Only 2000 tokens remaining!
    let result = parent.create_child(Uuid::new_v4(), Uuid::new_v4(), excessive_limits);

    assert!(result.is_err());
}

// ============================================================================
// Statistics and Metrics Tests
// ============================================================================

#[test]
fn test_dag_statistics() {
    let mut dag = TaskDAG::new("stats-test");

    let task_a = create_test_task("Task A");
    let task_b = create_test_task("Task B");
    let task_c = create_test_task("Task C");
    let task_d = create_test_task("Task D");

    dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();
    let id_d = dag.add_task(task_d).unwrap();

    // Initial stats
    let stats = dag.stats();
    assert_eq!(stats.total, 4);
    assert_eq!(stats.pending, 4);
    assert_eq!(stats.completed, 0);
    assert_eq!(stats.failed, 0);

    // Complete one, fail one, cancel one
    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Completed).unwrap();
    dag.update_task_status(id_c, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_c, TaskStatus::Running).unwrap();
    dag.update_task_status(id_c, TaskStatus::Failed).unwrap();
    dag.update_task_status(id_d, TaskStatus::Cancelled).unwrap();

    let stats = dag.stats();
    assert_eq!(stats.pending, 1);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.cancelled, 1);
}

#[test]
fn test_contract_utilization() {
    let limits = create_test_limits(10000, 1.0, 100);
    let mut contract = AgentContract::new(Uuid::new_v4(), Uuid::new_v4(), limits);

    // Use 50% of tokens, 25% of cost
    contract.record_tokens(5000).unwrap();
    contract.record_cost(0.25).unwrap();
    for _ in 0..10 {
        contract.record_api_call().unwrap();
    }

    let util = contract.utilization();

    // Check utilization percentages
    assert!((util.tokens - 50.0).abs() < 0.1);
    assert!((util.cost - 25.0).abs() < 0.1);
    assert!((util.api_calls - 10.0).abs() < 0.1);
}
