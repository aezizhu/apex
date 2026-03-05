//! Benchmarks for the DAG (Directed Acyclic Graph) execution engine.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use apex_core::dag::{Task, TaskDAG, TaskId, TaskInput, TaskStatus};

fn build_linear_dag(n: usize) -> (TaskDAG, Vec<TaskId>) {
    let mut dag = TaskDAG::new("linear-bench");
    let mut ids = Vec::with_capacity(n);
    for i in 0..n {
        let task = Task::new(format!("task-{i}"), TaskInput::default());
        let id = dag.add_task(task).unwrap();
        if let Some(&prev) = ids.last() { dag.add_dependency(prev, id).unwrap(); }
        ids.push(id);
    }
    (dag, ids)
}

fn build_fanout_dag(fan: usize) -> (TaskDAG, Vec<TaskId>) {
    let mut dag = TaskDAG::new("fanout-bench");
    let root = dag.add_task(Task::new("root", TaskInput::default())).unwrap();
    let mut ids = vec![root];
    for i in 0..fan {
        let child = dag.add_task(Task::new(format!("child-{i}"), TaskInput::default())).unwrap();
        dag.add_dependency(root, child).unwrap();
        ids.push(child);
    }
    (dag, ids)
}

fn build_layered_dag(layers: usize, width: usize) -> (TaskDAG, Vec<TaskId>) {
    let mut dag = TaskDAG::new("layered-bench");
    let mut prev_layer: Vec<TaskId> = Vec::new();
    let mut all_ids = Vec::new();
    for l in 0..layers {
        let mut current_layer = Vec::with_capacity(width);
        for w in 0..width {
            let task = Task::new(format!("L{l}-W{w}"), TaskInput::default());
            let id = dag.add_task(task).unwrap();
            for &prev in &prev_layer { dag.add_dependency(prev, id).unwrap(); }
            current_layer.push(id);
            all_ids.push(id);
        }
        prev_layer = current_layer;
    }
    (dag, all_ids)
}

fn bench_dag_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_construction");
    for size in [10, 100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| {
            b.iter(|| {
                let mut dag = TaskDAG::new("bench");
                for i in 0..n { dag.add_task(Task::new(format!("t-{i}"), TaskInput::default())).unwrap(); }
                black_box(dag)
            });
        });
    }
    group.finish();
}

fn bench_dag_dependency_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_dependency_insertion");
    for size in [10, 100, 1_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| {
            b.iter_batched(
                || { let mut dag = TaskDAG::new("bench"); let ids: Vec<_> = (0..n).map(|i| dag.add_task(Task::new(format!("t-{i}"), TaskInput::default())).unwrap()).collect(); (dag, ids) },
                |(mut dag, ids)| { for pair in ids.windows(2) { dag.add_dependency(pair[0], pair[1]).unwrap(); } black_box(dag) },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_dag_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_topological_sort");
    for size in [10, 100, 1_000] {
        group.bench_with_input(BenchmarkId::new("linear", size), &size, |b, &n| { let (dag, _) = build_linear_dag(n); b.iter(|| black_box(dag.topological_order().unwrap())); });
        group.bench_with_input(BenchmarkId::new("fanout", size), &size, |b, &n| { let (dag, _) = build_fanout_dag(n); b.iter(|| black_box(dag.topological_order().unwrap())); });
    }
    for (layers, width) in [(5, 20), (10, 10), (20, 5)] {
        let label = format!("{layers}x{width}");
        group.bench_with_input(BenchmarkId::new("layered", &label), &(layers, width), |b, &(l, w)| { let (dag, _) = build_layered_dag(l, w); b.iter(|| black_box(dag.topological_order().unwrap())); });
    }
    group.finish();
}

fn bench_dag_ready_tasks(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_ready_tasks");
    for fan in [10, 100, 1_000] {
        group.bench_with_input(BenchmarkId::from_parameter(fan), &fan, |b, &n| {
            let (mut dag, ids) = build_fanout_dag(n);
            dag.update_task_status(ids[0], TaskStatus::Ready).unwrap();
            dag.update_task_status(ids[0], TaskStatus::Running).unwrap();
            dag.update_task_status(ids[0], TaskStatus::Completed).unwrap();
            b.iter(|| black_box(dag.get_ready_tasks()));
        });
    }
    group.finish();
}

fn bench_dag_status_transitions(c: &mut Criterion) {
    c.bench_function("dag_status_transitions", |b| {
        b.iter_batched(
            || { let mut dag = TaskDAG::new("bench"); let id = dag.add_task(Task::new("task", TaskInput::default())).unwrap(); (dag, id) },
            |(mut dag, id)| { dag.update_task_status(id, TaskStatus::Ready).unwrap(); dag.update_task_status(id, TaskStatus::Running).unwrap(); dag.update_task_status(id, TaskStatus::Completed).unwrap(); black_box(&dag); },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_dag_cascading_cancel(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_cascading_cancel");
    for depth in [10, 100, 1_000] {
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, &n| {
            b.iter_batched(|| build_linear_dag(n), |(mut dag, ids)| { black_box(dag.cancel_dependents(ids[0]).unwrap()); }, criterion::BatchSize::SmallInput);
        });
    }
    group.finish();
}

fn bench_dag_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_stats");
    for size in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| { let (dag, _) = build_linear_dag(n); b.iter(|| black_box(dag.stats())); });
    }
    group.finish();
}

fn bench_dag_agent_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_agent_scenarios");
    group.sample_size(10);
    for agents in [100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(agents as u64));
        group.bench_with_input(BenchmarkId::new("construct_linear", agents), &agents, |b, &n| { b.iter(|| black_box(build_linear_dag(n))); });
        group.bench_with_input(BenchmarkId::new("construct_fanout", agents), &agents, |b, &n| { b.iter(|| black_box(build_fanout_dag(n))); });
        let width = (agents as f64).sqrt() as usize; let layers = agents / width;
        group.bench_with_input(BenchmarkId::new("construct_layered", agents), &(layers, width), |b, &(l, w)| { b.iter(|| black_box(build_layered_dag(l, w))); });
    }
    group.finish();
}

criterion_group!(benches, bench_dag_construction, bench_dag_dependency_insertion, bench_dag_topological_sort, bench_dag_ready_tasks, bench_dag_status_transitions, bench_dag_cascading_cancel, bench_dag_stats, bench_dag_agent_scenarios);
criterion_main!(benches);
