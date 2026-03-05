"""Configuration for the Apex application."""

import logging
import os
from pathlib import Path

from dotenv import load_dotenv

load_dotenv()

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
)

# Claude API (Anthropic)
ANTHROPIC_API_KEY = os.getenv("ANTHROPIC_API_KEY", "")

ANTHROPIC_BASE_URL = "https://api.anthropic.com/v1"
MODEL_NAME = os.getenv("APEX_MODEL", "claude-sonnet-4-6")
DEFAULT_MAX_TOKENS = 16384
DEFAULT_TEMPERATURE = 1.0

SWARM_DEFAULTS = {
    "max_researchers": 5,
    "stream_chunks": True,
    "timeout_per_phase": 120,
}

# Firecrawl web search API
FIRECRAWL_BASE_URL = os.getenv("FIRECRAWL_BASE_URL", "https://api-production-91c7.up.railway.app")

# Persistent storage for completed swarm reports
DATA_DIR = Path(__file__).resolve().parent.parent.parent.parent / "data"
REPORTS_DIR = DATA_DIR / "reports"
REPORTS_DIR.mkdir(parents=True, exist_ok=True)
