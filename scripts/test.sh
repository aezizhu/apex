#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Test Runner
# ══════════════════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                    Project Apex - Test Suite                       ${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"
echo ""

# Parse arguments
RUN_RUST=true
RUN_PYTHON=true
RUN_FRONTEND=true
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --rust-only)
            RUN_PYTHON=false
            RUN_FRONTEND=false
            shift
            ;;
        --python-only)
            RUN_RUST=false
            RUN_FRONTEND=false
            shift
            ;;
        --frontend-only)
            RUN_RUST=false
            RUN_PYTHON=false
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Track results
RUST_RESULT=0
PYTHON_RESULT=0
FRONTEND_RESULT=0

# ──────────────────────────────────────────────────────────────────────────────
# Rust Tests
# ──────────────────────────────────────────────────────────────────────────────
if [ "$RUN_RUST" = true ]; then
    echo -e "${YELLOW}▶ Running Rust tests...${NC}"
    echo ""

    cd src/backend/core

    if [ "$VERBOSE" = true ]; then
        SQLX_OFFLINE=true cargo test -- --nocapture || RUST_RESULT=$?
    else
        SQLX_OFFLINE=true cargo test || RUST_RESULT=$?
    fi

    if [ $RUST_RESULT -eq 0 ]; then
        echo -e "${GREEN}✓ Rust tests passed${NC}"
    else
        echo -e "${RED}✗ Rust tests failed${NC}"
    fi
    echo ""

    cd "$PROJECT_ROOT"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Python Tests
# ──────────────────────────────────────────────────────────────────────────────
if [ "$RUN_PYTHON" = true ]; then
    echo -e "${YELLOW}▶ Running Python tests...${NC}"
    echo ""

    cd src/backend/agents

    if [ "$VERBOSE" = true ]; then
        python -m pytest tests/ -v --tb=short || PYTHON_RESULT=$?
    else
        python -m pytest tests/ --tb=short || PYTHON_RESULT=$?
    fi

    if [ $PYTHON_RESULT -eq 0 ]; then
        echo -e "${GREEN}✓ Python tests passed${NC}"
    else
        echo -e "${RED}✗ Python tests failed${NC}"
    fi
    echo ""

    cd "$PROJECT_ROOT"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Frontend Tests
# ──────────────────────────────────────────────────────────────────────────────
if [ "$RUN_FRONTEND" = true ]; then
    echo -e "${YELLOW}▶ Running Frontend tests...${NC}"
    echo ""

    cd src/frontend

    if [ -f "package.json" ]; then
        if command -v npm &> /dev/null; then
            # Check if vitest is configured
            if grep -q "vitest" package.json 2>/dev/null; then
                npm run test || FRONTEND_RESULT=$?
            else
                echo -e "${YELLOW}⚠ No test runner configured for frontend${NC}"
                FRONTEND_RESULT=0
            fi
        else
            echo -e "${YELLOW}⚠ npm not found, skipping frontend tests${NC}"
            FRONTEND_RESULT=0
        fi
    fi

    if [ $FRONTEND_RESULT -eq 0 ]; then
        echo -e "${GREEN}✓ Frontend tests passed (or skipped)${NC}"
    else
        echo -e "${RED}✗ Frontend tests failed${NC}"
    fi
    echo ""

    cd "$PROJECT_ROOT"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────────────
echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                         Test Summary                               ${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"

TOTAL_RESULT=0

if [ "$RUN_RUST" = true ]; then
    if [ $RUST_RESULT -eq 0 ]; then
        echo -e "  Rust:     ${GREEN}PASSED${NC}"
    else
        echo -e "  Rust:     ${RED}FAILED${NC}"
        TOTAL_RESULT=1
    fi
fi

if [ "$RUN_PYTHON" = true ]; then
    if [ $PYTHON_RESULT -eq 0 ]; then
        echo -e "  Python:   ${GREEN}PASSED${NC}"
    else
        echo -e "  Python:   ${RED}FAILED${NC}"
        TOTAL_RESULT=1
    fi
fi

if [ "$RUN_FRONTEND" = true ]; then
    if [ $FRONTEND_RESULT -eq 0 ]; then
        echo -e "  Frontend: ${GREEN}PASSED${NC}"
    else
        echo -e "  Frontend: ${RED}FAILED${NC}"
        TOTAL_RESULT=1
    fi
fi

echo ""

if [ $TOTAL_RESULT -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
else
    echo -e "${RED}Some tests failed.${NC}"
fi

exit $TOTAL_RESULT
