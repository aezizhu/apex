"""
Document IR (Intermediate Representation) schema for Apex research reports.

Defines a structured, JSON-serializable document model that decouples
content generation from rendering. The pipeline is:

    LLM chapters (JSON) -> DocumentIR -> Renderer (HTML/PDF/etc.)

Inspired by BettaFish's ReportEngine IR but adapted for Apex's
multi-agent research context with English-language reports.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, ClassVar, Dict, List, Optional, Tuple


IR_VERSION = "1.0"

# ============================================================
# Inline marks — formatting applied within a text run
# ============================================================

ALLOWED_INLINE_MARKS: List[str] = [
    "bold",
    "italic",
    "underline",
    "strike",
    "code",
    "link",
    "color",
    "highlight",
    "subscript",
    "superscript",
    "math",
]

# ============================================================
# Block types — structural content elements
# ============================================================

ALLOWED_BLOCK_TYPES: List[str] = [
    "heading",
    "paragraph",
    "list",
    "table",
    "chart",
    "callout",
    "kpiGrid",
    "swotTable",
    "pestTable",
    "blockquote",
    "agentQuote",
    "code",
    "math",
    "figure",
    "toc",
    "hr",
]

# Maps Apex agent roles to display titles for agentQuote blocks
AGENT_TITLES: Dict[str, str] = {
    "coordinator": "Coordinator",
    "researcher": "Researcher",
    "analyst": "Analyst",
    "fact_checker": "Fact-Checker",
    "writer": "Writer",
}


# ============================================================
# Dataclass helpers
# ============================================================

def _filter_none(d: Dict[str, Any]) -> Dict[str, Any]:
    """Remove keys with None values for cleaner JSON output."""
    return {k: v for k, v in d.items() if v is not None}


# ============================================================
# Inline mark & text run
# ============================================================

@dataclass
class InlineMark:
    """A formatting mark applied to a text run (bold, link, etc.)."""

    type: str
    href: Optional[str] = None
    title: Optional[str] = None
    value: Optional[Any] = None

    def to_dict(self) -> Dict[str, Any]:
        d: Dict[str, Any] = {"type": self.type}
        if self.href is not None:
            d["href"] = self.href
        if self.title is not None:
            d["title"] = self.title
        if self.value is not None:
            d["value"] = self.value
        return d

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> InlineMark:
        return cls(
            type=data["type"],
            href=data.get("href"),
            title=data.get("title"),
            value=data.get("value"),
        )


@dataclass
class InlineRun:
    """A span of text with optional formatting marks."""

    text: str
    marks: List[InlineMark] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        d: Dict[str, Any] = {"text": self.text}
        if self.marks:
            d["marks"] = [m.to_dict() for m in self.marks]
        return d

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> InlineRun:
        marks = [InlineMark.from_dict(m) for m in data.get("marks", [])]
        return cls(text=data["text"], marks=marks)


# ============================================================
# Block types
# ============================================================

@dataclass
class Block:
    """
    A content block within a section/chapter.

    This is the base representation. Each block has a `type` field
    and type-specific data stored in a flat dict structure for
    flexibility. The `to_dict` / `from_dict` methods handle
    serialization without requiring a subclass per block type.
    """

    type: str
    data: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        result = {"type": self.type}
        result.update(self.data)
        return result

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Block:
        block_type = data.get("type", "paragraph")
        rest = {k: v for k, v in data.items() if k != "type"}
        return cls(type=block_type, data=rest)

    # ---- Convenience constructors for common block types ----

    @staticmethod
    def heading(
        level: int,
        text: str,
        anchor: str,
        *,
        numbering: Optional[str] = None,
        subtitle: Optional[str] = None,
    ) -> Block:
        data: Dict[str, Any] = {"level": level, "text": text, "anchor": anchor}
        if numbering:
            data["numbering"] = numbering
        if subtitle:
            data["subtitle"] = subtitle
        return Block(type="heading", data=data)

    @staticmethod
    def paragraph(inlines: List[InlineRun], *, align: Optional[str] = None) -> Block:
        data: Dict[str, Any] = {"inlines": [r.to_dict() for r in inlines]}
        if align:
            data["align"] = align
        return Block(type="paragraph", data=data)

    @staticmethod
    def plain_text(text: str) -> Block:
        """Shortcut: a paragraph with a single unstyled text run."""
        return Block.paragraph([InlineRun(text=text)])

    @staticmethod
    def bullet_list(items: List[str]) -> Block:
        """Shortcut: a simple bullet list from plain strings."""
        return Block(
            type="list",
            data={
                "listType": "bullet",
                "items": [
                    [{"type": "paragraph", "inlines": [{"text": item}]}]
                    for item in items
                ],
            },
        )

    @staticmethod
    def table(
        rows: List[Dict[str, Any]],
        *,
        caption: Optional[str] = None,
        zebra: bool = True,
    ) -> Block:
        data: Dict[str, Any] = {"rows": rows, "zebra": zebra}
        if caption:
            data["caption"] = caption
        return Block(type="table", data=data)

    @staticmethod
    def chart(
        chart_type: str,
        chart_data: Dict[str, Any],
        *,
        title: Optional[str] = None,
        caption: Optional[str] = None,
    ) -> Block:
        data: Dict[str, Any] = {"chartType": chart_type, "chartData": chart_data}
        if title:
            data["title"] = title
        if caption:
            data["caption"] = caption
        return Block(type="chart", data=data)

    @staticmethod
    def callout(
        tone: str,
        blocks: List[Dict[str, Any]],
        *,
        title: Optional[str] = None,
    ) -> Block:
        data: Dict[str, Any] = {"tone": tone, "blocks": blocks}
        if title:
            data["title"] = title
        return Block(type="callout", data=data)

    @staticmethod
    def kpi_grid(
        items: List[Dict[str, Any]],
        *,
        cols: Optional[int] = None,
    ) -> Block:
        data: Dict[str, Any] = {"items": items}
        if cols:
            data["cols"] = cols
        return Block(type="kpiGrid", data=data)

    @staticmethod
    def figure(
        src: str,
        *,
        alt: str = "",
        caption: Optional[str] = None,
    ) -> Block:
        data: Dict[str, Any] = {"img": {"src": src, "alt": alt}}
        if caption:
            data["caption"] = caption
        return Block(type="figure", data=data)

    @staticmethod
    def toc(*, depth: int = 3, auto_numbering: bool = True) -> Block:
        return Block(
            type="toc",
            data={"depth": depth, "autoNumbering": auto_numbering},
        )

    @staticmethod
    def hr() -> Block:
        return Block(type="hr", data={})


# ============================================================
# Chapter
# ============================================================

@dataclass
class Chapter:
    """
    A chapter in the document — the unit of LLM generation.

    Each chapter is generated independently by the pipeline,
    then assembled into a full DocumentIR by the stitcher.
    """

    chapter_id: str
    title: str
    anchor: str
    order: int
    blocks: List[Block] = field(default_factory=list)
    summary: Optional[str] = None
    metadata: Optional[Dict[str, Any]] = None
    errors: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        d: Dict[str, Any] = {
            "chapterId": self.chapter_id,
            "title": self.title,
            "anchor": self.anchor,
            "order": self.order,
            "blocks": [b.to_dict() for b in self.blocks],
        }
        if self.summary:
            d["summary"] = self.summary
        if self.metadata:
            d["metadata"] = self.metadata
        if self.errors:
            d["errors"] = self.errors
        return d

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Chapter:
        blocks = [Block.from_dict(b) for b in data.get("blocks", [])]
        return cls(
            chapter_id=data["chapterId"],
            title=data["title"],
            anchor=data["anchor"],
            order=data.get("order", 0),
            blocks=blocks,
            summary=data.get("summary"),
            metadata=data.get("metadata"),
            errors=data.get("errors", []),
        )


# ============================================================
# Document metadata & theme
# ============================================================

@dataclass
class DocumentMetadata:
    """Top-level metadata for the entire document."""

    title: str = ""
    subtitle: Optional[str] = None
    query: str = ""
    generated_at: str = field(
        default_factory=lambda: datetime.now(timezone.utc).isoformat()
    )
    template_name: Optional[str] = None
    report_type: Optional[str] = None
    word_count: Optional[int] = None
    source_count: Optional[int] = None
    agent_count: Optional[int] = None

    def to_dict(self) -> Dict[str, Any]:
        return _filter_none({
            "title": self.title,
            "subtitle": self.subtitle,
            "query": self.query,
            "generatedAt": self.generated_at,
            "templateName": self.template_name,
            "reportType": self.report_type,
            "wordCount": self.word_count,
            "sourceCount": self.source_count,
            "agentCount": self.agent_count,
        })

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> DocumentMetadata:
        return cls(
            title=data.get("title", ""),
            subtitle=data.get("subtitle"),
            query=data.get("query", ""),
            generated_at=data.get("generatedAt", ""),
            template_name=data.get("templateName"),
            report_type=data.get("reportType"),
            word_count=data.get("wordCount"),
            source_count=data.get("sourceCount"),
            agent_count=data.get("agentCount"),
        )


@dataclass
class ThemeTokens:
    """Visual theme configuration for the rendered report."""

    colors: Dict[str, str] = field(default_factory=lambda: {
        "bg": "#f8f9fa",
        "text": "#212529",
        "primary": "#6366f1",
        "secondary": "#64748b",
        "card": "#ffffff",
        "border": "#e2e8f0",
        "accent1": "#0ea5e9",
        "accent2": "#22c55e",
        "accent3": "#f59e0b",
        "accent4": "#ef4444",
    })
    fonts: Dict[str, str] = field(default_factory=lambda: {
        "body": "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
        "heading": "'Inter', 'SF Pro Display', -apple-system, sans-serif",
        "mono": "'JetBrains Mono', 'Fira Code', monospace",
    })

    def to_dict(self) -> Dict[str, Any]:
        return {"colors": self.colors, "fonts": self.fonts}

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> ThemeTokens:
        return cls(
            colors=data.get("colors", {}),
            fonts=data.get("fonts", {}),
        )


@dataclass
class TOCConfig:
    """Table of contents configuration."""

    depth: int = 3
    auto_numbering: bool = True
    title: str = "Table of Contents"

    def to_dict(self) -> Dict[str, Any]:
        return {
            "depth": self.depth,
            "autoNumbering": self.auto_numbering,
            "title": self.title,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> TOCConfig:
        return cls(
            depth=data.get("depth", 3),
            auto_numbering=data.get("autoNumbering", True),
            title=data.get("title", "Table of Contents"),
        )


# ============================================================
# DocumentIR — the top-level document
# ============================================================

@dataclass
class DocumentIR:
    """
    The complete intermediate representation of a research report.

    This is the central data structure that flows through the pipeline:
    the stitcher assembles it from chapters, and the renderer consumes
    it to produce HTML (or other output formats).
    """

    report_id: str
    version: str = IR_VERSION
    metadata: DocumentMetadata = field(default_factory=DocumentMetadata)
    theme: ThemeTokens = field(default_factory=ThemeTokens)
    toc: TOCConfig = field(default_factory=TOCConfig)
    chapters: List[Chapter] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "version": self.version,
            "reportId": self.report_id,
            "metadata": self.metadata.to_dict(),
            "themeTokens": self.theme.to_dict(),
            "toc": self.toc.to_dict(),
            "chapters": [c.to_dict() for c in self.chapters],
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> DocumentIR:
        return cls(
            report_id=data.get("reportId", ""),
            version=data.get("version", IR_VERSION),
            metadata=DocumentMetadata.from_dict(data.get("metadata", {})),
            theme=ThemeTokens.from_dict(data.get("themeTokens", {})),
            toc=TOCConfig.from_dict(data.get("toc", {})),
            chapters=[Chapter.from_dict(c) for c in data.get("chapters", [])],
        )

    def to_json(self, indent: int = 2) -> str:
        return json.dumps(self.to_dict(), ensure_ascii=False, indent=indent)

    @classmethod
    def from_json(cls, text: str) -> DocumentIR:
        return cls.from_dict(json.loads(text))


# ============================================================
# IR Validator
# ============================================================

class IRValidator:
    """
    Validates chapter-level IR structures before assembly.

    Returns (is_valid, error_list) tuples. Errors use dotted path
    notation for easy debugging (e.g. "blocks[2].inlines[0].text").
    """

    def validate_chapter(self, chapter: Dict[str, Any]) -> Tuple[bool, List[str]]:
        """Validate a raw chapter dict (as produced by LLM JSON output)."""
        errors: List[str] = []

        if not isinstance(chapter, dict):
            return False, ["chapter must be a dict"]

        for required in ("chapterId", "title", "anchor", "order", "blocks"):
            if required not in chapter:
                errors.append(f"missing chapter.{required}")

        blocks = chapter.get("blocks")
        if not isinstance(blocks, list) or not blocks:
            errors.append("chapter.blocks must be a non-empty array")
            return False, errors

        for idx, block in enumerate(blocks):
            self._validate_block(block, f"blocks[{idx}]", errors)

        return len(errors) == 0, errors

    def validate_document(self, doc: Dict[str, Any]) -> Tuple[bool, List[str]]:
        """Validate a full DocumentIR dict."""
        errors: List[str] = []

        if not isinstance(doc, dict):
            return False, ["document must be a dict"]

        if "reportId" not in doc:
            errors.append("missing reportId")
        if "chapters" not in doc or not isinstance(doc.get("chapters"), list):
            errors.append("missing or invalid chapters array")
            return False, errors

        for idx, chapter in enumerate(doc["chapters"]):
            ch_valid, ch_errors = self.validate_chapter(chapter)
            if not ch_valid:
                errors.extend(f"chapters[{idx}].{e}" for e in ch_errors)

        return len(errors) == 0, errors

    # ---- Block validators ----

    def _validate_block(self, block: Any, path: str, errors: List[str]) -> None:
        if not isinstance(block, dict):
            errors.append(f"{path} must be a dict")
            return

        block_type = block.get("type")
        if block_type not in ALLOWED_BLOCK_TYPES:
            errors.append(f"{path}.type unsupported: {block_type}")
            return

        validator = getattr(self, f"_validate_{block_type}", None)
        if validator:
            validator(block, path, errors)

    def _validate_heading(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        if "level" not in block or not isinstance(block["level"], int):
            errors.append(f"{path}.level must be an integer")
        if "text" not in block:
            errors.append(f"{path}.text missing")
        if "anchor" not in block:
            errors.append(f"{path}.anchor missing")

    def _validate_paragraph(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        inlines = block.get("inlines")
        if not isinstance(inlines, list) or not inlines:
            errors.append(f"{path}.inlines must be a non-empty array")
            return
        for idx, run in enumerate(inlines):
            self._validate_inline_run(run, f"{path}.inlines[{idx}]", errors)

    def _validate_list(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        if block.get("listType") not in {"ordered", "bullet", "task"}:
            errors.append(f"{path}.listType must be ordered/bullet/task")
        items = block.get("items")
        if not isinstance(items, list) or not items:
            errors.append(f"{path}.items must be a non-empty array")
            return
        for i, item in enumerate(items):
            if not isinstance(item, list):
                errors.append(f"{path}.items[{i}] must be an array of blocks")
                continue
            for j, sub_block in enumerate(item):
                self._validate_block(sub_block, f"{path}.items[{i}][{j}]", errors)

    def _validate_table(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        rows = block.get("rows")
        if not isinstance(rows, list) or not rows:
            errors.append(f"{path}.rows must be a non-empty array")
            return
        for r_idx, row in enumerate(rows):
            cells = row.get("cells") if isinstance(row, dict) else None
            if not isinstance(cells, list) or not cells:
                errors.append(f"{path}.rows[{r_idx}].cells must be a non-empty array")
                continue
            for c_idx, cell in enumerate(cells):
                if not isinstance(cell, dict):
                    errors.append(f"{path}.rows[{r_idx}].cells[{c_idx}] must be a dict")
                    continue
                cell_blocks = cell.get("blocks")
                if not isinstance(cell_blocks, list) or not cell_blocks:
                    errors.append(f"{path}.rows[{r_idx}].cells[{c_idx}].blocks must be a non-empty array")
                    continue
                for b_idx, sub_block in enumerate(cell_blocks):
                    self._validate_block(
                        sub_block,
                        f"{path}.rows[{r_idx}].cells[{c_idx}].blocks[{b_idx}]",
                        errors,
                    )

    def _validate_chart(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        if "chartType" not in block:
            errors.append(f"{path}.chartType missing")
        if "chartData" not in block:
            errors.append(f"{path}.chartData missing")

    def _validate_callout(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        if block.get("tone") not in {"info", "warning", "success", "danger"}:
            errors.append(f"{path}.tone must be info/warning/success/danger")
        blocks = block.get("blocks")
        if not isinstance(blocks, list) or not blocks:
            errors.append(f"{path}.blocks must be a non-empty array")
            return
        for idx, sub in enumerate(blocks):
            self._validate_block(sub, f"{path}.blocks[{idx}]", errors)

    def _validate_kpiGrid(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        items = block.get("items")
        if not isinstance(items, list) or not items:
            errors.append(f"{path}.items must be a non-empty array")
            return
        for idx, item in enumerate(items):
            if not isinstance(item, dict):
                errors.append(f"{path}.items[{idx}] must be a dict")
                continue
            if "label" not in item or "value" not in item:
                errors.append(f"{path}.items[{idx}] requires label and value")

    def _validate_swotTable(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        quadrants = ("strengths", "weaknesses", "opportunities", "threats")
        if not any(block.get(q) is not None for q in quadrants):
            errors.append(f"{path} requires at least one SWOT quadrant")
        for name in quadrants:
            entries = block.get(name)
            if entries is None:
                continue
            if not isinstance(entries, list):
                errors.append(f"{path}.{name} must be an array")

    def _validate_pestTable(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        factors = ("political", "economic", "social", "technological")
        if not any(block.get(f) is not None for f in factors):
            errors.append(f"{path} requires at least one PEST factor")
        for name in factors:
            entries = block.get(name)
            if entries is None:
                continue
            if not isinstance(entries, list):
                errors.append(f"{path}.{name} must be an array")

    def _validate_blockquote(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        inner = block.get("blocks")
        if not isinstance(inner, list) or not inner:
            errors.append(f"{path}.blocks must be a non-empty array")
            return
        for idx, sub in enumerate(inner):
            self._validate_block(sub, f"{path}.blocks[{idx}]", errors)

    def _validate_agentQuote(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        agent = block.get("agent")
        if not isinstance(agent, str):
            errors.append(f"{path}.agent must be a string")
        if "title" not in block:
            errors.append(f"{path}.title missing")
        inner = block.get("blocks")
        if not isinstance(inner, list) or not inner:
            errors.append(f"{path}.blocks must be a non-empty array")
            return
        for idx, sub in enumerate(inner):
            self._validate_block(sub, f"{path}.blocks[{idx}]", errors)

    def _validate_code(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        if "content" not in block:
            errors.append(f"{path}.content missing")

    def _validate_math(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        if "latex" not in block:
            errors.append(f"{path}.latex missing")

    def _validate_figure(self, block: Dict[str, Any], path: str, errors: List[str]) -> None:
        img = block.get("img")
        if not isinstance(img, dict):
            errors.append(f"{path}.img must be a dict")
            return
        if "src" not in img:
            errors.append(f"{path}.img.src missing")

    def _validate_inline_run(self, run: Any, path: str, errors: List[str]) -> None:
        if not isinstance(run, dict):
            errors.append(f"{path} must be a dict")
            return
        if "text" not in run:
            errors.append(f"{path}.text missing")
        marks = run.get("marks")
        if marks is None:
            return
        if not isinstance(marks, list):
            errors.append(f"{path}.marks must be an array")
            return
        for m_idx, mark in enumerate(marks):
            if not isinstance(mark, dict):
                errors.append(f"{path}.marks[{m_idx}] must be a dict")
                continue
            if mark.get("type") not in ALLOWED_INLINE_MARKS:
                errors.append(f"{path}.marks[{m_idx}].type unsupported: {mark.get('type')}")


# ============================================================
# JSON Schema text (for embedding in LLM prompts)
# ============================================================

CHAPTER_JSON_SCHEMA: Dict[str, Any] = {
    "type": "object",
    "required": ["chapterId", "title", "anchor", "order", "blocks"],
    "properties": {
        "chapterId": {"type": "string"},
        "title": {"type": "string"},
        "anchor": {"type": "string"},
        "order": {"type": "number"},
        "summary": {"type": "string"},
        "blocks": {
            "type": "array",
            "items": {
                "type": "object",
                "required": ["type"],
                "properties": {
                    "type": {"type": "string", "enum": ALLOWED_BLOCK_TYPES},
                },
            },
        },
    },
}

CHAPTER_JSON_SCHEMA_TEXT: str = json.dumps(
    CHAPTER_JSON_SCHEMA, ensure_ascii=False, indent=2
)


__all__ = [
    "IR_VERSION",
    "ALLOWED_INLINE_MARKS",
    "ALLOWED_BLOCK_TYPES",
    "AGENT_TITLES",
    "InlineMark",
    "InlineRun",
    "Block",
    "Chapter",
    "DocumentMetadata",
    "ThemeTokens",
    "TOCConfig",
    "DocumentIR",
    "IRValidator",
    "CHAPTER_JSON_SCHEMA",
    "CHAPTER_JSON_SCHEMA_TEXT",
]
