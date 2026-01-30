# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Makefile
# ══════════════════════════════════════════════════════════════════════════════

.PHONY: help setup dev build test lint clean docker-up docker-down \
        rust-build rust-test rust-watch rust-lint rust-bench \
        python-test python-lint python-dev \
        frontend-dev frontend-build frontend-test frontend-lint \
        db-migrate db-reset db-seed db-prepare \
        docker-build docker-logs docker-prune \
        health load-test pre-commit install-hooks

# Default target
help:
	@echo "Project Apex - Available Commands"
	@echo "══════════════════════════════════════════════════════════════════"
	@echo ""
	@echo "  make setup        - Install all dependencies"
	@echo "  make dev          - Start development environment"
	@echo "  make build        - Build all components"
	@echo "  make test         - Run all tests"
	@echo "  make lint         - Run linters"
	@echo "  make clean        - Clean build artifacts"
	@echo ""
	@echo "  make docker-up    - Start Docker Compose stack"
	@echo "  make docker-down  - Stop Docker Compose stack"
	@echo ""
	@echo "  make rust-build   - Build Rust backend"
	@echo "  make rust-test    - Test Rust backend"
	@echo "  make rust-watch   - Watch and rebuild Rust"
	@echo ""
	@echo "  make python-test  - Test Python agents"
	@echo "  make python-lint  - Lint Python code"
	@echo ""
	@echo "  make frontend-dev - Start frontend dev server"
	@echo "  make frontend-build - Build frontend"
	@echo ""
	@echo "  make db-migrate   - Run database migrations"
	@echo "  make db-reset     - Reset database"
	@echo ""

# ──────────────────────────────────────────────────────────────────────────────
# Setup
# ──────────────────────────────────────────────────────────────────────────────

setup: setup-rust setup-python setup-frontend
	@echo "✅ All dependencies installed"

setup-rust:
	@echo "📦 Installing Rust dependencies..."
	cd src/backend/core && cargo fetch

setup-python:
	@echo "📦 Installing Python dependencies..."
	cd src/backend/agents && pip install -e ".[dev]"

setup-frontend:
	@echo "📦 Installing frontend dependencies..."
	cd src/frontend && npm install

# ──────────────────────────────────────────────────────────────────────────────
# Development
# ──────────────────────────────────────────────────────────────────────────────

dev: docker-up
	@echo "🚀 Starting development environment..."
	@make -j3 rust-watch python-dev frontend-dev

rust-watch:
	cd src/backend/core && cargo watch -x run

python-dev:
	cd src/backend/agents && python main.py

frontend-dev:
	cd src/frontend && npm run dev

# ──────────────────────────────────────────────────────────────────────────────
# Build
# ──────────────────────────────────────────────────────────────────────────────

build: rust-build frontend-build
	@echo "✅ Build complete"

rust-build:
	@echo "🔨 Building Rust backend..."
	cd src/backend/core && cargo build --release

frontend-build:
	@echo "🔨 Building frontend..."
	cd src/frontend && npm run build

# ──────────────────────────────────────────────────────────────────────────────
# Test
# ──────────────────────────────────────────────────────────────────────────────

test: rust-test python-test frontend-test
	@echo "✅ All tests passed"

rust-test:
	@echo "🧪 Running Rust tests..."
	cd src/backend/core && cargo test

python-test:
	@echo "🧪 Running Python tests..."
	cd src/backend/agents && pytest

frontend-test:
	@echo "🧪 Running frontend tests..."
	cd src/frontend && npm test

# ──────────────────────────────────────────────────────────────────────────────
# Lint
# ──────────────────────────────────────────────────────────────────────────────

lint: rust-lint python-lint frontend-lint
	@echo "✅ Linting complete"

rust-lint:
	@echo "🔍 Linting Rust..."
	cd src/backend/core && cargo fmt --check && cargo clippy

python-lint:
	@echo "🔍 Linting Python..."
	cd src/backend/agents && ruff check . && ruff format --check .

frontend-lint:
	@echo "🔍 Linting frontend..."
	cd src/frontend && npm run lint

# ──────────────────────────────────────────────────────────────────────────────
# Docker
# ──────────────────────────────────────────────────────────────────────────────

docker-up:
	@echo "🐳 Starting Docker services..."
	docker-compose up -d postgres redis jaeger prometheus grafana loki

docker-down:
	@echo "🐳 Stopping Docker services..."
	docker-compose down

docker-build:
	@echo "🐳 Building Docker images..."
	docker-compose build

docker-logs:
	docker-compose logs -f

# ──────────────────────────────────────────────────────────────────────────────
# Database
# ──────────────────────────────────────────────────────────────────────────────

db-migrate:
	@echo "📊 Running migrations..."
	cd src/backend/core && sqlx migrate run

db-reset:
	@echo "📊 Resetting database..."
	docker-compose exec postgres psql -U apex -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
	@make db-migrate

db-seed:
	@echo "📊 Seeding database..."
	cd src/backend/core && cargo run --bin seed

# ──────────────────────────────────────────────────────────────────────────────
# Clean
# ──────────────────────────────────────────────────────────────────────────────

clean:
	@echo "🧹 Cleaning build artifacts..."
	cd src/backend/core && cargo clean
	cd src/frontend && rm -rf dist node_modules/.cache
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .pytest_cache -exec rm -rf {} + 2>/dev/null || true
	@echo "✅ Clean complete"

# ──────────────────────────────────────────────────────────────────────────────
# Release
# ──────────────────────────────────────────────────────────────────────────────

release: lint test build
	@echo "📦 Creating release..."
	@echo "Version: $(VERSION)"

benchmark rust-bench:
	@echo "📊 Running benchmarks..."
	cd src/backend/core && cargo bench

# ──────────────────────────────────────────────────────────────────────────────
# Health & Monitoring
# ──────────────────────────────────────────────────────────────────────────────

health:
	@echo "🏥 Health Check..."
	@echo ""
	@echo "API Server:"
	@curl -sf http://localhost:8080/health && echo " ✅ Healthy" || echo " ❌ Unhealthy"
	@echo ""
	@echo "PostgreSQL:"
	@docker-compose exec -T postgres pg_isready -U apex && echo " ✅ Ready" || echo " ❌ Not ready"
	@echo ""
	@echo "Redis:"
	@docker-compose exec -T redis redis-cli ping | grep -q PONG && echo " ✅ Ready" || echo " ❌ Not ready"
	@echo ""
	@echo "Jaeger:"
	@curl -sf http://localhost:16686 > /dev/null && echo " ✅ Ready" || echo " ❌ Not ready"
	@echo ""
	@echo "Prometheus:"
	@curl -sf http://localhost:9090/-/ready > /dev/null && echo " ✅ Ready" || echo " ❌ Not ready"
	@echo ""
	@echo "Grafana:"
	@curl -sf http://localhost:3001/api/health > /dev/null && echo " ✅ Ready" || echo " ❌ Not ready"

load-test:
	@echo "🔥 Running load tests..."
	./scripts/load-test.sh

# ──────────────────────────────────────────────────────────────────────────────
# Pre-commit Hooks
# ──────────────────────────────────────────────────────────────────────────────

install-hooks:
	@echo "🪝 Installing pre-commit hooks..."
	pip install pre-commit
	pre-commit install
	pre-commit install --hook-type commit-msg
	@echo "✅ Hooks installed"

pre-commit:
	@echo "🪝 Running pre-commit on all files..."
	pre-commit run --all-files

# ──────────────────────────────────────────────────────────────────────────────
# Database Utilities
# ──────────────────────────────────────────────────────────────────────────────

db-prepare:
	@echo "📊 Preparing SQLx offline data..."
	cd src/backend/core && cargo sqlx prepare

db-status:
	@echo "📊 Migration status..."
	./scripts/migrate.sh status

# ──────────────────────────────────────────────────────────────────────────────
# Docker Utilities
# ──────────────────────────────────────────────────────────────────────────────

docker-prune:
	@echo "🐳 Pruning Docker resources..."
	docker system prune -f
	docker volume prune -f

docker-full:
	@echo "🐳 Starting full stack..."
	docker-compose up -d

docker-restart:
	@echo "🐳 Restarting services..."
	docker-compose restart

# ──────────────────────────────────────────────────────────────────────────────
# CI/CD Helpers
# ──────────────────────────────────────────────────────────────────────────────

ci: lint test
	@echo "✅ CI checks passed"

ci-full: lint test build
	@echo "✅ Full CI checks passed"
