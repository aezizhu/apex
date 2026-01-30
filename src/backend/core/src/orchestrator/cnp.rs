//! Contract Net Protocol (CNP) — Task bidding and allocation engine.
//!
//! Implements the FIPA Contract Net Protocol for distributed task allocation:
//! 1. **Announce** — Orchestrator publishes a task announcement with requirements.
//! 2. **Bid** — Agents evaluate the announcement and submit bids.
//! 3. **Evaluate** — Orchestrator scores bids using a weighted evaluation function.
//! 4. **Award** — Winner is notified; runner-up is kept for failover.
//! 5. **Monitor** — Heartbeat tracking with automatic failover.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{ApexError, ErrorCode, Result};

// ═══════════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════════

/// Configuration for the CNP manager.
#[derive(Debug, Clone)]
pub struct CnpConfig {
    /// Minimum number of bids before evaluation can proceed.
    pub min_bid_count: usize,

    /// Default deadline for collecting bids (seconds).
    pub default_deadline_secs: u64,

    /// Heartbeat timeout — if no heartbeat within this window, failover triggers (seconds).
    pub heartbeat_timeout_secs: u64,

    /// Heartbeat interval — how often agents should send heartbeats (seconds).
    pub heartbeat_interval_secs: u64,

    /// Bid evaluation weights.
    pub weight_cost: f64,
    pub weight_duration: f64,
    pub weight_confidence: f64,
    pub weight_capability: f64,
}

impl Default for CnpConfig {
    fn default() -> Self {
        Self {
            min_bid_count: 1,
            default_deadline_secs: 30,
            heartbeat_timeout_secs: 15,
            heartbeat_interval_secs: 5,
            weight_cost: 0.40,
            weight_duration: 0.30,
            weight_confidence: 0.20,
            weight_capability: 0.10,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Protocol Messages
// ═══════════════════════════════════════════════════════════════════════════════

/// A task announcement broadcast to all agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAnnouncement {
    /// Unique identifier for the task.
    pub task_id: String,

    /// Human-readable description of the task.
    pub description: String,

    /// Capabilities required to execute this task.
    pub requirements: Vec<String>,

    /// Deadline for bid submission (seconds from now).
    pub deadline_secs: u64,

    /// Minimum number of bids the orchestrator wants before evaluating.
    pub min_bid_count: usize,

    /// Optional metadata for the task.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// A bid submitted by an agent in response to an announcement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBid {
    /// The bidding agent's unique identifier.
    pub agent_id: String,

    /// The task this bid is for.
    pub task_id: String,

    /// Estimated cost in dollars to execute the task.
    pub estimated_cost: f64,

    /// Estimated duration in seconds to complete the task.
    pub estimated_duration: f64,

    /// Agent's confidence in successfully completing the task (0.0–1.0).
    pub confidence: f64,

    /// Capabilities the agent possesses that match the task requirements.
    pub capabilities: Vec<String>,
}

/// The result of evaluating a single bid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidScore {
    /// The original bid.
    pub bid: AgentBid,

    /// Normalized score (0.0–1.0, higher is better).
    pub score: f64,

    /// Breakdown of individual score components.
    pub breakdown: ScoreBreakdown,
}

/// Detailed breakdown of how a bid was scored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub cost_score: f64,
    pub duration_score: f64,
    pub confidence_score: f64,
    pub capability_score: f64,
}

/// Award decision after bid evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwardDecision {
    /// The task being awarded.
    pub task_id: String,

    /// The winning bid.
    pub winning_bid: BidScore,

    /// Runner-up bid for failover (if available).
    pub runner_up: Option<BidScore>,

    /// Total number of bids received.
    pub total_bids: usize,
}

// ═══════════════════════════════════════════════════════════════════════════════
// CNP Manager
// ═══════════════════════════════════════════════════════════════════════════════

/// Redis key constants for the CNP protocol.
mod keys {
    pub const ANNOUNCEMENTS_CHANNEL: &str = "apex:cnp:announcements";

    pub fn bids_queue(task_id: &str) -> String {
        format!("apex:cnp:bids:{}", task_id)
    }

    pub fn awards_channel(agent_id: &str) -> String {
        format!("apex:cnp:awards:{}", agent_id)
    }

    pub fn heartbeat_key(task_id: &str) -> String {
        format!("apex:cnp:heartbeat:{}", task_id)
    }
}

/// The Contract Net Protocol manager.
///
/// Coordinates the full CNP lifecycle: announcing tasks, collecting bids,
/// evaluating them, awarding tasks, and monitoring execution via heartbeats.
pub struct CnpManager {
    /// Redis client for pub/sub and queue operations.
    redis_client: redis::Client,

    /// Protocol configuration.
    config: CnpConfig,
}

impl CnpManager {
    /// Create a new CNP manager.
    pub fn new(redis_client: redis::Client, config: CnpConfig) -> Self {
        Self {
            redis_client,
            config,
        }
    }

    /// Create a CNP manager with default configuration.
    pub fn with_defaults(redis_client: redis::Client) -> Self {
        Self::new(redis_client, CnpConfig::default())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Step 1: Announce Task
    // ─────────────────────────────────────────────────────────────────────────

    /// Publish a task announcement to the CNP announcements channel.
    ///
    /// All listening agents will receive this announcement and decide whether to bid.
    pub async fn announce_task(&self, announcement: &TaskAnnouncement) -> Result<()> {
        let payload = serde_json::to_string(announcement)?;

        let mut conn = self.redis_client.get_multiplexed_async_connection().await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheConnectionFailed,
                "Failed to connect to Redis for CNP announcement",
                e.to_string(),
            ))?;

        redis::cmd("PUBLISH")
            .arg(keys::ANNOUNCEMENTS_CHANNEL)
            .arg(&payload)
            .query_async::<_, i64>(&mut conn)
            .await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheError,
                "Failed to publish CNP task announcement",
                e.to_string(),
            ))?;

        tracing::info!(
            task_id = %announcement.task_id,
            requirements = ?announcement.requirements,
            deadline_secs = announcement.deadline_secs,
            "Task announcement published"
        );

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Step 2: Collect Bids
    // ─────────────────────────────────────────────────────────────────────────

    /// Collect bids for a task from the per-task bid queue.
    ///
    /// Blocks up to `deadline_secs` (or the configured default), pulling bids
    /// from the Redis list `apex:cnp:bids:{task_id}`. Stops early if `min_bid_count`
    /// bids have been collected and the remaining time is exhausted.
    pub async fn collect_bids(
        &self,
        task_id: &str,
        deadline_secs: Option<u64>,
    ) -> Result<Vec<AgentBid>> {
        let deadline = deadline_secs.unwrap_or(self.config.default_deadline_secs);
        let bid_key = keys::bids_queue(task_id);
        let mut bids = Vec::new();

        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(deadline);

        while start.elapsed() < timeout {
            let remaining = timeout.saturating_sub(start.elapsed());
            let remaining_secs = remaining.as_secs().max(1);

            let mut conn = self.redis_client.get_multiplexed_async_connection().await
                .map_err(|e| ApexError::with_internal(
                    ErrorCode::CacheConnectionFailed,
                    "Failed to connect to Redis for bid collection",
                    e.to_string(),
                ))?;

            let result: Option<(String, String)> = redis::cmd("BLPOP")
                .arg(&bid_key)
                .arg(remaining_secs)
                .query_async(&mut conn)
                .await
                .map_err(|e| ApexError::with_internal(
                    ErrorCode::CacheError,
                    "Failed to read bid from Redis",
                    e.to_string(),
                ))?;

            match result {
                Some((_key, value)) => {
                    match serde_json::from_str::<AgentBid>(&value) {
                        Ok(bid) => {
                            tracing::debug!(
                                task_id = %task_id,
                                agent_id = %bid.agent_id,
                                cost = bid.estimated_cost,
                                "Bid received"
                            );
                            bids.push(bid);
                        }
                        Err(e) => {
                            tracing::warn!(
                                task_id = %task_id,
                                error = %e,
                                "Ignoring malformed bid"
                            );
                        }
                    }
                }
                None => {
                    // Timeout on BLPOP — no more bids arriving
                    break;
                }
            }
        }

        tracing::info!(
            task_id = %task_id,
            bid_count = bids.len(),
            elapsed_ms = start.elapsed().as_millis(),
            "Bid collection complete"
        );

        Ok(bids)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Step 3: Evaluate Bids
    // ─────────────────────────────────────────────────────────────────────────

    /// Evaluate and score a set of bids for a task.
    ///
    /// Scoring weights (configurable):
    /// - Cost: 40% — lower is better
    /// - Duration: 30% — lower is better
    /// - Confidence: 20% — higher is better
    /// - Capability match: 10% — higher fraction of matched requirements is better
    ///
    /// Returns bids sorted by score (highest first).
    pub fn evaluate_bids(
        &self,
        bids: &[AgentBid],
        requirements: &[String],
    ) -> Vec<BidScore> {
        if bids.is_empty() {
            return Vec::new();
        }

        // Find min/max for normalization
        let min_cost = bids.iter().map(|b| b.estimated_cost).fold(f64::INFINITY, f64::min);
        let max_cost = bids.iter().map(|b| b.estimated_cost).fold(f64::NEG_INFINITY, f64::max);
        let min_duration = bids.iter().map(|b| b.estimated_duration).fold(f64::INFINITY, f64::min);
        let max_duration = bids.iter().map(|b| b.estimated_duration).fold(f64::NEG_INFINITY, f64::max);

        let cost_range = max_cost - min_cost;
        let duration_range = max_duration - min_duration;

        let mut scored: Vec<BidScore> = bids.iter().map(|bid| {
            // Normalize cost (lower is better → invert)
            let cost_score = if cost_range > 0.0 {
                1.0 - (bid.estimated_cost - min_cost) / cost_range
            } else {
                1.0
            };

            // Normalize duration (lower is better → invert)
            let duration_score = if duration_range > 0.0 {
                1.0 - (bid.estimated_duration - min_duration) / duration_range
            } else {
                1.0
            };

            // Confidence is already 0.0–1.0
            let confidence_score = bid.confidence.clamp(0.0, 1.0);

            // Capability match: fraction of requirements the agent can satisfy
            let capability_score = if requirements.is_empty() {
                1.0
            } else {
                let matched = requirements.iter()
                    .filter(|req| bid.capabilities.iter().any(|cap| cap == *req))
                    .count();
                matched as f64 / requirements.len() as f64
            };

            let score = self.config.weight_cost * cost_score
                + self.config.weight_duration * duration_score
                + self.config.weight_confidence * confidence_score
                + self.config.weight_capability * capability_score;

            BidScore {
                bid: bid.clone(),
                score,
                breakdown: ScoreBreakdown {
                    cost_score,
                    duration_score,
                    confidence_score,
                    capability_score,
                },
            }
        }).collect();

        // Sort descending by score
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Step 4: Award Task
    // ─────────────────────────────────────────────────────────────────────────

    /// Select the winning bid and publish an award decision.
    ///
    /// The winner is notified via `apex:cnp:awards:{agent_id}`.
    /// A runner-up is kept for failover if available.
    pub async fn award_task(
        &self,
        task_id: &str,
        scored_bids: &[BidScore],
    ) -> Result<AwardDecision> {
        if scored_bids.is_empty() {
            return Err(ApexError::with_internal(
                ErrorCode::AgentNotFound,
                "No bids received for task",
                format!("Task {} received zero bids", task_id),
            ));
        }

        let winning_bid = scored_bids[0].clone();
        let runner_up = scored_bids.get(1).cloned();

        let decision = AwardDecision {
            task_id: task_id.to_string(),
            winning_bid: winning_bid.clone(),
            runner_up,
            total_bids: scored_bids.len(),
        };

        // Publish award to the winning agent's channel
        let payload = serde_json::to_string(&decision)?;
        let award_key = keys::awards_channel(&winning_bid.bid.agent_id);

        let mut conn = self.redis_client.get_multiplexed_async_connection().await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheConnectionFailed,
                "Failed to connect to Redis for award publication",
                e.to_string(),
            ))?;

        redis::cmd("RPUSH")
            .arg(&award_key)
            .arg(&payload)
            .query_async::<_, i64>(&mut conn)
            .await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheError,
                "Failed to publish award decision",
                e.to_string(),
            ))?;

        tracing::info!(
            task_id = %task_id,
            winner = %winning_bid.bid.agent_id,
            score = winning_bid.score,
            total_bids = scored_bids.len(),
            "Task awarded"
        );

        Ok(decision)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Step 5: Monitor Execution
    // ─────────────────────────────────────────────────────────────────────────

    /// Monitor task execution via heartbeat.
    ///
    /// Checks the heartbeat key for the task. If no heartbeat is found within
    /// the configured timeout, triggers failover to the runner-up agent.
    ///
    /// Returns `Ok(true)` if the heartbeat is alive, `Ok(false)` if timed out.
    pub async fn check_heartbeat(&self, task_id: &str) -> Result<bool> {
        let heartbeat_key = keys::heartbeat_key(task_id);

        let mut conn = self.redis_client.get_multiplexed_async_connection().await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheConnectionFailed,
                "Failed to connect to Redis for heartbeat check",
                e.to_string(),
            ))?;

        let exists: bool = redis::cmd("EXISTS")
            .arg(&heartbeat_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| ApexError::with_internal(
                ErrorCode::CacheError,
                "Failed to check heartbeat",
                e.to_string(),
            ))?;

        Ok(exists)
    }

    /// Execute the full monitor loop: check heartbeat and failover if needed.
    ///
    /// This will repeatedly check the heartbeat at `heartbeat_interval_secs` intervals.
    /// If the heartbeat disappears, it awards the task to the runner-up (if any).
    ///
    /// Returns `Ok(())` when the task heartbeat expires and failover has been attempted.
    pub async fn monitor_execution(
        &self,
        decision: &AwardDecision,
    ) -> Result<()> {
        let interval = Duration::from_secs(self.config.heartbeat_interval_secs);
        let max_checks = self.config.heartbeat_timeout_secs / self.config.heartbeat_interval_secs + 1;

        for _ in 0..max_checks {
            tokio::time::sleep(interval).await;

            let alive = self.check_heartbeat(&decision.task_id).await?;
            if alive {
                tracing::trace!(
                    task_id = %decision.task_id,
                    "Heartbeat OK"
                );
                return Ok(());
            }
        }

        // Heartbeat expired — attempt failover
        tracing::warn!(
            task_id = %decision.task_id,
            original_agent = %decision.winning_bid.bid.agent_id,
            "Heartbeat expired, attempting failover"
        );

        if let Some(runner_up) = &decision.runner_up {
            // Award to runner-up
            let failover_decision = AwardDecision {
                task_id: decision.task_id.clone(),
                winning_bid: runner_up.clone(),
                runner_up: None,
                total_bids: decision.total_bids,
            };

            let payload = serde_json::to_string(&failover_decision)?;
            let award_key = keys::awards_channel(&runner_up.bid.agent_id);

            let mut conn = self.redis_client.get_multiplexed_async_connection().await
                .map_err(|e| ApexError::with_internal(
                    ErrorCode::CacheConnectionFailed,
                    "Failed to connect to Redis for failover award",
                    e.to_string(),
                ))?;

            redis::cmd("RPUSH")
                .arg(&award_key)
                .arg(&payload)
                .query_async::<_, i64>(&mut conn)
                .await
                .map_err(|e| ApexError::with_internal(
                    ErrorCode::CacheError,
                    "Failed to publish failover award",
                    e.to_string(),
                ))?;

            tracing::info!(
                task_id = %decision.task_id,
                failover_agent = %runner_up.bid.agent_id,
                "Failover award published"
            );
        } else {
            tracing::error!(
                task_id = %decision.task_id,
                "No runner-up available for failover"
            );
            return Err(ApexError::with_internal(
                ErrorCode::AgentNotFound,
                "Heartbeat expired and no runner-up available for failover",
                format!("Task {} lost its agent with no failover option", decision.task_id),
            ));
        }

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Full Protocol Flow
    // ─────────────────────────────────────────────────────────────────────────

    /// Run the complete CNP flow for a single task.
    ///
    /// 1. Announce the task
    /// 2. Collect bids until deadline
    /// 3. Evaluate and score bids
    /// 4. Award to the best bidder
    ///
    /// Returns the award decision (caller is responsible for monitoring).
    pub async fn run_protocol(
        &self,
        announcement: TaskAnnouncement,
    ) -> Result<AwardDecision> {
        // Step 1: Announce
        self.announce_task(&announcement).await?;

        // Step 2: Collect bids
        let bids = self.collect_bids(
            &announcement.task_id,
            Some(announcement.deadline_secs),
        ).await?;

        if bids.len() < announcement.min_bid_count {
            return Err(ApexError::with_internal(
                ErrorCode::AgentNotFound,
                "Insufficient bids received for task",
                format!(
                    "Task {} received {} bids but requires at least {}",
                    announcement.task_id, bids.len(), announcement.min_bid_count
                ),
            ));
        }

        // Step 3: Evaluate
        let scored = self.evaluate_bids(&bids, &announcement.requirements);

        // Step 4: Award
        let decision = self.award_task(&announcement.task_id, &scored).await?;

        Ok(decision)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Display implementations
// ═══════════════════════════════════════════════════════════════════════════════

impl std::fmt::Display for AgentBid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Bid(agent={}, task={}, cost=${:.4}, duration={:.1}s, confidence={:.2})",
            self.agent_id, self.task_id, self.estimated_cost, self.estimated_duration, self.confidence
        )
    }
}

impl std::fmt::Display for AwardDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Award(task={}, winner={}, score={:.4}, bids={})",
            self.task_id, self.winning_bid.bid.agent_id, self.winning_bid.score, self.total_bids
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bid(agent_id: &str, cost: f64, duration: f64, confidence: f64, caps: Vec<&str>) -> AgentBid {
        AgentBid {
            agent_id: agent_id.to_string(),
            task_id: "task-1".to_string(),
            estimated_cost: cost,
            estimated_duration: duration,
            confidence,
            capabilities: caps.into_iter().map(String::from).collect(),
        }
    }

    fn default_config() -> CnpConfig {
        CnpConfig::default()
    }

    fn make_manager() -> CnpManager {
        // Use a dummy Redis URL — these tests don't actually connect.
        let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
        CnpManager::new(client, default_config())
    }

    // ── Bid Evaluation ──────────────────────────────────────────────────

    #[test]
    fn test_evaluate_empty_bids() {
        let mgr = make_manager();
        let scored = mgr.evaluate_bids(&[], &[]);
        assert!(scored.is_empty());
    }

    #[test]
    fn test_evaluate_single_bid_perfect_score() {
        let mgr = make_manager();
        let bid = make_bid("agent-a", 1.0, 10.0, 1.0, vec!["rust", "python"]);
        let requirements = vec!["rust".to_string(), "python".to_string()];

        let scored = mgr.evaluate_bids(&[bid], &requirements);
        assert_eq!(scored.len(), 1);

        // Single bid: cost_score=1.0, duration_score=1.0, confidence=1.0, capability=1.0
        // Weighted: 0.4*1 + 0.3*1 + 0.2*1 + 0.1*1 = 1.0
        assert!((scored[0].score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_evaluate_prefers_cheaper_bid() {
        let mgr = make_manager();
        let cheap = make_bid("cheap", 0.50, 30.0, 0.8, vec!["rust"]);
        let expensive = make_bid("expensive", 5.00, 30.0, 0.8, vec!["rust"]);
        let requirements = vec!["rust".to_string()];

        let scored = mgr.evaluate_bids(&[cheap, expensive], &requirements);
        assert_eq!(scored[0].bid.agent_id, "cheap");
        assert!(scored[0].score > scored[1].score);
    }

    #[test]
    fn test_evaluate_prefers_faster_bid() {
        let mgr = make_manager();
        let fast = make_bid("fast", 2.0, 5.0, 0.8, vec!["rust"]);
        let slow = make_bid("slow", 2.0, 120.0, 0.8, vec!["rust"]);
        let requirements = vec!["rust".to_string()];

        let scored = mgr.evaluate_bids(&[fast, slow], &requirements);
        assert_eq!(scored[0].bid.agent_id, "fast");
    }

    #[test]
    fn test_evaluate_prefers_higher_confidence() {
        let mgr = make_manager();
        let confident = make_bid("confident", 2.0, 30.0, 0.99, vec!["rust"]);
        let uncertain = make_bid("uncertain", 2.0, 30.0, 0.30, vec!["rust"]);
        let requirements = vec!["rust".to_string()];

        let scored = mgr.evaluate_bids(&[confident, uncertain], &requirements);
        assert_eq!(scored[0].bid.agent_id, "confident");
    }

    #[test]
    fn test_evaluate_capability_match_matters() {
        let mgr = make_manager();
        let full_match = make_bid("full", 2.0, 30.0, 0.8, vec!["rust", "python", "docker"]);
        let partial_match = make_bid("partial", 2.0, 30.0, 0.8, vec!["rust"]);
        let requirements = vec!["rust".to_string(), "python".to_string(), "docker".to_string()];

        let scored = mgr.evaluate_bids(&[full_match, partial_match], &requirements);
        assert_eq!(scored[0].bid.agent_id, "full");
    }

    #[test]
    fn test_evaluate_no_requirements_gives_full_capability_score() {
        let mgr = make_manager();
        let bid = make_bid("agent-a", 1.0, 10.0, 0.9, vec![]);
        let scored = mgr.evaluate_bids(&[bid], &[]);
        // capability_score should be 1.0 when there are no requirements
        assert!((scored[0].breakdown.capability_score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_evaluate_sorts_descending() {
        let mgr = make_manager();
        let bids = vec![
            make_bid("worst", 10.0, 120.0, 0.1, vec![]),
            make_bid("best", 0.5, 5.0, 0.99, vec!["rust", "python"]),
            make_bid("mid", 3.0, 60.0, 0.5, vec!["rust"]),
        ];
        let requirements = vec!["rust".to_string(), "python".to_string()];

        let scored = mgr.evaluate_bids(&bids, &requirements);
        assert_eq!(scored[0].bid.agent_id, "best");
        assert_eq!(scored[2].bid.agent_id, "worst");
        // Verify monotonically decreasing scores
        for i in 0..scored.len() - 1 {
            assert!(scored[i].score >= scored[i + 1].score);
        }
    }

    // ── Award Decision (unit-level, no Redis) ───────────────────────────

    #[test]
    fn test_award_decision_display() {
        let decision = AwardDecision {
            task_id: "task-1".to_string(),
            winning_bid: BidScore {
                bid: make_bid("winner", 1.0, 10.0, 0.95, vec!["rust"]),
                score: 0.92,
                breakdown: ScoreBreakdown {
                    cost_score: 1.0,
                    duration_score: 1.0,
                    confidence_score: 0.95,
                    capability_score: 1.0,
                },
            },
            runner_up: None,
            total_bids: 3,
        };

        let display = format!("{}", decision);
        assert!(display.contains("winner"));
        assert!(display.contains("task-1"));
    }

    // ── Config defaults ─────────────────────────────────────────────────

    #[test]
    fn test_default_config_weights_sum_to_one() {
        let cfg = CnpConfig::default();
        let total = cfg.weight_cost + cfg.weight_duration + cfg.weight_confidence + cfg.weight_capability;
        assert!((total - 1.0).abs() < 1e-9, "Weights must sum to 1.0, got {}", total);
    }

    // ── Deadline / Timeout (structural) ─────────────────────────────────

    #[test]
    fn test_announcement_serialization_roundtrip() {
        let announcement = TaskAnnouncement {
            task_id: uuid::Uuid::new_v4().to_string(),
            description: "Analyze dataset".to_string(),
            requirements: vec!["python".to_string(), "pandas".to_string()],
            deadline_secs: 30,
            min_bid_count: 2,
            metadata: serde_json::json!({"priority": "high"}),
        };

        let json = serde_json::to_string(&announcement).unwrap();
        let deserialized: TaskAnnouncement = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.task_id, announcement.task_id);
        assert_eq!(deserialized.requirements.len(), 2);
        assert_eq!(deserialized.deadline_secs, 30);
    }

    #[test]
    fn test_bid_serialization_roundtrip() {
        let bid = make_bid("agent-x", 2.5, 45.0, 0.85, vec!["rust", "wasm"]);
        let json = serde_json::to_string(&bid).unwrap();
        let deserialized: AgentBid = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.agent_id, "agent-x");
        assert!((deserialized.estimated_cost - 2.5).abs() < 1e-9);
        assert_eq!(deserialized.capabilities.len(), 2);
    }

    // ── Failover logic (structural) ─────────────────────────────────────

    #[test]
    fn test_evaluate_produces_runner_up() {
        let mgr = make_manager();
        let bids = vec![
            make_bid("first", 1.0, 10.0, 0.9, vec!["rust"]),
            make_bid("second", 2.0, 20.0, 0.8, vec!["rust"]),
        ];
        let requirements = vec!["rust".to_string()];

        let scored = mgr.evaluate_bids(&bids, &requirements);
        assert_eq!(scored.len(), 2);
        // Both scored, runner-up is index 1
        assert_eq!(scored[0].bid.agent_id, "first");
        assert_eq!(scored[1].bid.agent_id, "second");
    }
}
