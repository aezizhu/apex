"""
Markdown template parser for Apex research reports.

Parses a markdown template into a structured list of sections that
drive per-chapter LLM generation. Supports:

  - `# Heading` and `## Heading` syntax (depth 1 & 2)
  - Bullet list outlines (`- item`) indented under sections
  - Numbered sections like `1.0 Title` or `1.1 Subtitle`

Each top-level section becomes a separate chapter for the LLM pipeline.
Sub-headings and bullets become the chapter's outline hints.
"""

from __future__ import annotations

import re
import unicodedata
from dataclasses import dataclass, field
from typing import Dict, List, Optional

SECTION_ORDER_STEP = 10


@dataclass
class TemplateSection:
    """
    A parsed section from a markdown template.

    Attributes:
        title: Display title (may include number prefix).
        slug: URL-friendly anchor identifier.
        order: Numeric ordering for chapter sequencing.
        depth: Heading depth (1 = top-level chapter, 2+ = subsection).
        raw_title: Original unprocessed title text.
        number: Extracted section number (e.g. "1.0", "2.3").
        chapter_id: Stable chapter identifier (e.g. "S1", "S2").
        outline: Sub-items parsed from indented bullets/sub-headings.
    """

    title: str
    slug: str
    order: int
    depth: int
    raw_title: str
    number: str = ""
    chapter_id: str = ""
    outline: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, object]:
        return {
            "title": self.title,
            "slug": self.slug,
            "order": self.order,
            "depth": self.depth,
            "number": self.number,
            "chapterId": self.chapter_id,
            "outline": self.outline,
        }


# Regex patterns — intentionally avoid `.*` to prevent ReDoS.

_heading_re = re.compile(
    r"(?P<marker>\#{1,6})[ \t]+(?P<title>[^\r\n]+)",
)

_bullet_re = re.compile(
    r"(?P<marker>[-*+])[ \t]+(?P<title>[^\r\n]+)",
)

_number_re = re.compile(
    r"""
    (?P<num>
        (?:0|[1-9]\d*)
        (?:\.(?:0|[1-9]\d*))*
    )
    (?:
        (?:[ \t.:\-]+)
        (?P<label>[^\r\n]*)
    )?
    """,
    re.VERBOSE,
)


def parse_template_sections(template_md: str) -> List[TemplateSection]:
    """
    Parse a markdown template into a list of chapter sections.

    Top-level headings (# and ##) and top-level numbered/bullet items
    become chapters. Indented bullets and sub-headings become outline
    items within the preceding chapter.

    Args:
        template_md: Full markdown template text.

    Returns:
        Ordered list of TemplateSection objects with stable chapter IDs.
    """
    sections: List[TemplateSection] = []
    current: Optional[TemplateSection] = None
    order = SECTION_ORDER_STEP
    used_slugs: set[str] = set()

    for raw_line in template_md.splitlines():
        if not raw_line.strip():
            continue

        indent = len(raw_line) - len(raw_line.lstrip(" "))
        stripped = raw_line.strip()

        meta = _classify_line(stripped, indent)
        if not meta:
            continue

        if meta["is_section"]:
            slug = _ensure_unique_slug(meta["slug"], used_slugs)
            section = TemplateSection(
                title=meta["title"],
                slug=slug,
                order=order,
                depth=meta["depth"],
                raw_title=meta["raw"],
                number=meta["number"],
            )
            sections.append(section)
            current = section
            order += SECTION_ORDER_STEP
        elif current is not None:
            current.outline.append(meta["title"])

    # Assign stable chapter IDs
    for idx, section in enumerate(sections, start=1):
        section.chapter_id = f"S{idx}"

    return sections


def _classify_line(stripped: str, indent: int) -> Optional[Dict[str, object]]:
    """Classify a line as section header, outline item, or irrelevant."""

    # Markdown heading
    m = _heading_re.fullmatch(stripped)
    if m:
        level = len(m.group("marker"))
        payload = _strip_markup(m.group("title").strip())
        info = _split_number(payload)
        slug = _build_slug(info["number"], info["title"])
        return {
            "is_section": level <= 2,
            "depth": level,
            "title": info["display"],
            "raw": payload,
            "number": info["number"],
            "slug": slug,
        }

    # Bullet item
    m = _bullet_re.fullmatch(stripped)
    if m:
        payload = _strip_markup(m.group("title").strip())
        info = _split_number(payload)
        slug = _build_slug(info["number"], info["title"])
        is_section = indent <= 1
        return {
            "is_section": is_section,
            "depth": 1 if is_section else 2,
            "title": info["display"],
            "raw": payload,
            "number": info["number"],
            "slug": slug,
        }

    # Bare numbered line (e.g. "1.1 Market Analysis")
    m = _number_re.fullmatch(stripped)
    if m and m.group("label"):
        title = m.group("label").strip()
        number = m.group("num")
        slug = _build_slug(number, title)
        is_section = indent == 0 and number.count(".") <= 1
        display = f"{number} {title}" if title else number
        return {
            "is_section": is_section,
            "depth": 1 if is_section else 2,
            "title": display,
            "raw": stripped,
            "number": number,
            "slug": slug,
        }

    return None


def _strip_markup(text: str) -> str:
    """Remove wrapping bold/italic markers (**text** or __text__)."""
    if text.startswith(("**", "__")) and text.endswith(("**", "__")) and len(text) > 4:
        return text[2:-2].strip()
    return text


def _split_number(payload: str) -> Dict[str, str]:
    """Split a section number prefix from the title text."""
    m = _number_re.fullmatch(payload)
    number = m.group("num") if m else ""
    label = m.group("label") if m else payload
    label = (label or "").strip()
    display = f"{number} {label}".strip() if number else label or payload
    return {"number": number, "title": label or payload, "display": display}


def _build_slug(number: str, title: str) -> str:
    """Generate a URL-friendly anchor from number or title."""
    if number:
        token = number.replace(".", "-")
    else:
        token = _slugify(title)
    return f"section-{token or 'section'}"


def _slugify(text: str) -> str:
    """Convert arbitrary text to a URL-safe slug."""
    text = unicodedata.normalize("NFKD", text)
    text = text.replace(" ", "-").replace("·", "-")
    text = re.sub(r"[^0-9a-zA-Z\u4e00-\u9fff-]+", "-", text)
    text = re.sub(r"-{2,}", "-", text)
    return text.strip("-").lower()


def _ensure_unique_slug(slug: str, used: set) -> str:
    """Append a numeric suffix if the slug is already taken."""
    if slug not in used:
        used.add(slug)
        return slug
    base = slug
    idx = 2
    while slug in used:
        slug = f"{base}-{idx}"
        idx += 1
    used.add(slug)
    return slug


__all__ = ["TemplateSection", "parse_template_sections"]
