"""Apex — Claude-powered Agent Swarm (FastAPI backend)."""

from __future__ import annotations

import asyncio
import json
import logging
import re
import uuid
from pathlib import Path
from typing import AsyncGenerator

import httpx
from fastapi import FastAPI, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import HTMLResponse, JSONResponse, StreamingResponse
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from contextlib import asynccontextmanager

from .config import (
    ANTHROPIC_API_KEY,
    ANTHROPIC_BASE_URL,
    DEFAULT_MAX_TOKENS,
    DEFAULT_TEMPERATURE,
    MODEL_NAME,
    REPORTS_DIR,
)
from .swarm import (
    EventEmitter,
    LLMClient,
    MessageBus,
    SwarmEngine,
    SwarmSession,
)
from .swarm.llm import _close_http_client as _close_llm_client
from .swarm.search import _close_http_client as _close_search_client

logger = logging.getLogger("apex")

# Shared HTTP client for connection pooling (used by /api/chat)
_chat_http_client: httpx.AsyncClient | None = None


def _get_chat_http_client() -> httpx.AsyncClient:
    """Get or create the shared HTTP client for chat endpoint."""
    global _chat_http_client
    if _chat_http_client is None:
        _chat_http_client = httpx.AsyncClient(
            timeout=120.0,
            limits=httpx.Limits(max_keepalive_connections=5, max_connections=10),
        )
    return _chat_http_client


def _validate_report_id(report_id: str) -> bool:
    """Return True if report_id is a safe hex string."""
    return re.fullmatch(r'[a-f0-9]+', report_id) is not None


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncGenerator[None, None]:
    """Manage application lifecycle - startup and shutdown."""
    # Startup: nothing to do, clients are created lazily
    yield
    # Shutdown: close shared HTTP clients
    global _chat_http_client
    if _chat_http_client is not None:
        await _chat_http_client.aclose()
        _chat_http_client = None
    await _close_llm_client()
    await _close_search_client()


STATIC_DIR = Path(__file__).parent / "static"

app = FastAPI(title="Apex — Claude Agent Swarm", lifespan=lifespan)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# ---------------------------------------------------------------------------
# Swarm globals
# ---------------------------------------------------------------------------
swarm_sessions: dict[str, SwarmSession] = {}
swarm_tasks: dict[str, asyncio.Task] = {}
event_emitter = EventEmitter()
llm_client = LLMClient()

# Mount static files
app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")


@app.get("/", response_class=HTMLResponse)
async def index() -> HTMLResponse:
    """Serve the chat frontend."""
    html_path = STATIC_DIR / "index.html"
    return HTMLResponse(content=html_path.read_text(encoding="utf-8"))


@app.post("/api/chat")
async def chat(request: Request) -> StreamingResponse:
    """Proxy user messages to the Claude API with SSE streaming."""
    body = await request.json()
    messages: list[dict] = body.get("messages", [])

    if not messages:
        return JSONResponse(status_code=400, content={"error": "messages is required"})

    # Extract system message for Anthropic format
    system_prompt = ""
    anthropic_msgs = []
    for msg in messages:
        if msg["role"] == "system":
            system_prompt = msg["content"]
        else:
            anthropic_msgs.append({"role": msg["role"], "content": msg["content"]})

    if not anthropic_msgs:
        return JSONResponse(status_code=400, content={"error": "at least one user message is required"})

    payload: dict = {
        "model": MODEL_NAME,
        "messages": anthropic_msgs,
        "stream": True,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "temperature": DEFAULT_TEMPERATURE,
    }
    if system_prompt:
        payload["system"] = system_prompt

    headers = {
        "Content-Type": "application/json",
        "x-api-key": ANTHROPIC_API_KEY,
        "anthropic-version": "2023-06-01",
    }

    async def event_stream() -> AsyncGenerator[str, None]:
        try:
            client = _get_chat_http_client()
            async with client.stream(
                "POST",
                f"{ANTHROPIC_BASE_URL}/messages",
                json=payload,
                headers=headers,
            ) as resp:
                if resp.status_code != 200:
                    error_body = await resp.aread()
                    error_msg = error_body.decode("utf-8", errors="replace")
                    logger.error("Claude API error %s: %s", resp.status_code, error_msg)
                    yield f"data: {json.dumps({'error': error_msg})}\n\n"
                    return

                async for line in resp.aiter_lines():
                    if not line:
                        continue
                    if line.startswith("data:"):
                        data_str = line[len("data:"):].strip()
                        try:
                            event = json.loads(data_str)
                            event_type = event.get("type", "")
                            if event_type == "content_block_delta":
                                delta = event.get("delta", {})
                                if delta.get("type") == "text_delta":
                                    text = delta.get("text", "")
                                    if text:
                                        # Re-emit as OpenAI-compatible SSE for the frontend
                                        sse_data = {
                                            "choices": [{"delta": {"content": text}}]
                                        }
                                        yield f"data: {json.dumps(sse_data)}\n\n"
                            elif event_type == "message_stop":
                                yield "data: [DONE]\n\n"
                                break
                        except json.JSONDecodeError:
                            continue
        except Exception as exc:
            logger.exception("Chat stream failed: %s", exc)
            yield f"data: {json.dumps({'error': str(exc)})}\n\n"

    return StreamingResponse(
        event_stream(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "X-Accel-Buffering": "no",
        },
    )


# ---------------------------------------------------------------------------
# Swarm endpoints
# ---------------------------------------------------------------------------


@app.post("/api/swarm/start")
async def swarm_start(request: Request) -> JSONResponse:
    """Launch a new swarm session for the given query."""
    body = await request.json()
    query: str = body.get("query", "")
    if not query:
        return JSONResponse(status_code=400, content={"error": "query is required"})

    session = SwarmSession(id=uuid.uuid4().hex[:12], query=query)
    swarm_sessions[session.id] = session

    bus = MessageBus(session)
    engine = SwarmEngine(session, bus, event_emitter, llm_client)

    async def _run_and_cleanup() -> None:
        try:
            await engine.run(query)
        finally:
            # Give WebSocket clients time to drain final events before cleanup
            await asyncio.sleep(2)
            swarm_sessions.pop(session.id, None)
            swarm_tasks.pop(session.id, None)

    task = asyncio.create_task(_run_and_cleanup())
    swarm_tasks[session.id] = task

    return JSONResponse(content={"session_id": session.id})


@app.post("/api/swarm/{session_id}/stop")
async def swarm_stop(session_id: str) -> JSONResponse:
    """Cancel a running swarm session."""
    session = swarm_sessions.get(session_id)
    if session is None:
        return JSONResponse(status_code=404, content={"error": "session not found"})
    session.cancelled = True
    task = swarm_tasks.get(session_id)
    if task is not None:
        task.cancel()
    await event_emitter.emit(session_id, "swarm_cancelled", {"message": "Swarm stopped by user"})
    return JSONResponse(content={"status": "stopped"})


@app.get("/api/swarm/history")
async def swarm_history() -> list[dict]:
    """List all completed swarm reports, newest first."""
    reports = []
    for f in sorted(REPORTS_DIR.glob("*.json"), key=lambda p: p.stat().st_mtime, reverse=True):
        try:
            data = json.loads(f.read_text(encoding="utf-8"))
            reports.append({
                "id": data["id"],
                "query": data.get("query", ""),
                "agents_count": data.get("agents_count", 0),
                "created_at": data.get("created_at", ""),
                "completed_at": data.get("completed_at", ""),
            })
        except Exception:
            continue
    return reports


@app.get("/api/swarm/history/{report_id}")
async def swarm_history_detail(report_id: str) -> JSONResponse:
    """Get a single completed swarm report, including rendered HTML if available."""
    if not _validate_report_id(report_id):
        return JSONResponse(status_code=404, content={"error": "report not found"})
    path = REPORTS_DIR / f"{report_id}.json"
    if not path.exists():
        return JSONResponse(status_code=404, content={"error": "report not found"})
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        # Attach rendered HTML from companion file if it exists
        html_path = REPORTS_DIR / f"{report_id}.html"
        if html_path.exists():
            data["report_html"] = html_path.read_text(encoding="utf-8")
        return JSONResponse(content=data)
    except Exception as exc:
        return JSONResponse(status_code=500, content={"error": str(exc)})


@app.get("/api/swarm/report/{report_id}/html", response_class=HTMLResponse)
async def swarm_report_html(report_id: str) -> HTMLResponse:
    """Serve a rendered HTML report by ID."""
    if not _validate_report_id(report_id):
        return HTMLResponse(
            content="<h1>Report not found</h1><p>No HTML report available for this ID.</p>",
            status_code=404,
        )
    html_path = REPORTS_DIR / f"{report_id}.html"
    if not html_path.exists():
        return HTMLResponse(
            content="<h1>Report not found</h1><p>No HTML report available for this ID.</p>",
            status_code=404,
        )
    return HTMLResponse(content=html_path.read_text(encoding="utf-8"))


@app.websocket("/ws/swarm/{session_id}")
async def swarm_ws(websocket: WebSocket, session_id: str) -> None:
    """Stream real-time swarm events to the frontend over WebSocket."""
    session = swarm_sessions.get(session_id)
    if session is None:
        await websocket.close(code=4004, reason="session not found")
        return

    await websocket.accept()

    queue: asyncio.Queue = asyncio.Queue()
    event_emitter.subscribe(session_id, queue)

    try:
        # Send initial state snapshot
        await websocket.send_json({
            "type": "snapshot",
            "session_id": session.id,
            "query": session.query,
            "phase": session.phase.value,
            "agents": {
                aid: {"id": a.id, "role": a.role.value, "name": a.name, "status": a.status}
                for aid, a in session.agents.items()
            },
            "tasks": [
                {"id": t.id, "description": t.description, "status": t.status}
                for t in session.tasks
            ],
        })

        # Fan out events until the client disconnects
        while True:
            event = await queue.get()
            await websocket.send_json(event)
    except WebSocketDisconnect:
        pass
    finally:
        event_emitter.unsubscribe(session_id, queue)
