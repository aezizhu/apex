/**
 * Project Apex - K6 Stress Test
 *
 * Stress testing to find the system's breaking point.
 * Progressively increases load beyond normal capacity to identify failure thresholds.
 *
 * Usage:
 *   k6 run benchmarks/k6/stress-test.js
 *   k6 run --out json=results.json benchmarks/k6/stress-test.js
 */

import http from 'k6/http';
import { check, sleep, group, fail } from 'k6';
import { Counter, Rate, Trend, Gauge } from 'k6/metrics';
import { randomString, randomIntBetween } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

// ============================================================================
// Configuration
// ============================================================================

const BASE_URL = __ENV.API_URL || 'http://localhost:8080';
const AUTH_TOKEN = __ENV.AUTH_TOKEN || '';

// Breaking point thresholds
const MAX_ACCEPTABLE_P99 = 5000;   // 5 seconds
const MAX_ACCEPTABLE_ERROR_RATE = 0.15; // 15%

// ============================================================================
// Custom Metrics
// ============================================================================

// Counters
const requestsAtBreakpoint = new Counter('apex_requests_at_breakpoint');
const errorsAtBreakpoint = new Counter('apex_errors_at_breakpoint');
const timeoutsCounter = new Counter('apex_timeouts');

// Rates
const errorRate = new Rate('apex_error_rate');
const timeoutRate = new Rate('apex_timeout_rate');

// Trends
const responseTimeUnderLoad = new Trend('apex_response_time_under_load', true);
const taskCreationUnderStress = new Trend('apex_task_creation_under_stress', true);
const dagExecutionUnderStress = new Trend('apex_dag_execution_under_stress', true);
const dbQueryUnderStress = new Trend('apex_db_query_under_stress', true);

// Gauges
const currentVUs = new Gauge('apex_current_vus');
const systemBreakpoint = new Gauge('apex_system_breakpoint');

// ============================================================================
// Test Options - Stress Test Stages
// ============================================================================

export const options = {
    // Stress test stages: progressively increase load to find breaking point
    stages: [
        // Stage 1: Normal load baseline
        { duration: '2m', target: 100 },   // Ramp up to normal load

        // Stage 2: Sustained normal load
        { duration: '5m', target: 100 },   // Maintain normal load

        // Stage 3: Start stress
        { duration: '2m', target: 200 },   // Ramp up to stress level

        // Stage 4: Sustained stress
        { duration: '5m', target: 200 },   // Maintain stress load

        // Stage 5: High stress
        { duration: '2m', target: 300 },   // Ramp up to high stress

        // Stage 6: Sustained high stress
        { duration: '5m', target: 300 },   // Maintain high stress

        // Stage 7: Breaking point test
        { duration: '2m', target: 400 },   // Approach breaking point

        // Stage 8: Maximum stress
        { duration: '5m', target: 400 },   // Test at maximum

        // Stage 9: Beyond breaking point
        { duration: '2m', target: 500 },   // Push beyond limits

        // Stage 10: Recovery phase
        { duration: '5m', target: 0 },     // Gradual recovery
    ],

    // Stress test thresholds (more lenient than load test)
    thresholds: {
        // HTTP metrics - more lenient for stress test
        'http_req_duration': ['p(50)<1000', 'p(95)<2000', 'p(99)<5000'],
        'http_req_failed': ['rate<0.15'],   // Allow up to 15% error rate

        // Custom stress metrics
        'apex_response_time_under_load': ['p(50)<1000', 'p(95)<3000'],
        'apex_error_rate': ['rate<0.20'],    // Max 20% errors during stress
        'apex_timeout_rate': ['rate<0.10'],  // Max 10% timeouts

        // Task operations under stress
        'apex_task_creation_under_stress': ['p(95)<2000', 'p(99)<5000'],

        // DAG operations under stress
        'apex_dag_execution_under_stress': ['p(95)<5000'],
    },

    // Tagging
    tags: {
        test_type: 'stress',
        environment: __ENV.ENVIRONMENT || 'local',
    },

    // Summary configuration
    summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(50)', 'p(90)', 'p(95)', 'p(99)'],

    // Batch settings for high concurrency
    batch: 20,
    batchPerHost: 10,

    // DNS caching
    dns: {
        ttl: '1m',
        select: 'roundRobin',
        policy: 'preferIPv4',
    },
};

// ============================================================================
// Helper Functions
// ============================================================================

function getHeaders() {
    const headers = {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'User-Agent': 'K6-StressTest/1.0',
    };

    if (AUTH_TOKEN) {
        headers['Authorization'] = `Bearer ${AUTH_TOKEN}`;
    }

    return headers;
}

function generateTaskPayload() {
    return JSON.stringify({
        name: `Stress Test Task ${randomString(8)}`,
        instruction: `Stress testing - ${randomString(16)}`,
        priority: randomIntBetween(1, 10),
        labels: ['stress-test', 'automated'],
        limits: {
            token_limit: randomIntBetween(1000, 5000),
            cost_limit: randomIntBetween(1, 50) / 100,
            time_limit: randomIntBetween(30, 300),
        },
        metadata: {
            source: 'k6-stress-test',
            timestamp: new Date().toISOString(),
            vu: __VU,
            iteration: __ITER,
        },
    });
}

function generateBulkTaskPayload(count) {
    const tasks = [];
    for (let i = 0; i < count; i++) {
        tasks.push({
            name: `Bulk Task ${i + 1} - ${randomString(6)}`,
            instruction: `Bulk task for stress testing`,
            priority: randomIntBetween(1, 5),
        });
    }
    return JSON.stringify({ tasks });
}

function generateComplexDAGPayload() {
    const nodeCount = randomIntBetween(5, 15);
    const nodes = [];

    // Create root node
    nodes.push({
        id: 'root',
        type: 'task',
        config: { instruction: 'Root task' },
    });

    // Create intermediate nodes with dependencies
    for (let i = 1; i < nodeCount - 1; i++) {
        const deps = [];
        // Random dependencies to previous nodes
        for (let j = 0; j < i; j++) {
            if (Math.random() > 0.7) {
                deps.push(nodes[j].id);
            }
        }
        if (deps.length === 0) {
            deps.push('root');
        }

        nodes.push({
            id: `node_${i}`,
            type: 'task',
            dependencies: deps,
            config: { instruction: `Intermediate task ${i}` },
        });
    }

    // Create final aggregator node
    nodes.push({
        id: 'final',
        type: 'aggregator',
        dependencies: nodes.slice(-3).map(n => n.id),
        config: { strategy: 'merge' },
    });

    return JSON.stringify({
        name: `Complex DAG ${randomString(6)}`,
        description: 'Complex DAG for stress testing',
        nodes: nodes,
    });
}

// ============================================================================
// Setup
// ============================================================================

export function setup() {
    console.log(`Starting stress test against ${BASE_URL}`);
    console.log('Stages: Normal -> Stress -> High Stress -> Breaking Point -> Recovery');

    // Verify API is up
    const res = http.get(`${BASE_URL}/health`, {
        headers: getHeaders(),
        timeout: '30s',
    });

    if (res.status !== 200) {
        console.error(`Health check failed with status ${res.status}`);
        fail('API is not healthy');
    }

    return {
        startTime: new Date().toISOString(),
        baseUrl: BASE_URL,
        breakpointVUs: 0,
        maxSuccessfulVUs: 0,
    };
}

// ============================================================================
// Main Test Function
// ============================================================================

export default function(data) {
    const headers = getHeaders();

    // Track current VU count
    currentVUs.add(__VU);

    // ========================================================================
    // Scenario 1: High-frequency health checks
    // ========================================================================
    group('Health Check Stress', () => {
        const res = http.get(`${BASE_URL}/health`, {
            headers,
            timeout: '10s',
        });

        const start = Date.now();
        const isTimeout = res.timings.duration >= 10000;

        if (isTimeout) {
            timeoutsCounter.add(1);
            timeoutRate.add(1);
        } else {
            timeoutRate.add(0);
        }

        responseTimeUnderLoad.add(res.timings.duration);

        const passed = check(res, {
            'health check responds': (r) => r.status === 200,
            'health check under 1s': (r) => r.timings.duration < 1000,
        });

        errorRate.add(passed ? 0 : 1);
    });

    sleep(0.1);

    // ========================================================================
    // Scenario 2: Rapid task listing
    // ========================================================================
    group('Task List Stress', () => {
        // Multiple rapid requests
        const requests = [
            ['tasks', { method: 'GET', url: `${BASE_URL}/api/v1/tasks?limit=50`, params: { headers } }],
            ['tasks_recent', { method: 'GET', url: `${BASE_URL}/api/v1/tasks?sort=created_at&order=desc&limit=20`, params: { headers } }],
            ['tasks_pending', { method: 'GET', url: `${BASE_URL}/api/v1/tasks?status=pending&limit=20`, params: { headers } }],
        ];

        const responses = http.batch(requests);

        for (const [name, res] of Object.entries(responses)) {
            responseTimeUnderLoad.add(res.timings.duration);

            const passed = check(res, {
                [`${name} responds`]: (r) => r.status === 200,
            });

            errorRate.add(passed ? 0 : 1);

            if (res.status >= 500 || res.timings.duration > MAX_ACCEPTABLE_P99) {
                requestsAtBreakpoint.add(1);
            }
        }
    });

    sleep(0.1);

    // ========================================================================
    // Scenario 3: Concurrent task creation
    // ========================================================================
    group('Task Creation Stress', () => {
        const batchSize = Math.min(5, Math.ceil(__VU / 50)); // Scale with VUs

        const requests = [];
        for (let i = 0; i < batchSize; i++) {
            requests.push([
                `task_${i}`,
                {
                    method: 'POST',
                    url: `${BASE_URL}/api/v1/tasks`,
                    body: generateTaskPayload(),
                    params: { headers, timeout: '30s' },
                },
            ]);
        }

        const responses = http.batch(requests);

        for (const [name, res] of Object.entries(responses)) {
            taskCreationUnderStress.add(res.timings.duration);

            const passed = check(res, {
                [`${name} created`]: (r) => r.status === 201 || r.status === 200,
                [`${name} under 2s`]: (r) => r.timings.duration < 2000,
            });

            errorRate.add(passed ? 0 : 1);

            if (!passed) {
                errorsAtBreakpoint.add(1);
            }
        }
    });

    sleep(0.2);

    // ========================================================================
    // Scenario 4: Agent operations under stress
    // ========================================================================
    group('Agent Operations Stress', () => {
        const requests = [
            ['agents', { method: 'GET', url: `${BASE_URL}/api/v1/agents`, params: { headers } }],
            ['agents_active', { method: 'GET', url: `${BASE_URL}/api/v1/agents?status=active`, params: { headers } }],
        ];

        const responses = http.batch(requests);

        for (const [name, res] of Object.entries(responses)) {
            responseTimeUnderLoad.add(res.timings.duration);

            check(res, {
                [`${name} responds`]: (r) => r.status === 200,
            });
        }
    });

    sleep(0.1);

    // ========================================================================
    // Scenario 5: DAG operations (complex workload)
    // ========================================================================
    if (__ITER % 5 === 0) { // Every 5th iteration
        group('DAG Stress', () => {
            // Create and execute a complex DAG
            const dagPayload = generateComplexDAGPayload();

            const createStart = Date.now();
            const createRes = http.post(`${BASE_URL}/api/v1/dags`, dagPayload, {
                headers,
                timeout: '30s',
            });

            if (createRes.status === 201 || createRes.status === 200) {
                try {
                    const dag = JSON.parse(createRes.body);
                    const dagId = dag.id || dag.dag_id;

                    if (dagId) {
                        // Execute the DAG
                        const execStart = Date.now();
                        const execRes = http.post(
                            `${BASE_URL}/api/v1/dags/${dagId}/execute`,
                            JSON.stringify({ async: true }),
                            { headers, timeout: '30s' }
                        );

                        dagExecutionUnderStress.add(Date.now() - execStart);

                        check(execRes, {
                            'dag execution accepted': (r) => r.status >= 200 && r.status < 300,
                        });

                        // Check execution status
                        if (execRes.status >= 200 && execRes.status < 300) {
                            try {
                                const execBody = JSON.parse(execRes.body);
                                const execId = execBody.execution_id || execBody.id;

                                if (execId) {
                                    // Poll for status (limited attempts)
                                    for (let i = 0; i < 3; i++) {
                                        sleep(1);
                                        const statusRes = http.get(
                                            `${BASE_URL}/api/v1/dags/${dagId}/executions/${execId}`,
                                            { headers }
                                        );

                                        if (statusRes.status === 200) {
                                            const status = JSON.parse(statusRes.body);
                                            if (status.status === 'completed' || status.status === 'failed') {
                                                break;
                                            }
                                        }
                                    }
                                }
                            } catch {
                                // Skip polling errors
                            }
                        }
                    }
                } catch {
                    // Skip DAG creation errors under stress
                }
            }
        });
    }

    sleep(0.2);

    // ========================================================================
    // Scenario 6: Database query stress
    // ========================================================================
    group('Database Query Stress', () => {
        const queries = [
            // Simple queries
            { endpoint: '/api/v1/tasks?limit=100', name: 'large_list' },
            { endpoint: '/api/v1/tasks/search', method: 'POST', body: { query: 'test', limit: 50 }, name: 'search' },
            // Aggregation queries
            { endpoint: '/api/v1/stats/tasks', name: 'task_stats' },
            { endpoint: '/api/v1/stats/agents', name: 'agent_stats' },
        ];

        for (const query of queries) {
            const start = Date.now();
            let res;

            if (query.method === 'POST') {
                res = http.post(
                    `${BASE_URL}${query.endpoint}`,
                    JSON.stringify(query.body),
                    { headers, timeout: '15s' }
                );
            } else {
                res = http.get(`${BASE_URL}${query.endpoint}`, {
                    headers,
                    timeout: '15s',
                });
            }

            dbQueryUnderStress.add(Date.now() - start);

            check(res, {
                [`${query.name} responds`]: (r) => r.status === 200 || r.status === 404,
            });
        }
    });

    sleep(0.1);

    // ========================================================================
    // Scenario 7: Concurrent mixed operations (realistic stress)
    // ========================================================================
    group('Mixed Operations Stress', () => {
        const operations = [
            { method: 'GET', url: `${BASE_URL}/api/v1/tasks`, params: { headers } },
            { method: 'GET', url: `${BASE_URL}/api/v1/agents`, params: { headers } },
            { method: 'GET', url: `${BASE_URL}/api/v1/dags`, params: { headers } },
            { method: 'GET', url: `${BASE_URL}/health`, params: { headers } },
            { method: 'GET', url: `${BASE_URL}/metrics`, params: { headers } },
        ];

        const requests = operations.map((op, i) => [`op_${i}`, op]);
        const responses = http.batch(requests);

        let allPassed = true;
        for (const [name, res] of Object.entries(responses)) {
            responseTimeUnderLoad.add(res.timings.duration);

            if (res.status !== 200) {
                allPassed = false;
            }
        }

        errorRate.add(allPassed ? 0 : 1);
    });

    // Random sleep to simulate realistic traffic patterns
    sleep(randomIntBetween(1, 3) / 10);
}

// ============================================================================
// Teardown
// ============================================================================

export function teardown(data) {
    console.log(`Stress test completed. Started at: ${data.startTime}`);
    console.log(`End time: ${new Date().toISOString()}`);
}

// ============================================================================
// Custom Summary Handler
// ============================================================================

export function handleSummary(data) {
    // Analyze breaking point
    const p99Duration = data.metrics.http_req_duration?.values?.['p(99)'] || 0;
    const failRate = data.metrics.http_req_failed?.values?.rate || 0;

    let breakpointAnalysis = 'System handled all stress levels';

    if (p99Duration > MAX_ACCEPTABLE_P99) {
        breakpointAnalysis = `Performance degradation detected: P99 ${p99Duration.toFixed(0)}ms exceeds threshold`;
    }

    if (failRate > MAX_ACCEPTABLE_ERROR_RATE) {
        breakpointAnalysis += ` | Error rate ${(failRate * 100).toFixed(1)}% exceeds threshold`;
    }

    const summary = {
        timestamp: new Date().toISOString(),
        testType: 'stress',
        environment: __ENV.ENVIRONMENT || 'local',
        baseUrl: BASE_URL,
        breakpointAnalysis,
        metrics: {
            http: {
                totalRequests: data.metrics.http_reqs?.values?.count || 0,
                failedRequests: data.metrics.http_req_failed?.values?.passes || 0,
                errorRate: ((data.metrics.http_req_failed?.values?.rate || 0) * 100).toFixed(2) + '%',
                duration: {
                    avg: data.metrics.http_req_duration?.values?.avg || 0,
                    p50: data.metrics.http_req_duration?.values?.['p(50)'] || 0,
                    p95: data.metrics.http_req_duration?.values?.['p(95)'] || 0,
                    p99: data.metrics.http_req_duration?.values?.['p(99)'] || 0,
                    max: data.metrics.http_req_duration?.values?.max || 0,
                },
            },
            stress: {
                requestsAtBreakpoint: data.metrics.apex_requests_at_breakpoint?.values?.count || 0,
                errorsAtBreakpoint: data.metrics.apex_errors_at_breakpoint?.values?.count || 0,
                timeouts: data.metrics.apex_timeouts?.values?.count || 0,
            },
            operations: {
                taskCreation: {
                    p95: data.metrics.apex_task_creation_under_stress?.values?.['p(95)'] || 0,
                    p99: data.metrics.apex_task_creation_under_stress?.values?.['p(99)'] || 0,
                },
                dagExecution: {
                    p95: data.metrics.apex_dag_execution_under_stress?.values?.['p(95)'] || 0,
                    p99: data.metrics.apex_dag_execution_under_stress?.values?.['p(99)'] || 0,
                },
                dbQueries: {
                    p95: data.metrics.apex_db_query_under_stress?.values?.['p(95)'] || 0,
                    p99: data.metrics.apex_db_query_under_stress?.values?.['p(99)'] || 0,
                },
            },
        },
        thresholds: data.thresholds || {},
    };

    return {
        'benchmarks/results/stress-test-summary.json': JSON.stringify(summary, null, 2),
        stdout: generateTextReport(data, summary),
    };
}

function generateTextReport(data, summary) {
    const lines = [];

    lines.push('');
    lines.push('================================================================');
    lines.push('       Project Apex - STRESS TEST RESULTS');
    lines.push('================================================================');
    lines.push('');
    lines.push(`Breaking Point Analysis: ${summary.breakpointAnalysis}`);
    lines.push('');
    lines.push('HTTP Metrics:');
    lines.push(`  Total Requests:    ${summary.metrics.http.totalRequests}`);
    lines.push(`  Failed Requests:   ${summary.metrics.http.failedRequests}`);
    lines.push(`  Error Rate:        ${summary.metrics.http.errorRate}`);
    lines.push('');
    lines.push('Response Time Distribution:');
    lines.push(`  Average:  ${summary.metrics.http.duration.avg.toFixed(2)}ms`);
    lines.push(`  P50:      ${summary.metrics.http.duration.p50.toFixed(2)}ms`);
    lines.push(`  P95:      ${summary.metrics.http.duration.p95.toFixed(2)}ms`);
    lines.push(`  P99:      ${summary.metrics.http.duration.p99.toFixed(2)}ms`);
    lines.push(`  Max:      ${summary.metrics.http.duration.max.toFixed(2)}ms`);
    lines.push('');
    lines.push('Stress Indicators:');
    lines.push(`  Requests at Breakpoint: ${summary.metrics.stress.requestsAtBreakpoint}`);
    lines.push(`  Errors at Breakpoint:   ${summary.metrics.stress.errorsAtBreakpoint}`);
    lines.push(`  Timeouts:               ${summary.metrics.stress.timeouts}`);
    lines.push('');
    lines.push('Operation Performance (P99):');
    lines.push(`  Task Creation: ${summary.metrics.operations.taskCreation.p99.toFixed(2)}ms`);
    lines.push(`  DAG Execution: ${summary.metrics.operations.dagExecution.p99.toFixed(2)}ms`);
    lines.push(`  DB Queries:    ${summary.metrics.operations.dbQueries.p99.toFixed(2)}ms`);
    lines.push('');

    // Threshold results
    lines.push('Threshold Results:');
    let passed = 0;
    let failed = 0;

    for (const [name, result] of Object.entries(data.thresholds || {})) {
        if (result.ok) {
            passed++;
            lines.push(`  [PASS] ${name}`);
        } else {
            failed++;
            lines.push(`  [FAIL] ${name}`);
        }
    }

    lines.push('');
    lines.push(`Summary: ${passed} passed, ${failed} failed`);
    lines.push('================================================================');
    lines.push('');

    return lines.join('\n');
}
