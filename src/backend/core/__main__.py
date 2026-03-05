"""Entry point for running the Apex FastAPI application."""

import os

import uvicorn

from .app import app  # noqa: F401

if __name__ == "__main__":
    uvicorn.run(
        "src.backend.core.app:app",
        host=os.getenv("APEX_HOST", "0.0.0.0"),
        port=int(os.getenv("APEX_PORT", "8000")),
        reload=os.getenv("APEX_RELOAD", "").lower() in ("1", "true", "yes"),
    )
