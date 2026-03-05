#!/bin/bash
set -e

echo "Setting up Apex..."
echo ""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Check Python
if command -v python3 &> /dev/null; then
    echo -e "${GREEN}✓${NC} python3 found"
else
    echo -e "${RED}✗${NC} python3 not found. Please install Python 3.11+"
    exit 1
fi

# Install dependencies
echo ""
echo "Installing dependencies..."
pip install -r requirements.txt
echo -e "${GREEN}✓${NC} Dependencies installed"

# Set up .env
echo ""
if [ ! -f .env ]; then
    if [ -f .env.example ]; then
        cp .env.example .env
        echo -e "${GREEN}✓${NC} Created .env from .env.example"
    else
        echo "MINIMAX_API_KEY=" > .env
        echo -e "${YELLOW}!${NC} Created empty .env — add your MINIMAX_API_KEY"
    fi
else
    echo -e "${YELLOW}!${NC} .env already exists, skipping"
fi

# Create data directory
mkdir -p data/reports
echo -e "${GREEN}✓${NC} Data directory ready"

echo ""
echo "═══════════════════════════════════════"
echo -e "${GREEN}Setup complete!${NC}"
echo ""
echo "Next steps:"
echo "  1. Edit .env and add your MINIMAX_API_KEY"
echo "  2. Run: python -m src.backend.core"
echo "  3. Open: http://localhost:8000"
echo "═══════════════════════════════════════"
