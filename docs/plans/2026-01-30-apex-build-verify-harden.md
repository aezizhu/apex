# Project Apex - Build, Verify & Harden Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Verify the entire Apex stack compiles/builds, fill implementation gaps, add missing frontend features (Task Timeline, Approval Queue enhancements, Agent Sight, Intervention Panel), complete test coverage, and produce a working MVP that starts with `docker-compose up`.

**Architecture:** Rust orchestration engine (axum + tonic + tokio) → Python agent workers (asyncio + OpenAI/Anthropic) → React dashboard (Vite + Tailwind + Plotly/D3) → PostgreSQL + Redis + Jaeger + Prometheus + Grafana. All services orchestrated via Docker Compose.

**Tech Stack:** Rust 1.75+, Python 3.11+, TypeScript 5.3+, React 18, Vite, Tailwind CSS, PostgreSQL 16, Redis 7, Docker

---

## Work Streams (Parallelizable)

These work streams are independent and can be dispatched to parallel agents:

---

### Stream A: Rust Backend - Fix Warnings & Verify Build

**Task A1: Clean up Rust warnings**

**Files:**
- Modify: `src/backend/core/src/orchestrator/mod.rs` (unused imports)
- Modify: `src/backend/core/src/api/handlers.rs` (unused imports)
- Modify: Multiple files with unused field warnings

**Steps:**
1. Run `cargo build 2>&1` in `src/backend/core/` and capture all warnings
2. Run `cargo fix --allow-dirty` to auto-fix unused imports
3. Add `#[allow(dead_code)]` to intentionally unused fields (config structs for future use)
4. Run `cargo build` again - target: 0 warnings
5. Run `cargo test` - verify all unit tests pass
6. Run `cargo clippy -- -D warnings` - fix any clippy lints

**Task A2: Verify database migrations work**

**Files:**
- Read: `src/backend/core/migrations/*.sql`
- Verify: Schema matches Rust struct definitions

**Steps:**
1. Start PostgreSQL via Docker: `docker run -d --name apex-pg -e POSTGRES_USER=apex -e POSTGRES_PASSWORD=apex_secret -e POSTGRES_DB=apex -p 5432:5432 postgres:16-alpine`
2. Apply migrations: `sqlx database create && sqlx migrate run` (or manual psql)
3. Verify all tables created: agents, tasks, dags, approvals, contracts, audit_log, events
4. Verify enums match Rust/Python enum definitions
5. Tear down test database

**Task A3: Verify gRPC proto compilation**

**Files:**
- Read: `src/backend/core/proto/apex.proto`
- Read: `src/backend/core/build.rs`

**Steps:**
1. Verify `protoc` is available or install it
2. Run `cargo build` and confirm tonic-build generates code
3. Verify generated types match handler signatures in `api/grpc.rs`

---

### Stream B: Python Agent Layer - Fill Gaps & Verify

**Task B1: Implement web_search tool**

**Files:**
- Modify: `src/backend/agents/apex_agents/tools.py`

**Steps:**
1. Replace the stub `web_search()` function with a real implementation using httpx
2. Use a search API (SerpAPI, Brave Search, or DuckDuckGo) with configurable API key
3. Return structured results: title, url, snippet
4. Add rate limiting (max 10 requests/minute)
5. Add proper error handling and timeout (10s)
6. Write test in `tests/test_tools.py` for web_search

**Task B2: Verify Python agent tests pass**

**Steps:**
1. `cd src/backend/agents && pip install -e ".[dev]"`
2. `pytest tests/ -v --tb=short`
3. Fix any failures
4. Run `mypy apex_agents/` - fix type errors
5. Run `ruff check apex_agents/` - fix lint issues

**Task B3: Add missing agent persistence**

**Files:**
- Modify: `src/backend/agents/apex_agents/executor.py`

**Steps:**
1. Add SQLAlchemy models for agent state persistence
2. Store agent metrics after each task execution
3. Load agent configuration from database on startup
4. Track cumulative stats (total_tokens, total_cost, success_count, fail_count)

---

### Stream C: Frontend - Build Missing Dashboard Features

**Task C1: Build Task Timeline (Gantt Chart) component**

**Files:**
- Create: `src/frontend/src/components/TaskTimeline/TaskTimeline.tsx`
- Create: `src/frontend/src/components/TaskTimeline/index.ts`
- Modify: `src/frontend/src/pages/Tasks.tsx` (integrate timeline view)

**Steps:**
1. Create TaskTimeline component using Plotly.js Gantt chart
2. Display tasks as horizontal bars with:
   - Color by status (running=blue, completed=green, failed=red, pending=gray)
   - Dependencies shown as arrows between bars
   - Critical path highlighted in bold red
3. Add zoom/pan controls
4. Add hover tooltips (task name, agent, duration, cost)
5. Integrate into Tasks page as a toggle view (List | Timeline)
6. Add WebSocket subscription for real-time updates

**Task C2: Enhance Agent Grid with confidence heatmap**

**Files:**
- Modify: `src/frontend/src/components/AgentGrid.tsx`

**Steps:**
1. Add confidence scoring to agent data model
2. Implement color gradient: high confidence = deep blue, low = red
3. Add heatmap toggle overlay
4. Add agent count by status in legend
5. Optimize rendering for 1000+ agents (virtualization if needed)

**Task C3: Build Intervention Panel component**

**Files:**
- Create: `src/frontend/src/components/InterventionPanel/InterventionPanel.tsx`
- Create: `src/frontend/src/components/InterventionPanel/index.ts`
- Modify: `src/frontend/src/pages/Agents.tsx` (integrate panel)

**Steps:**
1. Create side panel with 4 intervention actions:
   - **Nudge**: Send system message to agent (text input + send)
   - **Pause & Patch**: Freeze agent, show editable state JSON, resume button
   - **Takeover**: Switch to human control mode (shows agent's current context)
   - **Kill Switch**: Emergency halt with confirmation dialog
2. Wire to WebSocket for real-time agent control
3. Add keyboard shortcuts (N=nudge, P=pause, K=kill)
4. Add audit log entry for each intervention

**Task C4: Build Causal Trace Viewer**

**Files:**
- Create: `src/frontend/src/components/CausalTrace/CausalTraceViewer.tsx`
- Create: `src/frontend/src/components/CausalTrace/index.ts`

**Steps:**
1. Create a tree/graph visualization of agent reasoning chains
2. Show: LLM call → tool call → result → next LLM call
3. Color-code by cost (expensive steps in red)
4. Click to expand step details (full prompt, response, tokens)
5. Link to Jaeger trace ID for full distributed trace
6. Integrate into agent detail view

**Task C5: Verify frontend builds and fix issues**

**Steps:**
1. `cd src/frontend && npm install`
2. `npm run build` - fix any TypeScript errors
3. `npm run lint` - fix ESLint issues
4. `npm run test` - run vitest, fix failures
5. `npx playwright test` - run E2E tests (may need backend running)
6. Verify dark mode works across all pages
7. Check responsive layout at 768px and 1024px breakpoints

---

### Stream D: Integration & Docker Compose

**Task D1: Verify Docker Compose starts all services**

**Steps:**
1. Review Dockerfiles in `docker/` directory
2. Create missing Dockerfiles if needed (for api, worker, dashboard)
3. Run `docker-compose build` - fix any build failures
4. Run `docker-compose up` - verify all 11 services start
5. Check health endpoints:
   - `curl http://localhost:8080/health` (API)
   - `curl http://localhost:3000` (Dashboard)
   - `curl http://localhost:16686` (Jaeger)
   - `curl http://localhost:9090` (Prometheus)
   - `curl http://localhost:3001` (Grafana)
6. Fix networking issues between services
7. Verify database migrations run on startup

**Task D2: Create end-to-end smoke test**

**Files:**
- Create: `tests/e2e/smoke_test.py`

**Steps:**
1. Script that verifies the full stack works:
   - POST /api/v1/agents (create 3 agents)
   - POST /api/v1/dags (create a 3-task DAG)
   - POST /api/v1/dags/:id/execute (run it)
   - Poll GET /api/v1/dags/:id/status until complete
   - Verify all tasks completed
   - Verify WebSocket received status updates
   - Verify traces appear in Jaeger
   - Verify metrics appear in Prometheus
2. Run against Docker Compose stack
3. Report pass/fail with timing

**Task D3: Wire Grafana dashboards**

**Files:**
- Modify: `infra/observability/grafana/dashboards/`

**Steps:**
1. Verify pre-built dashboards load in Grafana
2. Create/fix dashboard panels:
   - Active agents count
   - Task throughput (tasks/minute)
   - P50/P95/P99 latency
   - Token consumption rate
   - Cost per task trend
   - Error rate
3. Set up Prometheus data source auto-provisioning
4. Set up Loki data source for log queries

---

### Stream E: Testing & Quality

**Task E1: Fill in stubbed integration tests**

**Files:**
- Modify: `tests/integration/test_api_tasks.py`
- Modify: `tests/integration/test_api_agents.py`
- Modify: `tests/integration/test_api_dags.py`
- Modify: `tests/integration/test_websocket.py`
- Modify: `tests/integration/test_workflow.py`
- Modify: `tests/integration/test_contracts.py`

**Steps:**
1. Review each test file, identify any TODO/placeholder tests
2. Implement missing test cases:
   - Task cancellation cascading
   - Contract limit enforcement (token, cost, time)
   - DAG cycle rejection
   - WebSocket reconnection handling
   - Concurrent task execution (10+ agents)
3. Run full integration suite against Docker Compose
4. Target: 90%+ test pass rate

**Task E2: Add Rust integration tests**

**Files:**
- Create: `src/backend/core/tests/integration/`

**Steps:**
1. Add integration tests for:
   - API endpoint responses (status codes, JSON shapes)
   - Database operations (CRUD for tasks, agents, DAGs)
   - WebSocket message flow
   - Contract enforcement
2. Use testcontainers-rs for PostgreSQL and Redis
3. Run with `cargo test --test integration`

**Task E3: Performance benchmark baseline**

**Steps:**
1. Run k6 load test: `k6 run benchmarks/k6/load-test.js`
2. Run stress test: `k6 run benchmarks/k6/stress-test.js`
3. Record baseline numbers:
   - Requests/second at steady state
   - P50, P95, P99 latency
   - Error rate under load
   - Memory/CPU usage
4. Compare against targets in spec (P95 < 100ms for POST /tasks)

---

## Execution Order

**Phase 1 (Parallel):** Streams A, B, C5, D1 - verify everything builds
**Phase 2 (Parallel):** Streams C1-C4, D2, D3 - build missing features
**Phase 3 (Parallel):** Streams E1, E2, E3 - testing and benchmarks
**Phase 4 (Sequential):** Final integration, smoke test, demo prep

---

## Success Criteria

- [ ] `cargo build` - 0 errors, <10 warnings
- [ ] `cargo test` - all pass
- [ ] `pytest` (agents) - all pass
- [ ] `npm run build` (frontend) - 0 errors
- [ ] `docker-compose up` - all 11 services healthy
- [ ] Smoke test passes (create agents → run DAG → verify completion)
- [ ] Dashboard shows real-time agent updates via WebSocket
- [ ] Task Timeline (Gantt) renders with dependencies
- [ ] Intervention Panel allows pause/resume/kill
- [ ] Grafana dashboards show metrics
- [ ] Jaeger shows distributed traces
- [ ] k6 benchmark: P95 < 100ms for task creation
