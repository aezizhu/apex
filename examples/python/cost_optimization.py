#!/usr/bin/env python3
"""
Apex Python SDK - Cost Optimization with FrugalGPT Routing

This example demonstrates how to use the Apex SDK to implement
cost-aware task routing strategies inspired by the FrugalGPT approach:

- Cascading model selection: start with a cheap model, escalate to
  more expensive models only when quality is insufficient.
- Budget-constrained batch processing: distribute a fixed budget
  across a batch of tasks, choosing the cheapest adequate model for
  each.
- Quality/cost trade-off metadata: attach cost and quality estimates
  to tasks for downstream analytics.
- Adaptive routing: adjust model selection based on historical
  performance data.

FrugalGPT Strategy Overview:
    Instead of always routing every request to the most capable (and
    most expensive) LLM, FrugalGPT uses a cascade:

    1. Send the prompt to the cheapest model first.
    2. If a quality scorer judges the response adequate, use it.
    3. Otherwise, escalate to the next model tier.

    This can reduce costs by 50-90% while maintaining >95% of the
    quality of always using the top-tier model.

Prerequisites:
    pip install apex-swarm

Run with:
    python cost_optimization.py
"""

from __future__ import annotations

import asyncio
import os
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any

from apex_sdk import AsyncApexClient
from apex_sdk.models import (
    DAGCreate,
    DAGEdge,
    DAGNode,
    TaskCreate,
    TaskInput,
    TaskPriority,
    TaskStatus,
)
from apex_sdk.exceptions import ApexAPIError

# =============================================================================
# Configuration
# =============================================================================

API_URL: str = os.environ.get("APEX_API_URL", "http://localhost:8080")
API_KEY: str = os.environ.get("APEX_API_KEY", "")

# Model tier definitions: ordered from cheapest to most expensive.
# Each tier has a name, cost per 1K tokens, and an estimated quality
# score (0-100) on a representative benchmark.

@dataclass(frozen=True)
class ModelTier:
    """Represents a single LLM model tier in the cascade."""
    name: str
    cost_per_1k_tokens: float   # USD
    estimated_quality: float    # 0-100 benchmark score
    provider: str               # e.g. "openai", "anthropic"
    model_id: str               # e.g. "gpt-4o-mini"


# Ordered cheapest-first -- the cascade tries each in order.
MODEL_CASCADE: list[ModelTier] = [
    ModelTier(
        name="Tier 1 (Economy)",
        cost_per_1k_tokens=0.0002,
        estimated_quality=72.0,
        provider="openai",
        model_id="gpt-4o-mini",
    ),
    ModelTier(
        name="Tier 2 (Standard)",
        cost_per_1k_tokens=0.003,
        estimated_quality=86.0,
        provider="anthropic",
        model_id="claude-3-5-haiku-20241022",
    ),
    ModelTier(
        name="Tier 3 (Premium)",
        cost_per_1k_tokens=0.015,
        estimated_quality=95.0,
        provider="anthropic",
        model_id="claude-sonnet-4-20250514",
    ),
]

# Quality threshold: if a model's output scores below this, escalate.
QUALITY_THRESHOLD: float = float(
    os.environ.get("QUALITY_THRESHOLD", "80.0")
)

# Budget cap for batch processing (USD).
BATCH_BUDGET: float = float(
    os.environ.get("BATCH_BUDGET", "5.00")
)


# =============================================================================
# 1. Cascading Model Selection DAG
# =============================================================================

def build_cascade_dag(prompt: str, task_name: str = "FrugalGPT Cascade") -> DAGCreate:
    """Build a DAG that implements the FrugalGPT cascade strategy.

    The DAG has the following structure for a 3-tier cascade::

        [Tier1 Generate] -> [Tier1 Evaluate]
                                  |
                          (quality < threshold)
                                  v
                          [Tier2 Generate] -> [Tier2 Evaluate]
                                                    |
                                            (quality < threshold)
                                                    v
                                            [Tier3 Generate]
                                                    |
                                                    v
                                            [Aggregate Result]

    Each "Evaluate" node checks whether the output meets the quality
    threshold. If it does, downstream generate nodes are skipped
    (via conditional edges), and the pipeline jumps to the aggregation
    step.

    Args:
        prompt: The user prompt to process through the cascade.
        task_name: Human-readable name for this cascade run.

    Returns:
        A :class:`DAGCreate` ready to submit.
    """
    nodes: list[DAGNode] = []
    edges: list[DAGEdge] = []

    for i, tier in enumerate(MODEL_CASCADE):
        tier_idx = i + 1
        is_last = i == len(MODEL_CASCADE) - 1

        # -- Generate node: send the prompt to this tier's model ----------
        gen_id = f"tier{tier_idx}-generate"
        nodes.append(
            DAGNode(
                id=gen_id,
                task_template=TaskCreate(
                    name=f"{task_name} - {tier.name} Generate",
                    description=f"Generate response using {tier.model_id}",
                    priority=TaskPriority.NORMAL,
                    input=TaskInput(data={
                        "prompt": prompt,
                        "model": tier.model_id,
                        "provider": tier.provider,
                        "tier": tier_idx,
                        "cost_per_1k_tokens": tier.cost_per_1k_tokens,
                    }),
                    metadata={
                        "frugalgpt_tier": tier_idx,
                        "model": tier.model_id,
                        "estimated_cost_per_1k": tier.cost_per_1k_tokens,
                    },
                    timeout_seconds=120,
                    retries=1,
                ),
                depends_on=(
                    [f"tier{tier_idx - 1}-evaluate"] if i > 0 else []
                ),
                # Only run tier 2+ if the previous tier did not meet quality
                condition=(
                    f"output.quality_score < {QUALITY_THRESHOLD}"
                    if i > 0
                    else None
                ),
            )
        )

        # -- Evaluate node (skip for the last tier) -----------------------
        if not is_last:
            eval_id = f"tier{tier_idx}-evaluate"
            nodes.append(
                DAGNode(
                    id=eval_id,
                    task_template=TaskCreate(
                        name=f"{task_name} - {tier.name} Evaluate",
                        description=(
                            f"Score the {tier.model_id} response against "
                            f"quality threshold ({QUALITY_THRESHOLD})"
                        ),
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={
                            "quality_threshold": QUALITY_THRESHOLD,
                            "evaluation_criteria": [
                                "relevance",
                                "accuracy",
                                "completeness",
                                "coherence",
                            ],
                        }),
                        metadata={"frugalgpt_stage": "evaluate"},
                        timeout_seconds=60,
                    ),
                    depends_on=[gen_id],
                )
            )
            edges.append(DAGEdge(source=gen_id, target=eval_id))

    # -- Aggregate node: pick the best result and compute total cost -------
    # It depends on the last generate node AND all evaluate nodes,
    # because whichever path completes feeds into the aggregator.
    aggregate_deps: list[str] = []
    for i, tier in enumerate(MODEL_CASCADE):
        tier_idx = i + 1
        is_last = i == len(MODEL_CASCADE) - 1
        if is_last:
            aggregate_deps.append(f"tier{tier_idx}-generate")
        else:
            aggregate_deps.append(f"tier{tier_idx}-evaluate")

    nodes.append(
        DAGNode(
            id="aggregate",
            task_template=TaskCreate(
                name=f"{task_name} - Aggregate Result",
                description=(
                    "Select the best response from completed tiers and "
                    "compute total cost savings vs. always using the top tier"
                ),
                priority=TaskPriority.HIGH,
                input=TaskInput(data={
                    "model_cascade": [
                        {"tier": i + 1, "model": t.model_id, "cost": t.cost_per_1k_tokens}
                        for i, t in enumerate(MODEL_CASCADE)
                    ],
                    "quality_threshold": QUALITY_THRESHOLD,
                }),
                metadata={"frugalgpt_stage": "aggregate"},
            ),
            depends_on=aggregate_deps,
        )
    )

    # Wire evaluate -> next-tier-generate edges (with conditions)
    for i in range(len(MODEL_CASCADE) - 1):
        tier_idx = i + 1
        next_tier_idx = tier_idx + 1
        eval_id = f"tier{tier_idx}-evaluate"
        next_gen_id = f"tier{next_tier_idx}-generate"

        edges.append(
            DAGEdge(
                source=eval_id,
                target=next_gen_id,
                condition=f"output.quality_score < {QUALITY_THRESHOLD}",
            )
        )
        # Also wire evaluate -> aggregate (quality met, skip remaining)
        edges.append(
            DAGEdge(
                source=eval_id,
                target="aggregate",
                condition=f"output.quality_score >= {QUALITY_THRESHOLD}",
            )
        )

    # Last tier generate -> aggregate (always)
    last_gen = f"tier{len(MODEL_CASCADE)}-generate"
    edges.append(DAGEdge(source=last_gen, target="aggregate"))

    return DAGCreate(
        name=task_name,
        description=(
            "FrugalGPT-style cascading inference: starts with the cheapest "
            "model and escalates only when quality is insufficient."
        ),
        nodes=nodes,
        edges=edges,
        tags=["frugalgpt", "cost-optimization"],
        metadata={
            "strategy": "cascade",
            "quality_threshold": QUALITY_THRESHOLD,
            "model_tiers": len(MODEL_CASCADE),
        },
    )


# =============================================================================
# 2. Budget-Constrained Batch Processing
# =============================================================================

@dataclass
class BatchItem:
    """A single item in a cost-optimized batch."""
    prompt: str
    estimated_tokens: int
    assigned_tier: ModelTier | None = None
    estimated_cost: float = 0.0


def plan_budget_allocation(
    items: list[BatchItem],
    budget: float,
    tiers: list[ModelTier],
) -> list[BatchItem]:
    """Assign each item to the cheapest tier that fits within the budget.

    The algorithm is greedy:
    1. Sort items by estimated token count (largest first).
    2. For each item, try tiers from cheapest to most expensive.
    3. Assign the cheapest tier whose accumulated cost still fits in
       the remaining budget.
    4. If even the cheapest tier exceeds the budget, skip the item.

    Args:
        items: List of :class:`BatchItem` to process.
        budget: Total budget in USD.
        tiers: Available model tiers, ordered cheapest-first.

    Returns:
        The same list with ``assigned_tier`` and ``estimated_cost`` set.
    """
    remaining = budget
    # Process largest items first so small items can fill gaps later
    sorted_items = sorted(items, key=lambda x: x.estimated_tokens, reverse=True)

    for item in sorted_items:
        for tier in tiers:
            cost = (item.estimated_tokens / 1000.0) * tier.cost_per_1k_tokens
            if cost <= remaining:
                item.assigned_tier = tier
                item.estimated_cost = cost
                remaining -= cost
                break
        # If no tier fits, item.assigned_tier stays None (skipped)

    return items


async def run_budget_batch(
    client: AsyncApexClient,
    prompts: list[str],
    budget: float,
) -> dict[str, Any]:
    """Execute a batch of prompts under a fixed budget constraint.

    Demonstrates the full workflow:
    1. Estimate token counts for each prompt.
    2. Allocate budget across prompts.
    3. Submit tasks with the assigned model tier.
    4. Collect results and report cost savings.

    Args:
        client: An authenticated :class:`AsyncApexClient`.
        prompts: List of prompt strings to process.
        budget: Maximum budget in USD.

    Returns:
        A summary dict with ``total_cost``, ``items_processed``,
        ``items_skipped``, and ``savings_vs_premium``.
    """
    # Step 1: Estimate tokens (rough heuristic: 1 token ~ 4 characters)
    items = [
        BatchItem(prompt=p, estimated_tokens=max(len(p) // 4, 50))
        for p in prompts
    ]

    # Step 2: Allocate budget
    plan_budget_allocation(items, budget, MODEL_CASCADE)

    # Step 3: Submit tasks for items that received an assignment
    task_ids: list[str] = []
    total_estimated_cost = 0.0
    items_skipped = 0

    for i, item in enumerate(items):
        if item.assigned_tier is None:
            items_skipped += 1
            print(f"  [SKIP] Item {i + 1}: exceeds remaining budget")
            continue

        task = await client.create_task(
            TaskCreate(
                name=f"Budget Batch Item {i + 1}",
                description=f"Process with {item.assigned_tier.model_id}",
                priority=TaskPriority.NORMAL,
                input=TaskInput(data={
                    "prompt": item.prompt,
                    "model": item.assigned_tier.model_id,
                    "provider": item.assigned_tier.provider,
                }),
                metadata={
                    "batch_index": i,
                    "assigned_tier": item.assigned_tier.name,
                    "estimated_cost_usd": round(item.estimated_cost, 6),
                    "estimated_tokens": item.estimated_tokens,
                },
                tags=["budget-batch", "frugalgpt"],
            )
        )
        task_ids.append(task.id)
        total_estimated_cost += item.estimated_cost
        print(
            f"  [SUBMIT] Item {i + 1}: {item.assigned_tier.name} "
            f"(~${item.estimated_cost:.4f})"
        )

    # Step 4: Compute savings vs. always using the premium tier
    premium_tier = MODEL_CASCADE[-1]
    premium_cost = sum(
        (item.estimated_tokens / 1000.0) * premium_tier.cost_per_1k_tokens
        for item in items
        if item.assigned_tier is not None
    )
    savings_pct = (
        ((premium_cost - total_estimated_cost) / premium_cost * 100)
        if premium_cost > 0
        else 0.0
    )

    summary = {
        "total_estimated_cost": round(total_estimated_cost, 4),
        "items_processed": len(task_ids),
        "items_skipped": items_skipped,
        "premium_cost_estimate": round(premium_cost, 4),
        "savings_vs_premium_pct": round(savings_pct, 1),
        "task_ids": task_ids,
    }

    return summary


# =============================================================================
# 3. Cost Analytics Report
# =============================================================================

def print_cost_report(summary: dict[str, Any]) -> None:
    """Print a formatted cost analytics report."""
    print("\n" + "=" * 60)
    print("  COST OPTIMIZATION REPORT")
    print("=" * 60)
    print(f"\n  Items processed:      {summary['items_processed']}")
    print(f"  Items skipped:        {summary['items_skipped']}")
    print(f"  Estimated cost:       ${summary['total_estimated_cost']:.4f}")
    print(f"  Premium-only cost:    ${summary['premium_cost_estimate']:.4f}")
    print(f"  Savings vs premium:   {summary['savings_vs_premium_pct']:.1f}%")
    print(f"\n  Model tier breakdown:")

    for tier in MODEL_CASCADE:
        print(f"    {tier.name}: ${tier.cost_per_1k_tokens}/1K tokens "
              f"(quality ~{tier.estimated_quality})")

    print()
    print("  Tip: Adjust QUALITY_THRESHOLD to trade off cost vs. quality.")
    print("       Lower threshold = more items handled by cheap models.")
    print("=" * 60)


# =============================================================================
# Main
# =============================================================================

async def main() -> None:
    """Demonstrate cost optimization strategies."""
    print("=" * 60)
    print("  Apex SDK - FrugalGPT Cost Optimization")
    print("=" * 60)

    async with AsyncApexClient(
        base_url=API_URL,
        api_key=API_KEY,
        timeout=60.0,
    ) as client:
        # -- Example 1: Build and submit a cascade DAG --------------------
        print("\n--- Cascade DAG ---\n")

        cascade_dag_def = build_cascade_dag(
            prompt="Explain the key differences between transformer and "
                   "state-space model architectures for sequence modeling.",
            task_name="Architecture Comparison Cascade",
        )

        print(f"DAG: {cascade_dag_def.name}")
        print(f"Nodes: {len(cascade_dag_def.nodes)}")
        print(f"Quality threshold: {QUALITY_THRESHOLD}")

        try:
            dag = await client.create_dag(cascade_dag_def)
            print(f"Created DAG: {dag.id}")

            # Start and let the orchestrator handle the cascade
            running = await client.start_dag(dag.id)
            print(f"Status: {running.status}")

            # Cleanup
            await client.delete_dag(dag.id)
            print("DAG cleaned up")
        except ApexAPIError as e:
            print(f"(API unavailable for demo: {e.message})")

        # -- Example 2: Budget-constrained batch --------------------------
        print("\n--- Budget-Constrained Batch ---\n")
        print(f"Budget: ${BATCH_BUDGET:.2f}")

        sample_prompts = [
            "What is photosynthesis?",
            "Summarize the key points of the Attention Is All You Need paper.",
            "Write a Python function to compute the Fibonacci sequence.",
            "Explain quantum entanglement to a 10-year-old.",
            "Compare and contrast REST and GraphQL API design patterns.",
            "Draft a project proposal for an autonomous drone delivery system.",
            "What are the environmental impacts of lithium mining?",
            "Translate 'Hello, how are you?' into Japanese, Korean, and Mandarin.",
        ]

        try:
            summary = await run_budget_batch(client, sample_prompts, BATCH_BUDGET)
            print_cost_report(summary)

            # Cleanup demo tasks
            for tid in summary.get("task_ids", []):
                try:
                    await client.delete_task(tid)
                except Exception:
                    pass
            print("\nDemo tasks cleaned up")
        except ApexAPIError as e:
            print(f"(API unavailable for demo: {e.message})")

    print("\n" + "=" * 60)
    print("  Cost optimization examples completed")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
