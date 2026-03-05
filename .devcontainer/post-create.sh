#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Devcontainer Post-Create Script
# Runs once after the container is created
# ══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

echo "═══════════════════════════════════════════════════════════════════════════"
echo "  Apex Development Container - Post-Create Setup"
echo "═══════════════════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────────────────────
# Rust Setup
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "[Rust] Installing project dependencies..."

if [ -f "/workspace/src/backend/core/Cargo.toml" ]; then
    cd /workspace/src/backend/core

    # Build dependencies to populate cache
    echo "[Rust] Building dependencies (this may take a while on first run)..."
    cargo fetch || true
    cargo build --release 2>/dev/null || true

    # Install additional Rust tools if not present
    echo "[Rust] Ensuring development tools are installed..."
    which cargo-watch &>/dev/null || cargo install cargo-watch --locked
    which sqlx &>/dev/null || cargo install sqlx-cli --locked

    echo "[Rust] Setup complete."
fi

# ─────────────────────────────────────────────────────────────────────────────
# Python Setup
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "[Python] Installing project dependencies..."

if [ -f "/workspace/src/backend/agents/pyproject.toml" ]; then
    cd /workspace/src/backend/agents

    # Create virtual environment if not exists
    if [ ! -d ".venv" ]; then
        echo "[Python] Creating virtual environment..."
        python -m venv .venv
    fi

    # Install dependencies
    echo "[Python] Installing dependencies..."
    source .venv/bin/activate
    pip install -e ".[dev]" 2>/dev/null || pip install -e . || true

    # Install pre-commit hooks
    if [ -f "/workspace/.pre-commit-config.yaml" ]; then
        echo "[Python] Installing pre-commit hooks..."
        cd /workspace
        pre-commit install || true
    fi

    echo "[Python] Setup complete."
fi

# ─────────────────────────────────────────────────────────────────────────────
# Node.js Setup
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "[Node.js] Installing project dependencies..."

if [ -f "/workspace/src/frontend/package.json" ]; then
    cd /workspace/src/frontend

    # Install dependencies
    echo "[Node.js] Installing npm dependencies..."
    npm install --legacy-peer-deps 2>/dev/null || true

    # Install Playwright browsers for E2E tests
    if grep -q "playwright" package.json 2>/dev/null; then
        echo "[Node.js] Installing Playwright browsers..."
        npx playwright install --with-deps chromium 2>/dev/null || true
    fi

    echo "[Node.js] Setup complete."
fi

# ─────────────────────────────────────────────────────────────────────────────
# Git Configuration
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "[Git] Configuring Git..."

cd /workspace

# Set safe directory
git config --global --add safe.directory /workspace

# Configure Git hooks path
if [ -d ".githooks" ]; then
    git config core.hooksPath .githooks
fi

echo "[Git] Configuration complete."

# ─────────────────────────────────────────────────────────────────────────────
# Database Setup
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "[Database] Setting up database..."

# Wait for PostgreSQL to be ready
echo "[Database] Waiting for PostgreSQL..."
until pg_isready -h postgres -U apex -d apex 2>/dev/null; do
    sleep 1
done

# Run migrations if sqlx is available
if [ -f "/workspace/src/backend/core/Cargo.toml" ]; then
    cd /workspace/src/backend/core

    if command -v sqlx &>/dev/null; then
        echo "[Database] Running database migrations..."
        sqlx database create 2>/dev/null || true
        sqlx migrate run 2>/dev/null || true
    fi
fi

echo "[Database] Setup complete."

# ─────────────────────────────────────────────────────────────────────────────
# Environment Setup
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "[Environment] Setting up environment..."

cd /workspace

# Copy example env file if .env doesn't exist
if [ ! -f ".env" ] && [ -f ".env.example" ]; then
    echo "[Environment] Creating .env from .env.example..."
    cp .env.example .env
fi

# Create local directories
mkdir -p /workspace/.cache /workspace/.local

echo "[Environment] Setup complete."

# ─────────────────────────────────────────────────────────────────────────────
# Final Message
# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════════════════════════"
echo "  Post-Create Setup Complete!"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""
echo "  Available services:"
echo "    - PostgreSQL: postgres:5432"
echo "    - Redis:      redis:6379"
echo "    - Jaeger UI:  http://localhost:16686"
echo "    - Grafana:    http://localhost:3001 (admin/admin)"
echo "    - MailHog:    http://localhost:8025"
echo ""
echo "  Quick start commands:"
echo "    make dev        - Start all services in dev mode"
echo "    make test       - Run all tests"
echo "    make lint       - Run linters"
echo "    make fmt        - Format code"
echo ""
echo "═══════════════════════════════════════════════════════════════════════════"
