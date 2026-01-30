/**
 * Apex TypeScript SDK - Real-Time Dashboard with WebSocket
 *
 * This example demonstrates building a live monitoring dashboard that
 * streams events from the Apex orchestrator over WebSocket:
 *
 * - Establishing and authenticating WebSocket connections
 * - Subscribing to filtered event streams
 * - Typed event handlers for every event type
 * - Building an auto-refreshing terminal dashboard
 * - Tracking aggregate statistics in real time
 * - Graceful reconnection with exponential back-off
 * - Combining REST polling with WebSocket streaming
 *
 * Prerequisites:
 *   npm install @apex-swarm/sdk
 *
 * Run with:
 *   npx ts-node realtime-dashboard.ts
 */

import {
  ApexClient,
  ApexWebSocket,
  WebSocketEventType,
  TaskCreatedPayload,
  TaskUpdatedPayload,
  TaskCompletedPayload,
  TaskFailedPayload,
  AgentStatusChangedPayload,
  DAGStartedPayload,
  DAGCompletedPayload,
  DAGFailedPayload,
  ApprovalRequiredPayload,
  HeartbeatPayload,
  WebSocketMessage,
  TaskStatus,
  AgentStatus,
  ApexError,
} from '@apex-swarm/sdk';

// =============================================================================
// Configuration
// =============================================================================

const API_URL: string = process.env.APEX_API_URL || 'http://localhost:8080';
const API_KEY: string = process.env.APEX_API_KEY || '';

/** How long the dashboard runs before auto-exiting (milliseconds). */
const DASHBOARD_DURATION_MS: number = parseInt(
  process.env.DASHBOARD_DURATION_MS || '120000',
  10
);

/** Dashboard refresh interval (milliseconds). */
const REFRESH_INTERVAL_MS = 2000;

const client = new ApexClient({
  baseUrl: API_URL,
  apiKey: API_KEY,
});

// =============================================================================
// Dashboard State
// =============================================================================

/** Strongly-typed dashboard statistics. */
interface DashboardState {
  /** Counters for task lifecycle events. */
  tasks: {
    created: number;
    running: number;
    completed: number;
    failed: number;
    cancelled: number;
  };
  /** Counters for agent status distribution. */
  agents: {
    idle: number;
    busy: number;
    offline: number;
    error: number;
  };
  /** Counters for DAG execution events. */
  dags: {
    started: number;
    completed: number;
    failed: number;
  };
  /** Pending approval count. */
  pendingApprovals: number;
  /** Rolling window of recent events for the activity feed. */
  recentEvents: Array<{
    time: string;
    category: 'task' | 'agent' | 'dag' | 'approval' | 'system';
    message: string;
  }>;
  /** Connection metadata. */
  connection: {
    connected: boolean;
    connectionId: string | null;
    reconnectCount: number;
    lastHeartbeat: string | null;
  };
}

/** Initialize a fresh dashboard state. */
function createInitialState(): DashboardState {
  return {
    tasks: { created: 0, running: 0, completed: 0, failed: 0, cancelled: 0 },
    agents: { idle: 0, busy: 0, offline: 0, error: 0 },
    dags: { started: 0, completed: 0, failed: 0 },
    pendingApprovals: 0,
    recentEvents: [],
    connection: {
      connected: false,
      connectionId: null,
      reconnectCount: 0,
      lastHeartbeat: null,
    },
  };
}

// Mutable state singleton
const state: DashboardState = createInitialState();

// =============================================================================
// Event Handlers
// =============================================================================

/**
 * Add an event to the recent-events feed.
 * Keeps at most 10 entries.
 */
function addEvent(
  category: DashboardState['recentEvents'][number]['category'],
  message: string
): void {
  state.recentEvents.unshift({
    time: new Date().toLocaleTimeString(),
    category,
    message,
  });
  if (state.recentEvents.length > 10) {
    state.recentEvents.pop();
  }
}

/** Wire all WebSocket event handlers to update dashboard state. */
function registerEventHandlers(ws: ApexWebSocket): void {
  // -- Connection lifecycle events ------------------------------------------

  ws.on('open', () => {
    state.connection.connected = true;
    addEvent('system', 'WebSocket connected');
  });

  ws.on('close', (_code: number, reason: string) => {
    state.connection.connected = false;
    addEvent('system', `WebSocket closed: ${reason}`);
  });

  ws.on('reconnecting', (attempt: number) => {
    state.connection.reconnectCount = attempt;
    addEvent('system', `Reconnecting (attempt ${attempt})...`);
  });

  ws.on('reconnected', () => {
    addEvent('system', 'WebSocket reconnected');
  });

  ws.on('error', (error) => {
    addEvent('system', `Error: ${error.message}`);
  });

  // -- Heartbeat ------------------------------------------------------------

  ws.on(WebSocketEventType.HEARTBEAT, (payload: HeartbeatPayload) => {
    state.connection.lastHeartbeat = payload.serverTime;
    state.connection.connectionId = payload.connectionId;
  });

  // -- Task events ----------------------------------------------------------

  ws.on(WebSocketEventType.TASK_CREATED, (payload: TaskCreatedPayload) => {
    state.tasks.created++;
    addEvent('task', `Created: ${payload.task.name}`);
  });

  ws.on(WebSocketEventType.TASK_UPDATED, (payload: TaskUpdatedPayload) => {
    const task = payload.task;
    // Track transitions into "running" status
    if (payload.changes.status === TaskStatus.RUNNING) {
      state.tasks.running++;
    }
    addEvent('task', `Updated: ${task.name} -> ${task.status}`);
  });

  ws.on(WebSocketEventType.TASK_COMPLETED, (payload: TaskCompletedPayload) => {
    state.tasks.completed++;
    // Decrement running counter if the task was running before completion
    if (state.tasks.running > 0) state.tasks.running--;
    addEvent('task', `Completed: ${payload.task.name} (${payload.duration}ms)`);
  });

  ws.on(WebSocketEventType.TASK_FAILED, (payload: TaskFailedPayload) => {
    state.tasks.failed++;
    if (state.tasks.running > 0) state.tasks.running--;
    addEvent('task', `FAILED: ${payload.task.name} - ${payload.error.message}`);
  });

  // -- Agent events ---------------------------------------------------------

  ws.on(
    WebSocketEventType.AGENT_STATUS_CHANGED,
    (payload: AgentStatusChangedPayload) => {
      const prev = payload.previousStatus as keyof DashboardState['agents'];
      const curr = payload.agent.status as keyof DashboardState['agents'];

      // Decrement old status, increment new status
      if (prev in state.agents && state.agents[prev] > 0) {
        state.agents[prev]--;
      }
      if (curr in state.agents) {
        state.agents[curr]++;
      }

      addEvent(
        'agent',
        `${payload.agent.name}: ${payload.previousStatus} -> ${payload.agent.status}`
      );

      // Alert on error status
      if (payload.agent.status === AgentStatus.ERROR) {
        addEvent('agent', `** ALERT ** Agent ${payload.agent.name} entered ERROR state`);
      }
    }
  );

  // -- DAG events -----------------------------------------------------------

  ws.on(WebSocketEventType.DAG_STARTED, (payload: DAGStartedPayload) => {
    state.dags.started++;
    addEvent('dag', `Started: ${payload.dag.name}`);
  });

  ws.on(WebSocketEventType.DAG_COMPLETED, (payload: DAGCompletedPayload) => {
    state.dags.completed++;
    addEvent('dag', `Completed: ${payload.dag.name} (${payload.duration}ms)`);
  });

  ws.on(WebSocketEventType.DAG_FAILED, (payload: DAGFailedPayload) => {
    state.dags.failed++;
    addEvent('dag', `FAILED: ${payload.dag.name} - ${payload.error.message}`);
  });

  // -- Approval events ------------------------------------------------------

  ws.on(
    WebSocketEventType.APPROVAL_REQUIRED,
    (payload: ApprovalRequiredPayload) => {
      state.pendingApprovals++;
      addEvent('approval', `Pending: ${payload.approval.reason || 'No reason'}`);
    }
  );
}

// =============================================================================
// Renderer
// =============================================================================

/**
 * Render the dashboard to the terminal.
 *
 * Uses ANSI escape codes for clearing and positioning. Works in most
 * modern terminals (iTerm2, Windows Terminal, Kitty, etc.).
 */
function renderDashboard(): void {
  // Clear screen and move cursor to top-left
  process.stdout.write('\x1B[2J\x1B[H');

  const bar = '='.repeat(62);
  const sep = '-'.repeat(42);
  const now = new Date().toLocaleString();

  const lines: string[] = [
    bar,
    '              APEX REAL-TIME DASHBOARD',
    bar,
    '',
    '  CONNECTION',
    `  ${sep}`,
    `    Status:        ${state.connection.connected ? 'CONNECTED' : 'DISCONNECTED'}`,
    `    Connection ID: ${state.connection.connectionId || 'N/A'}`,
    `    Reconnects:    ${state.connection.reconnectCount}`,
    `    Last heartbeat: ${state.connection.lastHeartbeat || 'N/A'}`,
    '',
    '  TASKS',
    `  ${sep}`,
    `    Created:   ${state.tasks.created}`,
    `    Running:   ${state.tasks.running}`,
    `    Completed: ${state.tasks.completed}`,
    `    Failed:    ${state.tasks.failed}`,
    '',
    '  AGENTS',
    `  ${sep}`,
    `    Idle:    ${state.agents.idle}`,
    `    Busy:    ${state.agents.busy}`,
    `    Offline: ${state.agents.offline}`,
    `    Error:   ${state.agents.error}`,
    '',
    '  DAGS',
    `  ${sep}`,
    `    Started:   ${state.dags.started}`,
    `    Completed: ${state.dags.completed}`,
    `    Failed:    ${state.dags.failed}`,
    '',
    `  PENDING APPROVALS: ${state.pendingApprovals}`,
    '',
    '  RECENT ACTIVITY',
    `  ${sep}`,
  ];

  if (state.recentEvents.length === 0) {
    lines.push('    (waiting for events...)');
  } else {
    for (const evt of state.recentEvents.slice(0, 8)) {
      const cat = evt.category.toUpperCase().padEnd(8);
      lines.push(`    [${evt.time}] ${cat} ${evt.message}`);
    }
  }

  lines.push('');
  lines.push(bar);
  lines.push(`  Last refresh: ${now}`);
  lines.push('  Press Ctrl+C to stop');

  console.log(lines.join('\n'));
}

// =============================================================================
// Main
// =============================================================================

async function main(): Promise<void> {
  console.log('Starting Apex Real-Time Dashboard...\n');

  // Get the WebSocket client with auto-reconnect enabled
  const ws = client.getWebSocket({
    autoReconnect: true,
    reconnectInterval: 1000,
    maxReconnectAttempts: 20,
    heartbeatInterval: 30000,
    heartbeatTimeout: 10000,
  });

  // Register all event handlers
  registerEventHandlers(ws);

  try {
    // Connect to the WebSocket server
    await ws.connect();

    // Subscribe to all event types for the dashboard
    ws.subscribe({
      event: [
        WebSocketEventType.TASK_CREATED,
        WebSocketEventType.TASK_UPDATED,
        WebSocketEventType.TASK_COMPLETED,
        WebSocketEventType.TASK_FAILED,
        WebSocketEventType.AGENT_STATUS_CHANGED,
        WebSocketEventType.DAG_STARTED,
        WebSocketEventType.DAG_COMPLETED,
        WebSocketEventType.DAG_FAILED,
        WebSocketEventType.APPROVAL_REQUIRED,
        WebSocketEventType.APPROVAL_RESOLVED,
        WebSocketEventType.HEARTBEAT,
      ],
    });

    // Start the render loop
    const renderInterval = setInterval(renderDashboard, REFRESH_INTERVAL_MS);

    // Auto-exit after DASHBOARD_DURATION_MS
    await new Promise<void>((resolve) => {
      const exitTimer = setTimeout(() => {
        clearInterval(renderInterval);
        resolve();
      }, DASHBOARD_DURATION_MS);

      // Also handle Ctrl+C gracefully
      process.on('SIGINT', () => {
        clearTimeout(exitTimer);
        clearInterval(renderInterval);
        resolve();
      });
    });

  } catch (error) {
    if (error instanceof ApexError) {
      console.error(`Apex error [${error.code}]: ${error.message}`);
    } else {
      console.error('Dashboard error:', error);
    }
  } finally {
    // Clean shutdown
    client.disconnectWebSocket();
    console.log('\nDashboard session ended.');
  }
}

main().catch(console.error);
