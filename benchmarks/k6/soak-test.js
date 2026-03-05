/**
 * Project Apex - K6 Soak Test
 *
 * Extended duration testing to identify:
 * - Memory leaks
 * - Resource exhaustion
 * - Connection pool issues
 * - Database connection leaks
 * - Long-running stability issues
 *
 * Usage:
 *   k6 run benchmarks/k6/soak-test.js
 *   k6 run --duration 8h benchmarks/k6/soak-test.js  # Extended duration
 */

import http from 'k6/http';
import ws from 'k6/ws';
import { check, sleep, group } from 'k6';
import { Counter, Rate, Trend, Gauge } from 'k6/metrics';
import { randomString, randomIntBetween } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

// ============================================================================
// Configuration
// ============================================================================

const BASE_URL = __ENV.API_URL || 'http://localhost:8080';
const WS_URL = __ENV.WS_URL || 'ws://localhost:8080';
const AUTH_TOKEN = __ENV.AUTH_TOKEN || '';

// Soak test duration (default 4 hours)
const SOAK_DURATION = __ENV.SOAK_DURATION || '4h';
const TARGET_VUS = parseInt(__ENV.TARGET_VUS) || 100;

// ============================================================================
// Custom Metrics for Soak Testing
// ============================================================================

// Memory/Resource tracking metrics
const memoryIndicator = new Gauge('apex_memory_indicator');
const connectionPoolUsage = new Gauge('apex_connection_pool_usage');
const activeWebsockets = new Gauge('apex_active_websockets');

// Degradation tracking
const responseTimeDegradation = new Trend('apex_response_degradation', true);
const hourlyErrorRate = new Rate('apex_hourly_error_rate');
const cumulativeErrors = new Counter('apex_cumulative_errors');

// Operation counters
const totalOperations = new Counter('apex_total_operations');
const taskOperations = new Counter('apex_task_operations');
const dagOperations = new Counter('apex_dag_operations');
const wsOperations = new Counter('apex_ws_operations');

// Long-running trends
const responseTimeOverTime = new Trend('apex_response_over_time', true);
const throughputOverTime = new Counter('apex_throughput_over_time');

// ============================================================================
// Test Options - Soak Test Configuration
// ============================================================================

export const options = {
    // Soak test stages
    stages: [
        // Ramp up phase
        { duration: '5m', target: TARGET_VUS },    // Gradual ramp to target

        // Sustained load phase (main soak period)
        { duration: SOAK_DURATION, target: TARGET_VUS },  // Hold steady for soak duration

        // Ramp down phase
        { duration: '5m', target: 0 },              // Gradual ramp down
    ],

    // Soak test thresholds (focus on stability over time)
    thresholds: {
        // HTTP performance should remain stable
        'http_req_duration': ['p(50)<300', 'p(95)<800', 'p(99)<1500'],
        'http_req_failed': ['rate<0.02'],  // Max 2% error rate over entire soak

        // Response time degradation should be minimal
        'apex_response_degradation': ['p(99)<2000'],  // No major degradation

        // Error accumulation
        'apex_hourly_error_rate': ['rate<0.05'],  // Max 5% hourly error rate

        // Operation-specific thresholds
        'http_req_duration{operation:health}': ['p(95)<100'],
        'http_req_duration{operation:list_tasks}': ['p(95)<500'],
        'http_req_duration{operation:create_task}': ['p(95)<1000'],
    },

    // Tags for segmentation
    tags: {
        test_type: 'soak',
        environment: __ENV.ENVIRONMENT || 'local',
    },

    // Summary configuration
    summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(50)', 'p(90)', 'p(95)', 'p(99)'],

    // No abort on threshold failure - we want to see how bad it gets
    thresholdAbortOnFail: false,
};

// ============================================================================
// Helper Functions
// ============================================================================

function getHeaders() {
    const headers = {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'User-Agent': 'K6-SoakTest/1.0',
    };

    if (AUTH_TOKEN) {
        headers['Authorization'] = `Bearer ${AUTH_TOKEN}`;
    }

    return headers;
}

function generateTaskPayload() {
    return JSON.stringify({
        name: `Soak Test Task ${randomString(8)}`,
        instruction: `Long-running soak test task - ${randomString(16)}`,
        priority: randomIntBetween(1, 10),
        labels: ['soak-test', 'automated', 'long-running'],
        limits: {
            token_limit: randomIntBetween(1000, 5000),
            cost_limit: randomIntBetween(1, 20) / 100,
            time_limit: randomIntBetween(60, 300),
        },
        metadata: {
            source: 'k6-soak-test',
            timestamp: new Date().toISOString(),
            hour: new Date().getHours(),
            vu: __VU,
            iteration: __ITER,
        },
    });
}

function generateSimpleDAGPayload() {
    return JSON.stringify({
        name: `Soak DAG ${randomString(6)}`,
        description: 'Simple DAG for soak testing',
        nodes: [
            {
                id: 'start',
                type: 'task',
                config: { instruction: 'Start task' },
            },
            {
                id: 'process',
                type: 'task',
                dependencies: ['start'],
                config: { instruction: 'Process task' },
            },
            {
                id: 'end',
                type: 'task',
                dependencies: ['process'],
                config: { instruction: 'End task' },
            },
        ],
    });
}

// Track baseline response times for degradation detection
let baselineResponseTime = null;

function trackDegradation(duration) {
    if (baselineResponseTime === null && __ITER === 0) {
        baselineResponseTime = duration;
    }

    if (baselineResponseTime !== null) {
        const degradation = duration - baselineResponseTime;
        responseTimeDegradation.add(degradation);
    }
}

// ============================================================================
// Setup
// ============================================================================

export function setup() {
    console.log(`Starting soak test against ${BASE_URL}`);
    console.log(`Duration: ${SOAK_DURATION}, Target VUs: ${TARGET_VUS}`);

    // Verify API health
    const res = http.get(`${BASE_URL}/health`, {
        headers: getHeaders(),
        timeout: '30s',
    });

    if (res.status !== 200) {
        throw new Error(`API health check failed: ${res.status}`);
    }

    // Capture initial metrics
    const metricsRes = http.get(`${BASE_URL}/metrics`, { headers: getHeaders() });

    return {
        startTime: new Date().toISOString(),
        startTimestamp: Date.now(),
        baseUrl: BASE_URL,
        initialHealth: res.body,
        initialMetrics: metricsRes.body,
    };
}

// ============================================================================
// Main Test Function
// ============================================================================

export default function(data) {
    const headers = getHeaders();
    const elapsed = (Date.now() - data.startTimestamp) / 1000 / 60; // Minutes elapsed

    // ========================================================================
    // Scenario 1: Continuous health monitoring
    // ========================================================================
    group('Health Monitoring', () => {
        const start = Date.now();
        const res = http.get(`${BASE_URL}/health`, {
            headers,
            tags: { operation: 'health' },
        });

        const duration = Date.now() - start;
        responseTimeOverTime.add(duration);
        throughputOverTime.add(1);
        totalOperations.add(1);

        trackDegradation(duration);

        const passed = check(res, {
            'health check OK': (r) => r.status === 200,
            'health response stable': (r) => r.timings.duration < 100,
        });

        if (!passed) {
            cumulativeErrors.add(1);
            hourlyErrorRate.add(1);
        } else {
            hourlyErrorRate.add(0);
        }

        // Check for memory/resource indicators in health response
        try {
            const health = JSON.parse(res.body);
            if (health.memory_usage) {
                memoryIndicator.add(health.memory_usage);
            }
            if (health.connection_pool) {
                connectionPoolUsage.add(health.connection_pool.used / health.connection_pool.max);
            }
        } catch {
            // Health response doesn't include detailed metrics
        }
    });

    sleep(0.5);

    // ========================================================================
    // Scenario 2: Steady task operations
    // ========================================================================
    group('Task Operations', () => {
        // List tasks
        const listStart = Date.now();
        const listRes = http.get(`${BASE_URL}/api/v1/tasks?limit=50`, {
            headers,
            tags: { operation: 'list_tasks' },
        });

        responseTimeOverTime.add(Date.now() - listStart);
        throughputOverTime.add(1);
        totalOperations.add(1);
        taskOperations.add(1);

        trackDegradation(listRes.timings.duration);

        check(listRes, {
            'list tasks OK': (r) => r.status === 200,
        });

        sleep(0.3);

        // Create task (less frequent)
        if (__ITER % 5 === 0) {
            const createStart = Date.now();
            const createRes = http.post(
                `${BASE_URL}/api/v1/tasks`,
                generateTaskPayload(),
                {
                    headers,
                    tags: { operation: 'create_task' },
                }
            );

            const createDuration = Date.now() - createStart;
            responseTimeOverTime.add(createDuration);
            throughputOverTime.add(1);
            totalOperations.add(1);
            taskOperations.add(1);

            trackDegradation(createDuration);

            const passed = check(createRes, {
                'create task OK': (r) => r.status === 201 || r.status === 200,
            });

            if (!passed) {
                cumulativeErrors.add(1);
                hourlyErrorRate.add(1);
            } else {
                hourlyErrorRate.add(0);

                // Get the created task to verify
                try {
                    const task = JSON.parse(createRes.body);
                    const taskId = task.id || task.task_id;

                    if (taskId) {
                        const getRes = http.get(`${BASE_URL}/api/v1/tasks/${taskId}`, { headers });
                        totalOperations.add(1);
                        taskOperations.add(1);

                        check(getRes, {
                            'get created task OK': (r) => r.status === 200,
                        });
                    }
                } catch {
                    // Skip verification errors
                }
            }
        }
    });

    sleep(0.3);

    // ========================================================================
    // Scenario 3: Agent monitoring
    // ========================================================================
    group('Agent Monitoring', () => {
        const res = http.get(`${BASE_URL}/api/v1/agents`, {
            headers,
            tags: { operation: 'list_agents' },
        });

        responseTimeOverTime.add(res.timings.duration);
        throughputOverTime.add(1);
        totalOperations.add(1);

        trackDegradation(res.timings.duration);

        check(res, {
            'list agents OK': (r) => r.status === 200,
        });
    });

    sleep(0.3);

    // ========================================================================
    // Scenario 4: DAG operations (periodic)
    // ========================================================================
    if (__ITER % 20 === 0) { // Every 20th iteration
        group('DAG Operations', () => {
            // Create DAG
            const createRes = http.post(
                `${BASE_URL}/api/v1/dags`,
                generateSimpleDAGPayload(),
                { headers }
            );

            totalOperations.add(1);
            dagOperations.add(1);

            trackDegradation(createRes.timings.duration);

            if (createRes.status === 201 || createRes.status === 200) {
                try {
                    const dag = JSON.parse(createRes.body);
                    const dagId = dag.id || dag.dag_id;

                    if (dagId) {
                        // Execute DAG
                        const execRes = http.post(
                            `${BASE_URL}/api/v1/dags/${dagId}/execute`,
                            JSON.stringify({ async: true }),
                            { headers }
                        );

                        totalOperations.add(1);
                        dagOperations.add(1);

                        check(execRes, {
                            'dag execution accepted': (r) => r.status >= 200 && r.status < 300,
                        });
                    }
                } catch {
                    // Skip DAG errors
                }
            }
        });
    }

    sleep(0.3);

    // ========================================================================
    // Scenario 5: Metrics collection
    // ========================================================================
    if (__ITER % 10 === 0) { // Every 10th iteration
        group('Metrics Collection', () => {
            const res = http.get(`${BASE_URL}/metrics`, { headers });

            totalOperations.add(1);

            check(res, {
                'metrics endpoint OK': (r) => r.status === 200,
            });

            // Parse metrics for resource monitoring
            if (res.status === 200) {
                try {
                    // Look for specific metrics in response
                    const body = res.body;

                    // Extract connection pool metrics if available
                    const poolMatch = body.match(/db_pool_connections_used\s+(\d+)/);
                    if (poolMatch) {
                        connectionPoolUsage.add(parseInt(poolMatch[1]));
                    }

                    // Extract memory metrics if available
                    const memMatch = body.match(/process_resident_memory_bytes\s+(\d+)/);
                    if (memMatch) {
                        memoryIndicator.add(parseInt(memMatch[1]) / 1024 / 1024); // MB
                    }
                } catch {
                    // Metrics parsing failed
                }
            }
        });
    }

    sleep(0.3);

    // ========================================================================
    // Scenario 6: Database stress (search queries)
    // ========================================================================
    group('Database Queries', () => {
        const searchPayload = JSON.stringify({
            query: randomString(3),
            limit: 25,
            offset: randomIntBetween(0, 100),
        });

        const res = http.post(
            `${BASE_URL}/api/v1/tasks/search`,
            searchPayload,
            { headers }
        );

        totalOperations.add(1);

        trackDegradation(res.timings.duration);

        check(res, {
            'search query OK': (r) => r.status === 200 || r.status === 404,
        });
    });

    sleep(0.3);

    // ========================================================================
    // Scenario 7: WebSocket connection test (periodic)
    // ========================================================================
    if (__ITER % 50 === 0 && __VU <= 10) { // Limited WebSocket tests
        group('WebSocket Stability', () => {
            const wsUrl = `${WS_URL}/ws/events`;

            const res = ws.connect(wsUrl, {}, function(socket) {
                activeWebsockets.add(1);
                wsOperations.add(1);

                socket.on('open', () => {
                    socket.send(JSON.stringify({
                        type: 'subscribe',
                        channels: ['tasks'],
                    }));
                });

                socket.on('message', (data) => {
                    wsOperations.add(1);
                });

                socket.on('close', () => {
                    activeWebsockets.add(-1);
                });

                // Keep connection for 10 seconds
                socket.setTimeout(() => {
                    socket.close();
                }, 10000);
            });

            check(res, {
                'ws connection OK': (r) => r && r.status === 101,
            });
        });
    }

    // Variable sleep to simulate realistic traffic
    sleep(randomIntBetween(5, 15) / 10);

    // ========================================================================
    // Periodic status logging (every ~5 minutes)
    // ========================================================================
    if (__ITER % 100 === 0 && __VU === 1) {
        console.log(`Soak test progress: ${elapsed.toFixed(1)} minutes elapsed, VU ${__VU}, Iteration ${__ITER}`);
    }
}

// ============================================================================
// Teardown
// ============================================================================

export function teardown(data) {
    const duration = (Date.now() - data.startTimestamp) / 1000 / 60; // Minutes

    console.log(`Soak test completed after ${duration.toFixed(1)} minutes`);
    console.log(`Started: ${data.startTime}`);
    console.log(`Ended: ${new Date().toISOString()}`);

    // Final health check
    const finalHealth = http.get(`${data.baseUrl}/health`, {
        headers: getHeaders(),
    });

    if (finalHealth.status !== 200) {
        console.error('WARNING: Final health check failed!');
    }
}

// ============================================================================
// Custom Summary Handler
// ============================================================================

export function handleSummary(data) {
    const duration = data.state?.testRunDurationMs
        ? (data.state.testRunDurationMs / 1000 / 60).toFixed(1)
        : 'unknown';

    // Analyze for degradation
    const avgResponseTime = data.metrics.http_req_duration?.values?.avg || 0;
    const p99ResponseTime = data.metrics.http_req_duration?.values?.['p(99)'] || 0;
    const errorRate = data.metrics.http_req_failed?.values?.rate || 0;

    // Degradation analysis
    let stabilityAnalysis = [];

    if (data.metrics.apex_response_degradation) {
        const degradationP99 = data.metrics.apex_response_degradation.values['p(99)'] || 0;
        if (degradationP99 > 500) {
            stabilityAnalysis.push(`Response time degradation detected: +${degradationP99.toFixed(0)}ms at P99`);
        }
    }

    if (errorRate > 0.02) {
        stabilityAnalysis.push(`Error rate exceeded threshold: ${(errorRate * 100).toFixed(2)}%`);
    }

    if (stabilityAnalysis.length === 0) {
        stabilityAnalysis.push('System remained stable throughout soak test');
    }

    const summary = {
        timestamp: new Date().toISOString(),
        testType: 'soak',
        durationMinutes: parseFloat(duration) || 0,
        environment: __ENV.ENVIRONMENT || 'local',
        targetVUs: TARGET_VUS,
        stabilityAnalysis,
        metrics: {
            http: {
                totalRequests: data.metrics.http_reqs?.values?.count || 0,
                requestsPerSecond: data.metrics.http_reqs?.values?.rate || 0,
                failedRequests: data.metrics.http_req_failed?.values?.passes || 0,
                errorRate: ((data.metrics.http_req_failed?.values?.rate || 0) * 100).toFixed(3) + '%',
                duration: {
                    avg: avgResponseTime,
                    p50: data.metrics.http_req_duration?.values?.['p(50)'] || 0,
                    p95: data.metrics.http_req_duration?.values?.['p(95)'] || 0,
                    p99: p99ResponseTime,
                    max: data.metrics.http_req_duration?.values?.max || 0,
                },
            },
            operations: {
                total: data.metrics.apex_total_operations?.values?.count || 0,
                tasks: data.metrics.apex_task_operations?.values?.count || 0,
                dags: data.metrics.apex_dag_operations?.values?.count || 0,
                websocket: data.metrics.apex_ws_operations?.values?.count || 0,
            },
            stability: {
                cumulativeErrors: data.metrics.apex_cumulative_errors?.values?.count || 0,
                degradation: {
                    avg: data.metrics.apex_response_degradation?.values?.avg || 0,
                    p99: data.metrics.apex_response_degradation?.values?.['p(99)'] || 0,
                },
            },
        },
        thresholds: data.thresholds || {},
    };

    return {
        'benchmarks/results/soak-test-summary.json': JSON.stringify(summary, null, 2),
        stdout: generateSoakReport(data, summary),
    };
}

function generateSoakReport(data, summary) {
    const lines = [];

    lines.push('');
    lines.push('================================================================');
    lines.push('       Project Apex - SOAK TEST RESULTS');
    lines.push('================================================================');
    lines.push('');
    lines.push(`Duration: ${summary.durationMinutes} minutes`);
    lines.push(`Target VUs: ${summary.targetVUs}`);
    lines.push('');
    lines.push('Stability Analysis:');
    for (const analysis of summary.stabilityAnalysis) {
        lines.push(`  - ${analysis}`);
    }
    lines.push('');
    lines.push('HTTP Metrics:');
    lines.push(`  Total Requests:      ${summary.metrics.http.totalRequests.toLocaleString()}`);
    lines.push(`  Requests/sec:        ${summary.metrics.http.requestsPerSecond.toFixed(2)}`);
    lines.push(`  Error Rate:          ${summary.metrics.http.errorRate}`);
    lines.push('');
    lines.push('Response Time Distribution:');
    lines.push(`  Average:  ${summary.metrics.http.duration.avg.toFixed(2)}ms`);
    lines.push(`  P50:      ${summary.metrics.http.duration.p50.toFixed(2)}ms`);
    lines.push(`  P95:      ${summary.metrics.http.duration.p95.toFixed(2)}ms`);
    lines.push(`  P99:      ${summary.metrics.http.duration.p99.toFixed(2)}ms`);
    lines.push(`  Max:      ${summary.metrics.http.duration.max.toFixed(2)}ms`);
    lines.push('');
    lines.push('Operations Executed:');
    lines.push(`  Total:      ${summary.metrics.operations.total.toLocaleString()}`);
    lines.push(`  Tasks:      ${summary.metrics.operations.tasks.toLocaleString()}`);
    lines.push(`  DAGs:       ${summary.metrics.operations.dags.toLocaleString()}`);
    lines.push(`  WebSocket:  ${summary.metrics.operations.websocket.toLocaleString()}`);
    lines.push('');
    lines.push('Stability Metrics:');
    lines.push(`  Cumulative Errors:      ${summary.metrics.stability.cumulativeErrors}`);
    lines.push(`  Degradation (avg):      ${summary.metrics.stability.degradation.avg.toFixed(2)}ms`);
    lines.push(`  Degradation (P99):      ${summary.metrics.stability.degradation.p99.toFixed(2)}ms`);
    lines.push('');

    // Threshold results
    lines.push('Threshold Results:');
    let passed = 0;
    let failed = 0;

    for (const [name, result] of Object.entries(data.thresholds || {})) {
        if (result.ok) {
            passed++;
        } else {
            failed++;
            lines.push(`  [FAIL] ${name}`);
        }
    }

    if (failed === 0) {
        lines.push('  All thresholds passed!');
    }

    lines.push('');
    lines.push(`Summary: ${passed} passed, ${failed} failed`);
    lines.push('================================================================');
    lines.push('');

    return lines.join('\n');
}
