#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Setup Script
# ══════════════════════════════════════════════════════════════════════════════

set -e

echo "🚀 Setting up Project Apex..."
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check prerequisites
check_command() {
    if command -v $1 &> /dev/null; then
        echo -e "${GREEN}✓${NC} $1 found"
        return 0
    else
        echo -e "${RED}✗${NC} $1 not found"
        return 1
    fi
}

echo "Checking prerequisites..."
echo "─────────────────────────"

MISSING=0

check_command "rust" || MISSING=1
check_command "cargo" || MISSING=1
check_command "python3" || MISSING=1
check_command "pip" || MISSING=1
check_command "node" || MISSING=1
check_command "npm" || MISSING=1
check_command "docker" || MISSING=1
check_command "docker-compose" || MISSING=1

echo ""

if [ $MISSING -eq 1 ]; then
    echo -e "${RED}Please install missing prerequisites and try again.${NC}"
    exit 1
fi

# Copy environment file
echo "Setting up environment..."
echo "─────────────────────────"

if [ ! -f .env ]; then
    cp .env.example .env
    echo -e "${GREEN}✓${NC} Created .env file"
else
    echo -e "${YELLOW}!${NC} .env already exists, skipping"
fi

# Install Rust dependencies
echo ""
echo "Installing Rust dependencies..."
echo "─────────────────────────────────"

cd src/backend/core
cargo fetch
echo -e "${GREEN}✓${NC} Rust dependencies installed"
cd ../../..

# Install Python dependencies
echo ""
echo "Installing Python dependencies..."
echo "──────────────────────────────────"

cd src/backend/agents
python3 -m pip install -e ".[dev]" --quiet
echo -e "${GREEN}✓${NC} Python dependencies installed"
cd ../../..

# Install frontend dependencies
echo ""
echo "Installing frontend dependencies..."
echo "────────────────────────────────────"

cd src/frontend
npm install --silent
echo -e "${GREEN}✓${NC} Frontend dependencies installed"
cd ../..

# Start Docker services
echo ""
echo "Starting Docker services..."
echo "───────────────────────────"

docker-compose up -d postgres redis jaeger prometheus grafana loki
echo -e "${GREEN}✓${NC} Docker services started"

# Wait for PostgreSQL
echo ""
echo "Waiting for PostgreSQL..."
echo "─────────────────────────"

for i in {1..30}; do
    if docker-compose exec -T postgres pg_isready -U apex &> /dev/null; then
        echo -e "${GREEN}✓${NC} PostgreSQL is ready"
        break
    fi
    sleep 1
done

# Run migrations
echo ""
echo "Running database migrations..."
echo "──────────────────────────────"

cd src/backend/core
if command -v sqlx &> /dev/null; then
    sqlx migrate run 2>/dev/null || echo -e "${YELLOW}!${NC} Migrations pending (run manually with 'make db-migrate')"
else
    echo -e "${YELLOW}!${NC} sqlx-cli not installed. Install with: cargo install sqlx-cli"
fi
cd ../../..

# Install pre-commit hooks
echo ""
echo "Installing pre-commit hooks..."
echo "──────────────────────────────"

if command -v pre-commit &> /dev/null; then
    pre-commit install 2>/dev/null && echo -e "${GREEN}✓${NC} Pre-commit hooks installed"
else
    pip install pre-commit --quiet
    pre-commit install 2>/dev/null && echo -e "${GREEN}✓${NC} Pre-commit hooks installed"
fi

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo -e "${GREEN}✓ Setup complete!${NC}"
echo ""
echo "Next steps:"
echo "  1. Edit .env with your API keys"
echo "  2. Run 'make dev' to start development servers"
echo ""
echo "Access points:"
echo "  • Dashboard:  http://localhost:3000"
echo "  • API:        http://localhost:8080"
echo "  • Grafana:    http://localhost:3001 (admin/apex_admin)"
echo "  • Jaeger:     http://localhost:16686"
echo "═══════════════════════════════════════════════════════════════"
