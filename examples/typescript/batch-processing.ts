/**
 * Apex TypeScript SDK - Batch Task Submission
 *
 * This example demonstrates patterns for submitting and managing large
 * batches of tasks efficiently:
 *
 * - Concurrent batch submission with configurable parallelism
 * - Progress tracking and completion percentage
 * - Error handling and partial-failure recovery
 * - Rate-limit-aware submission with adaptive back-off
 * - Collecting and aggregating batch results
 * - Cancellation of in-flight batches
 *
 * Prerequisites:
 *   npm install @apex-swarm/sdk
 *
 * Run with:
 *   npx ts-node batch-processing.ts
 */

import {
  ApexClient,
  ApexWebSocket,
  WebSocketEventType,
  TaskCompletedPayload,
  TaskFailedPayload,
  CreateTaskRequest,
  Task,
  TaskPriority,
  TaskStatus,
  ApexError,
  RateLimitError,
} from '@apex-swarm/sdk';

// =============================================================================
// Configuration
// =============================================================================

const API_URL: string = process.env.APEX_API_URL || 'http://localhost:8080';
const API_KEY: string = process.env.APEX_API_KEY || '';

/**
 * Maximum number of tasks submitted concurrently.
 * Keeps pressure on the API reasonable and avoids rate limits.
 */
const CONCURRENCY_LIMIT = 10;

/** Base delay between submissions when rate-limited (ms). */
const BASE_RATE_LIMIT_DELAY = 1000;

const client = new ApexClient({
  baseUrl: API_URL,
  apiKey: API_KEY,
  timeout: 30000,
  retries: 3,
  retryDelay: 1000,
});

// =============================================================================
// Types
// =============================================================================

/** Describes one item in a batch. */
interface BatchItem {
  /** Unique index within the batch (0-based). */
  index: number;
  /** The task creation request payload. */
  request: CreateTaskRequest;
}

/** Result of a single batch item after execution. */
interface BatchItemResult {
  index: number;
  taskId: string | null;
  status: 'completed' | 'failed' | 'skipped' | 'cancelled';
  output?: Record<string, unknown>;
  error?: string;
  durationMs?: number;
}

/** Summary of the entire batch run. */
interface BatchSummary {
  totalItems: number;
  submitted: number;
  completed: number;
  failed: number;
  skipped: number;
  cancelled: number;
  totalDurationMs: number;
  averageDurationMs: number;
  results: BatchItemResult[];
}

// =============================================================================
// Batch Submission Engine
// =============================================================================

/**
 * Submit a batch of tasks with controlled concurrency.
 *
 * Uses a semaphore pattern to limit the number of in-flight requests.
 * If a rate-limit error is encountered, the engine pauses and retries
 * with exponential back-off.
 *
 * @param items - Array of batch items to submit.
 * @param onProgress - Optional callback invoked after each submission.
 * @returns Array of created tasks (null for items that failed to submit).
 */
async function submitBatch(
  items: BatchItem[],
  onProgress?: (submitted: number, total: number) => void
): Promise<(Task | null)[]> {
  const results: (Task | null)[] = new Array(items.length).fill(null);
  let submittedCount = 0;
  let activeCount = 0;
  let rateLimitDelay = BASE_RATE_LIMIT_DELAY;

  // Process items using a concurrency-limited queue
  const queue = [...items];
  const workers: Promise<void>[] = [];

  for (let w = 0; w < CONCURRENCY_LIMIT; w++) {
    workers.push(
      (async () => {
        while (queue.length > 0) {
          const item = queue.shift();
          if (!item) break;

          activeCount++;

          try {
            const task = await client.createTask(item.request);
            results[item.index] = task;
            submittedCount++;

            // Reset rate-limit delay on success
            rateLimitDelay = BASE_RATE_LIMIT_DELAY;

            if (onProgress) {
              onProgress(submittedCount, items.length);
            }
          } catch (error) {
            if (error instanceof RateLimitError) {
              // Rate limited: pause this worker and re-queue the item
              const delay = error.retryAfter
                ? error.retryAfter * 1000
                : rateLimitDelay;

              console.warn(
                `  [Rate limit] Pausing ${delay}ms before retrying item ${item.index}`
              );

              await sleep(delay);

              // Exponential back-off for subsequent rate limits
              rateLimitDelay = Math.min(rateLimitDelay * 2, 30000);

              // Put the item back at the front of the queue
              queue.unshift(item);
            } else {
              // Non-retryable error: log and skip
              const msg =
                error instanceof ApexError
                  ? `${error.code}: ${error.message}`
                  : String(error);
              console.error(`  [Error] Item ${item.index}: ${msg}`);
              results[item.index] = null;
              submittedCount++;

              if (onProgress) {
                onProgress(submittedCount, items.length);
              }
            }
          } finally {
            activeCount--;
          }
        }
      })()
    );
  }

  await Promise.all(workers);
  return results;
}

/**
 * Wait for all submitted tasks to reach a terminal state.
 *
 * Uses WebSocket streaming for real-time updates, falling back to
 * REST polling if WebSocket is unavailable.
 *
 * @param tasks - Array of submitted tasks (null entries are skipped).
 * @param timeoutMs - Maximum time to wait (default: 10 minutes).
 * @param onProgress - Optional callback for progress updates.
 * @returns Array of batch item results.
 */
async function waitForBatchCompletion(
  tasks: (Task | null)[],
  timeoutMs: number = 600000,
  onProgress?: (completed: number, total: number) => void
): Promise<BatchItemResult[]> {
  const results: BatchItemResult[] = [];
  const pendingTasks = new Map<string, number>(); // taskId -> batch index
  let completedCount = 0;

  // Index non-null tasks
  for (let i = 0; i < tasks.length; i++) {
    const task = tasks[i];
    if (task) {
      pendingTasks.set(task.id, i);
    } else {
      // Task failed to submit -- mark as skipped
      results.push({
        index: i,
        taskId: null,
        status: 'skipped',
        error: 'Failed to submit',
      });
      completedCount++;
    }
  }

  const totalActive = pendingTasks.size;
  if (totalActive === 0) return results;

  console.log(`\n  Waiting for ${totalActive} tasks to complete...`);

  // Try WebSocket-based tracking first
  let useWebSocket = true;
  let ws: ApexWebSocket | null = null;

  try {
    ws = await client.connectWebSocket();
  } catch {
    console.log('  WebSocket unavailable, falling back to polling');
    useWebSocket = false;
  }

  if (useWebSocket && ws) {
    // Subscribe to completion/failure events for our tasks
    for (const taskId of pendingTasks.keys()) {
      ws.subscribeToTask(taskId);
    }

    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        resolve(); // timeout -- we'll handle stragglers via polling
      }, timeoutMs);

      const checkDone = (): void => {
        if (pendingTasks.size === 0) {
          clearTimeout(timer);
          resolve();
        }
      };

      ws!.on(
        WebSocketEventType.TASK_COMPLETED,
        (payload: TaskCompletedPayload) => {
          const idx = pendingTasks.get(payload.task.id);
          if (idx !== undefined) {
            results.push({
              index: idx,
              taskId: payload.task.id,
              status: 'completed',
              output: payload.task.output ?? undefined,
              durationMs: payload.duration,
            });
            pendingTasks.delete(payload.task.id);
            completedCount++;
            if (onProgress) onProgress(completedCount, tasks.length);
            checkDone();
          }
        }
      );

      ws!.on(
        WebSocketEventType.TASK_FAILED,
        (payload: TaskFailedPayload) => {
          const idx = pendingTasks.get(payload.task.id);
          if (idx !== undefined) {
            results.push({
              index: idx,
              taskId: payload.task.id,
              status: 'failed',
              error: payload.error.message,
            });
            pendingTasks.delete(payload.task.id);
            completedCount++;
            if (onProgress) onProgress(completedCount, tasks.length);
            checkDone();
          }
        }
      );
    });
  }

  // Polling fallback for any remaining tasks (or if WebSocket was unavailable)
  if (pendingTasks.size > 0) {
    console.log(`  Polling ${pendingTasks.size} remaining tasks...`);

    const deadline = Date.now() + timeoutMs;

    while (pendingTasks.size > 0 && Date.now() < deadline) {
      // Poll all pending tasks concurrently
      const pollPromises = Array.from(pendingTasks.entries()).map(
        async ([taskId, idx]) => {
          try {
            const task = await client.getTask(taskId);
            if (
              task.status === TaskStatus.COMPLETED ||
              task.status === TaskStatus.FAILED ||
              task.status === TaskStatus.CANCELLED
            ) {
              results.push({
                index: idx,
                taskId,
                status: task.status as 'completed' | 'failed' | 'cancelled',
                output: task.output ?? undefined,
                error: task.error?.message,
              });
              pendingTasks.delete(taskId);
              completedCount++;
              if (onProgress) onProgress(completedCount, tasks.length);
            }
          } catch {
            // Ignore polling errors; will retry next iteration
          }
        }
      );

      await Promise.all(pollPromises);

      if (pendingTasks.size > 0) {
        await sleep(3000); // poll every 3 seconds
      }
    }

    // Mark any remaining tasks as timed-out
    for (const [taskId, idx] of pendingTasks) {
      results.push({
        index: idx,
        taskId,
        status: 'failed',
        error: 'Timed out waiting for completion',
      });
    }
  }

  // Disconnect WebSocket
  client.disconnectWebSocket();

  // Sort results by index for consistent ordering
  results.sort((a, b) => a.index - b.index);
  return results;
}

// =============================================================================
// Batch Cancellation
// =============================================================================

/**
 * Cancel all in-flight tasks from a batch.
 *
 * Useful for aborting a batch when a critical error is detected or
 * when the user requests cancellation.
 *
 * @param tasks - Array of submitted tasks.
 * @returns Number of successfully cancelled tasks.
 */
async function cancelBatch(tasks: (Task | null)[]): Promise<number> {
  let cancelledCount = 0;

  const cancelPromises = tasks
    .filter((t): t is Task => t !== null)
    .map(async (task) => {
      try {
        await client.cancelTask(task.id);
        cancelledCount++;
      } catch {
        // Task may have already completed -- ignore errors
      }
    });

  await Promise.all(cancelPromises);
  return cancelledCount;
}

// =============================================================================
// Reporting
// =============================================================================

/**
 * Compute and print a summary report for the batch run.
 */
function printBatchSummary(
  results: BatchItemResult[],
  totalDurationMs: number
): BatchSummary {
  const completed = results.filter((r) => r.status === 'completed');
  const failed = results.filter((r) => r.status === 'failed');
  const skipped = results.filter((r) => r.status === 'skipped');
  const cancelled = results.filter((r) => r.status === 'cancelled');

  const durations = completed
    .map((r) => r.durationMs ?? 0)
    .filter((d) => d > 0);
  const avgDuration =
    durations.length > 0
      ? durations.reduce((a, b) => a + b, 0) / durations.length
      : 0;

  const summary: BatchSummary = {
    totalItems: results.length,
    submitted: results.length - skipped.length,
    completed: completed.length,
    failed: failed.length,
    skipped: skipped.length,
    cancelled: cancelled.length,
    totalDurationMs: totalDurationMs,
    averageDurationMs: Math.round(avgDuration),
    results,
  };

  console.log('\n' + '='.repeat(60));
  console.log('  BATCH PROCESSING REPORT');
  console.log('='.repeat(60));
  console.log(`\n  Total items:       ${summary.totalItems}`);
  console.log(`  Submitted:         ${summary.submitted}`);
  console.log(`  Completed:         ${summary.completed}`);
  console.log(`  Failed:            ${summary.failed}`);
  console.log(`  Skipped:           ${summary.skipped}`);
  console.log(`  Cancelled:         ${summary.cancelled}`);
  console.log(`\n  Total duration:    ${(totalDurationMs / 1000).toFixed(1)}s`);
  console.log(`  Avg task duration: ${summary.averageDurationMs}ms`);
  console.log(
    `  Success rate:      ${((summary.completed / Math.max(summary.submitted, 1)) * 100).toFixed(1)}%`
  );

  // Print failures if any
  if (failed.length > 0) {
    console.log('\n  FAILURES:');
    for (const f of failed.slice(0, 10)) {
      console.log(`    Item ${f.index}: ${f.error}`);
    }
    if (failed.length > 10) {
      console.log(`    ... and ${failed.length - 10} more`);
    }
  }

  console.log('\n' + '='.repeat(60));
  return summary;
}

// =============================================================================
// Example: Document Processing Batch
// =============================================================================

/**
 * Simulate a document processing batch.
 *
 * Creates 25 tasks that each process a document through summarization,
 * keyword extraction, and sentiment analysis.
 */
async function documentProcessingBatch(): Promise<void> {
  console.log('\n--- Document Processing Batch ---\n');

  // Generate batch items
  const documents = Array.from({ length: 25 }, (_, i) => ({
    id: `doc-${String(i + 1).padStart(3, '0')}`,
    title: `Quarterly Report Section ${i + 1}`,
    wordCount: 500 + Math.floor(Math.random() * 2000),
  }));

  const items: BatchItem[] = documents.map((doc, index) => ({
    index,
    request: {
      name: `Process: ${doc.title}`,
      description: `Summarize, extract keywords, and analyze sentiment for ${doc.id}`,
      priority: TaskPriority.NORMAL,
      input: {
        documentId: doc.id,
        operations: ['summarize', 'extract_keywords', 'sentiment_analysis'],
        maxSummaryLength: 200,
        topKeywords: 10,
      },
      metadata: {
        batch: 'quarterly-report-q4',
        documentId: doc.id,
        wordCount: doc.wordCount,
      },
      timeoutSeconds: 120,
      maxRetries: 2,
    },
  }));

  console.log(`Submitting ${items.length} document processing tasks...`);
  console.log(`Concurrency limit: ${CONCURRENCY_LIMIT}\n`);

  const batchStart = Date.now();

  // Step 1: Submit all tasks
  const tasks = await submitBatch(items, (submitted, total) => {
    const pct = ((submitted / total) * 100).toFixed(0);
    process.stdout.write(`\r  Submitted: ${submitted}/${total} (${pct}%)`);
  });
  console.log(); // newline after progress

  const successfulSubmissions = tasks.filter((t) => t !== null).length;
  console.log(`\n  ${successfulSubmissions} tasks submitted successfully`);

  // Step 2: Wait for completion
  const results = await waitForBatchCompletion(
    tasks,
    300000, // 5 minute timeout
    (completed, total) => {
      const pct = ((completed / total) * 100).toFixed(0);
      process.stdout.write(`\r  Progress: ${completed}/${total} (${pct}%)`);
    }
  );
  console.log(); // newline after progress

  const batchDuration = Date.now() - batchStart;

  // Step 3: Print report
  printBatchSummary(results, batchDuration);

  // Step 4: Cleanup demo tasks
  console.log('\nCleaning up demo tasks...');
  for (const task of tasks) {
    if (task) {
      try {
        await client.deleteTask(task.id);
      } catch {
        // Ignore cleanup errors
      }
    }
  }
  console.log('Cleanup complete');
}

// =============================================================================
// Example: Priority-Tiered Batch
// =============================================================================

/**
 * Demonstrate submitting tasks with different priority levels.
 *
 * Higher-priority tasks are submitted first to ensure they enter the
 * queue ahead of lower-priority items.
 */
async function priorityTieredBatch(): Promise<void> {
  console.log('\n--- Priority-Tiered Batch ---\n');

  // Create items with mixed priorities
  const priorities: Array<{ level: TaskPriority; label: string; count: number }> = [
    { level: TaskPriority.CRITICAL, label: 'Critical', count: 2 },
    { level: TaskPriority.HIGH, label: 'High', count: 5 },
    { level: TaskPriority.NORMAL, label: 'Normal', count: 10 },
    { level: TaskPriority.LOW, label: 'Low', count: 8 },
  ];

  const items: BatchItem[] = [];
  let index = 0;

  // Sort by priority (highest first) so critical items hit the queue first
  for (const tier of priorities) {
    for (let i = 0; i < tier.count; i++) {
      items.push({
        index: index++,
        request: {
          name: `${tier.label} Task ${i + 1}`,
          description: `${tier.label}-priority batch task`,
          priority: tier.level,
          input: {
            operation: 'analyze',
            data: { value: Math.random() * 100 },
          },
          metadata: {
            batch: 'priority-tiered-demo',
            priorityTier: tier.label,
          },
        },
      });
    }
  }

  console.log(`Total items: ${items.length}`);
  for (const tier of priorities) {
    console.log(`  ${tier.label}: ${tier.count}`);
  }

  const tasks = await submitBatch(items, (submitted, total) => {
    process.stdout.write(`\r  Submitted: ${submitted}/${total}`);
  });
  console.log();

  console.log(`\n  ${tasks.filter((t) => t !== null).length} tasks submitted`);

  // Cleanup
  for (const task of tasks) {
    if (task) {
      try {
        await client.deleteTask(task.id);
      } catch {
        // Ignore
      }
    }
  }
  console.log('  Cleaned up demo tasks');
}

// =============================================================================
// Utility
// =============================================================================

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// =============================================================================
// Main
// =============================================================================

async function main(): Promise<void> {
  console.log('='.repeat(60));
  console.log('  Apex SDK - Batch Task Processing (TypeScript)');
  console.log('='.repeat(60));

  try {
    // Verify API is reachable
    const health = await client.healthCheck();
    console.log(`\nAPI status: ${health.status}`);

    // Run batch examples
    await documentProcessingBatch();
    await priorityTieredBatch();

    console.log('\n' + '='.repeat(60));
    console.log('  Batch processing examples completed');
    console.log('='.repeat(60));
  } catch (error) {
    if (error instanceof ApexError) {
      console.error(`\nApex error [${error.code}]: ${error.message}`);
    } else {
      console.error('\nUnexpected error:', error);
    }
    process.exit(1);
  } finally {
    client.disconnectWebSocket();
  }
}

main().catch(console.error);
