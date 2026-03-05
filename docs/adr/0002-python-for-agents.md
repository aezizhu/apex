# ADR-0002: Python for Agent Execution

## Status

Accepted

## Date

2026-01-30

## Context

Apex agents are the units of work that perform actual tasks within workflows. These agents need to:

- Integrate with diverse AI/ML frameworks and APIs
- Be rapidly developed and iterated upon
- Handle complex data transformations
- Support dynamic behavior and runtime flexibility
- Be accessible to a wide range of developers

While the orchestration engine requires raw performance (see ADR-0001), agents have different requirements centered on flexibility, ecosystem integration, and developer productivity.

## Decision

We will implement agents in Python, executed in isolated environments managed by the Rust orchestration engine.

Key factors driving this decision:

1. **AI/ML Ecosystem**: Python dominates the AI/ML landscape with libraries like LangChain, LlamaIndex, Transformers, and OpenAI SDK. Agent development benefits directly from this ecosystem.

2. **Developer Productivity**: Python's dynamic typing and interpreted nature enable rapid prototyping and iteration. Agents are often experimental and benefit from fast feedback loops.

3. **Wide Adoption**: Python is one of the most widely known programming languages, maximizing the pool of potential agent developers.

4. **Data Processing**: Libraries like Pandas, NumPy, and Polars make complex data transformations trivial to implement.

5. **LLM Tool Calling**: Most LLM function-calling examples and best practices are documented in Python, reducing friction for agent authors.

6. **Dynamic Capabilities**: Python's reflection and metaprogramming capabilities support dynamic agent behavior patterns (plugin loading, runtime configuration).

## Consequences

### Positive

- Access to the entire Python AI/ML ecosystem
- Lower barrier to entry for agent authors
- Rapid development and iteration cycles
- Excellent library support for API integrations
- Strong async support via asyncio for I/O-bound agent work
- Rich testing ecosystem (pytest, hypothesis)

### Negative

- Performance overhead compared to compiled languages
- GIL limits true parallelism (mitigated by process isolation)
- Runtime type errors possible (mitigated by type hints and mypy)
- Dependency management complexity (mitigated by containerization)
- Memory overhead per-process higher than Rust

### Neutral

- Requires Python runtime in agent containers
- Forces clear boundary between orchestration (Rust) and execution (Python)
- Enables polyglot architecture for future language support

## Alternatives Considered

### TypeScript/JavaScript

JavaScript has a strong ecosystem for web APIs but lacks Python's AI/ML library depth. TypeScript adds type safety but the ecosystem is less mature for LLM development. However, TypeScript agents may be supported in future iterations.

### Rust Agents

While Rust agents would eliminate the FFI boundary, the development velocity trade-off is significant. Rust's compile times and stricter type system slow iteration. For performance-critical agents, we may support Rust as a secondary option.

### JVM Languages

Kotlin or Scala offer type safety and good performance but have limited AI/ML library support compared to Python. JVM startup time is also problematic for short-lived agent executions.

### Go

Go's simplicity is appealing but its AI/ML ecosystem is nascent. Most LLM SDKs prioritize Python, making Go a second-class citizen for agent development.

## References

- [LangChain Documentation](https://docs.langchain.com/)
- [OpenAI Python SDK](https://github.com/openai/openai-python)
- [Python Type Hints (PEP 484)](https://peps.python.org/pep-0484/)
- [asyncio Documentation](https://docs.python.org/3/library/asyncio.html)
