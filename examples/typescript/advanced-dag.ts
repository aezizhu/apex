/**
 * Apex TypeScript SDK - Advanced DAG with Conditional Branches
 *
 * This example demonstrates building complex DAG workflows that include:
 * - Multi-level conditional branching based on task output
 * - Error-handling nodes with fallback paths
 * - Parallel fan-out / fan-in patterns
 * - Approval gates embedded within a DAG
 * - Dynamic node configuration using metadata
 *
 * The DAG models a content moderation pipeline:
 *
 *   [Ingest Content]
 *        |
 *   [AI Moderation] ----> [Classify Severity]
 *                              |
 *                         (condition)
 *                        /     |      \
 *                   [Auto     [Queue    [Escalate to
 *                   Approve]   Review]   Legal Team]
 *                        \     |      /
 *                         [Publish Decision]
 *                              |
 *                         [Notify Author]
 *
 * Prerequisites:
 *   npm install @apex-swarm/sdk
 *
 * Run with:
 *   npx ts-node advanced-dag.ts
 */

import {
  ApexClient,
  CreateDAGRequest,
  DAGNode,
  DAGEdge,
  TaskPriority,
  DAGStatus,
  ApexError,
} from '@apex-swarm/sdk';

// =============================================================================
// Configuration
// =============================================================================

const API_URL: string = process.env.APEX_API_URL || 'http://localhost:8080';
const API_KEY: string = process.env.APEX_API_KEY || '';

const client = new ApexClient({
  baseUrl: API_URL,
  apiKey: API_KEY,
  timeout: 60000,
});

// =============================================================================
// Helper Types
// =============================================================================

/** Configuration for a single node in the DAG builder. */
interface NodeConfig {
  id: string;
  name: string;
  description: string;
  type: 'task' | 'condition' | 'approval' | 'parallel';
  input?: Record<string, unknown>;
  priority?: TaskPriority;
  timeout?: number;
  retries?: number;
  condition?: {
    expression: string;
    trueBranch: string;
    falseBranch: string;
  };
  approvalConfig?: {
    approvers: string[];
    requiredApprovals: number;
    timeoutSeconds: number;
  };
}

// =============================================================================
// DAG Builder
// =============================================================================

/**
 * Build the content moderation pipeline DAG.
 *
 * The pipeline processes user-submitted content through several stages:
 * 1. **Ingest** - Receive and normalize the content.
 * 2. **AI Moderation** - Run automated content analysis.
 * 3. **Classify Severity** - Determine severity (low / medium / high).
 * 4. **Conditional routing**:
 *    - Low severity: auto-approve immediately.
 *    - Medium severity: queue for human review.
 *    - High severity: escalate to the legal team (approval gate).
 * 5. **Publish Decision** - Record and publish the moderation outcome.
 * 6. **Notify Author** - Send the decision to the content author.
 */
function buildModerationPipeline(): CreateDAGRequest {
  // -- Node definitions -----------------------------------------------------

  const nodes: Omit<DAGNode, 'status' | 'taskId'>[] = [
    // Stage 1: Content ingestion
    {
      id: 'ingest',
      name: 'Ingest Content',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Ingest Submitted Content',
          description: 'Receive content, extract text/media, normalize format',
          priority: TaskPriority.NORMAL,
          input: {
            contentTypes: ['text', 'image', 'video'],
            extractMetadata: true,
            normalizeEncoding: 'utf-8',
          },
        },
        timeout: 60,
        retries: 2,
      },
    },

    // Stage 2: AI-powered moderation
    {
      id: 'ai-moderate',
      name: 'AI Moderation',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Run AI Moderation',
          description: 'Analyze content with multiple AI models for policy violations',
          priority: TaskPriority.HIGH,
          input: {
            models: ['text-classifier-v3', 'image-safety-v2', 'toxicity-scorer'],
            policies: ['hate_speech', 'violence', 'spam', 'misinformation'],
            confidenceThreshold: 0.7,
          },
        },
        timeout: 120,
        retries: 1,
      },
    },

    // Stage 3: Severity classification (condition node)
    //
    // This node evaluates the AI moderation output and routes to one of
    // three branches based on the computed severity score.
    {
      id: 'classify-severity',
      name: 'Classify Severity',
      type: 'condition',
      config: {
        condition: {
          // Expression evaluated against the ai-moderate node's output.
          // The orchestrator routes to trueBranch or falseBranch based on
          // nested conditions encoded in the edge conditions.
          expression: 'output.severityScore',
          trueBranch: 'auto-approve',    // severityScore < 30
          falseBranch: 'queue-review',   // severityScore >= 30
        },
      },
    },

    // Stage 4a: Auto-approve (low severity)
    {
      id: 'auto-approve',
      name: 'Auto-Approve Content',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Auto-Approve Low-Risk Content',
          description: 'Automatically approve content with low severity scores',
          priority: TaskPriority.LOW,
          input: {
            decision: 'approved',
            reason: 'Below severity threshold',
            addWatermark: false,
          },
        },
        timeout: 30,
      },
    },

    // Stage 4b: Queue for human review (medium severity)
    {
      id: 'queue-review',
      name: 'Queue for Human Review',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Queue Content for Review',
          description: 'Add content to the human review queue with context',
          priority: TaskPriority.NORMAL,
          input: {
            reviewQueue: 'content-moderation',
            priority: 'medium',
            includeAiAnalysis: true,
            slaHours: 24,
          },
        },
        timeout: 60,
      },
    },

    // Stage 4c: Escalate to legal (high severity) -- uses approval gate
    {
      id: 'escalate-legal',
      name: 'Escalate to Legal Team',
      type: 'approval',
      config: {
        approvalConfig: {
          approvers: ['legal-team@example.com', 'compliance@example.com'],
          requiredApprovals: 1,
          timeoutSeconds: 86400, // 24-hour SLA for legal review
          autoApprove: false,
        },
        taskTemplate: {
          name: 'Legal Team Escalation',
          description: 'High-severity content requires legal team sign-off',
          priority: TaskPriority.CRITICAL,
          input: {
            severity: 'high',
            requiresLegalReview: true,
            attachPolicyReport: true,
          },
        },
      },
    },

    // Stage 5: Publish the moderation decision
    // This node converges all three conditional branches.
    {
      id: 'publish-decision',
      name: 'Publish Decision',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Record and Publish Moderation Decision',
          description: 'Store the decision in the audit log and update content status',
          priority: TaskPriority.HIGH,
          input: {
            publishToAuditLog: true,
            updateContentStatus: true,
            retentionDays: 365,
          },
        },
        timeout: 30,
      },
    },

    // Stage 6: Notify the content author
    {
      id: 'notify-author',
      name: 'Notify Author',
      type: 'task',
      config: {
        taskTemplate: {
          name: 'Send Author Notification',
          description: 'Notify the content author of the moderation decision',
          priority: TaskPriority.NORMAL,
          input: {
            channels: ['email', 'in-app'],
            includeAppealLink: true,
            templateId: 'moderation-result-v2',
          },
        },
        timeout: 30,
      },
    },
  ];

  // -- Edge definitions (explicit graph wiring) -----------------------------

  const edges: Omit<DAGEdge, 'id'>[] = [
    // Sequential: ingest -> AI moderate -> classify
    { sourceNodeId: 'ingest', targetNodeId: 'ai-moderate' },
    { sourceNodeId: 'ai-moderate', targetNodeId: 'classify-severity' },

    // Conditional branches from severity classification
    {
      sourceNodeId: 'classify-severity',
      targetNodeId: 'auto-approve',
      condition: 'output.severityScore < 30',
    },
    {
      sourceNodeId: 'classify-severity',
      targetNodeId: 'queue-review',
      condition: 'output.severityScore >= 30 && output.severityScore < 70',
    },
    {
      sourceNodeId: 'classify-severity',
      targetNodeId: 'escalate-legal',
      condition: 'output.severityScore >= 70',
    },

    // All three branches converge to publish-decision
    { sourceNodeId: 'auto-approve', targetNodeId: 'publish-decision' },
    { sourceNodeId: 'queue-review', targetNodeId: 'publish-decision' },
    { sourceNodeId: 'escalate-legal', targetNodeId: 'publish-decision' },

    // Final notification
    { sourceNodeId: 'publish-decision', targetNodeId: 'notify-author' },
  ];

  return {
    name: 'Content Moderation Pipeline',
    description:
      'Multi-stage content moderation with AI analysis, severity-based ' +
      'conditional routing (auto-approve / human review / legal escalation), ' +
      'and author notification.',
    nodes,
    edges,
    metadata: {
      version: '3.0',
      team: 'trust-and-safety',
      slaTarget: '< 24 hours for all content',
    },
  };
}

// =============================================================================
// Utility: Print DAG Structure
// =============================================================================

/**
 * Print a human-readable summary of the DAG topology.
 */
function printDAGStructure(dag: CreateDAGRequest): void {
  console.log(`\nDAG: ${dag.name}`);
  console.log(`Description: ${dag.description}`);
  console.log(`\nNodes (${dag.nodes.length}):`);

  for (const node of dag.nodes) {
    const type = `[${node.type}]`;
    console.log(`  ${node.id} ${type} - ${node.name}`);

    // Show condition details if present
    if (node.type === 'condition' && node.config.condition) {
      const cond = node.config.condition;
      console.log(`    Expression: ${cond.expression}`);
      console.log(`    True  -> ${cond.trueBranch}`);
      console.log(`    False -> ${cond.falseBranch}`);
    }

    // Show approval config if present
    if (node.type === 'approval' && node.config.approvalConfig) {
      const ac = node.config.approvalConfig;
      console.log(`    Approvers: ${ac.approvers?.join(', ')}`);
      console.log(`    Required: ${ac.requiredApprovals}`);
      console.log(`    Timeout: ${ac.timeoutSeconds}s`);
    }
  }

  console.log(`\nEdges (${dag.edges.length}):`);
  for (const edge of dag.edges) {
    const cond = edge.condition ? ` [if ${edge.condition}]` : '';
    console.log(`  ${edge.sourceNodeId} -> ${edge.targetNodeId}${cond}`);
  }
}

// =============================================================================
// Main
// =============================================================================

async function main(): Promise<void> {
  console.log('='.repeat(70));
  console.log('  Apex SDK - Advanced DAG with Conditional Branches (TypeScript)');
  console.log('='.repeat(70));

  // Build the DAG definition
  const dagDef = buildModerationPipeline();

  console.log('\n--- DAG Definition ---');
  printDAGStructure(dagDef);

  // Submit to the API
  try {
    console.log('\n--- Submitting DAG ---\n');
    const dag = await client.createDAG(dagDef);
    console.log(`DAG created: ${dag.id}`);
    console.log(`Status: ${dag.status}`);

    // Start execution
    console.log('\n--- Starting Execution ---\n');
    const execution = await client.startDAG(dag.id, {
      contentId: 'content-2024-001',
      submittedBy: 'user-42',
      contentUrl: 'https://cdn.example.com/uploads/post-12345',
    });
    console.log(`Execution started: ${execution.id}`);
    console.log(`Status: ${execution.status}`);

    // Cleanup
    console.log('\n--- Cleanup ---\n');
    await client.deleteDAG(dag.id);
    console.log('DAG deleted');
  } catch (error) {
    if (error instanceof ApexError) {
      console.error(`API error [${error.code}]: ${error.message}`);
    } else {
      console.error('Unexpected error:', error);
    }
  }

  console.log('\n' + '='.repeat(70));
  console.log('  Advanced DAG example completed');
  console.log('='.repeat(70));
}

main().catch(console.error);
