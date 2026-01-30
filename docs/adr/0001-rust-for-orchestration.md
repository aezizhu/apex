# ADR-0001: Rust for Core Orchestration Engine

## Status

Accepted

## Date

2026-01-30

## Context

Apex requires a high-performance orchestration engine capable of managing complex workflows, coordinating multiple agents, and handling concurrent task execution at scale. The orchestration layer is the critical path for all operations and must provide:

- Sub-millisecond scheduling latency
- Memory safety without garbage collection pauses
- Predictable performance characteristics
- Strong concurrency primitives
- Ability to handle thousands of concurrent agent executions

The choice of implementation language fundamentally affects system reliability, performance ceiling, and operational characteristics.

## Decision

We will implement the core orchestration engine in Rust.

Key factors driving this decision:

1. **Memory Safety Without GC**: Rust's ownership model provides memory safety guarantees at compile time without runtime garbage collection, eliminating unpredictable pause times during critical scheduling operations.

2. **Zero-Cost Abstractions**: Rust's abstractions compile to efficient machine code, allowing us to write high-level orchestration logic without sacrificing performance.

3. **Fearless Concurrency**: The borrow checker prevents data races at compile time, enabling safe concurrent access to shared scheduling state across multiple threads.

4. **Async Runtime (Tokio)**: The Tokio ecosystem provides production-grade async I/O primitives ideal for managing thousands of concurrent agent connections.

5. **FFI Compatibility**: Rust's C-compatible FFI enables seamless integration with Python agents via PyO3, allowing tight coupling where needed.

6. **Type System**: Rust's expressive type system allows encoding complex invariants (e.g., DAG validity, contract constraints) directly in the type system.

## Consequences

### Positive

- Predictable latency with no GC pauses during scheduling
- Memory safety bugs caught at compile time rather than production
- Excellent performance for CPU-bound scheduling algorithms
- Strong ecosystem for async networking (Tokio, Hyper, Tonic)
- Single binary deployment simplifies operations
- Cross-platform compilation support

### Negative

- Steeper learning curve compared to Python or Go
- Longer initial development time due to satisfying the borrow checker
- Smaller talent pool for hiring Rust engineers
- Compilation times can be significant for large codebases
- Some dynamic patterns require more ceremony (e.g., plugin systems)

### Neutral

- Forces explicit handling of all error cases
- Requires upfront design of ownership semantics
- Binary size is larger than C but smaller than Go

## Alternatives Considered

### Go

Go offers simpler concurrency primitives and faster development velocity. However, its garbage collector introduces unpredictable latencies (typically 1-10ms), which is problematic for sub-millisecond scheduling targets. Go's lack of generics (prior to 1.18) and less expressive type system also limit our ability to encode invariants at compile time.

### C++

C++ provides comparable performance but lacks memory safety guarantees. The complexity of modern C++ (smart pointers, move semantics, undefined behavior) increases the risk of subtle bugs. Rust provides equivalent performance with stronger safety guarantees.

### Python

Python's GIL and interpreted nature make it unsuitable for high-performance concurrent workloads. While excellent for agent logic (see ADR-0002), it cannot meet the orchestration engine's performance requirements.

### JVM Languages (Kotlin/Scala)

JVM languages offer good performance and strong type systems but share Go's GC-related latency issues. JVM startup time and memory overhead are also concerns for containerized deployments.

## References

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Tokio: A Runtime for Writing Reliable Asynchronous Applications](https://tokio.rs/)
- [PyO3: Rust bindings for Python](https://pyo3.rs/)
- [Discord's Migration to Rust](https://discord.com/blog/why-discord-is-switching-from-go-to-rust)
