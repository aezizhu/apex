//! Agent definitions and management.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Unique identifier for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub Uuid);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent is available for work
    Idle,
    /// Agent is currently executing a task
    Busy,
    /// Agent encountered an error
    Error,
    /// Agent is paused (human intervention)
    Paused,
}

/// Tool configuration for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub enabled: bool,
}

/// An AI agent that can execute tasks.
pub struct Agent {
    /// Unique identifier
    pub id: AgentId,

    /// Human-readable name
    pub name: String,

    /// Model to use (e.g., "gpt-4", "claude-3-sonnet")
    pub model: String,

    /// System prompt defining agent's behavior
    pub system_prompt: String,

    /// Available tools
    pub tools: Vec<Tool>,

    /// Current status
    pub status: AgentStatus,

    /// Current load (number of active tasks)
    current_load: AtomicU32,

    /// Maximum concurrent tasks
    pub max_load: u32,

    /// Total successful task completions
    success_count: AtomicU64,

    /// Total failed task executions
    failure_count: AtomicU64,

    /// Total tokens consumed
    total_tokens: AtomicU64,

    /// Total cost incurred
    total_cost: std::sync::atomic::AtomicU64, // Stored as microdollars

    /// Reputation score (0.0 - 1.0)
    reputation_score: std::sync::atomic::AtomicU64, // Stored as millionths

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_active_at: Option<DateTime<Utc>>,
}

impl Agent {
    /// Create a new agent.
    pub fn new(name: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            id: AgentId::new(),
            name: name.into(),
            model: model.into(),
            system_prompt: String::new(),
            tools: Vec::new(),
            status: AgentStatus::Idle,
            current_load: AtomicU32::new(0),
            max_load: 10,
            success_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            total_tokens: AtomicU64::new(0),
            total_cost: std::sync::atomic::AtomicU64::new(0),
            reputation_score: std::sync::atomic::AtomicU64::new(1_000_000), // 1.0
            created_at: Utc::now(),
            last_active_at: None,
        }
    }

    /// Builder: set system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Builder: add a tool.
    pub fn with_tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    /// Builder: set max load.
    pub fn with_max_load(mut self, max: u32) -> Self {
        self.max_load = max;
        self
    }

    /// Check if agent is available for work.
    pub fn is_available(&self) -> bool {
        self.status == AgentStatus::Idle &&
            self.current_load.load(Ordering::Relaxed) < self.max_load
    }

    /// Acquire a slot (increment load).
    pub fn acquire_slot(&self) -> bool {
        let current = self.current_load.fetch_add(1, Ordering::SeqCst);
        if current >= self.max_load {
            self.current_load.fetch_sub(1, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// Release a slot (decrement load).
    pub fn release_slot(&self) {
        self.current_load.fetch_sub(1, Ordering::SeqCst);
    }

    /// Record a successful execution.
    pub fn record_success(&self, tokens: u64, cost: f64) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.total_tokens.fetch_add(tokens, Ordering::Relaxed);
        self.total_cost.fetch_add((cost * 1_000_000.0) as u64, Ordering::Relaxed);
        self.update_reputation(true);
    }

    /// Record a failed execution.
    pub fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.update_reputation(false);
    }

    /// Update reputation score based on outcome.
    fn update_reputation(&self, success: bool) {
        let current = self.reputation_score.load(Ordering::Relaxed);
        let adjustment = if success {
            // Slight increase on success (max 1.0)
            ((1_000_000 - current) / 100).min(10_000)
        } else {
            // Larger decrease on failure (min 0.0)
            (current / 50).min(50_000)
        };

        let new_score = if success {
            (current + adjustment).min(1_000_000)
        } else {
            current.saturating_sub(adjustment)
        };

        self.reputation_score.store(new_score, Ordering::Relaxed);
    }

    /// Get current load.
    pub fn current_load(&self) -> u32 {
        self.current_load.load(Ordering::Relaxed)
    }

    /// Get success count.
    pub fn success_count(&self) -> u64 {
        self.success_count.load(Ordering::Relaxed)
    }

    /// Get failure count.
    pub fn failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// Get success rate (0.0 - 1.0).
    pub fn success_rate(&self) -> f64 {
        let successes = self.success_count.load(Ordering::Relaxed);
        let failures = self.failure_count.load(Ordering::Relaxed);
        let total = successes + failures;

        if total == 0 {
            1.0 // No data yet, assume perfect
        } else {
            successes as f64 / total as f64
        }
    }

    /// Get total tokens consumed.
    pub fn total_tokens(&self) -> u64 {
        self.total_tokens.load(Ordering::Relaxed)
    }

    /// Get total cost in dollars.
    pub fn total_cost(&self) -> f64 {
        self.total_cost.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Get reputation score (0.0 - 1.0).
    pub fn reputation_score(&self) -> f64 {
        self.reputation_score.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Get agent stats.
    pub fn stats(&self) -> AgentStats {
        AgentStats {
            id: self.id,
            name: self.name.clone(),
            model: self.model.clone(),
            status: self.status.clone(),
            current_load: self.current_load(),
            max_load: self.max_load,
            success_count: self.success_count(),
            failure_count: self.failure_count(),
            success_rate: self.success_rate(),
            total_tokens: self.total_tokens(),
            total_cost: self.total_cost(),
            reputation_score: self.reputation_score(),
        }
    }
}

/// Serializable agent statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub id: AgentId,
    pub name: String,
    pub model: String,
    pub status: AgentStatus,
    pub current_load: u32,
    pub max_load: u32,
    pub success_count: u64,
    pub failure_count: u64,
    pub success_rate: f64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub reputation_score: f64,
}

/// Builder for creating agents with specific configurations.
pub struct AgentBuilder {
    name: String,
    model: String,
    system_prompt: Option<String>,
    tools: Vec<Tool>,
    max_load: u32,
}

impl AgentBuilder {
    pub fn new(name: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            model: model.into(),
            system_prompt: None,
            tools: Vec::new(),
            max_load: 10,
        }
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    pub fn max_load(mut self, max: u32) -> Self {
        self.max_load = max;
        self
    }

    pub fn build(self) -> Agent {
        let mut agent = Agent::new(self.name, self.model)
            .with_max_load(self.max_load);

        if let Some(prompt) = self.system_prompt {
            agent = agent.with_system_prompt(prompt);
        }

        for tool in self.tools {
            agent = agent.with_tool(tool);
        }

        agent
    }
}

// Predefined agent templates

impl Agent {
    /// Create a researcher agent.
    pub fn researcher() -> Self {
        Self::new("Researcher", "gpt-4o")
            .with_system_prompt(
                "You are a research agent specialized in gathering and synthesizing information. \
                 Use available tools to search, analyze, and compile comprehensive research reports."
            )
            .with_max_load(5)
    }

    /// Create a coder agent.
    pub fn coder() -> Self {
        Self::new("Coder", "claude-3.5-sonnet")
            .with_system_prompt(
                "You are a coding agent specialized in writing high-quality, well-tested code. \
                 Follow best practices, write clean code, and include appropriate tests."
            )
            .with_max_load(3)
    }

    /// Create a reviewer agent.
    pub fn reviewer() -> Self {
        Self::new("Reviewer", "gpt-4o")
            .with_system_prompt(
                "You are a review agent specialized in critically evaluating work. \
                 Check for errors, inconsistencies, and areas for improvement. \
                 Provide constructive feedback."
            )
            .with_max_load(10)
    }

    /// Create a planner agent.
    pub fn planner() -> Self {
        Self::new("Planner", "gpt-4o")
            .with_system_prompt(
                "You are a planning agent specialized in breaking down complex tasks. \
                 Create detailed execution plans with clear dependencies and milestones."
            )
            .with_max_load(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let agent = Agent::new("TestAgent", "gpt-4");
        assert_eq!(agent.name, "TestAgent");
        assert_eq!(agent.model, "gpt-4");
        assert!(agent.is_available());
    }

    #[test]
    fn test_agent_load_management() {
        let agent = Agent::new("TestAgent", "gpt-4").with_max_load(2);

        assert!(agent.acquire_slot());
        assert!(agent.acquire_slot());
        assert!(!agent.acquire_slot()); // Max reached

        agent.release_slot();
        assert!(agent.acquire_slot()); // Slot available again
    }

    #[test]
    fn test_reputation_updates() {
        let agent = Agent::new("TestAgent", "gpt-4");

        let initial = agent.reputation_score();
        assert!((initial - 1.0).abs() < 0.001);

        // Failures decrease reputation
        agent.record_failure();
        assert!(agent.reputation_score() < initial);

        // Successes increase reputation
        let after_failure = agent.reputation_score();
        agent.record_success(100, 0.01);
        assert!(agent.reputation_score() > after_failure);
    }
}
