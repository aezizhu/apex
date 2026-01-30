"""
Project Apex - Locust Load Testing

Comprehensive load testing using Locust framework with Python.
Provides a web UI for real-time monitoring and distributed testing capabilities.

Usage:
    # Web UI mode
    locust -f locustfile.py --host=http://localhost:8080

    # Headless mode
    locust -f locustfile.py --host=http://localhost:8080 \
        --headless --users 100 --spawn-rate 10 --run-time 5m

    # With CSV output
    locust -f locustfile.py --host=http://localhost:8080 \
        --headless --users 100 --spawn-rate 10 --run-time 5m \
        --csv=results/locust
"""

import json
import random
import string
import time
import os
from datetime import datetime
from typing import Dict, Any, Optional

from locust import HttpUser, task, between, events, tag
from locust.runners import MasterRunner, WorkerRunner

# ============================================================================
# Configuration
# ============================================================================

AUTH_TOKEN = os.environ.get("AUTH_TOKEN", "")
ENABLE_WEBSOCKET = os.environ.get("ENABLE_WEBSOCKET", "false").lower() == "true"

# Performance thresholds (in milliseconds)
THRESHOLDS = {
    "health_check": {"p50": 10, "p95": 25, "p99": 50},
    "list_tasks": {"p50": 50, "p95": 100, "p99": 200},
    "create_task": {"p50": 100, "p95": 200, "p99": 500},
    "list_agents": {"p50": 50, "p95": 100, "p99": 200},
    "list_dags": {"p50": 50, "p95": 100, "p99": 200},
    "execute_dag": {"p50": 200, "p95": 500, "p99": 1000},
    "search": {"p50": 100, "p95": 200, "p99": 300},
}

# ============================================================================
# Helper Functions
# ============================================================================


def random_string(length: int = 8) -> str:
    """Generate a random string of specified length."""
    return "".join(random.choices(string.ascii_lowercase + string.digits, k=length))


def generate_task_payload() -> Dict[str, Any]:
    """Generate a random task payload."""
    return {
        "name": f"Locust Test Task {random_string()}",
        "instruction": f"Automated load testing task - {random_string(16)}",
        "priority": random.randint(1, 10),
        "labels": ["locust-test", "automated", f"batch-{random.randint(1, 5)}"],
        "limits": {
            "token_limit": random.randint(1000, 10000),
            "cost_limit": round(random.uniform(0.01, 1.0), 2),
            "time_limit": random.randint(60, 600),
        },
        "metadata": {
            "source": "locust-load-test",
            "timestamp": datetime.utcnow().isoformat(),
        },
    }


def generate_dag_payload() -> Dict[str, Any]:
    """Generate a DAG payload with multiple nodes."""
    node_count = random.randint(3, 7)
    nodes = []

    # Root node
    nodes.append(
        {
            "id": "root",
            "type": "task",
            "config": {"instruction": "Root task"},
        }
    )

    # Intermediate nodes
    for i in range(1, node_count - 1):
        deps = []
        for j in range(i):
            if random.random() > 0.6:
                deps.append(nodes[j]["id"])
        if not deps:
            deps.append("root")

        nodes.append(
            {
                "id": f"node_{i}",
                "type": "task",
                "dependencies": deps,
                "config": {"instruction": f"Task {i}"},
            }
        )

    # Final aggregator
    nodes.append(
        {
            "id": "final",
            "type": "aggregator",
            "dependencies": [n["id"] for n in nodes[-2:]],
            "config": {"strategy": "merge"},
        }
    )

    return {
        "name": f"Locust DAG {random_string(6)}",
        "description": "DAG created for load testing",
        "nodes": nodes,
    }


# ============================================================================
# Custom Event Handlers
# ============================================================================


@events.test_start.add_listener
def on_test_start(environment, **kwargs):
    """Called when the test starts."""
    print("=" * 60)
    print("  Project Apex - Locust Load Test Started")
    print("=" * 60)
    print(f"  Target Host: {environment.host}")
    print(f"  Start Time: {datetime.utcnow().isoformat()}")
    print("=" * 60)


@events.test_stop.add_listener
def on_test_stop(environment, **kwargs):
    """Called when the test stops."""
    print("=" * 60)
    print("  Project Apex - Locust Load Test Completed")
    print("=" * 60)
    print(f"  End Time: {datetime.utcnow().isoformat()}")

    # Print summary statistics
    if environment.stats.total:
        stats = environment.stats.total
        print(f"  Total Requests: {stats.num_requests}")
        print(f"  Failed Requests: {stats.num_failures}")
        print(f"  Error Rate: {stats.fail_ratio * 100:.2f}%")
        print(f"  Average Response Time: {stats.avg_response_time:.2f}ms")
        print(f"  P50: {stats.get_response_time_percentile(0.50):.2f}ms")
        print(f"  P95: {stats.get_response_time_percentile(0.95):.2f}ms")
        print(f"  P99: {stats.get_response_time_percentile(0.99):.2f}ms")
        print(f"  Requests/sec: {stats.total_rps:.2f}")
    print("=" * 60)


@events.request.add_listener
def on_request(
    request_type,
    name,
    response_time,
    response_length,
    response,
    context,
    exception,
    **kwargs,
):
    """Called for each request - can be used for custom logging."""
    # Log slow requests
    if response_time > 5000:
        print(f"SLOW REQUEST: {name} took {response_time:.2f}ms")

    # Log failures
    if exception:
        print(f"FAILED REQUEST: {name} - {exception}")


# ============================================================================
# User Classes
# ============================================================================


class ApexAPIUser(HttpUser):
    """
    Standard API user simulating typical application usage patterns.
    """

    wait_time = between(1, 3)
    weight = 10  # Most common user type

    def on_start(self):
        """Called when a user starts."""
        self.headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "Locust-LoadTest/1.0",
        }
        if AUTH_TOKEN:
            self.headers["Authorization"] = f"Bearer {AUTH_TOKEN}"

        # Store created resources for cleanup/verification
        self.created_tasks = []
        self.created_dags = []

    def on_stop(self):
        """Called when a user stops."""
        # Optional: cleanup created resources
        pass

    @task(10)
    @tag("health", "critical")
    def health_check(self):
        """Health check endpoint - high frequency."""
        with self.client.get(
            "/health", headers=self.headers, name="GET /health", catch_response=True
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Health check failed: {response.status_code}")

    @task(8)
    @tag("tasks", "read")
    def list_tasks(self):
        """List tasks endpoint."""
        params = {"limit": random.choice([10, 25, 50]), "offset": random.randint(0, 100)}

        with self.client.get(
            "/api/v1/tasks",
            headers=self.headers,
            params=params,
            name="GET /api/v1/tasks",
            catch_response=True,
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"List tasks failed: {response.status_code}")

    @task(3)
    @tag("tasks", "write")
    def create_task(self):
        """Create a new task."""
        payload = generate_task_payload()

        with self.client.post(
            "/api/v1/tasks",
            headers=self.headers,
            json=payload,
            name="POST /api/v1/tasks",
            catch_response=True,
        ) as response:
            if response.status_code in [200, 201]:
                try:
                    data = response.json()
                    task_id = data.get("id") or data.get("task_id")
                    if task_id:
                        self.created_tasks.append(task_id)
                    response.success()
                except json.JSONDecodeError:
                    response.failure("Invalid JSON response")
            else:
                response.failure(f"Create task failed: {response.status_code}")

    @task(4)
    @tag("tasks", "read")
    def get_task_details(self):
        """Get details of a specific task."""
        # First get a task ID
        list_response = self.client.get(
            "/api/v1/tasks", headers=self.headers, params={"limit": 1}
        )

        if list_response.status_code == 200:
            try:
                data = list_response.json()
                tasks = data.get("tasks") or data.get("data") or data
                if isinstance(tasks, list) and tasks:
                    task_id = tasks[0].get("id") or tasks[0].get("task_id")
                    if task_id:
                        with self.client.get(
                            f"/api/v1/tasks/{task_id}",
                            headers=self.headers,
                            name="GET /api/v1/tasks/{id}",
                            catch_response=True,
                        ) as response:
                            if response.status_code == 200:
                                response.success()
                            else:
                                response.failure(
                                    f"Get task failed: {response.status_code}"
                                )
            except json.JSONDecodeError:
                pass

    @task(6)
    @tag("agents", "read")
    def list_agents(self):
        """List agents endpoint."""
        with self.client.get(
            "/api/v1/agents",
            headers=self.headers,
            name="GET /api/v1/agents",
            catch_response=True,
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"List agents failed: {response.status_code}")

    @task(4)
    @tag("dags", "read")
    def list_dags(self):
        """List DAGs endpoint."""
        with self.client.get(
            "/api/v1/dags",
            headers=self.headers,
            name="GET /api/v1/dags",
            catch_response=True,
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"List DAGs failed: {response.status_code}")

    @task(2)
    @tag("search", "read")
    def search_tasks(self):
        """Search tasks endpoint."""
        payload = {
            "query": random_string(3),
            "limit": 20,
            "offset": 0,
        }

        with self.client.post(
            "/api/v1/tasks/search",
            headers=self.headers,
            json=payload,
            name="POST /api/v1/tasks/search",
            catch_response=True,
        ) as response:
            if response.status_code in [200, 404]:
                response.success()
            else:
                response.failure(f"Search failed: {response.status_code}")

    @task(3)
    @tag("metrics")
    def get_metrics(self):
        """Get metrics endpoint."""
        with self.client.get(
            "/metrics",
            headers=self.headers,
            name="GET /metrics",
            catch_response=True,
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Metrics failed: {response.status_code}")


class ApexDAGUser(HttpUser):
    """
    DAG-focused user simulating workflow operations.
    """

    wait_time = between(2, 5)
    weight = 3  # Less common user type

    def on_start(self):
        """Called when a user starts."""
        self.headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "Locust-DAGTest/1.0",
        }
        if AUTH_TOKEN:
            self.headers["Authorization"] = f"Bearer {AUTH_TOKEN}"

        self.created_dags = []

    @task(5)
    @tag("dags", "read")
    def list_dags(self):
        """List all DAGs."""
        with self.client.get(
            "/api/v1/dags",
            headers=self.headers,
            name="GET /api/v1/dags",
            catch_response=True,
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"List DAGs failed: {response.status_code}")

    @task(3)
    @tag("dags", "write")
    def create_dag(self):
        """Create a new DAG."""
        payload = generate_dag_payload()

        with self.client.post(
            "/api/v1/dags",
            headers=self.headers,
            json=payload,
            name="POST /api/v1/dags",
            catch_response=True,
        ) as response:
            if response.status_code in [200, 201]:
                try:
                    data = response.json()
                    dag_id = data.get("id") or data.get("dag_id")
                    if dag_id:
                        self.created_dags.append(dag_id)
                    response.success()
                except json.JSONDecodeError:
                    response.failure("Invalid JSON response")
            else:
                response.failure(f"Create DAG failed: {response.status_code}")

    @task(2)
    @tag("dags", "execute")
    def execute_dag(self):
        """Execute an existing DAG."""
        if not self.created_dags:
            # Try to get an existing DAG
            list_response = self.client.get(
                "/api/v1/dags", headers=self.headers, params={"limit": 1}
            )
            if list_response.status_code == 200:
                try:
                    data = list_response.json()
                    dags = data.get("dags") or data.get("data") or data
                    if isinstance(dags, list) and dags:
                        dag_id = dags[0].get("id") or dags[0].get("dag_id")
                        if dag_id:
                            self.created_dags.append(dag_id)
                except json.JSONDecodeError:
                    return

        if self.created_dags:
            dag_id = random.choice(self.created_dags)

            with self.client.post(
                f"/api/v1/dags/{dag_id}/execute",
                headers=self.headers,
                json={"async": True},
                name="POST /api/v1/dags/{id}/execute",
                catch_response=True,
            ) as response:
                if response.status_code in [200, 201, 202]:
                    response.success()
                else:
                    response.failure(f"Execute DAG failed: {response.status_code}")

    @task(4)
    @tag("dags", "read")
    def get_dag_status(self):
        """Get DAG execution status."""
        if self.created_dags:
            dag_id = random.choice(self.created_dags)

            with self.client.get(
                f"/api/v1/dags/{dag_id}",
                headers=self.headers,
                name="GET /api/v1/dags/{id}",
                catch_response=True,
            ) as response:
                if response.status_code == 200:
                    response.success()
                elif response.status_code == 404:
                    # DAG might have been deleted
                    self.created_dags.remove(dag_id)
                    response.success()
                else:
                    response.failure(f"Get DAG failed: {response.status_code}")


class ApexHeavyUser(HttpUser):
    """
    Heavy user simulating intensive operations.
    """

    wait_time = between(0.5, 1.5)
    weight = 2  # Uncommon user type

    def on_start(self):
        """Called when a user starts."""
        self.headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "Locust-HeavyTest/1.0",
        }
        if AUTH_TOKEN:
            self.headers["Authorization"] = f"Bearer {AUTH_TOKEN}"

    @task(3)
    @tag("stress", "batch")
    def batch_task_creation(self):
        """Create multiple tasks in rapid succession."""
        for _ in range(5):
            payload = generate_task_payload()

            self.client.post(
                "/api/v1/tasks",
                headers=self.headers,
                json=payload,
                name="POST /api/v1/tasks (batch)",
            )

    @task(4)
    @tag("stress", "concurrent")
    def concurrent_reads(self):
        """Multiple concurrent read operations."""
        # Simulate concurrent requests
        endpoints = [
            "/api/v1/tasks",
            "/api/v1/agents",
            "/api/v1/dags",
            "/health",
            "/metrics",
        ]

        for endpoint in endpoints:
            self.client.get(
                endpoint, headers=self.headers, name=f"GET {endpoint} (concurrent)"
            )

    @task(2)
    @tag("stress", "large")
    def large_list_request(self):
        """Request large result sets."""
        with self.client.get(
            "/api/v1/tasks",
            headers=self.headers,
            params={"limit": 100},
            name="GET /api/v1/tasks (large)",
            catch_response=True,
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Large list failed: {response.status_code}")

    @task(3)
    @tag("stress", "search")
    def complex_search(self):
        """Complex search operations."""
        payloads = [
            {"query": random_string(5), "limit": 50},
            {"query": random_string(3), "limit": 100, "offset": 50},
            {"query": random_string(4), "filters": {"status": "pending"}},
        ]

        for payload in payloads:
            self.client.post(
                "/api/v1/tasks/search",
                headers=self.headers,
                json=payload,
                name="POST /api/v1/tasks/search (complex)",
            )


class ApexStatsUser(HttpUser):
    """
    Stats/monitoring focused user.
    """

    wait_time = between(5, 10)
    weight = 1  # Rare user type

    def on_start(self):
        """Called when a user starts."""
        self.headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "Locust-StatsTest/1.0",
        }
        if AUTH_TOKEN:
            self.headers["Authorization"] = f"Bearer {AUTH_TOKEN}"

    @task(5)
    @tag("stats")
    def get_task_stats(self):
        """Get task statistics."""
        with self.client.get(
            "/api/v1/stats/tasks",
            headers=self.headers,
            name="GET /api/v1/stats/tasks",
            catch_response=True,
        ) as response:
            if response.status_code in [200, 404]:
                response.success()
            else:
                response.failure(f"Task stats failed: {response.status_code}")

    @task(5)
    @tag("stats")
    def get_agent_stats(self):
        """Get agent statistics."""
        with self.client.get(
            "/api/v1/stats/agents",
            headers=self.headers,
            name="GET /api/v1/stats/agents",
            catch_response=True,
        ) as response:
            if response.status_code in [200, 404]:
                response.success()
            else:
                response.failure(f"Agent stats failed: {response.status_code}")

    @task(10)
    @tag("metrics")
    def get_prometheus_metrics(self):
        """Get Prometheus metrics."""
        with self.client.get(
            "/metrics",
            headers=self.headers,
            name="GET /metrics (prometheus)",
            catch_response=True,
        ) as response:
            if response.status_code == 200:
                response.success()
            else:
                response.failure(f"Metrics failed: {response.status_code}")

    @task(3)
    @tag("health")
    def detailed_health_check(self):
        """Detailed health check with component status."""
        with self.client.get(
            "/health/detailed",
            headers=self.headers,
            name="GET /health/detailed",
            catch_response=True,
        ) as response:
            if response.status_code in [200, 404]:
                response.success()
            else:
                response.failure(f"Detailed health failed: {response.status_code}")
