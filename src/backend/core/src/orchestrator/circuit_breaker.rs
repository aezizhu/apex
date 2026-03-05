//! Circuit Breaker for failure handling.
//!
//! Prevents cascade failures by temporarily stopping requests
//! when too many consecutive failures occur.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests allowed
    Closed,
    /// Too many failures - requests blocked
    Open,
    /// Testing if service recovered - limited requests allowed
    HalfOpen,
}

/// Circuit breaker for failure detection and recovery.
pub struct CircuitBreaker {
    /// Current state
    state: RwLock<CircuitState>,

    /// Consecutive failure count
    failure_count: AtomicU32,

    /// Failure threshold to trip the breaker
    failure_threshold: u32,

    /// Time the breaker was opened
    opened_at: RwLock<Option<Instant>>,

    /// Recovery timeout (how long to wait before trying again)
    recovery_timeout: Duration,

    /// Total successes (for metrics)
    total_successes: AtomicU64,

    /// Total failures (for metrics)
    total_failures: AtomicU64,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(failure_threshold: u32) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            failure_threshold,
            opened_at: RwLock::new(None),
            recovery_timeout: Duration::from_secs(30),
            total_successes: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
        }
    }

    /// Create with custom recovery timeout.
    pub fn with_recovery_timeout(mut self, timeout: Duration) -> Self {
        self.recovery_timeout = timeout;
        self
    }

    /// Check if execution is allowed.
    pub fn can_execute(&self) -> bool {
        let state = *self.state.read();

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if recovery timeout has elapsed
                if let Some(opened_at) = *self.opened_at.read() {
                    if opened_at.elapsed() >= self.recovery_timeout {
                        // Transition to half-open
                        *self.state.write() = CircuitState::HalfOpen;
                        tracing::info!("Circuit breaker transitioning to half-open");
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true, // Allow limited requests
        }
    }

    /// Record a successful execution.
    pub fn record_success(&self) {
        self.total_successes.fetch_add(1, Ordering::Relaxed);

        let state = *self.state.read();

        match state {
            CircuitState::HalfOpen => {
                // Success in half-open state - close the breaker
                self.failure_count.store(0, Ordering::Relaxed);
                *self.state.write() = CircuitState::Closed;
                *self.opened_at.write() = None;
                tracing::info!("Circuit breaker closed after successful recovery");
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // Shouldn't happen, but reset anyway
            }
        }
    }

    /// Record a failed execution.
    pub fn record_failure(&self) {
        self.total_failures.fetch_add(1, Ordering::Relaxed);

        let state = *self.state.read();

        match state {
            CircuitState::HalfOpen => {
                // Failure in half-open state - re-open the breaker
                *self.state.write() = CircuitState::Open;
                *self.opened_at.write() = Some(Instant::now());
                tracing::warn!("Circuit breaker re-opened after failed recovery attempt");
            }
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

                if failures >= self.failure_threshold {
                    // Trip the breaker
                    *self.state.write() = CircuitState::Open;
                    *self.opened_at.write() = Some(Instant::now());
                    tracing::warn!(
                        failures = failures,
                        threshold = self.failure_threshold,
                        "Circuit breaker opened due to consecutive failures"
                    );
                }
            }
            CircuitState::Open => {
                // Already open, nothing to do
            }
        }
    }

    /// Get current state.
    pub fn state(&self) -> CircuitState {
        *self.state.read()
    }

    /// Get metrics.
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            state: self.state(),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            failure_threshold: self.failure_threshold,
            total_successes: self.total_successes.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
        }
    }

    /// Force reset the circuit breaker.
    pub fn reset(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        *self.state.write() = CircuitState::Closed;
        *self.opened_at.write() = None;
        tracing::info!("Circuit breaker manually reset");
    }
}

/// Metrics for the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub state: CircuitState,
    pub failure_count: u32,
    pub failure_threshold: u32,
    pub total_successes: u64,
    pub total_failures: u64,
}

/// Reason why a per-agent circuit was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentCircuitOpenReason {
    /// Too many consecutive failures.
    ConsecutiveFailures,
    /// Agent was detected to be in a loop by the Python loop detector.
    LoopDetected,
}

/// Per-agent circuit breaker state for tracking individual agent health.
#[derive(Debug)]
struct AgentCircuitState {
    /// Current circuit state for this agent.
    state: CircuitState,
    /// Consecutive failure count.
    failure_count: u32,
    /// When the circuit was opened.
    opened_at: Option<Instant>,
    /// Current backoff multiplier (for exponential backoff).
    backoff_multiplier: u32,
    /// Total successes.
    total_successes: u64,
    /// Total failures.
    total_failures: u64,
    /// Reason the circuit was opened (if open).
    open_reason: Option<AgentCircuitOpenReason>,
}

impl AgentCircuitState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            opened_at: None,
            backoff_multiplier: 1,
            total_successes: 0,
            total_failures: 0,
            open_reason: None,
        }
    }
}

/// Per-agent circuit breaker metrics.
#[derive(Debug, Clone)]
pub struct AgentCircuitMetrics {
    pub agent_id: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub backoff_multiplier: u32,
    pub total_successes: u64,
    pub total_failures: u64,
    pub open_reason: Option<AgentCircuitOpenReason>,
}

/// Enhanced circuit breaker registry that tracks per-agent failure patterns.
///
/// Wraps the global `CircuitBreaker` and adds per-agent tracking with
/// exponential backoff and loop-detection-aware circuit opening.
pub struct AgentCircuitBreakerRegistry {
    /// Global circuit breaker (for system-wide failures).
    global: CircuitBreaker,

    /// Per-agent circuit states.
    agents: RwLock<HashMap<String, AgentCircuitState>>,

    /// Per-agent failure threshold.
    agent_failure_threshold: u32,

    /// Base recovery timeout for agents (multiplied by backoff_multiplier).
    base_recovery_timeout: Duration,

    /// Maximum backoff multiplier to cap exponential growth.
    max_backoff_multiplier: u32,
}

impl AgentCircuitBreakerRegistry {
    /// Create a new registry with the given global failure threshold.
    pub fn new(global_failure_threshold: u32, agent_failure_threshold: u32) -> Self {
        Self {
            global: CircuitBreaker::new(global_failure_threshold),
            agents: RwLock::new(HashMap::new()),
            agent_failure_threshold,
            base_recovery_timeout: Duration::from_secs(30),
            max_backoff_multiplier: 16,
        }
    }

    /// Set the base recovery timeout for agent-level circuits.
    pub fn with_base_recovery_timeout(mut self, timeout: Duration) -> Self {
        self.base_recovery_timeout = timeout;
        self.global = self.global.with_recovery_timeout(timeout);
        self
    }

    /// Check if a specific agent is allowed to execute.
    ///
    /// Returns `true` if both the global breaker and the agent-specific
    /// breaker allow execution.
    pub fn can_execute(&self, agent_id: &str) -> bool {
        // Global check first
        if !self.global.can_execute() {
            return false;
        }

        // Per-agent check
        let mut agents = self.agents.write();
        let agent_state = agents
            .entry(agent_id.to_string())
            .or_insert_with(AgentCircuitState::new);

        match agent_state.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(opened_at) = agent_state.opened_at {
                    let effective_timeout = self.base_recovery_timeout
                        * agent_state.backoff_multiplier;
                    if opened_at.elapsed() >= effective_timeout {
                        agent_state.state = CircuitState::HalfOpen;
                        tracing::info!(
                            agent_id = agent_id,
                            backoff_multiplier = agent_state.backoff_multiplier,
                            "Agent circuit breaker transitioning to half-open"
                        );
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful execution for an agent.
    pub fn record_success(&self, agent_id: &str) {
        self.global.record_success();

        let mut agents = self.agents.write();
        let agent_state = agents
            .entry(agent_id.to_string())
            .or_insert_with(AgentCircuitState::new);

        agent_state.total_successes += 1;

        match agent_state.state {
            CircuitState::HalfOpen => {
                // Recovery succeeded - close circuit and reset backoff
                agent_state.failure_count = 0;
                agent_state.state = CircuitState::Closed;
                agent_state.opened_at = None;
                agent_state.backoff_multiplier = 1;
                agent_state.open_reason = None;
                tracing::info!(
                    agent_id = agent_id,
                    "Agent circuit breaker closed after successful recovery"
                );
            }
            CircuitState::Closed => {
                agent_state.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed execution for an agent.
    pub fn record_failure(&self, agent_id: &str) {
        self.global.record_failure();

        let mut agents = self.agents.write();
        let agent_state = agents
            .entry(agent_id.to_string())
            .or_insert_with(AgentCircuitState::new);

        agent_state.total_failures += 1;

        match agent_state.state {
            CircuitState::HalfOpen => {
                // Failed recovery - re-open with increased backoff
                agent_state.state = CircuitState::Open;
                agent_state.opened_at = Some(Instant::now());
                agent_state.backoff_multiplier = std::cmp::min(
                    agent_state.backoff_multiplier * 2,
                    self.max_backoff_multiplier,
                );
                agent_state.open_reason = Some(AgentCircuitOpenReason::ConsecutiveFailures);
                tracing::warn!(
                    agent_id = agent_id,
                    backoff_multiplier = agent_state.backoff_multiplier,
                    "Agent circuit breaker re-opened with increased backoff"
                );
            }
            CircuitState::Closed => {
                agent_state.failure_count += 1;
                if agent_state.failure_count >= self.agent_failure_threshold {
                    agent_state.state = CircuitState::Open;
                    agent_state.opened_at = Some(Instant::now());
                    agent_state.open_reason = Some(AgentCircuitOpenReason::ConsecutiveFailures);
                    tracing::warn!(
                        agent_id = agent_id,
                        failures = agent_state.failure_count,
                        threshold = self.agent_failure_threshold,
                        "Agent circuit breaker opened due to consecutive failures"
                    );
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Open the circuit for a specific agent due to loop detection.
    ///
    /// Called when the Python-side loop detector flags an agent as stuck.
    /// Uses a longer initial backoff since loop detection indicates a
    /// systemic issue, not a transient failure.
    pub fn open_for_loop_detection(&self, agent_id: &str) {
        let mut agents = self.agents.write();
        let agent_state = agents
            .entry(agent_id.to_string())
            .or_insert_with(AgentCircuitState::new);

        agent_state.state = CircuitState::Open;
        agent_state.opened_at = Some(Instant::now());
        agent_state.open_reason = Some(AgentCircuitOpenReason::LoopDetected);
        // Loop detection gets a higher initial backoff multiplier
        agent_state.backoff_multiplier = std::cmp::max(agent_state.backoff_multiplier, 4);

        tracing::warn!(
            agent_id = agent_id,
            backoff_multiplier = agent_state.backoff_multiplier,
            "Agent circuit breaker opened due to loop detection"
        );
    }

    /// Get metrics for a specific agent.
    pub fn agent_metrics(&self, agent_id: &str) -> Option<AgentCircuitMetrics> {
        let agents = self.agents.read();
        agents.get(agent_id).map(|state| AgentCircuitMetrics {
            agent_id: agent_id.to_string(),
            state: state.state,
            failure_count: state.failure_count,
            backoff_multiplier: state.backoff_multiplier,
            total_successes: state.total_successes,
            total_failures: state.total_failures,
            open_reason: state.open_reason.clone(),
        })
    }

    /// Get global circuit breaker metrics.
    pub fn global_metrics(&self) -> CircuitBreakerMetrics {
        self.global.metrics()
    }

    /// Get the effective recovery timeout for a specific agent.
    pub fn effective_timeout(&self, agent_id: &str) -> Duration {
        let agents = self.agents.read();
        let multiplier = agents
            .get(agent_id)
            .map(|s| s.backoff_multiplier)
            .unwrap_or(1);
        self.base_recovery_timeout * multiplier
    }

    /// Force reset a specific agent's circuit.
    pub fn reset_agent(&self, agent_id: &str) {
        let mut agents = self.agents.write();
        if let Some(state) = agents.get_mut(agent_id) {
            state.state = CircuitState::Closed;
            state.failure_count = 0;
            state.opened_at = None;
            state.backoff_multiplier = 1;
            state.open_reason = None;
            tracing::info!(agent_id = agent_id, "Agent circuit breaker manually reset");
        }
    }

    /// Force reset the global circuit breaker and all agent circuits.
    pub fn reset_all(&self) {
        self.global.reset();
        let mut agents = self.agents.write();
        agents.clear();
        tracing::info!("All circuit breakers reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_trips_on_failures() {
        let breaker = CircuitBreaker::new(3);

        assert!(breaker.can_execute());
        assert_eq!(breaker.state(), CircuitState::Closed);

        // Record failures
        breaker.record_failure();
        breaker.record_failure();
        assert!(breaker.can_execute()); // Still closed

        breaker.record_failure(); // Third failure - should trip
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(!breaker.can_execute());
    }

    #[test]
    fn test_circuit_breaker_resets_on_success() {
        let breaker = CircuitBreaker::new(3);

        breaker.record_failure();
        breaker.record_failure();
        breaker.record_success(); // Should reset count

        breaker.record_failure();
        breaker.record_failure();
        // Still closed because count was reset
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_recovery() {
        let breaker = CircuitBreaker::new(1)
            .with_recovery_timeout(Duration::from_millis(10));

        breaker.record_failure(); // Trip to open
        assert_eq!(breaker.state(), CircuitState::Open);

        // Wait for recovery timeout
        std::thread::sleep(Duration::from_millis(20));

        // Should transition to half-open
        assert!(breaker.can_execute());
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        // Success should close
        breaker.record_success();
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    // --- AgentCircuitBreakerRegistry tests ---

    #[test]
    fn test_agent_registry_per_agent_isolation() {
        let registry = AgentCircuitBreakerRegistry::new(10, 2);

        // Fail agent_a
        registry.record_failure("agent_a");
        registry.record_failure("agent_a");

        // agent_a should be open, agent_b should still be closed
        assert!(!registry.can_execute("agent_a"));
        assert!(registry.can_execute("agent_b"));

        let a_metrics = registry.agent_metrics("agent_a").unwrap();
        assert_eq!(a_metrics.state, CircuitState::Open);
        assert_eq!(a_metrics.failure_count, 2);
    }

    #[test]
    fn test_agent_registry_exponential_backoff() {
        let registry = AgentCircuitBreakerRegistry::new(10, 1)
            .with_base_recovery_timeout(Duration::from_millis(10));

        // First failure - opens circuit, backoff = 1x
        registry.record_failure("agent_x");
        assert!(!registry.can_execute("agent_x"));

        let metrics = registry.agent_metrics("agent_x").unwrap();
        assert_eq!(metrics.backoff_multiplier, 1);

        // Wait for recovery, then fail again in half-open
        std::thread::sleep(Duration::from_millis(15));
        assert!(registry.can_execute("agent_x")); // transitions to half-open

        registry.record_failure("agent_x"); // re-opens with 2x backoff

        let metrics = registry.agent_metrics("agent_x").unwrap();
        assert_eq!(metrics.state, CircuitState::Open);
        assert_eq!(metrics.backoff_multiplier, 2);
    }

    #[test]
    fn test_agent_registry_backoff_reset_on_success() {
        let registry = AgentCircuitBreakerRegistry::new(10, 1)
            .with_base_recovery_timeout(Duration::from_millis(10));

        // Fail, wait, fail again (to increase backoff)
        registry.record_failure("agent_y");
        std::thread::sleep(Duration::from_millis(15));
        assert!(registry.can_execute("agent_y")); // half-open
        registry.record_failure("agent_y"); // backoff = 2x

        // Wait longer (2x base timeout), then succeed
        std::thread::sleep(Duration::from_millis(25));
        assert!(registry.can_execute("agent_y")); // half-open
        registry.record_success("agent_y"); // should close and reset backoff

        let metrics = registry.agent_metrics("agent_y").unwrap();
        assert_eq!(metrics.state, CircuitState::Closed);
        assert_eq!(metrics.backoff_multiplier, 1);
    }

    #[test]
    fn test_agent_registry_loop_detection_opens_circuit() {
        let registry = AgentCircuitBreakerRegistry::new(10, 5);

        // Agent is healthy
        assert!(registry.can_execute("looping_agent"));

        // Loop detected - opens circuit with high backoff
        registry.open_for_loop_detection("looping_agent");

        assert!(!registry.can_execute("looping_agent"));

        let metrics = registry.agent_metrics("looping_agent").unwrap();
        assert_eq!(metrics.state, CircuitState::Open);
        assert_eq!(metrics.open_reason, Some(AgentCircuitOpenReason::LoopDetected));
        assert!(metrics.backoff_multiplier >= 4);
    }

    #[test]
    fn test_agent_registry_global_overrides_agent() {
        let registry = AgentCircuitBreakerRegistry::new(2, 10);

        // Global failures trip global breaker
        registry.record_failure("agent_a");
        registry.record_failure("agent_b");

        // Global breaker is now open, so even a healthy agent_c is blocked
        assert!(!registry.can_execute("agent_c"));
    }

    #[test]
    fn test_agent_registry_reset_agent() {
        let registry = AgentCircuitBreakerRegistry::new(10, 1);

        registry.record_failure("agent_r");
        assert!(!registry.can_execute("agent_r"));

        registry.reset_agent("agent_r");
        assert!(registry.can_execute("agent_r"));
    }

    #[test]
    fn test_agent_registry_effective_timeout() {
        let registry = AgentCircuitBreakerRegistry::new(10, 1)
            .with_base_recovery_timeout(Duration::from_secs(5));

        // New agent gets base timeout
        assert_eq!(registry.effective_timeout("new_agent"), Duration::from_secs(5));

        // After failure, still 1x
        registry.record_failure("new_agent");
        assert_eq!(registry.effective_timeout("new_agent"), Duration::from_secs(5));

        // Loop detection bumps to 4x
        registry.open_for_loop_detection("loop_agent");
        assert_eq!(registry.effective_timeout("loop_agent"), Duration::from_secs(20));
    }
}
