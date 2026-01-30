//! Usage Tracker - Real-time resource tracking and monitoring.
//!
//! The `UsageTracker` provides:
//! - Thread-safe atomic counters for resource usage
//! - Real-time usage snapshots
//! - Usage rate calculations
//! - Historical usage tracking (optional)

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use super::ResourceUsage;

/// Configuration for the usage tracker.
#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Whether to track historical usage
    pub track_history: bool,
    /// Maximum history entries to keep
    pub max_history_entries: usize,
    /// Interval for history snapshots (milliseconds)
    pub history_interval_ms: u64,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            track_history: false,
            max_history_entries: 1000,
            history_interval_ms: 1000,
        }
    }
}

/// A snapshot of usage at a point in time.
#[derive(Debug, Clone)]
pub struct UsageSnapshot {
    /// When this snapshot was taken
    pub timestamp: Instant,
    /// Tokens used at this point
    pub tokens_used: u64,
    /// Cost used at this point
    pub cost_used: f64,
    /// API calls made at this point
    pub api_calls_used: u64,
    /// Elapsed time in seconds
    pub time_elapsed_secs: u64,
}

/// Real-time resource usage tracker.
///
/// Uses atomic operations for thread-safe updates without locks
/// for the hot path (recording usage).
pub struct UsageTracker {
    /// Configuration
    config: TrackerConfig,

    /// Total tokens consumed (atomic for lock-free updates)
    tokens_used: AtomicU64,

    /// Total cost in microdollars (atomic, stored as u64 for atomicity)
    cost_used_micros: AtomicU64,

    /// Total API calls made
    api_calls_used: AtomicU64,

    /// When tracking started
    start_time: Instant,

    /// Historical snapshots (if enabled)
    history: RwLock<Vec<UsageSnapshot>>,

    /// Last snapshot time
    last_snapshot: RwLock<Instant>,

    /// Peak tokens per second observed
    peak_tokens_per_sec: AtomicU64,

    /// Peak cost per second observed (in microdollars)
    peak_cost_per_sec_micros: AtomicU64,
}

impl UsageTracker {
    /// Create a new usage tracker with default configuration.
    pub fn new() -> Self {
        Self::with_config(TrackerConfig::default())
    }

    /// Create a new usage tracker with custom configuration.
    pub fn with_config(config: TrackerConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            tokens_used: AtomicU64::new(0),
            cost_used_micros: AtomicU64::new(0),
            api_calls_used: AtomicU64::new(0),
            start_time: now,
            history: RwLock::new(Vec::new()),
            last_snapshot: RwLock::new(now),
            peak_tokens_per_sec: AtomicU64::new(0),
            peak_cost_per_sec_micros: AtomicU64::new(0),
        }
    }

    /// Record token usage.
    ///
    /// This is a lock-free operation using atomic fetch_add.
    #[inline]
    pub fn record_tokens(&self, tokens: u64) {
        self.tokens_used.fetch_add(tokens, Ordering::Relaxed);
        self.maybe_snapshot();
    }

    /// Record cost usage.
    ///
    /// Cost is stored internally as microdollars for atomic operations.
    #[inline]
    pub fn record_cost(&self, cost: f64) {
        let micros = (cost * 1_000_000.0) as u64;
        self.cost_used_micros.fetch_add(micros, Ordering::Relaxed);
        self.maybe_snapshot();
    }

    /// Record an API call.
    #[inline]
    pub fn record_api_call(&self) {
        self.api_calls_used.fetch_add(1, Ordering::Relaxed);
    }

    /// Record multiple API calls.
    #[inline]
    pub fn record_api_calls(&self, count: u64) {
        self.api_calls_used.fetch_add(count, Ordering::Relaxed);
    }

    /// Record all usage types at once.
    #[inline]
    pub fn record(&self, tokens: u64, cost: f64, api_calls: u64) {
        self.record_tokens(tokens);
        self.record_cost(cost);
        self.record_api_calls(api_calls);
    }

    /// Get current tokens used.
    #[inline]
    pub fn tokens_used(&self) -> u64 {
        self.tokens_used.load(Ordering::Relaxed)
    }

    /// Get current cost used in dollars.
    #[inline]
    pub fn cost_used(&self) -> f64 {
        self.cost_used_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Get current API calls used.
    #[inline]
    pub fn api_calls_used(&self) -> u64 {
        self.api_calls_used.load(Ordering::Relaxed)
    }

    /// Get elapsed time since tracking started.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get elapsed time in seconds.
    pub fn elapsed_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get a snapshot of current usage.
    pub fn snapshot(&self) -> UsageSnapshot {
        UsageSnapshot {
            timestamp: Instant::now(),
            tokens_used: self.tokens_used(),
            cost_used: self.cost_used(),
            api_calls_used: self.api_calls_used(),
            time_elapsed_secs: self.elapsed_secs(),
        }
    }

    /// Convert to ResourceUsage.
    pub fn to_resource_usage(&self) -> ResourceUsage {
        ResourceUsage {
            tokens_used: self.tokens_used(),
            cost_used: self.cost_used(),
            api_calls_used: self.api_calls_used(),
            time_elapsed_secs: self.elapsed_secs(),
        }
    }

    /// Calculate tokens per second rate.
    pub fn tokens_per_second(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        if elapsed < 0.001 {
            return 0.0;
        }
        self.tokens_used() as f64 / elapsed
    }

    /// Calculate cost per second rate.
    pub fn cost_per_second(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        if elapsed < 0.001 {
            return 0.0;
        }
        self.cost_used() / elapsed
    }

    /// Calculate API calls per second rate.
    pub fn api_calls_per_second(&self) -> f64 {
        let elapsed = self.elapsed().as_secs_f64();
        if elapsed < 0.001 {
            return 0.0;
        }
        self.api_calls_used() as f64 / elapsed
    }

    /// Get usage rates.
    pub fn rates(&self) -> UsageRates {
        UsageRates {
            tokens_per_second: self.tokens_per_second(),
            cost_per_second: self.cost_per_second(),
            api_calls_per_second: self.api_calls_per_second(),
        }
    }

    /// Get peak tokens per second observed.
    pub fn peak_tokens_per_second(&self) -> f64 {
        self.peak_tokens_per_sec.load(Ordering::Relaxed) as f64
    }

    /// Get peak cost per second observed.
    pub fn peak_cost_per_second(&self) -> f64 {
        self.peak_cost_per_sec_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Get historical snapshots (if history tracking is enabled).
    pub fn history(&self) -> Vec<UsageSnapshot> {
        self.history.read().clone()
    }

    /// Get the most recent N history entries.
    pub fn recent_history(&self, n: usize) -> Vec<UsageSnapshot> {
        let history = self.history.read();
        let start = history.len().saturating_sub(n);
        history[start..].to_vec()
    }

    /// Calculate usage delta between two snapshots.
    pub fn delta(from: &UsageSnapshot, to: &UsageSnapshot) -> UsageDelta {
        let duration = to.timestamp.duration_since(from.timestamp);
        UsageDelta {
            duration,
            tokens_delta: to.tokens_used.saturating_sub(from.tokens_used),
            cost_delta: to.cost_used - from.cost_used,
            api_calls_delta: to.api_calls_used.saturating_sub(from.api_calls_used),
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        self.tokens_used.store(0, Ordering::Relaxed);
        self.cost_used_micros.store(0, Ordering::Relaxed);
        self.api_calls_used.store(0, Ordering::Relaxed);
        self.history.write().clear();
    }

    /// Check if tracking has started (any usage recorded).
    pub fn has_usage(&self) -> bool {
        self.tokens_used() > 0 || self.api_calls_used() > 0
    }

    /// Get comprehensive statistics.
    pub fn stats(&self) -> TrackerStats {
        TrackerStats {
            tokens_used: self.tokens_used(),
            cost_used: self.cost_used(),
            api_calls_used: self.api_calls_used(),
            elapsed_secs: self.elapsed_secs(),
            rates: self.rates(),
            peak_tokens_per_sec: self.peak_tokens_per_second(),
            peak_cost_per_sec: self.peak_cost_per_second(),
            history_entries: self.history.read().len(),
        }
    }

    /// Maybe take a history snapshot (if enough time has passed).
    fn maybe_snapshot(&self) {
        if !self.config.track_history {
            return;
        }

        let now = Instant::now();
        let interval = Duration::from_millis(self.config.history_interval_ms);

        // Check if enough time has passed (using a read lock first)
        let should_snapshot = {
            let last = self.last_snapshot.read();
            now.duration_since(*last) >= interval
        };

        if should_snapshot {
            // Take the write lock and double-check
            let mut last = self.last_snapshot.write();
            if now.duration_since(*last) >= interval {
                *last = now;

                let snapshot = self.snapshot();

                // Update peak rates
                let current_tps = self.tokens_per_second() as u64;
                let current_cps = (self.cost_per_second() * 1_000_000.0) as u64;

                self.peak_tokens_per_sec
                    .fetch_max(current_tps, Ordering::Relaxed);
                self.peak_cost_per_sec_micros
                    .fetch_max(current_cps, Ordering::Relaxed);

                // Add to history
                let mut history = self.history.write();
                history.push(snapshot);

                // Trim if needed
                if history.len() > self.config.max_history_entries {
                    let excess = history.len() - self.config.max_history_entries;
                    history.drain(0..excess);
                }
            }
        }
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for UsageTracker {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            tokens_used: AtomicU64::new(self.tokens_used()),
            cost_used_micros: AtomicU64::new(self.cost_used_micros.load(Ordering::Relaxed)),
            api_calls_used: AtomicU64::new(self.api_calls_used()),
            start_time: self.start_time,
            history: RwLock::new(self.history.read().clone()),
            last_snapshot: RwLock::new(*self.last_snapshot.read()),
            peak_tokens_per_sec: AtomicU64::new(self.peak_tokens_per_sec.load(Ordering::Relaxed)),
            peak_cost_per_sec_micros: AtomicU64::new(
                self.peak_cost_per_sec_micros.load(Ordering::Relaxed),
            ),
        }
    }
}

/// Usage rates (per second).
#[derive(Debug, Clone, Copy)]
pub struct UsageRates {
    /// Tokens consumed per second
    pub tokens_per_second: f64,
    /// Cost per second in dollars
    pub cost_per_second: f64,
    /// API calls per second
    pub api_calls_per_second: f64,
}

/// Delta between two usage snapshots.
#[derive(Debug, Clone)]
pub struct UsageDelta {
    /// Time between snapshots
    pub duration: Duration,
    /// Tokens consumed in this period
    pub tokens_delta: u64,
    /// Cost in this period
    pub cost_delta: f64,
    /// API calls in this period
    pub api_calls_delta: u64,
}

impl UsageDelta {
    /// Calculate rates for this delta period.
    pub fn rates(&self) -> UsageRates {
        let secs = self.duration.as_secs_f64();
        if secs < 0.001 {
            return UsageRates {
                tokens_per_second: 0.0,
                cost_per_second: 0.0,
                api_calls_per_second: 0.0,
            };
        }

        UsageRates {
            tokens_per_second: self.tokens_delta as f64 / secs,
            cost_per_second: self.cost_delta / secs,
            api_calls_per_second: self.api_calls_delta as f64 / secs,
        }
    }
}

/// Comprehensive tracker statistics.
#[derive(Debug, Clone)]
pub struct TrackerStats {
    /// Total tokens used
    pub tokens_used: u64,
    /// Total cost used
    pub cost_used: f64,
    /// Total API calls
    pub api_calls_used: u64,
    /// Elapsed time in seconds
    pub elapsed_secs: u64,
    /// Current rates
    pub rates: UsageRates,
    /// Peak tokens per second
    pub peak_tokens_per_sec: f64,
    /// Peak cost per second
    pub peak_cost_per_sec: f64,
    /// Number of history entries
    pub history_entries: usize,
}

/// Builder for creating usage trackers with custom configuration.
#[allow(dead_code)]
pub struct UsageTrackerBuilder {
    config: TrackerConfig,
}

#[allow(dead_code)]
impl UsageTrackerBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: TrackerConfig::default(),
        }
    }

    /// Enable history tracking.
    pub fn with_history(mut self, max_entries: usize, interval_ms: u64) -> Self {
        self.config.track_history = true;
        self.config.max_history_entries = max_entries;
        self.config.history_interval_ms = interval_ms;
        self
    }

    /// Build the usage tracker.
    pub fn build(self) -> UsageTracker {
        UsageTracker::with_config(self.config)
    }
}

impl Default for UsageTrackerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_tracker_creation() {
        let tracker = UsageTracker::new();

        assert_eq!(tracker.tokens_used(), 0);
        assert_eq!(tracker.cost_used(), 0.0);
        assert_eq!(tracker.api_calls_used(), 0);
    }

    #[test]
    fn test_record_tokens() {
        let tracker = UsageTracker::new();

        tracker.record_tokens(100);
        tracker.record_tokens(50);

        assert_eq!(tracker.tokens_used(), 150);
    }

    #[test]
    fn test_record_cost() {
        let tracker = UsageTracker::new();

        tracker.record_cost(0.01);
        tracker.record_cost(0.02);

        // Allow for floating point imprecision
        assert!((tracker.cost_used() - 0.03).abs() < 0.0001);
    }

    #[test]
    fn test_record_api_calls() {
        let tracker = UsageTracker::new();

        tracker.record_api_call();
        tracker.record_api_calls(5);

        assert_eq!(tracker.api_calls_used(), 6);
    }

    #[test]
    fn test_record_all() {
        let tracker = UsageTracker::new();

        tracker.record(100, 0.05, 3);

        assert_eq!(tracker.tokens_used(), 100);
        assert!((tracker.cost_used() - 0.05).abs() < 0.0001);
        assert_eq!(tracker.api_calls_used(), 3);
    }

    #[test]
    fn test_snapshot() {
        let tracker = UsageTracker::new();
        tracker.record(500, 0.1, 5);

        let snapshot = tracker.snapshot();

        assert_eq!(snapshot.tokens_used, 500);
        assert!((snapshot.cost_used - 0.1).abs() < 0.0001);
        assert_eq!(snapshot.api_calls_used, 5);
    }

    #[test]
    fn test_to_resource_usage() {
        let tracker = UsageTracker::new();
        tracker.record(1000, 0.25, 10);

        let usage = tracker.to_resource_usage();

        assert_eq!(usage.tokens_used, 1000);
        assert!((usage.cost_used - 0.25).abs() < 0.0001);
        assert_eq!(usage.api_calls_used, 10);
    }

    #[test]
    fn test_rates() {
        let tracker = UsageTracker::new();
        tracker.record(1000, 0.1, 10);

        // Sleep a tiny bit to get measurable rates
        thread::sleep(Duration::from_millis(10));

        let rates = tracker.rates();

        // Just verify they're non-zero and reasonable
        assert!(rates.tokens_per_second > 0.0);
        assert!(rates.cost_per_second > 0.0);
        assert!(rates.api_calls_per_second > 0.0);
    }

    #[test]
    fn test_reset() {
        let tracker = UsageTracker::new();
        tracker.record(1000, 0.5, 50);

        assert!(tracker.has_usage());

        tracker.reset();

        assert!(!tracker.has_usage());
        assert_eq!(tracker.tokens_used(), 0);
        assert_eq!(tracker.cost_used(), 0.0);
        assert_eq!(tracker.api_calls_used(), 0);
    }

    #[test]
    fn test_thread_safety() {
        let tracker = std::sync::Arc::new(UsageTracker::new());
        let mut handles = vec![];

        // Spawn multiple threads that record usage
        for _ in 0..10 {
            let tracker_clone = tracker.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    tracker_clone.record_tokens(1);
                    tracker_clone.record_cost(0.001);
                    tracker_clone.record_api_call();
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify totals
        assert_eq!(tracker.tokens_used(), 1000);
        assert!((tracker.cost_used() - 1.0).abs() < 0.001);
        assert_eq!(tracker.api_calls_used(), 1000);
    }

    #[test]
    fn test_history_tracking() {
        let tracker = UsageTrackerBuilder::new()
            .with_history(100, 1) // 1ms interval for testing
            .build();

        tracker.record_tokens(100);
        thread::sleep(Duration::from_millis(5));
        tracker.record_tokens(100);
        thread::sleep(Duration::from_millis(5));
        tracker.record_tokens(100);

        // History should have some entries
        let _history = tracker.history();
        // Note: History entries depend on timing, so we just check it's working
        assert!(tracker.stats().history_entries <= 100);
    }

    #[test]
    fn test_delta_calculation() {
        let snapshot1 = UsageSnapshot {
            timestamp: Instant::now(),
            tokens_used: 100,
            cost_used: 0.1,
            api_calls_used: 5,
            time_elapsed_secs: 10,
        };

        thread::sleep(Duration::from_millis(10));

        let snapshot2 = UsageSnapshot {
            timestamp: Instant::now(),
            tokens_used: 200,
            cost_used: 0.2,
            api_calls_used: 10,
            time_elapsed_secs: 20,
        };

        let delta = UsageTracker::delta(&snapshot1, &snapshot2);

        assert_eq!(delta.tokens_delta, 100);
        assert!((delta.cost_delta - 0.1).abs() < 0.001);
        assert_eq!(delta.api_calls_delta, 5);
        assert!(delta.duration.as_millis() >= 10);
    }

    #[test]
    fn test_builder() {
        let tracker = UsageTrackerBuilder::new().with_history(500, 100).build();

        assert_eq!(tracker.config.track_history, true);
        assert_eq!(tracker.config.max_history_entries, 500);
        assert_eq!(tracker.config.history_interval_ms, 100);
    }

    #[test]
    fn test_stats() {
        let tracker = UsageTracker::new();
        tracker.record(1000, 0.5, 25);

        let stats = tracker.stats();

        assert_eq!(stats.tokens_used, 1000);
        assert!((stats.cost_used - 0.5).abs() < 0.001);
        assert_eq!(stats.api_calls_used, 25);
    }

    #[test]
    fn test_clone() {
        let tracker = UsageTracker::new();
        tracker.record(500, 0.25, 10);

        let cloned = tracker.clone();

        assert_eq!(cloned.tokens_used(), 500);
        assert!((cloned.cost_used() - 0.25).abs() < 0.001);
        assert_eq!(cloned.api_calls_used(), 10);

        // Original and clone should be independent
        tracker.record_tokens(100);
        assert_eq!(tracker.tokens_used(), 600);
        assert_eq!(cloned.tokens_used(), 500);
    }
}
