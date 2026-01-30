# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for the Apex project.

## What is an ADR?

An Architecture Decision Record captures an important architectural decision made along with its context and consequences. ADRs provide a historical record of why decisions were made, enabling future team members to understand the rationale behind the system's design.

## ADR Index

| ID | Title | Status | Date |
|----|-------|--------|------|
| [0000](0000-adr-template.md) | ADR Template | Template | - |
| [0001](0001-rust-for-orchestration.md) | Rust for Core Orchestration Engine | Accepted | 2026-01-30 |
| [0002](0002-python-for-agents.md) | Python for Agent Execution | Accepted | 2026-01-30 |
| [0003](0003-dag-execution-model.md) | DAG-Based Task Execution Model | Accepted | 2026-01-30 |
| [0004](0004-frugalgpt-routing.md) | FrugalGPT Model Routing Strategy | Accepted | 2026-01-30 |
| [0005](0005-contract-framework.md) | Agent Contract Framework | Accepted | 2026-01-30 |
| [0006](0006-observability-stack.md) | Observability Architecture | Accepted | 2026-01-30 |

## Creating a New ADR

1. Copy `0000-adr-template.md` to a new file with the next available number
2. Fill in all sections with relevant details
3. Set the status to "Proposed"
4. Submit for review via pull request
5. Update status to "Accepted" once approved
6. Add entry to this README index

## ADR Statuses

- **Proposed**: Under discussion, not yet decided
- **Accepted**: Decision has been made and is in effect
- **Deprecated**: No longer applies but kept for historical reference
- **Superseded**: Replaced by a newer ADR (link to replacement)

## Key Decisions Summary

### Core Architecture

Apex uses a **hybrid Rust/Python architecture**:
- **Rust** powers the core orchestration engine for maximum performance and memory safety ([ADR-0001](0001-rust-for-orchestration.md))
- **Python** is used for agent implementation, leveraging the rich AI/ML ecosystem ([ADR-0002](0002-python-for-agents.md))

### Execution Model

Workflows are modeled as **Directed Acyclic Graphs (DAGs)** enabling automatic parallelization and clear dependency management ([ADR-0003](0003-dag-execution-model.md)).

### Cost Optimization

**FrugalGPT-inspired routing** dynamically selects the optimal model for each LLM request, balancing cost and quality ([ADR-0004](0004-frugalgpt-routing.md)).

### Agent Interfaces

A formal **contract framework** defines agent inputs, outputs, preconditions, and postconditions for compile-time validation ([ADR-0005](0005-contract-framework.md)).

### Observability

**OpenTelemetry-based observability** provides comprehensive tracing, metrics, and logging with LLM-specific extensions ([ADR-0006](0006-observability-stack.md)).
