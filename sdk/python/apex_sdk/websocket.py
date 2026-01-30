"""WebSocket client for real-time updates from the Apex API."""

from __future__ import annotations

import asyncio
import json
import logging
from collections.abc import AsyncGenerator
from typing import Any, Callable

import websockets
from websockets.asyncio.client import ClientConnection
from websockets.exceptions import ConnectionClosed, WebSocketException

from .exceptions import ApexWebSocketClosed, ApexWebSocketError
from .models import WebSocketEventType, WebSocketMessage, WebSocketSubscription

logger = logging.getLogger(__name__)


class ApexWebSocketClient:
    """WebSocket client for streaming real-time updates from the Apex API."""

    def __init__(
        self,
        base_url: str,
        api_key: str | None = None,
        token: str | None = None,
        reconnect: bool = True,
        reconnect_delay: float = 1.0,
        max_reconnect_delay: float = 60.0,
        ping_interval: float = 30.0,
        ping_timeout: float = 10.0,
    ) -> None:
        """
        Initialize the WebSocket client.

        Args:
            base_url: The base URL of the Apex API (will be converted to ws/wss).
            api_key: API key for authentication.
            token: Bearer token for authentication (alternative to api_key).
            reconnect: Whether to automatically reconnect on disconnection.
            reconnect_delay: Initial delay between reconnection attempts.
            max_reconnect_delay: Maximum delay between reconnection attempts.
            ping_interval: Interval between ping messages.
            ping_timeout: Timeout for ping responses.
        """
        # Convert HTTP URL to WebSocket URL
        ws_url = base_url.replace("http://", "ws://").replace("https://", "wss://")
        self.ws_url = f"{ws_url.rstrip('/')}/ws"
        self.api_key = api_key
        self.token = token
        self.reconnect = reconnect
        self.reconnect_delay = reconnect_delay
        self.max_reconnect_delay = max_reconnect_delay
        self.ping_interval = ping_interval
        self.ping_timeout = ping_timeout

        self._connection: ClientConnection | None = None
        self._subscription: WebSocketSubscription | None = None
        self._running = False
        self._reconnect_attempt = 0
        self._event_handlers: dict[WebSocketEventType, list[Callable[[WebSocketMessage], Any]]] = {}

    def _get_headers(self) -> dict[str, str]:
        """Get authentication headers for the WebSocket connection."""
        headers: dict[str, str] = {}
        if self.api_key:
            headers["X-API-Key"] = self.api_key
        elif self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        return headers

    async def connect(self) -> None:
        """Establish the WebSocket connection."""
        if self._connection is not None:
            return

        try:
            self._connection = await websockets.connect(
                self.ws_url,
                additional_headers=self._get_headers(),
                ping_interval=self.ping_interval,
                ping_timeout=self.ping_timeout,
            )
            self._reconnect_attempt = 0
            logger.info("WebSocket connected to %s", self.ws_url)

            # Re-subscribe if we had an active subscription
            if self._subscription:
                await self._send_subscription(self._subscription)

        except WebSocketException as e:
            raise ApexWebSocketError(f"Failed to connect to WebSocket: {e}") from e

    async def disconnect(self) -> None:
        """Close the WebSocket connection."""
        self._running = False
        if self._connection is not None:
            await self._connection.close()
            self._connection = None
            logger.info("WebSocket disconnected")

    async def subscribe(
        self,
        events: list[WebSocketEventType] | None = None,
        task_ids: list[str] | None = None,
        agent_ids: list[str] | None = None,
        dag_ids: list[str] | None = None,
    ) -> None:
        """
        Subscribe to specific events.

        Args:
            events: List of event types to subscribe to. If None, subscribes to all.
            task_ids: Filter events by task IDs.
            agent_ids: Filter events by agent IDs.
            dag_ids: Filter events by DAG IDs.
        """
        self._subscription = WebSocketSubscription(
            events=events or list(WebSocketEventType),
            task_ids=task_ids,
            agent_ids=agent_ids,
            dag_ids=dag_ids,
        )

        if self._connection is not None:
            await self._send_subscription(self._subscription)

    async def _send_subscription(self, subscription: WebSocketSubscription) -> None:
        """Send a subscription message to the server."""
        if self._connection is None:
            raise ApexWebSocketError("Not connected")

        message = {
            "type": "subscribe",
            "data": subscription.model_dump(by_alias=True, exclude_none=True),
        }
        await self._connection.send(json.dumps(message))
        logger.debug("Sent subscription: %s", subscription)

    def on_event(
        self, event_type: WebSocketEventType
    ) -> Callable[[Callable[[WebSocketMessage], Any]], Callable[[WebSocketMessage], Any]]:
        """
        Decorator to register an event handler.

        Usage:
            @client.on_event(WebSocketEventType.TASK_COMPLETED)
            async def handle_task_completed(message: WebSocketMessage):
                print(f"Task completed: {message.data}")
        """

        def decorator(
            func: Callable[[WebSocketMessage], Any]
        ) -> Callable[[WebSocketMessage], Any]:
            if event_type not in self._event_handlers:
                self._event_handlers[event_type] = []
            self._event_handlers[event_type].append(func)
            return func

        return decorator

    def add_event_handler(
        self, event_type: WebSocketEventType, handler: Callable[[WebSocketMessage], Any]
    ) -> None:
        """Add an event handler programmatically."""
        if event_type not in self._event_handlers:
            self._event_handlers[event_type] = []
        self._event_handlers[event_type].append(handler)

    def remove_event_handler(
        self, event_type: WebSocketEventType, handler: Callable[[WebSocketMessage], Any]
    ) -> None:
        """Remove an event handler."""
        if event_type in self._event_handlers:
            self._event_handlers[event_type].remove(handler)

    async def _dispatch_event(self, message: WebSocketMessage) -> None:
        """Dispatch an event to registered handlers."""
        handlers = self._event_handlers.get(message.type, [])
        for handler in handlers:
            try:
                result = handler(message)
                if asyncio.iscoroutine(result):
                    await result
            except Exception as e:
                logger.error("Error in event handler for %s: %s", message.type, e)

    async def _receive_message(self) -> WebSocketMessage:
        """Receive and parse a message from the WebSocket."""
        if self._connection is None:
            raise ApexWebSocketError("Not connected")

        try:
            raw_message = await self._connection.recv()
            data = json.loads(raw_message)
            return WebSocketMessage(**data)
        except json.JSONDecodeError as e:
            raise ApexWebSocketError(f"Invalid JSON message: {e}") from e
        except ConnectionClosed as e:
            raise ApexWebSocketClosed("Connection closed", code=e.code) from e

    async def listen(self) -> AsyncGenerator[WebSocketMessage, None]:
        """
        Listen for messages as an async generator.

        Yields:
            WebSocketMessage objects as they are received.

        Usage:
            async for message in client.listen():
                print(f"Received: {message.type}")
        """
        self._running = True
        current_delay = self.reconnect_delay

        while self._running:
            try:
                if self._connection is None:
                    await self.connect()

                message = await self._receive_message()

                # Dispatch to handlers
                await self._dispatch_event(message)

                # Reset reconnect delay on successful message
                current_delay = self.reconnect_delay

                yield message

            except ApexWebSocketClosed as e:
                logger.warning("WebSocket closed: %s (code: %s)", e.message, e.code)
                self._connection = None

                if not self.reconnect or not self._running:
                    raise

                # Exponential backoff
                self._reconnect_attempt += 1
                current_delay = min(
                    self.reconnect_delay * (2 ** (self._reconnect_attempt - 1)),
                    self.max_reconnect_delay,
                )
                logger.info(
                    "Reconnecting in %.1f seconds (attempt %d)...",
                    current_delay,
                    self._reconnect_attempt,
                )
                await asyncio.sleep(current_delay)

            except ApexWebSocketError as e:
                logger.error("WebSocket error: %s", e)
                self._connection = None

                if not self.reconnect or not self._running:
                    raise

                await asyncio.sleep(current_delay)

    async def run(self) -> None:
        """
        Run the WebSocket client with event handlers.

        This method blocks until disconnect() is called.
        """
        async for _ in self.listen():
            pass

    async def send_message(self, message_type: str, data: dict[str, Any]) -> None:
        """
        Send a custom message to the server.

        Args:
            message_type: The type of message to send.
            data: The message data.
        """
        if self._connection is None:
            raise ApexWebSocketError("Not connected")

        message = {"type": message_type, "data": data}
        await self._connection.send(json.dumps(message))

    @property
    def is_connected(self) -> bool:
        """Check if the WebSocket is connected."""
        return self._connection is not None and self._connection.state.name == "OPEN"

    async def __aenter__(self) -> "ApexWebSocketClient":
        """Async context manager entry."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        """Async context manager exit."""
        await self.disconnect()
