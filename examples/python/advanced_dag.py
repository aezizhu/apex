#!/usr/bin/env python3
"""
Apex Python SDK - Advanced DAG with Conditional Branches

This example demonstrates building complex DAG workflows that include:
- Multi-level conditional branching based on task output
- Error-handling nodes with fallback paths
- Parallel fan-out / fan-in patterns
- Dynamic node configuration using metadata propagation
- Approval gates embedded within a DAG
- Scheduled (cron-based) DAG execution

The DAG modeled here represents an ML model evaluation pipeline:

  [Ingest Data]
       |
  [Validate Schema]
       |
  [Feature Engineering] -----> [Train Model A] --+
       |                                          |
       +----> [Train Model B] ---+                |
       |                         +--> [Compare Models]
       +----> [Train Model C] ---+         |
                                      (condition)
                                     /          \
                            [Deploy Best]   [Request Human Review]
                                     \          /
                                      [Notify Stakeholders]

Prerequisites:
    pip install apex-swarm

Run with:
    python advanced_dag.py
"""

from __future__ import annotations

import os
import sys
from typing import Any

from apex_sdk import ApexClient
from apex_sdk.models import (
    DAGCreate,
    DAGEdge,
    DAGNode,
    DAGStatus,
    TaskCreate,
    TaskInput,
    TaskPriority,
    TaskStatus,
)
from apex_sdk.exceptions import ApexAPIError, ApexNotFoundError

# =============================================================================
# Configuration
# =============================================================================

API_URL: str = os.environ.get("APEX_API_URL", "http://localhost:8080")
API_KEY: str = os.environ.get("APEX_API_KEY", "")


def build_ml_pipeline_dag() -> DAGCreate:
    """Construct the full ML evaluation pipeline DAG definition.

    The pipeline performs the following stages:

    1. **Ingest** -- Load raw data from configured sources.
    2. **Validate** -- Run schema and quality checks.
    3. **Feature Engineering** -- Transform raw features into model-ready
       representations.
    4. **Train (parallel)** -- Train three candidate models concurrently
       (Model A, B, C) with different hyperparameters.
    5. **Compare** -- Evaluate all models on a held-out test set.
    6. **Conditional branch** -- If the best model exceeds a quality
       threshold, deploy it automatically; otherwise, request human
       review before proceeding.
    7. **Notify** -- Send results to stakeholders regardless of path.

    Returns:
        A fully configured :class:`DAGCreate` ready for submission.
    """
    # -- Helper to build a task-template node quickly -------------------------

    def _task_node(
        node_id: str,
        name: str,
        description: str,
        deps: list[str],
        *,
        priority: TaskPriority = TaskPriority.NORMAL,
        input_data: dict[str, Any] | None = None,
        timeout: int = 300,
        retries: int = 1,
        condition: str | None = None,
    ) -> DAGNode:
        """Create a DAGNode wrapping a TaskCreate template."""
        return DAGNode(
            id=node_id,
            task_template=TaskCreate(
                name=name,
                description=description,
                priority=priority,
                input=TaskInput(data=input_data or {}),
                timeout_seconds=timeout,
                retries=retries,
                tags=["ml-pipeline"],
            ),
            depends_on=deps,
            condition=condition,
        )

    # -- Node definitions -----------------------------------------------------

    nodes: list[DAGNode] = [
        # Stage 1: Data ingestion
        _task_node(
            "ingest",
            "Ingest Raw Data",
            "Load data from S3 bucket and streaming sources",
            deps=[],
            input_data={
                "sources": ["s3://ml-data/raw/", "kafka://events-topic"],
                "format": "parquet",
                "sample_rate": 1.0,
            },
            timeout=600,
        ),
        # Stage 2: Schema validation
        _task_node(
            "validate",
            "Validate Data Schema",
            "Run schema checks, null-rate analysis, and distribution drift detection",
            deps=["ingest"],
            input_data={
                "checks": ["schema", "null_rate", "drift"],
                "drift_threshold": 0.05,
                "fail_on_warning": False,
            },
        ),
        # Stage 3: Feature engineering
        _task_node(
            "features",
            "Feature Engineering",
            "Transform raw columns into ML-ready features",
            deps=["validate"],
            input_data={
                "transformations": [
                    "one_hot_encode",
                    "standard_scale",
                    "interaction_terms",
                ],
                "target_column": "label",
                "test_split": 0.2,
            },
            timeout=900,
        ),
        # Stage 4a: Train Model A (e.g. gradient-boosted tree)
        _task_node(
            "train-model-a",
            "Train Model A (XGBoost)",
            "Train an XGBoost classifier with grid-search hyperparameter tuning",
            deps=["features"],
            priority=TaskPriority.HIGH,
            input_data={
                "algorithm": "xgboost",
                "hyperparams": {
                    "max_depth": [3, 5, 7],
                    "learning_rate": [0.01, 0.1],
                    "n_estimators": [100, 300],
                },
                "cv_folds": 5,
            },
            timeout=1800,
            retries=2,
        ),
        # Stage 4b: Train Model B (e.g. random forest)
        _task_node(
            "train-model-b",
            "Train Model B (Random Forest)",
            "Train a random-forest classifier",
            deps=["features"],
            priority=TaskPriority.HIGH,
            input_data={
                "algorithm": "random_forest",
                "hyperparams": {
                    "n_estimators": [100, 500],
                    "max_features": ["sqrt", "log2"],
                },
                "cv_folds": 5,
            },
            timeout=1800,
            retries=2,
        ),
        # Stage 4c: Train Model C (e.g. neural network)
        _task_node(
            "train-model-c",
            "Train Model C (MLP)",
            "Train a multi-layer perceptron classifier",
            deps=["features"],
            priority=TaskPriority.HIGH,
            input_data={
                "algorithm": "mlp",
                "hyperparams": {
                    "hidden_layers": [(128, 64), (256, 128, 64)],
                    "dropout": [0.2, 0.3],
                    "epochs": 50,
                },
            },
            timeout=2400,
            retries=2,
        ),
        # Stage 5: Compare models (fan-in from all three training nodes)
        _task_node(
            "compare",
            "Compare Model Performance",
            "Evaluate all candidate models on the test set and rank by F1 score",
            deps=["train-model-a", "train-model-b", "train-model-c"],
            priority=TaskPriority.HIGH,
            input_data={
                "metrics": ["f1", "precision", "recall", "auc"],
                "primary_metric": "f1",
                "quality_threshold": 0.85,
            },
        ),
        # Stage 6a: Auto-deploy if quality threshold met
        # The condition expression references the compare node's output.
        _task_node(
            "deploy",
            "Deploy Best Model",
            "Package and deploy the winning model to the serving infrastructure",
            deps=["compare"],
            condition="output.best_f1 >= 0.85",
            input_data={
                "registry": "mlflow",
                "serving": "kubernetes",
                "canary_percent": 10,
            },
        ),
        # Stage 6b: Human review if threshold not met
        _task_node(
            "human-review",
            "Request Human Review",
            "Notify ML engineers for manual inspection when quality is below threshold",
            deps=["compare"],
            condition="output.best_f1 < 0.85",
            input_data={
                "notify_channels": ["slack", "email"],
                "review_dashboard_url": "https://ml.apex.internal/review",
            },
        ),
        # Stage 7: Notify stakeholders (both paths converge here)
        _task_node(
            "notify",
            "Notify Stakeholders",
            "Send summary report to project stakeholders",
            deps=["deploy", "human-review"],
            input_data={
                "channels": ["slack", "email"],
                "report_format": "html",
                "include_charts": True,
            },
        ),
    ]

    # -- Edge definitions (explicit graph wiring) -----------------------------

    edges: list[DAGEdge] = [
        # Sequential stages
        DAGEdge(source="ingest", target="validate"),
        DAGEdge(source="validate", target="features"),
        # Fan-out: features -> three parallel training nodes
        DAGEdge(source="features", target="train-model-a"),
        DAGEdge(source="features", target="train-model-b"),
        DAGEdge(source="features", target="train-model-c"),
        # Fan-in: all training nodes -> compare
        DAGEdge(source="train-model-a", target="compare"),
        DAGEdge(source="train-model-b", target="compare"),
        DAGEdge(source="train-model-c", target="compare"),
        # Conditional branches from compare
        DAGEdge(
            source="compare",
            target="deploy",
            condition="output.best_f1 >= 0.85",
        ),
        DAGEdge(
            source="compare",
            target="human-review",
            condition="output.best_f1 < 0.85",
        ),
        # Both conditional branches converge to notify
        DAGEdge(source="deploy", target="notify"),
        DAGEdge(source="human-review", target="notify"),
    ]

    return DAGCreate(
        name="ML Model Evaluation Pipeline",
        description=(
            "End-to-end pipeline: ingest data, validate, engineer features, "
            "train three candidate models in parallel, compare results, and "
            "conditionally deploy or escalate for human review."
        ),
        nodes=nodes,
        edges=edges,
        tags=["ml", "pipeline", "advanced"],
        metadata={
            "team": "ml-platform",
            "version": "2.1",
            "estimated_duration_minutes": 45,
        },
        # Optional: run daily at 02:00 UTC
        schedule="0 2 * * *",
    )


def print_dag_structure(dag_def: DAGCreate) -> None:
    """Print a human-readable representation of the DAG topology."""
    print(f"DAG: {dag_def.name}")
    print(f"Description: {dag_def.description}")
    print(f"Schedule: {dag_def.schedule or 'manual'}")
    print(f"Nodes ({len(dag_def.nodes)}):")

    for node in dag_def.nodes:
        deps = ", ".join(node.depends_on) if node.depends_on else "(root)"
        cond = f"  [condition: {node.condition}]" if node.condition else ""
        print(f"  - {node.id}: {node.task_template.name}  deps=[{deps}]{cond}")

    print(f"\nEdges ({len(dag_def.edges or [])}):")
    for edge in dag_def.edges or []:
        cond = f"  [if {edge.condition}]" if edge.condition else ""
        print(f"  {edge.source} -> {edge.target}{cond}")


# =============================================================================
# Main
# =============================================================================

def main() -> None:
    """Build, submit, and start the advanced DAG pipeline."""
    print("=" * 70)
    print("  Apex SDK - Advanced DAG with Conditional Branches")
    print("=" * 70)

    # Build the DAG definition locally before submitting
    dag_def = build_ml_pipeline_dag()
    print("\n--- DAG Definition ---\n")
    print_dag_structure(dag_def)

    # Submit the DAG to the Apex API
    client = ApexClient(base_url=API_URL, api_key=API_KEY, timeout=60.0)

    try:
        print("\n--- Submitting DAG ---\n")
        dag = client.create_dag(dag_def)
        print(f"DAG created: {dag.id} ({dag.name})")
        print(f"Status: {dag.status}")

        # Start execution with initial input parameters
        print("\n--- Starting Execution ---\n")
        running_dag = client.start_dag(
            dag.id,
            input_data={
                "run_id": "eval-2024-q4-001",
                "data_snapshot": "2024-12-01",
            },
        )
        print(f"DAG running: {running_dag.status}")
        print(f"Started at: {running_dag.started_at}")

        # In production you would monitor via WebSocket or polling.
        # See monitoring.py for a real-time monitoring example.

        # Cleanup for demo purposes
        print("\n--- Cleanup ---\n")
        client.delete_dag(dag.id)
        print("DAG deleted")

    except ApexAPIError as e:
        print(f"API error: {e.message}")
        sys.exit(1)
    except Exception as e:
        print(f"Unexpected error: {e}")
        sys.exit(1)
    finally:
        client.close()

    print("\n" + "=" * 70)
    print("  Advanced DAG example completed")
    print("=" * 70)


if __name__ == "__main__":
    main()
