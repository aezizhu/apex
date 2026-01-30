//! Benchmarks for the FrugalGPT-style Adaptive Model Router.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use apex_core::routing::{ModelRouter, ModelTier, RoutingConfig};

const SIMPLE_TASK: &str = "List the files in the directory";
const MEDIUM_TASK: &str = "Analyze the code structure and provide a summary of the architecture";
const COMPLEX_TASK: &str = "Analyze this complex mathematical proof, evaluate its correctness with detailed step-by-step reasoning, and design an advanced testing strategy";
const CODE_TASK: &str = "Debug the program and fix the code issue in the module";
const MATH_TASK: &str = "Calculate and prove the mathematical theorem step by step";

fn bench_routing_model_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_model_selection");
    let router = ModelRouter::new();
    for (label, desc) in [("simple", SIMPLE_TASK), ("medium", MEDIUM_TASK), ("complex", COMPLEX_TASK), ("code", CODE_TASK), ("math", MATH_TASK)] {
        group.bench_with_input(BenchmarkId::from_parameter(label), desc, |b, input| { b.iter(|| black_box(router.select_model(input))); });
    }
    group.finish();
}

fn bench_routing_batch_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_batch_selection");
    let router = ModelRouter::new();
    let tasks = vec![SIMPLE_TASK, MEDIUM_TASK, COMPLEX_TASK, CODE_TASK, MATH_TASK];
    for batch_size in [100, 1_000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(batch_size), &batch_size, |b, &n| {
            b.iter(|| { for i in 0..n { black_box(router.select_model(tasks[i % tasks.len()])); } });
        });
    }
    group.finish();
}

fn bench_routing_escalation(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_escalation");
    let router = ModelRouter::new();
    for (label, confidence, tier) in [("economy_low", 0.5_f64, ModelTier::Economy), ("economy_high", 0.9, ModelTier::Economy), ("standard_low", 0.5, ModelTier::Standard), ("standard_high", 0.8, ModelTier::Standard), ("premium", 0.5, ModelTier::Premium)] {
        group.bench_with_input(BenchmarkId::from_parameter(label), &(confidence, &tier), |b, &(conf, t)| {
            b.iter(|| { let should = router.should_escalate(conf, t); if should { black_box(router.escalate_tier(t)); } black_box(should) });
        });
    }
    group.finish();
}

fn bench_routing_cost_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_cost_estimation");
    let router = ModelRouter::new();
    for model in ["gpt-4o-mini", "gpt-4o", "claude-3.5-haiku", "claude-3.5-sonnet", "claude-opus-4"] {
        group.bench_with_input(BenchmarkId::from_parameter(model), model, |b, m| { b.iter(|| black_box(router.estimate_cost(m, 1000, 500))); });
    }
    group.finish();
}

fn bench_routing_configurations(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_configurations");
    let configs = vec![
        ("default", RoutingConfig::default()),
        ("strict", RoutingConfig { economy_threshold: 0.95, standard_threshold: 0.90, max_escalations: 1, enable_cascade: true }),
        ("relaxed", RoutingConfig { economy_threshold: 0.50, standard_threshold: 0.40, max_escalations: 3, enable_cascade: true }),
        ("no_cascade", RoutingConfig { enable_cascade: false, ..RoutingConfig::default() }),
    ];
    for (label, config) in &configs {
        group.bench_with_input(BenchmarkId::from_parameter(label), config, |b, cfg| { let router = ModelRouter::with_config(cfg.clone()); b.iter(|| black_box(router.select_model(MEDIUM_TASK))); });
    }
    group.finish();
}

fn bench_routing_model_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_model_lookup");
    let router = ModelRouter::new();
    for model in ["gpt-4o-mini", "claude-opus-4", "nonexistent-model"] {
        group.bench_with_input(BenchmarkId::from_parameter(model), model, |b, m| { b.iter(|| black_box(router.get_model(m))); });
    }
    group.finish();
}

criterion_group!(benches, bench_routing_model_selection, bench_routing_batch_selection, bench_routing_escalation, bench_routing_cost_estimation, bench_routing_configurations, bench_routing_model_lookup);
criterion_main!(benches);
