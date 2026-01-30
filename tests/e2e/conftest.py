"""Pytest configuration for e2e smoke tests."""

from __future__ import annotations


def pytest_configure(config) -> None:
    """Configure custom pytest markers for e2e tests."""
    config.addinivalue_line(
        "markers", "smoke: marks tests as smoke tests for quick validation"
    )
