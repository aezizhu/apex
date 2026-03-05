"""Tests for tool execution edge cases, decorator, and default registry."""

import pytest
from unittest.mock import AsyncMock, MagicMock

from apex_agents.tools import (
    Tool,
    ToolParameter,
    ToolRegistry,
    ToolResult,
)


class TestToolExecutionEdgeCases:
    """Tests for edge cases in tool execution."""

    @pytest.mark.asyncio
    async def test_tool_without_implementation(self):
        """Test executing a tool that has no func set."""
        tool_obj = Tool(
            name="empty_tool",
            description="A tool with no implementation",
            parameters=[],
            func=None,
        )

        result = await tool_obj.execute()

        assert result.success is False
        assert "no implementation" in result.error.lower()

    @pytest.mark.asyncio
    async def test_sync_function_tool(self):
        """Test executing a tool with a synchronous function."""
        def sync_func(x: int) -> str:
            return f"result is {x * 2}"

        tool_obj = Tool(
            name="sync_tool",
            description="A synchronous tool",
            parameters=[ToolParameter("x", "integer", "Input number")],
            func=sync_func,
        )

        result = await tool_obj.execute(x=5)

        assert result.success is True
        assert "result is 10" in result.output

    @pytest.mark.asyncio
    async def test_tool_execution_exception_returns_error_result(self):
        """Test that exceptions in tool func produce error ToolResult, not raised."""
        async def bad_func() -> str:
            raise RuntimeError("internal failure")

        tool_obj = Tool(
            name="bad_tool",
            description="Breaks",
            parameters=[],
            func=bad_func,
        )

        result = await tool_obj.execute()

        assert result.success is False
        assert "internal failure" in result.error

    def test_to_generic_schema(self):
        """Test the generic to_schema method."""
        tool_obj = Tool(
            name="my_tool",
            description="Does stuff",
            parameters=[
                ToolParameter("arg1", "string", "First arg", required=True),
                ToolParameter("arg2", "number", "Second arg", required=False),
            ],
        )

        schema = tool_obj.to_schema()

        assert schema["name"] == "my_tool"
        assert schema["description"] == "Does stuff"
        assert "arg1" in schema["parameters"]["properties"]
        assert "arg2" in schema["parameters"]["properties"]
        assert "arg1" in schema["parameters"]["required"]
        assert "arg2" not in schema["parameters"]["required"]

    def test_parameter_with_enum(self):
        """Test ToolParameter with enum values."""
        param = ToolParameter(
            name="format",
            type="string",
            description="Output format",
            enum=["json", "csv", "xml"],
        )

        schema = param.to_json_schema()

        assert schema["enum"] == ["json", "csv", "xml"]


class TestToolDecorator:
    """Tests for the @tool decorator."""

    def test_tool_decorator_creates_tool(self):
        """Test that the @tool decorator creates a Tool object."""
        from apex_agents.tools import tool

        @tool(
            name="decorated_tool",
            description="A decorated tool",
            parameters=[ToolParameter("x", "integer", "Input")],
        )
        async def my_tool(x: int) -> str:
            return f"result: {x}"

        assert isinstance(my_tool, Tool)
        assert my_tool.name == "decorated_tool"
        assert my_tool.description == "A decorated tool"
        assert my_tool.func is not None

    def test_tool_decorator_no_parameters(self):
        """Test decorator without explicit parameters list."""
        from apex_agents.tools import tool

        @tool(name="simple", description="Simple tool")
        async def simple_tool() -> str:
            return "done"

        assert simple_tool.parameters == []

    @pytest.mark.asyncio
    async def test_decorated_tool_execution(self):
        """Test that a decorated tool can be executed."""
        from apex_agents.tools import tool

        @tool(
            name="adder",
            description="Adds numbers",
            parameters=[
                ToolParameter("a", "integer", "First number"),
                ToolParameter("b", "integer", "Second number"),
            ],
        )
        async def adder(a: int, b: int) -> str:
            return str(a + b)

        result = await adder.execute(a=3, b=7)

        assert result.success is True
        assert result.output == "10"


class TestCreateDefaultRegistry:
    """Tests for create_default_registry."""

    def test_default_registry_contains_expected_tools(self):
        """Test that the default registry has all built-in tools."""
        from apex_agents.tools import create_default_registry

        registry = create_default_registry()

        expected_tools = [
            "web_search",
            "read_file",
            "write_file",
            "run_command",
            "http_request",
            "calculate",
        ]
        for name in expected_tools:
            assert registry.has(name), f"Missing tool: {name}"

        assert len(registry) == len(expected_tools)

    def test_default_registry_tools_have_schemas(self):
        """Test that all default tools produce valid schemas."""
        from apex_agents.tools import create_default_registry

        registry = create_default_registry()

        for fmt in ("openai", "anthropic", "generic"):
            schemas = registry.get_schemas(format=fmt)
            assert len(schemas) == 6
            for schema in schemas:
                assert isinstance(schema, dict)


class TestToolRegistryEdgeCases:
    """Tests for edge cases in ToolRegistry."""

    def test_list_names(self):
        """Test list_names returns tool names."""
        registry = ToolRegistry()
        tool_obj = Tool(name="t1", description="Test", func=None)
        registry.register(tool_obj)

        names = registry.list_names()
        assert names == ["t1"]

    def test_all_method(self):
        """Test the all() method returns all tools."""
        registry = ToolRegistry()
        t1 = Tool(name="t1", description="Test 1", func=None)
        t2 = Tool(name="t2", description="Test 2", func=None)
        registry.register(t1)
        registry.register(t2)

        all_tools = registry.all()
        assert len(all_tools) == 2

    def test_has_method(self):
        """Test the has() method."""
        registry = ToolRegistry()
        t1 = Tool(name="exists", description="Exists", func=None)
        registry.register(t1)

        assert registry.has("exists") is True
        assert registry.has("missing") is False

    def test_get_subset_ignores_missing(self):
        """Test that get_subset silently ignores missing tool names."""
        registry = ToolRegistry()
        t1 = Tool(name="present", description="Present", func=None)
        registry.register(t1)

        subset = registry.get_subset(["present", "absent"])
        assert len(subset) == 1
        assert subset[0].name == "present"


class TestCalculateTool:
    """Tests for the calculate built-in tool."""

    @pytest.mark.asyncio
    async def test_calculate_valid_expression(self):
        """Test calculate tool with valid expression."""
        from apex_agents.tools import calculate

        result = await calculate.func(expression="2 + 3 * 4")
        assert result == "14"

    @pytest.mark.asyncio
    async def test_calculate_invalid_characters(self):
        """Test calculate tool rejects expressions with invalid characters."""
        from apex_agents.tools import calculate

        result = await calculate.func(expression="import os")
        assert "invalid characters" in result.lower()

    @pytest.mark.asyncio
    async def test_calculate_division_by_zero(self):
        """Test calculate handles division by zero."""
        from apex_agents.tools import calculate

        result = await calculate.func(expression="1/0")
        assert "error" in result.lower()
