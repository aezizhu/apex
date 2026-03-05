"""Research report templates for the Apex swarm engine."""

from __future__ import annotations

from pathlib import Path

_TEMPLATE_DIR = Path(__file__).parent

_TEMPLATE_FILES: dict[str, str] = {
    "technology_deep_dive": "technology_deep_dive.md",
    "market_industry_analysis": "market_industry_analysis.md",
    "current_events_investigation": "current_events_investigation.md",
    "comparative_analysis": "comparative_analysis.md",
    "historical_trend_analysis": "historical_trend_analysis.md",
}

_TEMPLATE_DESCRIPTIONS: dict[str, str] = {
    "technology_deep_dive": (
        "In-depth analysis of a specific technology, scientific breakthrough, or technical domain. "
        "Covers architecture, key players, applications, challenges, and future trajectory."
    ),
    "market_industry_analysis": (
        "Comprehensive analysis of a market sector or industry vertical. "
        "Includes market sizing, competitive landscape, SWOT/PEST analysis, regional breakdown, and forecasts."
    ),
    "current_events_investigation": (
        "Multi-perspective investigation of a current event or unfolding situation. "
        "Features stakeholder mapping, fact-checking, sentiment analysis, and impact assessment."
    ),
    "comparative_analysis": (
        "Side-by-side evaluation of products, technologies, strategies, or entities. "
        "Includes feature comparison matrices, performance benchmarks, cost-benefit analysis, and verdicts."
    ),
    "historical_trend_analysis": (
        "Longitudinal analysis of how a topic or phenomenon has evolved over time. "
        "Covers evolution timeline, driving forces, turning points, data trends, and future projections."
    ),
}


def get_all_templates() -> dict[str, str]:
    """Return a mapping of template name to its full markdown content."""
    templates: dict[str, str] = {}
    for name, filename in _TEMPLATE_FILES.items():
        path = _TEMPLATE_DIR / filename
        templates[name] = path.read_text(encoding="utf-8")
    return templates


def get_template(name: str) -> str | None:
    """Return the markdown content of a single template by name, or None if not found."""
    filename = _TEMPLATE_FILES.get(name)
    if filename is None:
        return None
    path = _TEMPLATE_DIR / filename
    return path.read_text(encoding="utf-8")


def get_template_descriptions() -> dict[str, str]:
    """Return brief descriptions of each template for LLM-based template selection."""
    return dict(_TEMPLATE_DESCRIPTIONS)
