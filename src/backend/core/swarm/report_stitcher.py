"""
Report stitcher — assembles chapters into a complete DocumentIR.

Takes individually-generated chapter payloads (dicts from the LLM),
validates them, assigns unique anchors, generates a TOC, and produces
a fully assembled DocumentIR ready for rendering.
"""

from __future__ import annotations

from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Set, Tuple

from .report_ir import (
    IR_VERSION,
    Chapter,
    DocumentIR,
    DocumentMetadata,
    IRValidator,
    ThemeTokens,
    TOCConfig,
)


class ReportStitcher:
    """
    Assembles chapter payloads into a complete DocumentIR.

    Responsibilities:
      - Sort chapters by order
      - Ensure unique anchors across all chapters
      - Validate chapter structures
      - Build TOC entries from heading blocks
      - Inject metadata (title, subtitle, query, timestamps)
    """

    def __init__(self) -> None:
        self._seen_anchors: Set[str] = set()
        self._validator = IRValidator()

    def stitch(
        self,
        report_id: str,
        chapters: List[Dict[str, Any]],
        *,
        title: str = "",
        subtitle: Optional[str] = None,
        query: str = "",
        template_name: Optional[str] = None,
        theme: Optional[Dict[str, Any]] = None,
        toc_config: Optional[Dict[str, Any]] = None,
        extra_metadata: Optional[Dict[str, Any]] = None,
    ) -> DocumentIR:
        """
        Assemble chapters into a complete DocumentIR.

        Args:
            report_id: Unique identifier for this report.
            chapters: List of raw chapter dicts (from LLM JSON output).
            title: Report title.
            subtitle: Optional report subtitle.
            query: Original user query that spawned this report.
            template_name: Name of the template used.
            theme: Optional theme tokens override.
            toc_config: Optional TOC configuration override.
            extra_metadata: Additional metadata to merge.

        Returns:
            A fully assembled DocumentIR instance.
        """
        self._seen_anchors.clear()

        # Sort chapters by order
        ordered = sorted(chapters, key=lambda c: c.get("order", 0))

        # Process each chapter: assign IDs, deduplicate anchors
        processed_chapters: List[Chapter] = []
        for idx, raw_chapter in enumerate(ordered, start=1):
            raw_chapter.setdefault("chapterId", f"S{idx}")
            raw_chapter.setdefault("order", idx * 10)

            anchor = raw_chapter.get("anchor") or f"section-{idx}"
            raw_chapter["anchor"] = self._ensure_unique_anchor(anchor)

            chapter = Chapter.from_dict(raw_chapter)
            processed_chapters.append(chapter)

        # Build metadata
        metadata = DocumentMetadata(
            title=title,
            subtitle=subtitle,
            query=query,
            template_name=template_name,
            generated_at=datetime.now(timezone.utc).isoformat(),
        )

        # Build theme
        theme_tokens = ThemeTokens.from_dict(theme) if theme else ThemeTokens()

        # Build TOC config
        toc = TOCConfig.from_dict(toc_config) if toc_config else TOCConfig()

        # Merge extra metadata if provided
        if extra_metadata:
            metadata.report_type = extra_metadata.get("reportType", metadata.report_type)
            metadata.word_count = extra_metadata.get("wordCount", metadata.word_count)
            metadata.source_count = extra_metadata.get("sourceCount", metadata.source_count)
            metadata.agent_count = extra_metadata.get("agentCount", metadata.agent_count)

        doc = DocumentIR(
            report_id=report_id,
            metadata=metadata,
            theme=theme_tokens,
            toc=toc,
            chapters=processed_chapters,
        )

        return doc

    def validate_chapters(
        self, chapters: List[Dict[str, Any]]
    ) -> Tuple[bool, Dict[str, List[str]]]:
        """
        Validate all chapter dicts before stitching.

        Returns:
            Tuple of (all_valid, per_chapter_errors) where
            per_chapter_errors maps chapter IDs to error lists.
        """
        all_valid = True
        per_chapter: Dict[str, List[str]] = {}

        for chapter in chapters:
            ch_id = chapter.get("chapterId", "unknown")
            valid, errors = self._validator.validate_chapter(chapter)
            if not valid:
                all_valid = False
                per_chapter[ch_id] = errors

        return all_valid, per_chapter

    def generate_toc(self, doc: DocumentIR) -> List[Dict[str, Any]]:
        """
        Generate table of contents entries from the assembled document.

        Scans all chapters for heading blocks and produces a flat list
        of TOC entries with title, anchor, level, and chapter ID.

        Args:
            doc: The assembled DocumentIR.

        Returns:
            List of TOC entry dicts.
        """
        entries: List[Dict[str, Any]] = []
        max_depth = doc.toc.depth

        for chapter in doc.chapters:
            for block in chapter.blocks:
                block_dict = block.to_dict()
                if block_dict.get("type") != "heading":
                    continue

                level = block_dict.get("level", 1)
                if level > max_depth:
                    continue

                entries.append({
                    "title": block_dict.get("text", ""),
                    "anchor": block_dict.get("anchor", chapter.anchor),
                    "level": level,
                    "chapterId": chapter.chapter_id,
                    "numbering": block_dict.get("numbering"),
                })

        return entries

    def _ensure_unique_anchor(self, anchor: str) -> str:
        """Append a numeric suffix if the anchor is already used."""
        base = anchor
        counter = 2
        while anchor in self._seen_anchors:
            anchor = f"{base}-{counter}"
            counter += 1
        self._seen_anchors.add(anchor)
        return anchor


__all__ = ["ReportStitcher"]
