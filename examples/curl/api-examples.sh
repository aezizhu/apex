#!/bin/bash
#
# Apex REST API - curl Examples
#
# This script demonstrates common API operations using curl.
# Each example shows the HTTP method, endpoint, and expected response.
#
# Prerequisites:
#   - curl installed
#   - jq installed (optional, for JSON formatting)
#   - Apex server running at http://localhost:8080
#
# Usage:
#   chmod +x api-examples.sh
#   ./api-examples.sh
#
# Or run individual examples:
#   source api-examples.sh
#   health_check
#   create_task

set -e

# =============================================================================
# Configuration
# =============================================================================

# API Base URL - change this if your server is running elsewhere
BASE_URL="${APEX_API_URL:-http://localhost:8080}"

# API Key for authentication (optional if auth is disabled)
API_KEY="${APEX_API_KEY:-}"

# Common headers
CONTENT_TYPE="Content-Type: application/json"
ACCEPT="Accept: application/json"

# Build authorization header if API key is set
if [ -n "$API_KEY" ]; then
    AUTH_HEADER="Authorization: Bearer $API_KEY"
else
    AUTH_HEADER=""
fi

# Helper function to make requests
# Usage: api_request METHOD ENDPOINT [DATA]
api_request() {
    local method="$1"
    local endpoint="$2"
    local data="$3"

    local url="${BASE_URL}${endpoint}"

    echo ">>> $method $endpoint"
    echo ""

    if [ -n "$data" ]; then
        if [ -n "$AUTH_HEADER" ]; then
            curl -s -X "$method" "$url" \
                -H "$CONTENT_TYPE" \
                -H "$ACCEPT" \
                -H "$AUTH_HEADER" \
                -d "$data" | jq . 2>/dev/null || cat
        else
            curl -s -X "$method" "$url" \
                -H "$CONTENT_TYPE" \
                -H "$ACCEPT" \
                -d "$data" | jq . 2>/dev/null || cat
        fi
    else
        if [ -n "$AUTH_HEADER" ]; then
            curl -s -X "$method" "$url" \
                -H "$CONTENT_TYPE" \
                -H "$ACCEPT" \
                -H "$AUTH_HEADER" | jq . 2>/dev/null || cat
        else
            curl -s -X "$method" "$url" \
                -H "$CONTENT_TYPE" \
                -H "$ACCEPT" | jq . 2>/dev/null || cat
        fi
    fi

    echo ""
    echo ""
}

# =============================================================================
# Health Check
# =============================================================================

health_check() {
    echo "============================================================"
    echo "Health Check"
    echo "============================================================"
    echo "Check the API server health status."
    echo ""

    api_request GET "/health"
}

# =============================================================================
# Task API
# =============================================================================

create_task() {
    echo "============================================================"
    echo "Create Task"
    echo "============================================================"
    echo "Create a new task with name, description, and input data."
    echo ""

    api_request POST "/api/v1/tasks" '{
        "name": "Research AI Trends",
        "description": "Research and summarize the latest trends in AI agent architectures",
        "priority": "normal",
        "input": {
            "data": {
                "topic": "AI agent architectures",
                "depth": "comprehensive",
                "format": "markdown"
            }
        },
        "timeoutSeconds": 120,
        "retries": 2,
        "tags": ["research", "ai"],
        "metadata": {
            "project": "research-initiative",
            "requestedBy": "user-123"
        }
    }'
}

list_tasks() {
    echo "============================================================"
    echo "List Tasks"
    echo "============================================================"
    echo "Get a paginated list of tasks with optional filters."
    echo ""

    # List all tasks
    echo "--- All tasks (page 1, 10 items) ---"
    api_request GET "/api/v1/tasks?page=1&limit=10"

    # List pending tasks
    echo "--- Pending tasks ---"
    api_request GET "/api/v1/tasks?status=pending"

    # List with multiple filters
    echo "--- High priority tasks ---"
    api_request GET "/api/v1/tasks?priority=high&priority=critical"
}

get_task() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Get Task"
    echo "============================================================"
    echo "Get details for a specific task by ID."
    echo ""

    api_request GET "/api/v1/tasks/$task_id"
}

update_task() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Update Task"
    echo "============================================================"
    echo "Update task properties (priority, metadata, etc.)."
    echo ""

    api_request PATCH "/api/v1/tasks/$task_id" '{
        "priority": "high",
        "metadata": {
            "escalated": true,
            "reason": "urgent deadline"
        }
    }'
}

cancel_task() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Cancel Task"
    echo "============================================================"
    echo "Cancel a running task."
    echo ""

    api_request POST "/api/v1/tasks/$task_id/cancel"
}

retry_task() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Retry Task"
    echo "============================================================"
    echo "Retry a failed task."
    echo ""

    api_request POST "/api/v1/tasks/$task_id/retry"
}

pause_task() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Pause Task"
    echo "============================================================"
    echo "Pause a running task."
    echo ""

    api_request POST "/api/v1/tasks/$task_id/pause"
}

resume_task() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Resume Task"
    echo "============================================================"
    echo "Resume a paused task."
    echo ""

    api_request POST "/api/v1/tasks/$task_id/resume"
}

delete_task() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Delete Task"
    echo "============================================================"
    echo "Delete a task by ID."
    echo ""

    api_request DELETE "/api/v1/tasks/$task_id"
}

get_task_logs() {
    local task_id="${1:-550e8400-e29b-41d4-a716-446655440000}"

    echo "============================================================"
    echo "Get Task Logs"
    echo "============================================================"
    echo "Get logs for a specific task."
    echo ""

    api_request GET "/api/v1/tasks/$task_id/logs?limit=50"
}

# =============================================================================
# Agent API
# =============================================================================

create_agent() {
    echo "============================================================"
    echo "Create Agent"
    echo "============================================================"
    echo "Create a new agent with capabilities."
    echo ""

    api_request POST "/api/v1/agents" '{
        "name": "research-agent-01",
        "description": "Specialized agent for research tasks",
        "capabilities": ["web-search", "summarization", "analysis"],
        "metadata": {
            "model": "gpt-4-turbo",
            "region": "us-west-2"
        }
    }'
}

list_agents() {
    echo "============================================================"
    echo "List Agents"
    echo "============================================================"
    echo "Get a paginated list of agents."
    echo ""

    # List all agents
    echo "--- All agents ---"
    api_request GET "/api/v1/agents?page=1&limit=10"

    # List idle agents
    echo "--- Idle agents ---"
    api_request GET "/api/v1/agents?status=idle"

    # List agents with specific capability
    echo "--- Agents with web-search capability ---"
    api_request GET "/api/v1/agents?capabilities=web-search"
}

get_agent() {
    local agent_id="${1:-agent-123}"

    echo "============================================================"
    echo "Get Agent"
    echo "============================================================"
    echo "Get details for a specific agent."
    echo ""

    api_request GET "/api/v1/agents/$agent_id"
}

update_agent() {
    local agent_id="${1:-agent-123}"

    echo "============================================================"
    echo "Update Agent"
    echo "============================================================"
    echo "Update agent properties."
    echo ""

    api_request PATCH "/api/v1/agents/$agent_id" '{
        "capabilities": ["web-search", "summarization", "analysis", "code-review"],
        "metadata": {
            "updated": true
        }
    }'
}

delete_agent() {
    local agent_id="${1:-agent-123}"

    echo "============================================================"
    echo "Delete Agent"
    echo "============================================================"
    echo "Delete an agent by ID."
    echo ""

    api_request DELETE "/api/v1/agents/$agent_id"
}

assign_task_to_agent() {
    local agent_id="${1:-agent-123}"
    local task_id="${2:-task-456}"

    echo "============================================================"
    echo "Assign Task to Agent"
    echo "============================================================"
    echo "Assign a task to a specific agent."
    echo ""

    api_request POST "/api/v1/agents/$agent_id/assign" "{
        \"taskId\": \"$task_id\"
    }"
}

# =============================================================================
# DAG API
# =============================================================================

create_dag() {
    echo "============================================================"
    echo "Create DAG"
    echo "============================================================"
    echo "Create a new DAG with nodes and edges."
    echo ""

    api_request POST "/api/v1/dags" '{
        "name": "Research Pipeline",
        "description": "A sequential workflow for research, analysis, and reporting",
        "nodes": [
            {
                "id": "research",
                "name": "Research Phase",
                "type": "task",
                "config": {
                    "taskTemplate": {
                        "name": "Research AI Trends",
                        "description": "Gather information about current AI trends",
                        "priority": "normal",
                        "input": {
                            "topic": "AI agent architectures"
                        }
                    },
                    "timeout": 300,
                    "retries": 2
                }
            },
            {
                "id": "analyze",
                "name": "Analysis Phase",
                "type": "task",
                "config": {
                    "taskTemplate": {
                        "name": "Analyze Research",
                        "description": "Analyze the gathered data"
                    }
                }
            },
            {
                "id": "report",
                "name": "Report Generation",
                "type": "task",
                "config": {
                    "taskTemplate": {
                        "name": "Generate Report",
                        "description": "Create final report",
                        "priority": "high"
                    }
                }
            }
        ],
        "edges": [
            {"sourceNodeId": "research", "targetNodeId": "analyze"},
            {"sourceNodeId": "analyze", "targetNodeId": "report"}
        ],
        "metadata": {
            "project": "quarterly-research"
        }
    }'
}

list_dags() {
    echo "============================================================"
    echo "List DAGs"
    echo "============================================================"
    echo "Get a paginated list of DAGs."
    echo ""

    # List all DAGs
    echo "--- All DAGs ---"
    api_request GET "/api/v1/dags?page=1&limit=10"

    # List running DAGs
    echo "--- Running DAGs ---"
    api_request GET "/api/v1/dags?status=running"
}

get_dag() {
    local dag_id="${1:-dag-123}"

    echo "============================================================"
    echo "Get DAG"
    echo "============================================================"
    echo "Get details for a specific DAG."
    echo ""

    api_request GET "/api/v1/dags/$dag_id"
}

start_dag() {
    local dag_id="${1:-dag-123}"

    echo "============================================================"
    echo "Start DAG"
    echo "============================================================"
    echo "Start a DAG execution with optional input."
    echo ""

    api_request POST "/api/v1/dags/$dag_id/start" '{
        "input": {
            "customParameter": "value",
            "timestamp": "2024-01-15T10:30:00Z"
        }
    }'
}

pause_dag() {
    local dag_id="${1:-dag-123}"

    echo "============================================================"
    echo "Pause DAG"
    echo "============================================================"
    echo "Pause a running DAG."
    echo ""

    api_request POST "/api/v1/dags/$dag_id/pause"
}

resume_dag() {
    local dag_id="${1:-dag-123}"

    echo "============================================================"
    echo "Resume DAG"
    echo "============================================================"
    echo "Resume a paused DAG."
    echo ""

    api_request POST "/api/v1/dags/$dag_id/resume"
}

stop_dag() {
    local dag_id="${1:-dag-123}"

    echo "============================================================"
    echo "Stop DAG"
    echo "============================================================"
    echo "Stop a running DAG."
    echo ""

    api_request POST "/api/v1/dags/$dag_id/stop"
}

delete_dag() {
    local dag_id="${1:-dag-123}"

    echo "============================================================"
    echo "Delete DAG"
    echo "============================================================"
    echo "Delete a DAG by ID."
    echo ""

    api_request DELETE "/api/v1/dags/$dag_id"
}

get_dag_executions() {
    local dag_id="${1:-dag-123}"

    echo "============================================================"
    echo "Get DAG Executions"
    echo "============================================================"
    echo "Get execution history for a DAG."
    echo ""

    api_request GET "/api/v1/dags/$dag_id/executions?limit=10"
}

get_dag_execution() {
    local dag_id="${1:-dag-123}"
    local execution_id="${2:-exec-456}"

    echo "============================================================"
    echo "Get DAG Execution"
    echo "============================================================"
    echo "Get details for a specific DAG execution."
    echo ""

    api_request GET "/api/v1/dags/$dag_id/executions/$execution_id"
}

# =============================================================================
# Approval API
# =============================================================================

create_approval() {
    echo "============================================================"
    echo "Create Approval"
    echo "============================================================"
    echo "Create a new approval request."
    echo ""

    api_request POST "/api/v1/approvals" '{
        "taskId": "task-123",
        "approvers": ["admin@example.com", "manager@example.com"],
        "requiredApprovals": 1,
        "reason": "High-risk operation requires approval",
        "expiresAt": "2024-01-16T10:30:00Z",
        "metadata": {
            "operation": "delete_all",
            "target": "production"
        }
    }'
}

list_approvals() {
    echo "============================================================"
    echo "List Approvals"
    echo "============================================================"
    echo "Get a paginated list of approvals."
    echo ""

    # List all approvals
    echo "--- All approvals ---"
    api_request GET "/api/v1/approvals?page=1&limit=10"

    # List pending approvals
    echo "--- Pending approvals ---"
    api_request GET "/api/v1/approvals?status=pending"
}

get_approval() {
    local approval_id="${1:-approval-123}"

    echo "============================================================"
    echo "Get Approval"
    echo "============================================================"
    echo "Get details for a specific approval."
    echo ""

    api_request GET "/api/v1/approvals/$approval_id"
}

respond_to_approval() {
    local approval_id="${1:-approval-123}"

    echo "============================================================"
    echo "Respond to Approval"
    echo "============================================================"
    echo "Approve or reject an approval request."
    echo ""

    # Approve
    echo "--- Approving ---"
    api_request POST "/api/v1/approvals/$approval_id/respond" '{
        "decision": "approved",
        "comment": "Looks good, approved!"
    }'

    # Reject (alternative)
    # echo "--- Rejecting ---"
    # api_request POST "/api/v1/approvals/$approval_id/respond" '{
    #     "decision": "rejected",
    #     "comment": "Please review and resubmit"
    # }'
}

get_pending_approvals() {
    local approver_id="${1:-admin@example.com}"

    echo "============================================================"
    echo "Get Pending Approvals"
    echo "============================================================"
    echo "Get pending approvals for a specific approver."
    echo ""

    api_request GET "/api/v1/approvals/pending?approverId=$approver_id"
}

# =============================================================================
# gRPC Examples (using grpcurl)
# =============================================================================

grpc_examples() {
    echo "============================================================"
    echo "gRPC Examples (using grpcurl)"
    echo "============================================================"
    echo "These examples require grpcurl to be installed."
    echo "Install with: brew install grpcurl (macOS) or go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest"
    echo ""

    echo "--- Submit Task via gRPC ---"
    echo 'grpcurl -plaintext \
  -d '\''{"name": "Test Task", "input": {"instruction": "Hello world"}}'\'' \
  localhost:50051 apex.v1.ApexOrchestrator/SubmitTask'
    echo ""

    echo "--- Stream Agent Updates via gRPC ---"
    echo 'grpcurl -plaintext \
  -d '\''{}'\'' \
  localhost:50051 apex.v1.ApexOrchestrator/StreamAgentUpdates'
    echo ""

    echo "--- List Available Services ---"
    echo 'grpcurl -plaintext localhost:50051 list'
    echo ""

    echo "--- Describe Service ---"
    echo 'grpcurl -plaintext localhost:50051 describe apex.v1.ApexOrchestrator'
}

# =============================================================================
# Complete Workflow Example
# =============================================================================

complete_workflow() {
    echo "============================================================"
    echo "Complete Workflow Example"
    echo "============================================================"
    echo "This demonstrates a complete workflow:"
    echo "1. Create a task"
    echo "2. Check task status"
    echo "3. Wait for completion"
    echo "4. Get results"
    echo ""

    # Create a task
    echo "Step 1: Creating task..."
    TASK_RESPONSE=$(curl -s -X POST "${BASE_URL}/api/v1/tasks" \
        -H "$CONTENT_TYPE" \
        -H "$ACCEPT" \
        ${AUTH_HEADER:+-H "$AUTH_HEADER"} \
        -d '{
            "name": "Workflow Test Task",
            "description": "A task for testing the complete workflow",
            "priority": "normal"
        }')

    TASK_ID=$(echo "$TASK_RESPONSE" | jq -r '.data.id // .id // empty')

    if [ -z "$TASK_ID" ]; then
        echo "Failed to create task"
        echo "$TASK_RESPONSE"
        return 1
    fi

    echo "Created task: $TASK_ID"
    echo ""

    # Poll for completion
    echo "Step 2: Waiting for task completion..."
    MAX_ATTEMPTS=30
    ATTEMPT=0

    while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
        TASK_STATUS=$(curl -s -X GET "${BASE_URL}/api/v1/tasks/$TASK_ID" \
            -H "$ACCEPT" \
            ${AUTH_HEADER:+-H "$AUTH_HEADER"} | jq -r '.data.status // .status // empty')

        echo "  Status: $TASK_STATUS"

        if [ "$TASK_STATUS" = "completed" ]; then
            echo ""
            echo "Task completed successfully!"
            break
        elif [ "$TASK_STATUS" = "failed" ] || [ "$TASK_STATUS" = "cancelled" ]; then
            echo ""
            echo "Task $TASK_STATUS"
            break
        fi

        ATTEMPT=$((ATTEMPT + 1))
        sleep 2
    done

    # Get final result
    echo ""
    echo "Step 3: Getting final task details..."
    api_request GET "/api/v1/tasks/$TASK_ID"

    # Clean up
    echo "Step 4: Cleaning up..."
    api_request DELETE "/api/v1/tasks/$TASK_ID"

    echo "Workflow completed!"
}

# =============================================================================
# Main Menu
# =============================================================================

show_help() {
    echo "============================================================"
    echo "Apex REST API - curl Examples"
    echo "============================================================"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  health              - Check API health"
    echo ""
    echo "  Task Commands:"
    echo "  create_task         - Create a new task"
    echo "  list_tasks          - List all tasks"
    echo "  get_task [id]       - Get task by ID"
    echo "  update_task [id]    - Update a task"
    echo "  cancel_task [id]    - Cancel a task"
    echo "  retry_task [id]     - Retry a failed task"
    echo "  delete_task [id]    - Delete a task"
    echo "  get_task_logs [id]  - Get task logs"
    echo ""
    echo "  Agent Commands:"
    echo "  create_agent        - Create a new agent"
    echo "  list_agents         - List all agents"
    echo "  get_agent [id]      - Get agent by ID"
    echo "  update_agent [id]   - Update an agent"
    echo "  delete_agent [id]   - Delete an agent"
    echo ""
    echo "  DAG Commands:"
    echo "  create_dag          - Create a new DAG"
    echo "  list_dags           - List all DAGs"
    echo "  get_dag [id]        - Get DAG by ID"
    echo "  start_dag [id]      - Start DAG execution"
    echo "  pause_dag [id]      - Pause a DAG"
    echo "  resume_dag [id]     - Resume a DAG"
    echo "  stop_dag [id]       - Stop a DAG"
    echo "  delete_dag [id]     - Delete a DAG"
    echo ""
    echo "  Approval Commands:"
    echo "  create_approval     - Create an approval request"
    echo "  list_approvals      - List all approvals"
    echo "  get_approval [id]   - Get approval by ID"
    echo "  respond_to_approval [id] - Respond to approval"
    echo ""
    echo "  Other Commands:"
    echo "  grpc_examples       - Show gRPC examples"
    echo "  complete_workflow   - Run complete workflow example"
    echo "  all                 - Run all examples"
    echo "  help                - Show this help"
    echo ""
    echo "Environment Variables:"
    echo "  APEX_API_URL        - API base URL (default: http://localhost:8080)"
    echo "  APEX_API_KEY        - API key for authentication"
}

run_all() {
    health_check
    create_task
    list_tasks
    create_agent
    list_agents
    create_dag
    list_dags
    create_approval
    list_approvals
    grpc_examples
}

# =============================================================================
# Entry Point
# =============================================================================

if [ $# -eq 0 ]; then
    show_help
    exit 0
fi

case "$1" in
    health)           health_check ;;
    create_task)      create_task ;;
    list_tasks)       list_tasks ;;
    get_task)         get_task "$2" ;;
    update_task)      update_task "$2" ;;
    cancel_task)      cancel_task "$2" ;;
    retry_task)       retry_task "$2" ;;
    pause_task)       pause_task "$2" ;;
    resume_task)      resume_task "$2" ;;
    delete_task)      delete_task "$2" ;;
    get_task_logs)    get_task_logs "$2" ;;
    create_agent)     create_agent ;;
    list_agents)      list_agents ;;
    get_agent)        get_agent "$2" ;;
    update_agent)     update_agent "$2" ;;
    delete_agent)     delete_agent "$2" ;;
    assign_task)      assign_task_to_agent "$2" "$3" ;;
    create_dag)       create_dag ;;
    list_dags)        list_dags ;;
    get_dag)          get_dag "$2" ;;
    start_dag)        start_dag "$2" ;;
    pause_dag)        pause_dag "$2" ;;
    resume_dag)       resume_dag "$2" ;;
    stop_dag)         stop_dag "$2" ;;
    delete_dag)       delete_dag "$2" ;;
    get_dag_executions) get_dag_executions "$2" ;;
    get_dag_execution)  get_dag_execution "$2" "$3" ;;
    create_approval)  create_approval ;;
    list_approvals)   list_approvals ;;
    get_approval)     get_approval "$2" ;;
    respond_to_approval) respond_to_approval "$2" ;;
    get_pending_approvals) get_pending_approvals "$2" ;;
    grpc_examples)    grpc_examples ;;
    complete_workflow) complete_workflow ;;
    all)              run_all ;;
    help|--help|-h)   show_help ;;
    *)
        echo "Unknown command: $1"
        echo ""
        show_help
        exit 1
        ;;
esac
