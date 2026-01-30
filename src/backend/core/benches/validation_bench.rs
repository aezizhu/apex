//! Validation benchmarks. Run with: cargo bench --bench validation_bench
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;
use apex_core::validation::{validate_field, validate_request, Email, MaxItems, MaxLength, MinLength, Pattern, Range, Required, UniqueItems, Url, Uuid, Validate, ValidationErrors, ValidationResult};
struct CreateTaskRequest { name: String, instruction: String, priority: i32, labels: Vec<String>, email: String }
impl Validate for CreateTaskRequest {
    fn validate(&self) -> ValidationResult<()> {
        validate_request()
            .field(validate_field("name", &self.name).rule(Required).rule(MinLength(2)).rule(MaxLength(255)))
            .field(validate_field("instruction", &self.instruction).rule(Required).rule(MinLength(10)).rule(MaxLength(10000)))
            .field(validate_field("priority", &self.priority).rule(Range::new(1, 10)))
            .field(validate_field("labels", &self.labels).rule(MaxItems(20)).rule(UniqueItems))
            .field(validate_field("email", &self.email).rule(Required).rule(Email).rule(MaxLength(255)))
            .result()
    }
}
fn valid_request() -> CreateTaskRequest { CreateTaskRequest { name: "Analyze codebase".into(), instruction: "Analyze the Rust codebase and identify potential performance improvements focusing on hot paths.".into(), priority: 5, labels: vec!["performance".into(), "analysis".into(), "rust".into()], email: "user@example.com".into() } }
fn invalid_request() -> CreateTaskRequest { CreateTaskRequest { name: "".into(), instruction: "Short".into(), priority: 15, labels: vec!["a".into(), "a".into()], email: "not-an-email".into() } }
fn bench_individual_rules(c: &mut Criterion) {
    let mut g = c.benchmark_group("validation_individual_rules"); g.measurement_time(Duration::from_secs(5));
    g.bench_function("required_pass", |b| { let v = "hello"; b.iter(|| { black_box(validate_field("f", &v.to_string()).rule(Required).result()); }); });
    g.bench_function("required_fail", |b| { let v = ""; b.iter(|| { black_box(validate_field("f", &v.to_string()).rule(Required).result()); }); });
    g.bench_function("email_valid", |b| { let e = "user@example.com".to_string(); b.iter(|| { black_box(validate_field("e", &e).rule(Email).result()); }); });
    g.bench_function("email_invalid", |b| { let e = "not-an-email".to_string(); b.iter(|| { black_box(validate_field("e", &e).rule(Email).result()); }); });
    g.bench_function("min_length_pass", |b| { let v = "hello world".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(MinLength(5)).result()); }); });
    g.bench_function("max_length_pass", |b| { let v = "short".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(MaxLength(100)).result()); }); });
    g.bench_function("range_pass", |b| { let v = 5i32; b.iter(|| { black_box(validate_field("f", &v).rule(Range::new(1, 10)).result()); }); });
    g.bench_function("range_fail", |b| { let v = 15i32; b.iter(|| { black_box(validate_field("f", &v).rule(Range::new(1, 10)).result()); }); });
    g.bench_function("uuid_valid", |b| { let v = "550e8400-e29b-41d4-a716-446655440000".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(Uuid).result()); }); });
    g.bench_function("uuid_invalid", |b| { let v = "not-a-uuid".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(Uuid).result()); }); });
    g.bench_function("url_valid", |b| { let v = "https://example.com/api/v1/tasks".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(Url).result()); }); });
    g.finish();
}
fn bench_pattern_validation(c: &mut Criterion) {
    let mut g = c.benchmark_group("validation_pattern"); g.measurement_time(Duration::from_secs(5));
    g.bench_function("simple_pattern_match", |b| { let v = "valid_identifier_123".to_string(); b.iter(|| { let p = Pattern::new(r"^[a-z0-9_]+$").unwrap(); black_box(validate_field("f", &v).rule(p).result()); }); });
    g.bench_function("simple_pattern_no_match", |b| { let v = "Invalid!".to_string(); b.iter(|| { let p = Pattern::new(r"^[a-z0-9_]+$").unwrap(); black_box(validate_field("f", &v).rule(p).result()); }); });
    g.bench_function("complex_pattern", |b| { let v = "user+tag@sub.example.co.uk".to_string(); b.iter(|| { let p = Pattern::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap(); black_box(validate_field("f", &v).rule(p).result()); }); });
    g.finish();
}
fn bench_chained_validation(c: &mut Criterion) {
    let mut g = c.benchmark_group("validation_chained"); g.measurement_time(Duration::from_secs(5));
    g.bench_function("single_rule", |b| { let v = "hello".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(Required).result()); }); });
    g.bench_function("two_rules", |b| { let v = "hello".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(Required).rule(MinLength(3)).result()); }); });
    g.bench_function("three_rules", |b| { let v = "hello".to_string(); b.iter(|| { black_box(validate_field("f", &v).rule(Required).rule(MinLength(3)).rule(MaxLength(100)).result()); }); });
    g.bench_function("email_chain", |b| { let v = "user@example.com".to_string(); b.iter(|| { black_box(validate_field("e", &v).rule(Required).rule(Email).rule(MaxLength(255)).result()); }); });
    g.finish();
}
fn bench_request_validation(c: &mut Criterion) {
    let mut g = c.benchmark_group("validation_request"); g.measurement_time(Duration::from_secs(5));
    g.bench_function("valid_request", |b| { let req = valid_request(); b.iter(|| black_box(req.validate())); });
    g.bench_function("invalid_request", |b| { let req = invalid_request(); b.iter(|| black_box(req.validate())); });
    g.bench_function("is_valid_check", |b| { let req = valid_request(); b.iter(|| black_box(req.is_valid())); });
    g.finish();
}
fn bench_batch_validation(c: &mut Criterion) {
    let mut g = c.benchmark_group("validation_batch"); g.measurement_time(Duration::from_secs(8));
    for &bs in &[10, 100, 1000] {
        g.throughput(Throughput::Elements(bs as u64));
        g.bench_with_input(BenchmarkId::new("all_valid", bs), &bs, |b, &n| {
            let reqs: Vec<CreateTaskRequest> = (0..n).map(|i| CreateTaskRequest { name: format!("Task {}", i), instruction: format!("Analyze component {} and provide detailed recommendations for improvement", i), priority: (i%10) as i32 + 1, labels: vec![format!("b-{}", i)], email: format!("u{}@example.com", i) }).collect();
            b.iter(|| { let mut c = 0; for r in &reqs { if r.is_valid() { c += 1; } } black_box(c) });
        });
        g.bench_with_input(BenchmarkId::new("mixed", bs), &bs, |b, &n| {
            let reqs: Vec<CreateTaskRequest> = (0..n).map(|i| if i%3==0 { invalid_request() } else { CreateTaskRequest { name: format!("Task {}", i), instruction: format!("Analyze component {} with detailed step-by-step approach", i), priority: (i%10) as i32 + 1, labels: vec![format!("b-{}", i)], email: format!("u{}@example.com", i) } }).collect();
            b.iter(|| { let mut r = Vec::with_capacity(n); for req in &reqs { r.push(req.validate().is_ok()); } black_box(r) });
        });
    }
    g.finish();
}
fn bench_validation_errors(c: &mut Criterion) {
    let mut g = c.benchmark_group("validation_errors");
    g.bench_function("create_empty", |b| { b.iter(|| black_box(ValidationErrors::new())); });
    g.bench_function("add_required_error", |b| { b.iter(|| { let mut e = ValidationErrors::new(); e.add_required("f"); black_box(e) }); });
    g.bench_function("add_multiple_errors", |b| { b.iter(|| { let mut e = ValidationErrors::new(); e.add_required("name"); e.add_required("email"); e.add_required("instruction"); black_box(e) }); });
    g.bench_function("check_has_errors", |b| { let mut e = ValidationErrors::new(); e.add_required("name"); e.add_required("email"); b.iter(|| { black_box(e.has_errors("name")); black_box(e.has_errors("email")); black_box(e.has_errors("ne")); }); });
    g.bench_function("is_empty_check", |b| { let e = ValidationErrors::new(); b.iter(|| black_box(e.is_empty())); });
    g.bench_function("to_message_map", |b| { let mut e = ValidationErrors::new(); e.add_required("name"); e.add_required("email"); e.add_required("instruction"); b.iter(|| black_box(e.to_message_map())); });
    g.finish();
}
criterion_group!(benches, bench_individual_rules, bench_pattern_validation, bench_chained_validation, bench_request_validation, bench_batch_validation, bench_validation_errors);
criterion_main!(benches);
