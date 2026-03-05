"""Tests for configuration validation edge cases and boundary conditions."""

import os
from unittest.mock import patch

import pytest
from pydantic import ValidationError

from apex_agents.config import (
    BackendConfig,
    DatabaseConfig,
    Environment,
    LLMConfig,
    RedisConfig,
    Settings,
    WorkerConfig,
)


class TestWorkerConfigBoundaries:
    """Tests for WorkerConfig boundary validation."""

    def test_poll_interval_must_be_positive(self):
        """Test that poll_interval_seconds must be positive."""
        with pytest.raises(ValidationError):
            WorkerConfig(poll_interval_seconds=0)

    def test_heartbeat_interval_positive(self):
        """Test that heartbeat_interval_seconds must be positive."""
        with pytest.raises(ValidationError):
            WorkerConfig(heartbeat_interval_seconds=-1)

    def test_max_task_duration_positive(self):
        """Test max_task_duration_seconds must be positive."""
        with pytest.raises(ValidationError):
            WorkerConfig(max_task_duration_seconds=0)

    def test_graceful_shutdown_timeout_positive(self):
        """Test graceful_shutdown_timeout_seconds must be positive."""
        with pytest.raises(ValidationError):
            WorkerConfig(graceful_shutdown_timeout_seconds=-5)


class TestSettingsEnvironmentVariants:
    """Tests for Settings environment handling."""

    @patch.dict(
        os.environ,
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_ENVIRONMENT": "staging",
        },
    )
    def test_staging_environment(self):
        """Test staging environment is valid."""
        settings = Settings()
        assert settings.environment == Environment.STAGING
        assert not settings.is_production()
        assert not settings.is_development()

    @patch.dict(
        os.environ,
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_ENVIRONMENT": "invalid_env",
        },
    )
    def test_invalid_environment_value(self):
        """Test that an invalid environment value raises ValidationError."""
        with pytest.raises(ValidationError):
            Settings()


class TestLLMConfigCombinations:
    """Tests for LLM config key combinations."""

    @patch.dict(
        os.environ,
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-openai-key",
            "APEX_LLM_ANTHROPIC_API_KEY": "sk-ant-key",
        },
    )
    def test_both_llm_keys_configured(self):
        """Test that both LLM keys can be configured simultaneously."""
        config = LLMConfig()
        assert config.openai_api_key == "sk-openai-key"
        assert config.anthropic_api_key == "sk-ant-key"


class TestBackendConfigCustomValues:
    """Tests for BackendConfig with custom values."""

    @patch.dict(
        os.environ,
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_BACKEND_HOST": "api.production.com",
            "APEX_BACKEND_HTTP_PORT": "443",
        },
    )
    def test_backend_custom_values(self):
        """Test backend config with custom production values."""
        settings = Settings()
        assert settings.backend.host == "api.production.com"
        assert settings.backend.http_port == 443
        assert settings.backend.http_base_url == "http://api.production.com:443"


class TestDatabaseConfigCustomValues:
    """Tests for DatabaseConfig with custom values."""

    @patch.dict(
        os.environ,
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_DATABASE_URL": "postgresql://user:pass@db:5432/apex",
            "APEX_DATABASE_POOL_SIZE": "20",
        },
    )
    def test_database_custom_values(self):
        """Test database config with custom values."""
        settings = Settings()
        assert settings.database.url == "postgresql://user:pass@db:5432/apex"
        assert settings.database.pool_size == 20


class TestRedisConfigCustomValues:
    """Tests for RedisConfig with custom values."""

    @patch.dict(
        os.environ,
        {
            "APEX_LLM_OPENAI_API_KEY": "sk-test",
            "APEX_REDIS_URL": "redis://redis-primary:6380/1",
            "APEX_REDIS_POOL_SIZE": "25",
        },
    )
    def test_redis_custom_values(self):
        """Test redis config with custom values."""
        settings = Settings()
        assert settings.redis.url == "redis://redis-primary:6380/1"
        assert settings.redis.pool_size == 25
