#!/usr/bin/env python3
"""
Apex Python SDK - Async Streaming Example

This example demonstrates asynchronous operations and WebSocket streaming:
- Async client usage
- WebSocket connection for real-time updates
- Event-driven task monitoring
- Concurrent task execution
- Streaming log messages

Prerequisites:
    pip install apex-swarm

Run with:
    python async_streaming.py
"""

import asyncio
import os
import sys
from datetime import datetime
from typing import Optional

from apex_sdk import AsyncApexClient
from apex_sdk.models import (
    TaskCreate,
    TaskPriority,
    TaskStatus,
    TaskInput,
    DAGCreate,
    DAGNode,
    DAGEdge,
    WebSocketEventType,
    WebSocketMessage,
)
from apex_sdk.websocket import ApexWebSocketClient
from apex_sdk.exceptions import ApexAPIError, ApexWebSocketError

# =============================================================================
# Configuration
# =============================================================================

API_URL = os.environ.get("APEX_API_URL", "http://localhost:8080")
API_KEY = os.environ.get("APEX_API_KEY", "")


# =============================================================================
# Basic Async Client Usage
# =============================================================================

async def basic_async_example() -> None:
    """
    Demonstrate basic async client operations.

    The async client is ideal for:
    - High-concurrency scenarios
    - Non-blocking I/O operations
    - Integration with async frameworks (FastAPI, aiohttp, etc.)
    """
    print("\n--- Basic Async Client Usage ---\n")

    # Create async client
    async with AsyncApexClient(
        base_url=API_URL,
        api_key=API_KEY,
        timeout=30.0,
    ) as client:
        # Health check
        health = await client.health()
        print(f"API Status: {health.status}")
        print(f"Version: {health.version}")

        # Create a task
        task = await client.create_task(
            TaskCreate(
                name="Async Task Example",
                description="A task created using the async client",
                priority=TaskPriority.NORMAL,
                input=TaskInput(data={"message": "Hello from async!"}),
            )
        )
        print(f"\nTask created: {task.id}")
        print(f"  Name: {task.name}")
        print(f"  Status: {task.status}")

        # Get task details
        retrieved_task = await client.get_task(task.id)
        print(f"\nRetrieved task: {retrieved_task.name}")

        # List tasks
        tasks = await client.list_tasks(page=1, per_page=5)
        print(f"\nTotal tasks: {tasks.total}")

        # Clean up
        await client.delete_task(task.id)
        print(f"\nTask deleted: {task.id}")


# =============================================================================
# Concurrent Task Execution
# =============================================================================

async def concurrent_task_execution() -> None:
    """
    Execute multiple tasks concurrently using asyncio.gather.

    This demonstrates the performance benefits of async I/O when
    dealing with multiple independent operations.
    """
    print("\n--- Concurrent Task Execution ---\n")

    async with AsyncApexClient(base_url=API_URL, api_key=API_KEY) as client:
        # Create multiple tasks concurrently
        print("Creating 5 tasks concurrently...")

        task_definitions = [
            TaskCreate(
                name=f"Concurrent Task {i}",
                description=f"Task {i} created concurrently",
                priority=TaskPriority.NORMAL,
                input=TaskInput(data={"task_number": i}),
            )
            for i in range(1, 6)
        ]

        # Create all tasks concurrently
        start_time = datetime.now()
        tasks = await asyncio.gather(
            *[client.create_task(task_def) for task_def in task_definitions]
        )
        elapsed = (datetime.now() - start_time).total_seconds()

        print(f"Created {len(tasks)} tasks in {elapsed:.2f} seconds")

        for task in tasks:
            print(f"  - {task.name} ({task.id})")

        # Get all task statuses concurrently
        print("\nFetching task statuses concurrently...")

        statuses = await asyncio.gather(
            *[client.get_task(task.id) for task in tasks]
        )

        for task in statuses:
            print(f"  - {task.name}: {task.status}")

        # Clean up all tasks concurrently
        print("\nDeleting all tasks concurrently...")

        await asyncio.gather(
            *[client.delete_task(task.id) for task in tasks]
        )

        print("All tasks deleted")


# =============================================================================
# WebSocket Real-Time Updates
# =============================================================================

async def websocket_monitoring() -> None:
    """
    Connect to WebSocket for real-time updates.

    The WebSocket connection allows you to receive instant notifications
    about task, agent, and DAG events without polling.
    """
    print("\n--- WebSocket Real-Time Monitoring ---\n")

    # Create WebSocket client
    ws_client = ApexWebSocketClient(
        base_url=API_URL,
        api_key=API_KEY,
        reconnect=True,
        reconnect_delay=1.0,
        max_reconnect_delay=60.0,
    )

    try:
        # Connect to WebSocket
        await ws_client.connect()
        print("WebSocket connected!")

        # Subscribe to events
        await ws_client.subscribe(
            events=[
                WebSocketEventType.TASK_CREATED,
                WebSocketEventType.TASK_UPDATED,
                WebSocketEventType.TASK_COMPLETED,
                WebSocketEventType.TASK_FAILED,
                WebSocketEventType.AGENT_STATUS_CHANGED,
            ]
        )
        print("Subscribed to task and agent events")

        # Listen for events (with timeout for demo)
        print("\nListening for events (10 seconds)...")

        try:
            async for message in asyncio.timeout(10.0)(ws_client.listen()):
                print(f"\n[{message.type}] at {message.timestamp}")
                print(f"  Data: {message.data}")
        except asyncio.TimeoutError:
            print("\nTimeout reached, stopping listener")

    finally:
        await ws_client.disconnect()
        print("WebSocket disconnected")


# =============================================================================
# Event-Driven Task Monitoring
# =============================================================================

async def event_driven_task_monitoring(task_id: str) -> None:
    """
    Monitor a specific task using WebSocket events.

    This demonstrates how to wait for specific events and react accordingly.
    """
    print(f"\n--- Monitoring Task: {task_id} ---\n")

    ws_client = ApexWebSocketClient(
        base_url=API_URL,
        api_key=API_KEY,
    )

    completion_event = asyncio.Event()
    final_status: Optional[str] = None

    async def handle_task_completed(message: WebSocketMessage) -> None:
        nonlocal final_status
        if message.data.get("task", {}).get("id") == task_id:
            final_status = "completed"
            print(f"Task completed! Duration: {message.data.get('duration')}ms")
            completion_event.set()

    async def handle_task_failed(message: WebSocketMessage) -> None:
        nonlocal final_status
        if message.data.get("task", {}).get("id") == task_id:
            final_status = "failed"
            error = message.data.get("error", {})
            print(f"Task failed! Error: {error.get('message')}")
            completion_event.set()

    async def handle_task_updated(message: WebSocketMessage) -> None:
        if message.data.get("task", {}).get("id") == task_id:
            task = message.data.get("task", {})
            print(f"Task updated: {task.get('status')}")

    # Register event handlers
    ws_client.add_event_handler(WebSocketEventType.TASK_COMPLETED, handle_task_completed)
    ws_client.add_event_handler(WebSocketEventType.TASK_FAILED, handle_task_failed)
    ws_client.add_event_handler(WebSocketEventType.TASK_UPDATED, handle_task_updated)

    try:
        await ws_client.connect()

        # Subscribe to events for this specific task
        await ws_client.subscribe(
            events=[
                WebSocketEventType.TASK_UPDATED,
                WebSocketEventType.TASK_COMPLETED,
                WebSocketEventType.TASK_FAILED,
            ],
            task_ids=[task_id],
        )

        print("Waiting for task completion...")

        # Start listening in the background
        listen_task = asyncio.create_task(ws_client.run())

        # Wait for completion or timeout
        try:
            await asyncio.wait_for(completion_event.wait(), timeout=120.0)
            print(f"\nTask finished with status: {final_status}")
        except asyncio.TimeoutError:
            print("\nTimeout waiting for task completion")

        # Cancel the listener
        listen_task.cancel()
        try:
            await listen_task
        except asyncio.CancelledError:
            pass

    finally:
        await ws_client.disconnect()


# =============================================================================
# Streaming Log Messages
# =============================================================================

async def stream_task_logs() -> None:
    """
    Stream log messages from tasks in real-time.

    This is useful for monitoring task progress and debugging.
    """
    print("\n--- Streaming Task Logs ---\n")

    async with AsyncApexClient(base_url=API_URL, api_key=API_KEY) as client:
        # Create a task
        task = await client.create_task(
            TaskCreate(
                name="Logging Demo Task",
                description="A task that generates log messages",
                priority=TaskPriority.NORMAL,
            )
        )

        print(f"Created task: {task.id}")
        print("Streaming logs...\n")

        # Get WebSocket client from async client
        ws_client = client.websocket()

        try:
            await ws_client.connect()

            # Subscribe to log messages for this task
            await ws_client.subscribe(
                events=[WebSocketEventType.LOG_MESSAGE],
                task_ids=[task.id],
            )

            # Stream logs for 30 seconds or until task completes
            async def stream_logs():
                async for message in ws_client.listen():
                    if message.type == WebSocketEventType.LOG_MESSAGE:
                        log = message.data.get("log", {})
                        level = log.get("level", "INFO").upper()
                        msg = log.get("message", "")
                        timestamp = log.get("timestamp", "")
                        print(f"[{timestamp}] [{level}] {msg}")

            try:
                await asyncio.wait_for(stream_logs(), timeout=30.0)
            except asyncio.TimeoutError:
                print("\nLog streaming timeout")

        finally:
            await ws_client.disconnect()

            # Clean up
            await client.delete_task(task.id)
            print(f"\nTask deleted: {task.id}")


# =============================================================================
# DAG Streaming Execution
# =============================================================================

async def dag_streaming_execution() -> None:
    """
    Create and monitor a DAG execution with streaming updates.
    """
    print("\n--- DAG Streaming Execution ---\n")

    async with AsyncApexClient(base_url=API_URL, api_key=API_KEY) as client:
        # Create a simple DAG
        dag = await client.create_dag(
            DAGCreate(
                name="Streaming Pipeline",
                description="A DAG monitored via WebSocket",
                nodes=[
                    DAGNode(
                        id="step1",
                        task_template=TaskCreate(
                            name="Step 1",
                            description="First step",
                        ),
                        depends_on=[],
                    ),
                    DAGNode(
                        id="step2",
                        task_template=TaskCreate(
                            name="Step 2",
                            description="Second step",
                        ),
                        depends_on=["step1"],
                    ),
                    DAGNode(
                        id="step3",
                        task_template=TaskCreate(
                            name="Step 3",
                            description="Third step",
                        ),
                        depends_on=["step2"],
                    ),
                ],
                edges=[
                    DAGEdge(source="step1", target="step2"),
                    DAGEdge(source="step2", target="step3"),
                ],
            )
        )

        print(f"Created DAG: {dag.id}")

        # Get WebSocket client
        ws_client = client.websocket()

        completion_event = asyncio.Event()

        async def handle_dag_completed(message: WebSocketMessage) -> None:
            if message.data.get("dag", {}).get("id") == dag.id:
                print(f"\nDAG completed!")
                print(f"  Duration: {message.data.get('duration')}ms")
                completion_event.set()

        async def handle_dag_failed(message: WebSocketMessage) -> None:
            if message.data.get("dag", {}).get("id") == dag.id:
                error = message.data.get("error", {})
                print(f"\nDAG failed!")
                print(f"  Error: {error.get('message')}")
                completion_event.set()

        async def handle_task_updated(message: WebSocketMessage) -> None:
            task = message.data.get("task", {})
            if task.get("dag_id") == dag.id:
                print(f"  Task {task.get('name')}: {task.get('status')}")

        ws_client.add_event_handler(WebSocketEventType.DAG_COMPLETED, handle_dag_completed)
        ws_client.add_event_handler(WebSocketEventType.DAG_FAILED, handle_dag_failed)
        ws_client.add_event_handler(WebSocketEventType.TASK_UPDATED, handle_task_updated)

        try:
            await ws_client.connect()

            # Subscribe to DAG and task events
            await ws_client.subscribe(
                events=[
                    WebSocketEventType.DAG_STARTED,
                    WebSocketEventType.DAG_COMPLETED,
                    WebSocketEventType.DAG_FAILED,
                    WebSocketEventType.TASK_UPDATED,
                ],
                dag_ids=[dag.id],
            )

            # Start the DAG
            print("\nStarting DAG execution...")
            await client.start_dag(dag.id)

            # Start listening in the background
            listen_task = asyncio.create_task(ws_client.run())

            # Wait for completion
            try:
                await asyncio.wait_for(completion_event.wait(), timeout=120.0)
            except asyncio.TimeoutError:
                print("\nTimeout waiting for DAG completion")

            listen_task.cancel()
            try:
                await listen_task
            except asyncio.CancelledError:
                pass

        finally:
            await ws_client.disconnect()

            # Clean up
            await client.delete_dag(dag.id)
            print(f"\nDAG deleted: {dag.id}")


# =============================================================================
# Dashboard Feed Example
# =============================================================================

async def dashboard_feed() -> None:
    """
    Create a real-time dashboard feed using WebSocket.

    This demonstrates how to build a dashboard that shows live updates
    for tasks, agents, and DAGs.
    """
    print("\n--- Real-Time Dashboard Feed ---\n")

    # Stats tracking
    stats = {
        "tasks": {"created": 0, "completed": 0, "failed": 0},
        "agents": {"idle": 0, "busy": 0, "error": 0},
        "events": [],
    }

    def add_event(event_type: str, message: str) -> None:
        stats["events"].insert(0, {
            "type": event_type,
            "message": message,
            "timestamp": datetime.now().isoformat(),
        })
        # Keep only last 10 events
        stats["events"] = stats["events"][:10]

    def print_dashboard() -> None:
        print("\033[2J\033[H")  # Clear screen
        print("=" * 60)
        print("          APEX REAL-TIME DASHBOARD")
        print("=" * 60)
        print()
        print("TASKS")
        print("-" * 40)
        print(f"  Created:   {stats['tasks']['created']}")
        print(f"  Completed: {stats['tasks']['completed']}")
        print(f"  Failed:    {stats['tasks']['failed']}")
        print()
        print("AGENTS")
        print("-" * 40)
        print(f"  Idle:  {stats['agents']['idle']}")
        print(f"  Busy:  {stats['agents']['busy']}")
        print(f"  Error: {stats['agents']['error']}")
        print()
        print("RECENT EVENTS")
        print("-" * 40)
        for event in stats["events"][:5]:
            print(f"  [{event['timestamp'][:19]}] {event['message']}")
        print()
        print("=" * 60)
        print(f"Last updated: {datetime.now().strftime('%H:%M:%S')}")
        print("Press Ctrl+C to stop")

    ws_client = ApexWebSocketClient(
        base_url=API_URL,
        api_key=API_KEY,
    )

    async def handle_task_created(message: WebSocketMessage) -> None:
        stats["tasks"]["created"] += 1
        task = message.data.get("task", {})
        add_event("task", f"Task created: {task.get('name')}")
        print_dashboard()

    async def handle_task_completed(message: WebSocketMessage) -> None:
        stats["tasks"]["completed"] += 1
        task = message.data.get("task", {})
        add_event("task", f"Task completed: {task.get('name')}")
        print_dashboard()

    async def handle_task_failed(message: WebSocketMessage) -> None:
        stats["tasks"]["failed"] += 1
        task = message.data.get("task", {})
        add_event("task", f"Task failed: {task.get('name')}")
        print_dashboard()

    async def handle_agent_status(message: WebSocketMessage) -> None:
        agent = message.data.get("agent", {})
        prev_status = message.data.get("previous_status")
        new_status = agent.get("status")

        # Update counts
        if prev_status in stats["agents"]:
            stats["agents"][prev_status] -= 1
        if new_status in stats["agents"]:
            stats["agents"][new_status] += 1

        add_event("agent", f"Agent {agent.get('name')}: {prev_status} -> {new_status}")
        print_dashboard()

    ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, handle_task_created)
    ws_client.add_event_handler(WebSocketEventType.TASK_COMPLETED, handle_task_completed)
    ws_client.add_event_handler(WebSocketEventType.TASK_FAILED, handle_task_failed)
    ws_client.add_event_handler(WebSocketEventType.AGENT_STATUS_CHANGED, handle_agent_status)

    try:
        await ws_client.connect()

        # Subscribe to all relevant events
        await ws_client.subscribe(
            events=[
                WebSocketEventType.TASK_CREATED,
                WebSocketEventType.TASK_COMPLETED,
                WebSocketEventType.TASK_FAILED,
                WebSocketEventType.AGENT_STATUS_CHANGED,
            ]
        )

        # Initial dashboard
        print_dashboard()

        # Run for 60 seconds
        await asyncio.wait_for(ws_client.run(), timeout=60.0)

    except asyncio.TimeoutError:
        print("\nDashboard session ended")
    except KeyboardInterrupt:
        print("\nDashboard stopped by user")
    finally:
        await ws_client.disconnect()


# =============================================================================
# Main Entry Point
# =============================================================================

async def main() -> None:
    """Main async function to run all examples."""
    print("=" * 60)
    print("Apex Python SDK - Async Streaming Examples")
    print("=" * 60)

    try:
        # Basic async example
        await basic_async_example()

        # Concurrent task execution
        await concurrent_task_execution()

        # WebSocket monitoring (uncomment to run)
        # await websocket_monitoring()

        # DAG streaming execution (uncomment to run)
        # await dag_streaming_execution()

        # Dashboard feed (uncomment to run)
        # await dashboard_feed()

        print("\n" + "=" * 60)
        print("All async examples completed successfully!")
        print("=" * 60)

    except ApexAPIError as e:
        print(f"\nAPI Error: {e.message}")
        sys.exit(1)
    except ApexWebSocketError as e:
        print(f"\nWebSocket Error: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"\nExample failed with error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())
