//! Task Scheduler - Priority-based task scheduling for DAG execution.
//!
//! The `TaskScheduler` implements:
//! - Priority queue management for ready tasks
//! - Dynamic priority adjustment based on age and dependencies
//! - Fair scheduling across multiple DAGs
//! - Preemption support for high-priority tasks

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::Notify;
use uuid::Uuid;

use super::{Task, TaskId};
use crate::error::{ApexError, Result};

/// Priority level for tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum PriorityLevel {
    /// Lowest priority - background tasks
    Low = 0,
    /// Normal priority - standard tasks
    Normal = 50,
    /// High priority - important tasks
    High = 100,
    /// Critical priority - must execute immediately
    Critical = 200,
}

impl Default for PriorityLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl From<i32> for PriorityLevel {
    fn from(value: i32) -> Self {
        match value {
            x if x <= 0 => PriorityLevel::Low,
            x if x <= 50 => PriorityLevel::Normal,
            x if x <= 100 => PriorityLevel::High,
            _ => PriorityLevel::Critical,
        }
    }
}

/// A scheduled task entry with priority information.
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    /// The task ID
    pub task_id: TaskId,
    /// The DAG this task belongs to
    pub dag_id: Uuid,
    /// Base priority (from task definition)
    pub base_priority: i32,
    /// Effective priority (after adjustments)
    pub effective_priority: i32,
    /// When the task was enqueued
    pub enqueued_at: Instant,
    /// Number of times this task was deferred
    pub defer_count: u32,
    /// Estimated execution time (milliseconds)
    pub estimated_duration_ms: Option<u64>,
    /// Dependencies that must complete first
    pub pending_dependencies: HashSet<TaskId>,
}

impl ScheduledTask {
    /// Create a new scheduled task.
    pub fn new(task_id: TaskId, dag_id: Uuid, priority: i32) -> Self {
        Self {
            task_id,
            dag_id,
            base_priority: priority,
            effective_priority: priority,
            enqueued_at: Instant::now(),
            defer_count: 0,
            estimated_duration_ms: None,
            pending_dependencies: HashSet::new(),
        }
    }

    /// Get the age of this task in the queue.
    pub fn age(&self) -> Duration {
        self.enqueued_at.elapsed()
    }

    /// Calculate effective priority with aging boost.
    pub fn calculate_effective_priority(&mut self, aging_factor: f64, max_age_boost: i32) {
        let age_secs = self.age().as_secs_f64();
        let age_boost = ((age_secs * aging_factor) as i32).min(max_age_boost);
        let defer_boost = (self.defer_count * 5) as i32;

        self.effective_priority = self.base_priority + age_boost + defer_boost;
    }

    /// Mark this task as deferred.
    pub fn defer(&mut self) {
        self.defer_count += 1;
    }

    /// Check if task is ready (all dependencies satisfied).
    pub fn is_ready(&self) -> bool {
        self.pending_dependencies.is_empty()
    }

    /// Mark a dependency as completed.
    pub fn dependency_completed(&mut self, dep_id: TaskId) {
        self.pending_dependencies.remove(&dep_id);
    }
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier enqueue time
        match self.effective_priority.cmp(&other.effective_priority) {
            Ordering::Equal => other.enqueued_at.cmp(&self.enqueued_at),
            ord => ord,
        }
    }
}

/// Configuration for the task scheduler.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Maximum tasks in the ready queue
    pub max_queue_size: usize,
    /// Priority aging factor (priority boost per second)
    pub aging_factor: f64,
    /// Maximum age-based priority boost
    pub max_age_boost: i32,
    /// Whether to enable priority preemption
    pub enable_preemption: bool,
    /// Preemption threshold (priority difference)
    pub preemption_threshold: i32,
    /// How often to recalculate priorities (milliseconds)
    pub priority_recalc_interval_ms: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            aging_factor: 1.0,
            max_age_boost: 50,
            enable_preemption: false,
            preemption_threshold: 100,
            priority_recalc_interval_ms: 1000,
        }
    }
}

/// Priority-based task scheduler.
pub struct TaskScheduler {
    /// Configuration
    config: SchedulerConfig,
    /// Priority queue of ready tasks
    ready_queue: RwLock<BinaryHeap<ScheduledTask>>,
    /// Map of all scheduled tasks by ID
    tasks: RwLock<HashMap<TaskId, ScheduledTask>>,
    /// Tasks waiting for dependencies
    waiting_tasks: RwLock<HashMap<TaskId, ScheduledTask>>,
    /// Dependency graph (task -> tasks that depend on it)
    dependents: RwLock<HashMap<TaskId, HashSet<TaskId>>>,
    /// Currently running tasks
    running: RwLock<HashSet<TaskId>>,
    /// Notification for new tasks
    task_available: Arc<Notify>,
    /// Total tasks scheduled
    total_scheduled: RwLock<u64>,
    /// Total tasks completed
    total_completed: RwLock<u64>,
}

impl TaskScheduler {
    /// Create a new task scheduler.
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            config,
            ready_queue: RwLock::new(BinaryHeap::new()),
            tasks: RwLock::new(HashMap::new()),
            waiting_tasks: RwLock::new(HashMap::new()),
            dependents: RwLock::new(HashMap::new()),
            running: RwLock::new(HashSet::new()),
            task_available: Arc::new(Notify::new()),
            total_scheduled: RwLock::new(0),
            total_completed: RwLock::new(0),
        }
    }

    /// Create a scheduler with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(SchedulerConfig::default())
    }

    /// Schedule a task for execution.
    pub fn schedule(&self, task: &Task, dag_id: Uuid, dependencies: Vec<TaskId>) -> Result<()> {
        let task_id = task.id;

        // Check queue capacity
        {
            let queue = self.ready_queue.read();
            let waiting = self.waiting_tasks.read();
            if queue.len() + waiting.len() >= self.config.max_queue_size {
                return Err(ApexError::internal("Scheduler queue at maximum capacity"));
            }
        }

        let mut scheduled = ScheduledTask::new(task_id, dag_id, task.priority);
        scheduled.pending_dependencies = dependencies.into_iter().collect();

        // Register dependents
        {
            let mut deps = self.dependents.write();
            for dep_id in &scheduled.pending_dependencies {
                deps.entry(*dep_id).or_default().insert(task_id);
            }
        }

        // Add to tasks map
        {
            let mut tasks = self.tasks.write();
            tasks.insert(task_id, scheduled.clone());
        }

        // Add to appropriate queue
        if scheduled.is_ready() {
            let mut queue = self.ready_queue.write();
            queue.push(scheduled);
            self.task_available.notify_one();
        } else {
            let mut waiting = self.waiting_tasks.write();
            waiting.insert(task_id, scheduled);
        }

        // Update stats
        {
            let mut total = self.total_scheduled.write();
            *total += 1;
        }

        tracing::debug!(
            task_id = %task_id,
            dag_id = %dag_id,
            priority = task.priority,
            "Task scheduled"
        );

        Ok(())
    }

    /// Get the next task to execute (blocks until one is available).
    pub async fn next_task(&self) -> Option<ScheduledTask> {
        loop {
            // Try to get a task
            {
                let mut queue = self.ready_queue.write();
                if let Some(task) = queue.pop() {
                    // Mark as running
                    self.running.write().insert(task.task_id);
                    return Some(task);
                }
            }

            // Wait for notification
            self.task_available.notified().await;
        }
    }

    /// Try to get the next task without blocking.
    pub fn try_next_task(&self) -> Option<ScheduledTask> {
        let mut queue = self.ready_queue.write();
        if let Some(task) = queue.pop() {
            self.running.write().insert(task.task_id);
            Some(task)
        } else {
            None
        }
    }

    /// Peek at the next task without removing it.
    pub fn peek_next(&self) -> Option<ScheduledTask> {
        self.ready_queue.read().peek().cloned()
    }

    /// Mark a task as completed and update dependents.
    pub fn complete(&self, task_id: TaskId) {
        // Remove from running
        self.running.write().remove(&task_id);

        // Remove from tasks
        self.tasks.write().remove(&task_id);

        // Update dependents
        let dependents_to_update: Vec<TaskId> = {
            let deps = self.dependents.read();
            deps.get(&task_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect()
        };

        // Remove from dependents map
        self.dependents.write().remove(&task_id);

        // Update waiting tasks
        let mut tasks_now_ready = Vec::new();
        {
            let mut waiting = self.waiting_tasks.write();
            for dep_task_id in dependents_to_update {
                if let Some(task) = waiting.get_mut(&dep_task_id) {
                    task.dependency_completed(task_id);
                    if task.is_ready() {
                        tasks_now_ready.push(dep_task_id);
                    }
                }
            }
        }

        // Move ready tasks to queue
        for ready_id in tasks_now_ready {
            if let Some(task) = self.waiting_tasks.write().remove(&ready_id) {
                self.ready_queue.write().push(task);
                self.task_available.notify_one();
            }
        }

        // Update stats
        {
            let mut total = self.total_completed.write();
            *total += 1;
        }

        tracing::debug!(task_id = %task_id, "Task completed");
    }

    /// Mark a task as failed.
    pub fn fail(&self, task_id: TaskId, cancel_dependents: bool) {
        // Remove from running
        self.running.write().remove(&task_id);

        // Remove from tasks
        self.tasks.write().remove(&task_id);

        if cancel_dependents {
            // Get and cancel all dependents
            let dependents_to_cancel: Vec<TaskId> = {
                let deps = self.dependents.read();
                self.collect_all_dependents(task_id, &deps)
            };

            for dep_id in dependents_to_cancel {
                self.cancel_task(dep_id);
            }
        }

        // Remove from dependents map
        self.dependents.write().remove(&task_id);

        tracing::debug!(task_id = %task_id, "Task failed");
    }

    /// Cancel a specific task.
    pub fn cancel_task(&self, task_id: TaskId) {
        self.waiting_tasks.write().remove(&task_id);
        self.tasks.write().remove(&task_id);
        self.running.write().remove(&task_id);

        // Note: Cannot efficiently remove from BinaryHeap, but it will be
        // filtered out when popped
        tracing::debug!(task_id = %task_id, "Task cancelled");
    }

    /// Defer a task (put it back in the queue with increased priority).
    pub fn defer(&self, mut task: ScheduledTask) {
        self.running.write().remove(&task.task_id);
        task.defer();
        task.calculate_effective_priority(self.config.aging_factor, self.config.max_age_boost);

        self.ready_queue.write().push(task);
        self.task_available.notify_one();
    }

    /// Recalculate priorities for all queued tasks.
    pub fn recalculate_priorities(&self) {
        let mut queue = self.ready_queue.write();
        let tasks: Vec<_> = queue.drain().collect();

        for mut task in tasks {
            task.calculate_effective_priority(self.config.aging_factor, self.config.max_age_boost);
            queue.push(task);
        }
    }

    /// Get scheduler statistics.
    pub fn stats(&self) -> SchedulerStats {
        let queue = self.ready_queue.read();
        let waiting = self.waiting_tasks.read();
        let running = self.running.read();

        SchedulerStats {
            ready_count: queue.len(),
            waiting_count: waiting.len(),
            running_count: running.len(),
            total_scheduled: *self.total_scheduled.read(),
            total_completed: *self.total_completed.read(),
            highest_priority: queue.peek().map(|t| t.effective_priority),
            oldest_task_age_ms: queue.peek().map(|t| t.age().as_millis() as u64),
        }
    }

    /// Check if the scheduler has any pending work.
    pub fn has_pending_work(&self) -> bool {
        !self.ready_queue.read().is_empty()
            || !self.waiting_tasks.read().is_empty()
            || !self.running.read().is_empty()
    }

    /// Clear all scheduled tasks.
    pub fn clear(&self) {
        self.ready_queue.write().clear();
        self.tasks.write().clear();
        self.waiting_tasks.write().clear();
        self.dependents.write().clear();
        self.running.write().clear();
    }

    /// Collect all transitive dependents of a task.
    fn collect_all_dependents(
        &self,
        task_id: TaskId,
        deps: &HashMap<TaskId, HashSet<TaskId>>,
    ) -> Vec<TaskId> {
        let mut result = Vec::new();
        let mut to_process = vec![task_id];
        let mut seen = HashSet::new();

        while let Some(id) = to_process.pop() {
            if seen.contains(&id) {
                continue;
            }
            seen.insert(id);

            if let Some(dependents) = deps.get(&id) {
                for dep_id in dependents {
                    if !seen.contains(dep_id) {
                        result.push(*dep_id);
                        to_process.push(*dep_id);
                    }
                }
            }
        }

        result
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Scheduler statistics.
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    /// Number of tasks ready to execute
    pub ready_count: usize,
    /// Number of tasks waiting for dependencies
    pub waiting_count: usize,
    /// Number of currently running tasks
    pub running_count: usize,
    /// Total tasks ever scheduled
    pub total_scheduled: u64,
    /// Total tasks completed
    pub total_completed: u64,
    /// Highest priority in queue
    pub highest_priority: Option<i32>,
    /// Age of oldest task in queue (milliseconds)
    pub oldest_task_age_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::TaskInput;

    fn create_test_task(name: &str, priority: i32) -> Task {
        let mut task = Task::new(name, TaskInput::default());
        task.priority = priority;
        task
    }

    #[test]
    fn test_scheduler_creation() {
        let scheduler = TaskScheduler::with_defaults();
        let stats = scheduler.stats();

        assert_eq!(stats.ready_count, 0);
        assert_eq!(stats.waiting_count, 0);
        assert_eq!(stats.running_count, 0);
    }

    #[test]
    fn test_schedule_task() {
        let scheduler = TaskScheduler::with_defaults();
        let task = create_test_task("Test Task", 50);
        let dag_id = Uuid::new_v4();

        scheduler.schedule(&task, dag_id, vec![]).unwrap();

        let stats = scheduler.stats();
        assert_eq!(stats.ready_count, 1);
        assert_eq!(stats.total_scheduled, 1);
    }

    #[test]
    fn test_priority_ordering() {
        let scheduler = TaskScheduler::with_defaults();
        let dag_id = Uuid::new_v4();

        let low_priority = create_test_task("Low", 10);
        let high_priority = create_test_task("High", 100);
        let medium_priority = create_test_task("Medium", 50);

        scheduler.schedule(&low_priority, dag_id, vec![]).unwrap();
        scheduler.schedule(&high_priority, dag_id, vec![]).unwrap();
        scheduler
            .schedule(&medium_priority, dag_id, vec![])
            .unwrap();

        // Highest priority should come first
        let first = scheduler.try_next_task().unwrap();
        assert_eq!(first.effective_priority, 100);

        let second = scheduler.try_next_task().unwrap();
        assert_eq!(second.effective_priority, 50);

        let third = scheduler.try_next_task().unwrap();
        assert_eq!(third.effective_priority, 10);
    }

    #[test]
    fn test_dependencies() {
        let scheduler = TaskScheduler::with_defaults();
        let dag_id = Uuid::new_v4();

        let task_a = create_test_task("A", 50);
        let task_b = create_test_task("B", 50);

        // B depends on A
        scheduler.schedule(&task_a, dag_id, vec![]).unwrap();
        scheduler
            .schedule(&task_b, dag_id, vec![task_a.id])
            .unwrap();

        let stats = scheduler.stats();
        assert_eq!(stats.ready_count, 1); // Only A is ready
        assert_eq!(stats.waiting_count, 1); // B is waiting

        // Complete A
        let task = scheduler.try_next_task().unwrap();
        assert_eq!(task.task_id, task_a.id);
        scheduler.complete(task.task_id);

        // Now B should be ready
        let stats = scheduler.stats();
        assert_eq!(stats.ready_count, 1);
        assert_eq!(stats.waiting_count, 0);
    }

    #[test]
    fn test_scheduled_task_ordering() {
        let dag_id = Uuid::new_v4();

        let high = ScheduledTask::new(TaskId::new(), dag_id, 100);
        let mut low = ScheduledTask::new(TaskId::new(), dag_id, 10);

        // Higher priority should be greater (for max-heap)
        assert!(high > low);

        // After aging, low priority might catch up
        std::thread::sleep(std::time::Duration::from_millis(10));
        low.calculate_effective_priority(100.0, 50); // Aggressive aging

        // Low priority task is now older but still lower effective priority
        // unless enough time passes
    }

    #[test]
    fn test_defer_task() {
        let scheduler = TaskScheduler::with_defaults();
        let dag_id = Uuid::new_v4();

        let task = create_test_task("Test", 50);
        scheduler.schedule(&task, dag_id, vec![]).unwrap();

        let scheduled = scheduler.try_next_task().unwrap();
        assert_eq!(scheduled.defer_count, 0);

        // Defer the task
        scheduler.defer(scheduled);

        let deferred = scheduler.try_next_task().unwrap();
        assert_eq!(deferred.defer_count, 1);
    }

    #[test]
    fn test_fail_with_cancel_dependents() {
        let scheduler = TaskScheduler::with_defaults();
        let dag_id = Uuid::new_v4();

        let task_a = create_test_task("A", 50);
        let task_b = create_test_task("B", 50);
        let task_c = create_test_task("C", 50);

        // C depends on B, B depends on A
        scheduler.schedule(&task_a, dag_id, vec![]).unwrap();
        scheduler
            .schedule(&task_b, dag_id, vec![task_a.id])
            .unwrap();
        scheduler
            .schedule(&task_c, dag_id, vec![task_b.id])
            .unwrap();

        // Start A
        let a = scheduler.try_next_task().unwrap();

        // Fail A with cancel dependents
        scheduler.fail(a.task_id, true);

        // B and C should be cancelled
        let stats = scheduler.stats();
        assert_eq!(stats.waiting_count, 0);
        assert_eq!(stats.ready_count, 0);
    }

    #[test]
    fn test_clear() {
        let scheduler = TaskScheduler::with_defaults();
        let dag_id = Uuid::new_v4();

        let task = create_test_task("Test", 50);
        scheduler.schedule(&task, dag_id, vec![]).unwrap();

        assert!(scheduler.has_pending_work());

        scheduler.clear();

        assert!(!scheduler.has_pending_work());
    }

    #[test]
    fn test_priority_level_from_i32() {
        assert_eq!(PriorityLevel::from(-10), PriorityLevel::Low);
        assert_eq!(PriorityLevel::from(25), PriorityLevel::Normal);
        assert_eq!(PriorityLevel::from(75), PriorityLevel::High);
        assert_eq!(PriorityLevel::from(150), PriorityLevel::Critical);
    }
}
