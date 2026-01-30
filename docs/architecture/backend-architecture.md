# Project Apex - Backend Architecture

## Executive Summary

Project Apex is a world-class agent swarm orchestration system designed to manage thousands of concurrent AI agents executing complex, interdependent tasks. The architecture combines Rust (tokio) for high-performance core orchestration with Python (asyncio) for flexible agent execution.

---

## Table of Contents

1. [System Architecture Overview](#1-system-architecture-overview)
2. [DAG Executor Design](#2-dag-executor-design)
3. [Agent Contract Framework](#3-agent-contract-framework)
4. [Contract Net Protocol](#4-contract-net-protocol)
5. [FrugalGPT Adaptive Model Routing](#5-frugalgpt-adaptive-model-routing)
6. [API Specifications](#6-api-specifications)
7. [Database Schema Overview](#7-database-schema-overview)
8. [Rust Project Structure](#8-rust-project-structure)
9. [Key Rust Code Skeletons](#9-key-rust-code-skeletons)

---

## 1. System Architecture Overview

### ASCII System Diagram

```
+-----------------------------------------------------------------------------------+
|                              PROJECT APEX ARCHITECTURE                             |
+-----------------------------------------------------------------------------------+

                              +-------------------+
                              |   API Gateway     |
                              |  (tonic gRPC)     |
                              +--------+----------+
                                       |
                    +------------------+------------------+
                    |                                     |
           +--------v--------+                   +--------v--------+
           | Swarm Manager   |                   | Metrics/Monitor |
           | (Orchestrator)  |                   | (Prometheus)    |
           +--------+--------+                   +-----------------+
                    |
     +--------------+--------------+
     |              |              |
+----v----+   +----v----+   +----v----+
|   DAG   |   |Contract |   | Model   |
|Executor |   |  Net    |   | Router  |
|(Scheduler)  |Protocol |   |(Frugal) |
+----+----+   +----+----+   +----+----+
     |              |              |
     +--------------+--------------+
                    |
           +--------v--------+
           |   Worker Pool   |
           | (1000+ tokio    |
           |    tasks)       |
           +--------+--------+
                    |
     +--------------+--------------+--------------+
     |              |              |              |
+----v----+   +----v----+   +----v----+   +----v----+
| Agent   |   | Agent   |   | Agent   |   | Agent   |
| Runner  |   | Runner  |   | Runner  |   | Runner  |
|(Python) |   |(Python) |   |(Python) |   |(Python) |
+---------+   +---------+   +---------+   +---------+
     |              |              |              |
     +--------------+--------------+--------------+
                    |
           +--------v--------+
           |   PostgreSQL    |
           | (Task Queue &   |
           |   State Store)  |
           +-----------------+

+-----------------------------------------------------------------------------------+
|                           DATA FLOW LEGEND                                         |
|  ---->  Synchronous gRPC call                                                      |
|  ~~~~>  Async message/event                                                        |
|  <--->  Bidirectional stream                                                       |
+-----------------------------------------------------------------------------------+
```

### Component Responsibilities

| Component | Technology | Responsibility |
|-----------|------------|----------------|
| API Gateway | Rust + tonic | External interface, authentication, rate limiting |
| Swarm Manager | Rust + tokio | Orchestrates swarms, manages lifecycle |
| DAG Executor | Rust | Task scheduling, dependency resolution |
| Contract Net | Rust | Task bidding, award selection |
| Model Router | Rust | FrugalGPT model cascade logic |
| Worker Pool | Rust + tokio | Manages 1000+ concurrent agent tasks |
| Agent Runner | Python + asyncio | Executes individual agent logic |
| PostgreSQL | PostgreSQL 15+ | Persistent task queue, state storage |

---

## 2. DAG Executor Design

### 2.1 Job Queue (PostgreSQL-backed)

The job queue provides durable task persistence with exactly-once delivery semantics using PostgreSQL's `SKIP LOCKED` feature.

#### Queue Architecture

```
+------------------+     +------------------+     +------------------+
|  Task Producer   | --> |   PostgreSQL     | --> |  Task Consumer   |
|  (API/Swarm)     |     |   tasks table    |     |  (Worker Pool)   |
+------------------+     +------------------+     +------------------+
                               |
                               v
                    +--------------------+
                    | Status: pending    |
                    | Status: running    |
                    | Status: completed  |
                    | Status: failed     |
                    | Status: cancelled  |
                    +--------------------+
```

#### Task States

```
          +----------+
          | pending  |
          +----+-----+
               |
       +-------+-------+
       |               |
  +----v-----+   +-----v----+
  | running  |   |cancelled |
  +----+-----+   +----------+
       |
  +----+-----+-------+
  |          |       |
+-v---+  +---v--+ +--v------+
|done |  |failed| |timed_out|
+-----+  +--+---+ +---------+
            |
      +-----v-----+
      | retrying  |---> (back to pending)
      +-----------+
```

### 2.2 Worker Pool (tokio-based)

The worker pool manages concurrent agent execution using tokio's lightweight task system.

#### Pool Configuration

```rust
// Configuration for worker pool
pub struct WorkerPoolConfig {
    /// Maximum concurrent agents (target: 1000+)
    pub max_workers: usize,          // Default: 1024

    /// Workers per CPU core
    pub workers_per_core: usize,     // Default: 64

    /// Task queue buffer size
    pub queue_buffer: usize,         // Default: 10000

    /// Graceful shutdown timeout
    pub shutdown_timeout: Duration,  // Default: 30s

    /// Health check interval
    pub health_check_interval: Duration, // Default: 5s
}
```

#### Scaling Strategy

```
Load Level    | Active Workers | Strategy
--------------+----------------+---------------------------
Idle (0-10%)  | 64             | Minimum pool, fast response
Light (10-40%)| 256            | Scale up gradually
Medium (40-70)| 512            | Balanced utilization
Heavy (70-90%)| 768            | Near capacity
Max (90-100%) | 1024           | Full capacity, queue overflow to DB
Overload      | 1024 + Queue   | Backpressure, reject new tasks
```

### 2.3 Dependency Resolution

#### Topological Sort Algorithm

```
Input: Task DAG G = (V, E)
Output: Linear ordering of tasks respecting dependencies

TOPOLOGICAL_SORT(G):
    1. Calculate in-degree for each vertex
    2. Initialize queue Q with all vertices where in_degree = 0
    3. Initialize result list L = []

    4. While Q is not empty:
        a. Dequeue vertex v from Q
        b. Append v to L
        c. For each neighbor u of v:
            i.  Decrement in_degree[u]
            ii. If in_degree[u] == 0, enqueue u to Q

    5. If |L| != |V|:
        Return ERROR: Cycle detected

    6. Return L
```

#### Cycle Detection

```rust
/// Cycle detection using DFS with coloring
///
/// Colors:
/// - White (0): Unvisited
/// - Gray (1): Currently visiting (in recursion stack)
/// - Black (2): Fully processed
///
/// A cycle exists if we encounter a Gray node during DFS
```

### 2.4 Failure Handling

#### Retry Logic with Exponential Backoff

```
Attempt | Delay    | Total Wait
--------+----------+------------
1       | 0s       | 0s
2       | 1s       | 1s
3       | 2s       | 3s
4       | 4s       | 7s
5       | 8s       | 15s
6       | 16s      | 31s
7       | 32s      | 63s (max)

Formula: delay = min(base * 2^(attempt-1), max_delay)
         base = 1s, max_delay = 32s, max_attempts = 7
```

#### Circuit Breaker State Machine

```
                    +------------+
             +----->|   CLOSED   |<-----+
             |      +-----+------+      |
             |            |             |
             |      failure_count++     |
             |            |             |
             |    threshold reached     |
             |            |             |
             |      +-----v------+      |
     timeout |      |    OPEN    |      | success
     expired |      +-----+------+      |
             |            |             |
             |      timeout expires     |
             |            |             |
             |      +-----v------+      |
             +------+ HALF-OPEN  +------+
                    +------------+
                          |
                       failure
                          |
                    +-----v------+
                    |    OPEN    |
                    +------------+
```

#### Cascading Cancellation

When a parent task is cancelled, all descendant tasks must be cancelled:

```
Task A (cancelled)
  |
  +-- Task B (auto-cancel) ----+
  |     |                      |
  |     +-- Task D (auto-cancel)
  |     +-- Task E (auto-cancel)
  |
  +-- Task C (auto-cancel) ----+
        |                      |
        +-- Task F (auto-cancel)
```

---

## 3. Agent Contract Framework

### 3.1 Contract Structure

```rust
pub struct AgentContract {
    // === Identity ===
    pub contract_id: Uuid,
    pub agent_id: Uuid,
    pub parent_contract_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,

    // === Specifications ===
    pub input_spec: InputSpec,
    pub output_spec: OutputSpec,

    // === Resource Limits ===
    pub resource_limits: ResourceLimits,

    // === Temporal Bounds ===
    pub temporal_bounds: TemporalBounds,

    // === Success/Failure Criteria ===
    pub success_criteria: SuccessCriteria,
    pub failure_modes: Vec<FailureMode>,
}
```

#### Input/Output Specifications

```rust
pub struct InputSpec {
    /// JSON Schema for validating input
    pub schema: serde_json::Value,

    /// Required input keys
    pub required_fields: Vec<String>,

    /// Optional fields with defaults
    pub optional_fields: HashMap<String, serde_json::Value>,

    /// Maximum input payload size in bytes
    pub max_size_bytes: usize,
}

pub struct OutputSpec {
    /// JSON Schema for validating output
    pub schema: serde_json::Value,

    /// Required output keys
    pub required_fields: Vec<String>,

    /// Output validation function (optional)
    pub validator: Option<String>, // Function name to call

    /// Maximum output payload size
    pub max_size_bytes: usize,
}
```

#### Resource Limits

```rust
pub struct ResourceLimits {
    // === Token Limits ===
    /// Maximum input tokens
    pub max_input_tokens: u64,

    /// Maximum output tokens
    pub max_output_tokens: u64,

    /// Total token budget (input + output across all calls)
    pub total_token_budget: u64,

    // === Cost Limits ===
    /// Maximum cost in USD (micro-dollars for precision)
    pub max_cost_microdollars: u64,

    // === API Call Limits ===
    /// Maximum LLM API calls
    pub max_llm_calls: u32,

    /// Maximum external API calls
    pub max_external_api_calls: u32,

    /// Maximum tool/function calls
    pub max_tool_calls: u32,

    // === Compute Limits ===
    /// Maximum CPU time in milliseconds
    pub max_cpu_time_ms: u64,

    /// Maximum memory in bytes
    pub max_memory_bytes: u64,

    /// Maximum execution wall time
    pub max_wall_time: Duration,
}
```

#### Temporal Bounds

```rust
pub struct TemporalBounds {
    /// Earliest allowed start time
    pub not_before: Option<DateTime<Utc>>,

    /// Latest allowed completion time (hard deadline)
    pub deadline: DateTime<Utc>,

    /// Soft deadline (trigger warnings)
    pub soft_deadline: Option<DateTime<Utc>>,

    /// Maximum execution duration
    pub max_duration: Duration,

    /// Heartbeat interval (agent must report)
    pub heartbeat_interval: Duration,

    /// Heartbeat timeout (consider dead after)
    pub heartbeat_timeout: Duration,
}
```

#### Success/Failure Criteria

```rust
pub struct SuccessCriteria {
    /// Output must match this JSON schema
    pub output_schema_valid: bool,

    /// Minimum confidence score (0.0 - 1.0)
    pub min_confidence: Option<f64>,

    /// Custom validation function
    pub custom_validator: Option<String>,

    /// Required output fields that must be non-null
    pub required_non_null: Vec<String>,
}

pub struct FailureMode {
    /// Failure type identifier
    pub failure_type: FailureType,

    /// Is this failure retryable?
    pub retryable: bool,

    /// Maximum retries for this failure type
    pub max_retries: u32,

    /// Fallback action
    pub fallback: FallbackAction,
}

pub enum FailureType {
    Timeout,
    TokenLimitExceeded,
    CostLimitExceeded,
    ApiCallLimitExceeded,
    OutputValidationFailed,
    ExternalServiceError,
    AgentCrash,
    ContractViolation,
}

pub enum FallbackAction {
    Retry,
    EscalateToParent,
    UseDefaultOutput(serde_json::Value),
    CancelSubtree,
    AlertHuman,
}
```

### 3.2 Enforcement Mechanism

```
+------------------+     +------------------+     +------------------+
|  Agent Runtime   | --> | Contract Guard   | --> |   LLM Provider   |
|                  | <-- | (Interceptor)    | <-- |                  |
+------------------+     +------------------+     +------------------+
                               |
                               v
                    +--------------------+
                    | Budget Tracker     |
                    |--------------------|
                    | tokens_used: 1234  |
                    | cost_used: $0.05   |
                    | api_calls: 3       |
                    | time_elapsed: 45s  |
                    +--------------------+
                               |
            +------------------+------------------+
            |                  |                  |
       +----v----+       +----v----+       +----v----+
       | ALLOW   |       | THROTTLE|       |  DENY   |
       | (green) |       | (yellow)|       |  (red)  |
       +---------+       +---------+       +---------+
```

#### Pre-Call Validation

```rust
impl ContractGuard {
    pub async fn pre_call_check(&self, request: &LlmRequest) -> GuardDecision {
        // 1. Check token budget
        let estimated_tokens = self.estimate_tokens(request);
        if self.budget.tokens_used + estimated_tokens > self.contract.resource_limits.total_token_budget {
            return GuardDecision::Deny(DenyReason::TokenBudgetExceeded);
        }

        // 2. Check cost budget
        let estimated_cost = self.estimate_cost(request);
        if self.budget.cost_used + estimated_cost > self.contract.resource_limits.max_cost_microdollars {
            return GuardDecision::Deny(DenyReason::CostBudgetExceeded);
        }

        // 3. Check API call limit
        if self.budget.api_calls >= self.contract.resource_limits.max_llm_calls {
            return GuardDecision::Deny(DenyReason::ApiCallLimitExceeded);
        }

        // 4. Check temporal bounds
        if Utc::now() > self.contract.temporal_bounds.deadline {
            return GuardDecision::Deny(DenyReason::DeadlineExceeded);
        }

        // 5. Check for soft limits (throttle)
        if self.approaching_limits() {
            return GuardDecision::Throttle(ThrottleConfig {
                delay: Duration::from_millis(100),
                reduce_tokens: true,
            });
        }

        GuardDecision::Allow
    }
}
```

### 3.3 Conservation Law

**Fundamental Principle**: A parent contract's budget must be sufficient to cover all child budgets plus operational overhead.

```
CONSERVATION LAW:
    parent_budget >= sum(child_budgets) + overhead

Where:
    overhead = orchestration_cost + communication_cost + safety_margin

Example:
    Parent Budget: $1.00, 10000 tokens

    Child 1: $0.30, 3000 tokens
    Child 2: $0.25, 2500 tokens
    Child 3: $0.20, 2000 tokens
    ---------------------------------
    Subtotal: $0.75, 7500 tokens
    Overhead: $0.10, 1000 tokens (10% safety margin)
    ---------------------------------
    Required: $0.85, 8500 tokens
    Available: $1.00, 10000 tokens

    VALID (conservation law satisfied)
```

#### Budget Allocation Algorithm

```rust
pub struct BudgetAllocator;

impl BudgetAllocator {
    pub fn allocate_child_budgets(
        parent: &AgentContract,
        children: &[ChildRequest],
    ) -> Result<Vec<ResourceLimits>, AllocationError> {
        let total_requested: ResourceLimits = children
            .iter()
            .map(|c| &c.requested_limits)
            .sum();

        let overhead = parent.resource_limits.calculate_overhead();
        let available = parent.resource_limits.subtract(&overhead)?;

        if total_requested > available {
            // Option 1: Proportional scaling
            let scale_factor = available.as_ratio(&total_requested);
            return Ok(children
                .iter()
                .map(|c| c.requested_limits.scale(scale_factor))
                .collect());
        }

        // Conservation law satisfied
        Ok(children.iter().map(|c| c.requested_limits.clone()).collect())
    }
}
```

---

## 4. Contract Net Protocol (CNP)

### 4.1 Protocol Overview

```
Manager                          Bidders (Agent Pool)
   |                                    |
   |---- [1] Task Announcement (RFP) -->|
   |         (broadcast)                |
   |                                    |
   |<--- [2] Bid Submission ------------|
   |         (multiple bids)            |
   |                                    |
   |---- [3] Award Notification ------->|
   |         (to winner)                |
   |                                    |
   |---- [4] Rejection Notification --->|
   |         (to losers)                |
   |                                    |
   |<--- [5] Contract Acceptance -------|
   |                                    |
   |<~~~ [6] Heartbeats ~~~~~~~~~~~~~~~~|
   |         (periodic)                 |
   |                                    |
   |<--- [7] Result Submission ---------|
   |                                    |
```

### 4.2 Task Announcement (RFP)

```rust
pub struct TaskAnnouncement {
    /// Unique announcement ID
    pub announcement_id: Uuid,

    /// Task to be performed
    pub task: TaskDescriptor,

    /// Required capabilities
    pub required_capabilities: Vec<Capability>,

    /// Resource budget available
    pub available_budget: ResourceLimits,

    /// Deadline for bids
    pub bid_deadline: DateTime<Utc>,

    /// Task execution deadline
    pub execution_deadline: DateTime<Utc>,

    /// Minimum quality threshold
    pub min_quality_score: f64,

    /// Bid evaluation criteria weights
    pub evaluation_weights: EvaluationWeights,
}

pub struct EvaluationWeights {
    pub cost_weight: f64,      // e.g., 0.3
    pub time_weight: f64,      // e.g., 0.2
    pub quality_weight: f64,   // e.g., 0.3
    pub reliability_weight: f64, // e.g., 0.2
}
```

### 4.3 Bidding Mechanism

```rust
pub struct Bid {
    /// Bid identifier
    pub bid_id: Uuid,

    /// Reference to announcement
    pub announcement_id: Uuid,

    /// Bidding agent
    pub agent_id: Uuid,

    /// Proposed cost (marginal cost)
    pub proposed_cost: MarginalCost,

    /// Estimated completion time
    pub estimated_duration: Duration,

    /// Quality commitment
    pub quality_commitment: f64,

    /// Agent's capability proof
    pub capability_proof: CapabilityProof,

    /// Historical performance metrics
    pub performance_history: PerformanceMetrics,

    /// Bid submitted at
    pub submitted_at: DateTime<Utc>,
}

pub struct MarginalCost {
    /// Token cost estimate
    pub tokens: u64,

    /// Dollar cost estimate (micro-dollars)
    pub cost_microdollars: u64,

    /// Opportunity cost (what else could this agent do)
    pub opportunity_cost: f64,

    /// Confidence in estimate (0-1)
    pub confidence: f64,
}
```

#### Marginal Cost Calculation

```
MARGINAL_COST(task, agent) =
    direct_cost(task, agent) +
    opportunity_cost(agent) +
    risk_premium(task, agent)

Where:
    direct_cost = estimated_tokens * token_price(model)

    opportunity_cost = (queue_depth * avg_task_value) / time_to_complete

    risk_premium = base_risk * (1 - agent_reliability_score)
```

### 4.4 Award Selection Algorithm

```rust
impl AwardSelector {
    pub fn select_winner(&self, bids: Vec<Bid>, criteria: &EvaluationWeights) -> Option<Bid> {
        if bids.is_empty() {
            return None;
        }

        // Normalize all scores to 0-1 range
        let normalized = self.normalize_bids(&bids);

        // Calculate weighted score for each bid
        let scored: Vec<(Bid, f64)> = normalized
            .into_iter()
            .map(|bid| {
                let score =
                    criteria.cost_weight * (1.0 - bid.normalized_cost) +  // Lower cost = higher score
                    criteria.time_weight * (1.0 - bid.normalized_time) +  // Faster = higher score
                    criteria.quality_weight * bid.quality_commitment +
                    criteria.reliability_weight * bid.reliability_score;
                (bid.original, score)
            })
            .collect();

        // Sort by score descending
        let mut scored = scored;
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Return highest scoring bid
        scored.into_iter().next().map(|(bid, _)| bid)
    }
}
```

### 4.5 Heartbeat Monitoring

```
Agent                           Orchestrator
  |                                   |
  |------- heartbeat {status} ------->|
  |                                   | Update last_seen
  |                                   | Check health
  |                                   |
  |           (interval: 5s)          |
  |                                   |
  |------- heartbeat {status} ------->|
  |                                   |
  |       (missed heartbeat)          |
  |                                   |
  |                                   | timeout (15s)
  |                                   | Mark as unhealthy
  |                                   | Trigger recovery
  |                                   |
```

```rust
pub struct HeartbeatMessage {
    pub agent_id: Uuid,
    pub task_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub status: AgentStatus,
    pub progress: Progress,
    pub resource_usage: ResourceUsage,
}

pub struct Progress {
    pub percent_complete: f64,
    pub current_step: String,
    pub estimated_remaining: Duration,
}

pub enum AgentStatus {
    Healthy,
    Degraded { reason: String },
    Overloaded,
    ShuttingDown,
}
```

### 4.6 Failure Recovery

```
Task Assignment Failed
         |
         v
+-------------------+
| Check Runner-Up   |
| Bids Available?   |
+--------+----------+
         |
    +----+----+
    |         |
   Yes        No
    |         |
    v         v
+-------+  +----------------+
|Select |  |Re-announce     |
|Runner |  |Task (new RFP)  |
|Up     |  +--------+-------+
+---+---+           |
    |               v
    |      +----------------+
    |      |Wait for new    |
    |      |bids            |
    |      +--------+-------+
    |               |
    +-------+-------+
            |
            v
   +------------------+
   |Assign to new     |
   |winner            |
   +------------------+
```

```rust
pub struct FailureRecovery {
    /// Original announcement
    announcement: TaskAnnouncement,

    /// All received bids, sorted by score
    ranked_bids: Vec<(Bid, f64)>,

    /// Current attempt number
    attempt: u32,

    /// Maximum attempts before escalation
    max_attempts: u32,
}

impl FailureRecovery {
    pub async fn recover(&mut self) -> RecoveryResult {
        self.attempt += 1;

        // Try runner-up
        if let Some(runner_up) = self.get_next_runner_up() {
            return RecoveryResult::AssignToRunnerUp(runner_up);
        }

        // Re-announce with relaxed criteria
        if self.attempt < self.max_attempts {
            let relaxed = self.relax_criteria();
            return RecoveryResult::ReAnnounce(relaxed);
        }

        // Escalate to parent or human
        RecoveryResult::Escalate(EscalationReason::ExhaustedRetries)
    }
}
```

---

## 5. FrugalGPT Adaptive Model Routing

### 5.1 Model Cascade Architecture

```
                    Input Query
                         |
                         v
              +--------------------+
              | Complexity Analyzer|
              +----------+---------+
                         |
         +---------------+---------------+
         |               |               |
         v               v               v
    +---------+    +---------+    +---------+
    | Tier 1  |    | Tier 2  |    | Tier 3  |
    | Cheap   |    |   Mid   |    |Expensive|
    |---------|    |---------|    |---------|
    |GPT-4o-  |    |Claude   |    |Claude   |
    |mini     |    |Sonnet   |    |Opus     |
    |Haiku    |    |GPT-4o   |    |GPT-4    |
    +---------+    +---------+    +---------+
         |               |               |
         v               v               v
    +---------+    +---------+    +---------+
    | Quality |    | Quality |    | Quality |
    | Check   |    | Check   |    | (final) |
    +----+----+    +----+----+    +---------+
         |               |
    Pass | Fail     Pass | Fail
         |               |
         v               v
      Output        Escalate
```

### 5.2 Model Tiers

| Tier | Models | Cost/1K tokens | Use Case |
|------|--------|----------------|----------|
| 1 (Cheap) | GPT-4o-mini, Claude Haiku | $0.0001-0.0005 | Simple tasks, classification, extraction |
| 2 (Mid) | Claude Sonnet, GPT-4o | $0.003-0.015 | Moderate complexity, analysis |
| 3 (Expensive) | Claude Opus, GPT-4, o1 | $0.015-0.060 | Complex reasoning, critical tasks |

### 5.3 Scoring Function

```rust
pub struct QualityScorer {
    /// Confidence threshold for each tier
    tier_thresholds: [f64; 3],  // e.g., [0.85, 0.75, 0.0]
}

impl QualityScorer {
    pub fn score_output(&self, output: &LlmOutput, task: &Task) -> QualityScore {
        let mut score = QualityScore::default();

        // 1. Confidence from model (if available)
        score.model_confidence = output.confidence.unwrap_or(0.5);

        // 2. Output completeness
        score.completeness = self.check_completeness(output, &task.output_spec);

        // 3. Format validity
        score.format_valid = self.validate_format(output, &task.output_spec);

        // 4. Consistency check (if multiple samples)
        score.consistency = self.check_consistency(output);

        // 5. Task-specific heuristics
        score.task_heuristics = self.apply_task_heuristics(output, task);

        // Weighted combination
        score.overall =
            0.3 * score.model_confidence +
            0.25 * score.completeness +
            0.2 * score.format_valid +
            0.15 * score.consistency +
            0.1 * score.task_heuristics;

        score
    }
}
```

### 5.4 Escalation Logic

```rust
pub struct ModelRouter {
    tiers: Vec<ModelTier>,
    scorer: QualityScorer,
    cost_tracker: CostTracker,
}

impl ModelRouter {
    pub async fn route(&self, task: &Task, contract: &AgentContract) -> RoutingResult {
        for (tier_idx, tier) in self.tiers.iter().enumerate() {
            // Check if we have budget for this tier
            let estimated_cost = tier.estimate_cost(task);
            if !self.cost_tracker.can_afford(estimated_cost, contract) {
                continue; // Skip to cheaper alternative or fail
            }

            // Execute with this tier
            let output = tier.execute(task).await?;
            let score = self.scorer.score_output(&output, task);

            // Check if quality meets threshold
            let threshold = self.get_threshold(tier_idx, task);
            if score.overall >= threshold {
                return RoutingResult::Success {
                    output,
                    model: tier.model.clone(),
                    score,
                    cost: tier.actual_cost(&output),
                };
            }

            // Log and escalate
            tracing::info!(
                tier = tier_idx,
                score = score.overall,
                threshold = threshold,
                "Escalating to next tier"
            );
        }

        RoutingResult::ExhaustedTiers
    }

    fn get_threshold(&self, tier_idx: usize, task: &Task) -> f64 {
        // Higher tiers have lower thresholds (we accept more)
        // Critical tasks have higher thresholds
        let base_threshold = match tier_idx {
            0 => 0.90,  // Tier 1: very high bar
            1 => 0.80,  // Tier 2: high bar
            2 => 0.60,  // Tier 3: accept most
            _ => 0.50,
        };

        // Adjust for task criticality
        base_threshold * task.quality_multiplier
    }
}
```

### 5.5 Cost Aggregation

```rust
pub struct CostTracker {
    /// Costs by model
    model_costs: HashMap<String, CostRecord>,

    /// Total cost
    total_cost_microdollars: AtomicU64,

    /// Token counts
    total_input_tokens: AtomicU64,
    total_output_tokens: AtomicU64,
}

pub struct CostRecord {
    pub model_id: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microdollars: u64,
    pub call_count: u64,
}

impl CostTracker {
    pub fn record_usage(&self, model: &str, input_tokens: u64, output_tokens: u64) {
        let pricing = self.get_pricing(model);
        let cost =
            (input_tokens as f64 * pricing.input_per_token) +
            (output_tokens as f64 * pricing.output_per_token);

        let cost_microdollars = (cost * 1_000_000.0) as u64;

        self.total_cost_microdollars.fetch_add(cost_microdollars, Ordering::SeqCst);
        self.total_input_tokens.fetch_add(input_tokens, Ordering::SeqCst);
        self.total_output_tokens.fetch_add(output_tokens, Ordering::SeqCst);

        // Update per-model stats
        self.model_costs
            .entry(model.to_string())
            .or_default()
            .add(input_tokens, output_tokens, cost_microdollars);
    }

    pub fn get_total_cost_usd(&self) -> f64 {
        self.total_cost_microdollars.load(Ordering::SeqCst) as f64 / 1_000_000.0
    }
}
```

---

## 6. API Specifications (gRPC)

### 6.1 Service Definitions

```protobuf
syntax = "proto3";

package apex.v1;

// =============================================================================
// SWARM SERVICE - Managing agent swarms
// =============================================================================

service SwarmService {
    // Create a new swarm with initial configuration
    rpc CreateSwarm(CreateSwarmRequest) returns (CreateSwarmResponse);

    // Get swarm status and metrics
    rpc GetSwarm(GetSwarmRequest) returns (GetSwarmResponse);

    // List all swarms with optional filtering
    rpc ListSwarms(ListSwarmsRequest) returns (ListSwarmsResponse);

    // Terminate a swarm and all its agents
    rpc TerminateSwarm(TerminateSwarmRequest) returns (TerminateSwarmResponse);

    // Stream swarm events in real-time
    rpc StreamSwarmEvents(StreamSwarmEventsRequest) returns (stream SwarmEvent);
}

// =============================================================================
// AGENT SERVICE - Spawning and managing agents
// =============================================================================

service AgentService {
    // Spawn a new agent with contract
    rpc SpawnAgent(SpawnAgentRequest) returns (SpawnAgentResponse);

    // Get agent status
    rpc GetAgent(GetAgentRequest) returns (GetAgentResponse);

    // List agents with filtering
    rpc ListAgents(ListAgentsRequest) returns (ListAgentsResponse);

    // Terminate an agent
    rpc TerminateAgent(TerminateAgentRequest) returns (TerminateAgentResponse);

    // Send message to agent
    rpc SendMessage(SendMessageRequest) returns (SendMessageResponse);

    // Stream agent output
    rpc StreamAgentOutput(StreamAgentOutputRequest) returns (stream AgentOutput);
}

// =============================================================================
// TASK SERVICE - Task submission and management
// =============================================================================

service TaskService {
    // Submit a new task
    rpc SubmitTask(SubmitTaskRequest) returns (SubmitTaskResponse);

    // Submit a DAG of tasks
    rpc SubmitTaskDAG(SubmitTaskDAGRequest) returns (SubmitTaskDAGResponse);

    // Get task status
    rpc GetTaskStatus(GetTaskStatusRequest) returns (GetTaskStatusResponse);

    // Get task result
    rpc GetTaskResult(GetTaskResultRequest) returns (GetTaskResultResponse);

    // Cancel a task
    rpc CancelTask(CancelTaskRequest) returns (CancelTaskResponse);

    // Retry a failed task
    rpc RetryTask(RetryTaskRequest) returns (RetryTaskResponse);

    // Stream task progress
    rpc StreamTaskProgress(StreamTaskProgressRequest) returns (stream TaskProgress);
}

// =============================================================================
// CONTRACT SERVICE - Contract management
// =============================================================================

service ContractService {
    // Create a new contract
    rpc CreateContract(CreateContractRequest) returns (CreateContractResponse);

    // Get contract details
    rpc GetContract(GetContractRequest) returns (GetContractResponse);

    // Check contract budget status
    rpc GetContractBudget(GetContractBudgetRequest) returns (GetContractBudgetResponse);

    // Modify contract limits (with conservation law check)
    rpc ModifyContract(ModifyContractRequest) returns (ModifyContractResponse);
}
```

### 6.2 Message Definitions

```protobuf
// =============================================================================
// COMMON TYPES
// =============================================================================

message ResourceLimits {
    uint64 max_input_tokens = 1;
    uint64 max_output_tokens = 2;
    uint64 total_token_budget = 3;
    uint64 max_cost_microdollars = 4;
    uint32 max_llm_calls = 5;
    uint32 max_external_api_calls = 6;
    uint32 max_tool_calls = 7;
    uint64 max_cpu_time_ms = 8;
    uint64 max_memory_bytes = 9;
    google.protobuf.Duration max_wall_time = 10;
}

message TemporalBounds {
    google.protobuf.Timestamp not_before = 1;
    google.protobuf.Timestamp deadline = 2;
    google.protobuf.Timestamp soft_deadline = 3;
    google.protobuf.Duration max_duration = 4;
    google.protobuf.Duration heartbeat_interval = 5;
    google.protobuf.Duration heartbeat_timeout = 6;
}

// =============================================================================
// AGENT MESSAGES
// =============================================================================

message SpawnAgentRequest {
    string swarm_id = 1;
    string agent_type = 2;
    AgentContract contract = 3;
    bytes initial_input = 4;
    map<string, string> metadata = 5;
    optional string parent_agent_id = 6;
}

message SpawnAgentResponse {
    string agent_id = 1;
    AgentStatus status = 2;
    google.protobuf.Timestamp created_at = 3;
}

message AgentContract {
    string contract_id = 1;
    InputSpec input_spec = 2;
    OutputSpec output_spec = 3;
    ResourceLimits resource_limits = 4;
    TemporalBounds temporal_bounds = 5;
    SuccessCriteria success_criteria = 6;
    repeated FailureMode failure_modes = 7;
}

enum AgentStatus {
    AGENT_STATUS_UNSPECIFIED = 0;
    AGENT_STATUS_PENDING = 1;
    AGENT_STATUS_INITIALIZING = 2;
    AGENT_STATUS_RUNNING = 3;
    AGENT_STATUS_PAUSED = 4;
    AGENT_STATUS_COMPLETED = 5;
    AGENT_STATUS_FAILED = 6;
    AGENT_STATUS_TERMINATED = 7;
}

// =============================================================================
// TASK MESSAGES
// =============================================================================

message SubmitTaskRequest {
    string swarm_id = 1;
    TaskDescriptor task = 2;
    optional string assigned_agent_id = 3;
    Priority priority = 4;
}

message SubmitTaskDAGRequest {
    string swarm_id = 1;
    repeated TaskNode nodes = 2;
    repeated TaskEdge edges = 3;
    ResourceLimits total_budget = 4;
}

message TaskNode {
    string node_id = 1;
    TaskDescriptor task = 2;
    optional string preferred_agent_type = 3;
}

message TaskEdge {
    string from_node_id = 1;
    string to_node_id = 2;
    EdgeType edge_type = 3;
}

enum EdgeType {
    EDGE_TYPE_UNSPECIFIED = 0;
    EDGE_TYPE_DEPENDS_ON = 1;      // to depends on from
    EDGE_TYPE_DATA_FLOW = 2;       // data flows from -> to
    EDGE_TYPE_CANCEL_ON_FAIL = 3;  // cancel to if from fails
}

message TaskProgress {
    string task_id = 1;
    TaskStatus status = 2;
    float progress_percent = 3;
    string current_step = 4;
    google.protobuf.Duration estimated_remaining = 5;
    ResourceUsage resource_usage = 6;
    google.protobuf.Timestamp updated_at = 7;
}

enum TaskStatus {
    TASK_STATUS_UNSPECIFIED = 0;
    TASK_STATUS_PENDING = 1;
    TASK_STATUS_QUEUED = 2;
    TASK_STATUS_RUNNING = 3;
    TASK_STATUS_COMPLETED = 4;
    TASK_STATUS_FAILED = 5;
    TASK_STATUS_CANCELLED = 6;
    TASK_STATUS_TIMED_OUT = 7;
    TASK_STATUS_RETRYING = 8;
}

enum Priority {
    PRIORITY_UNSPECIFIED = 0;
    PRIORITY_LOW = 1;
    PRIORITY_NORMAL = 2;
    PRIORITY_HIGH = 3;
    PRIORITY_CRITICAL = 4;
}
```

---

## 7. Database Schema Overview

### 7.1 Entity Relationship Diagram

```
+------------------+       +------------------+       +------------------+
|     swarms       |       |     agents       |       |    contracts     |
|------------------|       |------------------|       |------------------|
| id (PK)          |<------| swarm_id (FK)    |       | id (PK)          |
| name             |       | id (PK)          |<------| agent_id (FK)    |
| status           |       | type             |       | input_spec       |
| config           |       | status           |       | output_spec      |
| created_at       |       | contract_id (FK) |------>| resource_limits  |
| updated_at       |       | parent_agent_id  |       | temporal_bounds  |
+------------------+       | created_at       |       | success_criteria |
                           | updated_at       |       | created_at       |
                           +------------------+       +------------------+
                                    |
                                    |
                           +--------v---------+
                           |      tasks       |
                           |------------------|
                           | id (PK)          |
                           | swarm_id (FK)    |
                           | agent_id (FK)    |
                           | parent_task_id   |
                           | status           |
                           | priority         |
                           | input            |
                           | output           |
                           | error            |
                           | created_at       |
                           | started_at       |
                           | completed_at     |
                           +------------------+
                                    |
                +-------------------+-------------------+
                |                                       |
       +--------v---------+                    +--------v---------+
       |   task_edges     |                    |     events       |
       |------------------|                    |------------------|
       | id (PK)          |                    | id (PK)          |
       | dag_id (FK)      |                    | entity_type      |
       | from_task_id(FK) |                    | entity_id        |
       | to_task_id (FK)  |                    | event_type       |
       | edge_type        |                    | payload          |
       +------------------+                    | created_at       |
                                               +------------------+
```

### 7.2 Core Tables

#### Tasks Table

```sql
CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_id UUID NOT NULL REFERENCES swarms(id),
    agent_id UUID REFERENCES agents(id),
    parent_task_id UUID REFERENCES tasks(id),
    dag_id UUID,  -- Groups tasks in same DAG

    -- Task Definition
    task_type VARCHAR(255) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 2,  -- 1=low, 2=normal, 3=high, 4=critical
    input JSONB NOT NULL,

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    progress_percent REAL DEFAULT 0,
    current_step VARCHAR(255),

    -- Results
    output JSONB,
    error JSONB,

    -- Resource Tracking
    tokens_used BIGINT DEFAULT 0,
    cost_microdollars BIGINT DEFAULT 0,

    -- Retry Tracking
    attempt_count INTEGER DEFAULT 0,
    max_attempts INTEGER DEFAULT 3,
    last_error TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scheduled_for TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    deadline TIMESTAMPTZ,

    -- Locking for queue
    locked_by VARCHAR(255),
    locked_at TIMESTAMPTZ,

    CONSTRAINT valid_status CHECK (status IN (
        'pending', 'queued', 'running', 'completed',
        'failed', 'cancelled', 'timed_out', 'retrying'
    ))
);

-- Indexes for efficient queue operations
CREATE INDEX idx_tasks_queue ON tasks (status, priority DESC, created_at)
    WHERE status IN ('pending', 'queued');
CREATE INDEX idx_tasks_swarm ON tasks (swarm_id, status);
CREATE INDEX idx_tasks_agent ON tasks (agent_id) WHERE agent_id IS NOT NULL;
CREATE INDEX idx_tasks_dag ON tasks (dag_id) WHERE dag_id IS NOT NULL;
```

#### Agents Table

```sql
CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    swarm_id UUID NOT NULL REFERENCES swarms(id),
    contract_id UUID NOT NULL REFERENCES contracts(id),
    parent_agent_id UUID REFERENCES agents(id),

    -- Agent Identity
    agent_type VARCHAR(255) NOT NULL,
    name VARCHAR(255),

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    health VARCHAR(50) NOT NULL DEFAULT 'unknown',

    -- Runtime Info
    worker_id VARCHAR(255),
    last_heartbeat TIMESTAMPTZ,

    -- Resource Usage
    tokens_used BIGINT DEFAULT 0,
    cost_microdollars BIGINT DEFAULT 0,
    api_calls INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,

    -- Metadata
    metadata JSONB DEFAULT '{}',

    CONSTRAINT valid_status CHECK (status IN (
        'pending', 'initializing', 'running', 'paused',
        'completed', 'failed', 'terminated'
    )),
    CONSTRAINT valid_health CHECK (health IN (
        'unknown', 'healthy', 'degraded', 'unhealthy', 'dead'
    ))
);

CREATE INDEX idx_agents_swarm ON agents (swarm_id, status);
CREATE INDEX idx_agents_heartbeat ON agents (last_heartbeat)
    WHERE status = 'running';
```

#### Contracts Table

```sql
CREATE TABLE contracts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID REFERENCES agents(id),
    parent_contract_id UUID REFERENCES contracts(id),

    -- Specifications
    input_spec JSONB NOT NULL,
    output_spec JSONB NOT NULL,

    -- Resource Limits
    max_input_tokens BIGINT NOT NULL,
    max_output_tokens BIGINT NOT NULL,
    total_token_budget BIGINT NOT NULL,
    max_cost_microdollars BIGINT NOT NULL,
    max_llm_calls INTEGER NOT NULL,
    max_external_api_calls INTEGER NOT NULL,
    max_tool_calls INTEGER NOT NULL,
    max_wall_time_seconds INTEGER NOT NULL,

    -- Temporal Bounds
    not_before TIMESTAMPTZ,
    deadline TIMESTAMPTZ NOT NULL,
    soft_deadline TIMESTAMPTZ,
    heartbeat_interval_seconds INTEGER NOT NULL DEFAULT 5,
    heartbeat_timeout_seconds INTEGER NOT NULL DEFAULT 15,

    -- Success Criteria
    success_criteria JSONB NOT NULL,
    failure_modes JSONB NOT NULL DEFAULT '[]',

    -- Current Usage (updated atomically)
    tokens_used BIGINT DEFAULT 0,
    cost_used_microdollars BIGINT DEFAULT 0,
    llm_calls_made INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    violation_reason TEXT,

    CONSTRAINT valid_status CHECK (status IN (
        'active', 'completed', 'violated', 'expired'
    ))
);

CREATE INDEX idx_contracts_agent ON contracts (agent_id);
CREATE INDEX idx_contracts_parent ON contracts (parent_contract_id);
```

#### Events Table

```sql
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Entity Reference
    entity_type VARCHAR(50) NOT NULL,  -- 'swarm', 'agent', 'task', 'contract'
    entity_id UUID NOT NULL,

    -- Event Info
    event_type VARCHAR(100) NOT NULL,
    severity VARCHAR(20) NOT NULL DEFAULT 'info',

    -- Payload
    payload JSONB NOT NULL DEFAULT '{}',

    -- Timestamp
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT valid_entity_type CHECK (entity_type IN (
        'swarm', 'agent', 'task', 'contract'
    )),
    CONSTRAINT valid_severity CHECK (severity IN (
        'debug', 'info', 'warning', 'error', 'critical'
    ))
);

-- Partitioned by time for efficient cleanup
CREATE INDEX idx_events_entity ON events (entity_type, entity_id, created_at DESC);
CREATE INDEX idx_events_type ON events (event_type, created_at DESC);
```

---

## 8. Rust Project Structure

```
apex-core/
├── Cargo.toml
├── Cargo.lock
├── build.rs                    # Proto compilation
├── .env.example
│
├── proto/
│   └── apex/
│       └── v1/
│           ├── common.proto
│           ├── swarm.proto
│           ├── agent.proto
│           ├── task.proto
│           └── contract.proto
│
├── src/
│   ├── main.rs                 # Entry point
│   ├── lib.rs                  # Library root
│   ├── config.rs               # Configuration management
│   ├── error.rs                # Error types
│   │
│   ├── orchestrator/
│   │   ├── mod.rs
│   │   ├── swarm.rs            # SwarmOrchestrator
│   │   ├── lifecycle.rs        # Swarm lifecycle management
│   │   └── metrics.rs          # Metrics collection
│   │
│   ├── dag/
│   │   ├── mod.rs
│   │   ├── graph.rs            # TaskDAG implementation
│   │   ├── executor.rs         # DAG executor
│   │   ├── scheduler.rs        # Task scheduling
│   │   ├── topology.rs         # Topological sort, cycle detection
│   │   └── failure.rs          # Cascading cancellation
│   │
│   ├── contracts/
│   │   ├── mod.rs
│   │   ├── contract.rs         # AgentContract
│   │   ├── guard.rs            # Contract enforcement
│   │   ├── budget.rs           # Budget tracking
│   │   ├── conservation.rs     # Conservation law
│   │   └── validation.rs       # Input/output validation
│   │
│   ├── cnp/
│   │   ├── mod.rs
│   │   ├── announcement.rs     # Task announcements
│   │   ├── bidding.rs          # Bid submission/evaluation
│   │   ├── award.rs            # Award selection
│   │   ├── heartbeat.rs        # Heartbeat monitoring
│   │   └── recovery.rs         # Failure recovery
│   │
│   ├── routing/
│   │   ├── mod.rs
│   │   ├── router.rs           # ModelRouter
│   │   ├── cascade.rs          # Model cascade logic
│   │   ├── scoring.rs          # Quality scoring
│   │   ├── cost.rs             # Cost tracking
│   │   └── providers/
│   │       ├── mod.rs
│   │       ├── openai.rs
│   │       ├── anthropic.rs
│   │       └── local.rs
│   │
│   ├── workers/
│   │   ├── mod.rs
│   │   ├── pool.rs             # WorkerPool
│   │   ├── worker.rs           # Individual worker
│   │   └── python_bridge.rs    # Python agent execution
│   │
│   ├── queue/
│   │   ├── mod.rs
│   │   ├── postgres.rs         # PostgreSQL queue
│   │   └── priority.rs         # Priority queue logic
│   │
│   ├── api/
│   │   ├── mod.rs
│   │   ├── server.rs           # gRPC server setup
│   │   ├── swarm_service.rs
│   │   ├── agent_service.rs
│   │   ├── task_service.rs
│   │   └── contract_service.rs
│   │
│   └── db/
│       ├── mod.rs
│       ├── pool.rs             # Connection pooling
│       ├── migrations/
│       └── queries/
│           ├── tasks.rs
│           ├── agents.rs
│           ├── contracts.rs
│           └── events.rs
│
├── tests/
│   ├── integration/
│   │   ├── dag_tests.rs
│   │   ├── contract_tests.rs
│   │   └── routing_tests.rs
│   └── common/
│       └── mod.rs
│
└── benches/
    ├── dag_benchmark.rs
    └── routing_benchmark.rs
```

### Cargo.toml

```toml
[package]
name = "apex-core"
version = "0.1.0"
edition = "2021"
authors = ["Apex Team"]
description = "High-performance agent swarm orchestration engine"

[dependencies]
# Async Runtime
tokio = { version = "1.35", features = ["full", "tracing"] }
tokio-stream = "0.1"

# gRPC
tonic = "0.10"
prost = "0.12"
prost-types = "0.12"

# Database
sqlx = { version = "0.7", features = [
    "runtime-tokio",
    "tls-rustls",
    "postgres",
    "uuid",
    "chrono",
    "json"
]}

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Logging & Tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Utilities
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
async-trait = "0.1"
futures = "0.3"
dashmap = "5.5"
parking_lot = "0.12"

# Configuration
config = "0.13"
dotenvy = "0.15"

# Metrics
metrics = "0.22"
metrics-exporter-prometheus = "0.13"

# Python Integration
pyo3 = { version = "0.20", features = ["auto-initialize"] }

[build-dependencies]
tonic-build = "0.10"

[dev-dependencies]
tokio-test = "0.4"
criterion = { version = "0.5", features = ["async_tokio"] }
fake = "2.9"
testcontainers = "0.15"

[[bench]]
name = "dag_benchmark"
harness = false
```

---

## 9. Key Rust Code Skeletons

### 9.1 SwarmOrchestrator

```rust
// src/orchestrator/swarm.rs

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::contracts::AgentContract;
use crate::dag::TaskDAG;
use crate::workers::WorkerPool;
use crate::cnp::ContractNetProtocol;
use crate::routing::ModelRouter;
use crate::db::DbPool;
use crate::error::ApexResult;

/// Central orchestrator managing all swarm operations
pub struct SwarmOrchestrator {
    /// Unique swarm identifier
    swarm_id: Uuid,

    /// Database connection pool
    db: DbPool,

    /// Worker pool for agent execution
    worker_pool: Arc<WorkerPool>,

    /// Contract Net Protocol handler
    cnp: Arc<ContractNetProtocol>,

    /// Model router for FrugalGPT
    router: Arc<ModelRouter>,

    /// Active DAGs
    active_dags: RwLock<Vec<Arc<TaskDAG>>>,

    /// Event broadcaster
    event_tx: broadcast::Sender<SwarmEvent>,

    /// Configuration
    config: SwarmConfig,

    /// Metrics collector
    metrics: SwarmMetrics,
}

impl SwarmOrchestrator {
    /// Create a new swarm orchestrator
    pub async fn new(config: SwarmConfig, db: DbPool) -> ApexResult<Self> {
        let (event_tx, _) = broadcast::channel(10000);

        let worker_pool = Arc::new(
            WorkerPool::new(config.worker_config.clone()).await?
        );

        let cnp = Arc::new(
            ContractNetProtocol::new(config.cnp_config.clone())
        );

        let router = Arc::new(
            ModelRouter::new(config.routing_config.clone()).await?
        );

        Ok(Self {
            swarm_id: Uuid::new_v4(),
            db,
            worker_pool,
            cnp,
            router,
            active_dags: RwLock::new(Vec::new()),
            event_tx,
            config,
            metrics: SwarmMetrics::new(),
        })
    }

    /// Spawn a new agent with the given contract
    pub async fn spawn_agent(
        &self,
        contract: AgentContract,
        parent_id: Option<Uuid>,
    ) -> ApexResult<Uuid> {
        // Validate contract
        contract.validate()?;

        // Check conservation law if parent exists
        if let Some(parent_id) = parent_id {
            self.verify_conservation_law(parent_id, &contract).await?;
        }

        // Persist agent to database
        let agent_id = self.db.create_agent(&contract, parent_id).await?;

        // Submit to worker pool
        self.worker_pool.submit(agent_id, contract.clone()).await?;

        // Emit event
        self.emit_event(SwarmEvent::AgentSpawned {
            agent_id,
            contract_id: contract.contract_id,
        });

        Ok(agent_id)
    }

    /// Submit a DAG of tasks for execution
    pub async fn submit_dag(&self, dag: TaskDAG) -> ApexResult<Uuid> {
        // Validate DAG structure
        dag.validate()?;

        // Detect cycles
        if dag.has_cycle() {
            return Err(ApexError::CycleDetected);
        }

        // Persist DAG
        let dag_id = self.db.create_dag(&dag).await?;

        // Get initial tasks (no dependencies)
        let ready_tasks = dag.get_ready_tasks();

        // Schedule initial tasks via CNP
        for task in ready_tasks {
            self.cnp.announce_task(task).await?;
        }

        // Track active DAG
        let dag = Arc::new(dag);
        self.active_dags.write().await.push(dag.clone());

        Ok(dag_id)
    }

    /// Handle task completion and trigger dependents
    pub async fn on_task_complete(&self, task_id: Uuid, result: TaskResult) -> ApexResult<()> {
        // Update task in database
        self.db.complete_task(task_id, &result).await?;

        // Find DAG containing this task
        let dags = self.active_dags.read().await;
        if let Some(dag) = dags.iter().find(|d| d.contains_task(task_id)) {
            // Get newly ready tasks
            let ready = dag.mark_complete(task_id)?;

            // Schedule ready tasks
            for task in ready {
                self.cnp.announce_task(task).await?;
            }

            // Check if DAG is complete
            if dag.is_complete() {
                self.emit_event(SwarmEvent::DagComplete { dag_id: dag.id });
            }
        }

        Ok(())
    }

    /// Cancel a task and all its dependents
    pub async fn cancel_task(&self, task_id: Uuid) -> ApexResult<()> {
        let dags = self.active_dags.read().await;
        if let Some(dag) = dags.iter().find(|d| d.contains_task(task_id)) {
            // Get all descendant tasks
            let to_cancel = dag.get_descendants(task_id);

            // Cancel each task
            for tid in to_cancel {
                self.db.cancel_task(tid).await?;
                self.worker_pool.cancel(tid).await?;
            }
        }

        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> ApexResult<()> {
        tracing::info!("Initiating swarm shutdown");

        // Stop accepting new tasks
        self.cnp.stop_announcements().await;

        // Wait for active tasks with timeout
        self.worker_pool.shutdown_graceful(self.config.shutdown_timeout).await?;

        // Persist final state
        self.db.mark_swarm_shutdown(self.swarm_id).await?;

        Ok(())
    }

    fn emit_event(&self, event: SwarmEvent) {
        let _ = self.event_tx.send(event);
    }

    async fn verify_conservation_law(
        &self,
        parent_id: Uuid,
        child_contract: &AgentContract,
    ) -> ApexResult<()> {
        let parent_contract = self.db.get_contract(parent_id).await?;
        let parent_remaining = parent_contract.remaining_budget();
        let overhead = parent_contract.calculate_overhead();

        if child_contract.resource_limits > parent_remaining.subtract(&overhead)? {
            return Err(ApexError::ConservationViolation {
                parent_remaining,
                child_requested: child_contract.resource_limits.clone(),
            });
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum SwarmEvent {
    AgentSpawned { agent_id: Uuid, contract_id: Uuid },
    AgentCompleted { agent_id: Uuid },
    AgentFailed { agent_id: Uuid, error: String },
    TaskScheduled { task_id: Uuid },
    TaskStarted { task_id: Uuid, agent_id: Uuid },
    TaskCompleted { task_id: Uuid },
    TaskFailed { task_id: Uuid, error: String },
    DagComplete { dag_id: Uuid },
    BudgetWarning { agent_id: Uuid, percent_used: f64 },
    ContractViolation { contract_id: Uuid, reason: String },
}
```

### 9.2 TaskDAG

```rust
// src/dag/graph.rs

use std::collections::{HashMap, HashSet, VecDeque};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::error::{ApexResult, ApexError};

/// Directed Acyclic Graph for task dependencies
pub struct TaskDAG {
    /// DAG identifier
    pub id: Uuid,

    /// All tasks in the DAG
    nodes: RwLock<HashMap<Uuid, TaskNode>>,

    /// Adjacency list: task_id -> dependent task_ids
    edges: RwLock<HashMap<Uuid, HashSet<Uuid>>>,

    /// Reverse adjacency: task_id -> dependency task_ids
    reverse_edges: RwLock<HashMap<Uuid, HashSet<Uuid>>>,

    /// Task completion status
    completed: RwLock<HashSet<Uuid>>,

    /// Total budget for entire DAG
    pub total_budget: ResourceLimits,
}

#[derive(Clone, Debug)]
pub struct TaskNode {
    pub id: Uuid,
    pub task: TaskDescriptor,
    pub status: TaskStatus,
    pub in_degree: usize,
}

impl TaskDAG {
    /// Create a new empty DAG
    pub fn new(total_budget: ResourceLimits) -> Self {
        Self {
            id: Uuid::new_v4(),
            nodes: RwLock::new(HashMap::new()),
            edges: RwLock::new(HashMap::new()),
            reverse_edges: RwLock::new(HashMap::new()),
            completed: RwLock::new(HashSet::new()),
            total_budget,
        }
    }

    /// Add a task node to the DAG
    pub fn add_task(&self, task: TaskDescriptor) -> Uuid {
        let id = Uuid::new_v4();
        let node = TaskNode {
            id,
            task,
            status: TaskStatus::Pending,
            in_degree: 0,
        };

        self.nodes.write().insert(id, node);
        self.edges.write().insert(id, HashSet::new());
        self.reverse_edges.write().insert(id, HashSet::new());

        id
    }

    /// Add a dependency edge: `from` must complete before `to` can start
    pub fn add_dependency(&self, from: Uuid, to: Uuid) -> ApexResult<()> {
        // Verify both nodes exist
        let nodes = self.nodes.read();
        if !nodes.contains_key(&from) || !nodes.contains_key(&to) {
            return Err(ApexError::InvalidNode);
        }
        drop(nodes);

        // Add edge
        self.edges.write().get_mut(&from).unwrap().insert(to);
        self.reverse_edges.write().get_mut(&to).unwrap().insert(from);

        // Increment in-degree of target
        self.nodes.write().get_mut(&to).unwrap().in_degree += 1;

        Ok(())
    }

    /// Check if DAG contains a cycle using DFS
    pub fn has_cycle(&self) -> bool {
        let nodes = self.nodes.read();
        let edges = self.edges.read();

        let mut white: HashSet<Uuid> = nodes.keys().copied().collect();
        let mut gray: HashSet<Uuid> = HashSet::new();
        let mut black: HashSet<Uuid> = HashSet::new();

        fn dfs(
            node: Uuid,
            edges: &HashMap<Uuid, HashSet<Uuid>>,
            white: &mut HashSet<Uuid>,
            gray: &mut HashSet<Uuid>,
            black: &mut HashSet<Uuid>,
        ) -> bool {
            white.remove(&node);
            gray.insert(node);

            if let Some(neighbors) = edges.get(&node) {
                for &neighbor in neighbors {
                    if gray.contains(&neighbor) {
                        return true; // Back edge found = cycle
                    }
                    if white.contains(&neighbor) && dfs(neighbor, edges, white, gray, black) {
                        return true;
                    }
                }
            }

            gray.remove(&node);
            black.insert(node);
            false
        }

        while let Some(&start) = white.iter().next() {
            if dfs(start, &edges, &mut white, &mut gray, &mut black) {
                return true;
            }
        }

        false
    }

    /// Get topological ordering of tasks
    pub fn topological_sort(&self) -> ApexResult<Vec<Uuid>> {
        let nodes = self.nodes.read();
        let edges = self.edges.read();

        let mut in_degree: HashMap<Uuid, usize> = nodes
            .iter()
            .map(|(id, node)| (*id, node.in_degree))
            .collect();

        let mut queue: VecDeque<Uuid> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut result = Vec::with_capacity(nodes.len());

        while let Some(node) = queue.pop_front() {
            result.push(node);

            if let Some(neighbors) = edges.get(&node) {
                for &neighbor in neighbors {
                    let degree = in_degree.get_mut(&neighbor).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        if result.len() != nodes.len() {
            return Err(ApexError::CycleDetected);
        }

        Ok(result)
    }

    /// Get all tasks ready to execute (no pending dependencies)
    pub fn get_ready_tasks(&self) -> Vec<TaskDescriptor> {
        let nodes = self.nodes.read();
        let completed = self.completed.read();
        let reverse = self.reverse_edges.read();

        nodes
            .values()
            .filter(|node| {
                node.status == TaskStatus::Pending &&
                reverse.get(&node.id)
                    .map(|deps| deps.iter().all(|d| completed.contains(d)))
                    .unwrap_or(true)
            })
            .map(|node| node.task.clone())
            .collect()
    }

    /// Mark a task as complete and return newly ready tasks
    pub fn mark_complete(&self, task_id: Uuid) -> ApexResult<Vec<TaskDescriptor>> {
        self.completed.write().insert(task_id);
        self.nodes.write().get_mut(&task_id)
            .ok_or(ApexError::InvalidNode)?
            .status = TaskStatus::Completed;

        Ok(self.get_ready_tasks())
    }

    /// Get all descendant tasks (for cascading cancellation)
    pub fn get_descendants(&self, task_id: Uuid) -> Vec<Uuid> {
        let edges = self.edges.read();
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(task_id);

        while let Some(current) = queue.pop_front() {
            if visited.insert(current) {
                if current != task_id {
                    result.push(current);
                }
                if let Some(children) = edges.get(&current) {
                    queue.extend(children.iter().copied());
                }
            }
        }

        result
    }

    /// Check if all tasks are complete
    pub fn is_complete(&self) -> bool {
        let nodes = self.nodes.read();
        let completed = self.completed.read();
        nodes.len() == completed.len()
    }

    /// Check if DAG contains a task
    pub fn contains_task(&self, task_id: Uuid) -> bool {
        self.nodes.read().contains_key(&task_id)
    }

    /// Validate DAG structure
    pub fn validate(&self) -> ApexResult<()> {
        if self.has_cycle() {
            return Err(ApexError::CycleDetected);
        }

        // Ensure all nodes have valid tasks
        for node in self.nodes.read().values() {
            node.task.validate()?;
        }

        Ok(())
    }
}
```

### 9.3 AgentContract

```rust
// src/contracts/contract.rs

use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{ApexResult, ApexError};

/// Complete contract defining agent capabilities and constraints
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentContract {
    // === Identity ===
    pub contract_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub parent_contract_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,

    // === Specifications ===
    pub input_spec: InputSpec,
    pub output_spec: OutputSpec,

    // === Resource Limits ===
    pub resource_limits: ResourceLimits,

    // === Temporal Bounds ===
    pub temporal_bounds: TemporalBounds,

    // === Success/Failure Criteria ===
    pub success_criteria: SuccessCriteria,
    pub failure_modes: Vec<FailureMode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputSpec {
    pub schema: serde_json::Value,
    pub required_fields: Vec<String>,
    pub optional_fields: HashMap<String, serde_json::Value>,
    pub max_size_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputSpec {
    pub schema: serde_json::Value,
    pub required_fields: Vec<String>,
    pub max_size_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ResourceLimits {
    // Token limits
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub total_token_budget: u64,

    // Cost limits (micro-dollars)
    pub max_cost_microdollars: u64,

    // API call limits
    pub max_llm_calls: u32,
    pub max_external_api_calls: u32,
    pub max_tool_calls: u32,

    // Compute limits
    pub max_cpu_time_ms: u64,
    pub max_memory_bytes: u64,
    pub max_wall_time: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporalBounds {
    pub not_before: Option<DateTime<Utc>>,
    pub deadline: DateTime<Utc>,
    pub soft_deadline: Option<DateTime<Utc>>,
    pub max_duration: Duration,
    pub heartbeat_interval: Duration,
    pub heartbeat_timeout: Duration,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SuccessCriteria {
    pub output_schema_valid: bool,
    pub min_confidence: Option<f64>,
    pub custom_validator: Option<String>,
    pub required_non_null: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailureMode {
    pub failure_type: FailureType,
    pub retryable: bool,
    pub max_retries: u32,
    pub fallback: FallbackAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FailureType {
    Timeout,
    TokenLimitExceeded,
    CostLimitExceeded,
    ApiCallLimitExceeded,
    OutputValidationFailed,
    ExternalServiceError,
    AgentCrash,
    ContractViolation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FallbackAction {
    Retry,
    EscalateToParent,
    UseDefaultOutput(serde_json::Value),
    CancelSubtree,
    AlertHuman,
}

impl AgentContract {
    /// Create a new contract builder
    pub fn builder() -> ContractBuilder {
        ContractBuilder::default()
    }

    /// Validate contract completeness and consistency
    pub fn validate(&self) -> ApexResult<()> {
        // Validate temporal bounds
        if self.temporal_bounds.deadline <= Utc::now() {
            return Err(ApexError::InvalidContract("Deadline already passed".into()));
        }

        if let Some(soft) = self.temporal_bounds.soft_deadline {
            if soft >= self.temporal_bounds.deadline {
                return Err(ApexError::InvalidContract(
                    "Soft deadline must be before hard deadline".into()
                ));
            }
        }

        // Validate resource limits
        if self.resource_limits.total_token_budget == 0 {
            return Err(ApexError::InvalidContract("Token budget cannot be zero".into()));
        }

        // Validate heartbeat config
        if self.temporal_bounds.heartbeat_timeout <= self.temporal_bounds.heartbeat_interval {
            return Err(ApexError::InvalidContract(
                "Heartbeat timeout must be greater than interval".into()
            ));
        }

        Ok(())
    }

    /// Calculate remaining budget based on usage
    pub fn remaining_budget(&self, usage: &ResourceUsage) -> ResourceLimits {
        ResourceLimits {
            max_input_tokens: self.resource_limits.max_input_tokens
                .saturating_sub(usage.input_tokens),
            max_output_tokens: self.resource_limits.max_output_tokens
                .saturating_sub(usage.output_tokens),
            total_token_budget: self.resource_limits.total_token_budget
                .saturating_sub(usage.total_tokens()),
            max_cost_microdollars: self.resource_limits.max_cost_microdollars
                .saturating_sub(usage.cost_microdollars),
            max_llm_calls: self.resource_limits.max_llm_calls
                .saturating_sub(usage.llm_calls),
            max_external_api_calls: self.resource_limits.max_external_api_calls
                .saturating_sub(usage.external_api_calls),
            max_tool_calls: self.resource_limits.max_tool_calls
                .saturating_sub(usage.tool_calls),
            max_cpu_time_ms: self.resource_limits.max_cpu_time_ms
                .saturating_sub(usage.cpu_time_ms),
            max_memory_bytes: self.resource_limits.max_memory_bytes,
            max_wall_time: self.resource_limits.max_wall_time
                .checked_sub(&usage.wall_time).unwrap_or_default(),
        }
    }

    /// Calculate overhead for child budget allocation (10% safety margin)
    pub fn calculate_overhead(&self) -> ResourceLimits {
        ResourceLimits {
            max_input_tokens: self.resource_limits.max_input_tokens / 10,
            max_output_tokens: self.resource_limits.max_output_tokens / 10,
            total_token_budget: self.resource_limits.total_token_budget / 10,
            max_cost_microdollars: self.resource_limits.max_cost_microdollars / 10,
            max_llm_calls: self.resource_limits.max_llm_calls / 10,
            max_external_api_calls: self.resource_limits.max_external_api_calls / 10,
            max_tool_calls: self.resource_limits.max_tool_calls / 10,
            max_cpu_time_ms: self.resource_limits.max_cpu_time_ms / 10,
            max_memory_bytes: self.resource_limits.max_memory_bytes,
            max_wall_time: self.resource_limits.max_wall_time / 10,
        }
    }

    /// Check if a child contract would violate conservation law
    pub fn can_allocate_child(
        &self,
        child_limits: &ResourceLimits,
        current_usage: &ResourceUsage
    ) -> bool {
        let remaining = self.remaining_budget(current_usage);
        let overhead = self.calculate_overhead();
        let available = remaining.subtract(&overhead);

        available.map(|a| child_limits <= &a).unwrap_or(false)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ResourceUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microdollars: u64,
    pub llm_calls: u32,
    pub external_api_calls: u32,
    pub tool_calls: u32,
    pub cpu_time_ms: u64,
    pub wall_time: Duration,
}

impl ResourceUsage {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

impl ResourceLimits {
    pub fn subtract(&self, other: &Self) -> ApexResult<Self> {
        Ok(Self {
            max_input_tokens: self.max_input_tokens
                .checked_sub(other.max_input_tokens)
                .ok_or(ApexError::BudgetExceeded)?,
            max_output_tokens: self.max_output_tokens
                .checked_sub(other.max_output_tokens)
                .ok_or(ApexError::BudgetExceeded)?,
            total_token_budget: self.total_token_budget
                .checked_sub(other.total_token_budget)
                .ok_or(ApexError::BudgetExceeded)?,
            max_cost_microdollars: self.max_cost_microdollars
                .checked_sub(other.max_cost_microdollars)
                .ok_or(ApexError::BudgetExceeded)?,
            max_llm_calls: self.max_llm_calls
                .checked_sub(other.max_llm_calls)
                .ok_or(ApexError::BudgetExceeded)?,
            max_external_api_calls: self.max_external_api_calls
                .checked_sub(other.max_external_api_calls)
                .ok_or(ApexError::BudgetExceeded)?,
            max_tool_calls: self.max_tool_calls
                .checked_sub(other.max_tool_calls)
                .ok_or(ApexError::BudgetExceeded)?,
            max_cpu_time_ms: self.max_cpu_time_ms
                .checked_sub(other.max_cpu_time_ms)
                .ok_or(ApexError::BudgetExceeded)?,
            max_memory_bytes: self.max_memory_bytes,
            max_wall_time: self.max_wall_time
                .checked_sub(&other.max_wall_time)
                .ok_or(ApexError::BudgetExceeded)?,
        })
    }
}

impl PartialOrd for ResourceLimits {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // A <= B iff all fields of A <= corresponding fields of B
        if self.max_input_tokens <= other.max_input_tokens
            && self.max_output_tokens <= other.max_output_tokens
            && self.total_token_budget <= other.total_token_budget
            && self.max_cost_microdollars <= other.max_cost_microdollars
            && self.max_llm_calls <= other.max_llm_calls
            && self.max_external_api_calls <= other.max_external_api_calls
            && self.max_tool_calls <= other.max_tool_calls
        {
            Some(std::cmp::Ordering::Less)
        } else {
            None
        }
    }
}
```

### 9.4 WorkerPool

```rust
// src/workers/pool.rs

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{mpsc, Semaphore, broadcast};
use tokio::task::JoinSet;
use uuid::Uuid;
use dashmap::DashMap;

use crate::contracts::AgentContract;
use crate::error::{ApexResult, ApexError};

/// High-performance worker pool for agent execution
pub struct WorkerPool {
    /// Configuration
    config: WorkerPoolConfig,

    /// Semaphore controlling max concurrent workers
    semaphore: Arc<Semaphore>,

    /// Task submission channel
    task_tx: mpsc::Sender<WorkerTask>,

    /// Active workers by agent ID
    active_workers: DashMap<Uuid, WorkerHandle>,

    /// Current worker count
    worker_count: AtomicUsize,

    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,

    /// Join set for worker tasks
    join_set: Arc<tokio::sync::Mutex<JoinSet<WorkerResult>>>,
}

#[derive(Clone, Debug)]
pub struct WorkerPoolConfig {
    /// Maximum concurrent agents
    pub max_workers: usize,

    /// Task queue buffer size
    pub queue_buffer: usize,

    /// Graceful shutdown timeout
    pub shutdown_timeout: std::time::Duration,

    /// Worker health check interval
    pub health_check_interval: std::time::Duration,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            max_workers: 1024,
            queue_buffer: 10000,
            shutdown_timeout: std::time::Duration::from_secs(30),
            health_check_interval: std::time::Duration::from_secs(5),
        }
    }
}

struct WorkerTask {
    agent_id: Uuid,
    contract: AgentContract,
    result_tx: mpsc::Sender<WorkerResult>,
}

struct WorkerHandle {
    agent_id: Uuid,
    cancel_tx: mpsc::Sender<()>,
    started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct WorkerResult {
    pub agent_id: Uuid,
    pub status: WorkerStatus,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone)]
pub enum WorkerStatus {
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

impl WorkerPool {
    /// Create a new worker pool
    pub async fn new(config: WorkerPoolConfig) -> ApexResult<Self> {
        let (task_tx, task_rx) = mpsc::channel(config.queue_buffer);
        let (shutdown_tx, _) = broadcast::channel(1);
        let semaphore = Arc::new(Semaphore::new(config.max_workers));

        let pool = Self {
            config: config.clone(),
            semaphore: semaphore.clone(),
            task_tx,
            active_workers: DashMap::new(),
            worker_count: AtomicUsize::new(0),
            shutdown_tx: shutdown_tx.clone(),
            join_set: Arc::new(tokio::sync::Mutex::new(JoinSet::new())),
        };

        // Start dispatcher task
        pool.start_dispatcher(task_rx, shutdown_tx.subscribe()).await;

        // Start health checker
        pool.start_health_checker().await;

        Ok(pool)
    }

    /// Submit an agent for execution
    pub async fn submit(
        &self,
        agent_id: Uuid,
        contract: AgentContract
    ) -> ApexResult<mpsc::Receiver<WorkerResult>> {
        let (result_tx, result_rx) = mpsc::channel(1);

        let task = WorkerTask {
            agent_id,
            contract,
            result_tx,
        };

        self.task_tx.send(task).await
            .map_err(|_| ApexError::PoolShutdown)?;

        Ok(result_rx)
    }

    /// Cancel a running agent
    pub async fn cancel(&self, agent_id: Uuid) -> ApexResult<()> {
        if let Some((_, handle)) = self.active_workers.remove(&agent_id) {
            let _ = handle.cancel_tx.send(()).await;
        }
        Ok(())
    }

    /// Get current worker count
    pub fn worker_count(&self) -> usize {
        self.worker_count.load(Ordering::SeqCst)
    }

    /// Get available capacity
    pub fn available_capacity(&self) -> usize {
        self.config.max_workers - self.worker_count()
    }

    /// Graceful shutdown
    pub async fn shutdown_graceful(&self, timeout: std::time::Duration) -> ApexResult<()> {
        // Signal shutdown
        let _ = self.shutdown_tx.send(());

        // Wait for all workers with timeout
        let mut join_set = self.join_set.lock().await;
        let deadline = tokio::time::Instant::now() + timeout;

        while let Ok(result) = tokio::time::timeout_at(deadline, join_set.join_next()).await {
            match result {
                Some(Ok(worker_result)) => {
                    tracing::info!(
                        agent_id = %worker_result.agent_id,
                        "Worker completed during shutdown"
                    );
                }
                Some(Err(e)) => {
                    tracing::warn!("Worker panicked during shutdown: {}", e);
                }
                None => break, // All workers done
            }
        }

        Ok(())
    }

    async fn start_dispatcher(
        &self,
        mut task_rx: mpsc::Receiver<WorkerTask>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) {
        let semaphore = self.semaphore.clone();
        let active_workers = self.active_workers.clone();
        let worker_count = Arc::new(self.worker_count);
        let join_set = self.join_set.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(task) = task_rx.recv() => {
                        // Acquire semaphore permit
                        let permit = semaphore.clone().acquire_owned().await.unwrap();
                        worker_count.fetch_add(1, Ordering::SeqCst);

                        let (cancel_tx, cancel_rx) = mpsc::channel(1);
                        let handle = WorkerHandle {
                            agent_id: task.agent_id,
                            cancel_tx,
                            started_at: chrono::Utc::now(),
                        };
                        active_workers.insert(task.agent_id, handle);

                        let active_workers_clone = active_workers.clone();
                        let worker_count_clone = worker_count.clone();

                        // Spawn worker task
                        join_set.lock().await.spawn(async move {
                            let result = Self::run_worker(
                                task.agent_id,
                                task.contract,
                                cancel_rx
                            ).await;

                            // Cleanup
                            active_workers_clone.remove(&task.agent_id);
                            worker_count_clone.fetch_sub(1, Ordering::SeqCst);
                            drop(permit);

                            // Send result
                            let _ = task.result_tx.send(result.clone()).await;
                            result
                        });
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Worker pool dispatcher shutting down");
                        break;
                    }
                }
            }
        });
    }

    async fn run_worker(
        agent_id: Uuid,
        contract: AgentContract,
        mut cancel_rx: mpsc::Receiver<()>,
    ) -> WorkerResult {
        let deadline = tokio::time::Instant::now() +
            contract.temporal_bounds.max_duration
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(3600));

        // Create contract guard for enforcement
        let guard = ContractGuard::new(contract.clone());

        tokio::select! {
            result = Self::execute_agent(agent_id, &contract, &guard) => {
                match result {
                    Ok(output) => WorkerResult {
                        agent_id,
                        status: WorkerStatus::Completed,
                        output: Some(output),
                        error: None,
                        resource_usage: guard.get_usage(),
                    },
                    Err(e) => WorkerResult {
                        agent_id,
                        status: WorkerStatus::Failed,
                        output: None,
                        error: Some(e.to_string()),
                        resource_usage: guard.get_usage(),
                    },
                }
            }
            _ = cancel_rx.recv() => {
                WorkerResult {
                    agent_id,
                    status: WorkerStatus::Cancelled,
                    output: None,
                    error: Some("Cancelled by request".into()),
                    resource_usage: guard.get_usage(),
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                WorkerResult {
                    agent_id,
                    status: WorkerStatus::TimedOut,
                    output: None,
                    error: Some("Exceeded deadline".into()),
                    resource_usage: guard.get_usage(),
                }
            }
        }
    }

    async fn execute_agent(
        agent_id: Uuid,
        contract: &AgentContract,
        guard: &ContractGuard,
    ) -> ApexResult<serde_json::Value> {
        // This would call the Python agent runner
        tracing::info!(agent_id = %agent_id, "Executing agent");

        // Simulate work
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        Ok(serde_json::json!({"status": "completed"}))
    }

    async fn start_health_checker(&self) {
        let active_workers = self.active_workers.clone();
        let interval = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;

                let now = chrono::Utc::now();
                for entry in active_workers.iter() {
                    let handle = entry.value();
                    let age = now - handle.started_at;
                    tracing::debug!(
                        agent_id = %handle.agent_id,
                        age_secs = age.num_seconds(),
                        "Worker health check"
                    );
                }
            }
        });
    }
}
```

### 9.5 ModelRouter

```rust
// src/routing/router.rs

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use async_trait::async_trait;

use crate::contracts::AgentContract;
use crate::error::ApexResult;

/// FrugalGPT-style adaptive model router
pub struct ModelRouter {
    /// Model tiers (ordered cheap to expensive)
    tiers: Vec<ModelTier>,

    /// Quality scorer
    scorer: Arc<QualityScorer>,

    /// Cost tracker
    cost_tracker: Arc<CostTracker>,

    /// Configuration
    config: RouterConfig,
}

#[derive(Clone, Debug)]
pub struct RouterConfig {
    /// Quality thresholds per tier
    pub tier_thresholds: Vec<f64>,

    /// Enable caching of responses
    pub enable_cache: bool,

    /// Maximum escalation attempts
    pub max_escalations: usize,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            tier_thresholds: vec![0.90, 0.80, 0.60],
            enable_cache: true,
            max_escalations: 3,
        }
    }
}

#[derive(Clone)]
pub struct ModelTier {
    pub name: String,
    pub models: Vec<ModelConfig>,
    pub cost_per_input_token: f64,
    pub cost_per_output_token: f64,
}

#[derive(Clone, Debug)]
pub struct ModelConfig {
    pub model_id: String,
    pub provider: Provider,
    pub max_tokens: u32,
    pub supports_functions: bool,
}

#[derive(Clone, Debug)]
pub enum Provider {
    OpenAI,
    Anthropic,
    Local,
}

#[derive(Clone, Debug)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub temperature: f64,
    pub functions: Option<Vec<Function>>,
}

#[derive(Clone, Debug)]
pub struct LlmResponse {
    pub content: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub model: String,
    pub confidence: Option<f64>,
    pub finish_reason: FinishReason,
}

#[derive(Clone, Debug)]
pub enum FinishReason {
    Stop,
    Length,
    FunctionCall,
    ContentFilter,
}

#[derive(Clone, Debug, Default)]
pub struct QualityScore {
    pub model_confidence: f64,
    pub completeness: f64,
    pub format_valid: f64,
    pub consistency: f64,
    pub task_heuristics: f64,
    pub overall: f64,
}

pub struct QualityScorer;

impl QualityScorer {
    pub fn score(&self, response: &LlmResponse, task: &TaskDescriptor) -> QualityScore {
        let model_confidence = response.confidence.unwrap_or(0.5);
        let completeness = self.score_completeness(response, task);
        let format_valid = self.score_format(response, task);
        let consistency = 1.0; // Would need multiple samples
        let task_heuristics = self.apply_heuristics(response, task);

        let overall =
            0.30 * model_confidence +
            0.25 * completeness +
            0.20 * format_valid +
            0.15 * consistency +
            0.10 * task_heuristics;

        QualityScore {
            model_confidence,
            completeness,
            format_valid,
            consistency,
            task_heuristics,
            overall,
        }
    }

    fn score_completeness(&self, response: &LlmResponse, _task: &TaskDescriptor) -> f64 {
        if response.content.is_empty() {
            return 0.0;
        }

        match response.finish_reason {
            FinishReason::Stop => 1.0,
            FinishReason::Length => 0.5,
            FinishReason::FunctionCall => 1.0,
            FinishReason::ContentFilter => 0.3,
        }
    }

    fn score_format(&self, _response: &LlmResponse, _task: &TaskDescriptor) -> f64 {
        1.0
    }

    fn apply_heuristics(&self, _response: &LlmResponse, _task: &TaskDescriptor) -> f64 {
        1.0
    }
}

pub struct CostTracker {
    total_cost_microdollars: AtomicU64,
    total_input_tokens: AtomicU64,
    total_output_tokens: AtomicU64,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            total_cost_microdollars: AtomicU64::new(0),
            total_input_tokens: AtomicU64::new(0),
            total_output_tokens: AtomicU64::new(0),
        }
    }

    pub fn record(&self, input_tokens: u64, output_tokens: u64, cost_microdollars: u64) {
        self.total_input_tokens.fetch_add(input_tokens, Ordering::SeqCst);
        self.total_output_tokens.fetch_add(output_tokens, Ordering::SeqCst);
        self.total_cost_microdollars.fetch_add(cost_microdollars, Ordering::SeqCst);
    }

    pub fn can_afford(&self, estimated_cost: u64, contract: &AgentContract) -> bool {
        let current = self.total_cost_microdollars.load(Ordering::SeqCst);
        current + estimated_cost <= contract.resource_limits.max_cost_microdollars
    }

    pub fn get_total_cost_usd(&self) -> f64 {
        self.total_cost_microdollars.load(Ordering::SeqCst) as f64 / 1_000_000.0
    }
}

impl ModelRouter {
    pub fn new(config: RouterConfig) -> Self {
        let tiers = vec![
            ModelTier {
                name: "cheap".into(),
                models: vec![
                    ModelConfig {
                        model_id: "gpt-4o-mini".into(),
                        provider: Provider::OpenAI,
                        max_tokens: 4096,
                        supports_functions: true,
                    },
                    ModelConfig {
                        model_id: "claude-3-haiku-20240307".into(),
                        provider: Provider::Anthropic,
                        max_tokens: 4096,
                        supports_functions: true,
                    },
                ],
                cost_per_input_token: 0.00000015,
                cost_per_output_token: 0.0000006,
            },
            ModelTier {
                name: "mid".into(),
                models: vec![
                    ModelConfig {
                        model_id: "gpt-4o".into(),
                        provider: Provider::OpenAI,
                        max_tokens: 4096,
                        supports_functions: true,
                    },
                    ModelConfig {
                        model_id: "claude-3-5-sonnet-20241022".into(),
                        provider: Provider::Anthropic,
                        max_tokens: 8192,
                        supports_functions: true,
                    },
                ],
                cost_per_input_token: 0.0000025,
                cost_per_output_token: 0.00001,
            },
            ModelTier {
                name: "expensive".into(),
                models: vec![
                    ModelConfig {
                        model_id: "gpt-4".into(),
                        provider: Provider::OpenAI,
                        max_tokens: 8192,
                        supports_functions: true,
                    },
                    ModelConfig {
                        model_id: "claude-3-opus-20240229".into(),
                        provider: Provider::Anthropic,
                        max_tokens: 4096,
                        supports_functions: true,
                    },
                ],
                cost_per_input_token: 0.00003,
                cost_per_output_token: 0.00006,
            },
        ];

        Self {
            tiers,
            scorer: Arc::new(QualityScorer),
            cost_tracker: Arc::new(CostTracker::new()),
            config,
        }
    }

    /// Route a request through the model cascade
    pub async fn route(
        &self,
        request: &LlmRequest,
        task: &TaskDescriptor,
        contract: &AgentContract,
    ) -> ApexResult<RoutingResult> {
        for (tier_idx, tier) in self.tiers.iter().enumerate() {
            // Check budget
            let estimated_cost = self.estimate_tier_cost(tier, request);
            if !self.cost_tracker.can_afford(estimated_cost, contract) {
                tracing::warn!(tier = tier.name, "Skipping tier due to budget constraints");
                continue;
            }

            // Execute with this tier
            let response = self.execute_tier(tier, request).await?;

            // Score quality
            let score = self.scorer.score(&response, task);

            // Check threshold
            let threshold = self.get_threshold(tier_idx, task);

            tracing::info!(
                tier = tier.name,
                score = score.overall,
                threshold = threshold,
                "Model tier result"
            );

            if score.overall >= threshold {
                // Record cost
                let actual_cost = self.calculate_cost(tier, &response);
                self.cost_tracker.record(
                    response.input_tokens,
                    response.output_tokens,
                    actual_cost,
                );

                return Ok(RoutingResult::Success {
                    response,
                    tier: tier.name.clone(),
                    score,
                    cost_microdollars: actual_cost,
                });
            }

            tracing::info!(from_tier = tier.name, "Escalating to next tier");
        }

        Ok(RoutingResult::ExhaustedTiers)
    }

    fn get_threshold(&self, tier_idx: usize, task: &TaskDescriptor) -> f64 {
        let base = self.config.tier_thresholds
            .get(tier_idx)
            .copied()
            .unwrap_or(0.5);

        base * task.quality_multiplier.unwrap_or(1.0)
    }

    fn estimate_tier_cost(&self, tier: &ModelTier, request: &LlmRequest) -> u64 {
        let estimated_input_tokens = self.estimate_input_tokens(request);
        let estimated_output_tokens = request.max_tokens as u64;

        let cost = (estimated_input_tokens as f64 * tier.cost_per_input_token) +
                   (estimated_output_tokens as f64 * tier.cost_per_output_token);

        (cost * 1_000_000.0) as u64
    }

    fn calculate_cost(&self, tier: &ModelTier, response: &LlmResponse) -> u64 {
        let cost = (response.input_tokens as f64 * tier.cost_per_input_token) +
                   (response.output_tokens as f64 * tier.cost_per_output_token);

        (cost * 1_000_000.0) as u64
    }

    fn estimate_input_tokens(&self, request: &LlmRequest) -> u64 {
        request.messages.iter()
            .map(|m| m.content.len() as u64 / 4)
            .sum()
    }

    async fn execute_tier(
        &self,
        tier: &ModelTier,
        _request: &LlmRequest
    ) -> ApexResult<LlmResponse> {
        let model = &tier.models[0];

        Ok(LlmResponse {
            content: "Response from model".into(),
            input_tokens: 100,
            output_tokens: 200,
            model: model.model_id.clone(),
            confidence: Some(0.85),
            finish_reason: FinishReason::Stop,
        })
    }
}

#[derive(Debug)]
pub enum RoutingResult {
    Success {
        response: LlmResponse,
        tier: String,
        score: QualityScore,
        cost_microdollars: u64,
    },
    ExhaustedTiers,
    BudgetExceeded,
}
```

---

## Appendix A: Error Types

```rust
// src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApexError {
    #[error("Cycle detected in task DAG")]
    CycleDetected,

    #[error("Invalid node reference")]
    InvalidNode,

    #[error("Invalid contract: {0}")]
    InvalidContract(String),

    #[error("Budget exceeded")]
    BudgetExceeded,

    #[error("Conservation law violation: parent has {parent_remaining:?}, child requested {child_requested:?}")]
    ConservationViolation {
        parent_remaining: ResourceLimits,
        child_requested: ResourceLimits,
    },

    #[error("Worker pool shutdown")]
    PoolShutdown,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type ApexResult<T> = Result<T, ApexError>;
```

---

## Appendix B: Configuration

```rust
// src/config.rs

use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct ApexConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub worker_pool: WorkerPoolConfig,
    pub routing: RouterConfig,
    pub cnp: CnpConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout: Duration,
}

#[derive(Debug, Deserialize)]
pub struct CnpConfig {
    pub bid_timeout: Duration,
    pub min_bidders: usize,
    pub heartbeat_interval: Duration,
    pub heartbeat_timeout: Duration,
}
```

---

## Performance Targets

| Metric | Target | Implementation |
|--------|--------|----------------|
| Concurrent agents | 1,000+ | tokio::Semaphore |
| Task spawn latency | <50ms | In-memory queue + async |
| DAG completion (10 tasks) | <5s | Parallel execution |
| gRPC latency (p99) | <10ms | tonic + connection pooling |
| Database query latency | <5ms | sqlx + prepared statements |
| Memory per agent | <10MB | Efficient Rust structs |
| Circuit breaker response | <1ms | Lock-free atomics |

---

## Document Metadata

- **Version**: 2.0.0
- **Last Updated**: 2025-01-29
- **Authors**: Project Apex Backend Team
- **Status**: Architecture Specification (Implementation Ready)
