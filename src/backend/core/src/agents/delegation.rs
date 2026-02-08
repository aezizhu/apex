//! Agent-to-agent delegation system.
//!
//! Enables agents to delegate subtasks to other agents based on configurable
//! strategies: direct assignment, broadcast, round-robin, least-busy, or
//! capability-based routing. This is the backbone of Apex's multi-agent
//! collaboration model.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Agent, AgentId};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Unique identifier for a delegation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DelegationId(pub Uuid);

impl DelegationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DelegationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DelegationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Priority level for delegation requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DelegationPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for DelegationPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// A request from one agent to delegate work to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationRequest {
    /// Unique identifier for this delegation.
    pub id: DelegationId,

    /// The agent initiating the delegation.
    pub from_agent: AgentId,

    /// Target agent (used with `Direct` strategy; ignored for others).
    pub to_agent: Option<AgentId>,

    /// Human-readable description of the delegated task.
    pub task: String,

    /// Structured task payload for the receiving agent.
    #[serde(default)]
    pub payload: serde_json::Value,

    /// Priority level.
    pub priority: DelegationPriority,

    /// Shared context the receiving agent may need.
    #[serde(default)]
    pub context: DelegationContext,

    /// Required capabilities (used with `Capability` strategy).
    #[serde(default)]
    pub required_capabilities: Vec<String>,

    /// When the request was created.
    pub created_at: DateTime<Utc>,

    /// Optional deadline for completion.
    pub deadline: Option<DateTime<Utc>>,
}

impl DelegationRequest {
    /// Create a new delegation request.
    pub fn new(from_agent: AgentId, task: impl Into<String>) -> Self {
        Self {
            id: DelegationId::new(),
            from_agent,
            to_agent: None,
            task: task.into(),
            payload: serde_json::Value::Null,
            priority: DelegationPriority::default(),
            context: DelegationContext::default(),
            required_capabilities: Vec::new(),
            created_at: Utc::now(),
            deadline: None,
        }
    }

    /// Builder: set a specific target agent (for Direct strategy).
    pub fn with_target(mut self, agent_id: AgentId) -> Self {
        self.to_agent = Some(agent_id);
        self
    }

    /// Builder: set the priority.
    pub fn with_priority(mut self, priority: DelegationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: set the context.
    pub fn with_context(mut self, context: DelegationContext) -> Self {
        self.context = context;
        self
    }

    /// Builder: set the payload.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Builder: add required capabilities.
    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.required_capabilities = caps;
        self
    }

    /// Builder: set deadline.
    pub fn with_deadline(mut self, deadline: DateTime<Utc>) -> Self {
        self.deadline = Some(deadline);
        self
    }
}

/// Shared context passed along with a delegation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DelegationContext {
    /// Parent task / conversation thread identifier.
    pub thread_id: Option<Uuid>,

    /// Key-value metadata the receiving agent can inspect.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Artifacts (file URLs, previous outputs, etc.) relevant to the task.
    #[serde(default)]
    pub artifacts: Vec<String>,

    /// The chain of agent IDs that have touched this delegation (audit trail).
    #[serde(default)]
    pub delegation_chain: Vec<AgentId>,
}

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

/// Strategy the `DelegationManager` uses to pick a target agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationStrategy {
    /// Send directly to a named agent (`to_agent` must be set).
    Direct,

    /// Broadcast the request to **all** available agents; the first to
    /// accept wins.
    Broadcast,

    /// Rotate through available agents in order.
    RoundRobin,

    /// Pick the agent with the lowest current load.
    LeastBusy,

    /// Match required capabilities against agent tool names / descriptions.
    Capability,
}

impl Default for DelegationStrategy {
    fn default() -> Self {
        Self::LeastBusy
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Outcome of a delegation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationResult {
    /// The original delegation request id.
    pub delegation_id: DelegationId,

    /// Whether the delegation was successfully routed.
    pub status: DelegationStatus,

    /// The agent that accepted the delegation (if any).
    pub assigned_agent: Option<AgentId>,

    /// Agents that were considered during routing.
    pub candidates_evaluated: usize,

    /// When the routing decision was made.
    pub resolved_at: DateTime<Utc>,

    /// Optional result data returned by the delegate.
    pub result_payload: Option<serde_json::Value>,

    /// Human-readable explanation of the routing decision.
    pub reason: String,
}

/// Status of a delegation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationStatus {
    /// Delegation was routed and accepted.
    Accepted,
    /// No suitable agent was found.
    NoAgentAvailable,
    /// The target agent (Direct strategy) was not found.
    TargetNotFound,
    /// The target agent is not available (busy / paused / error).
    TargetUnavailable,
    /// Delegation was explicitly rejected by the target.
    Rejected,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// Routes delegation requests to agents based on the configured strategy.
pub struct DelegationManager {
    /// Default strategy.
    strategy: DelegationStrategy,

    /// Registered agents (shared with the orchestrator).
    agents: Arc<DashMap<AgentId, Arc<Agent>>>,

    /// Round-robin index.
    rr_index: AtomicU64,

    /// Capability registry: agent_id -> Vec<capability>.
    capabilities: DashMap<AgentId, Vec<String>>,

    /// Completed delegation log (for analytics / debugging).
    history: DashMap<DelegationId, DelegationResult>,
}

impl DelegationManager {
    /// Create a new `DelegationManager`.
    pub fn new(
        strategy: DelegationStrategy,
        agents: Arc<DashMap<AgentId, Arc<Agent>>>,
    ) -> Self {
        Self {
            strategy,
            agents,
            rr_index: AtomicU64::new(0),
            capabilities: DashMap::new(),
            history: DashMap::new(),
        }
    }

    /// Register capabilities for an agent.
    pub fn register_capabilities(&self, agent_id: AgentId, caps: Vec<String>) {
        self.capabilities.insert(agent_id, caps);
    }

    /// Remove an agent's capability registration.
    pub fn unregister_capabilities(&self, agent_id: &AgentId) {
        self.capabilities.remove(agent_id);
    }

    /// Change the default strategy at runtime.
    pub fn set_strategy(&mut self, strategy: DelegationStrategy) {
        self.strategy = strategy;
    }

    /// Route a `DelegationRequest` using the manager's default strategy.
    pub fn delegate(&self, request: &DelegationRequest) -> DelegationResult {
        self.delegate_with_strategy(request, &self.strategy)
    }

    /// Route a `DelegationRequest` using an explicit strategy override.
    pub fn delegate_with_strategy(
        &self,
        request: &DelegationRequest,
        strategy: &DelegationStrategy,
    ) -> DelegationResult {
        let result = match strategy {
            DelegationStrategy::Direct => self.route_direct(request),
            DelegationStrategy::Broadcast => self.route_broadcast(request),
            DelegationStrategy::RoundRobin => self.route_round_robin(request),
            DelegationStrategy::LeastBusy => self.route_least_busy(request),
            DelegationStrategy::Capability => self.route_capability(request),
        };

        // Store in history
        self.history.insert(result.delegation_id, result.clone());

        result
    }

    /// Retrieve a past delegation result.
    pub fn get_result(&self, id: &DelegationId) -> Option<DelegationResult> {
        self.history.get(id).map(|r| r.clone())
    }

    /// Return delegation history length.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    // -- private routing implementations ------------------------------------

    fn route_direct(&self, request: &DelegationRequest) -> DelegationResult {
        let Some(target_id) = request.to_agent else {
            return self.fail_result(
                request.id,
                DelegationStatus::TargetNotFound,
                0,
                "Direct strategy requires `to_agent` to be set",
            );
        };

        let Some(agent_ref) = self.agents.get(&target_id) else {
            return self.fail_result(
                request.id,
                DelegationStatus::TargetNotFound,
                0,
                format!("Agent {} not found in registry", target_id.0),
            );
        };

        if !agent_ref.is_available() {
            return self.fail_result(
                request.id,
                DelegationStatus::TargetUnavailable,
                1,
                format!("Agent {} is not available", target_id.0),
            );
        }

        self.accept_result(request.id, target_id, 1, "Directly routed to target agent")
    }

    fn route_broadcast(&self, request: &DelegationRequest) -> DelegationResult {
        let mut evaluated = 0usize;

        // First available agent wins (could be extended with accept/reject protocol).
        for entry in self.agents.iter() {
            evaluated += 1;
            if entry.value().is_available() {
                return self.accept_result(
                    request.id,
                    *entry.key(),
                    evaluated,
                    "First available agent accepted broadcast",
                );
            }
        }

        self.fail_result(
            request.id,
            DelegationStatus::NoAgentAvailable,
            evaluated,
            "No agents available to accept broadcast",
        )
    }

    fn route_round_robin(&self, request: &DelegationRequest) -> DelegationResult {
        let available: Vec<AgentId> = self
            .agents
            .iter()
            .filter(|e| e.value().is_available())
            .map(|e| *e.key())
            .collect();

        if available.is_empty() {
            return self.fail_result(
                request.id,
                DelegationStatus::NoAgentAvailable,
                self.agents.len(),
                "No available agents for round-robin",
            );
        }

        let idx = self.rr_index.fetch_add(1, Ordering::Relaxed) as usize % available.len();
        let chosen = available[idx];

        self.accept_result(
            request.id,
            chosen,
            available.len(),
            format!("Round-robin selected agent at index {}", idx),
        )
    }

    fn route_least_busy(&self, request: &DelegationRequest) -> DelegationResult {
        let mut best: Option<(AgentId, u32)> = None;
        let mut evaluated = 0usize;

        for entry in self.agents.iter() {
            evaluated += 1;
            let agent = entry.value();
            if agent.is_available() {
                let load = agent.current_load();
                match &best {
                    None => best = Some((*entry.key(), load)),
                    Some((_, best_load)) if load < *best_load => {
                        best = Some((*entry.key(), load));
                    }
                    _ => {}
                }
            }
        }

        match best {
            Some((agent_id, load)) => self.accept_result(
                request.id,
                agent_id,
                evaluated,
                format!("Least-busy agent selected (current load: {})", load),
            ),
            None => self.fail_result(
                request.id,
                DelegationStatus::NoAgentAvailable,
                evaluated,
                "No available agents for least-busy routing",
            ),
        }
    }

    fn route_capability(&self, request: &DelegationRequest) -> DelegationResult {
        if request.required_capabilities.is_empty() {
            // Fall back to least-busy if no capabilities specified.
            return self.route_least_busy(request);
        }

        let mut best: Option<(AgentId, usize, u32)> = None; // (id, matched_caps, load)
        let mut evaluated = 0usize;

        for entry in self.agents.iter() {
            evaluated += 1;
            let agent = entry.value();
            if !agent.is_available() {
                continue;
            }

            let agent_id = *entry.key();
            let matched = if let Some(caps) = self.capabilities.get(&agent_id) {
                request
                    .required_capabilities
                    .iter()
                    .filter(|req| caps.iter().any(|c| c == *req))
                    .count()
            } else {
                // Also check agent's tool names as implicit capabilities.
                request
                    .required_capabilities
                    .iter()
                    .filter(|req| agent.tools.iter().any(|t| &t.name == *req))
                    .count()
            };

            if matched == 0 {
                continue;
            }

            let load = agent.current_load();
            match &best {
                None => best = Some((agent_id, matched, load)),
                Some((_, best_matched, best_load)) => {
                    // Prefer more capability matches; break ties with lower load.
                    if matched > *best_matched || (matched == *best_matched && load < *best_load) {
                        best = Some((agent_id, matched, load));
                    }
                }
            }
        }

        match best {
            Some((agent_id, matched, _)) => self.accept_result(
                request.id,
                agent_id,
                evaluated,
                format!(
                    "Capability-matched agent (matched {}/{} required capabilities)",
                    matched,
                    request.required_capabilities.len()
                ),
            ),
            None => self.fail_result(
                request.id,
                DelegationStatus::NoAgentAvailable,
                evaluated,
                "No agents match the required capabilities",
            ),
        }
    }

    // -- helpers ------------------------------------------------------------

    fn accept_result(
        &self,
        delegation_id: DelegationId,
        agent_id: AgentId,
        candidates_evaluated: usize,
        reason: impl Into<String>,
    ) -> DelegationResult {
        DelegationResult {
            delegation_id,
            status: DelegationStatus::Accepted,
            assigned_agent: Some(agent_id),
            candidates_evaluated,
            resolved_at: Utc::now(),
            result_payload: None,
            reason: reason.into(),
        }
    }

    fn fail_result(
        &self,
        delegation_id: DelegationId,
        status: DelegationStatus,
        candidates_evaluated: usize,
        reason: impl Into<String>,
    ) -> DelegationResult {
        DelegationResult {
            delegation_id,
            status,
            assigned_agent: None,
            candidates_evaluated,
            resolved_at: Utc::now(),
            result_payload: None,
            reason: reason.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Agent;

    fn setup_agents(count: usize) -> (Arc<DashMap<AgentId, Arc<Agent>>>, Vec<AgentId>) {
        let agents: Arc<DashMap<AgentId, Arc<Agent>>> = Arc::new(DashMap::new());
        let mut ids = Vec::new();
        for i in 0..count {
            let agent = Agent::new(format!("Agent-{}", i), "gpt-4o").with_max_load(5);
            let id = agent.id;
            agents.insert(id, Arc::new(agent));
            ids.push(id);
        }
        (agents, ids)
    }

    #[test]
    fn test_direct_delegation() {
        let (agents, ids) = setup_agents(2);
        let mgr = DelegationManager::new(DelegationStrategy::Direct, agents);

        let req = DelegationRequest::new(ids[0], "Review this code").with_target(ids[1]);
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::Accepted);
        assert_eq!(result.assigned_agent, Some(ids[1]));
    }

    #[test]
    fn test_direct_without_target_fails() {
        let (agents, ids) = setup_agents(1);
        let mgr = DelegationManager::new(DelegationStrategy::Direct, agents);

        let req = DelegationRequest::new(ids[0], "Do something");
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::TargetNotFound);
    }

    #[test]
    fn test_direct_unknown_target_fails() {
        let (agents, ids) = setup_agents(1);
        let mgr = DelegationManager::new(DelegationStrategy::Direct, agents);

        let unknown = AgentId::new();
        let req = DelegationRequest::new(ids[0], "Do something").with_target(unknown);
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::TargetNotFound);
    }

    #[test]
    fn test_broadcast_delegation() {
        let (agents, ids) = setup_agents(3);
        let mgr = DelegationManager::new(DelegationStrategy::Broadcast, agents);

        let req = DelegationRequest::new(ids[0], "Analyze data");
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::Accepted);
        assert!(result.assigned_agent.is_some());
    }

    #[test]
    fn test_round_robin_rotates() {
        let (agents, ids) = setup_agents(3);
        let mgr = DelegationManager::new(DelegationStrategy::RoundRobin, agents);

        let mut assigned = Vec::new();
        for _ in 0..6 {
            let req = DelegationRequest::new(ids[0], "Task");
            let result = mgr.delegate(&req);
            assert_eq!(result.status, DelegationStatus::Accepted);
            assigned.push(result.assigned_agent.unwrap());
        }

        // Should cycle through agents (not all the same).
        let unique: std::collections::HashSet<_> = assigned.iter().collect();
        assert!(unique.len() > 1, "Round-robin should distribute across agents");
    }

    #[test]
    fn test_least_busy_picks_lightest() {
        let (agents, ids) = setup_agents(3);

        // Load up agent 0 and 1.
        {
            let a0 = agents.get(&ids[0]).unwrap();
            a0.acquire_slot();
            a0.acquire_slot();
            a0.acquire_slot();
        }
        {
            let a1 = agents.get(&ids[1]).unwrap();
            a1.acquire_slot();
        }
        // Agent 2 has 0 load.

        let mgr = DelegationManager::new(DelegationStrategy::LeastBusy, agents);
        let req = DelegationRequest::new(ids[0], "Compute something");
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::Accepted);
        assert_eq!(result.assigned_agent, Some(ids[2]));
    }

    #[test]
    fn test_capability_routing() {
        let (agents, ids) = setup_agents(3);
        let mgr = DelegationManager::new(DelegationStrategy::Capability, agents);

        mgr.register_capabilities(ids[0], vec!["research".into(), "summarize".into()]);
        mgr.register_capabilities(ids[1], vec!["code".into(), "test".into()]);
        mgr.register_capabilities(ids[2], vec!["review".into()]);

        let req = DelegationRequest::new(ids[0], "Write tests")
            .with_capabilities(vec!["code".into(), "test".into()]);
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::Accepted);
        assert_eq!(result.assigned_agent, Some(ids[1]));
    }

    #[test]
    fn test_capability_no_match() {
        let (agents, ids) = setup_agents(2);
        let mgr = DelegationManager::new(DelegationStrategy::Capability, agents);

        mgr.register_capabilities(ids[0], vec!["research".into()]);
        mgr.register_capabilities(ids[1], vec!["code".into()]);

        let req = DelegationRequest::new(ids[0], "Deploy to production")
            .with_capabilities(vec!["devops".into(), "kubernetes".into()]);
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::NoAgentAvailable);
    }

    #[test]
    fn test_no_agents_available() {
        let agents: Arc<DashMap<AgentId, Arc<Agent>>> = Arc::new(DashMap::new());
        let mgr = DelegationManager::new(DelegationStrategy::LeastBusy, agents);

        let req = DelegationRequest::new(AgentId::new(), "Do work");
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::NoAgentAvailable);
    }

    #[test]
    fn test_delegation_history() {
        let (agents, ids) = setup_agents(2);
        let mgr = DelegationManager::new(DelegationStrategy::Direct, agents);

        let req = DelegationRequest::new(ids[0], "Task").with_target(ids[1]);
        let result = mgr.delegate(&req);

        assert_eq!(mgr.history_len(), 1);
        let stored = mgr.get_result(&result.delegation_id).unwrap();
        assert_eq!(stored.status, DelegationStatus::Accepted);
    }

    #[test]
    fn test_strategy_override() {
        let (agents, ids) = setup_agents(3);
        let mgr = DelegationManager::new(DelegationStrategy::Direct, agents);

        // Manager is configured for Direct, but we override to LeastBusy.
        let req = DelegationRequest::new(ids[0], "Work");
        let result = mgr.delegate_with_strategy(&req, &DelegationStrategy::LeastBusy);

        assert_eq!(result.status, DelegationStatus::Accepted);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(DelegationPriority::Critical > DelegationPriority::High);
        assert!(DelegationPriority::High > DelegationPriority::Normal);
        assert!(DelegationPriority::Normal > DelegationPriority::Low);
    }

    #[test]
    fn test_delegation_context_chain() {
        let (agents, ids) = setup_agents(2);
        let mgr = DelegationManager::new(DelegationStrategy::Direct, agents);

        let mut ctx = DelegationContext::default();
        ctx.delegation_chain.push(ids[0]);
        ctx.metadata
            .insert("origin".into(), serde_json::json!("planner"));

        let req = DelegationRequest::new(ids[0], "Sub-task")
            .with_target(ids[1])
            .with_context(ctx);
        let result = mgr.delegate(&req);

        assert_eq!(result.status, DelegationStatus::Accepted);
    }
}
