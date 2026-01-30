#!/usr/bin/env python3
"""
Apex Python SDK - Basic Usage Example

This example demonstrates fundamental operations with the Apex SDK:
- Client initialization
- Health checks
- Task creation and management
- Agent operations
- Error handling

Prerequisites:
    pip install apex-swarm

Run with:
    python basic_usage.py
"""

import os
import sys
import time
from datetime import datetime

# Import the Apex SDK
from apex_sdk import ApexClient, AsyncApexClient
from apex_sdk.models import (
    TaskCreate,
    TaskUpdate,
    TaskStatus,
    TaskPriority,
    TaskInput,
    AgentCreate,
    AgentUpdate,
    AgentStatus,
)
from apex_sdk.exceptions import (
    ApexAPIError,
    ApexAuthenticationError,
    ApexAuthorizationError,
    ApexNotFoundError,
    ApexValidationError,
    ApexRateLimitError,
    ApexServerError,
    ApexTimeoutError,
)

# =============================================================================
# Configuration
# =============================================================================

# Load configuration from environment variables
API_URL = os.environ.get("APEX_API_URL", "http://localhost:8080")
API_KEY = os.environ.get("APEX_API_KEY", "")


# =============================================================================
# Client Initialization
# =============================================================================

def initialize_client() -> ApexClient:
    """
    Create and configure an Apex client instance.

    The client handles authentication, retries, and error transformation.
    """
    # Method 1: Using direct initialization
    client = ApexClient(
        base_url=API_URL,
        api_key=API_KEY,
        timeout=30.0,       # 30 seconds timeout
        max_retries=3,      # Retry failed requests up to 3 times
        retry_delay=1.0,    # Initial retry delay of 1 second
    )

    return client


def initialize_client_with_context_manager() -> ApexClient:
    """
    Alternative: Using context manager for automatic cleanup.
    """
    # This client will be automatically closed when exiting the context
    return ApexClient(
        base_url=API_URL,
        api_key=API_KEY,
    )


# =============================================================================
# Health Check Example
# =============================================================================

def check_health(client: ApexClient) -> None:
    """
    Check the API health status.

    This is useful for:
    - Verifying connectivity before operations
    - Monitoring service health
    - Checking dependent service status (database, queue, etc.)
    """
    print("\n--- Health Check ---\n")

    try:
        health = client.health()

        print(f"API Status: {health.status}")
        print(f"Version: {health.version}")
        print(f"Uptime: {int(health.uptime / 60)} minutes")
        print("Services:")
        for service_name, status in health.services.items():
            print(f"  - {service_name}: {status}")

        # Check if all services are healthy
        all_healthy = all(s == "up" for s in health.services.values())
        if not all_healthy:
            print("\nWarning: Some services are not fully operational")

    except Exception as e:
        print(f"Health check failed: {e}")
        raise


# =============================================================================
# Task Examples
# =============================================================================

def create_task_example(client: ApexClient) -> str:
    """
    Create a new task.

    Tasks are the basic unit of work in Apex. Each task represents
    a single operation to be executed by an agent.
    """
    print("\n--- Create Task ---\n")

    try:
        # Create task input
        task_input = TaskInput(
            data={
                "topic": "AI agent architectures",
                "depth": "comprehensive",
                "format": "markdown",
            }
        )

        # Create the task
        task = client.create_task(
            TaskCreate(
                name="Research AI Trends",
                description="Research and summarize the latest trends in AI agent architectures",
                priority=TaskPriority.NORMAL,
                input=task_input,
                timeout_seconds=120,
                retries=2,
                tags=["research", "ai"],
                metadata={
                    "project": "research-initiative",
                    "requested_by": "user-123",
                },
            )
        )

        print("Task created successfully:")
        print(f"  ID: {task.id}")
        print(f"  Name: {task.name}")
        print(f"  Status: {task.status}")
        print(f"  Priority: {task.priority}")
        print(f"  Created: {task.created_at}")

        return task.id

    except Exception as e:
        print(f"Failed to create task: {e}")
        raise


def list_tasks_example(client: ApexClient) -> None:
    """
    List tasks with filtering and pagination.

    Use filters to find specific tasks based on status, priority, etc.
    """
    print("\n--- List Tasks ---\n")

    try:
        # List all pending tasks
        pending_tasks = client.list_tasks(
            status=TaskStatus.PENDING.value,
            page=1,
            per_page=10,
        )

        print(f"Found {pending_tasks.total} pending tasks:")
        for task in pending_tasks.items:
            print(f"  - {task.name} ({task.id})")

        # List tasks with specific tags
        tagged_tasks = client.list_tasks(
            tags=["research"],
            page=1,
            per_page=10,
        )

        print(f"\nFound {tagged_tasks.total} tasks with 'research' tag")

        # List all running tasks
        running_tasks = client.list_tasks(
            status=TaskStatus.RUNNING.value,
        )

        print(f"\nFound {running_tasks.total} running tasks")

    except Exception as e:
        print(f"Failed to list tasks: {e}")
        raise


def get_task_example(client: ApexClient, task_id: str) -> None:
    """
    Get detailed information about a specific task.
    """
    print("\n--- Get Task Details ---\n")

    try:
        task = client.get_task(task_id)

        print("Task details:")
        print(f"  ID: {task.id}")
        print(f"  Name: {task.name}")
        print(f"  Description: {task.description}")
        print(f"  Status: {task.status}")
        print(f"  Priority: {task.priority}")
        print(f"  Agent ID: {task.agent_id or 'Not assigned'}")
        print(f"  Created: {task.created_at}")
        print(f"  Updated: {task.updated_at}")

        if task.started_at:
            print(f"  Started: {task.started_at}")
        if task.completed_at:
            print(f"  Completed: {task.completed_at}")

        if task.input:
            print(f"  Input: {task.input.data}")

        if task.output:
            print(f"  Output: {task.output.result}")

        if task.error:
            print(f"  Error: {task.error.code} - {task.error.message}")

    except ApexNotFoundError:
        print(f"Task {task_id} not found")
    except Exception as e:
        print(f"Failed to get task: {e}")
        raise


def update_task_example(client: ApexClient, task_id: str) -> None:
    """
    Update task properties.
    """
    print("\n--- Update Task ---\n")

    try:
        updated_task = client.update_task(
            task_id,
            TaskUpdate(
                priority=TaskPriority.HIGH,
                metadata={
                    "escalated": True,
                    "reason": "urgent deadline",
                },
            ),
        )

        print(f"Task updated successfully:")
        print(f"  ID: {updated_task.id}")
        print(f"  New Priority: {updated_task.priority}")
        print(f"  Updated: {updated_task.updated_at}")

    except Exception as e:
        print(f"Failed to update task: {e}")
        raise


def wait_for_task_example(client: ApexClient, task_id: str) -> None:
    """
    Wait for a task to complete using polling.

    For real-time updates, consider using WebSockets instead.
    """
    print("\n--- Wait for Task ---\n")

    try:
        print("Waiting for task to complete...")

        timeout = 120  # 2 minutes
        poll_interval = 2  # Check every 2 seconds
        start_time = time.time()

        while True:
            task = client.get_task(task_id)

            # Check for completion states
            if task.status == TaskStatus.COMPLETED.value:
                print(f"\nTask completed successfully!")
                print(f"  Duration: {time.time() - start_time:.1f}s")
                if task.output:
                    print(f"  Output: {task.output.result}")
                break

            elif task.status in [TaskStatus.FAILED.value, TaskStatus.CANCELLED.value]:
                print(f"\nTask {task.status}!")
                if task.error:
                    print(f"  Error: {task.error.message}")
                break

            # Check timeout
            if time.time() - start_time > timeout:
                print("\nTimeout waiting for task completion")
                break

            # Show progress
            print(f"  Status: {task.status}")
            time.sleep(poll_interval)

    except Exception as e:
        print(f"Error while waiting for task: {e}")
        raise


def cancel_task_example(client: ApexClient, task_id: str) -> None:
    """
    Cancel a running task.
    """
    print("\n--- Cancel Task ---\n")

    try:
        cancelled_task = client.cancel_task(task_id)

        print(f"Task cancelled:")
        print(f"  ID: {cancelled_task.id}")
        print(f"  Status: {cancelled_task.status}")

    except Exception as e:
        print(f"Failed to cancel task: {e}")
        raise


def retry_task_example(client: ApexClient, task_id: str) -> None:
    """
    Retry a failed task.
    """
    print("\n--- Retry Task ---\n")

    try:
        retried_task = client.retry_task(task_id)

        print(f"Task retry initiated:")
        print(f"  ID: {retried_task.id}")
        print(f"  Status: {retried_task.status}")
        print(f"  Retry Count: {retried_task.retry_count}")

    except Exception as e:
        print(f"Failed to retry task: {e}")
        raise


# =============================================================================
# Agent Examples
# =============================================================================

def agent_examples(client: ApexClient) -> None:
    """
    Create and manage agents.

    Agents are workers that execute tasks. They can have specific
    capabilities and constraints.
    """
    print("\n--- Agent Operations ---\n")

    try:
        # Create a new agent
        agent = client.create_agent(
            AgentCreate(
                name="research-agent-01",
                description="Specialized agent for research tasks",
                capabilities=[
                    {"name": "web-search", "version": "1.0"},
                    {"name": "summarization", "version": "1.0"},
                    {"name": "analysis", "version": "1.0"},
                ],
                max_concurrent_tasks=5,
                tags=["research", "analysis"],
                metadata={
                    "model": "gpt-4-turbo",
                    "region": "us-west-2",
                },
            )
        )

        print("Agent created:")
        print(f"  ID: {agent.id}")
        print(f"  Name: {agent.name}")
        print(f"  Status: {agent.status}")
        print(f"  Capabilities: {len(agent.capabilities)} defined")

        # List all agents
        agents = client.list_agents(
            status=AgentStatus.IDLE.value,
            page=1,
            per_page=10,
        )

        print(f"\nFound {agents.total} idle agents:")
        for a in agents.items:
            print(f"  - {a.name} ({a.id})")

        # Update agent
        updated_agent = client.update_agent(
            agent.id,
            AgentUpdate(
                max_concurrent_tasks=10,
                metadata={
                    "updated_at": datetime.now().isoformat(),
                },
            ),
        )
        print(f"\nAgent updated:")
        print(f"  Max concurrent tasks: {updated_agent.max_concurrent_tasks}")

        # Get agent details
        agent_details = client.get_agent(agent.id)
        print(f"\nAgent stats:")
        print(f"  Total tasks completed: {agent_details.total_tasks_completed}")
        print(f"  Current tasks: {agent_details.current_tasks}")

        # Clean up: delete the test agent
        client.delete_agent(agent.id)
        print(f"\nTest agent deleted")

    except Exception as e:
        print(f"Agent operations failed: {e}")
        raise


# =============================================================================
# Error Handling Examples
# =============================================================================

def error_handling_example(client: ApexClient) -> None:
    """
    Demonstrate proper error handling patterns.

    The SDK throws typed exceptions that can be caught and handled appropriately.
    """
    print("\n--- Error Handling ---\n")

    # Example 1: Handle not found error
    try:
        client.get_task("non-existent-task-id")
    except ApexNotFoundError as e:
        print(f"Not Found Error: {e.message}")
        print("  -> Resource not found, handle gracefully")

    # Example 2: Handle validation error
    try:
        # This would raise a validation error if the name is empty
        client.create_task(
            TaskCreate(
                name="",  # Invalid: empty name
                description="Test task",
            )
        )
    except ApexValidationError as e:
        print(f"\nValidation Error: {e.message}")
        print("  -> Invalid request data")

    # Example 3: Handle authentication error
    try:
        # Using invalid credentials
        bad_client = ApexClient(
            base_url=API_URL,
            api_key="invalid-api-key",
        )
        bad_client.health()
    except ApexAuthenticationError as e:
        print(f"\nAuthentication Error: {e.message}")
        print("  -> Invalid credentials")

    # Example 4: Handle rate limiting
    try:
        # Simulate rate limiting (this won't actually trigger in normal use)
        # In production, you'd handle this with retry logic
        pass
    except ApexRateLimitError as e:
        print(f"\nRate Limit Error: {e.message}")
        print(f"  -> Retry after: {e.retry_after} seconds")

    # Example 5: General error handling
    try:
        client.get_task("some-task-id")
    except ApexAPIError as e:
        print(f"\nGeneral API Error: {e.message}")
        print(f"  Status Code: {e.status_code if hasattr(e, 'status_code') else 'N/A'}")

    print("\nError handling examples completed")


# =============================================================================
# Context Manager Example
# =============================================================================

def context_manager_example() -> None:
    """
    Use the client as a context manager for automatic cleanup.
    """
    print("\n--- Context Manager Usage ---\n")

    with ApexClient(base_url=API_URL, api_key=API_KEY) as client:
        health = client.health()
        print(f"API Status: {health.status}")

        # Create a task
        task = client.create_task(
            TaskCreate(
                name="Context Manager Test",
                description="Task created within context manager",
            )
        )
        print(f"Created task: {task.id}")

        # Clean up
        client.delete_task(task.id)
        print(f"Deleted task: {task.id}")

    # Client is automatically closed when exiting the context
    print("\nClient automatically closed")


# =============================================================================
# Main Entry Point
# =============================================================================

def main() -> None:
    """Main function to run all examples."""
    print("=" * 60)
    print("Apex Python SDK - Basic Usage Examples")
    print("=" * 60)

    # Initialize the client
    client = initialize_client()

    try:
        # Run examples in sequence
        check_health(client)
        task_id = create_task_example(client)
        list_tasks_example(client)
        get_task_example(client, task_id)
        update_task_example(client, task_id)

        # Note: The following examples require a running Apex server
        # Uncomment to run full workflow

        # wait_for_task_example(client, task_id)
        # cancel_task_example(client, task_id)
        # retry_task_example(client, task_id)
        # agent_examples(client)

        error_handling_example(client)
        context_manager_example()

        print("\n" + "=" * 60)
        print("All examples completed successfully!")
        print("=" * 60)

    except Exception as e:
        print(f"\nExample failed with error: {e}")
        sys.exit(1)

    finally:
        # Clean up
        client.close()


if __name__ == "__main__":
    main()
