/**
 * Project Apex - K6 Load Test
 *
 * Standard load testing with gradual ramp-up to simulate realistic traffic patterns.
 * Tests API endpoint latency, task creation throughput, and overall system performance.
 *
 * Usage:
 *   k6 run benchmarks/k6/load-test.js
 *   k6 run --out json=results.json benchmarks/k6/load-test.js
 *   K6_VUS=100 K6_DURATION=5m k6 run benchmarks/k6/load-test.js
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

// ============================================================================
// Custom Metrics
// ============================================================================

// Counters
const taskCreatedCounter = new Counter('apex_tasks_created');
const dagExecutedCounter = new Counter('apex_dags_executed');
const wsMessagesCounter = new Counter('apex_ws_messages');

// Rates
const taskCreationSuccessRate = new Rate('apex_task_creation_success');
const dagExecutionSuccessRate = new Rate('apex_dag_execution_success');

// Trends (for percentile analysis)
const taskCreationDuration = new Trend('apex_task_creation_duration', true);
const dagExecutionDuration = new Trend('apex_dag_execution_duration', true);
const taskListDuration = new Trend('apex_task_list_duration', true);
const agentListDuration = new Trend('apex_agent_list_duration', true);
const healthCheckDuration = new Trend('apex_health_check_duration', true);

// Gauges
const activeConnections = new Gauge('apex_active_connections');

// ============================================================================
// Test Options
// ============================================================================

export const options = {
    // Execution stages: ramp up, steady, peak, cool down
    stages: [
        { duration: '1m', target: 50 },   // Warm-up: ramp to 50 VUs
        { duration: '3m', target: 50 },   // Steady: maintain 50 VUs
        { duration: '1m', target: 100 },  // Ramp-up: increase to 100 VUs
        { duration: '3m', target: 100 },  // Peak: maintain 100 VUs
        { duration: '2m', target: 0 },    // Cool-down: ramp down to 0
    ],

    // Performance thresholds
    thresholds: {
        // HTTP metrics
        'http_req_duration': ['p(50)<200', 'p(95)<500', 'p(99)<1000'],
        'http_req_failed': ['rate<0.01'],   // Error rate < 1%
        'http_reqs': ['rate>100'],          // At least 100 req/s

        // Health check endpoint (should be fast)
        'apex_health_check_duration': ['p(50)<10', 'p(95)<25', 'p(99)<50'],

        // Task operations
        'apex_task_list_duration': ['p(50)<50', 'p(95)<100', 'p(99)<200'],
        'apex_task_creation_duration': ['p(50)<100', 'p(95)<200', 'p(99)<500'],
        'apex_task_creation_success': ['rate>0.95'],

        // Agent operations
        'apex_agent_list_duration': ['p(50)<50', 'p(95)<100', 'p(99)<200'],

        // DAG operations
        'apex_dag_execution_duration': ['p(50)<200', 'p(95)<500', 'p(99)<1000'],
        'apex_dag_execution_success': ['rate>0.90'],
    },

    // Tagging for results segmentation
    tags: {
        test_type: 'load',
        environment: __ENV.ENVIRONMENT || 'local',
    },

    // Summary configuration
    summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(50)', 'p(90)', 'p(95)', 'p(99)'],
};

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Get common HTTP headers including authentication if provided
 */
function getHeaders() {
    const headers = {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'User-Agent': 'K6-LoadTest/1.0',
    };

    if (AUTH_TOKEN) {
        headers['Authorization'] = `Bearer ${AUTH_TOKEN}`;
    }

    return headers;
}

/**
 * Generate a random task payload
 */
function generateTaskPayload() {
    return JSON.stringify({
        name: `Load Test Task ${randomString(8)}`,
        instruction: `Perform automated load testing task - ${randomString(16)}`,
        priority: randomIntBetween(1, 10),
        labels: ['load-test', 'automated', `batch-${randomIntBetween(1, 5)}`],
        limits: {
            token_limit: randomIntBetween(1000, 10000),
            cost_limit: randomIntBetween(1, 100) / 100,
            time_limit: randomIntBetween(60, 600),
        },
        metadata: {
            source: 'k6-load-test',
            timestamp: new Date().toISOString(),
            iteration: __ITER,
            vu: __VU,
        },
    });
}

/**
 * Generate a random DAG payload
 */
function generateDAGPayload() {
    return JSON.stringify({
        name: `Load Test DAG ${randomString(6)}`,
        description: 'DAG created for load testing purposes',
        nodes: [
            {
                id: 'node_1',
                type: 'task',
                config: {
                    instruction: 'First task in DAG',
                },
            },
            {
                id: 'node_2',
                type: 'task',
                dependencies: ['node_1'],
                config: {
                    instruction: 'Second task depending on first',
                },
            },
            {
                id: 'node_3',
                type: 'task',
                dependencies: ['node_1'],
                config: {
                    instruction: 'Third task depending on first',
                },
            },
            {
                id: 'node_4',
                type: 'aggregator',
                dependencies: ['node_2', 'node_3'],
                config: {
                    strategy: 'merge',
                },
            },
        ],
        metadata: {
            source: 'k6-load-test',
            timestamp: new Date().toISOString(),
        },
    });
}

// ============================================================================
// Setup and Teardown
// ============================================================================

/**
 * Setup function - runs once before the test
 */
export function setup() {
    console.log(`Starting load test against ${BASE_URL}`);

    // Verify API is accessible
    const healthRes = http.get(`${BASE_URL}/health`, {
        headers: getHeaders(),
        timeout: '10s',
    });

    if (healthRes.status !== 200) {
        console.error(`Health check failed: ${healthRes.status}`);
        throw new Error('API is not healthy, aborting test');
    }

    console.log('API health check passed, proceeding with load test');

    return {
        startTime: new Date().toISOString(),
        baseUrl: BASE_URL,
    };
}

/**
 * Teardown function - runs once after the test
 */
export function teardown(data) {
    console.log(`Load test completed. Started at: ${data.startTime}`);
    console.log(`End time: ${new Date().toISOString()}`);
}

// ============================================================================
// Test Scenarios
// ============================================================================

/**
 * Main test function executed by each VU
 */
export default function(data) {
    const headers = getHeaders();

    // Scenario 1: Health Check (lightweight, frequent)
    group('Health Check', () => {
        const start = Date.now();
        const res = http.get(`${BASE_URL}/health`, { headers });
        const duration = Date.now() - start;

        healthCheckDuration.add(duration);

        check(res, {
            'health check status is 200': (r) => r.status === 200,
            'health check response time < 50ms': (r) => r.timings.duration < 50,
            'health check has status field': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body.status !== undefined;
                } catch {
                    return false;
                }
            },
        });
    });

    sleep(0.5);

    // Scenario 2: List Tasks (read-heavy operation)
    group('List Tasks', () => {
        const start = Date.now();
        const res = http.get(`${BASE_URL}/api/v1/tasks`, { headers });
        const duration = Date.now() - start;

        taskListDuration.add(duration);

        check(res, {
            'list tasks status is 200': (r) => r.status === 200,
            'list tasks response time < 200ms': (r) => r.timings.duration < 200,
            'list tasks returns array': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return Array.isArray(body.tasks || body.data || body);
                } catch {
                    return false;
                }
            },
        });
    });

    sleep(0.3);

    // Scenario 3: List Agents
    group('List Agents', () => {
        const start = Date.now();
        const res = http.get(`${BASE_URL}/api/v1/agents`, { headers });
        const duration = Date.now() - start;

        agentListDuration.add(duration);

        check(res, {
            'list agents status is 200': (r) => r.status === 200,
            'list agents response time < 200ms': (r) => r.timings.duration < 200,
        });
    });

    sleep(0.3);

    // Scenario 4: Create Task (write operation - less frequent)
    if (__ITER % 3 === 0) { // Only every 3rd iteration
        group('Create Task', () => {
            const payload = generateTaskPayload();
            const start = Date.now();

            const res = http.post(`${BASE_URL}/api/v1/tasks`, payload, { headers });
            const duration = Date.now() - start;

            taskCreationDuration.add(duration);

            const success = check(res, {
                'create task status is 201 or 200': (r) => r.status === 201 || r.status === 200,
                'create task response time < 500ms': (r) => r.timings.duration < 500,
                'create task returns task id': (r) => {
                    try {
                        const body = JSON.parse(r.body);
                        return body.id !== undefined || body.task_id !== undefined;
                    } catch {
                        return false;
                    }
                },
            });

            taskCreationSuccessRate.add(success ? 1 : 0);

            if (success) {
                taskCreatedCounter.add(1);
            }
        });
    }

    sleep(0.5);

    // Scenario 5: Get Task by ID (if we created one)
    group('Get Task Details', () => {
        // First, get a task ID from the list
        const listRes = http.get(`${BASE_URL}/api/v1/tasks?limit=1`, { headers });

        if (listRes.status === 200) {
            try {
                const body = JSON.parse(listRes.body);
                const tasks = body.tasks || body.data || body;

                if (Array.isArray(tasks) && tasks.length > 0) {
                    const taskId = tasks[0].id || tasks[0].task_id;

                    if (taskId) {
                        const res = http.get(`${BASE_URL}/api/v1/tasks/${taskId}`, { headers });

                        check(res, {
                            'get task status is 200': (r) => r.status === 200,
                            'get task response time < 100ms': (r) => r.timings.duration < 100,
                        });
                    }
                }
            } catch {
                // Skip if parsing fails
            }
        }
    });

    sleep(0.3);

    // Scenario 6: List DAGs
    group('List DAGs', () => {
        const res = http.get(`${BASE_URL}/api/v1/dags`, { headers });

        check(res, {
            'list dags status is 200': (r) => r.status === 200,
            'list dags response time < 200ms': (r) => r.timings.duration < 200,
        });
    });

    sleep(0.3);

    // Scenario 7: DAG Operations (less frequent)
    if (__ITER % 10 === 0) { // Only every 10th iteration
        group('DAG Execution', () => {
            // Create a DAG
            const dagPayload = generateDAGPayload();
            const createRes = http.post(`${BASE_URL}/api/v1/dags`, dagPayload, { headers });

            if (createRes.status === 201 || createRes.status === 200) {
                try {
                    const body = JSON.parse(createRes.body);
                    const dagId = body.id || body.dag_id;

                    if (dagId) {
                        // Execute the DAG
                        const start = Date.now();
                        const execRes = http.post(
                            `${BASE_URL}/api/v1/dags/${dagId}/execute`,
                            JSON.stringify({ async: true }),
                            { headers }
                        );
                        const duration = Date.now() - start;

                        dagExecutionDuration.add(duration);

                        const success = check(execRes, {
                            'dag execution accepted': (r) => r.status >= 200 && r.status < 300,
                            'dag execution response time < 1000ms': (r) => r.timings.duration < 1000,
                        });

                        dagExecutionSuccessRate.add(success ? 1 : 0);

                        if (success) {
                            dagExecutedCounter.add(1);
                        }
                    }
                } catch {
                    // Skip if parsing fails
                }
            }
        });
    }

    sleep(0.5);

    // Scenario 8: Metrics Endpoint
    group('Metrics', () => {
        const res = http.get(`${BASE_URL}/metrics`, { headers });

        check(res, {
            'metrics endpoint accessible': (r) => r.status === 200,
            'metrics response time < 100ms': (r) => r.timings.duration < 100,
        });
    });

    sleep(0.3);

    // Scenario 9: Database Query Performance (via search endpoint)
    group('Search/Query', () => {
        const searchPayload = JSON.stringify({
            query: 'test',
            limit: 10,
            offset: 0,
        });

        const res = http.post(`${BASE_URL}/api/v1/tasks/search`, searchPayload, { headers });

        check(res, {
            'search returns results': (r) => r.status === 200 || r.status === 404,
            'search response time < 300ms': (r) => r.timings.duration < 300,
        });
    });

    sleep(randomIntBetween(1, 3));
}

// ============================================================================
// WebSocket Test Scenario
// ============================================================================

/**
 * WebSocket connection test (run separately or as part of scenarios)
 */
export function websocketTest() {
    const wsUrl = `${WS_URL}/ws/events`;

    const res = ws.connect(wsUrl, {}, function(socket) {
        activeConnections.add(1);

        socket.on('open', () => {
            console.log('WebSocket connected');

            // Subscribe to events
            socket.send(JSON.stringify({
                type: 'subscribe',
                channels: ['tasks', 'agents', 'dags'],
            }));
        });

        socket.on('message', (data) => {
            wsMessagesCounter.add(1);

            check(data, {
                'ws message is valid JSON': () => {
                    try {
                        JSON.parse(data);
                        return true;
                    } catch {
                        return false;
                    }
                },
            });
        });

        socket.on('close', () => {
            activeConnections.add(-1);
        });

        socket.on('error', (e) => {
            console.error('WebSocket error:', e);
        });

        // Keep connection alive for 30 seconds
        socket.setTimeout(() => {
            socket.close();
        }, 30000);
    });

    check(res, {
        'ws connection established': (r) => r && r.status === 101,
    });
}

// ============================================================================
// Summary Handler
// ============================================================================

/**
 * Custom summary handler for enhanced reporting
 */
export function handleSummary(data) {
    const summary = {
        timestamp: new Date().toISOString(),
        testType: 'load',
        environment: __ENV.ENVIRONMENT || 'local',
        baseUrl: BASE_URL,
        metrics: {
            http: {
                requests: data.metrics.http_reqs?.values?.count || 0,
                failures: data.metrics.http_req_failed?.values?.passes || 0,
                duration: {
                    avg: data.metrics.http_req_duration?.values?.avg || 0,
                    p50: data.metrics.http_req_duration?.values?.['p(50)'] || 0,
                    p95: data.metrics.http_req_duration?.values?.['p(95)'] || 0,
                    p99: data.metrics.http_req_duration?.values?.['p(99)'] || 0,
                },
            },
            custom: {
                tasksCreated: data.metrics.apex_tasks_created?.values?.count || 0,
                dagsExecuted: data.metrics.apex_dags_executed?.values?.count || 0,
                wsMessages: data.metrics.apex_ws_messages?.values?.count || 0,
            },
        },
        thresholds: data.thresholds || {},
    };

    return {
        'benchmarks/results/load-test-summary.json': JSON.stringify(summary, null, 2),
        stdout: textSummary(data, { indent: ' ', enableColors: true }),
    };
}

/**
 * Generate text summary
 */
function textSummary(data, options) {
    const lines = [];
    const indent = options.indent || '';

    lines.push('');
    lines.push(`${indent}===============================================`);
    lines.push(`${indent}  Project Apex - Load Test Results`);
    lines.push(`${indent}===============================================`);
    lines.push('');

    // HTTP metrics
    if (data.metrics.http_reqs) {
        lines.push(`${indent}HTTP Requests:`);
        lines.push(`${indent}  Total:     ${data.metrics.http_reqs.values.count}`);
        lines.push(`${indent}  Rate:      ${data.metrics.http_reqs.values.rate?.toFixed(2)} req/s`);
    }

    if (data.metrics.http_req_duration) {
        lines.push('');
        lines.push(`${indent}Response Times:`);
        lines.push(`${indent}  Average:   ${data.metrics.http_req_duration.values.avg?.toFixed(2)}ms`);
        lines.push(`${indent}  P50:       ${data.metrics.http_req_duration.values['p(50)']?.toFixed(2)}ms`);
        lines.push(`${indent}  P95:       ${data.metrics.http_req_duration.values['p(95)']?.toFixed(2)}ms`);
        lines.push(`${indent}  P99:       ${data.metrics.http_req_duration.values['p(99)']?.toFixed(2)}ms`);
    }

    if (data.metrics.http_req_failed) {
        lines.push('');
        lines.push(`${indent}Errors:`);
        lines.push(`${indent}  Failed:    ${data.metrics.http_req_failed.values.passes} (${(data.metrics.http_req_failed.values.rate * 100).toFixed(2)}%)`);
    }

    // Custom metrics
    lines.push('');
    lines.push(`${indent}Custom Metrics:`);

    if (data.metrics.apex_tasks_created) {
        lines.push(`${indent}  Tasks Created:  ${data.metrics.apex_tasks_created.values.count}`);
    }

    if (data.metrics.apex_dags_executed) {
        lines.push(`${indent}  DAGs Executed:  ${data.metrics.apex_dags_executed.values.count}`);
    }

    // Threshold results
    lines.push('');
    lines.push(`${indent}Threshold Results:`);

    let passed = 0;
    let failed = 0;

    for (const [name, result] of Object.entries(data.thresholds || {})) {
        if (result.ok) {
            passed++;
            lines.push(`${indent}  [PASS] ${name}`);
        } else {
            failed++;
            lines.push(`${indent}  [FAIL] ${name}`);
        }
    }

    lines.push('');
    lines.push(`${indent}Summary: ${passed} passed, ${failed} failed`);
    lines.push(`${indent}===============================================`);
    lines.push('');

    return lines.join('\n');
}
