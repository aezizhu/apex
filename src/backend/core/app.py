"""Apex — MiniMax-M2.5 Chat Application (FastAPI backend)."""

from __future__ import annotations

import asyncio
import json
import logging
import uuid
from pathlib import Path
from typing import AsyncGenerator

import httpx
from fastapi import FastAPI, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import HTMLResponse, StreamingResponse
from fastapi.staticfiles import StaticFiles

from .config import (
    DEFAULT_MAX_TOKENS,
    DEFAULT_TEMPERATURE,
    MINIMAX_API_KEY,
    MINIMAX_BASE_URL,
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

logger = logging.getLogger("apex")

STATIC_DIR = Path(__file__).parent / "static"

app = FastAPI(title="Apex — MiniMax-M2.5")

# ---------------------------------------------------------------------------
# Swarm globals
# ---------------------------------------------------------------------------
swarm_sessions: dict[str, SwarmSession] = {}
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
    """Proxy user messages to the MiniMax API with SSE streaming."""
    body = await request.json()
    messages: list[dict] = body.get("messages", [])

    payload = {
        "model": MODEL_NAME,
        "messages": messages,
        "stream": True,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "temperature": DEFAULT_TEMPERATURE,
    }

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {MINIMAX_API_KEY}",
    }

    async def event_stream() -> AsyncGenerator[str, None]:
        async with httpx.AsyncClient(timeout=120.0) as client:
            async with client.stream(
                "POST",
                f"{MINIMAX_BASE_URL}/chat/completions",
                json=payload,
                headers=headers,
            ) as resp:
                if resp.status_code != 200:
                    error_body = await resp.aread()
                    error_msg = error_body.decode("utf-8", errors="replace")
                    logger.error("MiniMax API error %s: %s", resp.status_code, error_msg)
                    yield f"data: {json.dumps({'error': error_msg})}\n\n"
                    return

                async for line in resp.aiter_lines():
                    if not line:
                        continue
                    # Forward SSE events as-is
                    if line.startswith("data:"):
                        data_str = line[len("data:"):].strip()
                        if data_str == "[DONE]":
                            yield "data: [DONE]\n\n"
                            break
                        yield f"data: {data_str}\n\n"

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
async def swarm_start(request: Request) -> dict:
    """Launch a new swarm session for the given query."""
    body = await request.json()
    query: str = body.get("query", "")
    if not query:
        return {"error": "query is required"}

    session = SwarmSession(id=uuid.uuid4().hex[:12], query=query)
    swarm_sessions[session.id] = session

    bus = MessageBus(session)
    engine = SwarmEngine(session, bus, event_emitter, llm_client)

    asyncio.create_task(engine.run(query))

    return {"session_id": session.id}


@app.post("/api/swarm/{session_id}/stop")
async def swarm_stop(session_id: str) -> dict:
    """Cancel a running swarm session."""
    session = swarm_sessions.get(session_id)
    if session is None:
        return {"error": "session not found"}
    session.cancelled = True
    await event_emitter.emit(session_id, "swarm_cancelled", {"message": "Swarm stopped by user"})
    return {"status": "stopped"}


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
async def swarm_history_detail(report_id: str) -> dict:
    """Get a single completed swarm report, including rendered HTML if available."""
    path = REPORTS_DIR / f"{report_id}.json"
    if not path.exists():
        return {"error": "report not found"}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        # Attach rendered HTML from companion file if it exists
        html_path = REPORTS_DIR / f"{report_id}.html"
        if html_path.exists():
            data["report_html"] = html_path.read_text(encoding="utf-8")
        return data
    except Exception as exc:
        return {"error": str(exc)}


@app.get("/api/swarm/report/{report_id}/html", response_class=HTMLResponse)
async def swarm_report_html(report_id: str) -> HTMLResponse:
    """Serve a rendered HTML report by ID."""
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
