"""
Rich HTML report renderer for Apex research reports.

Converts a DocumentIR into a self-contained, interactive HTML document with:
- Sidebar table of contents
- Hero section with KPI cards
- Chart.js visualizations (bar, line, pie, radar)
- Styled data tables, SWOT tables (2x2 grid), PEST tables
- Callout boxes (info, warning, danger, success)
- KPI dashboard grid
- Professional typography with heading hierarchy
- Theme token-driven color schemes
- Print/PDF-friendly CSS via @media print
- All CSS/JS inlined for standalone export
"""

from __future__ import annotations

import html
import json
import re
import uuid
from typing import Any, Dict, List, Optional

from .report_ir import (
    AGENT_TITLES,
    ALLOWED_BLOCK_TYPES,
    DocumentIR,
)


class ReportRenderer:
    """
    Renders a DocumentIR dict or object into self-contained HTML.

    Usage:
        renderer = ReportRenderer()
        html_str = renderer.render(document_ir)
    """

    def __init__(self) -> None:
        self._chart_configs: List[Dict[str, Any]] = []

    # ================================================================
    # Public API
    # ================================================================

    def render(self, doc: Any) -> str:
        """
        Render a DocumentIR to a complete HTML document string.

        Args:
            doc: Either a DocumentIR instance or a raw dict.

        Returns:
            Self-contained HTML string.
        """
        self._chart_configs = []

        if hasattr(doc, "to_dict"):
            data = doc.to_dict()
        elif isinstance(doc, dict):
            data = doc
        else:
            raise TypeError(f"Expected DocumentIR or dict, got {type(doc).__name__}")

        meta = data.get("metadata", {})
        theme = data.get("themeTokens", {})
        toc_cfg = data.get("toc", {})
        chapters = data.get("chapters", [])

        title = meta.get("title", "Research Report")
        subtitle = meta.get("subtitle", "")
        query = meta.get("query", "")
        generated_at = meta.get("generatedAt", "")

        css = self._build_css(theme)
        body = self._build_body(title, subtitle, query, generated_at, chapters, toc_cfg, theme)
        scripts = self._build_scripts()

        return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{_esc(title)}</title>
<style>{css}</style>
</head>
<body>
{body}
{scripts}
</body>
</html>"""

    # ================================================================
    # Body assembly
    # ================================================================

    def _build_body(
        self,
        title: str,
        subtitle: str,
        query: str,
        generated_at: str,
        chapters: List[Dict[str, Any]],
        toc_cfg: Dict[str, Any],
        theme: Dict[str, Any],
    ) -> str:
        header = self._render_header(title, subtitle, generated_at)
        toc = self._render_toc(chapters, toc_cfg)
        main_content = self._render_chapters(chapters)

        return f"""
<div class="report-container">
  <aside class="report-sidebar">
    {toc}
  </aside>
  <main class="report-main">
    {header}
    <div class="report-content">
      {main_content}
    </div>
  </main>
</div>"""

    def _render_header(self, title: str, subtitle: str, generated_at: str) -> str:
        sub_html = f'<p class="report-subtitle">{_esc(subtitle)}</p>' if subtitle else ""
        date_str = ""
        if generated_at:
            date_str = f'<span class="report-date">{_esc(generated_at[:10])}</span>'

        return f"""
<header class="report-header">
  <h1 class="report-title">{_esc(title)}</h1>
  {sub_html}
  <div class="report-meta">
    {date_str}
    <button class="print-btn" onclick="window.print()" title="Print / Export PDF">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="6 9 6 2 18 2 18 9"/><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"/><rect x="6" y="14" width="12" height="8"/>
      </svg>
      Print
    </button>
  </div>
</header>"""

    # ================================================================
    # Table of contents
    # ================================================================

    def _render_toc(self, chapters: List[Dict[str, Any]], toc_cfg: Dict[str, Any]) -> str:
        max_depth = toc_cfg.get("depth", 3)
        toc_title = toc_cfg.get("title", "Table of Contents")
        entries: List[str] = []

        for chapter in chapters:
            blocks = chapter.get("blocks", [])
            ch_anchor = chapter.get("anchor", "")
            for block in blocks:
                if block.get("type") != "heading":
                    continue
                level = block.get("level", 2)
                if level > max_depth:
                    continue
                text = block.get("text", "")
                anchor = block.get("anchor", ch_anchor)
                indent = "toc-indent" if level > 2 else ""
                entries.append(
                    f'<a class="toc-link {indent}" href="#{_esc(anchor)}" data-level="{level}">'
                    f'{_esc(text)}</a>'
                )

        links = "\n".join(entries) if entries else '<span class="toc-empty">No sections</span>'
        return f"""
<div class="toc-wrapper">
  <h3 class="toc-title">{_esc(toc_title)}</h3>
  <nav class="toc-nav">
    {links}
  </nav>
</div>"""

    # ================================================================
    # Chapters & blocks
    # ================================================================

    def _render_chapters(self, chapters: List[Dict[str, Any]]) -> str:
        parts: List[str] = []
        for chapter in chapters:
            anchor = chapter.get("anchor", "")
            blocks_html = self._render_blocks(chapter.get("blocks", []))
            parts.append(
                f'<section class="chapter" id="{_esc(anchor)}">\n{blocks_html}\n</section>'
            )
        return "\n".join(parts)

    def _render_blocks(self, blocks: List[Dict[str, Any]]) -> str:
        parts: List[str] = []
        for block in blocks:
            rendered = self._render_block(block)
            if rendered:
                parts.append(rendered)
        return "\n".join(parts)

    def _render_block(self, block: Dict[str, Any]) -> str:
        block_type = block.get("type", "")
        dispatch = {
            "heading": self._render_heading,
            "paragraph": self._render_paragraph,
            "list": self._render_list,
            "table": self._render_table,
            "chart": self._render_chart,
            "callout": self._render_callout,
            "kpiGrid": self._render_kpi_grid,
            "swotTable": self._render_swot_table,
            "pestTable": self._render_pest_table,
            "blockquote": self._render_blockquote,
            "agentQuote": self._render_agent_quote,
            "code": self._render_code,
            "math": self._render_math,
            "figure": self._render_figure,
            "toc": lambda b: "",  # TOC rendered separately in sidebar
            "hr": lambda b: "<hr/>",
        }
        renderer = dispatch.get(block_type)
        if renderer:
            try:
                return renderer(block)
            except Exception as e:
                return f'<div class="render-error">Block render error ({_esc(block_type)}): {_esc(str(e))}</div>'
        return f'<div class="render-error">Unknown block type: {_esc(block_type)}</div>'

    # ── Heading ──

    def _render_heading(self, block: Dict[str, Any]) -> str:
        level = min(max(block.get("level", 2), 1), 6)
        text = block.get("text", "")
        anchor = block.get("anchor", "")
        numbering = block.get("numbering", "")
        subtitle = block.get("subtitle", "")

        num_html = f'<span class="heading-num">{_esc(numbering)}</span>' if numbering else ""
        sub_html = f'<span class="heading-subtitle">{_esc(subtitle)}</span>' if subtitle else ""
        anchor_attr = f' id="{_esc(anchor)}"' if anchor else ""

        return (
            f'<h{level}{anchor_attr} class="section-heading">'
            f'{num_html}{_esc(text)}{sub_html}'
            f'</h{level}>'
        )

    # ── Paragraph ──

    def _render_paragraph(self, block: Dict[str, Any]) -> str:
        inlines = block.get("inlines", [])
        align = block.get("align", "")
        style = f' style="text-align:{_esc(align)}"' if align else ""
        content = self._render_inlines(inlines)
        return f"<p{style}>{content}</p>"

    def _render_inlines(self, inlines: List[Dict[str, Any]]) -> str:
        parts: List[str] = []
        for run in inlines:
            text = _esc(run.get("text", ""))
            marks = run.get("marks", [])
            for mark in reversed(marks or []):
                text = self._apply_mark(text, mark)
            parts.append(text)
        return "".join(parts)

    def _apply_mark(self, text: str, mark: Dict[str, Any]) -> str:
        mt = mark.get("type", "")
        if mt == "bold":
            return f"<strong>{text}</strong>"
        if mt == "italic":
            return f"<em>{text}</em>"
        if mt == "underline":
            return f'<span class="underline">{text}</span>'
        if mt == "strike":
            return f"<del>{text}</del>"
        if mt == "code":
            return f"<code>{text}</code>"
        if mt == "link":
            href = mark.get("href", "")
            title = mark.get("title", "")
            t = f' title="{_esc(title)}"' if title else ""
            return f'<a href="{_esc(href)}"{t} target="_blank" rel="noopener">{text}</a>'
        if mt == "color":
            val = mark.get("value", "")
            return f'<span style="color:{_esc(str(val))}">{text}</span>'
        if mt == "highlight":
            return f'<mark>{text}</mark>'
        if mt == "subscript":
            return f"<sub>{text}</sub>"
        if mt == "superscript":
            return f"<sup>{text}</sup>"
        if mt == "math":
            return f'<span class="math-inline">\\({text}\\)</span>'
        return text

    # ── List ──

    def _render_list(self, block: Dict[str, Any]) -> str:
        list_type = block.get("listType", "bullet")
        items = block.get("items", [])
        tag = "ol" if list_type == "ordered" else "ul"
        cls = f' class="task-list"' if list_type == "task" else ""

        li_parts: List[str] = []
        for item in items:
            if isinstance(item, list):
                inner = self._render_blocks(item)
            else:
                inner = _esc(str(item))
            li_parts.append(f"<li>{inner}</li>")

        return f"<{tag}{cls}>{''.join(li_parts)}</{tag}>"

    # ── Table ──

    def _render_table(self, block: Dict[str, Any]) -> str:
        rows = block.get("rows", [])
        caption = block.get("caption", "")
        zebra = block.get("zebra", True)
        cls = "data-table zebra" if zebra else "data-table"

        cap_html = f"<caption>{_esc(caption)}</caption>" if caption else ""
        parts: List[str] = [f'<div class="table-wrapper"><table class="{cls}">{cap_html}']

        for r_idx, row in enumerate(rows):
            cells = row.get("cells", []) if isinstance(row, dict) else []
            is_header = r_idx == 0
            if is_header:
                parts.append("<thead><tr>")
            else:
                if r_idx == 1:
                    parts.append("<tbody>")
                parts.append("<tr>")

            for cell in cells:
                if not isinstance(cell, dict):
                    continue
                tag = "th" if is_header else "td"
                colspan = cell.get("colspan", 1)
                rowspan = cell.get("rowspan", 1)
                align = cell.get("align", "")
                attrs: List[str] = []
                if colspan > 1:
                    attrs.append(f'colspan="{colspan}"')
                if rowspan > 1:
                    attrs.append(f'rowspan="{rowspan}"')
                if align:
                    attrs.append(f'style="text-align:{_esc(align)}"')
                attr_str = (" " + " ".join(attrs)) if attrs else ""

                cell_blocks = cell.get("blocks", [])
                cell_content = self._render_blocks(cell_blocks) if cell_blocks else ""
                parts.append(f"<{tag}{attr_str}>{cell_content}</{tag}>")

            parts.append("</tr>")
            if is_header:
                parts.append("</thead>")

        parts.append("</tbody></table></div>")
        return "\n".join(parts)

    # ── Chart (Chart.js) ──

    def _render_chart(self, block: Dict[str, Any]) -> str:
        chart_type = block.get("chartType", "bar")
        chart_data = block.get("chartData", {})
        title = block.get("title", "")
        caption = block.get("caption", "")
        chart_id = f"chart-{uuid.uuid4().hex[:8]}"
        config_id = f"config-{chart_id}"

        config = {
            "type": chart_type,
            "data": chart_data,
            "options": {
                "responsive": True,
                "maintainAspectRatio": True,
                "plugins": {
                    "legend": {"display": True, "position": "bottom"},
                },
            },
        }

        if title:
            config["options"]["plugins"]["title"] = {"display": True, "text": title}

        self._chart_configs.append({"id": chart_id, "config": config})

        title_html = f'<h4 class="chart-title">{_esc(title)}</h4>' if title else ""
        caption_html = f'<p class="chart-caption">{_esc(caption)}</p>' if caption else ""

        return f"""
<div class="chart-container">
  {title_html}
  <canvas id="{chart_id}" data-config-id="{config_id}"></canvas>
  <script type="application/json" id="{config_id}">{json.dumps(config, ensure_ascii=False)}</script>
  {caption_html}
</div>"""

    # ── Callout ──

    def _render_callout(self, block: Dict[str, Any]) -> str:
        tone = block.get("tone", "info")
        title = block.get("title", "")
        blocks = block.get("blocks", [])

        icons = {"info": "\u2139\ufe0f", "warning": "\u26a0\ufe0f", "success": "\u2705", "danger": "\u274c"}
        icon = icons.get(tone, "\u2139\ufe0f")
        title_html = f'<strong class="callout-title">{icon} {_esc(title)}</strong>' if title else f'<span class="callout-icon">{icon}</span>'
        inner = self._render_blocks(blocks)

        return f"""
<div class="callout callout-{_esc(tone)}">
  {title_html}
  <div class="callout-body">{inner}</div>
</div>"""

    # ── KPI Grid ──

    def _render_kpi_grid(self, block: Dict[str, Any]) -> str:
        items = block.get("items", [])
        cols = block.get("cols", 4)

        cards: List[str] = []
        for item in items:
            label = item.get("label", "")
            value = item.get("value", "")
            unit = item.get("unit", "")
            delta = item.get("delta", "")
            delta_tone = item.get("deltaTone", "neutral")

            delta_html = ""
            if delta:
                arrow = {"up": "\u2191", "down": "\u2193", "neutral": "\u2192"}.get(delta_tone, "")
                delta_html = f'<span class="kpi-delta kpi-delta-{_esc(delta_tone)}">{arrow} {_esc(delta)}</span>'

            unit_html = f'<span class="kpi-unit">{_esc(unit)}</span>' if unit else ""
            cards.append(f"""
<div class="kpi-card">
  <div class="kpi-label">{_esc(label)}</div>
  <div class="kpi-value">{_esc(value)}{unit_html}</div>
  {delta_html}
</div>""")

        return f'<div class="kpi-grid" style="--kpi-cols:{cols}">{"".join(cards)}</div>'

    # ── SWOT Table ──

    def _render_swot_table(self, block: Dict[str, Any]) -> str:
        title = block.get("title", "SWOT Analysis")
        quadrants = [
            ("strengths", "Strengths", "swot-s"),
            ("weaknesses", "Weaknesses", "swot-w"),
            ("opportunities", "Opportunities", "swot-o"),
            ("threats", "Threats", "swot-t"),
        ]

        cells: List[str] = []
        for key, label, cls in quadrants:
            items = block.get(key, [])
            items_html = self._render_swot_items(items)
            cells.append(f"""
<div class="swot-cell {cls}">
  <h4 class="swot-label">{_esc(label)}</h4>
  <div class="swot-items">{items_html}</div>
</div>""")

        summary = block.get("summary", "")
        summary_html = f'<p class="swot-summary">{_esc(summary)}</p>' if summary else ""

        return f"""
<div class="swot-table">
  <h3 class="swot-title">{_esc(title)}</h3>
  {summary_html}
  <div class="swot-grid">{"".join(cells)}</div>
</div>"""

    def _render_swot_items(self, items: List[Any]) -> str:
        parts: List[str] = []
        for item in items:
            if isinstance(item, str):
                parts.append(f'<div class="swot-item"><span>{_esc(item)}</span></div>')
            elif isinstance(item, dict):
                title = item.get("title") or item.get("label") or item.get("text") or ""
                detail = item.get("detail") or item.get("description") or ""
                impact = item.get("impact", "")
                impact_html = f'<span class="swot-impact">{_esc(impact)}</span>' if impact else ""
                detail_html = f'<span class="swot-detail">{_esc(detail)}</span>' if detail else ""
                parts.append(f"""
<div class="swot-item">
  <span class="swot-item-title">{_esc(title)}</span>
  {detail_html}
  {impact_html}
</div>""")
        return "".join(parts)

    # ── PEST Table ──

    def _render_pest_table(self, block: Dict[str, Any]) -> str:
        title = block.get("title", "PEST Analysis")
        factors = [
            ("political", "Political", "pest-p"),
            ("economic", "Economic", "pest-e"),
            ("social", "Social", "pest-s"),
            ("technological", "Technological", "pest-t"),
        ]

        strips: List[str] = []
        for key, label, cls in factors:
            items = block.get(key, [])
            if items is None:
                continue
            items_html = self._render_pest_items(items)
            strips.append(f"""
<div class="pest-strip {cls}">
  <h4 class="pest-label">{_esc(label)}</h4>
  <div class="pest-items">{items_html}</div>
</div>""")

        summary = block.get("summary", "")
        summary_html = f'<p class="pest-summary">{_esc(summary)}</p>' if summary else ""

        return f"""
<div class="pest-table">
  <h3 class="pest-title">{_esc(title)}</h3>
  {summary_html}
  <div class="pest-container">{"".join(strips)}</div>
</div>"""

    def _render_pest_items(self, items: List[Any]) -> str:
        parts: List[str] = []
        for item in items:
            if isinstance(item, str):
                parts.append(f'<div class="pest-item"><span>{_esc(item)}</span></div>')
            elif isinstance(item, dict):
                title = item.get("title") or item.get("label") or item.get("text") or ""
                detail = item.get("detail") or item.get("description") or ""
                trend = item.get("trend", "")
                trend_html = f'<span class="pest-trend">{_esc(trend)}</span>' if trend else ""
                detail_html = f'<span class="pest-detail">{_esc(detail)}</span>' if detail else ""
                parts.append(f"""
<div class="pest-item">
  <span class="pest-item-title">{_esc(title)}</span>
  {detail_html}
  {trend_html}
</div>""")
        return "".join(parts)

    # ── Blockquote ──

    def _render_blockquote(self, block: Dict[str, Any]) -> str:
        inner = self._render_blocks(block.get("blocks", []))
        return f"<blockquote>{inner}</blockquote>"

    # ── Agent Quote ──

    def _render_agent_quote(self, block: Dict[str, Any]) -> str:
        agent = block.get("agent", "")
        title = block.get("title", AGENT_TITLES.get(agent, agent))
        inner = self._render_blocks(block.get("blocks", []))
        return f"""
<div class="agent-quote agent-quote-{_esc(agent)}">
  <div class="agent-quote-header">
    <span class="agent-quote-icon">{_agent_icon(agent)}</span>
    <span class="agent-quote-title">{_esc(title)}</span>
  </div>
  <div class="agent-quote-body">{inner}</div>
</div>"""

    # ── Code ──

    def _render_code(self, block: Dict[str, Any]) -> str:
        lang = block.get("lang", "")
        content = block.get("content", "")
        caption = block.get("caption", "")
        lang_attr = f' class="language-{_esc(lang)}"' if lang else ""
        cap_html = f'<div class="code-caption">{_esc(caption)}</div>' if caption else ""
        return f"<pre><code{lang_attr}>{_esc(content)}</code></pre>{cap_html}"

    # ── Math ──

    def _render_math(self, block: Dict[str, Any]) -> str:
        latex = self._esc(block.get("latex", ""))
        display = block.get("displayMode", True)
        if display:
            return f'<div class="math-display">\\[{latex}\\]</div>'
        return f'<span class="math-inline">\\({latex}\\)</span>'

    # ── Figure ──

    def _render_figure(self, block: Dict[str, Any]) -> str:
        img = block.get("img", {})
        src = img.get("src", "")
        alt = img.get("alt", "")
        caption = block.get("caption", "")
        cap_html = f"<figcaption>{_esc(caption)}</figcaption>" if caption else ""
        return f"""
<figure>
  <img src="{_esc(src)}" alt="{_esc(alt)}" loading="lazy"/>
  {cap_html}
</figure>"""

    # ================================================================
    # CSS
    # ================================================================

    def _build_css(self, theme: Dict[str, Any]) -> str:
        colors = theme.get("colors", {})
        fonts = theme.get("fonts", {})

        # Resolve colors with defaults
        c = lambda key, default: colors.get(key, default)
        primary = c("primary", "#6366f1")
        secondary = c("secondary", "#64748b")
        bg = c("bg", "#0e0f14")
        text_color = c("text", "#e2e8f0")
        card = c("card", "#1a1b23")
        border = c("border", "#2a2b35")
        accent1 = c("accent1", "#0ea5e9")
        accent2 = c("accent2", "#22c55e")
        accent3 = c("accent3", "#f59e0b")
        accent4 = c("accent4", "#ef4444")

        font_body = fonts.get("body", "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif")
        font_heading = fonts.get("heading", "'Inter', -apple-system, sans-serif")
        font_mono = fonts.get("mono", "'JetBrains Mono', 'Fira Code', monospace")

        return f"""
/* ═══ Apex Report — Generated Styles ═══ */
:root {{
  --rp-primary: {primary};
  --rp-secondary: {secondary};
  --rp-bg: {bg};
  --rp-bg-surface: color-mix(in srgb, {bg}, #fff 4%);
  --rp-bg-card: {card};
  --rp-bg-raised: color-mix(in srgb, {card}, #fff 5%);
  --rp-text: {text_color};
  --rp-text-secondary: color-mix(in srgb, {text_color}, transparent 40%);
  --rp-text-muted: color-mix(in srgb, {text_color}, transparent 65%);
  --rp-border: {border};
  --rp-accent1: {accent1};
  --rp-accent2: {accent2};
  --rp-accent3: {accent3};
  --rp-accent4: {accent4};
  --rp-font-body: {font_body};
  --rp-font-heading: {font_heading};
  --rp-font-mono: {font_mono};
  --rp-radius: 8px;
  --rp-radius-sm: 4px;
}}

*, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}

body {{
  font-family: var(--rp-font-body);
  background: var(--rp-bg);
  color: var(--rp-text);
  line-height: 1.7;
  font-size: 15px;
  -webkit-font-smoothing: antialiased;
}}

/* ── Layout ── */
.report-container {{
  display: grid;
  grid-template-columns: 260px 1fr;
  min-height: 100vh;
}}

.report-sidebar {{
  position: sticky;
  top: 0;
  height: 100vh;
  overflow-y: auto;
  border-right: 1px solid var(--rp-border);
  background: var(--rp-bg-surface);
  padding: 24px 0;
}}

.report-main {{
  max-width: 900px;
  margin: 0 auto;
  padding: 32px 48px 80px;
  width: 100%;
}}

/* ── Header ── */
.report-header {{
  margin-bottom: 40px;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--rp-border);
}}
.report-title {{
  font-family: var(--rp-font-heading);
  font-size: 2.2rem;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
  color: var(--rp-text);
  margin-bottom: 8px;
}}
.report-subtitle {{
  font-size: 1.1rem;
  color: var(--rp-text-secondary);
  margin-bottom: 12px;
}}
.report-meta {{
  display: flex;
  align-items: center;
  gap: 12px;
}}
.report-date {{
  font-size: 0.85rem;
  color: var(--rp-text-muted);
  font-family: var(--rp-font-mono);
}}
.print-btn {{
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  font-size: 0.8rem;
  color: var(--rp-text-muted);
  background: transparent;
  border: 1px solid var(--rp-border);
  border-radius: var(--rp-radius-sm);
  cursor: pointer;
  transition: all 120ms;
  font-family: var(--rp-font-body);
}}
.print-btn:hover {{
  color: var(--rp-text);
  border-color: var(--rp-text-muted);
}}

/* ── TOC ── */
.toc-wrapper {{ padding: 0 16px; }}
.toc-title {{
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--rp-text-muted);
  padding: 0 12px;
  margin-bottom: 12px;
}}
.toc-nav {{
  display: flex;
  flex-direction: column;
}}
.toc-link {{
  display: block;
  padding: 5px 12px;
  font-size: 0.85rem;
  color: var(--rp-text-secondary);
  text-decoration: none;
  border-radius: var(--rp-radius-sm);
  border-left: 2px solid transparent;
  transition: all 100ms;
  line-height: 1.4;
}}
.toc-link:hover {{
  color: var(--rp-text);
  background: rgba(255,255,255,0.04);
  border-left-color: var(--rp-primary);
}}
.toc-link.toc-indent {{
  padding-left: 24px;
  font-size: 0.8rem;
}}

/* ── Headings ── */
.section-heading {{
  font-family: var(--rp-font-heading);
  font-weight: 700;
  letter-spacing: -0.02em;
  color: var(--rp-text);
  scroll-margin-top: 16px;
}}
h2.section-heading {{ font-size: 1.6rem; margin: 48px 0 16px; padding-top: 24px; border-top: 1px solid var(--rp-border); }}
h3.section-heading {{ font-size: 1.25rem; margin: 32px 0 12px; }}
h4.section-heading {{ font-size: 1.05rem; margin: 24px 0 8px; }}
h5.section-heading, h6.section-heading {{ font-size: 0.95rem; margin: 16px 0 6px; }}
.heading-num {{
  color: var(--rp-primary);
  margin-right: 8px;
  font-weight: 600;
}}
.heading-subtitle {{
  display: block;
  font-size: 0.85rem;
  font-weight: 400;
  color: var(--rp-text-muted);
  margin-top: 4px;
}}

/* ── Paragraph ── */
p {{
  margin-bottom: 12px;
  color: var(--rp-text-secondary);
}}
p:last-child {{ margin-bottom: 0; }}
strong {{ font-weight: 600; color: var(--rp-text); }}
a {{ color: var(--rp-primary); text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
mark {{ background: rgba(245,158,11,0.25); color: var(--rp-text); padding: 1px 3px; border-radius: 2px; }}
.underline {{ text-decoration: underline; text-underline-offset: 2px; }}
code {{
  font-family: var(--rp-font-mono);
  font-size: 0.88em;
  background: rgba(99,102,241,0.12);
  color: var(--rp-primary);
  padding: 2px 5px;
  border-radius: var(--rp-radius-sm);
}}

/* ── Lists ── */
ul, ol {{ margin: 8px 0 16px; padding-left: 24px; }}
li {{ margin-bottom: 4px; color: var(--rp-text-secondary); }}
li p {{ margin-bottom: 4px; }}

/* ── Code blocks ── */
pre {{
  background: var(--rp-bg);
  border: 1px solid var(--rp-border);
  border-radius: var(--rp-radius);
  padding: 16px 20px;
  margin: 12px 0;
  overflow-x: auto;
  font-size: 0.85rem;
  line-height: 1.6;
}}
pre code {{
  background: none;
  padding: 0;
  color: var(--rp-text-secondary);
  font-family: var(--rp-font-mono);
}}
.code-caption {{
  font-size: 0.8rem;
  color: var(--rp-text-muted);
  margin-top: -8px;
  margin-bottom: 12px;
}}

/* ── Tables ── */
.table-wrapper {{ overflow-x: auto; margin: 16px 0; }}
.data-table {{
  width: 100%;
  border-collapse: collapse;
  font-size: 0.88rem;
}}
.data-table th {{
  text-align: left;
  padding: 10px 14px;
  font-weight: 600;
  font-size: 0.8rem;
  letter-spacing: 0.02em;
  color: var(--rp-text);
  background: var(--rp-bg-raised);
  border-bottom: 2px solid var(--rp-border);
  white-space: nowrap;
}}
.data-table td {{
  padding: 8px 14px;
  color: var(--rp-text-secondary);
  border-bottom: 1px solid var(--rp-border);
  vertical-align: top;
}}
.data-table.zebra tbody tr:nth-child(even) {{
  background: rgba(255,255,255,0.015);
}}
.data-table tbody tr:hover {{
  background: rgba(255,255,255,0.03);
}}
.data-table caption {{
  font-size: 0.85rem;
  color: var(--rp-text-muted);
  margin-bottom: 8px;
  text-align: left;
}}

/* ── Charts ── */
.chart-container {{
  background: var(--rp-bg-card);
  border: 1px solid var(--rp-border);
  border-radius: var(--rp-radius);
  padding: 20px;
  margin: 20px 0;
}}
.chart-title {{
  font-size: 0.95rem;
  font-weight: 600;
  margin-bottom: 12px;
  color: var(--rp-text);
}}
.chart-caption {{
  font-size: 0.8rem;
  color: var(--rp-text-muted);
  margin-top: 8px;
}}

/* ── Callouts ── */
.callout {{
  border-radius: var(--rp-radius);
  padding: 16px 20px;
  margin: 16px 0;
  border-left: 4px solid;
}}
.callout-info    {{ background: rgba(14,165,233,0.08);  border-color: var(--rp-accent1); }}
.callout-warning {{ background: rgba(245,158,11,0.08);  border-color: var(--rp-accent3); }}
.callout-success {{ background: rgba(34,197,94,0.08);   border-color: var(--rp-accent2); }}
.callout-danger  {{ background: rgba(239,68,68,0.08);   border-color: var(--rp-accent4); }}
.callout-title {{
  display: block;
  font-size: 0.9rem;
  margin-bottom: 8px;
}}
.callout-info    .callout-title {{ color: var(--rp-accent1); }}
.callout-warning .callout-title {{ color: var(--rp-accent3); }}
.callout-success .callout-title {{ color: var(--rp-accent2); }}
.callout-danger  .callout-title {{ color: var(--rp-accent4); }}
.callout-body p {{ margin-bottom: 6px; }}

/* ── KPI Grid ── */
.kpi-grid {{
  display: grid;
  grid-template-columns: repeat(var(--kpi-cols, 4), 1fr);
  gap: 12px;
  margin: 20px 0;
}}
.kpi-card {{
  background: var(--rp-bg-card);
  border: 1px solid var(--rp-border);
  border-radius: var(--rp-radius);
  padding: 16px 20px;
  text-align: center;
}}
.kpi-label {{
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--rp-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 6px;
}}
.kpi-value {{
  font-size: 1.8rem;
  font-weight: 700;
  font-family: var(--rp-font-heading);
  color: var(--rp-text);
  line-height: 1.2;
}}
.kpi-unit {{
  font-size: 0.7em;
  font-weight: 400;
  color: var(--rp-text-muted);
  margin-left: 2px;
}}
.kpi-delta {{
  font-size: 0.8rem;
  font-weight: 500;
  margin-top: 4px;
  display: inline-block;
}}
.kpi-delta-up   {{ color: var(--rp-accent2); }}
.kpi-delta-down {{ color: var(--rp-accent4); }}
.kpi-delta-neutral {{ color: var(--rp-text-muted); }}

/* ── SWOT Table ── */
.swot-table {{ margin: 24px 0; }}
.swot-title {{
  font-size: 1.1rem;
  font-weight: 700;
  margin-bottom: 8px;
  color: var(--rp-text);
}}
.swot-summary {{
  font-size: 0.88rem;
  color: var(--rp-text-secondary);
  margin-bottom: 16px;
}}
.swot-grid {{
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 2px;
  border-radius: var(--rp-radius);
  overflow: hidden;
  border: 1px solid var(--rp-border);
}}
.swot-cell {{
  padding: 16px 20px;
  min-height: 120px;
}}
.swot-s {{ background: rgba(34,197,94,0.06); }}
.swot-w {{ background: rgba(239,68,68,0.06); }}
.swot-o {{ background: rgba(14,165,233,0.06); }}
.swot-t {{ background: rgba(245,158,11,0.06); }}
.swot-label {{
  font-size: 0.8rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 10px;
}}
.swot-s .swot-label {{ color: var(--rp-accent2); }}
.swot-w .swot-label {{ color: var(--rp-accent4); }}
.swot-o .swot-label {{ color: var(--rp-accent1); }}
.swot-t .swot-label {{ color: var(--rp-accent3); }}
.swot-item {{
  font-size: 0.85rem;
  padding: 6px 0;
  border-bottom: 1px solid rgba(255,255,255,0.04);
  color: var(--rp-text-secondary);
}}
.swot-item:last-child {{ border-bottom: none; }}
.swot-item-title {{ font-weight: 500; color: var(--rp-text); display: block; }}
.swot-detail {{ font-size: 0.8rem; color: var(--rp-text-muted); display: block; margin-top: 2px; }}
.swot-impact {{
  display: inline-block;
  font-size: 0.7rem;
  font-weight: 500;
  padding: 1px 6px;
  border-radius: 3px;
  background: rgba(255,255,255,0.06);
  color: var(--rp-text-muted);
  margin-top: 4px;
}}

/* ── PEST Table ── */
.pest-table {{ margin: 24px 0; }}
.pest-title {{
  font-size: 1.1rem;
  font-weight: 700;
  margin-bottom: 8px;
  color: var(--rp-text);
}}
.pest-summary {{
  font-size: 0.88rem;
  color: var(--rp-text-secondary);
  margin-bottom: 16px;
}}
.pest-container {{
  display: flex;
  flex-direction: column;
  gap: 2px;
  border: 1px solid var(--rp-border);
  border-radius: var(--rp-radius);
  overflow: hidden;
}}
.pest-strip {{
  padding: 16px 20px;
}}
.pest-p {{ background: rgba(99,102,241,0.06); }}
.pest-e {{ background: rgba(34,197,94,0.06); }}
.pest-s {{ background: rgba(14,165,233,0.06); }}
.pest-t {{ background: rgba(245,158,11,0.06); }}
.pest-label {{
  font-size: 0.8rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 10px;
}}
.pest-p .pest-label {{ color: var(--rp-primary); }}
.pest-e .pest-label {{ color: var(--rp-accent2); }}
.pest-s .pest-label {{ color: var(--rp-accent1); }}
.pest-t .pest-label {{ color: var(--rp-accent3); }}
.pest-item {{
  font-size: 0.85rem;
  padding: 6px 0;
  border-bottom: 1px solid rgba(255,255,255,0.04);
  color: var(--rp-text-secondary);
}}
.pest-item:last-child {{ border-bottom: none; }}
.pest-item-title {{ font-weight: 500; color: var(--rp-text); display: block; }}
.pest-detail {{ font-size: 0.8rem; color: var(--rp-text-muted); display: block; margin-top: 2px; }}
.pest-trend {{
  display: inline-block;
  font-size: 0.7rem;
  font-weight: 500;
  padding: 1px 6px;
  border-radius: 3px;
  background: rgba(255,255,255,0.06);
  color: var(--rp-text-muted);
  margin-top: 4px;
}}

/* ── Blockquote ── */
blockquote {{
  border-left: 3px solid var(--rp-primary);
  padding: 12px 20px;
  margin: 16px 0;
  background: rgba(99,102,241,0.06);
  border-radius: 0 var(--rp-radius) var(--rp-radius) 0;
}}
blockquote p {{ color: var(--rp-text-secondary); }}

/* ── Agent Quote ── */
.agent-quote {{
  border-radius: var(--rp-radius);
  padding: 16px 20px;
  margin: 16px 0;
  border: 1px solid var(--rp-border);
  background: var(--rp-bg-card);
}}
.agent-quote-header {{
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}}
.agent-quote-icon {{ font-size: 1.1rem; }}
.agent-quote-title {{
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--rp-text);
}}
.agent-quote-coordinator {{ border-left: 3px solid #5E6AD2; }}
.agent-quote-researcher  {{ border-left: 3px solid #5B9AF5; }}
.agent-quote-analyst      {{ border-left: 3px solid #9B8AEE; }}
.agent-quote-fact_checker {{ border-left: 3px solid #E8B931; }}
.agent-quote-writer       {{ border-left: 3px solid #3ECF8E; }}

/* ── Figure ── */
figure {{
  margin: 20px 0;
  text-align: center;
}}
figure img {{
  max-width: 100%;
  height: auto;
  border-radius: var(--rp-radius);
  border: 1px solid var(--rp-border);
}}
figcaption {{
  font-size: 0.82rem;
  color: var(--rp-text-muted);
  margin-top: 8px;
}}

/* ── Math ── */
.math-display {{
  margin: 16px 0;
  text-align: center;
  overflow-x: auto;
}}

/* ── HR ── */
hr {{
  border: none;
  height: 1px;
  background: var(--rp-border);
  margin: 32px 0;
}}

/* ── Chapter ── */
.chapter {{ margin-bottom: 16px; }}

/* ── Error ── */
.render-error {{
  padding: 12px 16px;
  background: rgba(239,68,68,0.08);
  border: 1px dashed var(--rp-accent4);
  border-radius: var(--rp-radius);
  font-size: 0.85rem;
  color: var(--rp-accent4);
  margin: 8px 0;
}}

/* ── Responsive ── */
@media (max-width: 900px) {{
  .report-container {{
    grid-template-columns: 1fr;
  }}
  .report-sidebar {{
    position: static;
    height: auto;
    border-right: none;
    border-bottom: 1px solid var(--rp-border);
    padding: 16px 0;
  }}
  .toc-nav {{
    flex-direction: row;
    flex-wrap: wrap;
    gap: 4px;
    padding: 0 12px;
  }}
  .toc-link {{
    flex: 0 0 auto;
    border-left: none;
    border-bottom: 2px solid transparent;
  }}
  .toc-link.toc-indent {{ padding-left: 12px; }}
  .report-main {{
    padding: 20px 16px 60px;
  }}
  .swot-grid {{ grid-template-columns: 1fr; }}
  .kpi-grid {{ grid-template-columns: repeat(2, 1fr); }}
}}

/* ── Print ── */
@media print {{
  body {{
    background: #fff;
    color: #111;
    font-size: 11pt;
  }}
  .report-container {{
    display: block;
  }}
  .report-sidebar {{
    display: none;
  }}
  .report-main {{
    max-width: 100%;
    padding: 0;
  }}
  .print-btn {{ display: none; }}
  .report-header {{
    border-bottom: 2px solid #111;
    margin-bottom: 24px;
  }}
  .report-title {{ color: #111; }}
  .chapter {{ page-break-inside: avoid; }}
  h2.section-heading {{ page-break-after: avoid; border-top-color: #ccc; }}
  .callout {{ border-color: #888; background: #f5f5f5; }}
  .kpi-card {{ background: #f5f5f5; border-color: #ddd; }}
  .swot-cell {{ background: #f8f8f8; }}
  .pest-strip {{ background: #f8f8f8; }}
  .chart-container {{ page-break-inside: avoid; }}
  blockquote {{ background: #f5f5f5; border-left-color: #888; }}
  .agent-quote {{ background: #f5f5f5; }}
  .data-table th {{ background: #eee; }}
  * {{ animation: none !important; transition: none !important; }}
  a {{ color: #111; text-decoration: underline; }}
  a[href^="http"]::after {{ content: " (" attr(href) ")"; font-size: 0.7em; color: #666; }}
  p, li {{ color: #333; }}
  strong {{ color: #111; }}
  .render-error {{ display: none; }}
}}
"""

    # ================================================================
    # Scripts (Chart.js hydration)
    # ================================================================

    def _build_scripts(self) -> str:
        chart_hydration = ""
        if self._chart_configs:
            chart_hydration = """
<script src="https://cdn.jsdelivr.net/npm/chart.js@4/dist/chart.umd.min.js"></script>
<script>
(function() {
  const palette = ['#6366f1','#0ea5e9','#22c55e','#f59e0b','#ef4444','#8b5cf6','#ec4899','#14b8a6','#f97316','#06b6d4','#a855f7','#84cc16'];

  document.querySelectorAll('canvas[data-config-id]').forEach(function(canvas) {
    var configEl = document.getElementById(canvas.dataset.configId);
    if (!configEl) return;
    try {
      var config = JSON.parse(configEl.textContent);
      // Inject palette colors if datasets lack backgroundColor
      if (config.data && config.data.datasets) {
        config.data.datasets.forEach(function(ds, i) {
          if (!ds.backgroundColor) {
            if (config.type === 'pie' || config.type === 'doughnut') {
              ds.backgroundColor = palette;
            } else {
              ds.backgroundColor = palette[i % palette.length] + '80';
              ds.borderColor = palette[i % palette.length];
              ds.borderWidth = ds.borderWidth || 2;
            }
          }
        });
      }
      // Dark mode defaults
      if (config.options) {
        config.options.color = '#94a3b8';
        if (config.options.scales) {
          Object.values(config.options.scales).forEach(function(s) {
            s.ticks = s.ticks || {};
            s.ticks.color = '#64748b';
            s.grid = s.grid || {};
            s.grid.color = 'rgba(255,255,255,0.06)';
          });
        }
      }
      new Chart(canvas.getContext('2d'), config);
    } catch(e) {
      var errP = document.createElement('p'); errP.style.cssText = 'color:#ef4444;font-size:0.85rem'; errP.textContent = 'Chart error: ' + e.message; canvas.parentElement.appendChild(errP);
    }
  });
})();
</script>"""

        # TOC active link tracking
        toc_script = """
<script>
(function() {
  var links = document.querySelectorAll('.toc-link');
  if (!links.length) return;
  var sections = [];
  links.forEach(function(link) {
    var id = link.getAttribute('href');
    if (id && id.startsWith('#')) {
      var el = document.getElementById(id.slice(1));
      if (el) sections.push({ el: el, link: link });
    }
  });
  if (!sections.length) return;
  var observer = new IntersectionObserver(function(entries) {
    entries.forEach(function(entry) {
      var match = sections.find(function(s) { return s.el === entry.target; });
      if (match) {
        if (entry.isIntersecting) {
          links.forEach(function(l) { l.style.color = ''; l.style.borderLeftColor = 'transparent'; });
          match.link.style.color = 'var(--rp-text)';
          match.link.style.borderLeftColor = 'var(--rp-primary)';
        }
      }
    });
  }, { rootMargin: '-20% 0px -70% 0px' });
  sections.forEach(function(s) { observer.observe(s.el); });
})();
</script>"""

        return toc_script + chart_hydration


# ================================================================
# Helpers
# ================================================================

def _esc(text: str) -> str:
    """HTML-escape a string."""
    return html.escape(str(text)) if text else ""


def _agent_icon(agent: str) -> str:
    """Return an emoji icon for an agent role."""
    icons = {
        "coordinator": "\U0001f9e0",
        "researcher": "\U0001f50d",
        "analyst": "\U0001f4ca",
        "fact_checker": "\u2705",
        "writer": "\u270d\ufe0f",
    }
    return icons.get(agent, "\U0001f916")


__all__ = ["ReportRenderer"]
