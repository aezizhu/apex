"""Tests for OpenTelemetry tracing utilities."""

from unittest.mock import MagicMock, patch

import pytest
from opentelemetry.trace import SpanKind, StatusCode

from apex_agents.config import Settings, TracingConfig
from apex_agents.tracing import (
    TaskSpanContext,
    add_span_attributes,
    create_span,
    current_span,
    extract_context,
    get_trace_context,
    get_tracer,
    init_tracing,
    inject_context,
    record_exception,
    set_span_status,
    set_trace_context,
    shutdown_tracing,
    traced,
    traced_async,
)


@pytest.fixture
def mock_settings():
    """Create mock settings."""
    with patch.dict(
        "os.environ",
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_TRACING_ENABLED": "true",
            "APEX_TRACING_SERVICE_NAME": "test-service",
        },
    ):
        return Settings()


class TestInitTracing:
    """Tests for tracing initialization."""

    def test_init_tracing_disabled(self):
        """Test tracing initialization when disabled."""
        with patch.dict("os.environ", {"APEX_LLM_OPENAI_API_KEY": "sk-test"}):
            settings = Settings()
            settings.tracing.enabled = False

            tracer = init_tracing(settings)

            assert tracer is not None

    def test_init_tracing_with_otlp(self):
        """Test tracing initialization with OTLP endpoint."""
        with patch.dict(
            "os.environ",
            {
                "APEX_LLM_OPENAI_API_KEY": "sk-test",
                "APEX_TRACING_ENABLED": "true",
                "APEX_TRACING_OTLP_ENDPOINT": "http://localhost:4317",
            },
        ):
            settings = Settings()

            with patch("apex_agents.tracing.OTLPSpanExporter") as mock_exporter:
                with patch("apex_agents.tracing.BatchSpanProcessor"):
                    tracer = init_tracing(settings)

                    assert tracer is not None
                    mock_exporter.assert_called_once()

    def test_init_tracing_with_console(self, mock_settings):
        """Test tracing initialization with console export."""
        mock_settings.tracing.console_export = True

        with patch("apex_agents.tracing.ConsoleSpanExporter") as mock_exporter:
            with patch("apex_agents.tracing.SimpleSpanProcessor"):
                tracer = init_tracing(mock_settings)

                assert tracer is not None


class TestGetTracer:
    """Tests for get_tracer function."""

    def test_get_tracer_default(self):
        """Test getting default tracer."""
        tracer = get_tracer()
        assert tracer is not None

    def test_get_tracer_named(self):
        """Test getting named tracer."""
        tracer = get_tracer("custom-tracer")
        assert tracer is not None


class TestContextPropagation:
    """Tests for context propagation."""

    def test_inject_and_extract_context(self):
        """Test injecting and extracting trace context."""
        # First inject context
        carrier = inject_context({})

        # May or may not have traceparent depending on active span
        assert isinstance(carrier, dict)

    def test_inject_context_creates_dict(self):
        """Test inject_context creates dict if none provided."""
        carrier = inject_context()
        assert isinstance(carrier, dict)

    def test_get_trace_context(self):
        """Test getting current trace context."""
        context = get_trace_context()
        assert isinstance(context, dict)

    def test_set_trace_context_with_ids(self):
        """Test setting trace context from IDs."""
        trace_id = "00000000000000000000000000000001"
        span_id = "0000000000000002"

        context = set_trace_context(trace_id, span_id)

        # Context should be set
        assert context is not None

    def test_set_trace_context_without_ids(self):
        """Test setting trace context without IDs."""
        context = set_trace_context(None, None)
        assert context is None


class TestSpanUtilities:
    """Tests for span utility functions."""

    def test_create_span(self):
        """Test creating a span."""
        span = create_span(
            "test-span",
            kind=SpanKind.INTERNAL,
            attributes={"key": "value"},
        )

        assert span is not None
        span.end()  # Clean up

    def test_current_span(self):
        """Test getting current span."""
        span = current_span()
        # Should return a span (possibly a NoopSpan)
        assert span is not None

    def test_add_span_attributes(self):
        """Test adding span attributes."""
        # Should not raise even without active span
        add_span_attributes({"key": "value", "count": 42})

    def test_record_exception(self):
        """Test recording exception on span."""
        exc = ValueError("Test error")

        # Should not raise even without active span
        record_exception(exc, {"extra": "info"})

    def test_set_span_status(self):
        """Test setting span status."""
        # Should not raise even without active span
        set_span_status(StatusCode.OK)
        set_span_status(StatusCode.ERROR, "Something went wrong")


class TestTracedDecorator:
    """Tests for @traced decorator."""

    def test_traced_function(self):
        """Test traced decorator on sync function."""

        @traced("test-operation")
        def my_function(x: int) -> int:
            return x * 2

        result = my_function(5)
        assert result == 10

    def test_traced_function_with_error(self):
        """Test traced decorator handles errors."""

        @traced("error-operation")
        def failing_function():
            raise ValueError("Test error")

        with pytest.raises(ValueError) as exc_info:
            failing_function()

        assert "Test error" in str(exc_info.value)

    def test_traced_function_with_attributes(self):
        """Test traced decorator with attributes."""

        @traced("attributed-op", attributes={"key": "value"})
        def attributed_function():
            return "result"

        result = attributed_function()
        assert result == "result"

    def test_traced_uses_function_name(self):
        """Test traced decorator uses function name if name not provided."""

        @traced()
        def auto_named_function():
            return 42

        result = auto_named_function()
        assert result == 42


class TestTracedAsyncDecorator:
    """Tests for @traced_async decorator."""

    @pytest.mark.asyncio
    async def test_traced_async_function(self):
        """Test traced_async decorator on async function."""

        @traced_async("async-operation")
        async def my_async_function(x: int) -> int:
            return x * 2

        result = await my_async_function(5)
        assert result == 10

    @pytest.mark.asyncio
    async def test_traced_async_function_with_error(self):
        """Test traced_async decorator handles errors."""

        @traced_async("async-error")
        async def failing_async_function():
            raise ValueError("Async error")

        with pytest.raises(ValueError) as exc_info:
            await failing_async_function()

        assert "Async error" in str(exc_info.value)

    @pytest.mark.asyncio
    async def test_traced_async_with_attributes(self):
        """Test traced_async decorator with attributes."""

        @traced_async("async-attributed", kind=SpanKind.CLIENT, attributes={"api": "test"})
        async def attributed_async():
            return "async result"

        result = await attributed_async()
        assert result == "async result"


class TestTaskSpanContext:
    """Tests for TaskSpanContext."""

    def test_context_manager_basic(self):
        """Test basic context manager usage."""
        with TaskSpanContext(
            task_id="task-123",
            task_name="test-task",
            agent_name="test-agent",
        ) as ctx:
            assert ctx.task_id == "task-123"
            assert ctx.task_name == "test-task"

    def test_context_manager_with_trace_ids(self):
        """Test context manager with trace propagation."""
        with TaskSpanContext(
            task_id="task-456",
            task_name="traced-task",
            agent_name="test-agent",
            trace_id="00000000000000000000000000000001",
            span_id="0000000000000002",
        ) as ctx:
            assert ctx.trace_id is not None

    def test_add_attribute(self):
        """Test adding attribute to span context."""
        with TaskSpanContext(
            task_id="task-789",
            task_name="attr-task",
            agent_name="test-agent",
        ) as ctx:
            ctx.add_attribute("custom.key", "custom.value")
            ctx.add_attribute("custom.count", 42)

    def test_add_event(self):
        """Test adding event to span context."""
        with TaskSpanContext(
            task_id="task-event",
            task_name="event-task",
            agent_name="test-agent",
        ) as ctx:
            ctx.add_event("processing_started", {"step": 1})
            ctx.add_event("processing_completed")

    def test_record_metrics(self):
        """Test recording metrics."""
        with TaskSpanContext(
            task_id="task-metrics",
            task_name="metrics-task",
            agent_name="test-agent",
        ) as ctx:
            ctx.record_metrics(tokens=100, cost=0.01, duration_ms=1500)

    def test_get_trace_context(self):
        """Test getting trace context for propagation."""
        with TaskSpanContext(
            task_id="task-ctx",
            task_name="ctx-task",
            agent_name="test-agent",
        ) as ctx:
            trace_ctx = ctx.get_trace_context()
            assert isinstance(trace_ctx, dict)

    def test_context_manager_with_exception(self):
        """Test context manager handles exceptions."""
        with pytest.raises(ValueError):
            with TaskSpanContext(
                task_id="task-error",
                task_name="error-task",
                agent_name="test-agent",
            ):
                raise ValueError("Test exception")


class TestShutdownTracing:
    """Tests for shutdown_tracing function."""

    def test_shutdown_tracing(self):
        """Test tracing shutdown."""
        with patch("apex_agents.tracing.trace") as mock_trace:
            mock_provider = MagicMock()
            mock_trace.get_tracer_provider.return_value = mock_provider

            shutdown_tracing()

            mock_provider.shutdown.assert_called_once()
