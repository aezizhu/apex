"""Web search client using Firecrawl API for real research data."""

from __future__ import annotations

import logging
from typing import Any

import httpx

from ..config import FIRECRAWL_API_KEY, FIRECRAWL_BASE_URL

logger = logging.getLogger("apex.swarm.search")

# Shared HTTP client for connection pooling
_http_client: httpx.AsyncClient | None = None


def _get_http_client() -> httpx.AsyncClient:
    """Get or create the shared HTTP client with connection pooling."""
    global _http_client
    if _http_client is None:
        _http_client = httpx.AsyncClient(
            timeout=30.0,
            limits=httpx.Limits(max_keepalive_connections=5, max_connections=10),
        )
    return _http_client


async def _close_http_client() -> None:
    """Close the shared HTTP client (call on shutdown)."""
    global _http_client
    if _http_client is not None:
        await _http_client.aclose()
        _http_client = None


class WebSearchClient:
    """Async wrapper around the Firecrawl v1 search + scrape endpoints."""

    def __init__(self) -> None:
        self._base = FIRECRAWL_BASE_URL
        self._headers: dict[str, str] = {"Content-Type": "application/json"}
        if FIRECRAWL_API_KEY:
            self._headers["Authorization"] = f"Bearer {FIRECRAWL_API_KEY}"

    async def search(self, query: str, limit: int = 5) -> list[dict[str, Any]]:
        """Search the web and return results with scraped markdown content."""
        try:
            client = _get_http_client()
            resp = await client.post(
                    f"{self._base}/v1/search",
                    headers=self._headers,
                    json={
                        "query": query,
                        "limit": limit,
                        "scrapeOptions": {"formats": ["markdown"]},
                    },
                )
            if resp.status_code != 200:
                logger.error("Firecrawl search error %s: %s", resp.status_code, resp.text[:200])
                return []
            data = resp.json()
            results = data.get("data", {})
            # Handle both formats: {web: [...]} and [...]
            if isinstance(results, dict):
                results = results.get("web", [])
            if not isinstance(results, list):
                return []
            return results
        except Exception as exc:
            logger.error("Web search failed: %s", exc)
            return []

    async def scrape(self, url: str) -> str:
        """Scrape a single URL and return markdown content."""
        try:
            client = _get_http_client()
            resp = await client.post(
                    f"{self._base}/v1/scrape",
                    headers=self._headers,
                    json={"url": url, "formats": ["markdown"]},
                )
            if resp.status_code != 200:
                return ""
            data = resp.json()
            return data.get("data", {}).get("markdown", "")
        except Exception as exc:
            logger.error("Scrape failed for %s: %s", url, exc)
            return ""

    @staticmethod
    def format_results(results: list[dict]) -> str:
        """Format search results into a context string for the LLM."""
        if not results:
            return "[No web results found]"

        parts: list[str] = []
        for i, r in enumerate(results, 1):
            title = r.get("title", "Untitled")
            url = r.get("url", "")
            desc = r.get("description", "")
            markdown = r.get("markdown", "")

            # Truncate markdown to keep context manageable
            if markdown and len(markdown) > 5000:
                markdown = markdown[:5000] + "\n\n[... truncated]"

            part = f"### Source {i}: {title}\n**URL:** {url}\n"
            if desc:
                part += f"**Summary:** {desc}\n"
            if markdown:
                part += f"\n{markdown}\n"
            parts.append(part)

        return "\n---\n\n".join(parts)
