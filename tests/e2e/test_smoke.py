"""End-to-end smoke tests for the Apex platform.

These tests verify that the core API endpoints are operational and that
a basic task workflow can be completed. They are designed to run against
a live (or Docker Compose) environment as a quick sanity check.

Usage:
    pytest tests/e2e/test_smoke.py -v
    TEST_API_URL=http://localhost:8081 pytest tests/e2e/test_smoke.py -v
"""

from __future__ import annotations

import asyncio
import os
from typing import Any

import httpx
import pytest


# ============================================================================
# Configuration
# ============================================================================

API_URL = os.environ.get("TEST_API_URL", "http://localhost:8081")
API_KEY = os.environ.get("TEST_API_KEY", "test-api-key")

HEADERS = {
    "Content-Type": "application/json",
    "Accept": "application/json",
    "X-API-Key": API_KEY,
}


# ============================================================================
# Fixtures
# ============================================================================


@pytest.fixture(scope="module")
def base_url() -> str:
    """Return the base API URL."""
    return API_URL


@pytest.fixture(scope="module")
def http_client(base_url: str):
    """Provide an httpx client for the test module."""
    with httpx.Client(
        base_url=base_url,
        headers=HEADERS,
        timeout=30.0,
    ) as client:
        yield client


@pytest.fixture(scope="module")
def async_http_client(base_url: str):
    """Provide an async httpx client for the test module."""
    client = httpx.AsyncClient(
        base_url=base_url,
        headers=HEADERS,
        timeout=30.0,
    )
    yield client
    # Close synchronously-safe in module teardown
    asyncio.get_event_loop().run_until_complete(client.aclose())


# ============================================================================
# Health & Readiness Smoke Tests
# ============================================================================


class TestHealthSmoke:
    """Verify the platform is up and healthy."""

    def test_health_endpoint_responds(self, http_client: httpx.Client) -> None:
        """The /health endpoint should return 200 with status info."""
        response = http_client.get("/health")

        assert response.status_code == 200
        body = response.json()
        assert "status" in body
        assert body["status"] in ("healthy", "degraded", "unhealthy")

    def test_health_contains_version(self, http_client: httpx.Client) -> None:
        """The health response should include a version string."""
        response = http_client.get("/health")

        assert response.status_code == 200
        body = response.json()
        assert "version" in body
        assert isinstance(body["version"], str)
        assert len(body["version"]) > 0

    def test_metrics_endpoint_responds(self, http_client: httpx.Client) -> None:
        """The /metrics endpoint should be accessible."""
        response = http_client.get("/metrics")

        # Metrics may return 200 or could be behind auth
        assert response.status_code in (200, 401, 403, 404)


# ============================================================================
# Task API Smoke Tests
# ============================================================================


class TestTaskSmoke:
    """Verify core task CRUD operations work end-to-end."""

    def test_list_tasks(self, http_client: httpx.Client) -> None:
        """GET /api/v1/tasks should return a list response."""
        response = http_client.get("/api/v1/tasks")

        assert response.status_code == 200
        body = response.json()
        # Should have pagination structure
        assert "items" in body or "tasks" in body or isinstance(body, list)

    def test_create_and_retrieve_task(self, http_client: httpx.Client) -> None:
        """Create a task, then retrieve it by ID."""
        # Create
        create_payload = {
            "name": "smoke-test-task",
            "description": "Created by e2e smoke test",
            "priority": "normal",
            "tags": ["smoke-test", "e2e"],
        }
        create_response = http_client.post("/api/v1/tasks", json=create_payload)

        assert create_response.status_code in (200, 201)
        task = create_response.json()
        task_id = task.get("id") or task.get("task_id")
        assert task_id is not None

        try:
            # Retrieve
            get_response = http_client.get(f"/api/v1/tasks/{task_id}")

            assert get_response.status_code == 200
            retrieved = get_response.json()
            assert retrieved.get("name") == "smoke-test-task"
            assert retrieved.get("description") == "Created by e2e smoke test"
        finally:
            # Cleanup
            http_client.delete(f"/api/v1/tasks/{task_id}")

    def test_create_update_delete_task(self, http_client: httpx.Client) -> None:
        """Full task lifecycle: create, update, delete."""
        # Create
        create_response = http_client.post(
            "/api/v1/tasks",
            json={"name": "lifecycle-smoke-task", "tags": ["smoke-test"]},
        )
        assert create_response.status_code in (200, 201)
        task = create_response.json()
        task_id = task.get("id") or task.get("task_id")
        assert task_id is not None

        try:
            # Update
            update_response = http_client.patch(
                f"/api/v1/tasks/{task_id}",
                json={"description": "Updated by smoke test"},
            )
            assert update_response.status_code == 200
            updated = update_response.json()
            assert updated.get("description") == "Updated by smoke test"

            # Delete
            delete_response = http_client.delete(f"/api/v1/tasks/{task_id}")
            assert delete_response.status_code in (200, 204)

            # Verify deleted
            verify_response = http_client.get(f"/api/v1/tasks/{task_id}")
            assert verify_response.status_code == 404
        except Exception:
            # Best-effort cleanup
            http_client.delete(f"/api/v1/tasks/{task_id}")
            raise

    def test_task_not_found_returns_404(self, http_client: httpx.Client) -> None:
        """Requesting a nonexistent task should return 404."""
        response = http_client.get("/api/v1/tasks/nonexistent-smoke-test-id")

        assert response.status_code == 404

    def test_task_validation_rejects_empty_name(
        self, http_client: httpx.Client
    ) -> None:
        """Creating a task with empty name should be rejected."""
        response = http_client.post("/api/v1/tasks", json={"name": ""})

        assert response.status_code == 422


# ============================================================================
# Agent API Smoke Tests
# ============================================================================


class TestAgentSmoke:
    """Verify core agent operations work end-to-end."""

    def test_list_agents(self, http_client: httpx.Client) -> None:
        """GET /api/v1/agents should return a list response."""
        response = http_client.get("/api/v1/agents")

        assert response.status_code == 200
        body = response.json()
        assert "items" in body or "agents" in body or isinstance(body, list)

    def test_create_and_delete_agent(self, http_client: httpx.Client) -> None:
        """Create an agent, verify it exists, then delete it."""
        # Create
        create_response = http_client.post(
            "/api/v1/agents",
            json={
                "name": "smoke-test-agent",
                "description": "Created by e2e smoke test",
                "max_concurrent_tasks": 1,
            },
        )
        assert create_response.status_code in (200, 201)
        agent = create_response.json()
        agent_id = agent.get("id") or agent.get("agent_id")
        assert agent_id is not None

        try:
            # Retrieve
            get_response = http_client.get(f"/api/v1/agents/{agent_id}")
            assert get_response.status_code == 200
            assert get_response.json().get("name") == "smoke-test-agent"
        finally:
            # Cleanup
            http_client.delete(f"/api/v1/agents/{agent_id}")


# ============================================================================
# DAG API Smoke Tests
# ============================================================================


class TestDAGSmoke:
    """Verify core DAG operations work end-to-end."""

    def test_list_dags(self, http_client: httpx.Client) -> None:
        """GET /api/v1/dags should return a list response."""
        response = http_client.get("/api/v1/dags")

        assert response.status_code == 200
        body = response.json()
        assert "items" in body or "dags" in body or isinstance(body, list)

    def test_create_and_start_dag(self, http_client: httpx.Client) -> None:
        """Create a simple DAG and start its execution."""
        # Create DAG
        dag_payload = {
            "name": "smoke-test-dag",
            "description": "DAG created by e2e smoke test",
            "nodes": [
                {
                    "id": "step-1",
                    "taskTemplate": {"name": "smoke-dag-task-1"},
                    "dependsOn": [],
                },
                {
                    "id": "step-2",
                    "taskTemplate": {"name": "smoke-dag-task-2"},
                    "dependsOn": ["step-1"],
                },
            ],
        }
        create_response = http_client.post("/api/v1/dags", json=dag_payload)
        assert create_response.status_code in (200, 201)
        dag = create_response.json()
        dag_id = dag.get("id") or dag.get("dag_id")
        assert dag_id is not None

        try:
            # Start execution
            start_response = http_client.post(f"/api/v1/dags/{dag_id}/execute")
            assert start_response.status_code in (200, 201, 202)

            # Verify DAG status changed
            get_response = http_client.get(f"/api/v1/dags/{dag_id}")
            assert get_response.status_code == 200
            status = get_response.json().get("status")
            assert status in ("pending", "running", "completed", "failed")
        finally:
            # Cleanup
            http_client.delete(f"/api/v1/dags/{dag_id}")


# ============================================================================
# Cross-Resource Smoke Tests
# ============================================================================


class TestCrossResourceSmoke:
    """Verify cross-resource interactions work end-to-end."""

    def test_assign_task_to_agent(self, http_client: httpx.Client) -> None:
        """Create an agent, assign a task to it, verify the relationship."""
        # Create agent
        agent_response = http_client.post(
            "/api/v1/agents",
            json={"name": "assignment-smoke-agent", "max_concurrent_tasks": 5},
        )
        assert agent_response.status_code in (200, 201)
        agent = agent_response.json()
        agent_id = agent.get("id") or agent.get("agent_id")

        try:
            # Create task assigned to agent
            task_response = http_client.post(
                "/api/v1/tasks",
                json={
                    "name": "assigned-smoke-task",
                    "agentId": agent_id,
                    "agent_id": agent_id,  # Support both naming conventions
                },
            )
            assert task_response.status_code in (200, 201)
            task = task_response.json()
            task_id = task.get("id") or task.get("task_id")

            # Verify assignment
            assigned_agent = task.get("agent_id") or task.get("agentId")
            assert assigned_agent == agent_id

            # Cleanup task
            http_client.delete(f"/api/v1/tasks/{task_id}")
        finally:
            # Cleanup agent
            http_client.delete(f"/api/v1/agents/{agent_id}")


# ============================================================================
# Authentication Smoke Tests
# ============================================================================


class TestAuthSmoke:
    """Verify authentication behavior."""

    def test_invalid_api_key_rejected(self, base_url: str) -> None:
        """Requests with invalid API key should be rejected."""
        with httpx.Client(
            base_url=base_url,
            headers={
                "Content-Type": "application/json",
                "X-API-Key": "invalid-smoke-test-key",
            },
            timeout=10.0,
        ) as client:
            response = client.get("/api/v1/tasks")
            # Should be 401 or 403
            assert response.status_code in (401, 403)

    def test_missing_api_key_rejected(self, base_url: str) -> None:
        """Requests without any API key should be rejected."""
        with httpx.Client(
            base_url=base_url,
            headers={"Content-Type": "application/json"},
            timeout=10.0,
        ) as client:
            response = client.get("/api/v1/tasks")
            # Should be 401 or 403
            assert response.status_code in (401, 403)
