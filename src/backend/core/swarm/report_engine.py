"""
Multi-stage Report Engine for the Apex research swarm.

Replaces the single-pass Writer agent with a structured pipeline
inspired by BettaFish's ReportEngine:

    template_selection → document_layout → word_budget
    → chapter_generation → stitching → rendering

Each stage uses a dedicated LLM agent role. The final output is a
DocumentIR object that can be rendered to HTML/Markdown.
"""

from __future__ import annotations

import asyncio
import json
import logging
import re
from typing import Any

from .events import EventEmitter
from .llm import LLMClient
from .models import AgentRole, AgentState, SwarmSession
from .report_ir import (
    ALLOWED_BLOCK_TYPES,
    CHAPTER_JSON_SCHEMA_TEXT,
    DocumentIR,
    IRValidator,
)
from .report_renderer import ReportRenderer
from .report_stitcher import ReportStitcher
from .report_templates import get_all_templates, get_template, get_template_descriptions
from .roles import ROLE_CONFIGS
from .template_parser import TemplateSection, parse_template_sections

logger = logging.getLogger("apex.swarm.report_engine")


# ============================================================
# JSON utilities
# ============================================================

def _strip_fences(text: str) -> str:
    """Remove markdown code fences from LLM output."""
    cleaned = text.strip()
    if cleaned.startswith("```json"):
        cleaned = cleaned[7:]
    elif cleaned.startswith("```"):
        cleaned = cleaned[3:]
    if cleaned.endswith("```"):
        cleaned = cleaned[:-3]
    return cleaned.strip()


def _parse_json(text: str, context: str = "") -> dict[str, Any]:
    """Parse JSON from LLM output with fallback extraction."""
    cleaned = _strip_fences(text)
    if not cleaned:
        raise ValueError(f"Empty LLM response in {context}")

    try:
        return json.loads(cleaned)
    except json.JSONDecodeError:
        pass

    # Try to extract JSON object from surrounding text
    start = cleaned.find("{")
    end = cleaned.rfind("}") + 1
    if start != -1 and end > start:
        try:
            return json.loads(cleaned[start:end])
        except json.JSONDecodeError:
            pass

    raise ValueError(f"Failed to parse JSON in {context}: {cleaned[:200]}")


def _slugify(text: str) -> str:
    """Convert text to a URL-friendly slug."""
    slug = re.sub(r"[^\w\s-]", "", text.lower())
    return re.sub(r"[\s_]+", "-", slug).strip("-")


# ============================================================
# ReportEngine
# ============================================================

class ReportEngine:
    """
    Multi-stage report generation pipeline.

    Designed to replace SwarmEngine._writing() with a structured,
    template-driven approach that produces DocumentIR output.

    Stages:
      1. Template Selection — pick the best template for the query
      2. Document Layout — design title, hero KPIs, TOC, theme
      3. Word Budget — allocate words and outlines per chapter
      4. Chapter Generation — generate each chapter as IR JSON
      5. Stitching — assemble chapters into DocumentIR
      6. Rendering — convert DocumentIR to HTML
    """

    def __init__(
        self,
        session: SwarmSession,
        emitter: EventEmitter,
        llm: LLMClient,
    ) -> None:
        self.session = session
        self.emitter = emitter
        self.llm = llm
        self.validator = IRValidator()
        self.stitcher = ReportStitcher()
        self.renderer = ReportRenderer()

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    def _spawn_agent(self, role: AgentRole, name: str) -> AgentState:
        agent = AgentState(role=role, name=name, status="working")
        self.session.agents[agent.id] = agent
        return agent

    async def _emit(self, event_type: str, data: dict) -> None:
        await self.emitter.emit(self.session.id, event_type, data)

    def _check_cancelled(self) -> None:
        if self.session.cancelled:
            raise asyncio.CancelledError("Report engine cancelled by user")

    async def _call_llm(self, messages: list[dict], role: AgentRole) -> str:
        """Non-streaming LLM call for structured JSON outputs."""
        self._check_cancelled()
        return await self.llm.call(messages, role)

    async def _stream_llm(self, messages: list[dict], role: AgentRole, agent: AgentState) -> str:
        """Streaming LLM call that emits output chunks."""
        collected: list[str] = []
        async for chunk in self.llm.stream(messages, role):
            self._check_cancelled()
            collected.append(chunk)
            await self._emit(
                "agent_output",
                {
                    "agent_id": agent.id,
                    "agent_name": agent.name,
                    "role": agent.role.value,
                    "content": chunk,
                },
            )
        full = "".join(collected)
        agent.status = "done"
        await self._emit(
            "agent_status",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value, "status": "done"},
        )
        return full

    # ------------------------------------------------------------------
    # Stage 1: Template Selection
    # ------------------------------------------------------------------

    async def select_template(
        self,
        query: str,
        research_results: list[str],
        plan: dict,
    ) -> dict[str, Any]:
        """
        Select the best report template and parse it into sections.

        Returns dict with keys:
          template_name, template_content, selection_reason, sections.
        """
        await self._emit("report_stage", {"stage": "template_selection", "status": "started"})

        agent = self._spawn_agent(AgentRole.TEMPLATE_SELECTOR, "Template-Selector")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )

        # Build context for template selection
        descriptions = get_template_descriptions()
        template_list = "\n".join(f"- {name}: {desc}" for name, desc in descriptions.items())

        # Summarize research for selection (first 500 chars each)
        research_summary = ""
        for i, result in enumerate(research_results[:3], 1):
            snippet = result[:500].replace("\n", " ")
            research_summary += f"\nResearch Brief {i}: {snippet}...\n"

        user_prompt = (
            f"Query: {query}\n\n"
            f"Report Title: {plan.get('title', 'Research Report')}\n\n"
            f"Sections planned: {len(plan.get('sections', []))}\n"
            f"Research briefs available: {len(research_results)}\n\n"
            f"Research Summary:{research_summary}\n\n"
            f"Available Templates:\n{template_list}\n\n"
            "Select the best template for this research report."
        )

        cfg = ROLE_CONFIGS[AgentRole.TEMPLATE_SELECTOR]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": user_prompt},
        ]

        raw = await self._call_llm(messages, AgentRole.TEMPLATE_SELECTOR)
        result = _parse_json(raw, "template_selection")

        template_name = result.get("template_name", "")
        template_content = get_template(template_name)
        if template_content is None:
            # Fuzzy match: try partial matching
            for name in descriptions:
                if name in template_name or template_name in name:
                    template_content = get_template(name)
                    template_name = name
                    break
            if template_content is None:
                # Fallback to technology_deep_dive
                template_name = "technology_deep_dive"
                template_content = get_template(template_name) or ""
                result["selection_reason"] = (
                    f"Fallback: template '{result.get('template_name')}' not found. "
                    f"Using {template_name}."
                )

        # Parse template into structured sections
        sections = parse_template_sections(template_content)

        agent.status = "done"
        await self._emit(
            "report_stage",
            {
                "stage": "template_selection",
                "status": "completed",
                "template": template_name,
                "sections": len(sections),
                "reason": result.get("selection_reason", ""),
            },
        )

        return {
            "template_name": template_name,
            "template_content": template_content,
            "selection_reason": result.get("selection_reason", "LLM selection"),
            "sections": sections,
        }

    # ------------------------------------------------------------------
    # Stage 2: Document Layout
    # ------------------------------------------------------------------

    async def design_layout(
        self,
        query: str,
        plan: dict,
        template_result: dict[str, Any],
        research_results: list[str],
    ) -> dict[str, Any]:
        """
        Design the document layout: title, hero KPIs, TOC, theme.

        Uses parsed TemplateSection objects to provide structured
        section information to the LLM.

        Returns the layout design dict.
        """
        await self._emit("report_stage", {"stage": "layout_design", "status": "started"})

        agent = self._spawn_agent(AgentRole.LAYOUT_DESIGNER, "Layout-Designer")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )

        # Build section info from parsed template sections
        sections: list[TemplateSection] = template_result.get("sections", [])
        section_list = "\n".join(
            f"- {s.chapter_id} ({s.number}): {s.title} — outline: {', '.join(s.outline[:3])}"
            for s in sections
        )

        # Also include plan sections
        plan_sections = plan.get("sections", [])
        plan_list = "\n".join(
            f"- {s.get('id', '')}: {s.get('title', '')} — {s.get('description', '')}"
            for s in plan_sections
        )

        # Combine research highlights for KPI extraction
        research_highlights = ""
        for i, result in enumerate(research_results[:4], 1):
            snippet = result[:800].replace("\n", " ")
            research_highlights += f"\nBrief {i}: {snippet}\n"

        user_prompt = (
            f"Query: {query}\n\n"
            f"Report Type: {template_result.get('template_name', 'general')}\n\n"
            f"Template Sections (parsed):\n{section_list}\n\n"
            f"Research Plan Sections:\n{plan_list}\n\n"
            f"Research Highlights:{research_highlights}\n\n"
            "Design the document layout with title, hero KPIs, table of contents, and theme.\n"
            "Map your tocPlan chapters to the template sections above — use their chapter IDs "
            "and titles as a starting point, but adapt based on the research content."
        )

        cfg = ROLE_CONFIGS[AgentRole.LAYOUT_DESIGNER]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": user_prompt},
        ]

        raw = await self._call_llm(messages, AgentRole.LAYOUT_DESIGNER)
        layout = _parse_json(raw, "layout_design")

        # Ensure required fields
        layout.setdefault("title", plan.get("title", f"Report: {query}"))
        layout.setdefault("subtitle", "")
        layout.setdefault("hero", {"kpis": []})
        layout.setdefault("tocPlan", [])
        layout.setdefault("tocTitle", "Table of Contents")
        layout.setdefault("themeTokens", {})

        # If tocPlan is empty, generate from parsed template sections
        if not layout["tocPlan"] and sections:
            layout["tocPlan"] = [
                {
                    "chapterId": s.chapter_id,
                    "display": s.title,
                    "description": ", ".join(s.outline[:3]) if s.outline else s.title,
                    "allowSwot": False,
                    "allowPest": False,
                }
                for s in sections
            ]

        agent.status = "done"
        await self._emit(
            "report_stage",
            {
                "stage": "layout_design",
                "status": "completed",
                "title": layout.get("title"),
                "chapters": len(layout.get("tocPlan", [])),
            },
        )

        return layout

    # ------------------------------------------------------------------
    # Stage 3: Word Budget
    # ------------------------------------------------------------------

    async def plan_word_budget(
        self,
        query: str,
        plan: dict,
        layout: dict[str, Any],
        template_result: dict[str, Any],
        research_results: list[str],
    ) -> dict[str, Any]:
        """
        Allocate word counts and create outlines for each chapter.

        Returns word plan with totalWords, globalGuidelines, and chapters.
        """
        await self._emit("report_stage", {"stage": "budgeting", "status": "started"})

        agent = self._spawn_agent(AgentRole.BUDGET_PLANNER, "Budget-Planner")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )

        toc_summary = "\n".join(
            f"- {ch.get('chapterId', f'ch-{i}')}: {ch.get('display', ch.get('title', 'Untitled'))}"
            for i, ch in enumerate(layout.get("tocPlan", []), 1)
        )

        # Include parsed template section outlines for richer budgeting
        sections: list[TemplateSection] = template_result.get("sections", [])
        template_outlines = ""
        if sections:
            template_outlines = "\n\nTemplate Section Outlines:\n"
            for s in sections:
                outline_str = "; ".join(s.outline[:5]) if s.outline else "N/A"
                template_outlines += f"- {s.chapter_id} {s.title}: {outline_str}\n"

        research_overview = f"Number of research briefs: {len(research_results)}\n"
        for i, r in enumerate(research_results[:5], 1):
            research_overview += f"Brief {i} length: {len(r)} chars\n"

        user_prompt = (
            f"Query: {query}\n\n"
            f"Report Title: {layout.get('title', 'Report')}\n"
            f"Template: {template_result.get('template_name', 'general')}\n\n"
            f"Table of Contents:\n{toc_summary}\n\n"
            f"Research Data:\n{research_overview}\n"
            f"{template_outlines}\n\n"
            "Allocate word counts and create outlines for each chapter.\n"
            "Target total: 10,000-15,000 words."
        )

        cfg = ROLE_CONFIGS[AgentRole.BUDGET_PLANNER]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": user_prompt},
        ]

        raw = await self._call_llm(messages, AgentRole.BUDGET_PLANNER)
        budget = _parse_json(raw, "budgeting")

        # Normalize and validate
        budget.setdefault("totalWords", 12000)
        budget.setdefault("globalGuidelines", [])
        chapters = budget.get("chapters", [])
        if isinstance(chapters, dict):
            chapters = list(chapters.values())
        budget["chapters"] = [ch for ch in chapters if isinstance(ch, dict)]

        if not isinstance(budget["totalWords"], (int, float)):
            budget["totalWords"] = 12000
        if not isinstance(budget["globalGuidelines"], list):
            budget["globalGuidelines"] = []

        agent.status = "done"
        await self._emit(
            "report_stage",
            {
                "stage": "budgeting",
                "status": "completed",
                "total_words": budget["totalWords"],
                "chapter_count": len(budget["chapters"]),
            },
        )

        return budget

    # ------------------------------------------------------------------
    # Stage 4: Chapter Generation
    # ------------------------------------------------------------------

    async def generate_chapter(
        self,
        chapter_plan: dict[str, Any],
        chapter_index: int,
        total_chapters: int,
        query: str,
        research_results: list[str],
        analysis: str,
        fact_check: str,
        layout: dict[str, Any],
        word_plan: dict[str, Any],
        template_name: str,
    ) -> dict[str, Any]:
        """
        Generate a single chapter as Document IR JSON.

        Retries once on validation failure before falling back.
        Returns the chapter dict (validated).
        """
        chapter_id = chapter_plan.get("chapterId", f"ch-{chapter_index + 1:02d}")
        title = chapter_plan.get("title", chapter_plan.get("display", f"Chapter {chapter_index + 1}"))

        agent = self._spawn_agent(AgentRole.CHAPTER_WRITER, f"Writer-{chapter_id}")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )
        await self._emit(
            "chapter_status",
            {"chapterId": chapter_id, "chapter_title": title, "chapter_index": chapter_index, "status": "running"},
        )

        # Build research excerpt (distribute across chapters)
        research_for_chapter = self._distribute_research(
            research_results, chapter_index, total_chapters
        )

        # Build the prompt
        prompt = self._build_chapter_prompt(
            chapter_id, title, chapter_index, total_chapters,
            chapter_plan, layout, word_plan, template_name,
            research_for_chapter, analysis, fact_check,
        )

        cfg = ROLE_CONFIGS[AgentRole.CHAPTER_WRITER]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": prompt},
        ]

        # First attempt
        chapter_json = await self._attempt_chapter_generation(
            messages, agent, chapter_id, title, chapter_index
        )

        # Validate
        valid, errors = self.validator.validate_chapter(chapter_json)
        if not valid:
            logger.warning("Chapter %s attempt 1 validation errors: %s", chapter_id, errors[:5])

            # Retry once with error feedback
            await self._emit(
                "chapter_status",
                {"chapterId": chapter_id, "chapter_title": title, "chapter_index": chapter_index, "status": "retrying"},
            )

            retry_agent = self._spawn_agent(AgentRole.CHAPTER_WRITER, f"Writer-{chapter_id}-retry")
            retry_prompt = (
                f"{prompt}\n\n"
                f"## IMPORTANT: Previous attempt had validation errors:\n"
                + "\n".join(f"- {e}" for e in errors[:10])
                + "\n\nFix these issues in your output. Ensure every paragraph has 'inlines' "
                "with 'text' and 'marks'. Ensure headings have 'anchor'. Ensure lists have 'listType'."
            )
            retry_messages = [
                {"role": "system", "content": cfg["system_prompt"]},
                {"role": "user", "content": retry_prompt},
            ]

            chapter_json = await self._attempt_chapter_generation(
                retry_messages, retry_agent, chapter_id, title, chapter_index
            )

            # Validate retry
            valid, errors = self.validator.validate_chapter(chapter_json)
            if not valid:
                logger.warning("Chapter %s retry still has errors: %s — repairing", chapter_id, errors[:5])
                chapter_json = self._repair_chapter(chapter_json, errors)

        await self._emit(
            "chapter_status",
            {"chapterId": chapter_id, "chapter_title": title, "chapter_index": chapter_index, "status": "completed"},
        )

        return chapter_json

    async def _attempt_chapter_generation(
        self,
        messages: list[dict],
        agent: AgentState,
        chapter_id: str,
        title: str,
        chapter_index: int,
    ) -> dict[str, Any]:
        """Run a single attempt at generating a chapter, with JSON parse + normalization."""
        raw = await self._stream_llm(messages, AgentRole.CHAPTER_WRITER, agent)

        # Parse JSON
        try:
            chapter_json = _parse_json(raw, f"chapter_{chapter_id}")
        except ValueError as exc:
            logger.error("Chapter %s JSON parse failed: %s", chapter_id, exc)
            return self._build_fallback_chapter(chapter_id, title, chapter_index, raw)

        # Ensure required fields
        chapter_json.setdefault("chapterId", chapter_id)
        chapter_json.setdefault("title", title)
        chapter_json.setdefault("anchor", _slugify(title))
        chapter_json.setdefault("order", chapter_index + 1)

        # Handle nested chapter wrapper (some LLMs wrap in {"chapter": {...}})
        if "chapter" in chapter_json and isinstance(chapter_json["chapter"], dict):
            inner = chapter_json["chapter"]
            inner.setdefault("chapterId", chapter_id)
            inner.setdefault("title", title)
            inner.setdefault("anchor", _slugify(title))
            inner.setdefault("order", chapter_index + 1)
            chapter_json = inner

        return chapter_json

    def _build_chapter_prompt(
        self,
        chapter_id: str,
        title: str,
        chapter_index: int,
        total_chapters: int,
        chapter_plan: dict[str, Any],
        layout: dict[str, Any],
        word_plan: dict[str, Any],
        template_name: str,
        research_for_chapter: str,
        analysis: str,
        fact_check: str,
    ) -> str:
        """Build the full user prompt for chapter generation."""
        user_prompt = (
            f"# Chapter Assignment\n\n"
            f"**Report Title**: {layout.get('title', 'Report')}\n"
            f"**Report Template**: {template_name}\n"
            f"**Chapter**: {chapter_id} — {title}\n"
            f"**Order**: {chapter_index + 1} of {total_chapters}\n\n"
        )

        # Word budget for this chapter
        target_words = chapter_plan.get("targetWords", 1200)
        emphasis = chapter_plan.get("emphasis", "")
        key_points = chapter_plan.get("keyPoints", [])
        visualizations = chapter_plan.get("visualizations", [])

        user_prompt += (
            f"## Word Budget\n"
            f"- Target: {target_words} words\n"
            f"- Min: {chapter_plan.get('minWords', target_words - 300)} words\n"
            f"- Max: {chapter_plan.get('maxWords', target_words + 300)} words\n"
            f"- Emphasis: {emphasis}\n\n"
        )

        if key_points:
            user_prompt += "## Key Points to Cover\n"
            for kp in key_points:
                user_prompt += f"- {kp}\n"
            user_prompt += "\n"

        if visualizations:
            user_prompt += "## Required Visualizations\n"
            for viz in visualizations:
                if isinstance(viz, dict):
                    user_prompt += f"- {viz.get('type', 'table')}: {viz.get('description', '')}\n"
                else:
                    user_prompt += f"- {viz}\n"
            user_prompt += "\n"

        # Research data
        user_prompt += (
            f"## Research Data\n\n{research_for_chapter}\n\n"
            f"## Analytical Synthesis (excerpt)\n\n{analysis[:2000]}\n\n"
            f"## Fact-Check Notes (excerpt)\n\n{fact_check[:1000]}\n\n"
        )

        # Global guidelines
        guidelines = word_plan.get("globalGuidelines", [])
        if guidelines:
            user_prompt += "## Writing Guidelines\n"
            for g in guidelines:
                user_prompt += f"- {g}\n"
            user_prompt += "\n"

        user_prompt += (
            f"## IR Schema Reference\n\n"
            f"Allowed block types: {', '.join(ALLOWED_BLOCK_TYPES)}\n\n"
            f"Produce a JSON chapter object with: chapterId, title, anchor, order, blocks[]\n"
            f"Each block has a type and type-specific fields.\n\n"
            f"Generate the chapter now. Output ONLY valid JSON."
        )

        return user_prompt

    async def generate_all_chapters(
        self,
        query: str,
        research_results: list[str],
        analysis: str,
        fact_check: str,
        layout: dict[str, Any],
        word_plan: dict[str, Any],
        template_name: str,
    ) -> list[dict[str, Any]]:
        """
        Generate all chapters with controlled concurrency.

        Uses the tocPlan from layout and chapter budgets from word_plan.
        Falls back to word_plan chapters if tocPlan is empty.
        """
        await self._emit("report_stage", {"stage": "chapter_writing", "status": "started"})

        # Build merged chapter list from tocPlan + word budget
        chapter_plans = self._merge_chapter_plans(layout, word_plan)

        if not chapter_plans:
            logger.warning("No chapters to generate, using fallback single chapter")
            chapter_plans = [
                {
                    "chapterId": "ch-01",
                    "title": "Research Findings",
                    "targetWords": 3000,
                    "keyPoints": ["Present all research findings"],
                }
            ]

        total = len(chapter_plans)
        chapters: list[dict[str, Any]] = []

        # Generate sequentially to manage LLM rate limits
        for i, plan in enumerate(chapter_plans):
            self._check_cancelled()
            await self._emit(
                "report_progress",
                {"progress": 20 + round(60 * i / total), "message": f"Writing chapter {i + 1}/{total}"},
            )
            chapter = await self.generate_chapter(
                plan, i, total,
                query, research_results, analysis, fact_check,
                layout, word_plan, template_name,
            )
            chapters.append(chapter)

        await self._emit(
            "report_stage",
            {"stage": "chapter_writing", "status": "completed", "chapters": len(chapters)},
        )

        return chapters

    # ------------------------------------------------------------------
    # Stage 5: Stitching (via ReportStitcher)
    # ------------------------------------------------------------------

    def stitch_document(
        self,
        report_id: str,
        query: str,
        layout: dict[str, Any],
        word_plan: dict[str, Any],
        template_name: str,
        chapters: list[dict[str, Any]],
        source_count: int = 0,
        agent_count: int = 0,
    ) -> DocumentIR:
        """
        Assemble chapters into a complete DocumentIR using ReportStitcher.

        Validates chapters first, then delegates stitching to the
        ReportStitcher for proper anchor deduplication and TOC generation.
        """
        # Pre-validate and log any issues
        all_valid, per_chapter_errors = self.stitcher.validate_chapters(chapters)
        if not all_valid:
            for ch_id, errors in per_chapter_errors.items():
                logger.warning("Stitching: chapter %s has %d errors", ch_id, len(errors))

        # Build theme dict from layout
        theme_tokens = layout.get("themeTokens", {})
        theme_dict: dict[str, Any] = {"colors": {}, "fonts": {}}
        if isinstance(theme_tokens, dict):
            if "primary_color" in theme_tokens:
                theme_dict["colors"]["primary"] = theme_tokens["primary_color"]
            if "secondary_color" in theme_tokens:
                theme_dict["colors"]["secondary"] = theme_tokens["secondary_color"]
            if "chart_palette" in theme_tokens:
                palette = theme_tokens["chart_palette"]
                if isinstance(palette, list):
                    for i, color in enumerate(palette[:4]):
                        theme_dict["colors"][f"accent{i + 1}"] = color

        # Build extra metadata
        extra_metadata = {
            "reportType": template_name.replace("_", " ").title(),
            "wordCount": word_plan.get("totalWords"),
            "sourceCount": source_count,
            "agentCount": agent_count,
        }

        # Delegate to ReportStitcher
        doc = self.stitcher.stitch(
            report_id=report_id,
            chapters=chapters,
            title=layout.get("title", f"Report: {query}"),
            subtitle=layout.get("subtitle"),
            query=query,
            template_name=template_name,
            theme=theme_dict if theme_dict["colors"] else None,
            toc_config={
                "depth": 3,
                "autoNumbering": True,
                "title": layout.get("tocTitle", "Table of Contents"),
            },
            extra_metadata=extra_metadata,
        )

        return doc

    # ------------------------------------------------------------------
    # Stage 6: Rendering (HTML via ReportRenderer)
    # ------------------------------------------------------------------

    def render_to_html(self, doc: DocumentIR) -> str:
        """
        Render DocumentIR to a self-contained HTML document.

        Uses the ReportRenderer for full-featured output with
        Chart.js visualizations, styled tables, SWOT/PEST grids, etc.
        """
        return self.renderer.render(doc)

    def render_to_markdown(self, doc: DocumentIR) -> str:
        """
        Render DocumentIR to Markdown text (fallback renderer).

        Used when a simpler output format is needed.
        """
        lines: list[str] = []

        # Title
        lines.append(f"# {doc.metadata.title}")
        if doc.metadata.subtitle:
            lines.append(f"*{doc.metadata.subtitle}*")
        lines.append("")

        # Metadata
        lines.append(f"*Generated: {doc.metadata.generated_at}*")
        if doc.metadata.template_name:
            lines.append(f"*Template: {doc.metadata.template_name}*")
        lines.append("")
        lines.append("---")
        lines.append("")

        # Chapters
        for chapter in doc.chapters:
            for block in chapter.blocks:
                lines.append(self._render_block_md(block))
            lines.append("")

        # Footer
        lines.append("---")
        lines.append("")
        lines.append(
            "*This report was generated by the Apex AI Research Swarm. "
            f"Sources consulted: {doc.metadata.source_count or 'N/A'}. "
            f"Agents deployed: {doc.metadata.agent_count or 'N/A'}.*"
        )

        return "\n".join(lines)

    def _render_block_md(self, block: Any) -> str:
        """Render a single block to Markdown."""
        if hasattr(block, "to_dict"):
            data = block.to_dict()
        elif isinstance(block, dict):
            data = block
        else:
            return ""

        btype = data.get("type", "")

        if btype == "heading":
            level = data.get("level", 2)
            text = data.get("text", "")
            numbering = data.get("numbering", "")
            prefix = "#" * min(level, 6)
            num_str = f"{numbering} " if numbering else ""
            return f"{prefix} {num_str}{text}\n"

        if btype == "paragraph":
            inlines = data.get("inlines", [])
            return self._render_inlines_md(inlines) + "\n"

        if btype == "list":
            items = data.get("items", [])
            list_type = data.get("listType", "bullet")
            lines = []
            for i, item_blocks in enumerate(items, 1):
                prefix = f"{i}." if list_type == "ordered" else "-"
                if isinstance(item_blocks, list):
                    for sub in item_blocks:
                        if isinstance(sub, dict):
                            text = self._extract_text(sub)
                            lines.append(f"{prefix} {text}")
                            prefix = "  "
                elif isinstance(item_blocks, dict):
                    text = self._extract_text(item_blocks)
                    lines.append(f"{prefix} {text}")
            return "\n".join(lines) + "\n"

        if btype == "table":
            return self._render_table_md(data) + "\n"

        if btype == "callout":
            tone = data.get("tone", "info")
            title = data.get("title", "")
            inner_blocks = data.get("blocks", [])
            inner_text = "\n".join(
                self._extract_text(b) for b in inner_blocks if isinstance(b, dict)
            )
            header = f"> **{tone.upper()}: {title}**" if title else f"> **{tone.upper()}**"
            return f"{header}\n> {inner_text}\n"

        if btype == "blockquote":
            inner_blocks = data.get("blocks", [])
            inner_text = "\n".join(
                self._extract_text(b) for b in inner_blocks if isinstance(b, dict)
            )
            return f"> {inner_text}\n"

        if btype == "kpiGrid":
            items = data.get("items", [])
            parts = []
            for item in items:
                if isinstance(item, dict):
                    label = item.get("label", "")
                    value = item.get("value", "")
                    parts.append(f"**{label}**: {value}")
            return " | ".join(parts) + "\n"

        if btype == "chart":
            chart_type = data.get("chartType", "chart")
            title = data.get("title", "")
            caption = data.get("caption", "")
            return f"*[{chart_type}: {title}]* {caption}\n"

        if btype == "hr":
            return "---\n"

        if btype == "code":
            lang = data.get("language", data.get("lang", ""))
            content = data.get("content", "")
            return f"```{lang}\n{content}\n```\n"

        if btype == "toc":
            return "*[Table of Contents]*\n"

        # Fallback
        text = data.get("text", data.get("content", ""))
        if text:
            return f"{text}\n"
        return ""

    def _render_inlines_md(self, inlines: list) -> str:
        """Render inline runs to Markdown text."""
        parts: list[str] = []
        for run in inlines:
            if not isinstance(run, dict):
                continue
            text = run.get("text", "")
            marks = run.get("marks", [])
            for mark in marks:
                if not isinstance(mark, dict):
                    continue
                mtype = mark.get("type", "")
                if mtype == "bold":
                    text = f"**{text}**"
                elif mtype == "italic":
                    text = f"*{text}*"
                elif mtype == "code":
                    text = f"`{text}`"
                elif mtype == "link":
                    href = mark.get("href", "")
                    text = f"[{text}]({href})"
                elif mtype == "strike":
                    text = f"~~{text}~~"
            parts.append(text)
        return "".join(parts)

    def _render_table_md(self, data: dict) -> str:
        """Render a table block to Markdown."""
        rows = data.get("rows", [])
        if not rows:
            return ""

        lines: list[str] = []
        for row_idx, row in enumerate(rows):
            if not isinstance(row, dict):
                continue
            cells = row.get("cells", [])
            cell_texts: list[str] = []
            for cell in cells:
                if isinstance(cell, dict):
                    blocks = cell.get("blocks", [])
                    text = " ".join(
                        self._extract_text(b) for b in blocks if isinstance(b, dict)
                    )
                    cell_texts.append(text.strip() or "\u2014")
                else:
                    cell_texts.append(str(cell))
            lines.append("| " + " | ".join(cell_texts) + " |")

            # Add separator after first row (header)
            if row_idx == 0:
                lines.append("|" + "|".join("---" for _ in cell_texts) + "|")

        return "\n".join(lines)

    def _extract_text(self, block: dict) -> str:
        """Extract plain text from a block."""
        btype = block.get("type", "")
        if btype == "paragraph":
            inlines = block.get("inlines", [])
            return self._render_inlines_md(inlines)
        for key in ("text", "content", "value"):
            val = block.get(key)
            if isinstance(val, str):
                return val
        return ""

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _distribute_research(
        self,
        research_results: list[str],
        chapter_index: int,
        total_chapters: int,
    ) -> str:
        """
        Distribute research data across chapters.

        Each chapter gets all research (for context) but emphasis on
        the portions most relevant to its position.
        """
        if not research_results:
            return "[No research data available]"

        # Give each chapter the full research, truncated to fit context
        max_per_brief = 3000 // max(len(research_results), 1)
        parts: list[str] = []
        for i, result in enumerate(research_results):
            # Shift emphasis: earlier chapters get more from earlier briefs
            offset = (chapter_index * len(result)) // max(total_chapters, 1)
            excerpt = result[offset:offset + max_per_brief]
            if not excerpt:
                excerpt = result[:max_per_brief]
            parts.append(f"### Research Brief {i + 1}\n{excerpt}")

        return "\n\n".join(parts)

    def _merge_chapter_plans(
        self,
        layout: dict[str, Any],
        word_plan: dict[str, Any],
    ) -> list[dict[str, Any]]:
        """
        Merge tocPlan from layout with chapter budgets from word_plan.

        Uses tocPlan as the primary structure and enriches with word
        budget data (targetWords, keyPoints, emphasis, etc.).
        """
        toc_entries = layout.get("tocPlan", [])
        budget_chapters = word_plan.get("chapters", [])

        # Index budget by chapterId
        budget_by_id: dict[str, dict] = {}
        for bc in budget_chapters:
            if isinstance(bc, dict) and bc.get("chapterId"):
                budget_by_id[bc["chapterId"]] = bc

        if not toc_entries:
            # No tocPlan, use budget chapters directly
            return budget_chapters

        merged: list[dict[str, Any]] = []
        for entry in toc_entries:
            if not isinstance(entry, dict):
                continue
            ch_id = entry.get("chapterId", "")
            chapter = {
                "chapterId": ch_id,
                "title": entry.get("display", entry.get("title", "Untitled")),
                "description": entry.get("description", ""),
                "allowSwot": entry.get("allowSwot", False),
                "allowPest": entry.get("allowPest", False),
            }

            # Merge budget data if available
            budget = budget_by_id.get(ch_id, {})
            chapter["targetWords"] = budget.get("targetWords", 1200)
            chapter["minWords"] = budget.get("minWords", 900)
            chapter["maxWords"] = budget.get("maxWords", 1500)
            chapter["emphasis"] = budget.get("emphasis", "")
            chapter["keyPoints"] = budget.get("keyPoints", [])
            chapter["visualizations"] = budget.get("visualizations", [])

            merged.append(chapter)

        return merged

    def _build_fallback_chapter(
        self,
        chapter_id: str,
        title: str,
        order: int,
        raw_text: str,
    ) -> dict[str, Any]:
        """Build a minimal valid chapter when JSON parsing fails."""
        return {
            "chapterId": chapter_id,
            "title": title,
            "anchor": _slugify(title),
            "order": order + 1,
            "blocks": [
                {
                    "type": "heading",
                    "level": 2,
                    "text": title,
                    "anchor": _slugify(title),
                },
                {
                    "type": "callout",
                    "tone": "warning",
                    "title": "Content Generation Note",
                    "blocks": [
                        {
                            "type": "paragraph",
                            "inlines": [
                                {
                                    "text": (
                                        "This chapter's structured output could not be fully parsed. "
                                        "The raw content is preserved below."
                                    ),
                                    "marks": [],
                                }
                            ],
                        }
                    ],
                },
                {
                    "type": "paragraph",
                    "inlines": [
                        {"text": raw_text[:3000] if raw_text else "[No content]", "marks": []},
                    ],
                },
            ],
        }

    def _repair_chapter(
        self,
        chapter: dict[str, Any],
        errors: list[str],
    ) -> dict[str, Any]:
        """Attempt local repair of common IR validation errors."""
        blocks = chapter.get("blocks", [])
        repaired_blocks: list[dict] = []

        for block in blocks:
            if not isinstance(block, dict):
                if isinstance(block, str) and block.strip():
                    repaired_blocks.append({
                        "type": "paragraph",
                        "inlines": [{"text": block, "marks": []}],
                    })
                continue

            btype = block.get("type", "")
            if btype not in ALLOWED_BLOCK_TYPES:
                # Convert to paragraph
                text = block.get("text", block.get("content", json.dumps(block)))
                repaired_blocks.append({
                    "type": "paragraph",
                    "inlines": [{"text": str(text), "marks": []}],
                })
                continue

            # Fix paragraph blocks missing inlines
            if btype == "paragraph" and "inlines" not in block:
                text = block.get("text", block.get("content", ""))
                block["inlines"] = [{"text": str(text), "marks": []}]

            # Fix heading blocks missing anchor
            if btype == "heading" and "anchor" not in block:
                block["anchor"] = _slugify(block.get("text", "section"))

            # Fix list blocks missing listType
            if btype == "list" and block.get("listType") not in {"ordered", "bullet", "task"}:
                block["listType"] = "bullet"

            # Fix chart blocks missing required fields — skip if unrepairable
            if btype == "chart":
                if "chartType" not in block:
                    block["chartType"] = "bar"
                if "chartData" not in block:
                    # Cannot repair a chart without data, convert to callout
                    title = block.get("title", "Chart")
                    repaired_blocks.append({
                        "type": "callout",
                        "tone": "info",
                        "title": f"Chart: {title}",
                        "blocks": [{
                            "type": "paragraph",
                            "inlines": [{"text": f"[Chart data unavailable: {title}]", "marks": []}],
                        }],
                    })
                    continue

            repaired_blocks.append(block)

        if not repaired_blocks:
            repaired_blocks = [{
                "type": "paragraph",
                "inlines": [{"text": "[Content pending]", "marks": []}],
            }]

        chapter["blocks"] = repaired_blocks
        return chapter

    # ------------------------------------------------------------------
    # Main pipeline orchestrator
    # ------------------------------------------------------------------

    async def run(
        self,
        query: str,
        plan: dict,
        research_results: list[str],
        analysis: str,
        fact_check: str,
    ) -> tuple[DocumentIR, str]:
        """
        Execute the full report generation pipeline.

        Args:
            query: The original research query.
            plan: The coordinator's research plan (title, sections, tasks).
            research_results: Raw research briefs from researchers.
            analysis: The analyst's synthesis.
            fact_check: The fact-checker's verification report.

        Returns:
            Tuple of (DocumentIR, rendered_html).
        """
        await self._emit("report_progress", {"progress": 0, "message": "Starting report generation"})

        # Stage 1: Template Selection
        template_result = await self.select_template(query, research_results, plan)
        await self._emit("report_progress", {"progress": 5, "message": "Template selected"})

        # Stage 2: Document Layout
        layout = await self.design_layout(query, plan, template_result, research_results)
        await self._emit("report_progress", {"progress": 10, "message": "Layout designed"})

        # Stage 3: Word Budget
        word_plan = await self.plan_word_budget(
            query, plan, layout, template_result, research_results
        )
        await self._emit("report_progress", {"progress": 15, "message": "Word budget allocated"})

        # Stage 4: Chapter Generation
        chapters = await self.generate_all_chapters(
            query, research_results, analysis, fact_check,
            layout, word_plan, template_result["template_name"],
        )
        await self._emit("report_progress", {"progress": 85, "message": "All chapters generated"})

        # Stage 5: Stitching
        await self._emit("report_stage", {"stage": "stitching", "status": "started"})
        doc = self.stitch_document(
            report_id=self.session.id,
            query=query,
            layout=layout,
            word_plan=word_plan,
            template_name=template_result["template_name"],
            chapters=chapters,
            source_count=len(research_results) * 8,  # ~8 sources per brief
            agent_count=len(self.session.agents),
        )
        await self._emit("report_stage", {"stage": "stitching", "status": "completed"})
        await self._emit("report_progress", {"progress": 90, "message": "Document assembled"})

        # Stage 6: Rendering
        await self._emit("report_stage", {"stage": "rendering", "status": "started"})
        html_output = self.render_to_html(doc)
        await self._emit(
            "report_stage",
            {
                "stage": "rendering",
                "status": "completed",
                "chapter_count": len(doc.chapters),
            },
        )
        await self._emit("report_progress", {"progress": 100, "message": "Report complete"})

        return doc, html_output
