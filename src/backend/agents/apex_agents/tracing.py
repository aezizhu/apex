"""
OpenTelemetry tracing setup and utilities.

Provides distributed tracing capabilities with context propagation
for the Apex agent runtime.
"""

from __future__ import annotations

import functools
import logging
from contextvars import ContextVar
from typing import Any, Callable, ParamSpec, TypeVar

from opentelemetry import trace
from opentelemetry.context import Context, attach, detach, get_current
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
from opentelemetry.propagate import extract, inject
from opentelemetry.sdk.resources import Resource
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import (
    BatchSpanProcessor,
    ConsoleSpanExporter,
    SimpleSpanProcessor,
)
from opentelemetry.sdk.trace.sampling import TraceIdRatioBased
from opentelemetry.semconv.resource import ResourceAttributes
from opentelemetry.trace import (
    Span,
    SpanKind,
    Status,
    StatusCode,
    Tracer,
    get_current_span,
    set_span_in_context,
)
from opentelemetry.trace.propagation.tracecontext import TraceContextTextMapPropagator

from apex_agents.config import Settings, TracingConfig

logger = logging.getLogger(__name__)

# Context variable for the current trace context carrier
_trace_context_carrier: ContextVar[dict[str, str]] = ContextVar(
    "trace_context_carrier", default={}
)

# Global tracer instance
_tracer: Tracer | None = None

# Type variables for decorator
P = ParamSpec("P")
R = TypeVar("R")


def init_tracing(settings: Settings | None = None) -> Tracer:
    """
    Initialize OpenTelemetry tracing.

    This should be called once at application startup to configure
    the tracing provider and exporters.

    Args:
        settings: Application settings. If None, loads from environment.

    Returns:
        Configured Tracer instance.
    """
    global _tracer

    if settings is None:
        from apex_agents.config import get_settings

        settings = get_settings()

    config = settings.tracing

    if not config.enabled:
        logger.info("Tracing is disabled")
        _tracer = trace.get_tracer(__name__)
        return _tracer

    # Create resource with service information
    resource = Resource.create(
        {
            ResourceAttributes.SERVICE_NAME: config.service_name,
            ResourceAttributes.SERVICE_VERSION: config.service_version,
            ResourceAttributes.DEPLOYMENT_ENVIRONMENT: config.environment,
        }
    )

    # Configure sampler
    sampler = TraceIdRatioBased(config.sample_rate)

    # Create tracer provider
    provider = TracerProvider(
        resource=resource,
        sampler=sampler,
    )

    # Add exporters
    if config.otlp_endpoint:
        # OTLP exporter for production use
        otlp_exporter = OTLPSpanExporter(endpoint=config.otlp_endpoint, insecure=True)
        provider.add_span_processor(BatchSpanProcessor(otlp_exporter))
        logger.info("OTLP tracing exporter configured", extra={"endpoint": config.otlp_endpoint})

    if config.console_export or settings.is_development():
        # Console exporter for development/debugging
        console_exporter = ConsoleSpanExporter()
        provider.add_span_processor(SimpleSpanProcessor(console_exporter))
        logger.info("Console tracing exporter configured")

    # Set as global provider
    trace.set_tracer_provider(provider)

    _tracer = trace.get_tracer(config.service_name, config.service_version)
    logger.info(
        "Tracing initialized",
        extra={
            "service_name": config.service_name,
            "sample_rate": config.sample_rate,
        },
    )

    return _tracer


def get_tracer(name: str | None = None) -> Tracer:
    """
    Get a tracer instance.

    Args:
        name: Optional tracer name. If None, returns the default tracer.

    Returns:
        Tracer instance.
    """
    global _tracer

    if name:
        return trace.get_tracer(name)

    if _tracer is None:
        _tracer = trace.get_tracer(__name__)

    return _tracer


def shutdown_tracing() -> None:
    """
    Shutdown the tracing provider.

    This should be called during application shutdown to ensure
    all spans are exported.
    """
    provider = trace.get_tracer_provider()
    if hasattr(provider, "shutdown"):
        provider.shutdown()
        logger.info("Tracing provider shutdown complete")


# =============================================================================
# Context Propagation
# =============================================================================


def extract_context(carrier: dict[str, str]) -> Context:
    """
    Extract trace context from a carrier (e.g., HTTP headers).

    Args:
        carrier: Dictionary containing trace context headers.

    Returns:
        OpenTelemetry Context with extracted trace information.
    """
    propagator = TraceContextTextMapPropagator()
    return propagator.extract(carrier=carrier)


def inject_context(carrier: dict[str, str] | None = None) -> dict[str, str]:
    """
    Inject current trace context into a carrier.

    Args:
        carrier: Optional dictionary to inject into. Creates new if None.

    Returns:
        Dictionary with injected trace context headers.
    """
    if carrier is None:
        carrier = {}

    propagator = TraceContextTextMapPropagator()
    propagator.inject(carrier=carrier)
    return carrier


def get_trace_context() -> dict[str, str]:
    """
    Get the current trace context as a dictionary.

    Returns:
        Dictionary with traceparent and tracestate headers.
    """
    return inject_context({})


def set_trace_context(trace_id: str | None = None, span_id: str | None = None) -> Context | None:
    """
    Set trace context from task trace IDs.

    Args:
        trace_id: W3C trace ID string.
        span_id: W3C span ID string.

    Returns:
        OpenTelemetry Context if context was set, None otherwise.
    """
    if not trace_id:
        return None

    # Build traceparent header
    # Format: {version}-{trace_id}-{span_id}-{trace_flags}
    span_id_value = span_id or "0000000000000000"
    traceparent = f"00-{trace_id}-{span_id_value}-01"

    carrier = {"traceparent": traceparent}
    context = extract_context(carrier)

    # Store in context var for later use
    _trace_context_carrier.set(carrier)

    return context


def with_context(context: Context) -> object:
    """
    Attach a context and return a token for detachment.

    Args:
        context: OpenTelemetry Context to attach.

    Returns:
        Token to use with detach().
    """
    return attach(context)


def without_context(token: object) -> None:
    """
    Detach a previously attached context.

    Args:
        token: Token returned from with_context().
    """
    detach(token)  # type: ignore[arg-type]


# =============================================================================
# Span Utilities
# =============================================================================


def create_span(
    name: str,
    kind: SpanKind = SpanKind.INTERNAL,
    attributes: dict[str, Any] | None = None,
    context: Context | None = None,
) -> Span:
    """
    Create a new span.

    Args:
        name: Span name.
        kind: Span kind (INTERNAL, SERVER, CLIENT, PRODUCER, CONSUMER).
        attributes: Optional span attributes.
        context: Optional parent context.

    Returns:
        New Span instance.
    """
    tracer = get_tracer()
    return tracer.start_span(
        name=name,
        kind=kind,
        attributes=attributes,
        context=context,
    )


def current_span() -> Span:
    """
    Get the current active span.

    Returns:
        Current Span or NoopSpan if none active.
    """
    return get_current_span()


def add_span_attributes(attributes: dict[str, Any]) -> None:
    """
    Add attributes to the current span.

    Args:
        attributes: Dictionary of attributes to add.
    """
    span = current_span()
    for key, value in attributes.items():
        span.set_attribute(key, value)


def record_exception(exception: Exception, attributes: dict[str, Any] | None = None) -> None:
    """
    Record an exception on the current span.

    Args:
        exception: Exception to record.
        attributes: Optional additional attributes.
    """
    span = current_span()
    span.record_exception(exception, attributes=attributes)
    span.set_status(Status(StatusCode.ERROR, str(exception)))


def set_span_status(code: StatusCode, description: str | None = None) -> None:
    """
    Set the status of the current span.

    Args:
        code: Status code (OK, ERROR, UNSET).
        description: Optional status description.
    """
    span = current_span()
    span.set_status(Status(code, description))


# =============================================================================
# Decorators
# =============================================================================


def traced(
    name: str | None = None,
    kind: SpanKind = SpanKind.INTERNAL,
    attributes: dict[str, Any] | None = None,
) -> Callable[[Callable[P, R]], Callable[P, R]]:
    """
    Decorator to trace a function with a span.

    Args:
        name: Span name. Defaults to function name.
        kind: Span kind.
        attributes: Static attributes to add to the span.

    Returns:
        Decorated function.

    Example:
        @traced("process_task", attributes={"task.type": "data_processing"})
        def process_task(task_id: str) -> Result:
            ...
    """

    def decorator(func: Callable[P, R]) -> Callable[P, R]:
        span_name = name or func.__name__

        @functools.wraps(func)
        def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
            tracer = get_tracer()
            with tracer.start_as_current_span(
                span_name,
                kind=kind,
                attributes=attributes,
            ) as span:
                try:
                    result = func(*args, **kwargs)
                    span.set_status(Status(StatusCode.OK))
                    return result
                except Exception as e:
                    span.record_exception(e)
                    span.set_status(Status(StatusCode.ERROR, str(e)))
                    raise

        return wrapper

    return decorator


def traced_async(
    name: str | None = None,
    kind: SpanKind = SpanKind.INTERNAL,
    attributes: dict[str, Any] | None = None,
) -> Callable[..., Any]:
    """
    Decorator to trace an async function with a span.

    Args:
        name: Span name. Defaults to function name.
        kind: Span kind.
        attributes: Static attributes to add to the span.

    Returns:
        Decorated async function.

    Example:
        @traced_async("fetch_data", kind=SpanKind.CLIENT)
        async def fetch_data(url: str) -> dict:
            ...
    """

    def decorator(func: Callable[P, R]) -> Callable[P, R]:
        span_name = name or func.__name__

        @functools.wraps(func)
        async def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
            tracer = get_tracer()
            with tracer.start_as_current_span(
                span_name,
                kind=kind,
                attributes=attributes,
            ) as span:
                try:
                    result = await func(*args, **kwargs)  # type: ignore[misc]
                    span.set_status(Status(StatusCode.OK))
                    return result  # type: ignore[no-any-return]
                except Exception as e:
                    span.record_exception(e)
                    span.set_status(Status(StatusCode.ERROR, str(e)))
                    raise

        return wrapper  # type: ignore[return-value]

    return decorator


# =============================================================================
# Task-Specific Tracing
# =============================================================================


class TaskSpanContext:
    """
    Context manager for task execution tracing.

    Provides a convenient way to trace task execution with proper
    context propagation and error handling.
    """

    def __init__(
        self,
        task_id: str,
        task_name: str,
        agent_name: str,
        trace_id: str | None = None,
        span_id: str | None = None,
    ):
        """
        Initialize task span context.

        Args:
            task_id: Unique task identifier.
            task_name: Human-readable task name.
            agent_name: Name of the executing agent.
            trace_id: Optional parent trace ID.
            span_id: Optional parent span ID.
        """
        self.task_id = task_id
        self.task_name = task_name
        self.agent_name = agent_name
        self.trace_id = trace_id
        self.span_id = span_id
        self._span: Span | None = None
        self._token: object | None = None

    def __enter__(self) -> "TaskSpanContext":
        """Start the task span."""
        # Set parent context if available
        if self.trace_id:
            parent_context = set_trace_context(self.trace_id, self.span_id)
            if parent_context:
                self._token = attach(parent_context)

        # Create task span
        tracer = get_tracer()
        self._span = tracer.start_span(
            f"task_{self.task_name}",
            kind=SpanKind.CONSUMER,
            attributes={
                "task.id": self.task_id,
                "task.name": self.task_name,
                "agent.name": self.agent_name,
            },
        )

        # Make it the current span
        context = set_span_in_context(self._span)
        self._context_token = attach(context)

        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        """End the task span."""
        if self._span:
            if exc_val:
                self._span.record_exception(exc_val)
                self._span.set_status(Status(StatusCode.ERROR, str(exc_val)))
            else:
                self._span.set_status(Status(StatusCode.OK))
            self._span.end()

        if hasattr(self, "_context_token"):
            detach(self._context_token)  # type: ignore[arg-type]

        if self._token:
            detach(self._token)  # type: ignore[arg-type]

    def add_attribute(self, key: str, value: Any) -> None:
        """Add an attribute to the span."""
        if self._span:
            self._span.set_attribute(key, value)

    def add_event(self, name: str, attributes: dict[str, Any] | None = None) -> None:
        """Add an event to the span."""
        if self._span:
            self._span.add_event(name, attributes=attributes)

    def record_metrics(self, tokens: int, cost: float, duration_ms: int) -> None:
        """Record execution metrics as span attributes."""
        if self._span:
            self._span.set_attributes(
                {
                    "task.tokens_used": tokens,
                    "task.cost_dollars": cost,
                    "task.duration_ms": duration_ms,
                }
            )

    def get_trace_context(self) -> dict[str, str]:
        """Get the current trace context for propagation."""
        return inject_context({})
