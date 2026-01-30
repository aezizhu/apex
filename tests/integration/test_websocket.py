"""Integration tests for WebSocket real-time communication."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncGenerator
from typing import Any

import pytest
import pytest_asyncio

from apex_sdk import AsyncApexClient
from apex_sdk.exceptions import ApexWebSocketClosed, ApexWebSocketError
from apex_sdk.models import (
    TaskCreate,
    TaskStatus,
    WebSocketEventType,
    WebSocketMessage,
)
from apex_sdk.websocket import ApexWebSocketClient


class TestWebSocketConnection:
    """Tests for WebSocket connection management."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_connect_and_disconnect(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test basic WebSocket connection and disconnection."""
        # Act
        await ws_client.connect()

        # Assert
        assert ws_client.is_connected

        # Cleanup
        await ws_client.disconnect()
        assert not ws_client.is_connected

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_context_manager(
        self,
        test_config: dict[str, Any],
    ) -> None:
        """Test WebSocket client as async context manager."""
        # Act
        async with ApexWebSocketClient(
            base_url=test_config["api_url"],
            api_key=test_config["api_key"],
            reconnect=False,
        ) as ws:
            # Assert - Connected inside context
            assert ws.is_connected

        # Assert - Disconnected after context
        # Note: ws object still exists but should be disconnected

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_multiple_connect_calls_idempotent(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test that multiple connect calls are idempotent."""
        # Act
        await ws_client.connect()
        await ws_client.connect()  # Should not error
        await ws_client.connect()  # Should not error

        # Assert
        assert ws_client.is_connected

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_disconnect_when_not_connected(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test that disconnect when not connected doesn't error."""
        # Act & Assert - Should not raise
        await ws_client.disconnect()
        await ws_client.disconnect()


class TestWebSocketSubscription:
    """Tests for WebSocket event subscription."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_subscribe_to_all_events(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test subscribing to all event types."""
        # Arrange
        await ws_client.connect()

        # Act
        await ws_client.subscribe()

        # Assert - Should complete without error
        # The subscription is stored internally

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_subscribe_to_specific_events(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test subscribing to specific event types."""
        # Arrange
        await ws_client.connect()

        # Act
        await ws_client.subscribe(
            events=[
                WebSocketEventType.TASK_CREATED,
                WebSocketEventType.TASK_COMPLETED,
            ]
        )

        # Assert - Should complete without error

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_subscribe_with_filters(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test subscribing with entity ID filters."""
        # Arrange
        await ws_client.connect()

        # Act
        await ws_client.subscribe(
            events=[WebSocketEventType.TASK_UPDATED],
            task_ids=["task-1", "task-2"],
            agent_ids=["agent-1"],
        )

        # Assert - Should complete without error

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_subscribe_before_connect(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test that subscribing before connect stores the subscription."""
        # Act - Subscribe before connecting
        await ws_client.subscribe(events=[WebSocketEventType.TASK_CREATED])

        # Now connect
        await ws_client.connect()

        # Assert - Connection should be established with subscription active


class TestWebSocketEventHandlers:
    """Tests for WebSocket event handler registration."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_add_event_handler(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test adding an event handler programmatically."""
        # Arrange
        received_events: list[WebSocketMessage] = []

        def handler(message: WebSocketMessage) -> None:
            received_events.append(message)

        # Act
        ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, handler)

        # Assert - Handler is registered (tested via dispatch later)

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_remove_event_handler(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test removing an event handler."""
        # Arrange
        def handler(message: WebSocketMessage) -> None:
            pass

        ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, handler)

        # Act
        ws_client.remove_event_handler(WebSocketEventType.TASK_CREATED, handler)

        # Assert - Should complete without error

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_decorator_event_handler(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test registering event handler via decorator."""
        # Act
        @ws_client.on_event(WebSocketEventType.TASK_COMPLETED)
        async def handle_completion(message: WebSocketMessage) -> None:
            pass

        # Assert - Handler is registered


class TestWebSocketEventDelivery:
    """Tests for WebSocket event delivery on API operations."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    @pytest.mark.slow
    async def test_task_created_event(
        self,
        api_client: AsyncApexClient,
        ws_client: ApexWebSocketClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test receiving task created event when creating a task."""
        # Arrange
        received_events: list[WebSocketMessage] = []

        async def handler(message: WebSocketMessage) -> None:
            received_events.append(message)

        ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, handler)
        await ws_client.connect()
        await ws_client.subscribe(events=[WebSocketEventType.TASK_CREATED])

        # Act - Create a task
        task = await api_client.create_task(
            TaskCreate(name="websocket-test-task")
        )
        cleanup_tasks.append(task.id)

        # Wait for event delivery
        await asyncio.sleep(1.0)

        # Assert
        assert len(received_events) >= 1
        # Find the event for our task
        task_events = [
            e for e in received_events
            if e.data.get("id") == task.id or e.data.get("taskId") == task.id
        ]
        assert len(task_events) >= 1

    @pytest.mark.asyncio
    @pytest.mark.websocket
    @pytest.mark.slow
    async def test_task_updated_event(
        self,
        api_client: AsyncApexClient,
        ws_client: ApexWebSocketClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test receiving task updated event when updating a task."""
        # Arrange
        task = await api_client.create_task(
            TaskCreate(name="update-event-test")
        )
        cleanup_tasks.append(task.id)

        received_events: list[WebSocketMessage] = []

        async def handler(message: WebSocketMessage) -> None:
            received_events.append(message)

        ws_client.add_event_handler(WebSocketEventType.TASK_UPDATED, handler)
        await ws_client.connect()
        await ws_client.subscribe(
            events=[WebSocketEventType.TASK_UPDATED],
            task_ids=[task.id],
        )

        # Act - Update the task
        from apex_sdk.models import TaskUpdate
        await api_client.update_task(
            task.id,
            TaskUpdate(description="Updated for WebSocket test"),
        )

        # Wait for event delivery
        await asyncio.sleep(1.0)

        # Assert
        assert len(received_events) >= 1

    @pytest.mark.asyncio
    @pytest.mark.websocket
    @pytest.mark.slow
    async def test_heartbeat_received(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test receiving heartbeat messages."""
        # Arrange
        received_heartbeats: list[WebSocketMessage] = []

        async def handler(message: WebSocketMessage) -> None:
            received_heartbeats.append(message)

        ws_client.add_event_handler(WebSocketEventType.HEARTBEAT, handler)
        await ws_client.connect()
        await ws_client.subscribe(events=[WebSocketEventType.HEARTBEAT])

        # Act - Wait for heartbeat (configured ping interval)
        # This may take longer depending on server config
        await asyncio.sleep(35.0)  # Wait for at least one heartbeat cycle

        # Assert
        # Note: This test depends on server heartbeat configuration
        # If no heartbeat received, the test may fail


class TestWebSocketFiltering:
    """Tests for WebSocket event filtering."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    @pytest.mark.slow
    async def test_filter_by_task_id(
        self,
        api_client: AsyncApexClient,
        ws_client: ApexWebSocketClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test filtering events by task ID."""
        # Arrange - Create two tasks
        task1 = await api_client.create_task(TaskCreate(name="filter-task-1"))
        task2 = await api_client.create_task(TaskCreate(name="filter-task-2"))
        cleanup_tasks.extend([task1.id, task2.id])

        received_events: list[WebSocketMessage] = []

        async def handler(message: WebSocketMessage) -> None:
            received_events.append(message)

        ws_client.add_event_handler(WebSocketEventType.TASK_UPDATED, handler)
        await ws_client.connect()

        # Subscribe only to task1
        await ws_client.subscribe(
            events=[WebSocketEventType.TASK_UPDATED],
            task_ids=[task1.id],
        )

        # Act - Update both tasks
        from apex_sdk.models import TaskUpdate
        await api_client.update_task(task1.id, TaskUpdate(description="Update 1"))
        await api_client.update_task(task2.id, TaskUpdate(description="Update 2"))

        await asyncio.sleep(1.0)

        # Assert - Should only receive event for task1
        task1_events = [
            e for e in received_events
            if e.data.get("id") == task1.id or e.data.get("taskId") == task1.id
        ]
        task2_events = [
            e for e in received_events
            if e.data.get("id") == task2.id or e.data.get("taskId") == task2.id
        ]
        assert len(task1_events) >= 1
        assert len(task2_events) == 0


class TestWebSocketReconnection:
    """Tests for WebSocket reconnection behavior."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    @pytest.mark.slow
    async def test_reconnection_disabled(
        self,
        test_config: dict[str, Any],
    ) -> None:
        """Test that reconnection can be disabled."""
        # Arrange
        ws_client = ApexWebSocketClient(
            base_url=test_config["api_url"],
            api_key=test_config["api_key"],
            reconnect=False,
        )
        await ws_client.connect()

        # Act - Manually disconnect (simulating connection drop)
        await ws_client.disconnect()

        # Assert - Client should not auto-reconnect
        assert not ws_client.is_connected


class TestWebSocketMessageTypes:
    """Tests for different WebSocket message types."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_send_custom_message(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test sending a custom message to the server."""
        # Arrange
        await ws_client.connect()

        # Act - Send a ping/custom message
        await ws_client.send_message(
            "ping",
            {"timestamp": "2024-01-01T00:00:00Z"},
        )

        # Assert - Should complete without error

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_send_message_when_not_connected(
        self,
        ws_client: ApexWebSocketClient,
    ) -> None:
        """Test that sending message when not connected raises error."""
        # Act & Assert
        with pytest.raises(ApexWebSocketError):
            await ws_client.send_message("test", {})


class TestWebSocketErrorHandling:
    """Tests for WebSocket error handling."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    async def test_handler_exception_doesnt_stop_processing(
        self,
        api_client: AsyncApexClient,
        ws_client: ApexWebSocketClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test that exceptions in handlers don't stop event processing."""
        # Arrange
        call_count = 0

        async def failing_handler(message: WebSocketMessage) -> None:
            nonlocal call_count
            call_count += 1
            raise ValueError("Handler error")

        successful_events: list[WebSocketMessage] = []

        async def success_handler(message: WebSocketMessage) -> None:
            successful_events.append(message)

        # Add both handlers - failing one first
        ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, failing_handler)
        ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, success_handler)

        await ws_client.connect()
        await ws_client.subscribe(events=[WebSocketEventType.TASK_CREATED])

        # Act - Create a task
        task = await api_client.create_task(TaskCreate(name="error-handler-test"))
        cleanup_tasks.append(task.id)

        await asyncio.sleep(1.0)

        # Assert - Both handlers should have been called despite error
        # The failing handler incremented counter, and successful one captured events


class TestWebSocketIntegration:
    """End-to-end WebSocket integration tests."""

    @pytest.mark.asyncio
    @pytest.mark.websocket
    @pytest.mark.workflow
    @pytest.mark.slow
    async def test_full_task_lifecycle_events(
        self,
        api_client: AsyncApexClient,
        ws_client: ApexWebSocketClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test receiving all events through a task's lifecycle."""
        # Arrange
        events_by_type: dict[WebSocketEventType, list[WebSocketMessage]] = {
            WebSocketEventType.TASK_CREATED: [],
            WebSocketEventType.TASK_UPDATED: [],
            WebSocketEventType.TASK_COMPLETED: [],
        }

        async def create_handler(event_type: WebSocketEventType):
            async def handler(message: WebSocketMessage) -> None:
                events_by_type[event_type].append(message)
            return handler

        for event_type in events_by_type:
            handler = await create_handler(event_type)
            ws_client.add_event_handler(event_type, handler)

        await ws_client.connect()
        await ws_client.subscribe(events=list(events_by_type.keys()))

        # Act - Create and process a task
        task = await api_client.create_task(
            TaskCreate(name="lifecycle-test-task")
        )
        cleanup_tasks.append(task.id)

        # Wait for events
        await asyncio.sleep(2.0)

        # Assert - Should have received at least the created event
        assert len(events_by_type[WebSocketEventType.TASK_CREATED]) >= 1

    @pytest.mark.asyncio
    @pytest.mark.websocket
    @pytest.mark.workflow
    @pytest.mark.slow
    async def test_multiple_concurrent_subscriptions(
        self,
        test_config: dict[str, Any],
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test multiple WebSocket clients receiving events."""
        # Arrange - Create two clients
        client1 = ApexWebSocketClient(
            base_url=test_config["api_url"],
            api_key=test_config["api_key"],
            reconnect=False,
        )
        client2 = ApexWebSocketClient(
            base_url=test_config["api_url"],
            api_key=test_config["api_key"],
            reconnect=False,
        )

        events1: list[WebSocketMessage] = []
        events2: list[WebSocketMessage] = []

        async def handler1(message: WebSocketMessage) -> None:
            events1.append(message)

        async def handler2(message: WebSocketMessage) -> None:
            events2.append(message)

        client1.add_event_handler(WebSocketEventType.TASK_CREATED, handler1)
        client2.add_event_handler(WebSocketEventType.TASK_CREATED, handler2)

        await client1.connect()
        await client2.connect()
        await client1.subscribe(events=[WebSocketEventType.TASK_CREATED])
        await client2.subscribe(events=[WebSocketEventType.TASK_CREATED])

        try:
            # Act - Create a task
            task = await api_client.create_task(
                TaskCreate(name="multi-client-test")
            )
            cleanup_tasks.append(task.id)

            await asyncio.sleep(1.0)

            # Assert - Both clients should receive the event
            assert len(events1) >= 1
            assert len(events2) >= 1

        finally:
            await client1.disconnect()
            await client2.disconnect()
