//! Benchmarks for the caching layer.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use apex_core::cache::{Cache, CacheConfig, CacheKey, KeyBuilder, KeyType};
use apex_core::cache::key::{hash_for_key, hash_composite_key};

fn bench_cache_key_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key_construction");
    group.bench_function("simple", |b| { b.iter(|| black_box(CacheKey::new(KeyType::Task).with_id("task-123"))); });
    group.bench_function("namespaced", |b| { b.iter(|| black_box(CacheKey::new(KeyType::Task).with_namespace("project-456").with_id("task-789"))); });
    group.bench_function("full", |b| { b.iter(|| black_box(CacheKey::new(KeyType::Task).with_namespace("project-456").with_id("task-789").with_segment("details").with_tag("urgent").with_tag("project:abc").with_version(2).with_ttl(Duration::from_secs(300)))); });
    group.bench_function("convenience_task", |b| { b.iter(|| black_box(CacheKey::task("task-123"))); });
    group.bench_function("convenience_task_in_project", |b| { b.iter(|| black_box(CacheKey::task_in_project("task-123", "project-456"))); });
    group.bench_function("key_builder", |b| { b.iter(|| black_box(KeyBuilder::task().namespace("tenant-123").id("task-456").tag("high-priority").version(1).build())); });
    group.finish();
}

fn bench_cache_key_build_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key_build_string");
    let simple_key = CacheKey::new(KeyType::Task).with_id("task-123");
    let complex_key = CacheKey::new(KeyType::Task).with_namespace("project-456").with_id("task-789").with_segment("details").with_segment("metadata").with_version(3);
    group.bench_function("simple", |b| { b.iter(|| black_box(simple_key.build())); });
    group.bench_function("complex", |b| { b.iter(|| black_box(complex_key.build())); });
    group.bench_function("display_format", |b| { b.iter(|| black_box(format!("{}", simple_key))); });
    group.finish();
}

fn bench_cache_key_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key_hashing");
    group.bench_function("hash_for_key_str", |b| { b.iter(|| black_box(hash_for_key(&"test-value-string"))); });
    group.bench_function("hash_for_key_int", |b| { b.iter(|| black_box(hash_for_key(&42u64))); });
    group.bench_function("hash_composite_3", |b| { b.iter(|| black_box(hash_composite_key(["endpoint", "GET", "params"]))); });
    group.bench_function("hash_composite_6", |b| { b.iter(|| black_box(hash_composite_key(["api", "v1", "tasks", "list", "page=1", "limit=50"]))); });
    group.finish();
}

fn bench_cache_key_type_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key_type_ops");
    for kt in [KeyType::Task, KeyType::Agent, KeyType::Session, KeyType::Metrics, KeyType::Config] {
        let label = format!("{:?}", kt);
        group.bench_with_input(BenchmarkId::new("default_ttl", &label), &kt, |b, t| { b.iter(|| black_box(t.default_ttl())); });
        group.bench_with_input(BenchmarkId::new("prefix", &label), &kt, |b, t| { b.iter(|| black_box(t.prefix())); });
    }
    group.finish();
}

fn bench_cache_inmemory_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_inmemory_set");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for cap in [100, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(cap), &cap, |b, &capacity| {
            let cache = Cache::in_memory(capacity);
            b.iter(|| { let key = CacheKey::task("bench-task"); rt.block_on(async { cache.set(&key, &"bench-value").await.unwrap(); }); });
        });
    }
    group.finish();
}

fn bench_cache_inmemory_get_hit(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_inmemory_get_hit");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cache = Cache::in_memory(10_000);
    let key = CacheKey::task("bench-task");
    rt.block_on(async { cache.set(&key, &"bench-value").await.unwrap(); });
    group.bench_function("get_hit", |b| { b.iter(|| { rt.block_on(async { let val: Option<String> = cache.get(&key).await.unwrap(); black_box(val); }); }); });
    group.finish();
}

fn bench_cache_inmemory_get_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_inmemory_get_miss");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cache = Cache::in_memory(10_000);
    let key = CacheKey::task("nonexistent-key");
    group.bench_function("get_miss", |b| { b.iter(|| { rt.block_on(async { let val: Option<String> = cache.get(&key).await.unwrap(); black_box(val); }); }); });
    group.finish();
}

fn bench_cache_inmemory_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_inmemory_throughput");
    let rt = tokio::runtime::Runtime::new().unwrap();
    for ops in [100, 1_000] {
        group.throughput(Throughput::Elements(ops as u64));
        group.bench_with_input(BenchmarkId::from_parameter(ops), &ops, |b, &n| {
            let cache = Cache::in_memory(10_000);
            b.iter(|| { rt.block_on(async {
                for i in 0..n { let key = CacheKey::task(format!("task-{i}")); cache.set(&key, &format!("value-{i}")).await.unwrap(); }
                for i in 0..n { let key = CacheKey::task(format!("task-{i}")); let _: Option<String> = cache.get(&key).await.unwrap(); }
            }); });
        });
    }
    group.finish();
}

fn bench_cache_config(c: &mut Criterion) {
    c.bench_function("cache_config_builder", |b| {
        b.iter(|| black_box(CacheConfig::builder().default_ttl(Duration::from_secs(600)).max_entry_size(2 * 1024 * 1024).enable_metrics(true).namespace_prefix("apex:bench:").enable_compression(false).compression_threshold(2048).build()));
    });
}

criterion_group!(benches, bench_cache_key_construction, bench_cache_key_build_string, bench_cache_key_hashing, bench_cache_key_type_ops, bench_cache_inmemory_set, bench_cache_inmemory_get_hit, bench_cache_inmemory_get_miss, bench_cache_inmemory_throughput, bench_cache_config);
criterion_main!(benches);
