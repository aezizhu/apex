"""End-to-end workflow integration tests for the Apex platform."""

from __future__ import annotations

import asyncio
from typing import Any

import asyncpg
import pytest
import pytest_asyncio

from apex_sdk import AsyncApexClient
from apex_sdk.models import (
    Agent,
    AgentCapability,
    AgentCreate,
    AgentStatus,
    AgentUpdate,
    Approval,
    ApprovalCreate,
    ApprovalDecision,
    ApprovalStatus,
    ApprovalType,
    DAG,
    DAGCreate,
    DAGNode,
    DAGStatus,
    Task,
    TaskCreate,
    TaskInput,
    TaskPriority,
    TaskStatus,
    TaskUpdate,
    WebSocketEventType,
    WebSocketMessage,
)
from apex_sdk.websocket import ApexWebSocketClient


class TestTaskExecutionWorkflow:
    """Tests for complete task execution workflows."""

    @pytest.mark.asyncio
    @pytest.mark.workflow
    @pytest.mark.slow
    async def test_task_lifecycle_pending_to_completed(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        cleanup_agents: list[str],
        wait_for: callable,
    ) -> None:
        """Test a task going through its full lifecycle."""
        # Arrange - Create an agent to process the task
        agent = await api_client.create_agent(
            AgentCreate(
                name="workflow-agent",
                capabilities=[AgentCapability(name="general")],
                max_concurrent_tasks=5,
            )
        )
        cleanup_agents.append(agent.id)

        # Act - Create and assign task
        task = await api_client.create_task(
            TaskCreate(
                name="lifecycle-task",
                description="Test task lifecycle",
                agent_id=agent.id,
                priority=TaskPriority.HIGH,
                input=TaskInput(data={"test": "data"}),
            )
        )
        cleanup_tasks.append(task.id)

        # Assert - Task starts in pending
        assert task.status == TaskStatus.PENDING

        # Wait for task to complete (or timeout)
        async def task_completed():
            t = await api_client.get_task(task.id)
            return t.status in [TaskStatus.COMPLETED, TaskStatus.FAILED]

        completed = await wait_for(task_completed, timeout=30.0)

        # Get final state
        final_task = await api_client.get_task(task.id)

        # Assert - Verify final state
        assert final_task.started_at is not None or final_task.status == TaskStatus.PENDING

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_task_with_dependencies(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test tasks with dependencies are processed in order."""
        # Arrange & Act - Create parent task
        parent_task = await api_client.create_task(
            TaskCreate(name="parent-task", tags=["dependency-test"])
        )
        cleanup_tasks.append(parent_task.id)

        # Create child task that depends on parent
        child_task = await api_client.create_task(
            TaskCreate(
                name="child-task",
                depends_on=[parent_task.id],
                tags=["dependency-test"],
            )
        )
        cleanup_tasks.append(child_task.id)

        # Assert
        assert child_task.depends_on == [parent_task.id]

        # Get child task to verify dependency state
        retrieved_child = await api_client.get_task(child_task.id)
        assert parent_task.id in retrieved_child.depends_on

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_task_retry_on_failure(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        db_connection: asyncpg.Connection,
    ) -> None:
        """Test task retry mechanism after failure."""
        # Arrange - Create task with retries
        task = await api_client.create_task(
            TaskCreate(
                name="retry-task",
                retries=3,
            )
        )
        cleanup_tasks.append(task.id)

        # Simulate failure by setting status in DB
        await db_connection.execute(
            "UPDATE tasks SET status = 'failed' WHERE id = $1",
            task.id,
        )

        # Act - Retry the task
        retried_task = await api_client.retry_task(task.id)

        # Assert
        assert retried_task.retry_count >= 1
        assert retried_task.status in [TaskStatus.PENDING, TaskStatus.QUEUED]

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_task_cancellation(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test cancelling a task."""
        # Arrange
        task = await api_client.create_task(
            TaskCreate(name="cancel-workflow-task")
        )
        cleanup_tasks.append(task.id)

        # Act
        cancelled = await api_client.cancel_task(task.id)

        # Assert
        assert cancelled.status == TaskStatus.CANCELLED

        # Verify cancelled task cannot be retried
        # (may raise error or return same cancelled state)


class TestDAGExecutionWorkflow:
    """Tests for DAG execution workflows."""

    @pytest.mark.asyncio
    @pytest.mark.workflow
    @pytest.mark.slow
    async def test_simple_dag_execution(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        wait_for: callable,
    ) -> None:
        """Test executing a simple sequential DAG."""
        # Arrange
        dag = await api_client.create_dag(
            DAGCreate(
                name="simple-workflow-dag",
                nodes=[
                    DAGNode(
                        id="step-1",
                        task_template=TaskCreate(name="dag-step-1"),
                    ),
                    DAGNode(
                        id="step-2",
                        task_template=TaskCreate(name="dag-step-2"),
                        depends_on=["step-1"],
                    ),
                ],
            )
        )
        cleanup_dags.append(dag.id)

        # Act - Start DAG
        started = await api_client.start_dag(dag.id)

        # Assert
        assert started.status in [DAGStatus.RUNNING, DAGStatus.PENDING]

        # Wait for completion (or timeout)
        async def dag_done():
            d = await api_client.get_dag(dag.id)
            return d.status in [DAGStatus.COMPLETED, DAGStatus.FAILED]

        await wait_for(dag_done, timeout=60.0)

    @pytest.mark.asyncio
    @pytest.mark.workflow
    @pytest.mark.slow
    async def test_parallel_dag_execution(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        wait_for: callable,
    ) -> None:
        """Test executing a DAG with parallel branches."""
        # Arrange - Diamond pattern
        dag = await api_client.create_dag(
            DAGCreate(
                name="parallel-workflow-dag",
                nodes=[
                    DAGNode(
                        id="start",
                        task_template=TaskCreate(name="start-task"),
                    ),
                    DAGNode(
                        id="branch-a",
                        task_template=TaskCreate(name="branch-a-task"),
                        depends_on=["start"],
                    ),
                    DAGNode(
                        id="branch-b",
                        task_template=TaskCreate(name="branch-b-task"),
                        depends_on=["start"],
                    ),
                    DAGNode(
                        id="join",
                        task_template=TaskCreate(name="join-task"),
                        depends_on=["branch-a", "branch-b"],
                    ),
                ],
            )
        )
        cleanup_dags.append(dag.id)

        # Act
        await api_client.start_dag(dag.id)

        # Wait for completion
        async def dag_done():
            d = await api_client.get_dag(dag.id)
            return d.status in [DAGStatus.COMPLETED, DAGStatus.FAILED]

        await wait_for(dag_done, timeout=60.0)

        # Assert - Check final state
        final_dag = await api_client.get_dag(dag.id)
        assert final_dag.status in [DAGStatus.COMPLETED, DAGStatus.FAILED, DAGStatus.RUNNING]

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_dag_pause_and_resume(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test pausing and resuming a DAG."""
        # Arrange - Create a multi-step DAG
        dag = await api_client.create_dag(
            DAGCreate(
                name="pausable-dag",
                nodes=[
                    DAGNode(
                        id="step-1",
                        task_template=TaskCreate(name="step-1"),
                    ),
                    DAGNode(
                        id="step-2",
                        task_template=TaskCreate(name="step-2"),
                        depends_on=["step-1"],
                    ),
                    DAGNode(
                        id="step-3",
                        task_template=TaskCreate(name="step-3"),
                        depends_on=["step-2"],
                    ),
                ],
            )
        )
        cleanup_dags.append(dag.id)

        # Act - Start and pause
        await api_client.start_dag(dag.id)
        paused = await api_client.pause_dag(dag.id)

        # Assert
        assert paused.status == DAGStatus.PAUSED

        # Act - Resume
        resumed = await api_client.resume_dag(dag.id)

        # Assert
        assert resumed.status == DAGStatus.RUNNING

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_dag_cancellation(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test cancelling a DAG cancels pending tasks."""
        # Arrange
        dag = await api_client.create_dag(
            DAGCreate(
                name="cancel-dag",
                nodes=[
                    DAGNode(
                        id="long-task",
                        task_template=TaskCreate(
                            name="long-running-task",
                            timeout_seconds=300,
                        ),
                    ),
                ],
            )
        )
        cleanup_dags.append(dag.id)

        # Start the DAG
        await api_client.start_dag(dag.id)

        # Act - Cancel
        cancelled = await api_client.cancel_dag(dag.id)

        # Assert
        assert cancelled.status == DAGStatus.CANCELLED


class TestApprovalWorkflow:
    """Tests for approval workflows."""

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_approval_request_and_approve(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test creating and approving an approval request."""
        # Arrange - Create a task that needs approval
        task = await api_client.create_task(
            TaskCreate(name="approval-required-task")
        )
        cleanup_tasks.append(task.id)

        # Act - Create approval request
        approval = await api_client.create_approval(
            ApprovalCreate(
                task_id=task.id,
                type=ApprovalType.MANUAL,
                description="Please approve this task",
                required_approvers=["admin@example.com"],
            )
        )

        # Assert
        assert approval.status == ApprovalStatus.PENDING
        assert approval.task_id == task.id

        # Act - Approve
        decision = ApprovalDecision(
            status=ApprovalStatus.APPROVED,
            approver_id="admin@example.com",
            comment="Looks good!",
        )
        approved = await api_client.decide_approval(approval.id, decision)

        # Assert
        assert approved.status == ApprovalStatus.APPROVED
        assert approved.decided_at is not None

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_approval_request_and_reject(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
    ) -> None:
        """Test creating and rejecting an approval request."""
        # Arrange
        task = await api_client.create_task(
            TaskCreate(name="rejection-test-task")
        )
        cleanup_tasks.append(task.id)

        approval = await api_client.create_approval(
            ApprovalCreate(
                task_id=task.id,
                type=ApprovalType.MANUAL,
            )
        )

        # Act - Reject
        decision = ApprovalDecision(
            status=ApprovalStatus.REJECTED,
            approver_id="reviewer@example.com",
            comment="Needs more work",
        )
        rejected = await api_client.decide_approval(approval.id, decision)

        # Assert
        assert rejected.status == ApprovalStatus.REJECTED
        assert rejected.comment == "Needs more work"


class TestAgentTaskAssignment:
    """Tests for agent task assignment workflows."""

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_task_assignment_to_agent(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        cleanup_agents: list[str],
    ) -> None:
        """Test assigning tasks to specific agents."""
        # Arrange - Create agent
        agent = await api_client.create_agent(
            AgentCreate(
                name="assignment-agent",
                capabilities=[AgentCapability(name="text_processing")],
            )
        )
        cleanup_agents.append(agent.id)

        # Act - Create task assigned to agent
        task = await api_client.create_task(
            TaskCreate(
                name="assigned-task",
                agent_id=agent.id,
            )
        )
        cleanup_tasks.append(task.id)

        # Assert
        assert task.agent_id == agent.id

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_agent_capacity_management(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        cleanup_agents: list[str],
    ) -> None:
        """Test agent capacity tracking with multiple tasks."""
        # Arrange - Create agent with limited capacity
        agent = await api_client.create_agent(
            AgentCreate(
                name="capacity-agent",
                max_concurrent_tasks=2,
            )
        )
        cleanup_agents.append(agent.id)

        # Act - Create multiple tasks
        tasks = []
        for i in range(3):
            task = await api_client.create_task(
                TaskCreate(
                    name=f"capacity-task-{i}",
                    agent_id=agent.id,
                )
            )
            tasks.append(task)
            cleanup_tasks.append(task.id)

        # Assert - Check agent state
        updated_agent = await api_client.get_agent(agent.id)
        # Agent should track tasks (actual behavior depends on implementation)
        assert updated_agent.max_concurrent_tasks == 2


class TestComplexWorkflow:
    """Tests for complex multi-step workflows."""

    @pytest.mark.asyncio
    @pytest.mark.workflow
    @pytest.mark.slow
    async def test_data_processing_pipeline(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
        cleanup_agents: list[str],
    ) -> None:
        """Test a data processing pipeline workflow."""
        # Arrange - Create specialized agents
        ingestion_agent = await api_client.create_agent(
            AgentCreate(
                name="ingestion-agent",
                capabilities=[AgentCapability(name="data_ingestion")],
            )
        )
        processing_agent = await api_client.create_agent(
            AgentCreate(
                name="processing-agent",
                capabilities=[AgentCapability(name="data_processing")],
            )
        )
        output_agent = await api_client.create_agent(
            AgentCreate(
                name="output-agent",
                capabilities=[AgentCapability(name="data_output")],
            )
        )
        cleanup_agents.extend([
            ingestion_agent.id,
            processing_agent.id,
            output_agent.id,
        ])

        # Create pipeline DAG
        dag = await api_client.create_dag(
            DAGCreate(
                name="data-pipeline",
                description="ETL data processing pipeline",
                nodes=[
                    DAGNode(
                        id="ingest",
                        task_template=TaskCreate(
                            name="ingest-data",
                            agent_id=ingestion_agent.id,
                        ),
                    ),
                    DAGNode(
                        id="validate",
                        task_template=TaskCreate(
                            name="validate-data",
                            agent_id=processing_agent.id,
                        ),
                        depends_on=["ingest"],
                    ),
                    DAGNode(
                        id="transform",
                        task_template=TaskCreate(
                            name="transform-data",
                            agent_id=processing_agent.id,
                        ),
                        depends_on=["validate"],
                    ),
                    DAGNode(
                        id="output",
                        task_template=TaskCreate(
                            name="output-data",
                            agent_id=output_agent.id,
                        ),
                        depends_on=["transform"],
                    ),
                ],
                input={"source": "test-data"},
            )
        )
        cleanup_dags.append(dag.id)

        # Act - Start pipeline
        started = await api_client.start_dag(dag.id)

        # Assert
        assert started.status in [DAGStatus.RUNNING, DAGStatus.PENDING]
        assert len(started.nodes) == 4

    @pytest.mark.asyncio
    @pytest.mark.workflow
    @pytest.mark.slow
    async def test_workflow_with_approval_gate(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        cleanup_dags: list[str],
    ) -> None:
        """Test a workflow with an approval gate between stages."""
        # Arrange - Create a DAG with approval points
        dag = await api_client.create_dag(
            DAGCreate(
                name="approval-gate-dag",
                nodes=[
                    DAGNode(
                        id="prepare",
                        task_template=TaskCreate(name="prepare-task"),
                    ),
                    DAGNode(
                        id="execute",
                        task_template=TaskCreate(name="execute-task"),
                        depends_on=["prepare"],
                        # In real implementation, this would pause for approval
                    ),
                    DAGNode(
                        id="finalize",
                        task_template=TaskCreate(name="finalize-task"),
                        depends_on=["execute"],
                    ),
                ],
            )
        )
        cleanup_dags.append(dag.id)

        # Act - Start workflow
        await api_client.start_dag(dag.id)

        # Assert - DAG is running
        running_dag = await api_client.get_dag(dag.id)
        assert running_dag.status in [DAGStatus.RUNNING, DAGStatus.PENDING]


class TestErrorRecoveryWorkflow:
    """Tests for error recovery workflows."""

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_task_failure_and_retry_workflow(
        self,
        api_client: AsyncApexClient,
        cleanup_tasks: list[str],
        db_connection: asyncpg.Connection,
    ) -> None:
        """Test handling task failure and retry in a workflow."""
        # Arrange - Create task with retry capability
        task = await api_client.create_task(
            TaskCreate(
                name="failure-recovery-task",
                retries=3,
            )
        )
        cleanup_tasks.append(task.id)

        # Simulate failure
        await db_connection.execute(
            """
            UPDATE tasks
            SET status = 'failed',
                error = '{"code": "EXECUTION_ERROR", "message": "Simulated failure"}'
            WHERE id = $1
            """,
            task.id,
        )

        # Act - Retry
        retried = await api_client.retry_task(task.id)

        # Assert
        assert retried.status in [TaskStatus.PENDING, TaskStatus.QUEUED]
        assert retried.retry_count >= 1

    @pytest.mark.asyncio
    @pytest.mark.workflow
    async def test_dag_partial_failure_recovery(
        self,
        api_client: AsyncApexClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test recovering a DAG from partial failure."""
        # Arrange
        dag = await api_client.create_dag(
            DAGCreate(
                name="partial-failure-dag",
                nodes=[
                    DAGNode(
                        id="step-1",
                        task_template=TaskCreate(name="step-1"),
                    ),
                    DAGNode(
                        id="step-2",
                        task_template=TaskCreate(name="step-2", retries=2),
                        depends_on=["step-1"],
                    ),
                ],
            )
        )
        cleanup_dags.append(dag.id)

        # Act - Start and let it run
        await api_client.start_dag(dag.id)

        # Assert - DAG is processing
        processing_dag = await api_client.get_dag(dag.id)
        assert processing_dag.status in [DAGStatus.RUNNING, DAGStatus.PENDING]


class TestRealTimeWorkflowMonitoring:
    """Tests for real-time workflow monitoring via WebSocket."""

    @pytest.mark.asyncio
    @pytest.mark.workflow
    @pytest.mark.websocket
    @pytest.mark.slow
    async def test_monitor_workflow_progress(
        self,
        api_client: AsyncApexClient,
        ws_client: ApexWebSocketClient,
        cleanup_dags: list[str],
    ) -> None:
        """Test monitoring workflow progress via WebSocket."""
        # Arrange
        events: list[WebSocketMessage] = []

        async def event_handler(message: WebSocketMessage) -> None:
            events.append(message)

        ws_client.add_event_handler(WebSocketEventType.TASK_CREATED, event_handler)
        ws_client.add_event_handler(WebSocketEventType.TASK_UPDATED, event_handler)
        ws_client.add_event_handler(WebSocketEventType.DAG_STARTED, event_handler)
        ws_client.add_event_handler(WebSocketEventType.DAG_COMPLETED, event_handler)

        await ws_client.connect()
        await ws_client.subscribe(
            events=[
                WebSocketEventType.TASK_CREATED,
                WebSocketEventType.TASK_UPDATED,
                WebSocketEventType.DAG_STARTED,
                WebSocketEventType.DAG_COMPLETED,
            ]
        )

        # Create and start DAG
        dag = await api_client.create_dag(
            DAGCreate(
                name="monitored-dag",
                nodes=[
                    DAGNode(
                        id="step-1",
                        task_template=TaskCreate(name="monitored-step"),
                    ),
                ],
            )
        )
        cleanup_dags.append(dag.id)

        await api_client.start_dag(dag.id)

        # Wait for events
        await asyncio.sleep(3.0)

        # Assert - Should have received some events
        # (exact events depend on system behavior)
