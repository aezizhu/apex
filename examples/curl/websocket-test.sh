#!/bin/bash
#
# Apex WebSocket API - Testing with wscat
#
# This script demonstrates WebSocket connections and message handling.
# It uses wscat for interactive testing and provides automated test scenarios.
#
# Prerequisites:
#   - wscat installed (npm install -g wscat)
#   - Apex server running at ws://localhost:8080/ws
#
# Usage:
#   chmod +x websocket-test.sh
#   ./websocket-test.sh
#
# Or run individual tests:
#   source websocket-test.sh
#   interactive_session

set -e

# =============================================================================
# Configuration
# =============================================================================

# WebSocket URL - change this if your server is running elsewhere
WS_URL="${APEX_WS_URL:-ws://localhost:8080/ws}"
API_URL="${APEX_API_URL:-http://localhost:8080}"

# API Key for authentication
API_KEY="${APEX_API_KEY:-}"

# Build WebSocket URL with authentication
if [ -n "$API_KEY" ]; then
    WS_URL_WITH_AUTH="${WS_URL}?apiKey=${API_KEY}"
else
    WS_URL_WITH_AUTH="${WS_URL}"
fi

# =============================================================================
# Helper Functions
# =============================================================================

check_wscat() {
    if ! command -v wscat &> /dev/null; then
        echo "Error: wscat is not installed."
        echo "Install with: npm install -g wscat"
        exit 1
    fi
}

check_websocat() {
    if ! command -v websocat &> /dev/null; then
        echo "Warning: websocat is not installed."
        echo "Some automated tests require websocat."
        echo "Install with: brew install websocat (macOS) or cargo install websocat"
        return 1
    fi
    return 0
}

# =============================================================================
# Interactive Session
# =============================================================================

interactive_session() {
    echo "============================================================"
    echo "Interactive WebSocket Session"
    echo "============================================================"
    echo "Connecting to: $WS_URL"
    echo ""
    echo "Commands you can send:"
    echo "  {\"type\": \"ping\"}                              - Send ping"
    echo "  {\"type\": \"subscribe\", \"event\": \"task.*\"}     - Subscribe to task events"
    echo "  {\"type\": \"subscribe\", \"event\": \"agent.*\"}    - Subscribe to agent events"
    echo "  {\"type\": \"unsubscribe\", \"subscriptionId\": \"...\"} - Unsubscribe"
    echo ""
    echo "Press Ctrl+C to exit"
    echo ""

    check_wscat

    if [ -n "$API_KEY" ]; then
        wscat -c "$WS_URL_WITH_AUTH"
    else
        wscat -c "$WS_URL"
    fi
}

# =============================================================================
# Subscription Examples
# =============================================================================

show_subscription_examples() {
    echo "============================================================"
    echo "WebSocket Subscription Examples"
    echo "============================================================"
    echo ""
    echo "Copy and paste these JSON messages in wscat:"
    echo ""

    echo "--- Subscribe to all task events ---"
    echo '{
  "type": "subscribe",
  "subscriptionId": "sub_tasks_all",
  "event": ["task.created", "task.updated", "task.completed", "task.failed"],
  "filter": {}
}'
    echo ""

    echo "--- Subscribe to specific task ---"
    echo '{
  "type": "subscribe",
  "subscriptionId": "sub_task_123",
  "event": ["task.updated", "task.completed", "task.failed", "log.message"],
  "filter": {
    "taskId": "your-task-id-here"
  }
}'
    echo ""

    echo "--- Subscribe to agent events ---"
    echo '{
  "type": "subscribe",
  "subscriptionId": "sub_agents",
  "event": "agent.status_changed",
  "filter": {}
}'
    echo ""

    echo "--- Subscribe to specific agent ---"
    echo '{
  "type": "subscribe",
  "subscriptionId": "sub_agent_123",
  "event": "agent.status_changed",
  "filter": {
    "agentId": "your-agent-id-here"
  }
}'
    echo ""

    echo "--- Subscribe to DAG events ---"
    echo '{
  "type": "subscribe",
  "subscriptionId": "sub_dags",
  "event": ["dag.started", "dag.completed", "dag.failed"],
  "filter": {}
}'
    echo ""

    echo "--- Subscribe to specific DAG ---"
    echo '{
  "type": "subscribe",
  "subscriptionId": "sub_dag_123",
  "event": ["dag.started", "dag.completed", "dag.failed"],
  "filter": {
    "dagId": "your-dag-id-here"
  }
}'
    echo ""

    echo "--- Subscribe to approval events ---"
    echo '{
  "type": "subscribe",
  "subscriptionId": "sub_approvals",
  "event": ["approval.required", "approval.resolved"],
  "filter": {}
}'
    echo ""

    echo "--- Unsubscribe ---"
    echo '{
  "type": "unsubscribe",
  "subscriptionId": "sub_tasks_all"
}'
    echo ""

    echo "--- Ping (keep-alive) ---"
    echo '{
  "type": "ping"
}'
    echo ""
}

# =============================================================================
# Event Format Reference
# =============================================================================

show_event_formats() {
    echo "============================================================"
    echo "WebSocket Event Formats"
    echo "============================================================"
    echo ""
    echo "These are the event formats you'll receive from the server:"
    echo ""

    echo "--- task.created ---"
    echo '{
  "type": "task.created",
  "payload": {
    "task": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Research Task",
      "status": "pending",
      "priority": "normal",
      "createdAt": "2024-01-15T10:30:00Z"
    }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}'
    echo ""

    echo "--- task.updated ---"
    echo '{
  "type": "task.updated",
  "payload": {
    "task": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Research Task",
      "status": "running"
    },
    "changes": {
      "status": "running"
    }
  },
  "timestamp": "2024-01-15T10:30:05Z"
}'
    echo ""

    echo "--- task.completed ---"
    echo '{
  "type": "task.completed",
  "payload": {
    "task": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Research Task",
      "status": "completed",
      "output": { "result": "..." }
    },
    "duration": 12345
  },
  "timestamp": "2024-01-15T10:30:30Z"
}'
    echo ""

    echo "--- task.failed ---"
    echo '{
  "type": "task.failed",
  "payload": {
    "task": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Research Task",
      "status": "failed"
    },
    "error": {
      "code": "EXECUTION_ERROR",
      "message": "Task execution failed"
    }
  },
  "timestamp": "2024-01-15T10:30:30Z"
}'
    echo ""

    echo "--- agent.status_changed ---"
    echo '{
  "type": "agent.status_changed",
  "payload": {
    "agent": {
      "id": "agent-123",
      "name": "research-agent-01",
      "status": "busy",
      "currentTaskId": "task-456"
    },
    "previousStatus": "idle"
  },
  "timestamp": "2024-01-15T10:30:00Z"
}'
    echo ""

    echo "--- dag.started ---"
    echo '{
  "type": "dag.started",
  "payload": {
    "dag": {
      "id": "dag-123",
      "name": "Research Pipeline",
      "status": "running"
    },
    "execution": {
      "id": "exec-456",
      "startedAt": "2024-01-15T10:30:00Z"
    }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}'
    echo ""

    echo "--- dag.completed ---"
    echo '{
  "type": "dag.completed",
  "payload": {
    "dag": {
      "id": "dag-123",
      "name": "Research Pipeline",
      "status": "completed"
    },
    "execution": {
      "id": "exec-456",
      "completedAt": "2024-01-15T10:35:00Z"
    },
    "duration": 300000
  },
  "timestamp": "2024-01-15T10:35:00Z"
}'
    echo ""

    echo "--- approval.required ---"
    echo '{
  "type": "approval.required",
  "payload": {
    "approval": {
      "id": "approval-123",
      "taskId": "task-456",
      "status": "pending",
      "requestedBy": "user@example.com",
      "approvers": ["admin@example.com"],
      "reason": "High-risk operation",
      "expiresAt": "2024-01-16T10:30:00Z"
    }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}'
    echo ""

    echo "--- log.message ---"
    echo '{
  "type": "log.message",
  "payload": {
    "log": {
      "id": "log-123",
      "taskId": "task-456",
      "level": "info",
      "message": "Processing step 1 of 3",
      "timestamp": "2024-01-15T10:30:15Z"
    }
  },
  "timestamp": "2024-01-15T10:30:15Z"
}'
    echo ""

    echo "--- heartbeat ---"
    echo '{
  "type": "heartbeat",
  "payload": {
    "serverTime": "2024-01-15T10:30:00Z",
    "connectionId": "conn-789"
  },
  "timestamp": "2024-01-15T10:30:00Z"
}'
    echo ""
}

# =============================================================================
# Automated Tests (requires websocat)
# =============================================================================

test_connection() {
    echo "============================================================"
    echo "Test: Basic Connection"
    echo "============================================================"
    echo ""

    if ! check_websocat; then
        echo "Skipping automated test (websocat not installed)"
        return
    fi

    echo "Testing connection to $WS_URL..."

    # Send a ping and wait for response
    RESPONSE=$(echo '{"type": "ping"}' | timeout 5 websocat "$WS_URL_WITH_AUTH" 2>&1 || true)

    if [ -n "$RESPONSE" ]; then
        echo "Connection successful!"
        echo "Response: $RESPONSE"
    else
        echo "Connection failed or timed out"
    fi
}

test_subscription() {
    echo "============================================================"
    echo "Test: Event Subscription"
    echo "============================================================"
    echo ""

    if ! check_websocat; then
        echo "Skipping automated test (websocat not installed)"
        return
    fi

    echo "Testing subscription to task events..."

    # Subscribe to task events
    SUBSCRIBE_MSG='{"type": "subscribe", "subscriptionId": "test_sub", "event": ["task.created", "task.updated"]}'

    # Send subscription and capture response
    RESPONSE=$(echo "$SUBSCRIBE_MSG" | timeout 5 websocat "$WS_URL_WITH_AUTH" 2>&1 || true)

    if [ -n "$RESPONSE" ]; then
        echo "Subscription response: $RESPONSE"
    else
        echo "No response received (subscription may still be active)"
    fi
}

test_full_flow() {
    echo "============================================================"
    echo "Test: Full Flow (WebSocket + REST)"
    echo "============================================================"
    echo ""

    if ! check_websocat; then
        echo "Skipping automated test (websocat not installed)"
        return
    fi

    echo "This test:"
    echo "1. Opens WebSocket connection"
    echo "2. Subscribes to task events"
    echo "3. Creates a task via REST API"
    echo "4. Receives task.created event via WebSocket"
    echo ""

    # Create a named pipe for WebSocket communication
    PIPE_FILE=$(mktemp -u)
    mkfifo "$PIPE_FILE"

    # Start WebSocket listener in background
    echo "Starting WebSocket listener..."
    (
        # Subscribe to task events
        echo '{"type": "subscribe", "subscriptionId": "flow_test", "event": ["task.created"]}'
        # Keep connection open
        sleep 30
    ) | websocat "$WS_URL_WITH_AUTH" > "$PIPE_FILE" 2>&1 &
    WS_PID=$!

    # Give WebSocket time to connect
    sleep 2

    # Create a task via REST API
    echo "Creating task via REST API..."
    TASK_RESPONSE=$(curl -s -X POST "${API_URL}/api/v1/tasks" \
        -H "Content-Type: application/json" \
        -H "Accept: application/json" \
        ${API_KEY:+-H "Authorization: Bearer $API_KEY"} \
        -d '{
            "name": "WebSocket Test Task",
            "description": "Task to test WebSocket events"
        }')

    echo "REST API Response: $TASK_RESPONSE"

    # Read WebSocket events
    echo ""
    echo "WebSocket events received:"
    timeout 5 cat "$PIPE_FILE" || true

    # Cleanup
    kill $WS_PID 2>/dev/null || true
    rm -f "$PIPE_FILE"

    echo ""
    echo "Test completed!"
}

# =============================================================================
# Dashboard Monitor (using websocat)
# =============================================================================

dashboard_monitor() {
    echo "============================================================"
    echo "Real-Time Dashboard Monitor"
    echo "============================================================"
    echo ""

    if ! check_websocat; then
        echo "Skipping (websocat not installed)"
        return
    fi

    echo "Starting real-time dashboard monitor..."
    echo "Press Ctrl+C to stop"
    echo ""

    # Subscribe to all events and display them
    SUBSCRIBE_MSG='{
        "type": "subscribe",
        "subscriptionId": "dashboard",
        "event": [
            "task.created",
            "task.updated",
            "task.completed",
            "task.failed",
            "agent.status_changed",
            "dag.started",
            "dag.completed",
            "dag.failed",
            "approval.required"
        ]
    }'

    # Use websocat with pretty printing
    echo "$SUBSCRIBE_MSG" | websocat "$WS_URL_WITH_AUTH" | while read -r line; do
        TIMESTAMP=$(date '+%H:%M:%S')
        EVENT_TYPE=$(echo "$line" | jq -r '.type // "unknown"' 2>/dev/null)
        echo "[$TIMESTAMP] $EVENT_TYPE"
        echo "$line" | jq '.' 2>/dev/null || echo "$line"
        echo ""
    done
}

# =============================================================================
# JavaScript Client Example
# =============================================================================

show_js_example() {
    echo "============================================================"
    echo "JavaScript WebSocket Client Example"
    echo "============================================================"
    echo ""
    echo "Copy this code to use in a browser or Node.js:"
    echo ""

    cat << 'EOF'
// Browser or Node.js WebSocket Client Example

const WS_URL = 'ws://localhost:8080/ws';
const API_KEY = ''; // Your API key if required

// Build URL with authentication
const url = API_KEY ? `${WS_URL}?apiKey=${API_KEY}` : WS_URL;

// Create WebSocket connection
const ws = new WebSocket(url);

// Connection opened
ws.onopen = () => {
    console.log('Connected to Apex WebSocket');

    // Subscribe to task events
    ws.send(JSON.stringify({
        type: 'subscribe',
        subscriptionId: 'task_events',
        event: ['task.created', 'task.updated', 'task.completed', 'task.failed'],
        filter: {}
    }));

    console.log('Subscribed to task events');
};

// Handle incoming messages
ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    console.log('Received:', message.type);

    switch (message.type) {
        case 'task.created':
            console.log('Task created:', message.payload.task.name);
            break;
        case 'task.completed':
            console.log('Task completed:', message.payload.task.name);
            console.log('Duration:', message.payload.duration, 'ms');
            break;
        case 'task.failed':
            console.log('Task failed:', message.payload.error.message);
            break;
        case 'heartbeat':
            console.log('Heartbeat received');
            break;
        default:
            console.log('Event:', message);
    }
};

// Handle errors
ws.onerror = (error) => {
    console.error('WebSocket error:', error);
};

// Handle connection close
ws.onclose = (event) => {
    console.log('WebSocket closed:', event.code, event.reason);
};

// Send ping periodically to keep connection alive
setInterval(() => {
    if (ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'ping' }));
    }
}, 30000);

// Clean up on page unload
window.addEventListener('beforeunload', () => {
    ws.close();
});
EOF
    echo ""
}

# =============================================================================
# Python Client Example
# =============================================================================

show_python_example() {
    echo "============================================================"
    echo "Python WebSocket Client Example"
    echo "============================================================"
    echo ""
    echo "Copy this code for a Python WebSocket client:"
    echo ""

    cat << 'EOF'
#!/usr/bin/env python3
"""Simple Python WebSocket client for Apex."""

import asyncio
import json
import websockets

WS_URL = "ws://localhost:8080/ws"
API_KEY = ""  # Your API key if required


async def main():
    # Build URL with authentication
    url = f"{WS_URL}?apiKey={API_KEY}" if API_KEY else WS_URL

    async with websockets.connect(url) as websocket:
        print("Connected to Apex WebSocket")

        # Subscribe to task events
        await websocket.send(json.dumps({
            "type": "subscribe",
            "subscriptionId": "task_events",
            "event": ["task.created", "task.updated", "task.completed", "task.failed"],
            "filter": {}
        }))
        print("Subscribed to task events")

        # Listen for messages
        async for raw_message in websocket:
            message = json.loads(raw_message)
            event_type = message.get("type", "unknown")

            print(f"Received: {event_type}")

            if event_type == "task.created":
                task = message["payload"]["task"]
                print(f"  Task created: {task['name']}")

            elif event_type == "task.completed":
                task = message["payload"]["task"]
                duration = message["payload"]["duration"]
                print(f"  Task completed: {task['name']} ({duration}ms)")

            elif event_type == "task.failed":
                error = message["payload"]["error"]
                print(f"  Task failed: {error['message']}")

            elif event_type == "heartbeat":
                print("  Heartbeat received")

            else:
                print(f"  Event: {message}")


if __name__ == "__main__":
    asyncio.run(main())
EOF
    echo ""
}

# =============================================================================
# Main Menu
# =============================================================================

show_help() {
    echo "============================================================"
    echo "Apex WebSocket API - Testing with wscat"
    echo "============================================================"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  interactive         - Start interactive wscat session"
    echo "  subscriptions       - Show subscription examples"
    echo "  events              - Show event format reference"
    echo "  test_connection     - Test WebSocket connection"
    echo "  test_subscription   - Test event subscription"
    echo "  test_full_flow      - Test full flow (WebSocket + REST)"
    echo "  dashboard           - Start real-time dashboard monitor"
    echo "  js_example          - Show JavaScript client example"
    echo "  python_example      - Show Python client example"
    echo "  help                - Show this help"
    echo ""
    echo "Environment Variables:"
    echo "  APEX_WS_URL         - WebSocket URL (default: ws://localhost:8080/ws)"
    echo "  APEX_API_URL        - REST API URL (default: http://localhost:8080)"
    echo "  APEX_API_KEY        - API key for authentication"
    echo ""
    echo "Requirements:"
    echo "  - wscat: npm install -g wscat"
    echo "  - websocat (optional): brew install websocat"
}

# =============================================================================
# Entry Point
# =============================================================================

if [ $# -eq 0 ]; then
    show_help
    exit 0
fi

case "$1" in
    interactive)      interactive_session ;;
    subscriptions)    show_subscription_examples ;;
    events)           show_event_formats ;;
    test_connection)  test_connection ;;
    test_subscription) test_subscription ;;
    test_full_flow)   test_full_flow ;;
    dashboard)        dashboard_monitor ;;
    js_example)       show_js_example ;;
    python_example)   show_python_example ;;
    help|--help|-h)   show_help ;;
    *)
        echo "Unknown command: $1"
        echo ""
        show_help
        exit 1
        ;;
esac
