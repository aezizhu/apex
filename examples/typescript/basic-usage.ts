/**
 * Apex TypeScript SDK - Basic Usage Example
 *
 * This example demonstrates fundamental operations with the Apex SDK:
 * - Client initialization
 * - Health checks
 * - Task creation and management
 * - Agent operations
 * - Error handling
 *
 * Prerequisites:
 *   npm install @apex-swarm/sdk
 *
 * Run with:
 *   npx ts-node basic-usage.ts
 */

import {
  ApexClient,
  createApexClient,
  TaskStatus,
  TaskPriority,
  AgentStatus,
  ApexError,
} from '@apex-swarm/sdk';

// =============================================================================
// Configuration
// =============================================================================

// Load configuration from environment variables
const API_URL = process.env.APEX_API_URL || 'http://localhost:8080';
const API_KEY = process.env.APEX_API_KEY || '';

// =============================================================================
// Client Initialization
// =============================================================================

/**
 * Create and configure an Apex client instance.
 *
 * The client handles authentication, retries, and error transformation.
 */
function initializeClient(): ApexClient {
  // Method 1: Using the class constructor
  const client = new ApexClient({
    baseUrl: API_URL,
    apiKey: API_KEY,
    timeout: 30000,      // 30 seconds timeout
    retries: 3,          // Retry failed requests up to 3 times
    retryDelay: 1000,    // Initial retry delay of 1 second
    maxRetryDelay: 30000, // Maximum retry delay of 30 seconds
  });

  return client;

  // Method 2: Using the factory function (alternative)
  // return createApexClient({ baseUrl: API_URL, apiKey: API_KEY });
}

// =============================================================================
// Health Check Example
// =============================================================================

/**
 * Check the API health status.
 *
 * This is useful for:
 * - Verifying connectivity before operations
 * - Monitoring service health
 * - Checking dependent service status (database, queue, etc.)
 */
async function checkHealth(client: ApexClient): Promise<void> {
  console.log('\n--- Health Check ---\n');

  try {
    const health = await client.healthCheck();

    console.log('API Status:', health.status);
    console.log('Version:', health.version);
    console.log('Uptime:', Math.floor(health.uptime / 60), 'minutes');
    console.log('Services:');
    console.log('  - Database:', health.services.database);
    console.log('  - Queue:', health.services.queue);
    console.log('  - WebSocket:', health.services.websocket);

    // Check if all services are healthy
    const allHealthy = Object.values(health.services).every((s) => s === 'up');
    if (!allHealthy) {
      console.warn('\nWarning: Some services are not fully operational');
    }
  } catch (error) {
    console.error('Health check failed:', error);
    throw error;
  }
}

// =============================================================================
// Task Examples
// =============================================================================

/**
 * Create a new task.
 *
 * Tasks are the basic unit of work in Apex. Each task represents
 * a single operation to be executed by an agent.
 */
async function createTaskExample(client: ApexClient): Promise<string> {
  console.log('\n--- Create Task ---\n');

  try {
    const task = await client.createTask({
      name: 'Research AI Trends',
      description: 'Research and summarize the latest trends in AI agent architectures',
      priority: TaskPriority.NORMAL,
      input: {
        topic: 'AI agent architectures',
        depth: 'comprehensive',
        format: 'markdown',
      },
      metadata: {
        project: 'research-initiative',
        requestedBy: 'user-123',
      },
      timeoutSeconds: 120,
      maxRetries: 2,
    });

    console.log('Task created successfully:');
    console.log('  ID:', task.id);
    console.log('  Name:', task.name);
    console.log('  Status:', task.status);
    console.log('  Priority:', task.priority);
    console.log('  Created:', task.createdAt);

    return task.id;
  } catch (error) {
    console.error('Failed to create task:', error);
    throw error;
  }
}

/**
 * List tasks with filtering and pagination.
 *
 * Use filters to find specific tasks based on status, priority, etc.
 */
async function listTasksExample(client: ApexClient): Promise<void> {
  console.log('\n--- List Tasks ---\n');

  try {
    // List all pending tasks
    const pendingTasks = await client.listTasks({
      status: TaskStatus.PENDING,
      limit: 10,
      page: 1,
      sortBy: 'createdAt',
      sortOrder: 'desc',
    });

    console.log(`Found ${pendingTasks.pagination.total} pending tasks:`);
    for (const task of pendingTasks.data) {
      console.log(`  - ${task.name} (${task.id})`);
    }

    // List high-priority tasks
    const highPriorityTasks = await client.listTasks({
      priority: [TaskPriority.HIGH, TaskPriority.CRITICAL],
    });

    console.log(`\nFound ${highPriorityTasks.pagination.total} high-priority tasks`);

    // List tasks with multiple status filters
    const activeTasks = await client.listTasks({
      status: [TaskStatus.PENDING, TaskStatus.RUNNING, TaskStatus.QUEUED],
    });

    console.log(`\nFound ${activeTasks.pagination.total} active tasks`);
  } catch (error) {
    console.error('Failed to list tasks:', error);
    throw error;
  }
}

/**
 * Get task details and wait for completion.
 *
 * This demonstrates polling and the waitForTask helper method.
 */
async function waitForTaskExample(client: ApexClient, taskId: string): Promise<void> {
  console.log('\n--- Wait for Task ---\n');

  try {
    console.log('Waiting for task to complete...');

    // Method 1: Manual polling
    // let task = await client.getTask(taskId);
    // while (task.status !== 'completed' && task.status !== 'failed') {
    //   await new Promise(resolve => setTimeout(resolve, 2000));
    //   task = await client.getTask(taskId);
    //   console.log('  Status:', task.status);
    // }

    // Method 2: Using waitForTask helper (recommended)
    const completedTask = await client.waitForTask(taskId, {
      pollInterval: 2000,  // Check every 2 seconds
      timeout: 120000,     // Wait up to 2 minutes
      useWebSocket: false, // Use polling (set true for WebSocket updates)
    });

    console.log('Task completed:');
    console.log('  Status:', completedTask.status);
    console.log('  Started:', completedTask.startedAt);
    console.log('  Completed:', completedTask.completedAt);

    if (completedTask.output) {
      console.log('  Output:', JSON.stringify(completedTask.output, null, 2));
    }
  } catch (error) {
    console.error('Task failed or timed out:', error);
    throw error;
  }
}

/**
 * Update and manage task lifecycle.
 *
 * Tasks can be updated, paused, resumed, cancelled, or retried.
 */
async function manageTaskExample(client: ApexClient, taskId: string): Promise<void> {
  console.log('\n--- Manage Task ---\n');

  try {
    // Update task metadata
    const updated = await client.updateTask(taskId, {
      priority: TaskPriority.HIGH,
      metadata: {
        escalated: true,
        reason: 'urgent deadline',
      },
    });
    console.log('Task updated, new priority:', updated.priority);

    // Get task logs
    const logs = await client.getTaskLogs(taskId, { limit: 50 });
    console.log(`\nTask has ${logs.length} log entries`);
    for (const log of logs.slice(-5)) {
      console.log(`  [${log.level}] ${log.message}`);
    }

    // Pause a running task (if applicable)
    // const paused = await client.pauseTask(taskId);
    // console.log('Task paused');

    // Resume a paused task
    // const resumed = await client.resumeTask(taskId);
    // console.log('Task resumed');

    // Cancel a task
    // const cancelled = await client.cancelTask(taskId);
    // console.log('Task cancelled');

    // Retry a failed task
    // const retried = await client.retryTask(taskId);
    // console.log('Task retried');
  } catch (error) {
    console.error('Failed to manage task:', error);
    throw error;
  }
}

// =============================================================================
// Agent Examples
// =============================================================================

/**
 * Create and configure an agent.
 *
 * Agents are workers that execute tasks. They can have specific
 * capabilities and constraints.
 */
async function agentExamples(client: ApexClient): Promise<void> {
  console.log('\n--- Agent Operations ---\n');

  try {
    // Create a new agent
    const agent = await client.createAgent({
      name: 'research-agent-01',
      description: 'Specialized agent for research tasks',
      capabilities: ['web-search', 'summarization', 'analysis'],
      metadata: {
        model: 'gpt-4-turbo',
        region: 'us-west-2',
      },
    });

    console.log('Agent created:');
    console.log('  ID:', agent.id);
    console.log('  Name:', agent.name);
    console.log('  Status:', agent.status);
    console.log('  Capabilities:', agent.capabilities.join(', '));

    // List all agents
    const agents = await client.listAgents({
      status: AgentStatus.IDLE,
      capabilities: ['web-search'],
    });

    console.log(`\nFound ${agents.pagination.total} idle agents with web-search capability`);

    // Get agent stats
    const agentDetails = await client.getAgent(agent.id);
    console.log('\nAgent stats:');
    console.log('  Total tasks completed:', agentDetails.stats.totalTasksCompleted);
    console.log('  Total tasks failed:', agentDetails.stats.totalTasksFailed);
    console.log('  Average task duration:', agentDetails.stats.averageTaskDuration, 'ms');

    // Assign a task to an agent
    // const assignedTask = await client.assignTask(agent.id, taskId);
    // console.log('Task assigned to agent');

    // Update agent
    const updatedAgent = await client.updateAgent(agent.id, {
      capabilities: ['web-search', 'summarization', 'analysis', 'code-review'],
    });
    console.log('\nAgent capabilities updated:', updatedAgent.capabilities.join(', '));

    // Clean up: delete the test agent
    await client.deleteAgent(agent.id);
    console.log('\nTest agent deleted');
  } catch (error) {
    console.error('Agent operations failed:', error);
    throw error;
  }
}

// =============================================================================
// Error Handling Examples
// =============================================================================

/**
 * Demonstrate proper error handling patterns.
 *
 * The SDK throws typed errors that can be caught and handled appropriately.
 */
async function errorHandlingExample(client: ApexClient): Promise<void> {
  console.log('\n--- Error Handling ---\n');

  try {
    // Try to get a non-existent task
    await client.getTask('non-existent-task-id');
  } catch (error) {
    if (error instanceof ApexError) {
      console.log('Caught ApexError:');
      console.log('  Code:', error.code);
      console.log('  Message:', error.message);

      // Handle specific error types
      switch (error.code) {
        case 'NOT_FOUND':
          console.log('  -> Resource not found, handle gracefully');
          break;
        case 'UNAUTHORIZED':
          console.log('  -> Authentication required');
          break;
        case 'FORBIDDEN':
          console.log('  -> Insufficient permissions');
          break;
        case 'RATE_LIMITED':
          console.log('  -> Rate limit exceeded, retry later');
          break;
        case 'VALIDATION_ERROR':
          console.log('  -> Invalid request data');
          break;
        case 'SERVER_ERROR':
          console.log('  -> Server error, retry with backoff');
          break;
        default:
          console.log('  -> Unexpected error');
      }
    } else {
      // Re-throw non-Apex errors
      throw error;
    }
  }
}

// =============================================================================
// Run Task and Wait Example
// =============================================================================

/**
 * Convenience method to create and run a task in one call.
 *
 * This combines task creation and waiting for completion.
 */
async function runTaskExample(client: ApexClient): Promise<void> {
  console.log('\n--- Run Task (Create + Wait) ---\n');

  try {
    console.log('Running task and waiting for completion...');

    const completedTask = await client.runTask(
      {
        name: 'Quick Analysis',
        description: 'Analyze a simple dataset',
        priority: TaskPriority.HIGH,
        input: {
          data: [1, 2, 3, 4, 5],
          operation: 'sum',
        },
      },
      {
        pollInterval: 1000,
        timeout: 60000,
      }
    );

    console.log('Task completed:');
    console.log('  Result:', JSON.stringify(completedTask.output, null, 2));
  } catch (error) {
    console.error('Run task failed:', error);
    throw error;
  }
}

// =============================================================================
// Main Entry Point
// =============================================================================

async function main(): Promise<void> {
  console.log('='.repeat(60));
  console.log('Apex TypeScript SDK - Basic Usage Examples');
  console.log('='.repeat(60));

  // Initialize the client
  const client = initializeClient();

  try {
    // Run examples in sequence
    await checkHealth(client);
    const taskId = await createTaskExample(client);
    await listTasksExample(client);

    // Note: The following examples require a running Apex server
    // Uncomment to run full workflow

    // await manageTaskExample(client, taskId);
    // await waitForTaskExample(client, taskId);
    // await agentExamples(client);
    // await runTaskExample(client);

    await errorHandlingExample(client);

    console.log('\n' + '='.repeat(60));
    console.log('All examples completed successfully!');
    console.log('='.repeat(60));
  } catch (error) {
    console.error('\nExample failed with error:', error);
    process.exit(1);
  } finally {
    // Clean up: disconnect WebSocket if connected
    client.disconnectWebSocket();
  }
}

// Run the examples
main().catch(console.error);
