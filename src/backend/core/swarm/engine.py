"""Swarm engine — orchestrates the multi-agent research pipeline."""

from __future__ import annotations

import asyncio
import json
import logging
from datetime import datetime, timezone
from typing import Any

from ..config import REPORTS_DIR
from .bus import MessageBus
from .events import EventEmitter
from .llm import LLMClient
from .models import AgentMessage, AgentRole, AgentState, SwarmPhase, SwarmSession, Task
from .report_engine import ReportEngine
from .roles import ROLE_CONFIGS
from .search import WebSearchClient

logger = logging.getLogger("apex.swarm.engine")

# Reflection prompt — asks LLM to evaluate initial research and identify gaps
REFLECTION_PROMPT = (
    "You are a Research Reflection Specialist. You have completed an initial research brief "
    "and must now evaluate its depth, identify critical gaps, and generate targeted follow-up queries.\n\n"
    "Analyze the research brief below and respond with a JSON object containing:\n\n"
    '1. "gaps": Array of specific information gaps (missing data, perspectives, context, stakeholders)\n'
    '2. "confidence": Object mapping key claims to "high", "medium", or "low" confidence\n'
    '3. "follow_up_queries": Array of 2-3 specific web search queries designed to:\n'
    "   - Fill the most critical information gaps\n"
    "   - Verify claims that rely on single sources\n"
    "   - Explore alternative perspectives not yet covered\n"
    "   - Find concrete data (statistics, dollar amounts, dates) where the brief is vague\n"
    '4. "contradictions": Array of any conflicting information found in the sources\n\n'
    "Each follow-up query must be specific and searchable — not vague.\n\n"
    "Respond ONLY with valid JSON. No markdown fences, no explanation."
)


def _parse_json_safe(raw: str) -> dict:
    """Attempt to parse JSON from LLM output, with fallback extraction."""
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        start = raw.find("{")
        end = raw.rfind("}") + 1
        if start != -1 and end > start:
            try:
                return json.loads(raw[start:end])
            except json.JSONDecodeError:
                pass
    return {}


class SwarmEngine:
    """Run a full swarm pipeline for a single session."""

    def __init__(
        self,
        session: SwarmSession,
        bus: MessageBus,
        emitter: EventEmitter,
        llm: LLMClient,
    ) -> None:
        self.session = session
        self.bus = bus
        self.emitter = emitter
        self.llm = llm
        self.search = WebSearchClient()

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
            raise asyncio.CancelledError("Swarm cancelled by user")

    async def _set_phase(self, phase: SwarmPhase) -> None:
        self._check_cancelled()
        self.session.phase = phase
        await self._emit("phase_change", {"phase": phase.value})

    async def _stream_agent(
        self, agent: AgentState, messages: list[dict]
    ) -> str:
        """Stream LLM output for *agent*, emitting chunks, and return the full text."""
        collected: list[str] = []
        async for chunk in self.llm.stream(messages, agent.role):
            self._check_cancelled()
            collected.append(chunk)
            await self._emit(
                "agent_output",
                {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value, "content": chunk},
            )
        full = "".join(collected)
        agent.status = "done"
        await self._emit(
            "agent_status",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value, "status": "done"},
        )
        return full

    def _save_report(
        self,
        query: str,
        report: str,
        *,
        doc_ir: object | None = None,
        report_html: str = "",
    ) -> None:
        """Persist completed report to disk.

        Saves up to three files:
          - {id}.json  — metadata + plain-text report (always)
          - {id}_ir.json — DocumentIR serialization (if doc_ir provided)
          - {id}.html — rendered HTML report (if report_html provided)
        """
        try:
            completed_at = datetime.now(timezone.utc).isoformat()
            data = {
                "id": self.session.id,
                "query": query,
                "report": report,
                "agents_count": len(self.session.agents),
                "tasks_count": len(self.session.tasks),
                "created_at": self.session.created_at,
                "completed_at": completed_at,
                "has_html": bool(report_html),
            }
            path = REPORTS_DIR / f"{self.session.id}.json"
            path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
            logger.info("Saved report %s (%d chars)", self.session.id, len(report))

            # Save DocumentIR as separate JSON
            if doc_ir is not None:
                ir_path = REPORTS_DIR / f"{self.session.id}_ir.json"
                ir_data = doc_ir.to_dict() if hasattr(doc_ir, "to_dict") else {}
                ir_path.write_text(json.dumps(ir_data, ensure_ascii=False, indent=2), encoding="utf-8")
                logger.info("Saved IR %s", ir_path.name)

            # Save rendered HTML
            if report_html:
                html_path = REPORTS_DIR / f"{self.session.id}.html"
                html_path.write_text(report_html, encoding="utf-8")
                logger.info("Saved HTML report %s (%d chars)", html_path.name, len(report_html))

        except Exception as exc:
            logger.error("Failed to save report: %s", exc)

    # ------------------------------------------------------------------
    # Pipeline phases
    # ------------------------------------------------------------------

    async def _planning(self, query: str) -> dict:
        """Coordinator decomposes the query into a structured research plan.

        Returns the full parsed plan dict with keys: title, sections, tasks.
        Returns empty dict if coordinator rejects the query.
        """
        await self._set_phase(SwarmPhase.PLANNING)

        agent = self._spawn_agent(AgentRole.COORDINATOR, "Coordinator")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )

        cfg = ROLE_CONFIGS[AgentRole.COORDINATOR]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": query},
        ]

        raw = await self._stream_agent(agent, messages)

        # Parse the JSON plan from the coordinator
        parsed = {}
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            start = raw.find("{")
            end = raw.rfind("}") + 1
            if start != -1 and end > start:
                try:
                    parsed = json.loads(raw[start:end])
                except json.JSONDecodeError:
                    pass

        task_list = parsed.get("tasks", [])

        # Handle validation error (coordinator rejected the query)
        if not task_list:
            error_msg = parsed.get("error", "Could not generate research tasks for this query.")
            await self._emit("error", {"message": error_msg})
            return {}

        # Create Task objects with section mapping
        for t in task_list:
            task = Task(description=t.get("description", ""))
            self.session.tasks.append(task)
            await self._emit(
                "task_update",
                {"task_id": task.id, "description": task.description, "status": task.status},
            )

        return parsed

    async def _research_one(self, task: Task, task_meta: dict, report_title: str) -> str:
        """Run a single researcher with reflection cycles: search -> analyze -> reflect -> deepen.

        task_meta contains the coordinator's task dict with 'section' and 'focus' fields.
        """
        agent = self._spawn_agent(AgentRole.RESEARCHER, f"Researcher-{task.id[:6]}")
        task.assigned_to = agent.id
        task.status = "running"
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )
        await self._emit(
            "task_update",
            {"task_id": task.id, "description": task.description, "status": task.status},
        )

        section_id = task_meta.get("section", "")
        focus = task_meta.get("focus", "")
        cfg = ROLE_CONFIGS[AgentRole.RESEARCHER]

        # ------------------------------------------------------------------
        # Round 1: Initial Research
        # ------------------------------------------------------------------
        await self._emit(
            "agent_status",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
             "status": "researching (round 1)"},
        )
        await self._emit(
            "agent_output",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
             "content": f"[Round 1 — Searching the web for: {task.description}]\n\n"},
        )

        search_results = await self.search.search(task.description, limit=8)
        web_context = WebSearchClient.format_results(search_results)
        source_count = len(search_results)

        await self._emit(
            "agent_output",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
             "content": f"[Found {source_count} sources. Analyzing...]\n\n"},
        )

        user_prompt = (
            f"# Research Task\n\n"
            f"**Report title:** {report_title}\n"
            f"**Search query:** {task.description}\n"
            f"**Target section:** {section_id}\n"
            f"**Focus:** {focus}\n\n"
            f"## Web Sources ({source_count} results)\n\n{web_context}\n\n"
            "Analyze these sources and produce your structured research brief following the output format exactly."
        )

        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": user_prompt},
        ]

        # Stream initial brief (do NOT use _stream_agent — we continue after this)
        initial_collected: list[str] = []
        async for chunk in self.llm.stream(messages, AgentRole.RESEARCHER):
            self._check_cancelled()
            initial_collected.append(chunk)
            await self._emit(
                "agent_output",
                {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value, "content": chunk},
            )
        initial_brief = "".join(initial_collected)

        # ------------------------------------------------------------------
        # Round 2: Reflection & Deepening
        # ------------------------------------------------------------------
        await self._emit(
            "agent_status",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
             "status": "reflecting on findings"},
        )
        await self._emit(
            "agent_output",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
             "content": "\n\n[Reflecting on initial findings to identify gaps...]\n\n"},
        )

        reflection_messages = [
            {"role": "system", "content": REFLECTION_PROMPT},
            {"role": "user", "content": (
                f"**Research Task:** {task.description}\n"
                f"**Focus:** {focus}\n\n"
                f"## Initial Research Brief\n\n{initial_brief}"
            )},
        ]
        reflection_raw = await self.llm.call(reflection_messages, AgentRole.RESEARCHER)
        reflection = _parse_json_safe(reflection_raw)

        follow_up_queries = reflection.get("follow_up_queries", [])[:2]
        gaps = reflection.get("gaps", [])
        confidence = reflection.get("confidence", {})
        contradictions = reflection.get("contradictions", [])

        # Execute follow-up searches
        all_followup_results: list[dict] = []
        if follow_up_queries:
            await self._emit(
                "agent_status",
                {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
                 "status": "deepening research (round 2)"},
            )

            gap_summary = ", ".join(gaps[:3]) if gaps else "additional perspectives"
            await self._emit(
                "agent_output",
                {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
                 "content": (
                     f"[Reflection identified gaps: {gap_summary}]\n"
                     f"[Conducting {len(follow_up_queries)} follow-up searches...]\n\n"
                 )},
            )

            for fq in follow_up_queries:
                self._check_cancelled()
                fq_results = await self.search.search(fq, limit=5)
                all_followup_results.extend(fq_results)
                await self._emit(
                    "agent_output",
                    {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
                     "content": f"[Follow-up: \"{fq}\" — {len(fq_results)} sources]\n"},
                )

        # Produce enriched brief if we have follow-up data
        if all_followup_results:
            followup_context = WebSearchClient.format_results(all_followup_results)
            followup_count = len(all_followup_results)

            await self._emit(
                "agent_output",
                {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value,
                 "content": f"\n[Enriching brief with {followup_count} additional sources...]\n\n"},
            )

            enrichment_prompt = (
                f"# Research Enrichment\n\n"
                f"**Report title:** {report_title}\n"
                f"**Original query:** {task.description}\n"
                f"**Section:** {section_id}\n"
                f"**Focus:** {focus}\n\n"
                f"## Initial Brief\n\n{initial_brief}\n\n"
                f"## Reflection — Identified Gaps\n{json.dumps(gaps, ensure_ascii=False)}\n\n"
                f"## Confidence Ratings\n{json.dumps(confidence, ensure_ascii=False)}\n\n"
                f"## Contradictions Found\n{json.dumps(contradictions, ensure_ascii=False)}\n\n"
                f"## Follow-up Sources ({followup_count} results)\n\n{followup_context}\n\n"
                "---\n\n"
                "Produce an ENRICHED research brief that:\n"
                "1. Incorporates all new findings from follow-up research\n"
                "2. Fills the identified gaps where new evidence is available\n"
                "3. Updates confidence levels — mark claims as high/medium/low confidence\n"
                "4. Addresses contradictions with evidence from both sides\n"
                "5. Preserves all valid original findings\n"
                "6. Notes remaining gaps that could not be filled\n\n"
                "Follow the same structured format (Key Findings, Detailed Analysis, Data Points, Sources, Information Gaps)."
            )

            enrichment_messages = [
                {"role": "system", "content": cfg["system_prompt"]},
                {"role": "user", "content": enrichment_prompt},
            ]

            enriched_collected: list[str] = []
            async for chunk in self.llm.stream(enrichment_messages, AgentRole.RESEARCHER):
                self._check_cancelled()
                enriched_collected.append(chunk)
                await self._emit(
                    "agent_output",
                    {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value, "content": chunk},
                )
            result = "".join(enriched_collected)
        else:
            result = initial_brief

        # ------------------------------------------------------------------
        # Finalize
        # ------------------------------------------------------------------
        agent.status = "done"
        await self._emit(
            "agent_status",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value, "status": "done"},
        )

        # Append deduplicated source list
        all_results = search_results + all_followup_results
        if all_results:
            seen_urls: set[str] = set()
            source_lines: list[str] = []
            for r in all_results:
                url = r.get("url", "")
                if url and url not in seen_urls:
                    seen_urls.add(url)
                    source_lines.append(f"- [{r.get('title', 'Link')}]({url})")
            if source_lines:
                result += "\n\n### Sources\n" + "\n".join(source_lines)

        task.status = "done"
        task.result = result
        await self._emit(
            "task_update",
            {"task_id": task.id, "description": task.description, "status": "done"},
        )

        self.bus.post(
            AgentMessage(
                from_agent=agent.id,
                to_agent="analyst",
                content=result,
                msg_type="result",
            )
        )
        return result

    async def _researching(self, plan: dict) -> list[str]:
        """Run researchers with controlled concurrency."""
        await self._set_phase(SwarmPhase.RESEARCHING)

        task_metas = plan.get("tasks", [])
        report_title = plan.get("title", "Research Report")

        # 3 concurrent researchers for throughput
        sem = asyncio.Semaphore(3)

        async def _safe_research(task: Task, meta: dict) -> str:
            async with sem:
                try:
                    return await self._research_one(task, meta, report_title)
                except Exception as exc:
                    logger.error("Researcher error: %s", exc)
                    return f"[Research Error: {exc}]"

        results = await asyncio.gather(
            *[
                _safe_research(task, meta)
                for task, meta in zip(self.session.tasks, task_metas)
            ]
        )
        return list(results)

    async def _analyzing(self, research_results: list[str], plan: dict) -> str:
        """Analyst synthesises the research findings with the report outline."""
        await self._set_phase(SwarmPhase.ANALYZING)

        agent = self._spawn_agent(AgentRole.ANALYST, "Analyst")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )

        # Build the report outline from the coordinator's plan
        sections = plan.get("sections", [])
        report_title = plan.get("title", "Research Report")
        outline = f"# Report Outline: {report_title}\n\n"
        for s in sections:
            outline += f"- **{s.get('id', '')}. {s.get('title', '')}**: {s.get('description', '')}\n"

        combined_research = "\n\n---\n\n".join(
            f"## Research Brief {i + 1}\n{r}" for i, r in enumerate(research_results)
        )

        user_prompt = (
            f"{outline}\n\n"
            f"---\n\n"
            f"# Research Briefs ({len(research_results)} researchers)\n\n"
            f"{combined_research}\n\n"
            "Synthesize all research briefs into your analytical framework, "
            "using the report outline to organize themes where possible."
        )

        cfg = ROLE_CONFIGS[AgentRole.ANALYST]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": user_prompt},
        ]

        return await self._stream_agent(agent, messages)

    async def _fact_checking(self, analysis: str, research_results: list[str]) -> str:
        """Fact-checker verifies the analysis against original research data."""
        await self._set_phase(SwarmPhase.FACT_CHECKING)

        agent = self._spawn_agent(AgentRole.FACT_CHECKER, "Fact-Checker")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )

        # Give the fact-checker both the analysis AND the original research briefs
        combined_research = "\n\n---\n\n".join(
            f"## Original Research Brief {i + 1}\n{r}" for i, r in enumerate(research_results)
        )

        user_prompt = (
            f"# Analytical Report to Verify\n\n{analysis}\n\n"
            f"---\n\n"
            f"# Original Research Briefs (for cross-referencing)\n\n{combined_research}\n\n"
            "Verify the analytical report's claims against the original research briefs. "
            "Check for unsupported claims, misrepresented data, and missing context."
        )

        cfg = ROLE_CONFIGS[AgentRole.FACT_CHECKER]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": user_prompt},
        ]

        return await self._stream_agent(agent, messages)

    async def _writing(
        self,
        query: str,
        plan: dict,
        research_results: list[str],
        analysis: str,
        fact_check: str,
    ) -> tuple[str, object | None, str]:
        """Run the multi-stage ReportEngine pipeline.

        Returns:
            Tuple of (plain_text_summary, DocumentIR, rendered_html).
            If the engine fails, falls back to a single-pass Writer
            and returns (markdown, None, "").
        """
        await self._set_phase(SwarmPhase.WRITING)

        try:
            report_engine = ReportEngine(self.session, self.emitter, self.llm)
            doc_ir, html_output = await report_engine.run(
                query, plan, research_results, analysis, fact_check,
            )

            # Build a plain-text summary from chapter titles
            report_title = plan.get("title", "Research Report")
            summary_lines = [f"# {report_title}", ""]
            for ch in doc_ir.chapters:
                summary_lines.append(f"## {ch.title}")
                # First paragraph block as summary
                for block in ch.blocks:
                    block_data = block.to_dict() if hasattr(block, "to_dict") else block
                    if block_data.get("type") == "paragraph":
                        inlines = block_data.get("inlines", [])
                        text = " ".join(
                            run.get("text", "") if isinstance(run, dict) else str(run)
                            for run in inlines
                        )
                        if text.strip():
                            summary_lines.append(text[:300])
                            break
                summary_lines.append("")
            plain_summary = "\n".join(summary_lines)

            return plain_summary, doc_ir, html_output

        except Exception as exc:
            logger.error("ReportEngine failed, falling back to single-pass Writer: %s", exc)
            await self._emit(
                "error",
                {"message": f"Report engine error: {exc}. Falling back to simple writer."},
            )
            # Fallback to single-pass writer
            markdown = await self._writing_fallback(query, plan, research_results, analysis, fact_check)
            return markdown, None, ""

    async def _writing_fallback(
        self,
        query: str,
        plan: dict,
        research_results: list[str],
        analysis: str,
        fact_check: str,
    ) -> str:
        """Single-pass Writer fallback if the multi-stage pipeline fails."""
        agent = self._spawn_agent(AgentRole.WRITER, "Writer")
        await self._emit(
            "agent_spawned",
            {"agent_id": agent.id, "agent_name": agent.name, "role": agent.role.value},
        )

        sections = plan.get("sections", [])
        report_title = plan.get("title", "Research Report")
        outline = f"# Report Outline: {report_title}\n\n"
        for s in sections:
            outline += f"- **{s.get('id', '')}. {s.get('title', '')}**: {s.get('description', '')}\n"

        combined_research = "\n\n---\n\n".join(
            f"## Research Brief {i + 1}\n{r}" for i, r in enumerate(research_results)
        )

        prompt = (
            f"# Assignment\n\n"
            f"**Original query:** {query}\n\n"
            f"{outline}\n\n"
            f"---\n\n"
            f"# Raw Research Data ({len(research_results)} briefs)\n\n"
            f"{combined_research}\n\n"
            f"---\n\n"
            f"# Analytical Synthesis\n\n{analysis}\n\n"
            f"---\n\n"
            f"# Fact-Check Report\n\n{fact_check}\n\n"
            f"---\n\n"
            "Write the final report following the report outline structure. "
            "The report title should be: " + report_title + ". "
            "Use ALL the data above — research briefs for raw evidence, "
            "analysis for thematic structure, and fact-check for confidence caveats."
        )

        cfg = ROLE_CONFIGS[AgentRole.WRITER]
        messages = [
            {"role": "system", "content": cfg["system_prompt"]},
            {"role": "user", "content": prompt},
        ]

        return await self._stream_agent(agent, messages)

    # ------------------------------------------------------------------
    # Main entry point
    # ------------------------------------------------------------------

    @staticmethod
    def _is_trivial_query(query: str) -> str | None:
        """Return an error message if the query is too trivial for a research swarm, else None."""
        q = query.strip()
        # Too short to be a real research question
        if len(q) < 10:
            return "Your query is too short. Please provide a detailed research question or topic."
        # Count real words (not just numbers/symbols)
        words = [w for w in q.split() if any(c.isalpha() for c in w)]
        if len(words) < 3:
            return "Please provide a more detailed research question. The swarm works best with specific, complex topics."
        # Simple math / calculator expressions
        stripped = q.replace(" ", "")
        if all(c in "0123456789+-*/=().^%?whatis" for c in stripped.lower()):
            return "This looks like a simple calculation, not a research topic. Please ask a research question instead."
        return None

    async def run(self, query: str) -> None:
        """Execute the full swarm pipeline."""
        try:
            # Pre-check: reject trivially non-research queries before wasting LLM calls
            trivial_err = self._is_trivial_query(query)
            if trivial_err:
                await self._emit("error", {"message": trivial_err})
                await self._emit("swarm_complete", {"report": ""})
                return

            # 1. Planning — coordinator produces title, sections, and tasks
            plan = await self._planning(query)
            if not plan:
                await self._emit("swarm_complete", {"report": ""})
                return

            # 2. Researching — parallel web search + analysis with section context
            research_results = await self._researching(plan)

            # 3. Analyzing — synthesize with report outline
            analysis = await self._analyzing(research_results, plan)

            # 4. Fact-checking — verify against original research data
            fact_check = await self._fact_checking(analysis, research_results)

            # 5. Writing — multi-stage report pipeline (with fallback)
            final, doc_ir, report_html = await self._writing(
                query, plan, research_results, analysis, fact_check,
            )

            # 6. Complete
            self.session.phase = SwarmPhase.COMPLETE
            self.session.final_output = final
            self._save_report(query, final, doc_ir=doc_ir, report_html=report_html)

            complete_data: dict[str, Any] = {"report": final}
            if report_html:
                complete_data["report_html"] = report_html
            await self._emit("swarm_complete", complete_data)

        except asyncio.CancelledError:
            logger.info("Swarm cancelled for session %s", self.session.id)
        except Exception as exc:
            logger.exception("Swarm pipeline error")
            await self._emit("error", {"message": str(exc)})
