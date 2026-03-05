/**
 * Apex TypeScript SDK - Real-Time Monitoring Example
 *
 * This example demonstrates WebSocket-based real-time monitoring:
 * - Establishing WebSocket connections
 * - Subscribing to different event types
 * - Handling task, agent, and DAG updates
 * - Building a real-time dashboard data feed
 * - Reconnection handling
 *
 * Prerequisites:
 *   npm install @apex-swarm/sdk
 *
 * Run with:
 *   npx ts-node real-time-monitoring.ts
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
  LogMessagePayload,
  HeartbeatPayload,
  Task,
  Agent,
  DAG,
} from '@apex-swarm/sdk';

// =============================================================================
// Configuration
// =============================================================================

const API_URL = process.env.APEX_API_URL || 'http://localhost:8080';
const API_KEY = process.env.APEX_API_KEY || '';
const WS_URL = process.env.APEX_WS_URL || API_URL.replace(/^http/, 'ws') + '/ws';

// Initialize the client
const client = new ApexClient({
  baseUrl: API_URL,
  apiKey: API_KEY,
  websocketUrl: WS_URL,
});

// =============================================================================
// Basic WebSocket Connection
// =============================================================================

/**
 * Establish a basic WebSocket connection and handle events.
 */
async function basicWebSocketExample(): Promise<void> {
  console.log('\n--- Basic WebSocket Connection ---\n');

  // Get the WebSocket client from the main client
  const ws = client.getWebSocket({
    autoReconnect: true,
    reconnectInterval: 1000,
    maxReconnectAttempts: 10,
    heartbeatInterval: 30000,
    heartbeatTimeout: 10000,
  });

  // Set up event handlers before connecting
  ws.on('open', () => {
    console.log('WebSocket connected!');
  });

  ws.on('close', (code: number, reason: string) => {
    console.log(`WebSocket closed: ${code} - ${reason}`);
  });

  ws.on('error', (error) => {
    console.error('WebSocket error:', error.message);
  });

  ws.on('reconnecting', (attempt: number) => {
    console.log(`Reconnecting... attempt ${attempt}`);
  });

  ws.on('reconnected', () => {
    console.log('WebSocket reconnected!');
  });

  // Connect to the WebSocket server
  try {
    await ws.connect();
    console.log('Connection ID:', ws.getConnectionId());
    console.log('Is connected:', ws.isConnected());
  } catch (error) {
    console.error('Failed to connect:', error);
    throw error;
  }

  return;
}

// =============================================================================
// Task Event Monitoring
// =============================================================================

/**
 * Monitor task events in real-time.
 */
async function monitorTaskEvents(): Promise<void> {
  console.log('\n--- Monitoring Task Events ---\n');

  const ws = await client.connectWebSocket();

  // Subscribe to all task events
  ws.on(WebSocketEventType.TASK_CREATED, (payload: TaskCreatedPayload) => {
    console.log('\n[TASK CREATED]');
    console.log('  ID:', payload.task.id);
    console.log('  Name:', payload.task.name);
    console.log('  Status:', payload.task.status);
    console.log('  Priority:', payload.task.priority);
  });

  ws.on(WebSocketEventType.TASK_UPDATED, (payload: TaskUpdatedPayload) => {
    console.log('\n[TASK UPDATED]');
    console.log('  ID:', payload.task.id);
    console.log('  Name:', payload.task.name);
    console.log('  Status:', payload.task.status);
    console.log('  Changes:', Object.keys(payload.changes).join(', '));
  });

  ws.on(WebSocketEventType.TASK_COMPLETED, (payload: TaskCompletedPayload) => {
    console.log('\n[TASK COMPLETED]');
    console.log('  ID:', payload.task.id);
    console.log('  Name:', payload.task.name);
    console.log('  Duration:', payload.duration, 'ms');
    if (payload.task.output) {
      console.log('  Output:', JSON.stringify(payload.task.output).slice(0, 100));
    }
  });

  ws.on(WebSocketEventType.TASK_FAILED, (payload: TaskFailedPayload) => {
    console.log('\n[TASK FAILED]');
    console.log('  ID:', payload.task.id);
    console.log('  Name:', payload.task.name);
    console.log('  Error Code:', payload.error.code);
    console.log('  Error Message:', payload.error.message);
  });

  console.log('Listening for task events...');
  console.log('Press Ctrl+C to stop\n');
}

/**
 * Monitor a specific task by ID.
 */
async function monitorSpecificTask(taskId: string): Promise<Task> {
  console.log(`\n--- Monitoring Task: ${taskId} ---\n`);

  const ws = await client.connectWebSocket();

  // Subscribe to events for this specific task
  ws.subscribeToTask(taskId);

  console.log('Subscribed to task events');

  // Log events
  ws.on(WebSocketEventType.LOG_MESSAGE, (payload: LogMessagePayload) => {
    if (payload.log.taskId === taskId) {
      console.log(`[${payload.log.level.toUpperCase()}] ${payload.log.message}`);
    }
  });

  // Wait for task completion or failure
  return new Promise((resolve, reject) => {
    ws.on(WebSocketEventType.TASK_COMPLETED, (payload: TaskCompletedPayload) => {
      if (payload.task.id === taskId) {
        console.log('\nTask completed successfully!');
        resolve(payload.task);
      }
    });

    ws.on(WebSocketEventType.TASK_FAILED, (payload: TaskFailedPayload) => {
      if (payload.task.id === taskId) {
        console.log('\nTask failed!');
        reject(new Error(payload.error.message));
      }
    });
  });
}

// =============================================================================
// Agent Status Monitoring
// =============================================================================

/**
 * Monitor agent status changes.
 */
async function monitorAgentStatus(): Promise<void> {
  console.log('\n--- Monitoring Agent Status ---\n');

  const ws = await client.connectWebSocket();

  // Track agent statuses
  const agentStatuses = new Map<string, string>();

  ws.on(
    WebSocketEventType.AGENT_STATUS_CHANGED,
    (payload: AgentStatusChangedPayload) => {
      const prev = agentStatuses.get(payload.agent.id) || 'unknown';
      agentStatuses.set(payload.agent.id, payload.agent.status);

      console.log('\n[AGENT STATUS CHANGED]');
      console.log('  Agent:', payload.agent.name);
      console.log('  ID:', payload.agent.id);
      console.log('  Previous:', payload.previousStatus);
      console.log('  Current:', payload.agent.status);
      console.log('  Current Task:', payload.agent.currentTaskId || 'none');

      // Alert on error status
      if (payload.agent.status === 'error') {
        console.log('  *** ALERT: Agent entered error state! ***');
      }
    }
  );

  console.log('Listening for agent status changes...');
}

/**
 * Monitor a specific agent.
 */
async function monitorSpecificAgent(agentId: string): Promise<void> {
  console.log(`\n--- Monitoring Agent: ${agentId} ---\n`);

  const ws = await client.connectWebSocket();

  // Subscribe to this agent's events
  ws.subscribeToAgent(agentId);

  ws.on(
    WebSocketEventType.AGENT_STATUS_CHANGED,
    (payload: AgentStatusChangedPayload) => {
      if (payload.agent.id === agentId) {
        console.log(`Agent ${payload.agent.name}: ${payload.previousStatus} -> ${payload.agent.status}`);
      }
    }
  );
}

// =============================================================================
// DAG Monitoring
// =============================================================================

/**
 * Monitor DAG execution events.
 */
async function monitorDAGExecution(): Promise<void> {
  console.log('\n--- Monitoring DAG Execution ---\n');

  const ws = await client.connectWebSocket();

  ws.on(WebSocketEventType.DAG_STARTED, (payload: DAGStartedPayload) => {
    console.log('\n[DAG STARTED]');
    console.log('  DAG:', payload.dag.name);
    console.log('  ID:', payload.dag.id);
    console.log('  Execution ID:', payload.execution.id);
    console.log('  Nodes:', payload.dag.nodes.length);
    console.log('  Started:', payload.execution.startedAt);
  });

  ws.on(WebSocketEventType.DAG_COMPLETED, (payload: DAGCompletedPayload) => {
    console.log('\n[DAG COMPLETED]');
    console.log('  DAG:', payload.dag.name);
    console.log('  ID:', payload.dag.id);
    console.log('  Execution ID:', payload.execution.id);
    console.log('  Duration:', payload.duration, 'ms');
    console.log('  Completed:', payload.execution.completedAt);
  });

  ws.on(WebSocketEventType.DAG_FAILED, (payload: DAGFailedPayload) => {
    console.log('\n[DAG FAILED]');
    console.log('  DAG:', payload.dag.name);
    console.log('  ID:', payload.dag.id);
    console.log('  Error:', payload.error.message);
  });

  console.log('Listening for DAG events...');
}

/**
 * Monitor a specific DAG execution.
 */
async function monitorSpecificDAG(dagId: string): Promise<void> {
  console.log(`\n--- Monitoring DAG: ${dagId} ---\n`);

  const ws = await client.connectWebSocket();

  // Subscribe to this DAG's events
  ws.subscribeToDAG(dagId);

  // Also monitor task events within the DAG
  ws.on(WebSocketEventType.TASK_UPDATED, (payload: TaskUpdatedPayload) => {
    if (payload.task.dagId === dagId) {
      console.log(`  Task ${payload.task.name}: ${payload.task.status}`);
    }
  });

  ws.on(WebSocketEventType.DAG_COMPLETED, (payload: DAGCompletedPayload) => {
    if (payload.dag.id === dagId) {
      console.log('\nDAG execution completed!');
      console.log('Duration:', payload.duration, 'ms');
    }
  });
}

// =============================================================================
// Approval Monitoring
// =============================================================================

/**
 * Monitor approval requests.
 */
async function monitorApprovals(): Promise<void> {
  console.log('\n--- Monitoring Approvals ---\n');

  const ws = await client.connectWebSocket();

  // Subscribe to approval events
  ws.subscribeToApprovals();

  ws.on(WebSocketEventType.APPROVAL_REQUIRED, (payload: ApprovalRequiredPayload) => {
    console.log('\n[APPROVAL REQUIRED]');
    console.log('  ID:', payload.approval.id);
    console.log('  Task ID:', payload.approval.taskId);
    console.log('  DAG ID:', payload.approval.dagId || 'N/A');
    console.log('  Requested by:', payload.approval.requestedBy);
    console.log('  Approvers:', payload.approval.approvers.join(', '));
    console.log('  Required:', payload.approval.requiredApprovals);
    console.log('  Reason:', payload.approval.reason);
    console.log('  Expires:', payload.approval.expiresAt);
  });

  ws.on(WebSocketEventType.APPROVAL_RESOLVED, (payload) => {
    console.log('\n[APPROVAL RESOLVED]');
    console.log('  ID:', payload.approval.id);
    console.log('  Decision:', payload.decision);
    console.log('  Status:', payload.approval.status);
  });

  console.log('Listening for approval events...');
}

// =============================================================================
// Heartbeat Monitoring
// =============================================================================

/**
 * Monitor WebSocket heartbeats.
 */
async function monitorHeartbeat(): Promise<void> {
  console.log('\n--- Monitoring Heartbeat ---\n');

  const ws = await client.connectWebSocket();

  ws.on(WebSocketEventType.HEARTBEAT, (payload: HeartbeatPayload) => {
    console.log(`[HEARTBEAT] Server time: ${payload.serverTime}, Connection: ${payload.connectionId}`);
  });

  console.log('Listening for heartbeats...');
}

// =============================================================================
// Dashboard Data Feed
// =============================================================================

/**
 * Statistics interface for dashboard.
 */
interface DashboardStats {
  tasks: {
    total: number;
    pending: number;
    running: number;
    completed: number;
    failed: number;
  };
  agents: {
    total: number;
    idle: number;
    busy: number;
    offline: number;
    error: number;
  };
  dags: {
    total: number;
    running: number;
    completed: number;
    failed: number;
  };
  recentEvents: Array<{
    type: string;
    message: string;
    timestamp: Date;
  }>;
}

/**
 * Create a real-time dashboard data feed.
 */
async function createDashboardFeed(): Promise<void> {
  console.log('\n--- Real-Time Dashboard Feed ---\n');

  const ws = await client.connectWebSocket();

  // Initialize stats
  const stats: DashboardStats = {
    tasks: { total: 0, pending: 0, running: 0, completed: 0, failed: 0 },
    agents: { total: 0, idle: 0, busy: 0, offline: 0, error: 0 },
    dags: { total: 0, running: 0, completed: 0, failed: 0 },
    recentEvents: [],
  };

  // Helper to add event
  function addEvent(type: string, message: string): void {
    stats.recentEvents.unshift({
      type,
      message,
      timestamp: new Date(),
    });

    // Keep only last 10 events
    if (stats.recentEvents.length > 10) {
      stats.recentEvents.pop();
    }
  }

  // Task events
  ws.on(WebSocketEventType.TASK_CREATED, (payload: TaskCreatedPayload) => {
    stats.tasks.total++;
    stats.tasks.pending++;
    addEvent('task', `Task created: ${payload.task.name}`);
    printDashboard(stats);
  });

  ws.on(WebSocketEventType.TASK_UPDATED, (payload: TaskUpdatedPayload) => {
    // Update status counts based on changes
    if (payload.changes.status) {
      const oldStatus = payload.task.status;
      const newStatus = payload.changes.status;

      // Decrement old status (simplified logic)
      if (oldStatus === 'running') stats.tasks.running--;
      if (oldStatus === 'pending') stats.tasks.pending--;

      // Increment new status
      if (newStatus === 'running') stats.tasks.running++;
      if (newStatus === 'pending') stats.tasks.pending++;
    }
    addEvent('task', `Task updated: ${payload.task.name}`);
    printDashboard(stats);
  });

  ws.on(WebSocketEventType.TASK_COMPLETED, (payload: TaskCompletedPayload) => {
    stats.tasks.running--;
    stats.tasks.completed++;
    addEvent('task', `Task completed: ${payload.task.name}`);
    printDashboard(stats);
  });

  ws.on(WebSocketEventType.TASK_FAILED, (payload: TaskFailedPayload) => {
    stats.tasks.running--;
    stats.tasks.failed++;
    addEvent('task', `Task failed: ${payload.task.name}`);
    printDashboard(stats);
  });

  // Agent events
  ws.on(WebSocketEventType.AGENT_STATUS_CHANGED, (payload: AgentStatusChangedPayload) => {
    // Decrement old status
    if (payload.previousStatus === 'idle') stats.agents.idle--;
    if (payload.previousStatus === 'busy') stats.agents.busy--;
    if (payload.previousStatus === 'offline') stats.agents.offline--;
    if (payload.previousStatus === 'error') stats.agents.error--;

    // Increment new status
    if (payload.agent.status === 'idle') stats.agents.idle++;
    if (payload.agent.status === 'busy') stats.agents.busy++;
    if (payload.agent.status === 'offline') stats.agents.offline++;
    if (payload.agent.status === 'error') stats.agents.error++;

    addEvent('agent', `Agent ${payload.agent.name}: ${payload.previousStatus} -> ${payload.agent.status}`);
    printDashboard(stats);
  });

  // DAG events
  ws.on(WebSocketEventType.DAG_STARTED, (payload: DAGStartedPayload) => {
    stats.dags.total++;
    stats.dags.running++;
    addEvent('dag', `DAG started: ${payload.dag.name}`);
    printDashboard(stats);
  });

  ws.on(WebSocketEventType.DAG_COMPLETED, (payload: DAGCompletedPayload) => {
    stats.dags.running--;
    stats.dags.completed++;
    addEvent('dag', `DAG completed: ${payload.dag.name}`);
    printDashboard(stats);
  });

  ws.on(WebSocketEventType.DAG_FAILED, (payload: DAGFailedPayload) => {
    stats.dags.running--;
    stats.dags.failed++;
    addEvent('dag', `DAG failed: ${payload.dag.name}`);
    printDashboard(stats);
  });

  // Approval events
  ws.on(WebSocketEventType.APPROVAL_REQUIRED, (payload: ApprovalRequiredPayload) => {
    addEvent('approval', `Approval required: ${payload.approval.reason}`);
    printDashboard(stats);
  });

  // Initial dashboard
  printDashboard(stats);
  console.log('\nListening for real-time updates...');
}

/**
 * Print dashboard statistics.
 */
function printDashboard(stats: DashboardStats): void {
  console.clear();
  console.log('='.repeat(60));
  console.log('              APEX REAL-TIME DASHBOARD');
  console.log('='.repeat(60));
  console.log();

  // Tasks section
  console.log('TASKS');
  console.log('-'.repeat(40));
  console.log(`  Total:     ${stats.tasks.total}`);
  console.log(`  Pending:   ${stats.tasks.pending}`);
  console.log(`  Running:   ${stats.tasks.running}`);
  console.log(`  Completed: ${stats.tasks.completed}`);
  console.log(`  Failed:    ${stats.tasks.failed}`);
  console.log();

  // Agents section
  console.log('AGENTS');
  console.log('-'.repeat(40));
  console.log(`  Total:   ${stats.agents.total}`);
  console.log(`  Idle:    ${stats.agents.idle}`);
  console.log(`  Busy:    ${stats.agents.busy}`);
  console.log(`  Offline: ${stats.agents.offline}`);
  console.log(`  Error:   ${stats.agents.error}`);
  console.log();

  // DAGs section
  console.log('DAGS');
  console.log('-'.repeat(40));
  console.log(`  Total:     ${stats.dags.total}`);
  console.log(`  Running:   ${stats.dags.running}`);
  console.log(`  Completed: ${stats.dags.completed}`);
  console.log(`  Failed:    ${stats.dags.failed}`);
  console.log();

  // Recent events
  console.log('RECENT EVENTS');
  console.log('-'.repeat(40));
  if (stats.recentEvents.length === 0) {
    console.log('  No events yet...');
  } else {
    for (const event of stats.recentEvents.slice(0, 5)) {
      const time = event.timestamp.toLocaleTimeString();
      console.log(`  [${time}] ${event.message}`);
    }
  }

  console.log();
  console.log('='.repeat(60));
  console.log(`Last updated: ${new Date().toLocaleString()}`);
}

// =============================================================================
// Subscription Management
// =============================================================================

/**
 * Demonstrate subscription management.
 */
async function subscriptionManagement(): Promise<void> {
  console.log('\n--- Subscription Management ---\n');

  const ws = await client.connectWebSocket();

  // Subscribe to specific events
  const taskSubscription = ws.subscribe({
    event: [
      WebSocketEventType.TASK_CREATED,
      WebSocketEventType.TASK_COMPLETED,
    ],
    filter: {
      // Filter by agent ID (optional)
      // agentId: 'specific-agent-id',
    },
  });
  console.log('Task subscription ID:', taskSubscription);

  // Subscribe to specific task
  const specificTaskSub = ws.subscribeToTask('task-123');
  console.log('Specific task subscription ID:', specificTaskSub);

  // Subscribe to specific agent
  const agentSub = ws.subscribeToAgent('agent-456');
  console.log('Agent subscription ID:', agentSub);

  // Subscribe to specific DAG
  const dagSub = ws.subscribeToDAG('dag-789');
  console.log('DAG subscription ID:', dagSub);

  // Subscribe to all approvals
  const approvalSub = ws.subscribeToApprovals();
  console.log('Approval subscription ID:', approvalSub);

  // Later: unsubscribe from specific subscription
  ws.unsubscribe(taskSubscription);
  console.log('Unsubscribed from task events');
}

// =============================================================================
// Wait For Specific Event
// =============================================================================

/**
 * Wait for a specific event to occur.
 */
async function waitForSpecificEvent(): Promise<void> {
  console.log('\n--- Wait For Specific Event ---\n');

  const ws = await client.connectWebSocket();

  // Wait for a task to complete with timeout
  try {
    console.log('Waiting for any task completion...');

    const completedTask = await ws.waitFor<TaskCompletedPayload>(
      WebSocketEventType.TASK_COMPLETED,
      (payload) => payload.task.status === 'completed',
      30000 // 30 second timeout
    );

    console.log('Task completed:', completedTask.task.name);
    console.log('Duration:', completedTask.duration, 'ms');
  } catch (error) {
    console.log('Timeout waiting for task completion');
  }
}

// =============================================================================
// Custom Message Handler
// =============================================================================

/**
 * Handle all messages with custom logic.
 */
async function customMessageHandler(): Promise<void> {
  console.log('\n--- Custom Message Handler ---\n');

  const ws = await client.connectWebSocket();

  // Listen to all raw messages
  ws.on('message', (message) => {
    console.log(`[${message.type}] at ${message.timestamp}`);

    // Custom routing logic
    switch (message.type) {
      case WebSocketEventType.TASK_CREATED:
      case WebSocketEventType.TASK_UPDATED:
      case WebSocketEventType.TASK_COMPLETED:
      case WebSocketEventType.TASK_FAILED:
        console.log('  -> Task event, routing to task handler');
        break;

      case WebSocketEventType.AGENT_STATUS_CHANGED:
        console.log('  -> Agent event, routing to agent handler');
        break;

      case WebSocketEventType.DAG_STARTED:
      case WebSocketEventType.DAG_COMPLETED:
      case WebSocketEventType.DAG_FAILED:
        console.log('  -> DAG event, routing to DAG handler');
        break;

      case WebSocketEventType.APPROVAL_REQUIRED:
      case WebSocketEventType.APPROVAL_RESOLVED:
        console.log('  -> Approval event, routing to approval handler');
        break;

      case WebSocketEventType.HEARTBEAT:
        // Typically silent
        break;

      default:
        console.log('  -> Unknown event type');
    }
  });
}

// =============================================================================
// Main Entry Point
// =============================================================================

async function main(): Promise<void> {
  console.log('='.repeat(60));
  console.log('Apex TypeScript SDK - Real-Time Monitoring Examples');
  console.log('='.repeat(60));

  try {
    // Basic connection example
    await basicWebSocketExample();

    // Subscription management
    await subscriptionManagement();

    // Choose one of the monitoring modes to run:

    // Option 1: Monitor all task events
    // await monitorTaskEvents();

    // Option 2: Monitor agent status
    // await monitorAgentStatus();

    // Option 3: Monitor DAG execution
    // await monitorDAGExecution();

    // Option 4: Monitor approvals
    // await monitorApprovals();

    // Option 5: Real-time dashboard feed
    // await createDashboardFeed();

    // Option 6: Custom message handler
    // await customMessageHandler();

    console.log('\n' + '='.repeat(60));
    console.log('WebSocket examples initialized!');
    console.log('Uncomment a monitoring function in main() to start real-time monitoring.');
    console.log('='.repeat(60));

  } catch (error) {
    console.error('\nExample failed with error:', error);
    process.exit(1);
  } finally {
    // Keep the connection open for real-time monitoring
    // Comment out to keep running:
    client.disconnectWebSocket();
  }
}

// Run the examples
main().catch(console.error);
