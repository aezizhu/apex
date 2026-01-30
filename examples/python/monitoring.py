#!/usr/bin/env python3
"""
Apex Python SDK - Real-Time Monitoring with WebSocket

This example demonstrates how to build a live monitoring console that
streams events from the Apex orchestrator over WebSocket:

- Connecting and authenticating the WebSocket client
- Subscribing to filtered event streams (tasks, agents, DAGs)
- Using the decorator-based event handler API
- Using the programmatic event handler API
- Building an auto-updating terminal dashboard
- Graceful reconnection with exponential back-off
- Concurrent REST polling alongside WebSocket streaming

Prerequisites:
    pip install apex-swarm

Run with:
    python monitoring.py
"""

from __future__ import annotations

import asyncio
import os
import sys
from collections import defaultdict
from datetime import datetime, timezone
from typing import Any

from apex_sdk import AsyncApexClient
from apex_sdk.models import (
    TaskCreate,
    TaskInput,
    TaskPriority,
    WebSocketEventType,
    WebSocketMessage,
)
from apex_sdk.websocket import ApexWebSocketClient
from apex_sdk.exceptions import ApexAPIError, ApexWebSocketError

# =============================================================================
# Configuration
# =============================================================================

API_URL: str = os.environ.get("APEX_API_URL", "http://localhost:8080")
API_KEY: str = os.environ.get("APEX_API_KEY", "")

# How long the demo dashboard runs before exiting (seconds).
DASHBOARD_DURATION: int = int(os.environ.get("DASHBOARD_DURATION", "120"))


# =============================================================================
# 1. Simple Event Listener
# =============================================================================

async def simple_event_listener() -> None:
    """Connect to the WebSocket and print every event received.

    This is the most basic monitoring pattern: subscribe to all events
    and iterate over messages as they arrive.
    """
    print("\n--- Simple Event Listener ---\n")

    ws = ApexWebSocketClient(
        base_url=API_URL,
        api_key=API_KEY,
        reconnect=True,
        reconnect_delay=1.0,
        max_reconnect_delay=30.0,
    )

    try:
        await ws.connect()
        print("WebSocket connected")

        # Subscribe to all task and agent events
        await ws.subscribe(
            events=[
                WebSocketEventType.TASK_CREATED,
                WebSocketEventType.TASK_UPDATED,
                WebSocketEventType.TASK_COMPLETED,
                WebSocketEventType.TASK_FAILED,
                WebSocketEventType.AGENT_STATUS_CHANGED,
                WebSocketEventType.DAG_STARTED,
                WebSocketEventType.DAG_COMPLETED,
                WebSocketEventType.DAG_FAILED,
            ]
        )
        print("Subscribed to events -- listening for 30 seconds...\n")

        # Stream messages, auto-exit after 30 seconds
        async for message in _timeout_listen(ws, timeout=30.0):
            _print_event(message)

    except ApexWebSocketError as e:
        print(f"WebSocket error: {e}")
    finally:
        await ws.disconnect()
        print("\nDisconnected")


def _print_event(msg: WebSocketMessage) -> None:
    """Pretty-print a single WebSocket message."""
    ts = msg.timestamp.strftime("%H:%M:%S") if msg.timestamp else "??:??:??"
    print(f"  [{ts}] {msg.type.value}")

    # Show a brief summary depending on the event type
    data = msg.data
    if "task" in data:
        task = data["task"]
        print(f"         Task: {task.get('name', 'N/A')} ({task.get('id', '?')[:8]}...)")
        print(f"         Status: {task.get('status', '?')}")
    elif "agent" in data:
        agent = data["agent"]
        print(f"         Agent: {agent.get('name', 'N/A')} -> {agent.get('status', '?')}")
    elif "dag" in data:
        dag = data["dag"]
        print(f"         DAG: {dag.get('name', 'N/A')} ({dag.get('id', '?')[:8]}...)")


async def _timeout_listen(
    ws: ApexWebSocketClient,
    timeout: float,
) -> Any:  # AsyncGenerator
    """Yield messages from the WebSocket until *timeout* seconds elapse."""
    try:
        async with asyncio.timeout(timeout):
            async for message in ws.listen():
                yield message
    except TimeoutError:
        return


# =============================================================================
# 2. Decorator-Based Handler
# =============================================================================

async def decorator_based_monitoring() -> None:
    """Register handlers using the ``@ws.on_event(...)`` decorator.

    This pattern is convenient when you know at import time which events
    you want to handle and prefer a declarative style.
    """
    print("\n--- Decorator-Based Event Handlers ---\n")

    ws = ApexWebSocketClient(base_url=API_URL, api_key=API_KEY)

    # Register handlers before connecting
    @ws.on_event(WebSocketEventType.TASK_COMPLETED)
    async def on_task_completed(message: WebSocketMessage) -> None:
        task = message.data.get("task", {})
        print(f"  [COMPLETED] {task.get('name')} -- output keys: "
              f"{list(task.get('output', {}).keys())}")

    @ws.on_event(WebSocketEventType.TASK_FAILED)
    async def on_task_failed(message: WebSocketMessage) -> None:
        task = message.data.get("task", {})
        error = message.data.get("error", {})
        print(f"  [FAILED] {task.get('name')} -- {error.get('code')}: "
              f"{error.get('message')}")

    @ws.on_event(WebSocketEventType.AGENT_STATUS_CHANGED)
    async def on_agent_status(message: WebSocketMessage) -> None:
        agent = message.data.get("agent", {})
        prev = message.data.get("previous_status", "?")
        print(f"  [AGENT] {agent.get('name')}: {prev} -> {agent.get('status')}")

    try:
        await ws.connect()
        await ws.subscribe()  # subscribe to all events
        print("Listening with decorator handlers for 30 seconds...\n")

        # ws.run() blocks while dispatching events to registered handlers
        await asyncio.wait_for(ws.run(), timeout=30.0)
    except asyncio.TimeoutError:
        print("\nTimeout reached")
    except ApexWebSocketError as e:
        print(f"WebSocket error: {e}")
    finally:
        await ws.disconnect()


# =============================================================================
# 3. Filtered Task Monitor
# =============================================================================

async def filtered_task_monitor(task_ids: list[str]) -> None:
    """Monitor a specific set of tasks by their IDs.

    Useful when you have just submitted a batch of tasks and want to
    track only those, ignoring the rest of the cluster's activity.

    Args:
        task_ids: List of task UUIDs to subscribe to.
    """
    print(f"\n--- Filtered Task Monitor ({len(task_ids)} tasks) ---\n")

    ws = ApexWebSocketClient(base_url=API_URL, api_key=API_KEY)

    completed: set[str] = set()
    failed: set[str] = set()
    done_event = asyncio.Event()

    async def _on_complete(message: WebSocketMessage) -> None:
        tid = message.data.get("task", {}).get("id")
        if tid and tid in task_ids:
            completed.add(tid)
            print(f"  Task {tid[:8]}... completed  "
                  f"({len(completed) + len(failed)}/{len(task_ids)})")
            if len(completed) + len(failed) >= len(task_ids):
                done_event.set()

    async def _on_fail(message: WebSocketMessage) -> None:
        tid = message.data.get("task", {}).get("id")
        if tid and tid in task_ids:
            failed.add(tid)
            print(f"  Task {tid[:8]}... FAILED  "
                  f"({len(completed) + len(failed)}/{len(task_ids)})")
            if len(completed) + len(failed) >= len(task_ids):
                done_event.set()

    ws.add_event_handler(WebSocketEventType.TASK_COMPLETED, _on_complete)
    ws.add_event_handler(WebSocketEventType.TASK_FAILED, _on_fail)

    try:
        await ws.connect()

        # Subscribe only to events for our tasks
        await ws.subscribe(
            events=[
                WebSocketEventType.TASK_COMPLETED,
                WebSocketEventType.TASK_FAILED,
                WebSocketEventType.TASK_UPDATED,
            ],
            task_ids=task_ids,
        )

        print("Waiting for all tasks to finish...")

        # Run the WebSocket listener in the background
        listen_task = asyncio.create_task(ws.run())

        # Wait until all tasks are done (or timeout after 5 minutes)
        try:
            await asyncio.wait_for(done_event.wait(), timeout=300.0)
        except asyncio.TimeoutError:
            print("\nTimed out waiting for tasks")

        listen_task.cancel()
        try:
            await listen_task
        except asyncio.CancelledError:
            pass

    finally:
        await ws.disconnect()

    print(f"\nResults: {len(completed)} completed, {len(failed)} failed")


# =============================================================================
# 4. Terminal Dashboard
# =============================================================================

async def terminal_dashboard() -> None:
    """Build an auto-refreshing terminal dashboard.

    Combines WebSocket streaming (for push updates) with periodic REST
    polling (for aggregate counters) to render a compact status display.
    """
    print("\n--- Terminal Dashboard ---\n")

    # Counters updated by WebSocket events
    counters: dict[str, int] = defaultdict(int)
    recent_events: list[dict[str, str]] = []

    def _add_recent(event_type: str, summary: str) -> None:
        recent_events.insert(0, {
            "time": datetime.now(timezone.utc).strftime("%H:%M:%S"),
            "type": event_type,
            "summary": summary,
        })
        # Keep the last 8 events
        del recent_events[8:]

    # -- WebSocket handlers ---------------------------------------------------

    ws = ApexWebSocketClient(base_url=API_URL, api_key=API_KEY)

    async def _on_task_created(msg: WebSocketMessage) -> None:
        counters["tasks_created"] += 1
        task = msg.data.get("task", {})
        _add_recent("TASK+", task.get("name", "unnamed"))

    async def _on_task_completed(msg: WebSocketMessage) -> None:
        counters["tasks_completed"] += 1
        task = msg.data.get("task", {})
        _add_recent("TASK_OK", task.get("name", "unnamed"))

    async def _on_task_failed(msg: WebSocketMessage) -> None:
        counters["tasks_failed"] += 1
        task = msg.data.get("task", {})
        _add_recent("TASK_ERR", task.get("name", "unnamed"))

    async def _on_agent_changed(msg: WebSocketMessage) -> None:
        agent = msg.data.get("agent", {})
        new_status = agent.get("status", "?")
        counters[f"agents_{new_status}"] += 1
        _add_recent("AGENT", f"{agent.get('name', '?')} -> {new_status}")

    ws.add_event_handler(WebSocketEventType.TASK_CREATED, _on_task_created)
    ws.add_event_handler(WebSocketEventType.TASK_COMPLETED, _on_task_completed)
    ws.add_event_handler(WebSocketEventType.TASK_FAILED, _on_task_failed)
    ws.add_event_handler(WebSocketEventType.AGENT_STATUS_CHANGED, _on_agent_changed)

    # -- Render function ------------------------------------------------------

    def render() -> None:
        """Render the dashboard to the terminal."""
        # Use ANSI escape codes to clear screen and move cursor home
        print("\033[2J\033[H", end="")
        print("=" * 62)
        print("            APEX MONITORING DASHBOARD")
        print("=" * 62)
        print()
        print("  TASKS")
        print("  " + "-" * 40)
        print(f"    Created:   {counters['tasks_created']}")
        print(f"    Completed: {counters['tasks_completed']}")
        print(f"    Failed:    {counters['tasks_failed']}")
        print()
        print("  AGENTS")
        print("  " + "-" * 40)
        print(f"    Idle:    {counters.get('agents_idle', 0)}")
        print(f"    Busy:    {counters.get('agents_busy', 0)}")
        print(f"    Error:   {counters.get('agents_error', 0)}")
        print()
        print("  RECENT EVENTS")
        print("  " + "-" * 40)
        if not recent_events:
            print("    (waiting for events...)")
        for evt in recent_events[:6]:
            print(f"    [{evt['time']}] {evt['type']:10s} {evt['summary']}")
        print()
        print("=" * 62)
        now = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
        print(f"  Last refresh: {now}")
        print(f"  Duration remaining: {DASHBOARD_DURATION}s  |  Ctrl+C to stop")

    # -- Main loop ------------------------------------------------------------

    try:
        await ws.connect()
        await ws.subscribe()  # subscribe to everything
        print("Dashboard connected -- starting render loop")

        # Run the WebSocket listener concurrently with the render loop
        listen_task = asyncio.create_task(ws.run())

        start = asyncio.get_event_loop().time()
        while asyncio.get_event_loop().time() - start < DASHBOARD_DURATION:
            render()
            await asyncio.sleep(2.0)  # refresh every 2 seconds

        listen_task.cancel()
        try:
            await listen_task
        except asyncio.CancelledError:
            pass

    except KeyboardInterrupt:
        print("\nDashboard stopped by user")
    except ApexWebSocketError as e:
        print(f"WebSocket error: {e}")
    finally:
        await ws.disconnect()
        print("Dashboard session ended")


# =============================================================================
# 5. Combined REST + WebSocket Monitoring
# =============================================================================

async def combined_monitoring() -> None:
    """Demonstrate using the async REST client alongside WebSocket.

    This pattern is useful when you need to:
    - Create tasks via REST, then immediately track them via WebSocket.
    - Periodically fetch aggregate statistics via REST while streaming
      individual events over WebSocket.
    """
    print("\n--- Combined REST + WebSocket Monitoring ---\n")

    async with AsyncApexClient(
        base_url=API_URL,
        api_key=API_KEY,
        timeout=30.0,
    ) as client:
        # Check health first
        health = await client.health()
        print(f"API health: {health.status}")

        # Create a few tasks via REST
        task_ids: list[str] = []
        for i in range(3):
            task = await client.create_task(
                TaskCreate(
                    name=f"Monitored Task {i + 1}",
                    description=f"Task {i + 1} created for combined monitoring demo",
                    priority=TaskPriority.NORMAL,
                    input=TaskInput(data={"index": i}),
                )
            )
            task_ids.append(task.id)
            print(f"  Created: {task.name} ({task.id[:8]}...)")

        # Now open a WebSocket to track those tasks
        ws = client.websocket()

        async def _on_update(msg: WebSocketMessage) -> None:
            task_data = msg.data.get("task", {})
            if task_data.get("id") in task_ids:
                print(f"  [WS] {task_data.get('name')}: {task_data.get('status')}")

        ws.add_event_handler(WebSocketEventType.TASK_UPDATED, _on_update)
        ws.add_event_handler(WebSocketEventType.TASK_COMPLETED, _on_update)
        ws.add_event_handler(WebSocketEventType.TASK_FAILED, _on_update)

        try:
            await ws.connect()
            await ws.subscribe(
                events=[
                    WebSocketEventType.TASK_UPDATED,
                    WebSocketEventType.TASK_COMPLETED,
                    WebSocketEventType.TASK_FAILED,
                ],
                task_ids=task_ids,
            )
            print("\nListening for updates on created tasks (20s)...\n")

            # Run WebSocket for up to 20 seconds
            await asyncio.wait_for(ws.run(), timeout=20.0)

        except asyncio.TimeoutError:
            print("\nMonitoring period ended")
        finally:
            await ws.disconnect()

            # Cleanup: delete the demo tasks
            for tid in task_ids:
                await client.delete_task(tid)
            print("Demo tasks cleaned up")


# =============================================================================
# Main
# =============================================================================

async def main() -> None:
    """Run monitoring examples."""
    print("=" * 62)
    print("  Apex SDK - Real-Time Monitoring Examples")
    print("=" * 62)

    try:
        # Example 1: Simple event listener
        await simple_event_listener()

        # Example 2: Decorator-based handlers
        # await decorator_based_monitoring()

        # Example 3: Filtered task monitor (requires task IDs)
        # await filtered_task_monitor(["task-id-1", "task-id-2"])

        # Example 4: Terminal dashboard (runs for DASHBOARD_DURATION seconds)
        # await terminal_dashboard()

        # Example 5: Combined REST + WebSocket
        # await combined_monitoring()

        print("\n" + "=" * 62)
        print("  Monitoring examples completed")
        print("=" * 62)

    except ApexAPIError as e:
        print(f"\nAPI Error: {e.message}")
        sys.exit(1)
    except Exception as e:
        print(f"\nUnexpected error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())
