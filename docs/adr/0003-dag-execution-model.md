# ADR-0003: DAG-Based Task Execution Model

## Status

Accepted

## Date

2026-01-30

## Context

Apex workflows consist of multiple agents that must execute in a coordinated manner. Workflows have complex dependency relationships:

- Some agents must complete before others can start
- Some agents can run in parallel
- Results from earlier agents feed into later agents
- Failure of one agent may affect downstream agents
- Dynamic branching and conditional execution are required

We need an execution model that captures these relationships while enabling maximum parallelism and providing clear semantics for failure handling.

## Decision

We will model task execution as a Directed Acyclic Graph (DAG) where:

- **Nodes** represent agent executions (tasks)
- **Edges** represent dependencies (data flow and ordering constraints)
- **Execution** proceeds by topological traversal with maximum parallelism

The DAG execution engine will:

1. **Parse** workflow definitions into validated DAG structures
2. **Schedule** ready tasks (those with all dependencies satisfied) in parallel
3. **Track** task states (pending, running, completed, failed, skipped)
4. **Propagate** outputs from completed tasks to dependent tasks
5. **Handle** failures according to configurable policies (fail-fast, continue, retry)
6. **Support** dynamic DAG modification (conditional branches, loops via unrolling)

DAGs are validated at compile time to ensure acyclicity and type compatibility between connected nodes.

## Consequences

### Positive

- Clear visualization of workflow structure and dependencies
- Maximum parallelism achieved automatically via topological scheduling
- Well-understood execution semantics from prior art (Airflow, Prefect, Dask)
- Natural representation of data flow between agents
- Compile-time validation prevents cycles and type mismatches
- Easy to reason about partial failures and recovery

### Negative

- Pure DAGs cannot represent cycles (loops require unrolling or separate constructs)
- Complex conditional logic requires careful DAG construction
- Dynamic DAG modification adds implementation complexity
- Large DAGs can be expensive to serialize/deserialize
- Memory overhead for maintaining full DAG state in large workflows

### Neutral

- Requires explicit dependency declaration (no implicit ordering)
- Forces decomposition of work into discrete tasks
- Creates natural checkpointing boundaries

## Alternatives Considered

### Linear Pipeline Model

A simple linear sequence of agents is easy to implement but cannot express parallelism or complex dependencies. This would leave significant performance on the table.

### Petri Nets

Petri nets can model complex concurrent systems including cycles. However, they are less intuitive for most developers and harder to visualize. The additional expressiveness is rarely needed.

### Actor Model

Pure actor-based systems (like Akka) offer flexibility but lack explicit structure. Debugging and visualizing actor message flows is difficult. DAGs provide clearer static analysis.

### Event-Driven Architecture

Event-driven systems are highly flexible but can lead to implicit dependencies that are hard to trace. DAGs make dependencies explicit and analyzable.

### Temporal Workflows

Temporal's replay-based model is powerful for long-running workflows but adds complexity. Our DAG model covers most use cases with simpler semantics. Integration with Temporal-style durability is a future consideration.

## References

- [Apache Airflow DAG Concepts](https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/dags.html)
- [Prefect Flow Design](https://docs.prefect.io/latest/concepts/flows/)
- [Dask Task Graphs](https://docs.dask.org/en/stable/graphs.html)
- [DAG-based Machine Learning Pipelines](https://www.kubeflow.org/docs/components/pipelines/)
