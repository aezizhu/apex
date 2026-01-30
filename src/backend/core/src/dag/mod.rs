//! DAG (Directed Acyclic Graph) execution engine for task orchestration.
//!
//! This module handles:
//! - Task dependency resolution via topological sort
//! - Cycle detection
//! - Parallel execution of independent tasks
//! - Failure handling and cascading cancellation

mod task;
mod executor;
mod scheduler;

pub use task::{Task, TaskId, TaskStatus, TaskInput, TaskOutput, Artifact};
pub use executor::DagExecutor;
pub use scheduler::TaskScheduler;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::{toposort, is_cyclic_directed};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{ApexError, Result};

/// A Directed Acyclic Graph of tasks with dependencies.
#[derive(Debug, Clone)]
pub struct TaskDAG {
    /// The underlying graph structure
    graph: DiGraph<Task, ()>,

    /// Map from TaskId to graph node index for O(1) lookup
    task_index: HashMap<TaskId, NodeIndex>,

    /// Unique identifier for this DAG
    id: Uuid,

    /// Human-readable name
    name: String,

    /// Creation timestamp
    created_at: chrono::DateTime<chrono::Utc>,
}

impl TaskDAG {
    /// Create a new empty DAG.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            graph: DiGraph::new(),
            task_index: HashMap::new(),
            id: Uuid::new_v4(),
            name: name.into(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Add a task to the DAG.
    pub fn add_task(&mut self, task: Task) -> Result<TaskId> {
        let task_id = task.id;

        if self.task_index.contains_key(&task_id) {
            return Err(ApexError::task_already_exists(task_id.0));
        }

        let node_idx = self.graph.add_node(task);
        self.task_index.insert(task_id, node_idx);

        Ok(task_id)
    }

    /// Add a dependency: `from` must complete before `to` can start.
    pub fn add_dependency(&mut self, from: TaskId, to: TaskId) -> Result<()> {
        let from_idx = self.task_index.get(&from)
            .ok_or_else(|| ApexError::task_not_found(from.0))?;
        let to_idx = self.task_index.get(&to)
            .ok_or_else(|| ApexError::task_not_found(to.0))?;

        self.graph.add_edge(*from_idx, *to_idx, ());

        // Check for cycles after adding edge
        if is_cyclic_directed(&self.graph) {
            // Remove the edge we just added
            if let Some(edge) = self.graph.find_edge(*from_idx, *to_idx) {
                self.graph.remove_edge(edge);
            }
            return Err(ApexError::cycle_detected(format!(
                "Adding edge {:?} -> {:?} would create a cycle",
                from, to
            )));
        }

        Ok(())
    }

    /// Get tasks in topological order (respecting dependencies).
    pub fn topological_order(&self) -> Result<Vec<TaskId>> {
        toposort(&self.graph, None)
            .map(|nodes| {
                nodes.into_iter()
                    .map(|idx| self.graph[idx].id)
                    .collect()
            })
            .map_err(|cycle| {
                let task = &self.graph[cycle.node_id()];
                ApexError::cycle_detected(format!("Cycle involving task: {:?}", task.id))
            })
    }

    /// Get all tasks that are ready to execute (all dependencies completed).
    pub fn get_ready_tasks(&self) -> Vec<TaskId> {
        self.task_index
            .iter()
            .filter(|(_, &node_idx)| {
                let task = &self.graph[node_idx];

                // Task must be pending
                if task.status != TaskStatus::Pending {
                    return false;
                }

                // All predecessors must be completed
                self.graph
                    .neighbors_directed(node_idx, petgraph::Direction::Incoming)
                    .all(|pred_idx| {
                        self.graph[pred_idx].status == TaskStatus::Completed
                    })
            })
            .map(|(task_id, _)| *task_id)
            .collect()
    }

    /// Check if all tasks are completed.
    pub fn is_complete(&self) -> bool {
        self.graph.node_weights().all(|task| {
            matches!(task.status, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled)
        })
    }

    /// Get a task by ID.
    pub fn get_task(&self, task_id: TaskId) -> Option<&Task> {
        self.task_index.get(&task_id)
            .map(|idx| &self.graph[*idx])
    }

    /// Get a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, task_id: TaskId) -> Option<&mut Task> {
        self.task_index.get(&task_id)
            .map(|idx| &mut self.graph[*idx])
    }

    /// Update task status.
    pub fn update_task_status(&mut self, task_id: TaskId, status: TaskStatus) -> Result<()> {
        let task = self.get_task_mut(task_id)
            .ok_or_else(|| ApexError::task_not_found(task_id.0))?;

        // Validate state transition
        if !task.status.can_transition_to(&status) {
            return Err(ApexError::invalid_state_transition(&task.status, &status));
        }

        task.status = status;
        Ok(())
    }

    /// Cancel all tasks that depend on the given task (cascading cancellation).
    pub fn cancel_dependents(&mut self, task_id: TaskId) -> Result<Vec<TaskId>> {
        let node_idx = self.task_index.get(&task_id)
            .ok_or_else(|| ApexError::task_not_found(task_id.0))?;

        let mut cancelled = Vec::new();
        let mut to_cancel: Vec<NodeIndex> = self.graph
            .neighbors_directed(*node_idx, petgraph::Direction::Outgoing)
            .collect();

        while let Some(idx) = to_cancel.pop() {
            let task = &mut self.graph[idx];
            if task.status == TaskStatus::Pending {
                task.status = TaskStatus::Cancelled;
                cancelled.push(task.id);

                // Add this task's dependents to the cancellation queue
                to_cancel.extend(
                    self.graph.neighbors_directed(idx, petgraph::Direction::Outgoing)
                );
            }
        }

        Ok(cancelled)
    }

    /// Get statistics about the DAG.
    pub fn stats(&self) -> DagStats {
        let mut stats = DagStats::default();

        for task in self.graph.node_weights() {
            stats.total += 1;
            match task.status {
                TaskStatus::Pending => stats.pending += 1,
                TaskStatus::Ready => stats.ready += 1,
                TaskStatus::Running => stats.running += 1,
                TaskStatus::Completed => stats.completed += 1,
                TaskStatus::Failed => stats.failed += 1,
                TaskStatus::Cancelled => stats.cancelled += 1,
            }
        }

        stats
    }

    pub fn id(&self) -> Uuid { self.id }
    pub fn name(&self) -> &str { &self.name }
    pub fn created_at(&self) -> chrono::DateTime<chrono::Utc> { self.created_at }
}

#[derive(Debug, Default, Clone)]
pub struct DagStats {
    pub total: usize,
    pub pending: usize,
    pub ready: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_creation() {
        let dag = TaskDAG::new("test-dag");
        assert!(dag.is_complete()); // Empty DAG is complete
    }

    #[test]
    fn test_cycle_detection() {
        let mut dag = TaskDAG::new("test-dag");

        let task_a = Task::new("Task A", TaskInput::default());
        let task_b = Task::new("Task B", TaskInput::default());
        let task_c = Task::new("Task C", TaskInput::default());

        let id_a = dag.add_task(task_a).unwrap();
        let id_b = dag.add_task(task_b).unwrap();
        let id_c = dag.add_task(task_c).unwrap();

        // A -> B -> C (valid)
        dag.add_dependency(id_a, id_b).unwrap();
        dag.add_dependency(id_b, id_c).unwrap();

        // C -> A would create cycle (invalid)
        let result = dag.add_dependency(id_c, id_a);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), crate::error::ErrorCode::DagCycleDetected);
    }

    #[test]
    fn test_topological_order() {
        let mut dag = TaskDAG::new("test-dag");

        let task_a = Task::new("Task A", TaskInput::default());
        let task_b = Task::new("Task B", TaskInput::default());
        let task_c = Task::new("Task C", TaskInput::default());

        let id_a = dag.add_task(task_a).unwrap();
        let id_b = dag.add_task(task_b).unwrap();
        let id_c = dag.add_task(task_c).unwrap();

        dag.add_dependency(id_a, id_b).unwrap();
        dag.add_dependency(id_b, id_c).unwrap();

        let order = dag.topological_order().unwrap();

        // A must come before B, B must come before C
        let pos_a = order.iter().position(|id| *id == id_a).unwrap();
        let pos_b = order.iter().position(|id| *id == id_b).unwrap();
        let pos_c = order.iter().position(|id| *id == id_c).unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }
}
