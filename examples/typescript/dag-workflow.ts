/**
 * Apex TypeScript SDK - DAG Workflow Example
 *
 * This example demonstrates how to create and manage DAG (Directed Acyclic Graph)
 * workflows for complex multi-step task orchestration:
 * - Creating DAG definitions with nodes and edges
 * - Building multi-step workflows with dependencies
 * - Parallel task execution
 * - Conditional branching
 * - Workflow monitoring and control
 *
 * Prerequisites:
 *   npm install @apex-swarm/sdk
 *
 * Run with:
 *   npx ts-node dag-workflow.ts
 */

import {
  ApexClient,
  TaskStatus,
  TaskPriority,
  DAGStatus,
  CreateDAGRequest,
  DAGNode,
  DAGExecution,
} from '@apex-swarm/sdk';

// =============================================================================
// Configuration
// =============================================================================

const API_URL = process.env.APEX_API_URL || 'http://localhost:8080';
const API_KEY = process.env.APEX_API_KEY || '';

// Initialize the client
const client = new ApexClient({
  baseUrl: API_URL,
  apiKey: API_KEY,
  timeout: 60000,
});

// =============================================================================
// Simple Sequential DAG
// =============================================================================

/**
 * Create a simple sequential workflow: Research -> Analyze -> Report
 *
 * This demonstrates a linear workflow where each step depends on the previous one.
 *
 * Flow:
 *   [Research] --> [Analyze] --> [Report]
 */
async function createSequentialDAG(): Promise<string> {
  console.log('\n--- Creating Sequential DAG ---\n');

  const dagDefinition: CreateDAGRequest = {
    name: 'Research Pipeline',
    description: 'A sequential workflow for research, analysis, and reporting',
    nodes: [
      {
        id: 'research',
        name: 'Research Phase',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Research AI Trends',
            description: 'Gather information about current AI trends',
            priority: TaskPriority.NORMAL,
            input: {
              topic: 'AI agent architectures 2024',
              sources: ['academic', 'industry', 'news'],
            },
          },
          timeout: 300,  // 5 minutes
          retries: 2,
        },
      },
      {
        id: 'analyze',
        name: 'Analysis Phase',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Analyze Research Results',
            description: 'Analyze the gathered research data',
            priority: TaskPriority.NORMAL,
            input: {
              analysisType: 'comprehensive',
              focusAreas: ['trends', 'challenges', 'opportunities'],
            },
          },
          timeout: 180,
          retries: 1,
        },
      },
      {
        id: 'report',
        name: 'Report Generation',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Generate Executive Summary',
            description: 'Create a summary report from the analysis',
            priority: TaskPriority.HIGH,
            input: {
              format: 'markdown',
              maxLength: 2000,
              includeCharts: true,
            },
          },
          timeout: 120,
          retries: 1,
        },
      },
    ],
    edges: [
      // Research must complete before Analyze starts
      { sourceNodeId: 'research', targetNodeId: 'analyze' },
      // Analyze must complete before Report starts
      { sourceNodeId: 'analyze', targetNodeId: 'report' },
    ],
    metadata: {
      project: 'quarterly-research',
      team: 'research-ops',
    },
  };

  const dag = await client.createDAG(dagDefinition);

  console.log('Sequential DAG created:');
  console.log('  ID:', dag.id);
  console.log('  Name:', dag.name);
  console.log('  Nodes:', dag.nodes.length);
  console.log('  Status:', dag.status);

  return dag.id;
}

// =============================================================================
// Parallel Execution DAG
// =============================================================================

/**
 * Create a DAG with parallel execution branches.
 *
 * This demonstrates how independent tasks can run simultaneously
 * and then converge for a final aggregation step.
 *
 * Flow:
 *                  +--> [Web Search] --+
 *   [Initialize] --+--> [Database Query] --+--> [Aggregate] --> [Format Output]
 *                  +--> [API Call] ----+
 */
async function createParallelDAG(): Promise<string> {
  console.log('\n--- Creating Parallel Execution DAG ---\n');

  const dagDefinition: CreateDAGRequest = {
    name: 'Multi-Source Data Pipeline',
    description: 'Gather data from multiple sources in parallel',
    nodes: [
      // Initial setup node
      {
        id: 'init',
        name: 'Initialize Pipeline',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Pipeline Initialization',
            description: 'Set up parameters and validate inputs',
            priority: TaskPriority.NORMAL,
            input: { query: 'AI market trends' },
          },
        },
      },
      // Parallel data collection nodes
      {
        id: 'web-search',
        name: 'Web Search',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Web Search',
            description: 'Search the web for relevant information',
            priority: TaskPriority.NORMAL,
            input: { searchEngines: ['google', 'bing'] },
          },
          timeout: 120,
        },
      },
      {
        id: 'db-query',
        name: 'Database Query',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Query Internal Database',
            description: 'Search internal knowledge base',
            priority: TaskPriority.NORMAL,
            input: { databases: ['knowledge_base', 'reports'] },
          },
          timeout: 60,
        },
      },
      {
        id: 'api-call',
        name: 'External API Call',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Fetch External API Data',
            description: 'Get data from external APIs',
            priority: TaskPriority.NORMAL,
            input: { apis: ['market_data', 'news_feed'] },
          },
          timeout: 90,
        },
      },
      // Aggregation node - waits for all parallel tasks
      {
        id: 'aggregate',
        name: 'Aggregate Results',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Aggregate Data Sources',
            description: 'Combine and deduplicate results from all sources',
            priority: TaskPriority.HIGH,
            input: { deduplicationStrategy: 'similarity' },
          },
        },
      },
      // Final output formatting
      {
        id: 'format',
        name: 'Format Output',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Generate Final Report',
            description: 'Format the aggregated data for output',
            priority: TaskPriority.NORMAL,
            input: { outputFormat: 'json' },
          },
        },
      },
    ],
    edges: [
      // Init leads to three parallel branches
      { sourceNodeId: 'init', targetNodeId: 'web-search' },
      { sourceNodeId: 'init', targetNodeId: 'db-query' },
      { sourceNodeId: 'init', targetNodeId: 'api-call' },
      // All parallel branches converge to aggregate
      { sourceNodeId: 'web-search', targetNodeId: 'aggregate' },
      { sourceNodeId: 'db-query', targetNodeId: 'aggregate' },
      { sourceNodeId: 'api-call', targetNodeId: 'aggregate' },
      // Aggregate leads to format
      { sourceNodeId: 'aggregate', targetNodeId: 'format' },
    ],
    metadata: {
      project: 'data-pipeline',
      parallelism: 3,
    },
  };

  const dag = await client.createDAG(dagDefinition);

  console.log('Parallel DAG created:');
  console.log('  ID:', dag.id);
  console.log('  Name:', dag.name);
  console.log('  Nodes:', dag.nodes.length);
  console.log('  Parallel branches: 3');

  return dag.id;
}

// =============================================================================
// Conditional Branching DAG
// =============================================================================

/**
 * Create a DAG with conditional branching based on task results.
 *
 * Flow:
 *   [Evaluate] --condition: score > 80--> [Fast Track]
 *              --condition: score <= 80--> [Standard Review] --> [Manager Review]
 *
 *   Both paths converge to [Finalize]
 */
async function createConditionalDAG(): Promise<string> {
  console.log('\n--- Creating Conditional Branching DAG ---\n');

  const dagDefinition: CreateDAGRequest = {
    name: 'Approval Workflow',
    description: 'Conditional approval workflow based on evaluation score',
    nodes: [
      {
        id: 'evaluate',
        name: 'Initial Evaluation',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Evaluate Submission',
            description: 'Score the submission based on criteria',
            priority: TaskPriority.HIGH,
            input: { criteria: ['quality', 'completeness', 'accuracy'] },
          },
        },
      },
      // Conditional node that routes based on evaluation result
      {
        id: 'condition-check',
        name: 'Score Check',
        type: 'condition',
        config: {
          condition: {
            expression: 'output.score > 80',
            trueBranch: 'fast-track',
            falseBranch: 'standard-review',
          },
        },
      },
      // Fast track path for high scores
      {
        id: 'fast-track',
        name: 'Fast Track Approval',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Auto-Approve High Score',
            description: 'Automatically approve high-scoring submissions',
            priority: TaskPriority.NORMAL,
            input: { approvalType: 'automatic' },
          },
        },
      },
      // Standard review path
      {
        id: 'standard-review',
        name: 'Standard Review',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Manual Review Required',
            description: 'Flag for manual review',
            priority: TaskPriority.NORMAL,
            input: { reviewLevel: 'standard' },
          },
        },
      },
      // Additional approval step for standard path
      {
        id: 'manager-review',
        name: 'Manager Review',
        type: 'approval',
        config: {
          approvalConfig: {
            approvers: ['manager@example.com'],
            requiredApprovals: 1,
            timeoutSeconds: 86400,  // 24 hours
          },
        },
      },
      // Final step - both paths converge here
      {
        id: 'finalize',
        name: 'Finalize Decision',
        type: 'task',
        config: {
          taskTemplate: {
            name: 'Complete Workflow',
            description: 'Finalize the approval decision and notify stakeholders',
            priority: TaskPriority.NORMAL,
            input: { notifyStakeholders: true },
          },
        },
      },
    ],
    edges: [
      { sourceNodeId: 'evaluate', targetNodeId: 'condition-check' },
      // Conditional edges with conditions
      {
        sourceNodeId: 'condition-check',
        targetNodeId: 'fast-track',
        condition: 'output.score > 80',
      },
      {
        sourceNodeId: 'condition-check',
        targetNodeId: 'standard-review',
        condition: 'output.score <= 80',
      },
      { sourceNodeId: 'standard-review', targetNodeId: 'manager-review' },
      // Both paths converge to finalize
      { sourceNodeId: 'fast-track', targetNodeId: 'finalize' },
      { sourceNodeId: 'manager-review', targetNodeId: 'finalize' },
    ],
    metadata: {
      workflowType: 'approval',
      conditionalBranching: true,
    },
  };

  const dag = await client.createDAG(dagDefinition);

  console.log('Conditional DAG created:');
  console.log('  ID:', dag.id);
  console.log('  Name:', dag.name);
  console.log('  Includes conditional branching and approval node');

  return dag.id;
}

// =============================================================================
// DAG Execution and Monitoring
// =============================================================================

/**
 * Start a DAG execution and monitor its progress.
 */
async function startAndMonitorDAG(dagId: string): Promise<DAGExecution> {
  console.log('\n--- Starting DAG Execution ---\n');

  // Start the DAG with input parameters
  const execution = await client.startDAG(dagId, {
    customInput: 'Test execution',
    timestamp: new Date().toISOString(),
  });

  console.log('DAG execution started:');
  console.log('  Execution ID:', execution.id);
  console.log('  DAG ID:', execution.dagId);
  console.log('  Status:', execution.status);
  console.log('  Started:', execution.startedAt);

  return execution;
}

/**
 * Wait for a DAG to complete using polling.
 */
async function waitForDAGCompletion(
  dagId: string,
  executionId: string
): Promise<DAGExecution> {
  console.log('\n--- Monitoring DAG Execution ---\n');

  // Use the built-in waitForDAG helper
  const completedExecution = await client.waitForDAG(dagId, executionId, {
    pollInterval: 3000,  // Check every 3 seconds
    timeout: 600000,     // Wait up to 10 minutes
  });

  console.log('DAG execution completed:');
  console.log('  Status:', completedExecution.status);
  console.log('  Completed:', completedExecution.completedAt);

  // Print node execution details
  console.log('\nNode execution details:');
  for (const nodeExec of completedExecution.nodeExecutions) {
    console.log(`  ${nodeExec.nodeId}:`);
    console.log(`    Status: ${nodeExec.status}`);
    console.log(`    Task ID: ${nodeExec.taskId || 'N/A'}`);
    if (nodeExec.error) {
      console.log(`    Error: ${nodeExec.error.message}`);
    }
  }

  return completedExecution;
}

/**
 * Monitor DAG execution progress with detailed status updates.
 */
async function monitorDAGProgress(dagId: string, executionId: string): Promise<void> {
  console.log('\n--- Detailed DAG Progress ---\n');

  let isRunning = true;
  const statusHistory: string[] = [];

  while (isRunning) {
    const execution = await client.getDAGExecution(dagId, executionId);

    // Check for status changes
    const currentStatus = `${execution.status} - Nodes: ${getNodeStatusSummary(execution)}`;
    if (statusHistory[statusHistory.length - 1] !== currentStatus) {
      statusHistory.push(currentStatus);
      console.log(`[${new Date().toISOString()}] ${currentStatus}`);

      // Print individual node progress
      for (const nodeExec of execution.nodeExecutions) {
        const indicator = getStatusIndicator(nodeExec.status);
        console.log(`  ${indicator} ${nodeExec.nodeId}: ${nodeExec.status}`);
      }
    }

    // Check if execution is complete
    if (
      execution.status === DAGStatus.COMPLETED ||
      execution.status === DAGStatus.FAILED
    ) {
      isRunning = false;
    } else {
      // Wait before next poll
      await new Promise((resolve) => setTimeout(resolve, 2000));
    }
  }
}

/**
 * Get a summary of node statuses.
 */
function getNodeStatusSummary(execution: DAGExecution): string {
  const counts: Record<string, number> = {};

  for (const nodeExec of execution.nodeExecutions) {
    counts[nodeExec.status] = (counts[nodeExec.status] || 0) + 1;
  }

  return Object.entries(counts)
    .map(([status, count]) => `${status}:${count}`)
    .join(', ');
}

/**
 * Get a visual indicator for task status.
 */
function getStatusIndicator(status: TaskStatus): string {
  switch (status) {
    case TaskStatus.COMPLETED:
      return '[OK]';
    case TaskStatus.RUNNING:
      return '[..]';
    case TaskStatus.FAILED:
      return '[XX]';
    case TaskStatus.PENDING:
    case TaskStatus.QUEUED:
      return '[--]';
    case TaskStatus.PAUSED:
      return '[||]';
    default:
      return '[??]';
  }
}

// =============================================================================
// DAG Management Operations
// =============================================================================

/**
 * List and filter DAGs.
 */
async function listDAGs(): Promise<void> {
  console.log('\n--- List DAGs ---\n');

  // List all DAGs
  const allDags = await client.listDAGs();
  console.log(`Total DAGs: ${allDags.pagination.total}`);

  // List only active DAGs
  const activeDags = await client.listDAGs({
    status: DAGStatus.ACTIVE,
  });
  console.log(`Active DAGs: ${activeDags.pagination.total}`);

  // List running DAGs
  const runningDags = await client.listDAGs({
    status: DAGStatus.RUNNING,
  });
  console.log(`Running DAGs: ${runningDags.pagination.total}`);

  // Display DAG list
  console.log('\nDAG List:');
  for (const dag of allDags.data) {
    console.log(`  - ${dag.name} (${dag.id})`);
    console.log(`    Status: ${dag.status}, Nodes: ${dag.nodes.length}`);
  }
}

/**
 * Update a DAG definition.
 */
async function updateDAG(dagId: string): Promise<void> {
  console.log('\n--- Update DAG ---\n');

  const updated = await client.updateDAG(dagId, {
    description: 'Updated description with additional details',
    metadata: {
      lastModified: new Date().toISOString(),
      modifiedBy: 'admin',
    },
  });

  console.log('DAG updated:');
  console.log('  ID:', updated.id);
  console.log('  Description:', updated.description);
}

/**
 * DAG control operations: pause, resume, stop.
 */
async function controlDAG(dagId: string): Promise<void> {
  console.log('\n--- DAG Control Operations ---\n');

  // Pause a running DAG
  // const paused = await client.pauseDAG(dagId);
  // console.log('DAG paused:', paused.status);

  // Resume a paused DAG
  // const resumed = await client.resumeDAG(dagId);
  // console.log('DAG resumed:', resumed.status);

  // Stop a running DAG
  // const stopped = await client.stopDAG(dagId);
  // console.log('DAG stopped:', stopped.status);

  console.log('Control operations available: pause, resume, stop');
}

/**
 * Get DAG execution history.
 */
async function getDAGHistory(dagId: string): Promise<void> {
  console.log('\n--- DAG Execution History ---\n');

  const executions = await client.getDAGExecutions(dagId, {
    limit: 10,
    offset: 0,
  });

  console.log(`Found ${executions.pagination.total} executions:`);

  for (const exec of executions.data) {
    const duration = exec.completedAt && exec.startedAt
      ? `${Math.round((new Date(exec.completedAt).getTime() - new Date(exec.startedAt).getTime()) / 1000)}s`
      : 'in progress';

    console.log(`  ${exec.id}:`);
    console.log(`    Status: ${exec.status}`);
    console.log(`    Duration: ${duration}`);
    console.log(`    Started: ${exec.startedAt}`);
  }
}

// =============================================================================
// Complete Workflow Example
// =============================================================================

/**
 * Run a complete workflow: create DAG, execute, and monitor.
 */
async function runCompleteWorkflow(): Promise<void> {
  console.log('\n--- Running Complete Workflow ---\n');

  // Create the DAG
  const dagId = await createSequentialDAG();

  // Start execution
  const execution = await startAndMonitorDAG(dagId);

  // Monitor progress (with detailed updates)
  // Uncomment for real execution:
  // await monitorDAGProgress(dagId, execution.id);

  // Wait for completion
  // Uncomment for real execution:
  // const completed = await waitForDAGCompletion(dagId, execution.id);

  // Get execution history
  await getDAGHistory(dagId);

  // Clean up: delete the test DAG
  await client.deleteDAG(dagId);
  console.log('\nTest DAG deleted');
}

// =============================================================================
// Main Entry Point
// =============================================================================

async function main(): Promise<void> {
  console.log('='.repeat(60));
  console.log('Apex TypeScript SDK - DAG Workflow Examples');
  console.log('='.repeat(60));

  try {
    // Health check first
    const health = await client.healthCheck();
    console.log('API Status:', health.status);

    // Create different types of DAGs
    const sequentialDagId = await createSequentialDAG();
    const parallelDagId = await createParallelDAG();
    const conditionalDagId = await createConditionalDAG();

    // List all DAGs
    await listDAGs();

    // Update a DAG
    await updateDAG(sequentialDagId);

    // Control operations (demonstration)
    await controlDAG(sequentialDagId);

    // Run complete workflow (commented out - requires running server)
    // await runCompleteWorkflow();

    // Clean up test DAGs
    console.log('\n--- Cleanup ---\n');
    await client.deleteDAG(sequentialDagId);
    await client.deleteDAG(parallelDagId);
    await client.deleteDAG(conditionalDagId);
    console.log('All test DAGs deleted');

    console.log('\n' + '='.repeat(60));
    console.log('All DAG examples completed successfully!');
    console.log('='.repeat(60));
  } catch (error) {
    console.error('\nExample failed with error:', error);
    process.exit(1);
  }
}

// Run the examples
main().catch(console.error);
