"""Tests for the Tools framework."""

import json
from unittest.mock import AsyncMock, MagicMock, patch

import httpx
import pytest

from apex_agents.tools import (
    Tool,
    ToolError,
    ToolParameter,
    ToolRegistry,
    ToolResult,
    _parse_duckduckgo_results,
    web_search,
)


class TestToolParameter:
    """Tests for ToolParameter."""

    def test_required_parameter(self):
        """Test required parameter creation."""
        param = ToolParameter(
            name="query",
            type="string",
            description="The search query",
        )

        assert param.name == "query"
        assert param.type == "string"
        assert param.description == "The search query"
        assert param.required is True
        assert param.default is None

    def test_optional_parameter(self):
        """Test optional parameter with default."""
        param = ToolParameter(
            name="limit",
            type="integer",
            description="Max results",
            required=False,
            default=10,
        )

        assert param.required is False
        assert param.default == 10

    def test_to_json_schema(self):
        """Test conversion to JSON schema."""
        param = ToolParameter(
            name="count",
            type="integer",
            description="Number of items",
        )

        schema = param.to_json_schema()

        assert schema["type"] == "integer"
        assert schema["description"] == "Number of items"


class TestTool:
    """Tests for Tool."""

    @pytest.fixture
    def sample_tool(self):
        """Create a sample tool."""
        async def search_func(query: str, limit: int = 5) -> str:
            return f"Results for '{query}' (limit: {limit})"

        return Tool(
            name="web_search",
            description="Search the web for information",
            parameters=[
                ToolParameter("query", "string", "The search query"),
                ToolParameter("limit", "integer", "Max results", required=False, default=5),
            ],
            func=search_func,
        )

    def test_tool_creation(self, sample_tool):
        """Test tool creation."""
        assert sample_tool.name == "web_search"
        assert sample_tool.description == "Search the web for information"
        assert len(sample_tool.parameters) == 2

    def test_to_openai_schema(self, sample_tool):
        """Test conversion to OpenAI function schema."""
        schema = sample_tool.to_openai_schema()

        assert schema["type"] == "function"
        assert schema["function"]["name"] == "web_search"
        assert schema["function"]["description"] == "Search the web for information"
        assert "parameters" in schema["function"]
        assert "query" in schema["function"]["parameters"]["properties"]

    def test_to_anthropic_schema(self, sample_tool):
        """Test conversion to Anthropic tool schema."""
        schema = sample_tool.to_anthropic_schema()

        assert schema["name"] == "web_search"
        assert schema["description"] == "Search the web for information"
        assert "input_schema" in schema
        assert schema["input_schema"]["type"] == "object"

    @pytest.mark.asyncio
    async def test_tool_execution(self, sample_tool):
        """Test tool execution."""
        result = await sample_tool.execute(query="test query", limit=3)

        assert result.success is True
        assert "test query" in result.output
        assert "limit: 3" in result.output

    @pytest.mark.asyncio
    async def test_tool_execution_with_defaults(self, sample_tool):
        """Test tool execution with default parameter."""
        result = await sample_tool.execute(query="another query")

        assert result.success is True
        assert "limit: 5" in result.output

    @pytest.mark.asyncio
    async def test_tool_execution_error(self):
        """Test tool execution error handling."""
        async def failing_func(x: int) -> str:
            raise ValueError("Something went wrong")

        tool = Tool(
            name="failing_tool",
            description="A tool that fails",
            parameters=[ToolParameter("x", "integer", "Input")],
            func=failing_func,
        )

        result = await tool.execute(x=5)

        assert result.success is False
        assert "Something went wrong" in result.error


class TestToolRegistry:
    """Tests for ToolRegistry."""

    @pytest.fixture
    def registry(self):
        """Create a tool registry."""
        return ToolRegistry()

    @pytest.fixture
    def sample_tools(self):
        """Create sample tools."""
        async def search(query: str) -> str:
            return f"Search: {query}"

        async def read_file(path: str) -> str:
            return f"Content of {path}"

        async def write_file(path: str, content: str) -> str:
            return f"Wrote to {path}"

        return [
            Tool(
                name="search",
                description="Search tool",
                parameters=[ToolParameter("query", "string", "Query")],
                func=search,
            ),
            Tool(
                name="read_file",
                description="Read file tool",
                parameters=[ToolParameter("path", "string", "File path")],
                func=read_file,
            ),
            Tool(
                name="write_file",
                description="Write file tool",
                parameters=[
                    ToolParameter("path", "string", "File path"),
                    ToolParameter("content", "string", "Content"),
                ],
                func=write_file,
            ),
        ]

    def test_register_tool(self, registry, sample_tools):
        """Test registering tools."""
        for tool in sample_tools:
            registry.register(tool)

        assert len(registry) == 3
        assert "search" in registry
        assert "read_file" in registry
        assert "write_file" in registry

    def test_get_tool(self, registry, sample_tools):
        """Test getting a tool by name."""
        for tool in sample_tools:
            registry.register(tool)

        tool = registry.get("search")
        assert tool is not None
        assert tool.name == "search"

    def test_get_nonexistent_tool(self, registry):
        """Test getting a tool that doesn't exist."""
        tool = registry.get("nonexistent")
        assert tool is None

    def test_list_tools(self, registry, sample_tools):
        """Test listing all tools."""
        for tool in sample_tools:
            registry.register(tool)

        tools = registry.list()
        assert len(tools) == 3
        names = [t.name for t in tools]
        assert "search" in names
        assert "read_file" in names
        assert "write_file" in names

    def test_get_schemas_openai(self, registry, sample_tools):
        """Test getting OpenAI schemas for all tools."""
        for tool in sample_tools:
            registry.register(tool)

        schemas = registry.get_schemas(format="openai")
        assert len(schemas) == 3
        for schema in schemas:
            assert schema["type"] == "function"

    def test_get_schemas_anthropic(self, registry, sample_tools):
        """Test getting Anthropic schemas for all tools."""
        for tool in sample_tools:
            registry.register(tool)

        schemas = registry.get_schemas(format="anthropic")
        assert len(schemas) == 3
        for schema in schemas:
            assert "input_schema" in schema

    def test_get_subset(self, registry, sample_tools):
        """Test getting a subset of tools."""
        for tool in sample_tools:
            registry.register(tool)

        subset = registry.get_subset(["search", "read_file"])
        assert len(subset) == 2
        names = [t.name for t in subset]
        assert "search" in names
        assert "read_file" in names
        assert "write_file" not in names

    @pytest.mark.asyncio
    async def test_execute_tool(self, registry, sample_tools):
        """Test executing a tool through the registry."""
        for tool in sample_tools:
            registry.register(tool)

        result = await registry.execute("search", query="test")
        assert result.success is True
        assert "Search: test" in result.output

    @pytest.mark.asyncio
    async def test_execute_nonexistent_tool(self, registry):
        """Test executing a tool that doesn't exist."""
        result = await registry.execute("nonexistent", arg="value")
        assert result.success is False
        assert "not found" in result.error.lower()

    def test_duplicate_registration(self, registry, sample_tools):
        """Test registering a tool with the same name."""
        registry.register(sample_tools[0])

        with pytest.raises(ValueError) as exc_info:
            registry.register(sample_tools[0])

        assert "already registered" in str(exc_info.value).lower()


class TestToolResult:
    """Tests for ToolResult."""

    def test_successful_result(self):
        """Test creating a successful result."""
        result = ToolResult(
            success=True,
            output="Operation completed",
        )

        assert result.success is True
        assert result.output == "Operation completed"
        assert result.error is None

    def test_failed_result(self):
        """Test creating a failed result."""
        result = ToolResult(
            success=False,
            output="",
            error="Something went wrong",
        )

        assert result.success is False
        assert result.error == "Something went wrong"

    def test_result_with_metadata(self):
        """Test result with additional metadata."""
        result = ToolResult(
            success=True,
            output="Found 5 results",
            metadata={"count": 5, "source": "web"},
        )

        assert result.metadata["count"] == 5
        assert result.metadata["source"] == "web"


class TestToolError:
    """Tests for ToolError."""

    def test_tool_error_creation(self):
        """Test creating a ToolError."""
        error = ToolError("search", "Query too long")

        assert error.tool_name == "search"
        assert error.message == "Query too long"
        assert "search" in str(error)
        assert "Query too long" in str(error)

    def test_tool_error_with_cause(self):
        """Test ToolError with underlying cause."""
        cause = ValueError("Invalid input")
        error = ToolError("process", "Processing failed", cause=cause)

        assert error.cause is cause


class TestParseDuckDuckGoResults:
    """Tests for the DuckDuckGo HTML result parser."""

    def test_parse_results_with_links_and_snippets(self):
        """Test parsing HTML with result links and snippets."""
        html = """
        <a class="result__a" href="https://example.com/page1">Example Page 1</a>
        <a class="result__snippet">This is the first snippet.</a>
        <a class="result__a" href="https://example.com/page2">Example Page 2</a>
        <a class="result__snippet">This is the second snippet.</a>
        """

        results = _parse_duckduckgo_results(html, max_results=5)

        assert len(results) == 2
        assert results[0]["title"] == "Example Page 1"
        assert results[0]["url"] == "https://example.com/page1"
        assert results[0]["snippet"] == "This is the first snippet."
        assert results[1]["title"] == "Example Page 2"

    def test_parse_results_limits_count(self):
        """Test that parser respects max_results."""
        html = ""
        for i in range(10):
            html += f'<a class="result__a" href="https://example.com/{i}">Page {i}</a>\n'
            html += f'<a class="result__snippet">Snippet {i}</a>\n'

        results = _parse_duckduckgo_results(html, max_results=3)

        assert len(results) == 3

    def test_parse_empty_html(self):
        """Test parsing empty HTML returns no results."""
        results = _parse_duckduckgo_results("", max_results=5)
        assert results == []

    def test_parse_html_strips_tags(self):
        """Test that HTML tags are stripped from titles and snippets."""
        html = """
        <a class="result__a" href="https://example.com"><b>Bold</b> Title</a>
        <a class="result__snippet">Some <em>italic</em> text.</a>
        """

        results = _parse_duckduckgo_results(html, max_results=5)

        assert len(results) == 1
        assert results[0]["title"] == "Bold Title"
        assert results[0]["snippet"] == "Some italic text."


class TestWebSearchTool:
    """Tests for the web_search tool."""

    @pytest.mark.asyncio
    async def test_web_search_success(self):
        """Test successful web search with mocked response."""
        mock_html = """
        <a class="result__a" href="https://example.com/test">Test Result</a>
        <a class="result__snippet">A test snippet from example.com</a>
        <a class="result__a" href="https://other.com/page">Other Result</a>
        <a class="result__snippet">Another snippet here</a>
        """

        mock_response = MagicMock()
        mock_response.text = mock_html
        mock_response.raise_for_status = MagicMock()

        with patch("apex_agents.tools.httpx.AsyncClient") as mock_client_cls:
            mock_client = AsyncMock()
            mock_client.get.return_value = mock_response
            mock_client.__aenter__ = AsyncMock(return_value=mock_client)
            mock_client.__aexit__ = AsyncMock(return_value=False)
            mock_client_cls.return_value = mock_client

            # Reset rate limiter
            import apex_agents.tools
            apex_agents.tools._last_search_time = 0.0

            result = await web_search.func(query="test query", num_results=5)

        parsed = json.loads(result)
        assert len(parsed) == 2
        assert parsed[0]["title"] == "Test Result"
        assert parsed[0]["url"] == "https://example.com/test"

    @pytest.mark.asyncio
    async def test_web_search_timeout(self):
        """Test web search handles timeout."""
        with patch("apex_agents.tools.httpx.AsyncClient") as mock_client_cls:
            mock_client = AsyncMock()
            mock_client.get.side_effect = httpx.TimeoutException("timed out")
            mock_client.__aenter__ = AsyncMock(return_value=mock_client)
            mock_client.__aexit__ = AsyncMock(return_value=False)
            mock_client_cls.return_value = mock_client

            import apex_agents.tools
            apex_agents.tools._last_search_time = 0.0

            result = await web_search.func(query="test", num_results=5)

        assert "timed out" in result.lower()

    @pytest.mark.asyncio
    async def test_web_search_no_results(self):
        """Test web search with no results."""
        mock_response = MagicMock()
        mock_response.text = "<html><body>No results</body></html>"
        mock_response.raise_for_status = MagicMock()

        with patch("apex_agents.tools.httpx.AsyncClient") as mock_client_cls:
            mock_client = AsyncMock()
            mock_client.get.return_value = mock_response
            mock_client.__aenter__ = AsyncMock(return_value=mock_client)
            mock_client.__aexit__ = AsyncMock(return_value=False)
            mock_client_cls.return_value = mock_client

            import apex_agents.tools
            apex_agents.tools._last_search_time = 0.0

            result = await web_search.func(query="obscure query xyz", num_results=5)

        assert "no results" in result.lower()

    @pytest.mark.asyncio
    async def test_web_search_http_error(self):
        """Test web search handles HTTP errors."""
        mock_response = MagicMock()
        mock_response.status_code = 503
        http_error = httpx.HTTPStatusError(
            "Service Unavailable",
            request=MagicMock(),
            response=mock_response,
        )

        with patch("apex_agents.tools.httpx.AsyncClient") as mock_client_cls:
            mock_client = AsyncMock()
            mock_client.get.side_effect = http_error
            mock_client.__aenter__ = AsyncMock(return_value=mock_client)
            mock_client.__aexit__ = AsyncMock(return_value=False)
            mock_client_cls.return_value = mock_client

            import apex_agents.tools
            apex_agents.tools._last_search_time = 0.0

            result = await web_search.func(query="test", num_results=5)

        assert "503" in result
