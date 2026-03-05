//! Benchmarks for the Apex Orchestrator
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use uuid::Uuid;

/// Benchmark DAG creation with varying task counts.
fn bench_dag_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_creation");

    for task_count in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*task_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(task_count),
            task_count,
            |b, &count| {
                b.iter(|| {
                    let mut tasks: Vec<Uuid> = Vec::with_capacity(count);
                    for _ in 0..count {
                        tasks.push(black_box(Uuid::new_v4()));
                    }
                    tasks
                });
            },
        );
    }
    group.finish();
}

/// Benchmark topological sort algorithm.
fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("topological_sort");

    for task_count in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(task_count),
            task_count,
            |b, &count| {
                // Create a linear dependency chain
                let tasks: Vec<usize> = (0..count).collect();
                let mut deps: HashMap<usize, Vec<usize>> = HashMap::new();
                for i in 1..count {
                    deps.insert(i, vec![i - 1]);
                }

                b.iter(|| {
                    // Simple Kahn's algorithm simulation
                    let mut in_degree: HashMap<usize, usize> = HashMap::new();
                    for &task in &tasks {
                        in_degree.insert(task, 0);
                    }
                    for (_, predecessors) in &deps {
                        for &pred in predecessors {
                            *in_degree.entry(pred).or_insert(0) += 1;
                        }
                    }

                    let mut result = Vec::with_capacity(count);
                    let mut queue: Vec<usize> = in_degree
                        .iter()
                        .filter(|(_, &deg)| deg == 0)
                        .map(|(&task, _)| task)
                        .collect();

                    while let Some(task) = queue.pop() {
                        result.push(black_box(task));
                        if let Some(predecessors) = deps.get(&task) {
                            for &pred in predecessors {
                                if let Some(deg) = in_degree.get_mut(&pred) {
                                    *deg = deg.saturating_sub(1);
                                    if *deg == 0 {
                                        queue.push(pred);
                                    }
                                }
                            }
                        }
                    }
                    result
                });
            },
        );
    }
    group.finish();
}

/// Benchmark contract limit checking.
fn bench_contract_validation(c: &mut Criterion) {
    c.bench_function("contract_limit_check", |b| {
        let token_limit = 10000u64;
        let cost_limit = 1.0f64;
        let api_limit = 100u32;

        b.iter(|| {
            let tokens_used = black_box(5000u64);
            let cost_used = black_box(0.5f64);
            let api_calls = black_box(50u32);

            // Simulate limit checking
            let token_ok = tokens_used < token_limit;
            let cost_ok = cost_used < cost_limit;
            let api_ok = api_calls < api_limit;

            black_box(token_ok && cost_ok && api_ok)
        });
    });
}

/// Benchmark model tier selection (FrugalGPT routing simulation).
fn bench_model_routing(c: &mut Criterion) {
    c.bench_function("frugal_gpt_routing", |b| {
        let tiers = vec![
            ("gpt-4o-mini", 0.0, 0.95),     // model, cost_per_1k, accuracy
            ("gpt-4o", 0.005, 0.98),        // medium tier
            ("claude-3-opus", 0.015, 0.99), // premium tier
        ];

        b.iter(|| {
            let complexity = black_box(0.7f64);
            let budget = black_box(0.01f64);

            // Find best model within budget
            let mut selected = &tiers[0];
            for tier in &tiers {
                if tier.1 <= budget && tier.2 >= complexity {
                    selected = tier;
                }
            }
            black_box(selected.0)
        });
    });
}

/// Benchmark priority queue operations.
fn bench_task_scheduling(c: &mut Criterion) {
    use std::collections::BinaryHeap;

    c.bench_function("task_queue_operations", |b| {
        b.iter(|| {
            let mut heap: BinaryHeap<(i32, Uuid)> = BinaryHeap::new();

            // Insert 100 tasks with random priorities
            for i in 0..100 {
                heap.push((black_box(i % 10), Uuid::new_v4()));
            }

            // Pop 50 tasks
            let mut popped = Vec::with_capacity(50);
            for _ in 0..50 {
                if let Some(task) = heap.pop() {
                    popped.push(black_box(task));
                }
            }
            popped
        });
    });
}

/// Benchmark UUID generation (used extensively).
fn bench_uuid_generation(c: &mut Criterion) {
    c.bench_function("uuid_v4_generation", |b| {
        b.iter(|| black_box(Uuid::new_v4()));
    });
}

/// Benchmark resource utilization calculation.
fn bench_utilization_calculation(c: &mut Criterion) {
    c.bench_function("utilization_calculation", |b| {
        b.iter(|| {
            let tokens_used = black_box(7500u64);
            let tokens_limit = black_box(10000u64);
            let cost_used = black_box(0.75f64);
            let cost_limit = black_box(1.0f64);
            let api_calls = black_box(80u32);
            let api_limit = black_box(100u32);

            let token_util = (tokens_used as f64 / tokens_limit as f64) * 100.0;
            let cost_util = (cost_used / cost_limit) * 100.0;
            let api_util = (api_calls as f64 / api_limit as f64) * 100.0;

            black_box((token_util, cost_util, api_util))
        });
    });
}

criterion_group!(
    benches,
    bench_dag_creation,
    bench_topological_sort,
    bench_contract_validation,
    bench_model_routing,
    bench_task_scheduling,
    bench_uuid_generation,
    bench_utilization_calculation,
);
criterion_main!(benches);
