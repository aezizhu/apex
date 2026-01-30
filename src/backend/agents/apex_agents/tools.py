"""
Tool framework for agent capabilities.
"""

from __future__ import annotations

import asyncio
import json as json_module
import re
import subprocess
import time
from dataclasses import dataclass, field
import builtins
from typing import TYPE_CHECKING, Any, TypeVar
from urllib.parse import quote_plus

if TYPE_CHECKING:
    from collections.abc import Callable

import httpx
import structlog
from opentelemetry import trace

logger = structlog.get_logger()
tracer = trace.get_tracer(__name__)

T = TypeVar("T")


@dataclass
class ToolResult:
    """Result from executing a tool."""
    success: bool
    output: str
    error: str | None = None
    metadata: dict[str, Any] = field(default_factory=dict)


class ToolError(Exception):
    """Error raised by tool execution."""

    def __init__(self, tool_name: str, message: str, cause: Exception | None = None):
        self.tool_name = tool_name
        self.message = message
        self.cause = cause
        super().__init__(f"Tool '{tool_name}': {message}")


@dataclass
class ToolParameter:
    """Definition of a tool parameter."""
    name: str
    type: str  # "string", "number", "boolean", "array", "object", "integer"
    description: str
    required: bool = True
    enum: list[str] | None = None
    default: Any = None

    def to_json_schema(self) -> dict[str, Any]:
        """Convert parameter to JSON schema."""
        schema: dict[str, Any] = {
            "type": self.type,
            "description": self.description,
        }
        if self.enum:
            schema["enum"] = self.enum
        return schema


@dataclass
class Tool:
    """
    A tool that agents can use.

    Example:
        @tool(
            name="web_search",
            description="Search the web for information",
            parameters=[
                ToolParameter("query", "string", "The search query", required=True),
                ToolParameter("limit", "number", "Max results", required=False, default=5),
            ]
        )
        async def web_search(query: str, limit: int = 5) -> str:
            # Implementation
            pass
    """
    name: str
    description: str
    parameters: list[ToolParameter] = field(default_factory=list)
    func: Callable[..., Any] | None = None

    def _build_parameters_schema(self) -> dict[str, Any]:
        """Build the parameters JSON schema object."""
        properties = {}
        required = []

        for param in self.parameters:
            properties[param.name] = param.to_json_schema()
            if param.required:
                required.append(param.name)

        return {
            "type": "object",
            "properties": properties,
            "required": required,
        }

    def to_schema(self) -> dict[str, Any]:
        """Convert to generic tool schema."""
        return {
            "name": self.name,
            "description": self.description,
            "parameters": self._build_parameters_schema(),
        }

    def to_openai_schema(self) -> dict[str, Any]:
        """Convert to OpenAI function calling schema."""
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self._build_parameters_schema(),
            },
        }

    def to_anthropic_schema(self) -> dict[str, Any]:
        """Convert to Anthropic tool use schema."""
        return {
            "name": self.name,
            "description": self.description,
            "input_schema": self._build_parameters_schema(),
        }

    async def execute(self, **kwargs: Any) -> ToolResult:
        """Execute the tool with given arguments."""
        if self.func is None:
            return ToolResult(
                success=False,
                output="",
                error=f"Tool {self.name} has no implementation",
            )

        with tracer.start_as_current_span(
            f"tool_execute_{self.name}",
            attributes={"tool.name": self.name}
        ):
            try:
                # Handle both sync and async functions
                if asyncio.iscoroutinefunction(self.func):
                    result = await self.func(**kwargs)
                else:
                    result = self.func(**kwargs)
                return ToolResult(success=True, output=str(result))
            except Exception as e:
                return ToolResult(success=False, output="", error=str(e))


def tool(
    name: str,
    description: str,
    parameters: list[ToolParameter] | None = None,
) -> Callable[[Callable[..., T]], Tool]:
    """Decorator to create a tool from a function."""
    def decorator(func: Callable[..., T]) -> Tool:
        return Tool(
            name=name,
            description=description,
            parameters=parameters or [],
            func=func,
        )
    return decorator


class ToolRegistry:
    """Registry of available tools."""

    def __init__(self) -> None:
        self._tools: dict[str, Tool] = {}
        self._logger = logger.bind(component="tool_registry")

    def __len__(self) -> int:
        """Return the number of registered tools."""
        return len(self._tools)

    def __contains__(self, name: str) -> bool:
        """Check if a tool is registered."""
        return name in self._tools

    def register(self, tool_obj: Tool) -> None:
        """Register a tool. Raises ValueError if already registered."""
        if tool_obj.name in self._tools:
            raise ValueError(f"Tool already registered: {tool_obj.name}")
        self._tools[tool_obj.name] = tool_obj
        self._logger.info("Tool registered", tool=tool_obj.name)

    def get(self, name: str) -> Tool | None:
        """Get a tool by name. Returns None if not found."""
        return self._tools.get(name)

    def has(self, name: str) -> bool:
        """Check if a tool exists."""
        return name in self._tools

    def list(self) -> list[Tool]:
        """List all registered tools."""
        return list(self._tools.values())

    def list_names(self) -> builtins.list[str]:
        """List all registered tool names."""
        return builtins.list(self._tools.keys())

    def all(self) -> builtins.list[Tool]:
        """Get all registered tools."""
        return builtins.list(self._tools.values())

    def get_schemas(self, format: str = "openai") -> builtins.list[dict[str, Any]]:
        """Get schemas for all tools in the specified format."""
        if format == "openai":
            return [t.to_openai_schema() for t in self._tools.values()]
        elif format == "anthropic":
            return [t.to_anthropic_schema() for t in self._tools.values()]
        else:
            return [t.to_schema() for t in self._tools.values()]

    def get_subset(self, names: builtins.list[str]) -> builtins.list[Tool]:
        """Get a subset of tools by name."""
        return [self._tools[n] for n in names if n in self._tools]

    async def execute(self, name: str, **kwargs: Any) -> ToolResult:
        """Execute a tool by name."""
        tool_obj = self.get(name)
        if tool_obj is None:
            return ToolResult(
                success=False,
                output="",
                error=f"Tool not found: {name}",
            )
        return await tool_obj.execute(**kwargs)


# ═══════════════════════════════════════════════════════════════════════════════
# Built-in Tools
# ═══════════════════════════════════════════════════════════════════════════════

# Rate limiting state for web search
_last_search_time: float = 0.0
_SEARCH_MIN_INTERVAL: float = 1.0  # Minimum seconds between searches


@tool(
    name="web_search",
    description="Search the web for information using DuckDuckGo",
    parameters=[
        ToolParameter("query", "string", "The search query"),
        ToolParameter("num_results", "number", "Number of results to return", required=False),
    ]
)
async def web_search(query: str, num_results: int = 5) -> str:
    """Search the web using DuckDuckGo HTML search."""
    global _last_search_time

    # Rate limiting
    now = time.monotonic()
    elapsed = now - _last_search_time
    if elapsed < _SEARCH_MIN_INTERVAL:
        await asyncio.sleep(_SEARCH_MIN_INTERVAL - elapsed)
    _last_search_time = time.monotonic()

    search_url = f"https://html.duckduckgo.com/html/?q={quote_plus(query)}"

    try:
        async with httpx.AsyncClient(
            timeout=10.0,
            follow_redirects=True,
            headers={
                "User-Agent": (
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
                    "AppleWebKit/537.36 (KHTML, like Gecko) "
                    "Chrome/120.0.0.0 Safari/537.36"
                ),
            },
        ) as client:
            response = await client.get(search_url)
            response.raise_for_status()
            html = response.text

        results = _parse_duckduckgo_results(html, num_results)

        if not results:
            return f"No results found for '{query}'."

        # Format results as structured text
        return json_module.dumps(results, indent=2)

    except httpx.TimeoutException:
        return f"Search timed out for query: '{query}'"
    except httpx.HTTPStatusError as e:
        return f"Search request failed with status {e.response.status_code}"
    except Exception as e:
        return f"Search failed: {e}"


def _parse_duckduckgo_results(html: str, max_results: int = 5) -> list[dict[str, str]]:
    """
    Parse search results from DuckDuckGo HTML response.

    Returns a list of dicts with 'title', 'url', and 'snippet' keys.
    """
    results: list[dict[str, str]] = []

    # DuckDuckGo HTML search returns results in <a class="result__a"> tags
    # and snippets in <a class="result__snippet"> tags
    # Each result is in a <div class="result results_links results_links_deep web-result">

    # Extract result blocks
    result_blocks = re.findall(
        r'<div[^>]*class="[^"]*result[^"]*links[^"]*"[^>]*>(.*?)</div>\s*</div>',
        html,
        re.DOTALL,
    )

    # If block-level parsing fails, try individual element parsing
    if not result_blocks:
        # Fallback: extract links and snippets directly
        links = re.findall(
            r'<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>(.*?)</a>',
            html,
            re.DOTALL,
        )
        snippets = re.findall(
            r'<a[^>]*class="result__snippet"[^>]*>(.*?)</a>',
            html,
            re.DOTALL,
        )

        for i, (url, title) in enumerate(links[:max_results]):
            clean_title = re.sub(r"<[^>]+>", "", title).strip()
            clean_snippet = ""
            if i < len(snippets):
                clean_snippet = re.sub(r"<[^>]+>", "", snippets[i]).strip()

            if clean_title and url:
                results.append({
                    "title": clean_title,
                    "url": url,
                    "snippet": clean_snippet,
                })
        return results

    for block in result_blocks[:max_results]:
        # Extract URL and title
        link_match = re.search(
            r'<a[^>]*class="result__a"[^>]*href="([^"]*)"[^>]*>(.*?)</a>',
            block,
            re.DOTALL,
        )
        # Extract snippet
        snippet_match = re.search(
            r'<a[^>]*class="result__snippet"[^>]*>(.*?)</a>',
            block,
            re.DOTALL,
        )

        if link_match:
            url = link_match.group(1)
            title = re.sub(r"<[^>]+>", "", link_match.group(2)).strip()
            snippet = ""
            if snippet_match:
                snippet = re.sub(r"<[^>]+>", "", snippet_match.group(1)).strip()

            if title and url:
                results.append({
                    "title": title,
                    "url": url,
                    "snippet": snippet,
                })

    return results


@tool(
    name="read_file",
    description="Read the contents of a file",
    parameters=[
        ToolParameter("path", "string", "The file path to read"),
    ]
)
async def read_file(path: str) -> str:
    """Read a file's contents."""
    try:
        with open(path, "r") as f:
            content = f.read()
        return content[:10000]  # Limit size
    except Exception as e:
        return f"Error reading file: {e}"


@tool(
    name="write_file",
    description="Write content to a file",
    parameters=[
        ToolParameter("path", "string", "The file path to write to"),
        ToolParameter("content", "string", "The content to write"),
    ]
)
async def write_file(path: str, content: str) -> str:
    """Write content to a file."""
    try:
        with open(path, "w") as f:
            f.write(content)
        return f"Successfully wrote to {path}"
    except Exception as e:
        return f"Error writing file: {e}"


@tool(
    name="run_command",
    description="Run a shell command in a sandboxed environment",
    parameters=[
        ToolParameter("command", "string", "The command to run"),
        ToolParameter("timeout", "number", "Timeout in seconds", required=False),
    ]
)
async def run_command(command: str, timeout: int = 30) -> str:
    """Run a shell command (with safety limits)."""
    try:
        result = subprocess.run(
            command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        output = result.stdout + result.stderr
        return output[:5000]  # Limit output size
    except subprocess.TimeoutExpired:
        return f"Command timed out after {timeout} seconds"
    except Exception as e:
        return f"Error running command: {e}"


@tool(
    name="http_request",
    description="Make an HTTP request to a URL",
    parameters=[
        ToolParameter("url", "string", "The URL to request"),
        ToolParameter("method", "string", "HTTP method", required=False),
        ToolParameter("body", "string", "Request body (for POST/PUT)", required=False),
    ]
)
async def http_request(url: str, method: str = "GET", body: str | None = None) -> str:
    """Make an HTTP request."""
    async with httpx.AsyncClient(timeout=30.0) as client:
        try:
            if method.upper() == "GET":
                response = await client.get(url)
            elif method.upper() == "POST":
                response = await client.post(url, content=body)
            elif method.upper() == "PUT":
                response = await client.put(url, content=body)
            elif method.upper() == "DELETE":
                response = await client.delete(url)
            else:
                return f"Unsupported method: {method}"

            return f"Status: {response.status_code}\n\n{response.text[:5000]}"
        except Exception as e:
            return f"Request failed: {e}"


@tool(
    name="calculate",
    description="Perform a mathematical calculation",
    parameters=[
        ToolParameter("expression", "string", "The mathematical expression to evaluate"),
    ]
)
async def calculate(expression: str) -> str:
    """Evaluate a mathematical expression safely."""
    try:
        # Only allow safe math operations
        allowed = set("0123456789+-*/().% ")
        if not all(c in allowed for c in expression):
            return "Error: Expression contains invalid characters"

        result = eval(expression, {"__builtins__": {}}, {})
        return str(result)
    except Exception as e:
        return f"Calculation error: {e}"


def create_default_registry() -> ToolRegistry:
    """Create a registry with default tools."""
    registry = ToolRegistry()
    registry.register(web_search)
    registry.register(read_file)
    registry.register(write_file)
    registry.register(run_command)
    registry.register(http_request)
    registry.register(calculate)
    return registry
