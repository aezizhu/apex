//! Integration tests for DAG execution.

use apex_core::dag::{TaskDAG, Task, TaskStatus, TaskInput};

#[test]
fn test_empty_dag_is_complete() {
    let dag = TaskDAG::new("test-dag");
    assert!(dag.is_complete());
}

#[test]
fn test_add_single_task() {
    let mut dag = TaskDAG::new("test-dag");

    let task = Task::new("Task A", TaskInput::default());
    let task_id = dag.add_task(task).unwrap();

    assert!(!dag.is_complete());

    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], task_id);
}

#[test]
fn test_linear_dependency_chain() {
    let mut dag = TaskDAG::new("test-dag");

    // Create A -> B -> C chain
    let task_a = Task::new("Task A", TaskInput::default());
    let task_b = Task::new("Task B", TaskInput::default());
    let task_c = Task::new("Task C", TaskInput::default());

    let id_a = dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();

    dag.add_dependency(id_a, id_b).unwrap();
    dag.add_dependency(id_b, id_c).unwrap();

    // Initially only A should be ready
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_a);

    // Complete A, B should become ready
    dag.update_task_status(id_a, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_a, TaskStatus::Running).unwrap();
    dag.update_task_status(id_a, TaskStatus::Completed).unwrap();
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_b);

    // Complete B, C should become ready
    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Completed).unwrap();
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_c);

    // Complete C, DAG should be complete
    dag.update_task_status(id_c, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_c, TaskStatus::Running).unwrap();
    dag.update_task_status(id_c, TaskStatus::Completed).unwrap();
    assert!(dag.is_complete());
}

#[test]
fn test_parallel_tasks() {
    let mut dag = TaskDAG::new("test-dag");

    // A branches to B and C, both merge into D
    //     A
    //    / \
    //   B   C
    //    \ /
    //     D

    let task_a = Task::new("Task A", TaskInput::default());
    let task_b = Task::new("Task B", TaskInput::default());
    let task_c = Task::new("Task C", TaskInput::default());
    let task_d = Task::new("Task D", TaskInput::default());

    let id_a = dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();
    let id_d = dag.add_task(task_d).unwrap();

    dag.add_dependency(id_a, id_b).unwrap();
    dag.add_dependency(id_a, id_c).unwrap();
    dag.add_dependency(id_b, id_d).unwrap();
    dag.add_dependency(id_c, id_d).unwrap();

    // Initially only A should be ready
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_a);

    // Complete A, both B and C should become ready
    dag.update_task_status(id_a, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_a, TaskStatus::Running).unwrap();
    dag.update_task_status(id_a, TaskStatus::Completed).unwrap();
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 2);
    assert!(ready.contains(&id_b));
    assert!(ready.contains(&id_c));

    // Complete B only, D should not be ready yet
    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Completed).unwrap();
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_c);

    // Complete C, D should become ready
    dag.update_task_status(id_c, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_c, TaskStatus::Running).unwrap();
    dag.update_task_status(id_c, TaskStatus::Completed).unwrap();
    let ready = dag.get_ready_tasks();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], id_d);
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

    // C -> A would create cycle (should fail)
    let result = dag.add_dependency(id_c, id_a);
    assert!(result.is_err());
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

#[test]
fn test_cascading_cancellation() {
    let mut dag = TaskDAG::new("test-dag");

    let task_a = Task::new("Task A", TaskInput::default());
    let task_b = Task::new("Task B", TaskInput::default());
    let task_c = Task::new("Task C", TaskInput::default());
    let task_d = Task::new("Task D", TaskInput::default());

    let id_a = dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();
    let id_d = dag.add_task(task_d).unwrap();

    // A -> B -> C -> D
    dag.add_dependency(id_a, id_b).unwrap();
    dag.add_dependency(id_b, id_c).unwrap();
    dag.add_dependency(id_c, id_d).unwrap();

    // Fail task B, should cancel C and D
    dag.update_task_status(id_a, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_a, TaskStatus::Running).unwrap();
    dag.update_task_status(id_a, TaskStatus::Completed).unwrap();
    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Failed).unwrap();

    let cancelled = dag.cancel_dependents(id_b).unwrap();
    assert_eq!(cancelled.len(), 2);
    assert!(cancelled.contains(&id_c));
    assert!(cancelled.contains(&id_d));

    // Verify statuses
    assert_eq!(dag.get_task(id_c).unwrap().status, TaskStatus::Cancelled);
    assert_eq!(dag.get_task(id_d).unwrap().status, TaskStatus::Cancelled);
}

#[test]
fn test_dag_stats() {
    let mut dag = TaskDAG::new("test-dag");

    let task_a = Task::new("Task A", TaskInput::default());
    let task_b = Task::new("Task B", TaskInput::default());
    let task_c = Task::new("Task C", TaskInput::default());

    dag.add_task(task_a).unwrap();
    let id_b = dag.add_task(task_b).unwrap();
    let id_c = dag.add_task(task_c).unwrap();

    let stats = dag.stats();
    assert_eq!(stats.total, 3);
    assert_eq!(stats.pending, 3);
    assert_eq!(stats.completed, 0);

    dag.update_task_status(id_b, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_b, TaskStatus::Running).unwrap();
    dag.update_task_status(id_b, TaskStatus::Completed).unwrap();
    dag.update_task_status(id_c, TaskStatus::Ready).unwrap();
    dag.update_task_status(id_c, TaskStatus::Running).unwrap();
    dag.update_task_status(id_c, TaskStatus::Failed).unwrap();

    let stats = dag.stats();
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.failed, 1);
    assert_eq!(stats.pending, 1);
}

#[test]
fn test_duplicate_task_id_rejected() {
    let mut dag = TaskDAG::new("test-dag");

    let task = Task::new("Task A", TaskInput::default());
    let task_id = task.id;

    dag.add_task(task.clone()).unwrap();

    // Try to add same task again
    let mut duplicate = Task::new("Task A Duplicate", TaskInput::default());
    duplicate.id = task_id;

    let result = dag.add_task(duplicate);
    assert!(result.is_err());
}
