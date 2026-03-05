//! Comprehensive unit tests for Circuit Breaker state transitions.
//!
//! Tests cover:
//! - State transitions (Closed -> Open -> HalfOpen -> Closed)
//! - Failure threshold triggering
//! - Recovery timeout behavior
//! - Success/failure recording
//! - Metrics tracking
//! - Concurrent access scenarios
//! - Edge cases and boundary conditions

use apex_core::orchestrator::{
    circuit_breaker::CircuitState,
    CircuitBreaker,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Circuit Breaker Creation Tests
// ============================================================================

#[test]
fn test_circuit_breaker_creation_default_state() {
    let breaker = CircuitBreaker::new(5);

    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
}

#[test]
fn test_circuit_breaker_various_thresholds() {
    for threshold in [1, 3, 5, 10, 100] {
        let breaker = CircuitBreaker::new(threshold);
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.can_execute());
    }
}

#[test]
fn test_circuit_breaker_with_custom_recovery_timeout() {
    let breaker = CircuitBreaker::new(3).with_recovery_timeout(Duration::from_secs(60));

    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[test]
fn test_circuit_breaker_with_zero_timeout() {
    let breaker = CircuitBreaker::new(3).with_recovery_timeout(Duration::ZERO);

    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[test]
fn test_circuit_breaker_with_very_short_timeout() {
    let breaker = CircuitBreaker::new(3).with_recovery_timeout(Duration::from_nanos(1));

    assert_eq!(breaker.state(), CircuitState::Closed);
}

// ============================================================================
// State Transition: Closed -> Open Tests
// ============================================================================

#[test]
fn test_transition_closed_to_open_on_failures() {
    let breaker = CircuitBreaker::new(3);

    // First two failures - should stay closed
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    // Third failure - should trip to open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());
}

#[test]
fn test_threshold_of_one() {
    let breaker = CircuitBreaker::new(1);

    // Single failure should trip immediately
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());
}

#[test]
fn test_high_threshold() {
    let breaker = CircuitBreaker::new(100);

    // 99 failures should not trip
    for _ in 0..99 {
        breaker.record_failure();
    }
    assert_eq!(breaker.state(), CircuitState::Closed);

    // 100th failure trips
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
}

#[test]
fn test_failures_reset_on_success() {
    let breaker = CircuitBreaker::new(3);

    // Two failures
    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    // Success resets count
    breaker.record_success();

    // Now need 3 more failures to trip
    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    // Only third failure after reset trips
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
}

#[test]
fn test_alternating_success_failure() {
    let breaker = CircuitBreaker::new(3);

    // Alternating should never trip (success resets count)
    for _ in 0..100 {
        breaker.record_failure();
        breaker.record_success();
    }

    assert_eq!(breaker.state(), CircuitState::Closed);
}

// ============================================================================
// State Transition: Open -> HalfOpen Tests
// ============================================================================

#[test]
fn test_transition_open_to_half_open() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(10));

    // Trip to open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());

    // Wait for recovery timeout
    thread::sleep(Duration::from_millis(20));

    // Should transition to half-open on can_execute check
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);
}

#[test]
fn test_stays_open_before_timeout() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_secs(10)); // Long timeout

    // Trip to open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Should stay open (timeout not elapsed)
    assert!(!breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::Open);
}

#[test]
fn test_open_blocks_all_executions() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_secs(60));

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Multiple checks should all return false
    for _ in 0..100 {
        assert!(!breaker.can_execute());
    }
}

// ============================================================================
// State Transition: HalfOpen -> Closed Tests
// ============================================================================

#[test]
fn test_transition_half_open_to_closed_on_success() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(10));

    // Trip to open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Wait and transition to half-open
    thread::sleep(Duration::from_millis(20));
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Success closes the breaker
    breaker.record_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
}

#[test]
fn test_half_open_allows_execution() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(10));

    breaker.record_failure();
    thread::sleep(Duration::from_millis(20));

    // Should allow execution in half-open state
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);
    assert!(breaker.can_execute()); // Can execute multiple times
}

// ============================================================================
// State Transition: HalfOpen -> Open Tests
// ============================================================================

#[test]
fn test_transition_half_open_to_open_on_failure() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(10));

    // Trip to open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Wait and transition to half-open
    thread::sleep(Duration::from_millis(20));
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Failure in half-open reopens
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());
}

#[test]
fn test_multiple_recovery_attempts() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(10));

    for _ in 0..5 {
        // Trip to open
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait and transition to half-open
        thread::sleep(Duration::from_millis(20));
        assert!(breaker.can_execute());
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        // Fail again in half-open
        breaker.record_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
    }

    // Eventually succeed
    thread::sleep(Duration::from_millis(20));
    assert!(breaker.can_execute());
    breaker.record_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
}

// ============================================================================
// Manual Reset Tests
// ============================================================================

#[test]
fn test_reset_from_open_state() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_secs(60));

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    breaker.reset();
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
}

#[test]
fn test_reset_from_half_open_state() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(10));

    breaker.record_failure();
    thread::sleep(Duration::from_millis(20));
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    breaker.reset();
    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[test]
fn test_reset_from_closed_state() {
    let breaker = CircuitBreaker::new(3);

    // Add some failures without tripping
    breaker.record_failure();
    breaker.record_failure();

    breaker.reset();
    assert_eq!(breaker.state(), CircuitState::Closed);

    // Should need full threshold to trip again
    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
}

#[test]
fn test_reset_clears_failure_count() {
    let breaker = CircuitBreaker::new(3);

    breaker.record_failure();
    breaker.record_failure();
    breaker.reset();

    // Need 3 new failures to trip
    breaker.record_failure();
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
}

// ============================================================================
// Metrics Tests
// ============================================================================

#[test]
fn test_metrics_initial_state() {
    let breaker = CircuitBreaker::new(5);
    let metrics = breaker.metrics();

    assert_eq!(metrics.state, CircuitState::Closed);
    assert_eq!(metrics.failure_count, 0);
    assert_eq!(metrics.failure_threshold, 5);
    assert_eq!(metrics.total_successes, 0);
    assert_eq!(metrics.total_failures, 0);
}

#[test]
fn test_metrics_after_successes() {
    let breaker = CircuitBreaker::new(5);

    for _ in 0..10 {
        breaker.record_success();
    }

    let metrics = breaker.metrics();
    assert_eq!(metrics.total_successes, 10);
    assert_eq!(metrics.total_failures, 0);
    assert_eq!(metrics.failure_count, 0);
}

#[test]
fn test_metrics_after_failures() {
    let breaker = CircuitBreaker::new(5);

    for _ in 0..3 {
        breaker.record_failure();
    }

    let metrics = breaker.metrics();
    assert_eq!(metrics.total_failures, 3);
    assert_eq!(metrics.failure_count, 3);
    assert_eq!(metrics.state, CircuitState::Closed);
}

#[test]
fn test_metrics_after_trip() {
    let breaker = CircuitBreaker::new(3);

    for _ in 0..3 {
        breaker.record_failure();
    }

    let metrics = breaker.metrics();
    assert_eq!(metrics.state, CircuitState::Open);
    assert_eq!(metrics.total_failures, 3);
    assert_eq!(metrics.failure_count, 3);
}

#[test]
fn test_metrics_mixed_operations() {
    let breaker = CircuitBreaker::new(5);

    breaker.record_success();
    breaker.record_success();
    breaker.record_failure();
    breaker.record_success();
    breaker.record_failure();

    let metrics = breaker.metrics();
    assert_eq!(metrics.total_successes, 3);
    assert_eq!(metrics.total_failures, 2);
}

#[test]
fn test_metrics_failure_count_resets_on_success() {
    let breaker = CircuitBreaker::new(5);

    breaker.record_failure();
    breaker.record_failure();
    breaker.record_success();

    let metrics = breaker.metrics();
    assert_eq!(metrics.failure_count, 0); // Reset by success
    assert_eq!(metrics.total_failures, 2); // Total still tracked
}

// ============================================================================
// CircuitState Tests
// ============================================================================

#[test]
fn test_circuit_state_equality() {
    assert_eq!(CircuitState::Closed, CircuitState::Closed);
    assert_eq!(CircuitState::Open, CircuitState::Open);
    assert_eq!(CircuitState::HalfOpen, CircuitState::HalfOpen);

    assert_ne!(CircuitState::Closed, CircuitState::Open);
    assert_ne!(CircuitState::Open, CircuitState::HalfOpen);
    assert_ne!(CircuitState::HalfOpen, CircuitState::Closed);
}

#[test]
fn test_circuit_state_clone() {
    let state = CircuitState::HalfOpen;
    let cloned = state;
    assert_eq!(state, cloned);
}

#[test]
fn test_circuit_state_copy() {
    let state = CircuitState::Open;
    let copied = state;
    assert_eq!(state, copied);
}

#[test]
fn test_circuit_state_debug() {
    let closed = format!("{:?}", CircuitState::Closed);
    let open = format!("{:?}", CircuitState::Open);
    let half_open = format!("{:?}", CircuitState::HalfOpen);

    assert!(closed.contains("Closed"));
    assert!(open.contains("Open"));
    assert!(half_open.contains("HalfOpen"));
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

#[test]
fn test_concurrent_success_recording() {
    let breaker = Arc::new(CircuitBreaker::new(10));
    let mut handles = vec![];

    for _ in 0..100 {
        let breaker_clone = breaker.clone();
        let handle = thread::spawn(move || {
            breaker_clone.record_success();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let metrics = breaker.metrics();
    assert_eq!(metrics.total_successes, 100);
    assert_eq!(metrics.state, CircuitState::Closed);
}

#[test]
fn test_concurrent_failure_recording() {
    let breaker = Arc::new(CircuitBreaker::new(50));
    let mut handles = vec![];

    for _ in 0..100 {
        let breaker_clone = breaker.clone();
        let handle = thread::spawn(move || {
            breaker_clone.record_failure();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let metrics = breaker.metrics();
    assert_eq!(metrics.total_failures, 100);
    assert_eq!(metrics.state, CircuitState::Open); // Should be tripped
}

#[test]
fn test_concurrent_mixed_operations() {
    let breaker = Arc::new(CircuitBreaker::new(100));
    let mut handles = vec![];

    // Half success, half failure
    for i in 0..100 {
        let breaker_clone = breaker.clone();
        let handle = thread::spawn(move || {
            if i % 2 == 0 {
                breaker_clone.record_success();
            } else {
                breaker_clone.record_failure();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let metrics = breaker.metrics();
    // Due to success resets, should likely stay closed
    // Total counts should be accurate
    assert!(metrics.total_successes + metrics.total_failures == 100);
}

#[test]
fn test_concurrent_can_execute_checks() {
    let breaker = Arc::new(CircuitBreaker::new(10));
    let mut handles = vec![];

    for _ in 0..100 {
        let breaker_clone = breaker.clone();
        let handle = thread::spawn(move || {
            let _ = breaker_clone.can_execute();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should still be in valid state
    let metrics = breaker.metrics();
    assert!(matches!(
        metrics.state,
        CircuitState::Closed | CircuitState::Open | CircuitState::HalfOpen
    ));
}

#[test]
fn test_concurrent_reset() {
    let breaker = Arc::new(CircuitBreaker::new(5));

    // Trip the breaker first
    for _ in 0..5 {
        breaker.record_failure();
    }
    assert_eq!(breaker.state(), CircuitState::Open);

    let mut handles = vec![];

    // Concurrent resets
    for _ in 0..10 {
        let breaker_clone = breaker.clone();
        let handle = thread::spawn(move || {
            breaker_clone.reset();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should be closed after resets
    assert_eq!(breaker.state(), CircuitState::Closed);
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[test]
fn test_recording_in_open_state() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_secs(60));

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Recording success/failure in open state
    breaker.record_success();
    assert_eq!(breaker.state(), CircuitState::Open); // Should stay open

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open); // Should stay open
}

#[test]
fn test_very_rapid_state_transitions() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_nanos(1));

    for _ in 0..100 {
        breaker.record_failure(); // Trip to open
                                  // With 1ns timeout, should immediately allow half-open
        if breaker.can_execute() {
            breaker.record_success(); // Close it
        }
    }

    // Should end up in a valid state
    let state = breaker.state();
    assert!(matches!(
        state,
        CircuitState::Closed | CircuitState::Open | CircuitState::HalfOpen
    ));
}

#[test]
fn test_zero_threshold_breaker() {
    // This is an edge case - zero threshold doesn't make practical sense
    // but the code should handle it gracefully
    let breaker = CircuitBreaker::new(0);

    // With threshold 0, first failure should trip
    breaker.record_failure();
    // Behavior depends on implementation - >= vs >
    // Current impl uses >= so this should trip
    assert_eq!(breaker.state(), CircuitState::Open);
}

#[test]
fn test_max_u32_threshold() {
    let breaker = CircuitBreaker::new(u32::MAX);

    // Should never trip with normal usage
    for _ in 0..1000 {
        breaker.record_failure();
    }

    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[test]
fn test_metrics_struct_clone() {
    let breaker = CircuitBreaker::new(5);
    breaker.record_success();
    breaker.record_failure();

    let metrics = breaker.metrics();
    let cloned = metrics.clone();

    assert_eq!(metrics.state, cloned.state);
    assert_eq!(metrics.total_successes, cloned.total_successes);
    assert_eq!(metrics.total_failures, cloned.total_failures);
}

#[test]
fn test_metrics_struct_debug() {
    let breaker = CircuitBreaker::new(5);
    let metrics = breaker.metrics();
    let debug_str = format!("{:?}", metrics);

    assert!(debug_str.contains("CircuitBreakerMetrics"));
}

// ============================================================================
// Recovery Timeout Behavior Tests
// ============================================================================

#[test]
fn test_recovery_timing_exact() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(50));

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Should be open before timeout
    thread::sleep(Duration::from_millis(25));
    assert!(!breaker.can_execute());

    // Wait for full timeout
    thread::sleep(Duration::from_millis(30));
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);
}

#[test]
fn test_recovery_timeout_multiple_checks() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(200));

    breaker.record_failure();

    // Multiple checks before timeout should all return false
    for _ in 0..10 {
        assert!(!breaker.can_execute());
        thread::sleep(Duration::from_millis(3));
    }

    // After timeout, should transition
    thread::sleep(Duration::from_millis(200));
    assert!(breaker.can_execute());
}

#[test]
fn test_long_recovery_timeout() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_secs(3600)); // 1 hour

    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Should definitely not transition in a short time
    thread::sleep(Duration::from_millis(10));
    assert!(!breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::Open);
}

// ============================================================================
// Full State Machine Cycle Tests
// ============================================================================

#[test]
fn test_complete_state_machine_cycle() {
    let breaker = CircuitBreaker::new(2).with_recovery_timeout(Duration::from_millis(10));

    // Start: Closed
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());

    // Success keeps it closed
    breaker.record_success();
    assert_eq!(breaker.state(), CircuitState::Closed);

    // First failure
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    // Second failure trips to Open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());

    // Wait for recovery
    thread::sleep(Duration::from_millis(20));

    // Transitions to HalfOpen
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Success closes it
    breaker.record_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
}

#[test]
fn test_state_machine_with_failed_recovery() {
    let breaker = CircuitBreaker::new(1).with_recovery_timeout(Duration::from_millis(10));

    // Closed -> Open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Wait for recovery -> HalfOpen
    thread::sleep(Duration::from_millis(20));
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Failure reopens -> Open
    breaker.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());

    // Wait for recovery again -> HalfOpen
    thread::sleep(Duration::from_millis(20));
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // This time succeed -> Closed
    breaker.record_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
}
