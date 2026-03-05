#!/usr/bin/env python3
"""
Apex Python SDK - DAG Workflow Example

This example demonstrates how to create and manage DAG (Directed Acyclic Graph)
workflows for complex multi-step task orchestration:
- Creating DAG definitions with nodes and edges
- Building multi-step workflows with dependencies
- Parallel task execution
- Workflow monitoring and control

Prerequisites:
    pip install apex-swarm

Run with:
    python dag_workflow.py
"""

import os
import sys
import time
from datetime import datetime

from apex_sdk import ApexClient
from apex_sdk.models import (
    DAGCreate,
    DAGUpdate,
    DAGNode,
    DAGEdge,
    DAGStatus,
    TaskCreate,
    TaskPriority,
    TaskStatus,
    TaskInput,
)
from apex_sdk.exceptions import ApexAPIError, ApexNotFoundError

# =============================================================================
# Configuration
# =============================================================================

API_URL = os.environ.get("APEX_API_URL", "http://localhost:8080")
API_KEY = os.environ.get("APEX_API_KEY", "")

# Initialize the client
client = ApexClient(
    base_url=API_URL,
    api_key=API_KEY,
    timeout=60.0,
)


# =============================================================================
# Simple Sequential DAG
# =============================================================================

def create_sequential_dag() -> str:
    """
    Create a simple sequential workflow: Research -> Analyze -> Report

    This demonstrates a linear workflow where each step depends on the previous one.

    Flow:
      [Research] --> [Analyze] --> [Report]
    """
    print("\n--- Creating Sequential DAG ---\n")

    dag = client.create_dag(
        DAGCreate(
            name="Research Pipeline",
            description="A sequential workflow for research, analysis, and reporting",
            nodes=[
                DAGNode(
                    id="research",
                    task_template=TaskCreate(
                        name="Research AI Trends",
                        description="Gather information about current AI trends",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(
                            data={
                                "topic": "AI agent architectures 2024",
                                "sources": ["academic", "industry", "news"],
                            }
                        ),
                        timeout_seconds=300,
                        retries=2,
                    ),
                    depends_on=[],
                ),
                DAGNode(
                    id="analyze",
                    task_template=TaskCreate(
                        name="Analyze Research Results",
                        description="Analyze the gathered research data",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(
                            data={
                                "analysis_type": "comprehensive",
                                "focus_areas": ["trends", "challenges", "opportunities"],
                            }
                        ),
                        timeout_seconds=180,
                        retries=1,
                    ),
                    depends_on=["research"],  # Depends on research node
                ),
                DAGNode(
                    id="report",
                    task_template=TaskCreate(
                        name="Generate Executive Summary",
                        description="Create a summary report from the analysis",
                        priority=TaskPriority.HIGH,
                        input=TaskInput(
                            data={
                                "format": "markdown",
                                "max_length": 2000,
                                "include_charts": True,
                            }
                        ),
                        timeout_seconds=120,
                        retries=1,
                    ),
                    depends_on=["analyze"],  # Depends on analyze node
                ),
            ],
            edges=[
                DAGEdge(source="research", target="analyze"),
                DAGEdge(source="analyze", target="report"),
            ],
            tags=["research", "sequential"],
            metadata={
                "project": "quarterly-research",
                "team": "research-ops",
            },
        )
    )

    print("Sequential DAG created:")
    print(f"  ID: {dag.id}")
    print(f"  Name: {dag.name}")
    print(f"  Nodes: {len(dag.nodes)}")
    print(f"  Status: {dag.status}")

    return dag.id


# =============================================================================
# Parallel Execution DAG
# =============================================================================

def create_parallel_dag() -> str:
    """
    Create a DAG with parallel execution branches.

    This demonstrates how independent tasks can run simultaneously
    and then converge for a final aggregation step.

    Flow:
                    +--> [Web Search] --+
      [Initialize] --+--> [Database Query] --+--> [Aggregate] --> [Format Output]
                    +--> [API Call] ----+
    """
    print("\n--- Creating Parallel Execution DAG ---\n")

    dag = client.create_dag(
        DAGCreate(
            name="Multi-Source Data Pipeline",
            description="Gather data from multiple sources in parallel",
            nodes=[
                # Initial setup node
                DAGNode(
                    id="init",
                    task_template=TaskCreate(
                        name="Pipeline Initialization",
                        description="Set up parameters and validate inputs",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"query": "AI market trends"}),
                    ),
                    depends_on=[],
                ),
                # Parallel data collection nodes
                DAGNode(
                    id="web-search",
                    task_template=TaskCreate(
                        name="Web Search",
                        description="Search the web for relevant information",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"search_engines": ["google", "bing"]}),
                        timeout_seconds=120,
                    ),
                    depends_on=["init"],
                ),
                DAGNode(
                    id="db-query",
                    task_template=TaskCreate(
                        name="Query Internal Database",
                        description="Search internal knowledge base",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"databases": ["knowledge_base", "reports"]}),
                        timeout_seconds=60,
                    ),
                    depends_on=["init"],
                ),
                DAGNode(
                    id="api-call",
                    task_template=TaskCreate(
                        name="Fetch External API Data",
                        description="Get data from external APIs",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"apis": ["market_data", "news_feed"]}),
                        timeout_seconds=90,
                    ),
                    depends_on=["init"],
                ),
                # Aggregation node - waits for all parallel tasks
                DAGNode(
                    id="aggregate",
                    task_template=TaskCreate(
                        name="Aggregate Data Sources",
                        description="Combine and deduplicate results from all sources",
                        priority=TaskPriority.HIGH,
                        input=TaskInput(data={"deduplication_strategy": "similarity"}),
                    ),
                    depends_on=["web-search", "db-query", "api-call"],  # All parallel nodes
                ),
                # Final output formatting
                DAGNode(
                    id="format",
                    task_template=TaskCreate(
                        name="Generate Final Report",
                        description="Format the aggregated data for output",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"output_format": "json"}),
                    ),
                    depends_on=["aggregate"],
                ),
            ],
            edges=[
                # Init leads to three parallel branches
                DAGEdge(source="init", target="web-search"),
                DAGEdge(source="init", target="db-query"),
                DAGEdge(source="init", target="api-call"),
                # All parallel branches converge to aggregate
                DAGEdge(source="web-search", target="aggregate"),
                DAGEdge(source="db-query", target="aggregate"),
                DAGEdge(source="api-call", target="aggregate"),
                # Aggregate leads to format
                DAGEdge(source="aggregate", target="format"),
            ],
            metadata={
                "project": "data-pipeline",
                "parallelism": 3,
            },
        )
    )

    print("Parallel DAG created:")
    print(f"  ID: {dag.id}")
    print(f"  Name: {dag.name}")
    print(f"  Nodes: {len(dag.nodes)}")
    print(f"  Parallel branches: 3")

    return dag.id


# =============================================================================
# Conditional Branching DAG
# =============================================================================

def create_conditional_dag() -> str:
    """
    Create a DAG with conditional branching based on task results.

    This simulates a workflow where different paths are taken
    based on the evaluation result.

    Flow:
      [Evaluate] --> [Fast Track] (if score > 80)
                 --> [Standard Review] --> [Manager Review] (if score <= 80)

      Both paths converge to [Finalize]
    """
    print("\n--- Creating Conditional Branching DAG ---\n")

    dag = client.create_dag(
        DAGCreate(
            name="Approval Workflow",
            description="Conditional approval workflow based on evaluation score",
            nodes=[
                DAGNode(
                    id="evaluate",
                    task_template=TaskCreate(
                        name="Evaluate Submission",
                        description="Score the submission based on criteria",
                        priority=TaskPriority.HIGH,
                        input=TaskInput(
                            data={"criteria": ["quality", "completeness", "accuracy"]}
                        ),
                    ),
                    depends_on=[],
                    condition=None,  # Entry point
                ),
                # Fast track path for high scores
                DAGNode(
                    id="fast-track",
                    task_template=TaskCreate(
                        name="Auto-Approve High Score",
                        description="Automatically approve high-scoring submissions",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"approval_type": "automatic"}),
                    ),
                    depends_on=["evaluate"],
                    condition="output.score > 80",  # Only runs if score > 80
                ),
                # Standard review path
                DAGNode(
                    id="standard-review",
                    task_template=TaskCreate(
                        name="Manual Review Required",
                        description="Flag for manual review",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"review_level": "standard"}),
                    ),
                    depends_on=["evaluate"],
                    condition="output.score <= 80",  # Only runs if score <= 80
                ),
                # Additional approval step for standard path
                DAGNode(
                    id="manager-review",
                    task_template=TaskCreate(
                        name="Manager Approval",
                        description="Get manager approval for the submission",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"approval_required": True}),
                    ),
                    depends_on=["standard-review"],
                ),
                # Final step - both paths converge here
                DAGNode(
                    id="finalize",
                    task_template=TaskCreate(
                        name="Complete Workflow",
                        description="Finalize the approval decision and notify stakeholders",
                        priority=TaskPriority.NORMAL,
                        input=TaskInput(data={"notify_stakeholders": True}),
                    ),
                    depends_on=["fast-track", "manager-review"],  # Either path can lead here
                ),
            ],
            edges=[
                DAGEdge(source="evaluate", target="fast-track", condition="output.score > 80"),
                DAGEdge(source="evaluate", target="standard-review", condition="output.score <= 80"),
                DAGEdge(source="standard-review", target="manager-review"),
                DAGEdge(source="fast-track", target="finalize"),
                DAGEdge(source="manager-review", target="finalize"),
            ],
            metadata={
                "workflow_type": "approval",
                "conditional_branching": True,
            },
        )
    )

    print("Conditional DAG created:")
    print(f"  ID: {dag.id}")
    print(f"  Name: {dag.name}")
    print(f"  Includes conditional branching")

    return dag.id


# =============================================================================
# DAG Execution and Monitoring
# =============================================================================

def start_dag_execution(dag_id: str) -> str:
    """
    Start a DAG execution and return the execution ID.
    """
    print("\n--- Starting DAG Execution ---\n")

    dag = client.start_dag(
        dag_id,
        input_data={
            "custom_input": "Test execution",
            "timestamp": datetime.now().isoformat(),
        },
    )

    print("DAG execution started:")
    print(f"  DAG ID: {dag.id}")
    print(f"  Status: {dag.status}")
    print(f"  Started: {dag.started_at}")

    return dag.id


def monitor_dag_progress(dag_id: str) -> None:
    """
    Monitor DAG execution progress with detailed status updates.
    """
    print("\n--- Monitoring DAG Progress ---\n")

    timeout = 600  # 10 minutes
    poll_interval = 3  # Check every 3 seconds
    start_time = time.time()

    previous_status = None

    while True:
        dag = client.get_dag(dag_id)

        # Check for status change
        current_status = f"{dag.status} - Nodes: {get_node_status_summary(dag)}"

        if current_status != previous_status:
            previous_status = current_status
            timestamp = datetime.now().strftime("%H:%M:%S")
            print(f"[{timestamp}] {current_status}")

            # Print individual node progress
            for task_status in dag.task_statuses:
                indicator = get_status_indicator(task_status.status)
                print(f"  {indicator} {task_status.node_id}: {task_status.status}")

        # Check if execution is complete
        if dag.status in [DAGStatus.COMPLETED.value, DAGStatus.FAILED.value]:
            print(f"\nDAG execution finished: {dag.status}")
            if dag.completed_at:
                duration = dag.completed_at - dag.started_at if dag.started_at else None
                print(f"  Duration: {duration}")
            break

        # Check timeout
        if time.time() - start_time > timeout:
            print("\nTimeout waiting for DAG completion")
            break

        time.sleep(poll_interval)


def get_node_status_summary(dag) -> str:
    """Get a summary of node statuses."""
    counts = {}
    for task_status in dag.task_statuses:
        status = task_status.status
        counts[status] = counts.get(status, 0) + 1

    return ", ".join(f"{status}:{count}" for status, count in counts.items())


def get_status_indicator(status: str) -> str:
    """Get a visual indicator for task status."""
    indicators = {
        TaskStatus.COMPLETED.value: "[OK]",
        TaskStatus.RUNNING.value: "[..]",
        TaskStatus.FAILED.value: "[XX]",
        TaskStatus.PENDING.value: "[--]",
        TaskStatus.QUEUED.value: "[--]",
        TaskStatus.PAUSED.value: "[||]",
    }
    return indicators.get(status, "[??]")


# =============================================================================
# DAG Management Operations
# =============================================================================

def list_dags() -> None:
    """List and filter DAGs."""
    print("\n--- List DAGs ---\n")

    # List all DAGs
    all_dags = client.list_dags()
    print(f"Total DAGs: {all_dags.total}")

    # List by status
    running_dags = client.list_dags(status=DAGStatus.RUNNING.value)
    print(f"Running DAGs: {running_dags.total}")

    completed_dags = client.list_dags(status=DAGStatus.COMPLETED.value)
    print(f"Completed DAGs: {completed_dags.total}")

    # Display DAG list
    print("\nDAG List:")
    for dag in all_dags.items:
        print(f"  - {dag.name} ({dag.id})")
        print(f"    Status: {dag.status}, Nodes: {len(dag.nodes)}")


def update_dag(dag_id: str) -> None:
    """Update a DAG definition."""
    print("\n--- Update DAG ---\n")

    updated = client.update_dag(
        dag_id,
        DAGUpdate(
            description="Updated description with additional details",
            metadata={
                "last_modified": datetime.now().isoformat(),
                "modified_by": "admin",
            },
        ),
    )

    print("DAG updated:")
    print(f"  ID: {updated.id}")
    print(f"  Description: {updated.description}")


def control_dag_operations(dag_id: str) -> None:
    """Demonstrate DAG control operations: pause, resume, cancel."""
    print("\n--- DAG Control Operations ---\n")

    # Pause a running DAG
    # paused = client.pause_dag(dag_id)
    # print(f"DAG paused: {paused.status}")

    # Resume a paused DAG
    # resumed = client.resume_dag(dag_id)
    # print(f"DAG resumed: {resumed.status}")

    # Cancel a running DAG
    # cancelled = client.cancel_dag(dag_id)
    # print(f"DAG cancelled: {cancelled.status}")

    print("Control operations available: pause, resume, cancel")


def delete_dag(dag_id: str) -> None:
    """Delete a DAG."""
    print(f"\nDeleting DAG: {dag_id}")
    client.delete_dag(dag_id)
    print("DAG deleted successfully")


# =============================================================================
# Complete Workflow Example
# =============================================================================

def run_complete_workflow() -> None:
    """Run a complete workflow: create DAG, execute, and monitor."""
    print("\n--- Running Complete Workflow ---\n")

    # Create the DAG
    dag_id = create_sequential_dag()

    # Start execution
    start_dag_execution(dag_id)

    # Monitor progress
    # Uncomment for real execution:
    # monitor_dag_progress(dag_id)

    # Clean up
    delete_dag(dag_id)


# =============================================================================
# Main Entry Point
# =============================================================================

def main() -> None:
    """Main function to run all examples."""
    print("=" * 60)
    print("Apex Python SDK - DAG Workflow Examples")
    print("=" * 60)

    try:
        # Health check first
        health = client.health()
        print(f"API Status: {health.status}")

        # Create different types of DAGs
        sequential_dag_id = create_sequential_dag()
        parallel_dag_id = create_parallel_dag()
        conditional_dag_id = create_conditional_dag()

        # List all DAGs
        list_dags()

        # Update a DAG
        update_dag(sequential_dag_id)

        # Control operations (demonstration)
        control_dag_operations(sequential_dag_id)

        # Run complete workflow (commented out - requires running server)
        # run_complete_workflow()

        # Clean up test DAGs
        print("\n--- Cleanup ---\n")
        delete_dag(sequential_dag_id)
        delete_dag(parallel_dag_id)
        delete_dag(conditional_dag_id)
        print("All test DAGs deleted")

        print("\n" + "=" * 60)
        print("All DAG examples completed successfully!")
        print("=" * 60)

    except ApexAPIError as e:
        print(f"\nAPI Error: {e.message}")
        sys.exit(1)
    except Exception as e:
        print(f"\nExample failed with error: {e}")
        sys.exit(1)
    finally:
        client.close()


if __name__ == "__main__":
    main()
