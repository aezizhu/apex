"""
Configuration management using Pydantic Settings.

Loads configuration from environment variables with validation.
"""

from __future__ import annotations

from enum import Enum
from functools import lru_cache
from typing import Any

from pydantic import Field, field_validator, model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

from apex_agents.routing import DEFAULT_CASCADE, RoutingConfig


class Environment(str, Enum):
    """Deployment environment."""

    DEVELOPMENT = "development"
    STAGING = "staging"
    PRODUCTION = "production"


class LogLevel(str, Enum):
    """Log level options."""

    DEBUG = "DEBUG"
    INFO = "INFO"
    WARNING = "WARNING"
    ERROR = "ERROR"
    CRITICAL = "CRITICAL"


class BackendConfig(BaseSettings):
    """Configuration for connecting to the Rust backend."""

    model_config = SettingsConfigDict(env_prefix="APEX_BACKEND_")

    host: str = Field(default="localhost", description="Rust backend host")
    http_port: int = Field(default=8080, description="REST API port")
    grpc_port: int = Field(default=50051, description="gRPC port")
    use_grpc: bool = Field(default=False, description="Use gRPC instead of REST")
    timeout_seconds: float = Field(default=30.0, description="Request timeout")
    max_retries: int = Field(default=3, description="Maximum retry attempts")

    @property
    def http_base_url(self) -> str:
        """Get the HTTP base URL for the backend."""
        return f"http://{self.host}:{self.http_port}"

    @property
    def grpc_address(self) -> str:
        """Get the gRPC address for the backend."""
        return f"{self.host}:{self.grpc_port}"


class RedisConfig(BaseSettings):
    """Configuration for Redis connection."""

    model_config = SettingsConfigDict(env_prefix="APEX_REDIS_")

    url: str = Field(default="redis://localhost:6379", description="Redis connection URL")
    pool_size: int = Field(default=10, description="Connection pool size")
    task_queue_key: str = Field(default="apex:tasks:queue", description="Task queue key")
    result_queue_key: str = Field(default="apex:tasks:results", description="Result queue key")
    heartbeat_key_prefix: str = Field(
        default="apex:workers:heartbeat:", description="Worker heartbeat key prefix"
    )
    heartbeat_ttl_seconds: int = Field(default=30, description="Heartbeat TTL in seconds")


class DatabaseConfig(BaseSettings):
    """Configuration for PostgreSQL database."""

    model_config = SettingsConfigDict(env_prefix="APEX_DATABASE_")

    url: str = Field(
        default="postgresql://apex:apex@localhost:5432/apex",
        description="PostgreSQL connection URL",
    )
    pool_size: int = Field(default=10, description="Connection pool size")
    pool_overflow: int = Field(default=5, description="Pool overflow limit")
    echo: bool = Field(default=False, description="Echo SQL statements")


class LLMConfig(BaseSettings):
    """Configuration for LLM providers."""

    model_config = SettingsConfigDict(env_prefix="APEX_LLM_")

    openai_api_key: str | None = Field(default=None, description="OpenAI API key")
    anthropic_api_key: str | None = Field(default=None, description="Anthropic API key")
    default_model: str = Field(default="gpt-4o-mini", description="Default model to use")
    timeout_seconds: float = Field(default=120.0, description="LLM request timeout")
    max_retries: int = Field(default=3, description="Maximum retry attempts")

    @model_validator(mode="after")
    def validate_api_keys(self) -> "LLMConfig":
        """Ensure at least one API key is configured."""
        if not self.openai_api_key and not self.anthropic_api_key:
            raise ValueError(
                "At least one LLM API key must be configured "
                "(APEX_LLM_OPENAI_API_KEY or APEX_LLM_ANTHROPIC_API_KEY)"
            )
        return self


class TracingConfig(BaseSettings):
    """Configuration for OpenTelemetry tracing."""

    model_config = SettingsConfigDict(env_prefix="APEX_TRACING_")

    enabled: bool = Field(default=True, description="Enable tracing")
    otlp_endpoint: str | None = Field(
        default=None, description="OTLP exporter endpoint (e.g., http://localhost:4317)"
    )
    service_name: str = Field(default="apex-agents", description="Service name for traces")
    service_version: str = Field(default="0.1.0", description="Service version")
    environment: str = Field(default="development", description="Deployment environment")
    sample_rate: float = Field(default=1.0, ge=0.0, le=1.0, description="Trace sample rate")
    console_export: bool = Field(
        default=False, description="Export traces to console (for development)"
    )


class WorkerConfig(BaseSettings):
    """Configuration for the worker process."""

    model_config = SettingsConfigDict(env_prefix="APEX_WORKER_")

    worker_id: str | None = Field(
        default=None, description="Unique worker ID (auto-generated if not set)"
    )
    num_agents: int = Field(default=5, ge=1, le=100, description="Number of concurrent agents")
    poll_interval_seconds: float = Field(
        default=1.0, ge=0.1, le=60.0, description="Task queue poll interval"
    )
    heartbeat_interval_seconds: float = Field(
        default=10.0, ge=1.0, le=60.0, description="Heartbeat interval"
    )
    max_task_duration_seconds: int = Field(
        default=300, ge=10, le=3600, description="Maximum task execution time"
    )
    graceful_shutdown_timeout_seconds: int = Field(
        default=30, ge=5, le=300, description="Graceful shutdown timeout"
    )


class Settings(BaseSettings):
    """Main application settings combining all configuration sections."""

    model_config = SettingsConfigDict(
        env_prefix="APEX_",
        env_nested_delimiter="__",
        case_sensitive=False,
    )

    # General settings
    environment: Environment = Field(
        default=Environment.DEVELOPMENT, description="Deployment environment"
    )
    debug: bool = Field(default=False, description="Enable debug mode")
    log_level: LogLevel = Field(default=LogLevel.INFO, description="Log level")
    log_json: bool = Field(default=True, description="Output logs as JSON")

    # Sub-configurations
    backend: BackendConfig = Field(default_factory=BackendConfig)
    redis: RedisConfig = Field(default_factory=RedisConfig)
    database: DatabaseConfig = Field(default_factory=DatabaseConfig)
    llm: LLMConfig = Field(default_factory=lambda: LLMConfig(_env_prefix="APEX_LLM_"))  # type: ignore[call-arg]
    tracing: TracingConfig = Field(default_factory=TracingConfig)
    worker: WorkerConfig = Field(default_factory=WorkerConfig)
    routing: RoutingConfig = Field(default_factory=RoutingConfig)

    @field_validator("environment", mode="before")
    @classmethod
    def validate_environment(cls, v: Any) -> Environment:
        """Parse environment from string."""
        if isinstance(v, str):
            return Environment(v.lower())
        return v  # type: ignore[no-any-return]

    @field_validator("log_level", mode="before")
    @classmethod
    def validate_log_level(cls, v: Any) -> LogLevel:
        """Parse log level from string."""
        if isinstance(v, str):
            return LogLevel(v.upper())
        return v  # type: ignore[no-any-return]

    def is_production(self) -> bool:
        """Check if running in production."""
        return self.environment == Environment.PRODUCTION

    def is_development(self) -> bool:
        """Check if running in development."""
        return self.environment == Environment.DEVELOPMENT


@lru_cache
def get_settings() -> Settings:
    """
    Get cached application settings.

    Settings are loaded once and cached for the lifetime of the process.
    Uses environment variables with the APEX_ prefix.

    Returns:
        Settings instance with all configuration loaded.
    """
    return Settings()


def reload_settings() -> Settings:
    """
    Force reload settings (clears cache).

    Use this when settings need to be refreshed during runtime.

    Returns:
        Fresh Settings instance.
    """
    get_settings.cache_clear()
    return get_settings()
