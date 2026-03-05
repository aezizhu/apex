"""Web search client using Firecrawl API for real research data."""

from __future__ import annotations

import logging
from typing import Any

import httpx

from ..config import FIRECRAWL_BASE_URL

logger = logging.getLogger("apex.swarm.search")


class WebSearchClient:
    """Async wrapper around the Firecrawl v2 search + scrape endpoints."""

    def __init__(self) -> None:
        self._base = FIRECRAWL_BASE_URL

    async def search(self, query: str, limit: int = 5) -> list[dict[str, Any]]:
        """Search the web and return results with scraped markdown content."""
        try:
            async with httpx.AsyncClient(timeout=30.0) as client:
                resp = await client.post(
                    f"{self._base}/v2/search",
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
            async with httpx.AsyncClient(timeout=30.0) as client:
                resp = await client.post(
                    f"{self._base}/v2/scrape",
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
