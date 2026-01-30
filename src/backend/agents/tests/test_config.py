"""Tests for configuration management."""

import os
from unittest.mock import patch

import pytest
from pydantic import ValidationError

from apex_agents.config import (
    BackendConfig,
    DatabaseConfig,
    Environment,
    LLMConfig,
    LogLevel,
    RedisConfig,
    Settings,
    TracingConfig,
    WorkerConfig,
    get_settings,
    reload_settings,
)


class TestEnvironment:
    """Tests for Environment enum."""

    def test_environment_values(self):
        """Test environment enum values."""
        assert Environment.DEVELOPMENT.value == "development"
        assert Environment.STAGING.value == "staging"
        assert Environment.PRODUCTION.value == "production"


class TestLogLevel:
    """Tests for LogLevel enum."""

    def test_log_level_values(self):
        """Test log level enum values."""
        assert LogLevel.DEBUG.value == "DEBUG"
        assert LogLevel.INFO.value == "INFO"
        assert LogLevel.WARNING.value == "WARNING"
        assert LogLevel.ERROR.value == "ERROR"


class TestBackendConfig:
    """Tests for BackendConfig."""

    def test_default_values(self):
        """Test default configuration values."""
        config = BackendConfig()

        assert config.host == "localhost"
        assert config.http_port == 8080
        assert config.grpc_port == 50051
        assert config.use_grpc is False
        assert config.timeout_seconds == 30.0

    def test_http_base_url(self):
        """Test HTTP base URL generation."""
        config = BackendConfig(host="api.example.com", http_port=9000)

        assert config.http_base_url == "http://api.example.com:9000"

    def test_grpc_address(self):
        """Test gRPC address generation."""
        config = BackendConfig(host="grpc.example.com", grpc_port=50052)

        assert config.grpc_address == "grpc.example.com:50052"

    @patch.dict(os.environ, {"APEX_BACKEND_HOST": "custom.host", "APEX_BACKEND_HTTP_PORT": "3000"})
    def test_env_override(self):
        """Test environment variable override."""
        config = BackendConfig()

        assert config.host == "custom.host"
        assert config.http_port == 3000


class TestRedisConfig:
    """Tests for RedisConfig."""

    def test_default_values(self):
        """Test default configuration values."""
        config = RedisConfig()

        assert config.url == "redis://localhost:6379"
        assert config.pool_size == 10
        assert config.task_queue_key == "apex:tasks:queue"
        assert config.result_queue_key == "apex:tasks:results"

    @patch.dict(os.environ, {"APEX_REDIS_URL": "redis://custom:6380"})
    def test_env_override(self):
        """Test environment variable override."""
        config = RedisConfig()

        assert config.url == "redis://custom:6380"


class TestDatabaseConfig:
    """Tests for DatabaseConfig."""

    def test_default_values(self):
        """Test default configuration values."""
        config = DatabaseConfig()

        assert "postgresql://" in config.url
        assert config.pool_size == 10
        assert config.pool_overflow == 5
        assert config.echo is False


class TestLLMConfig:
    """Tests for LLMConfig."""

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test-key"})
    def test_with_openai_key(self):
        """Test configuration with OpenAI key."""
        config = LLMConfig()

        assert config.openai_api_key == "sk-test-key"
        assert config.anthropic_api_key is None
        assert config.default_model == "gpt-4o-mini"

    @patch.dict(os.environ, {"APEX_LLM_ANTHROPIC_API_KEY": "sk-ant-test"})
    def test_with_anthropic_key(self):
        """Test configuration with Anthropic key."""
        config = LLMConfig()

        # Note: openai_api_key may be set from environment (conftest autouse fixture)
        assert config.anthropic_api_key == "sk-ant-test"

    @patch.dict(os.environ, {}, clear=True)
    def test_missing_api_keys(self):
        """Test validation error when no API keys provided."""
        # Clear any existing env vars that might have keys
        for key in list(os.environ.keys()):
            if "APEX_LLM" in key:
                del os.environ[key]

        with pytest.raises(ValidationError) as exc_info:
            LLMConfig()

        assert "API key must be configured" in str(exc_info.value)


class TestTracingConfig:
    """Tests for TracingConfig."""

    @patch.dict(os.environ, {"APEX_TRACING_ENABLED": "true"}, clear=False)
    def test_default_values(self):
        """Test default configuration values."""
        config = TracingConfig()

        assert config.enabled is True
        assert config.otlp_endpoint is None
        assert config.service_name == "apex-agents"
        assert config.sample_rate == 1.0
        assert config.console_export is False

    def test_sample_rate_validation(self):
        """Test sample rate validation."""
        # Valid values
        config = TracingConfig(sample_rate=0.5)
        assert config.sample_rate == 0.5

        config = TracingConfig(sample_rate=0.0)
        assert config.sample_rate == 0.0

        config = TracingConfig(sample_rate=1.0)
        assert config.sample_rate == 1.0

        # Invalid values
        with pytest.raises(ValidationError):
            TracingConfig(sample_rate=-0.1)

        with pytest.raises(ValidationError):
            TracingConfig(sample_rate=1.5)


class TestWorkerConfig:
    """Tests for WorkerConfig."""

    def test_default_values(self):
        """Test default configuration values."""
        config = WorkerConfig()

        assert config.worker_id is None
        assert config.num_agents == 5
        assert config.poll_interval_seconds == 1.0
        assert config.heartbeat_interval_seconds == 10.0
        assert config.max_task_duration_seconds == 300
        assert config.graceful_shutdown_timeout_seconds == 30

    def test_num_agents_validation(self):
        """Test num_agents validation."""
        # Valid values
        config = WorkerConfig(num_agents=1)
        assert config.num_agents == 1

        config = WorkerConfig(num_agents=100)
        assert config.num_agents == 100

        # Invalid values
        with pytest.raises(ValidationError):
            WorkerConfig(num_agents=0)

        with pytest.raises(ValidationError):
            WorkerConfig(num_agents=101)


class TestSettings:
    """Tests for main Settings class."""

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test", "APEX_LOG_JSON": "true"})
    def test_default_values(self):
        """Test default settings values."""
        settings = Settings()

        assert settings.environment == Environment.DEVELOPMENT
        assert settings.debug is False
        assert settings.log_level == LogLevel.INFO
        assert settings.log_json is True

    @patch.dict(
        os.environ,
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_ENVIRONMENT": "production",
            "APEX_DEBUG": "true",
            "APEX_LOG_LEVEL": "ERROR",
        },
    )
    def test_env_override(self):
        """Test environment variable overrides."""
        settings = Settings()

        assert settings.environment == Environment.PRODUCTION
        assert settings.debug is True
        assert settings.log_level == LogLevel.ERROR

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test"})
    def test_is_production(self):
        """Test is_production helper."""
        settings = Settings(environment=Environment.PRODUCTION)
        assert settings.is_production() is True

        settings = Settings(environment=Environment.DEVELOPMENT)
        assert settings.is_production() is False

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test"})
    def test_is_development(self):
        """Test is_development helper."""
        settings = Settings(environment=Environment.DEVELOPMENT)
        assert settings.is_development() is True

        settings = Settings(environment=Environment.PRODUCTION)
        assert settings.is_development() is False

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test", "APEX_ENVIRONMENT": "PRODUCTION"})
    def test_case_insensitive_environment(self):
        """Test that environment parsing is case insensitive."""
        settings = Settings()
        assert settings.environment == Environment.PRODUCTION

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test", "APEX_LOG_LEVEL": "warning"})
    def test_case_insensitive_log_level(self):
        """Test that log level parsing is case insensitive."""
        settings = Settings()
        assert settings.log_level == LogLevel.WARNING


class TestGetSettings:
    """Tests for get_settings function."""

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test"})
    def test_caching(self):
        """Test that settings are cached."""
        # Clear cache first
        reload_settings()

        settings1 = get_settings()
        settings2 = get_settings()

        assert settings1 is settings2

    @patch.dict(os.environ, {"APEX_LLM_OPENAI_API_KEY": "sk-test"})
    def test_reload_settings(self):
        """Test that reload_settings clears cache."""
        settings1 = get_settings()
        settings2 = reload_settings()

        # Should be different instances after reload
        assert settings1 is not settings2
