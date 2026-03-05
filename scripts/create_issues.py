#!/usr/bin/env python3
"""
Script to create GitHub issues from the code review findings.
"""

import subprocess
import json

# Define issues to create
issues = [
    {
        "title": "[CRITICAL] Security Issues: Hardcoded Credentials, Default JWT Secrets, Unrestricted CORS",
        "body": """## Summary

This issue tracks multiple critical security vulnerabilities found in the Apex codebase that require immediate attention.

## Issues Found

### 1. Hardcoded Database Credentials (Critical)
- **File:** `src/backend/core/src/main.rs:32`
- Default fallback config contains hardcoded credentials: `postgres://apex:apex_secret@localhost:5432/apex`
- Creates security vulnerability if config fails to load

### 2. Default JWT Secret (Critical)
- **File:** `src/backend/core/src/websocket/mod.rs:78`
- Default JWT secret is `change-me-in-production`
- Will be used if no secret is configured

### 3. Unrestricted CORS (Critical)
- **File:** `src/backend/core/src/api/mod.rs:112-115`
- `CorsLayer` configured with `allow_origin(Any)` 
- Permits cross-origin requests from any domain

### 4. Authentication Enabled by Default
- **File:** `src/backend/core/src/middleware/auth.rs:398`
- `AuthConfig::default()` sets `enabled: true` but `jwt_secret` is None
- Will cause runtime errors

### 5. Task Creation Without Authentication
- **File:** `src/backend/core/src/api/handlers.rs:162`
- `create_task` handler doesn't check or associate with authenticated user
- Any client can create tasks without authentication

## Priority: CRITICAL
These issues expose the application to security vulnerabilities and must be fixed before any production deployment.
""",
        "labels": "critical,security"
    },
    {
        "title": "[CRITICAL] Concurrency Bugs: Agent Selection Race Condition, Busy-Waiting Loop",
        "body": """## Summary

Critical concurrency issues found in the Rust backend that can cause race conditions and performance problems.

## Issues Found

### 1. Agent Selection Race Condition
- **File:** `src/backend/core/src/orchestrator/mod.rs:355-358`
- Finding an available agent iterates over DashMap without holding a lock
- Multiple concurrent tasks could all see the same 'available' agent

### 2. Busy-Waiting Loop
- **File:** `src/backend/core/src/orchestrator/mod.rs:229-233`
- DAG execution uses fixed 100ms sleep when no tasks are ready
- Causes unnecessary CPU spinning

### 3. Agent Slot Acquisition Race Condition
- **File:** `src/backend/core/src/agents/mod.rs:164-171`
- `acquire_slot` method has race condition with fetch_add
- Multiple threads could all think they acquired a slot

### 4. New Redis Connection Per Task
- **File:** `src/backend/core/src/orchestrator/mod.rs:405-422`
- Each task execution creates a new Redis connection
- Creates connection overhead for high-throughput scenarios

## Priority: CRITICAL
These concurrency issues can cause data corruption, incorrect task assignment, and performance degradation.
""",
        "labels": "critical,concurrency,performance"
    },
    {
        "title": "[HIGH] Configuration Issues: Ignored Values, Missing Validation, Hardcoded Limits",
        "body": """## Summary

Configuration management issues that cause configured values to be ignored.

## Issues Found

### 1. Database Pool Size Ignored
- **File:** `src/backend/core/src/db/mod.rs:26-31`
- Hardcodes max_connections(20) and min_connections(5)
- Ignores config values passed to the function

### 2. API Call Limit Hardcoded
- **File:** `src/backend/core/src/orchestrator/mod.rs:81`
- `api_call_limit: 100` is hardcoded with no configuration option

### 3. Config Load Silently Ignores Errors
- **File:** `src/backend/core/src/config.rs:30-41`
- Uses `unwrap_or_else` which silently falls back to defaults
- Can hide configuration errors in production

### 4. Config Doesn't Validate Required Fields
- **File:** `src/backend/core/src/config.rs:197-205`
- Doesn't validate that required fields like database.url are set
- Fails later with unclear error messages

### 5. Server Doesn't Handle Port Bind Failures
- **File:** `src/backend/core/src/main.rs:121`
- If port is already in use, server panics with unclear error

## Priority: HIGH
These issues cause configured values to be ignored, leading to unexpected behavior.
""",
        "labels": "high,configuration"
    },
    {
        "title": "[CRITICAL] Python SDK: Authentication Mismatch, Missing Endpoints, Hardcoded Retry",
        "body": """## Summary

Critical issues in Python SDK that cause API incompatibility and functionality gaps.

## Issues Found

### 1. Authentication Header Mismatch with TypeScript SDK
- **File:** `sdk/python/apex_sdk/client.py:146-150`
- Python uses `X-API-Key` header, TypeScript uses `Authorization: Bearer`
- Clients using TypeScript SDK cannot authenticate to servers expecting X-API-Key

### 2. Missing /api/v1 Prefix in Endpoints
- **File:** `sdk/python/apex_sdk/client.py:335-346`
- Uses `/health`, `/tasks`, `/agents` without `/api/v1` prefix
- TypeScript SDK uses `/api/v1/` prefix

### 3. Response Format Inconsistency
- **File:** `sdk/python/apex_sdk/client.py:230-231`
- Python directly returns response.json() without wrapper
- TypeScript expects `{ success: true, data: ... }` wrapper

### 4. Missing Python SDK Endpoints
- **File:** `sdk/python/apex_sdk/client.py`
- Missing: task logs (`get_task_logs`)
- Missing: child tasks (`get_child_tasks`)
- Missing: agent task assignment (`assign_task`, `unassign_task`)
- Missing: DAG executions (`get_dag_executions`, `get_dag_execution`)
- Missing: `cancel_approval`, `get_pending_approvals`

### 5. Retry Logic Ignores max_retries Parameter
- **File:** `sdk/python/apex_sdk/client.py:233-257`
- Hardcodes `stop=stop_after_attempt(3)` ignoring configured max_retries

## Priority: CRITICAL
These issues cause SDK interoperability problems and missing functionality.
""",
        "labels": "critical,sdk,python"
    },
    {
        "title": "[HIGH] TypeScript SDK: Authentication Bug, Type Inconsistencies, Missing Tests",
        "body": """## Summary

TypeScript SDK issues that cause type unsafety and missing functionality.

## Issues Found

### 1. Authentication Header Mismatch
- **File:** `sdk/typescript/src/client.ts:167`
- Uses `Authorization: Bearer ${apiKey}` instead of `X-API-Key`
- Incompatible with Python SDK and possibly server expectations

### 2. Type Inconsistencies with Python
- **File:** `sdk/typescript/src/types.ts`
- TaskStatus has extra `WAITING_APPROVAL` value not in Python
- DAGStatus values don't match (DRAFT, ACTIVE vs PENDING)
- WebSocketEventType differs (APPROVAL_RESOLVED vs APPROVAL_COMPLETED)

### 3. Unsafe Type Casting
- **File:** `sdk/typescript/src/client.ts:842`
- Uses `as never` casting which defeats TypeScript's type safety

### 4. Missing waitFor Equivalent
- **File:** `sdk/typescript/src/websocket.ts`
- No method to wait for specific events with timeout

### 5. Missing WebSocket Tests
- **File:** `sdk/typescript/tests/websocket.test.ts`
- WebSocket functionality has no test coverage

## Priority: HIGH
These issues cause type unsafety and API incompatibility.
""",
        "labels": "high,sdk,typescript"
    },
    {
        "title": "[HIGH] Frontend: Critical Issues - Broken State, No Auth, Unsafe Types",
        "body": """## Summary

Critical issues in the React frontend that break functionality and security.

## Issues Found

### 1. WebSocket Connection State Returns Ref (Not Reactive)
- **File:** `src/frontend/src/hooks/useWebSocket.ts:263`
- Returns `connectionState.current` which is a ref, won't trigger re-renders
- Users won't see connection status changes

### 2. No Protected Routes / Authentication
- **File:** `src/frontend/src/App.tsx:20-32`
- All routes are publicly accessible
- No authentication check, no redirect to login

### 3. WebSocket Messages Use Unsafe `any` Type
- **File:** `src/frontend/src/types/index.ts:253-254`
- Uses `[key: string]: unknown` without proper validation
- All message handlers cast from unknown unsafely

### 4. Keyboard Shortcuts Exposed Without Auth
- **File:** `src/frontend/src/pages/Settings.tsx:164-170`
- Shows keyboard shortcuts (j/k/a/d) without checking user permissions
- Anyone can approve/deny actions

### 5. Duplicate Data Fetching
- **File:** `src/frontend/src/pages/Dashboard.tsx:96-109`
- Each page fetches same data independently
- Causes redundant API calls

### 6. No Message Validation
- **File:** `src/frontend/src/hooks/useWebSocket.ts:60-168`
- Messages processed without schema validation
- Malformed messages could cause runtime errors

## Priority: HIGH
These issues break core functionality and expose security vulnerabilities.
""",
        "labels": "high,frontend,react"
    },
    {
        "title": "[MEDIUM] Code Quality: Large Files, Duplicate Code, Missing Tests",
        "body": """## Summary

Code quality issues that make the codebase harder to maintain.

## Rust Backend Issues

### 1. Empty Orchestrator Tests
- **File:** `src/backend/core/src/orchestrator/mod.rs:589-592`
- Test module is empty, provides no coverage for core logic

### 2. Cycle Detection Reactive Not Preventive
- **File:** `src/backend/core/src/dag/mod.rs:79`
- Adds edge then removes if cycle found - inefficient for large graphs

### 3. get_ready_tasks O(n) Each Call
- **File:** `src/backend/core/src/dag/mod.rs:108-128`
- Iterates entire graph each call, not maintaining ready queue

### 4. Contract Created But Never Used
- **File:** `src/backend/core/src/orchestrator/mod.rs:376`
- AgentContract created but never stored

### 5. Task Creation Without Transaction
- **File:** `src/backend/core/src/api/handlers.rs:169-180`
- insert_dag and insert_task called separately without transaction

## Frontend Issues

### 1. Dashboard.tsx is 900+ Lines
- **File:** `src/frontend/src/pages/Dashboard.tsx:1-905`
- Contains 5 inline components violating Single Responsibility Principle

### 2. Tasks.tsx is 1000+ Lines
- **File:** `src/frontend/src/pages/Tasks.tsx:1-1039`
- Contains multiple inline components

### 3. Agents.tsx is 640+ Lines
- **File:** `src/frontend/src/pages/Agents.tsx:1-642`
- Multiple responsibilities in single file

## Priority: MEDIUM
These issues increase maintenance burden and technical debt.
""",
        "labels": "medium,code-quality"
    },
    {
        "title": "[HIGH] Test Coverage Gaps: Missing Critical Path Tests",
        "body": """## Summary

Test coverage gaps that leave critical functionality untested.

## Issues Found

### 1. Frontend Tests Are Shallow
- **File:** `src/frontend/src/__tests__/**/*.test.tsx`
- Tests verify component renders with mocks
- Don't test actual functionality or error handling

### 2. No Contract Enforcement Tests
- **File:** `tests/integration/`
- Contracts define resource limits but no tests verify enforcement
- Critical business logic completely untested

### 3. Integration Test Migrations Not Run
- **File:** `.github/workflows/ci.yml:554-563`
- Has placeholder migration commands
- Tests may run against uninitialized database

### 4. No Redis Integration Tests
- **File:** `src/backend/agents/tests/`
- All tests use mocks, no real Redis connection tests
- Can't catch real connection issues

### 5. Frontend E2E Coverage Limited
- **File:** `src/frontend/e2e/*.spec.ts`
- Only 10 files covering main flows
- Many user paths untested

### 6. No Benchmark Regression Tracking
- **File:** `src/backend/core/benches/*.rs`
- Benchmarks exist but no comparison to baseline
- Can't detect performance regressions

### 7. WebSocket Tests Limited
- **File:** `tests/integration/test_websocket.py`
- Critical real-time functionality has limited test coverage

## Priority: HIGH
These gaps mean critical bugs could ship to production undetected.
""",
        "labels": "high,testing,coverage"
    },
    {
        "title": "[HIGH] Python Backend Bugs: eval(), shell=True, Silent Errors",
        "body": """## Summary

Critical bugs in Python agents that cause security vulnerabilities and silent failures.

## Issues Found

### 1. eval() in Calculate Tool - Command Injection Risk
- **File:** `src/backend/agents/apex_agents/tools.py:488`
- Uses `eval()` to execute Python code from user input
- Extremely dangerous - allows arbitrary code execution

### 2. shell=True in subprocess - Command Injection
- **File:** `src/backend/agents/apex_agents/tools.py:431`
- Uses `shell=True` in subprocess.run
- Enables command injection attacks

### 3. API Key Not Validated
- **File:** `src/backend/agents/apex_agents/config.py:17`
- API key loaded without validation
- Silent failures if key is invalid

### 4. HTTP Client Created Per Request - No Connection Pooling
- **File:** `src/backend/agents/apex_agents/llm.py:85`
- Creates new HTTP client for each request
- Severe performance impact

### 5. Same Issue in apex_agents/llm.py
- **File:** `src/backend/agents/apex_agents/llm.py:173,254`
- Creates HTTP client per request

### 6. Tracing Bug - Undefined Variable
- **File:** `src/backend/agents/apex_agents/tracing.py:500`
- Uses `self._context_token` which is undefined
- Should be `self._token`

### 7. Unbounded Memory Growth
- **File:** `src/backend/core/swarm/events.py`
- Event buffer grows without bounds

### 8. Global Rate Limiting Race Conditions
- **File:** `src/backend/agents/apex_agents/llm.py`
- `_last_search_time`, `_last_call_time` have race conditions

## Priority: HIGH
These bugs cause security vulnerabilities and silent failures.
""",
        "labels": "high,python,bug,security"
    }
]

# Create issues
for issue in issues:
    cmd = [
        "gh", "issue", "create",
        "--title", issue["title"],
        "--body", issue["body"],
        "--label", issue["labels"]
    ]
    result = subprocess.run(cmd, capture_output=True, text=True, cwd="/Users/aezi/Desktop/Apex")
    print(f"Created: {issue['title'][:60]}...")
    if result.returncode != 0:
        print(f"Error: {result.stderr}")

print(f"\nTotal issues created: {len(issues)}")
