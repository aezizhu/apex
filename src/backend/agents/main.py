#!/usr/bin/env python3
"""
Apex Agent Runtime - Main Entry Point

Starts the worker pool that pulls tasks from the orchestrator
and executes them using AI agents.

Usage:
    # Run with default settings (1 worker)
    python main.py

    # Run with multiple workers
    python main.py --workers 4

    # Run with custom config
    APEX_WORKER_NUM_AGENTS=10 python main.py

Environment Variables:
    APEX_ENVIRONMENT         - deployment environment (development/staging/production)
    APEX_DEBUG               - enable debug mode
    APEX_LOG_LEVEL           - log level (DEBUG/INFO/WARNING/ERROR)
    APEX_LOG_JSON            - output logs as JSON

    APEX_BACKEND_HOST        - Rust backend host
    APEX_BACKEND_HTTP_PORT   - REST API port
    APEX_BACKEND_GRPC_PORT   - gRPC port

    APEX_REDIS_URL           - Redis connection URL
    APEX_DATABASE_URL        - PostgreSQL connection URL

    APEX_LLM_OPENAI_API_KEY  - OpenAI API key
    APEX_LLM_ANTHROPIC_API_KEY - Anthropic API key
    APEX_LLM_DEFAULT_MODEL   - Default LLM model

    APEX_TRACING_ENABLED     - Enable OpenTelemetry tracing
    APEX_TRACING_OTLP_ENDPOINT - OTLP exporter endpoint

    APEX_WORKER_NUM_AGENTS   - Number of concurrent agents per worker
    APEX_WORKER_POLL_INTERVAL_SECONDS - Task queue poll interval
    APEX_WORKER_HEARTBEAT_INTERVAL_SECONDS - Heartbeat interval
"""

from __future__ import annotations

import argparse
import asyncio
import logging
import sys
from typing import NoReturn

import structlog

from apex_agents.config import Settings, get_settings
from apex_agents.worker import run_worker, run_worker_pool


def setup_logging(settings: Settings) -> None:
    """
    Configure structured logging.

    Args:
        settings: Application settings.
    """
    # Configure structlog processors
    processors: list[structlog.typing.Processor] = [
        structlog.contextvars.merge_contextvars,
        structlog.processors.add_log_level,
        structlog.processors.StackInfoRenderer(),
        structlog.processors.TimeStamper(fmt="iso"),
    ]

    if settings.log_json:
        # JSON output for production
        processors.extend([
            structlog.processors.format_exc_info,
            structlog.processors.JSONRenderer(),
        ])
    else:
        # Pretty output for development
        processors.extend([
            structlog.dev.ConsoleRenderer(colors=True),
        ])

    structlog.configure(
        processors=processors,
        wrapper_class=structlog.make_filtering_bound_logger(
            getattr(logging, settings.log_level.value)
        ),
        context_class=dict,
        logger_factory=structlog.PrintLoggerFactory(),
        cache_logger_on_first_use=True,
    )

    # Also configure standard logging
    logging.basicConfig(
        format="%(message)s",
        stream=sys.stdout,
        level=getattr(logging, settings.log_level.value),
    )


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Apex Agent Runtime - AI Agent Worker",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )

    parser.add_argument(
        "--workers",
        "-w",
        type=int,
        default=1,
        help="Number of worker processes (default: 1)",
    )

    parser.add_argument(
        "--agents",
        "-a",
        type=int,
        default=None,
        help="Number of concurrent agents per worker (overrides APEX_WORKER_NUM_AGENTS)",
    )

    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug mode",
    )

    parser.add_argument(
        "--backend-url",
        type=str,
        default=None,
        help="Backend URL (overrides APEX_BACKEND_HOST/PORT)",
    )

    parser.add_argument(
        "--redis-url",
        type=str,
        default=None,
        help="Redis URL (overrides APEX_REDIS_URL)",
    )

    parser.add_argument(
        "--version",
        action="version",
        version="%(prog)s 0.1.0",
    )

    return parser.parse_args()


def apply_cli_overrides(settings: Settings, args: argparse.Namespace) -> Settings:
    """
    Apply command line argument overrides to settings.

    Args:
        settings: Base settings.
        args: Parsed command line arguments.

    Returns:
        Settings with overrides applied.
    """
    # Note: Pydantic settings are immutable by default, so we create new instances
    # For simplicity, we modify the settings in-place here since they're mutable
    if args.agents is not None:
        settings.worker.num_agents = args.agents

    if args.debug:
        settings.debug = True
        settings.log_level = "DEBUG"

    if args.backend_url:
        # Parse URL and update settings
        from urllib.parse import urlparse

        parsed = urlparse(args.backend_url)
        if parsed.hostname:
            settings.backend.host = parsed.hostname
        if parsed.port:
            settings.backend.http_port = parsed.port

    if args.redis_url:
        settings.redis.url = args.redis_url

    return settings


def print_banner(settings: Settings, num_workers: int) -> None:
    """Print startup banner."""
    banner = """
    ╔═══════════════════════════════════════════════════════════════╗
    ║                     APEX AGENT RUNTIME                        ║
    ║              World's No. 1 Agent Swarm Orchestrator           ║
    ╚═══════════════════════════════════════════════════════════════╝
    """
    print(banner)
    print(f"    Environment:    {settings.environment.value}")
    print(f"    Workers:        {num_workers}")
    print(f"    Agents/Worker:  {settings.worker.num_agents}")
    print(f"    Backend:        {settings.backend.http_base_url}")
    print(f"    Redis:          {settings.redis.url}")
    print(f"    Log Level:      {settings.log_level.value}")
    print()


async def main_async(args: argparse.Namespace) -> int:
    """
    Async main function.

    Args:
        args: Parsed command line arguments.

    Returns:
        Exit code.
    """
    # Load settings
    settings = get_settings()
    settings = apply_cli_overrides(settings, args)

    # Setup logging
    setup_logging(settings)

    logger = structlog.get_logger()
    logger.info(
        "Starting Apex Agent Runtime",
        environment=settings.environment.value,
        workers=args.workers,
        agents_per_worker=settings.worker.num_agents,
    )

    # Print banner in non-JSON mode
    if not settings.log_json:
        print_banner(settings, args.workers)

    try:
        if args.workers == 1:
            # Single worker mode
            await run_worker(settings)
        else:
            # Multi-worker mode
            await run_worker_pool(
                num_workers=args.workers,
                settings=settings,
            )

        return 0

    except KeyboardInterrupt:
        logger.info("Shutdown requested by user")
        return 0

    except Exception as e:
        logger.exception("Fatal error", error=str(e))
        return 1


def main() -> NoReturn:
    """Main entry point."""
    args = parse_args()

    try:
        exit_code = asyncio.run(main_async(args))
    except KeyboardInterrupt:
        exit_code = 0

    sys.exit(exit_code)


if __name__ == "__main__":
    main()
