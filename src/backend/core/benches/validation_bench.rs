//! Benchmarks for the request validation framework.
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use apex_core::validation::{validate_field, validate_request, Email, MaxItems, MaxLength, MinLength, Pattern, Range, Required, UniqueItems, Validate, ValidationResult, ValidationRule};

struct CreateTaskRequest { name: String, description: String, priority: i32, tags: Vec<String> }
impl Validate for CreateTaskRequest {
    fn validate(&self) -> ValidationResult<()> {
        validate_request()
            .field(validate_field("name", &self.name).rule(Required).rule(MinLength(2)).rule(MaxLength(100)))
            .field(validate_field("description", &self.description).rule(Required).rule(MaxLength(1000)))
            .field(validate_field("priority", &self.priority).rule(Range::new(0, 10)))
            .field(validate_field("tags", &self.tags).rule(MaxItems(20)).rule(UniqueItems))
            .result()
    }
}
fn valid_request() -> CreateTaskRequest { CreateTaskRequest { name: "Benchmark Task".into(), description: "A task for benchmarking".into(), priority: 5, tags: vec!["bench".into(), "test".into()] } }
fn invalid_request() -> CreateTaskRequest { CreateTaskRequest { name: "".into(), description: "x".repeat(2000), priority: 99, tags: vec!["a".into(), "a".into()] } }

fn bench_validation_individual_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_individual_rules");
    let valid_str = "hello@example.com".to_string(); let empty_str = "".to_string(); let long_str = "x".repeat(500);
    group.bench_function("required_pass", |b| { b.iter(|| black_box(Required.validate(&valid_str))); });
    group.bench_function("required_fail", |b| { b.iter(|| black_box(Required.validate(&empty_str))); });
    group.bench_function("email_pass", |b| { b.iter(|| black_box(Email.validate(&valid_str))); });
    group.bench_function("email_fail", |b| { let bad = "not-an-email".to_string(); b.iter(|| black_box(Email.validate(&bad))); });
    group.bench_function("min_length_pass", |b| { b.iter(|| black_box(MinLength(3).validate(&valid_str))); });
    group.bench_function("max_length_pass", |b| { b.iter(|| black_box(MaxLength(255).validate(&valid_str))); });
    group.bench_function("max_length_fail", |b| { b.iter(|| black_box(MaxLength(100).validate(&long_str))); });
    group.bench_function("range_pass", |b| { let v = 50i32; b.iter(|| black_box(Range::new(0, 100).validate(&v))); });
    group.bench_function("range_fail", |b| { let v = 200i32; b.iter(|| black_box(Range::new(0, 100).validate(&v))); });
    group.finish();
}

fn bench_validation_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_pattern");
    let valid = "ABC".to_string(); let invalid = "abc123".to_string();
    group.bench_function("compile_and_validate", |b| { b.iter(|| { let p = Pattern::new(r"^[A-Z]{3}$").unwrap(); black_box(p.validate(&valid)) }); });
    group.bench_function("precompiled_pass", |b| { let p = Pattern::new(r"^[A-Z]{3}$").unwrap(); b.iter(|| black_box(p.validate(&valid))); });
    group.bench_function("precompiled_fail", |b| { let p = Pattern::new(r"^[A-Z]{3}$").unwrap(); b.iter(|| black_box(p.validate(&invalid))); });
    group.bench_function("complex_pattern", |b| { let p = Pattern::new(r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9-]+\.[a-zA-Z]{2,}$").unwrap(); let e = "test@example.com".to_string(); b.iter(|| black_box(p.validate(&e))); });
    group.finish();
}

fn bench_validation_chained(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_chained");
    let valid_email = "user@example.com".to_string();
    group.bench_function("3_rules_pass", |b| { b.iter(|| black_box(validate_field("email", &valid_email).rule(Required).rule(Email).rule(MaxLength(255)).result())); });
    group.bench_function("3_rules_fail_all", |b| { let empty = "".to_string(); b.iter(|| black_box(validate_field("email", &empty).rule(Required).rule(MinLength(5)).rule(Email).result())); });
    group.bench_function("5_rules_pass", |b| { b.iter(|| black_box(validate_field("email", &valid_email).rule(Required).rule(MinLength(5)).rule(MaxLength(255)).rule(Email).rule(MaxLength(500)).result())); });
    group.finish();
}

fn bench_validation_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_request");
    group.bench_function("valid_request", |b| { let r = valid_request(); b.iter(|| black_box(r.validate())); });
    group.bench_function("invalid_request", |b| { let r = invalid_request(); b.iter(|| black_box(r.validate())); });
    group.bench_function("is_valid_check", |b| { let r = valid_request(); b.iter(|| black_box(r.is_valid())); });
    group.finish();
}

fn bench_validation_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_batch");
    for batch_size in [10, 100, 1_000] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(BenchmarkId::new("valid", batch_size), &batch_size, |b, &n| { let reqs: Vec<_> = (0..n).map(|_| valid_request()).collect(); b.iter(|| { for r in &reqs { black_box(r.validate()); } }); });
        group.bench_with_input(BenchmarkId::new("mixed", batch_size), &batch_size, |b, &n| { let reqs: Vec<_> = (0..n).map(|i| if i % 3 == 0 { invalid_request() } else { valid_request() }).collect(); b.iter(|| { for r in &reqs { black_box(r.validate()); } }); });
    }
    group.finish();
}

fn bench_validation_errors(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_errors");
    group.bench_function("collect_errors", |b| { let r = invalid_request(); b.iter(|| { if let Err(e) = r.validate() { black_box(e.error_count()); } }); });
    group.finish();
}

criterion_group!(benches, bench_validation_individual_rules, bench_validation_pattern, bench_validation_chained, bench_validation_request, bench_validation_batch, bench_validation_errors);
criterion_main!(benches);
