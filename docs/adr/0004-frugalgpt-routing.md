# ADR-0004: FrugalGPT Model Routing Strategy

## Status

Accepted

## Date

2026-01-30

## Context

Apex agents make numerous LLM calls across workflows. Different tasks have varying complexity and quality requirements:

- Simple classification tasks may work well with smaller, cheaper models
- Complex reasoning tasks may require frontier models
- Some tasks are latency-sensitive, others are throughput-oriented
- Cost can vary 100x between model tiers
- Quality requirements differ by use case

Naively routing all requests to the most capable (and expensive) model wastes resources. Routing everything to the cheapest model sacrifices quality. We need an intelligent routing strategy.

## Decision

We will implement a FrugalGPT-inspired model routing system that dynamically selects the optimal model for each request based on:

1. **Task Complexity Analysis**: Estimate task difficulty using input features (length, domain keywords, required capabilities)

2. **Cascade Routing**: Start with cheaper models and escalate to more expensive ones only when confidence is low:
   - Route to small model first
   - If confidence < threshold, retry with medium model
   - If still uncertain, escalate to frontier model

3. **Quality Scoring**: Learn a quality prediction model that estimates expected output quality for each (task, model) pair

4. **Cost-Aware Optimization**: Optimize for quality subject to budget constraints, or minimize cost subject to quality constraints

5. **Adaptive Learning**: Continuously update routing decisions based on observed outcomes and user feedback

The router will support multiple providers (OpenAI, Anthropic, Google, open-source) and model tiers (small, medium, large, frontier).

## Consequences

### Positive

- Significant cost reduction (typically 50-90% based on FrugalGPT research)
- Maintains quality by escalating complex tasks to better models
- Reduces latency for simple tasks by using faster small models
- Enables budget-aware workflow execution
- Provides automatic model selection, reducing developer burden
- Graceful degradation when specific models are unavailable

### Negative

- Additional latency from cascade retries on complex tasks
- Complexity in maintaining quality prediction models
- Requires historical data to train routing models effectively
- May make suboptimal decisions for novel task types
- Increased system complexity vs. single-model approach

### Neutral

- Requires defining quality metrics for evaluation
- Creates dependency on multiple model providers
- Introduces routing decisions as a source of non-determinism

## Alternatives Considered

### Single Model Approach

Using a single model for all requests is simple but either wastes money (if using top-tier) or sacrifices quality (if using budget models). This doesn't leverage the diversity of available models.

### Static Routing Rules

Hardcoded rules (e.g., "use GPT-4 for summarization, GPT-3.5 for classification") are inflexible and require manual maintenance as models evolve. They also can't adapt to task-specific complexity.

### User-Specified Models

Requiring users to specify models for each agent increases cognitive load and leads to suboptimal choices. Most users don't have intuition for model capabilities.

### Cost-Only Optimization

Always choosing the cheapest model that meets minimum requirements ignores the quality-cost tradeoff. Some tasks benefit from higher quality even at increased cost.

### Ensemble Approaches

Running multiple models and combining outputs (ensemble methods) provides robustness but multiplies cost. Cascade routing achieves similar benefits more efficiently.

## References

- [FrugalGPT: How to Use Large Language Models While Reducing Cost and Improving Performance](https://arxiv.org/abs/2305.05176)
- [RouteLLM: Learning to Route LLMs with Preference Data](https://arxiv.org/abs/2406.18665)
- [Optimizing LLM Cost and Quality](https://www.anthropic.com/news/optimizing-llm-usage)
- [LLM Cascade Architectures](https://arxiv.org/abs/2310.03094)
