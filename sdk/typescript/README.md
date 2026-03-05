# Project Apex TypeScript SDK

A comprehensive TypeScript SDK for interacting with the Project Apex API. This SDK provides full type safety, automatic retries with exponential backoff, and WebSocket support for real-time updates.

## Installation

```bash
npm install @apex/typescript-sdk
```

Or with yarn:

```bash
yarn add @apex/typescript-sdk
```

## Quick Start

```typescript
import { createApexClient } from '@apex/typescript-sdk';

// Create a client instance
const client = createApexClient({
  baseUrl: 'https://api.apex.example.com',
  apiKey: 'your-api-key',
});

// Create and run a task
const task = await client.createTask({
  name: 'Process Data',
  priority: 'high',
  input: { dataSource: 'warehouse-1' },
});

console.log(`Task created: ${task.id}`);
```

## Configuration

### Client Options

```typescript
import { createApexClient, ApexClientConfig } from '@apex/typescript-sdk';

const config: ApexClientConfig = {
  // Required
  baseUrl: 'https://api.apex.example.com',

  // Optional
  apiKey: 'your-api-key',           // API key for authentication
  timeout: 30000,                    // Request timeout in ms (default: 30000)
  retries: 3,                        // Number of retry attempts (default: 3)
  retryDelay: 1000,                  // Initial retry delay in ms (default: 1000)
  maxRetryDelay: 30000,              // Maximum retry delay in ms (default: 30000)
  headers: {                         // Custom headers
    'X-Custom-Header': 'value',
  },
  websocketUrl: 'wss://ws.apex.example.com/ws', // WebSocket URL (auto-derived if not specified)
};

const client = createApexClient(config);
```

## Tasks API

### List Tasks

```typescript
import { TaskStatus, TaskPriority } from '@apex/typescript-sdk';

// List all tasks
const { data: tasks, pagination } = await client.listTasks();

// With filters
const { data: pendingTasks } = await client.listTasks({
  status: TaskStatus.PENDING,
  priority: TaskPriority.HIGH,
  page: 1,
  limit: 20,
  sortBy: 'createdAt',
  sortOrder: 'desc',
});
```

### Create a Task

```typescript
const task = await client.createTask({
  name: 'Data Processing Job',
  description: 'Process daily sales data',
  priority: 'high',
  input: {
    source: 's3://bucket/data.csv',
    format: 'csv',
  },
  metadata: {
    department: 'sales',
  },
  timeoutSeconds: 3600,
  maxRetries: 3,
});
```

### Get, Update, and Delete Tasks

```typescript
// Get a task
const task = await client.getTask('task-123');

// Update a task
const updatedTask = await client.updateTask('task-123', {
  priority: 'critical',
  metadata: { urgent: true },
});

// Delete a task
await client.deleteTask('task-123');
```

### Task Operations

```typescript
// Cancel a running task
await client.cancelTask('task-123');

// Retry a failed task
await client.retryTask('task-123');

// Pause a task
await client.pauseTask('task-123');

// Resume a paused task
await client.resumeTask('task-123');

// Get task logs
const logs = await client.getTaskLogs('task-123', {
  limit: 100,
  level: 'error',
});
```

### Wait for Task Completion

```typescript
// Using polling
const completedTask = await client.waitForTask('task-123', {
  pollInterval: 2000,  // Check every 2 seconds
  timeout: 300000,     // 5 minute timeout
});

// Using WebSocket (more efficient)
const completedTask = await client.waitForTask('task-123', {
  useWebSocket: true,
  timeout: 300000,
});

// Create and wait in one call
const result = await client.runTask({
  name: 'Quick Job',
  input: { data: 'test' },
}, {
  timeout: 60000,
});
```

## Agents API

### Manage Agents

```typescript
import { AgentStatus } from '@apex/typescript-sdk';

// List agents
const { data: agents } = await client.listAgents({
  status: AgentStatus.IDLE,
  capabilities: ['data-processing', 'ml-inference'],
});

// Create an agent
const agent = await client.createAgent({
  name: 'Worker-1',
  description: 'General purpose worker',
  capabilities: ['data-processing', 'file-conversion'],
});

// Get agent details
const agent = await client.getAgent('agent-123');

// Update an agent
const updatedAgent = await client.updateAgent('agent-123', {
  capabilities: ['data-processing', 'ml-inference', 'file-conversion'],
});

// Delete an agent
await client.deleteAgent('agent-123');
```

### Task Assignment

```typescript
// Get tasks assigned to an agent
const { data: tasks } = await client.getAgentTasks('agent-123');

// Assign a task to an agent
await client.assignTask('agent-123', 'task-456');

// Unassign a task
await client.unassignTask('agent-123', 'task-456');
```

## DAGs API

DAGs (Directed Acyclic Graphs) allow you to define complex workflows with dependencies.

### Create a DAG

```typescript
const dag = await client.createDAG({
  name: 'Data Pipeline',
  description: 'ETL pipeline for daily data',
  nodes: [
    {
      id: 'extract',
      name: 'Extract Data',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Extract',
          input: { source: 's3://bucket/raw' },
        },
      },
    },
    {
      id: 'transform',
      name: 'Transform Data',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Transform',
        },
      },
    },
    {
      id: 'approval',
      name: 'Quality Check',
      type: 'approval',
      config: {
        approvalConfig: {
          approvers: ['qa-team'],
          requiredApprovals: 1,
          timeoutSeconds: 3600,
        },
      },
    },
    {
      id: 'load',
      name: 'Load Data',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Load',
          input: { destination: 'warehouse' },
        },
      },
    },
  ],
  edges: [
    { sourceNodeId: 'extract', targetNodeId: 'transform' },
    { sourceNodeId: 'transform', targetNodeId: 'approval' },
    { sourceNodeId: 'approval', targetNodeId: 'load' },
  ],
});
```

### Execute DAGs

```typescript
// Start a DAG
const execution = await client.startDAG('dag-123', {
  date: '2024-01-15',
  region: 'us-east-1',
});

// Wait for completion
const result = await client.waitForDAG('dag-123', execution.id, {
  pollInterval: 5000,
  timeout: 600000,
});

// Or use the convenience method
const result = await client.runDAG('dag-123', { date: '2024-01-15' });
```

### DAG Control

```typescript
// Pause a running DAG
await client.pauseDAG('dag-123');

// Resume a paused DAG
await client.resumeDAG('dag-123');

// Stop a DAG
await client.stopDAG('dag-123');

// Get execution history
const { data: executions } = await client.getDAGExecutions('dag-123', {
  limit: 10,
});
```

## Approvals API

### Manage Approvals

```typescript
import { ApprovalStatus } from '@apex/typescript-sdk';

// List pending approvals
const { data: approvals } = await client.listApprovals({
  status: ApprovalStatus.PENDING,
});

// Get pending approvals for a specific user
const myApprovals = await client.getPendingApprovals('user-123');

// Create an approval request
const approval = await client.createApproval({
  taskId: 'task-123',
  approvers: ['manager-1', 'manager-2'],
  requiredApprovals: 1,
  reason: 'Deploying to production',
  expiresAt: new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString(),
});
```

### Respond to Approvals

```typescript
// Approve
await client.respondToApproval('approval-123', {
  decision: 'approved',
  comment: 'Looks good, approved for deployment',
});

// Reject
await client.respondToApproval('approval-123', {
  decision: 'rejected',
  comment: 'Missing security review',
});

// Cancel an approval request
await client.cancelApproval('approval-123');
```

## WebSocket Streaming

### Connect and Subscribe

```typescript
import { WebSocketEventType } from '@apex/typescript-sdk';

// Get WebSocket client
const ws = await client.connectWebSocket();

// Subscribe to all task events
ws.on(WebSocketEventType.TASK_CREATED, (payload) => {
  console.log('Task created:', payload.task.name);
});

ws.on(WebSocketEventType.TASK_COMPLETED, (payload) => {
  console.log('Task completed:', payload.task.id, 'Duration:', payload.duration);
});

ws.on(WebSocketEventType.TASK_FAILED, (payload) => {
  console.error('Task failed:', payload.task.id, payload.error.message);
});
```

### Targeted Subscriptions

```typescript
// Subscribe to a specific task
const subscriptionId = ws.subscribeToTask('task-123');

// Subscribe to agent status changes
ws.subscribeToAgent('agent-456');

// Subscribe to DAG events
ws.subscribeToDAG('dag-789');

// Subscribe to all approval events
ws.subscribeToApprovals();

// Unsubscribe
ws.unsubscribe(subscriptionId);
```

### Wait for Events

```typescript
// Wait for a specific event
const payload = await ws.waitFor(
  WebSocketEventType.TASK_COMPLETED,
  (p) => p.task.id === 'task-123',
  60000 // timeout in ms
);
```

### Connection Management

```typescript
// Handle connection events
ws.on('open', () => console.log('Connected'));
ws.on('close', (code, reason) => console.log('Disconnected:', code, reason));
ws.on('error', (error) => console.error('Error:', error.message));
ws.on('reconnecting', (attempt) => console.log('Reconnecting:', attempt));
ws.on('reconnected', () => console.log('Reconnected'));

// Check connection status
if (ws.isConnected()) {
  console.log('Connection ID:', ws.getConnectionId());
}

// Disconnect
client.disconnectWebSocket();
```

## Error Handling

The SDK provides typed error classes for different scenarios:

```typescript
import {
  ApexError,
  AuthenticationError,
  AuthorizationError,
  NotFoundError,
  ValidationError,
  RateLimitError,
  TimeoutError,
  MaxRetriesExceededError,
} from '@apex/typescript-sdk';

try {
  await client.getTask('non-existent-task');
} catch (error) {
  if (error instanceof NotFoundError) {
    console.log('Task not found:', error.resourceId);
  } else if (error instanceof AuthenticationError) {
    console.log('Invalid API key');
  } else if (error instanceof AuthorizationError) {
    console.log('Permission denied for:', error.resource);
  } else if (error instanceof ValidationError) {
    console.log('Validation errors:', error.validationErrors);
  } else if (error instanceof RateLimitError) {
    console.log('Rate limited. Retry after:', error.retryAfter, 'seconds');
  } else if (error instanceof TimeoutError) {
    console.log('Request timed out after:', error.timeoutMs, 'ms');
  } else if (error instanceof MaxRetriesExceededError) {
    console.log('Max retries exceeded. Attempts:', error.attempts);
  } else if (error instanceof ApexError) {
    console.log('API error:', error.code, error.message);
  }
}
```

## Request Cancellation

```typescript
const controller = new AbortController();

// Start a request
const promise = client.listTasks({}, { signal: controller.signal });

// Cancel it
controller.abort();

try {
  await promise;
} catch (error) {
  console.log('Request cancelled');
}
```

## TypeScript Support

The SDK is written in TypeScript and provides full type definitions:

```typescript
import type {
  Task,
  Agent,
  DAG,
  Approval,
  TaskStatus,
  CreateTaskRequest,
  PaginatedResponse,
} from '@apex/typescript-sdk';

// Full type inference
const task: Task = await client.getTask('task-123');
const status: TaskStatus = task.status;

// Type-safe request bodies
const request: CreateTaskRequest = {
  name: 'My Task',
  priority: 'high', // Type-checked
  input: { key: 'value' },
};
```

## Best Practices

### 1. Use Environment Variables for Configuration

```typescript
const client = createApexClient({
  baseUrl: process.env.APEX_API_URL!,
  apiKey: process.env.APEX_API_KEY!,
});
```

### 2. Implement Proper Error Handling

```typescript
async function processTask(taskId: string) {
  try {
    const task = await client.waitForTask(taskId, { timeout: 60000 });
    return task.output;
  } catch (error) {
    if (error instanceof TimeoutError) {
      // Handle timeout - maybe cancel the task
      await client.cancelTask(taskId);
    }
    throw error;
  }
}
```

### 3. Use WebSocket for Real-Time Updates

```typescript
// Prefer WebSocket over polling for better efficiency
const ws = await client.connectWebSocket();

ws.subscribeToTask(taskId);
ws.on(WebSocketEventType.TASK_COMPLETED, handleCompletion);
ws.on(WebSocketEventType.TASK_FAILED, handleFailure);
```

### 4. Clean Up Resources

```typescript
// Always disconnect WebSocket when done
process.on('SIGTERM', () => {
  client.disconnectWebSocket();
  process.exit(0);
});
```

## License

MIT
