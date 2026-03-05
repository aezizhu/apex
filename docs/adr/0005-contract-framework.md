# ADR-0005: Agent Contract Framework

## Status

Accepted

## Date

2026-01-30

## Context

Apex agents are composed into workflows where the output of one agent becomes the input of another. Without clear interface definitions:

- Type mismatches between agents cause runtime failures
- Agents cannot be reliably reused across workflows
- Testing agents in isolation is difficult
- Documentation of agent capabilities is informal
- Validation of agent compatibility requires execution

We need a formal mechanism to define agent interfaces, enforce compatibility, and enable static analysis of workflows.

## Decision

We will implement a contract framework where each agent declares:

1. **Input Schema**: Typed specification of required and optional inputs
   ```python
   inputs = {
       "query": str,
       "context": Optional[List[Document]],
       "max_tokens": int = 1000
   }
   ```

2. **Output Schema**: Typed specification of outputs
   ```python
   outputs = {
       "response": str,
       "sources": List[Citation],
       "confidence": float
   }
   ```

3. **Preconditions**: Invariants that must hold before execution
   ```python
   preconditions = [
       lambda inputs: len(inputs["query"]) > 0,
       lambda inputs: inputs["max_tokens"] > 0
   ]
   ```

4. **Postconditions**: Invariants that must hold after execution
   ```python
   postconditions = [
       lambda outputs: 0.0 <= outputs["confidence"] <= 1.0,
       lambda outputs: len(outputs["response"]) > 0
   ]
   ```

5. **Resource Requirements**: Declared computational needs
   ```python
   resources = {
       "memory_mb": 512,
       "timeout_seconds": 30,
       "gpu": False
   }
   ```

Contracts are validated at workflow compile time (static type checking) and runtime (pre/postcondition assertions).

## Consequences

### Positive

- Compile-time detection of type mismatches between agents
- Self-documenting agent interfaces
- Enables automated testing via property-based generation
- Clear resource requirements for scheduling decisions
- Supports IDE autocompletion and type checking
- Facilitates agent marketplace and reuse

### Negative

- Additional ceremony when defining agents
- Preconditions/postconditions add runtime overhead
- Schema evolution requires careful versioning
- Complex types may be difficult to express
- Learning curve for contract specification

### Neutral

- Forces explicit interface design upfront
- Creates clear boundaries between agent responsibilities
- Requires tooling support for contract validation

## Alternatives Considered

### No Contracts (Duck Typing)

Python's duck typing allows flexible composition but defers all errors to runtime. This is particularly problematic in long-running workflows where type mismatches are discovered late.

### JSON Schema Only

JSON Schema provides type definitions but lacks support for preconditions, postconditions, and resource declarations. It also has awkward Python integration.

### Protocol Buffers / gRPC

Protobuf provides strong typing and cross-language support but is oriented toward RPC rather than local function composition. The generated code is less Pythonic.

### Pydantic Models Only

Pydantic handles input/output validation well but doesn't support preconditions, postconditions, or resource requirements. We will use Pydantic internally for schema validation while extending it with our contract features.

### Design by Contract Libraries

Python DbC libraries (like icontract) provide precondition/postcondition support but lack schema typing and resource declarations. Our framework integrates both.

## References

- [Design by Contract (Bertrand Meyer)](https://www.eiffel.com/values/design-by-contract/introduction/)
- [Pydantic Documentation](https://docs.pydantic.dev/)
- [Python Type Hints](https://docs.python.org/3/library/typing.html)
- [Property-Based Testing with Hypothesis](https://hypothesis.readthedocs.io/)
