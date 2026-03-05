#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Integration Test Runner
# ══════════════════════════════════════════════════════════════════════════════
#
# This script manages the integration test infrastructure and runs tests.
#
# Usage:
#   ./scripts/run-integration-tests.sh [options]
#
# Options:
#   --setup-only       Only set up infrastructure, don't run tests
#   --teardown-only    Only tear down infrastructure
#   --no-teardown      Keep infrastructure running after tests
#   --verbose, -v      Run tests with verbose output
#   --coverage         Generate coverage report
#   --parallel, -p     Run tests in parallel
#   --filter, -k       Run tests matching pattern (pytest -k)
#   --markers, -m      Run tests with specific markers (pytest -m)
#   --debug            Start infrastructure with debug profile
#   --help, -h         Show this help message
#
# Examples:
#   ./scripts/run-integration-tests.sh                    # Run all tests
#   ./scripts/run-integration-tests.sh -v                 # Verbose output
#   ./scripts/run-integration-tests.sh -k "test_create"   # Filter tests
#   ./scripts/run-integration-tests.sh -m "not slow"      # Skip slow tests
#   ./scripts/run-integration-tests.sh --coverage         # With coverage
#   ./scripts/run-integration-tests.sh --setup-only       # Just start infra
#
# ══════════════════════════════════════════════════════════════════════════════

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="$PROJECT_ROOT/tests/integration/docker-compose.test.yml"

# Default configuration
SETUP_ONLY=false
TEARDOWN_ONLY=false
NO_TEARDOWN=false
VERBOSE=false
COVERAGE=false
PARALLEL=false
DEBUG=false
FILTER=""
MARKERS=""

# Test configuration (can be overridden by environment)
export TEST_API_URL="${TEST_API_URL:-http://localhost:8081}"
export TEST_WS_URL="${TEST_WS_URL:-ws://localhost:8081/ws}"
export TEST_API_KEY="${TEST_API_KEY:-test-api-key}"
export TEST_DB_URL="${TEST_DB_URL:-postgres://apex:apex_test@localhost:5433/apex_test}"
export TEST_REDIS_URL="${TEST_REDIS_URL:-redis://localhost:6380}"
export TEST_TIMEOUT="${TEST_TIMEOUT:-30}"

# ══════════════════════════════════════════════════════════════════════════════
# Functions
# ══════════════════════════════════════════════════════════════════════════════

print_header() {
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}          Project Apex - Integration Test Suite                     ${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"
    echo ""
}

print_help() {
    head -35 "$0" | tail -30
}

log_info() {
    echo -e "${CYAN}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_dependencies() {
    log_info "Checking dependencies..."

    # Check Docker
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi

    # Check Docker Compose
    if ! command -v docker &> /dev/null || ! docker compose version &> /dev/null; then
        log_error "Docker Compose is not available"
        exit 1
    fi

    # Check Python
    if ! command -v python &> /dev/null && ! command -v python3 &> /dev/null; then
        log_error "Python is not installed"
        exit 1
    fi

    # Check pytest
    if ! python -m pytest --version &> /dev/null && ! python3 -m pytest --version &> /dev/null; then
        log_warning "pytest is not installed. Installing..."
        pip install pytest pytest-asyncio pytest-cov || pip3 install pytest pytest-asyncio pytest-cov
    fi

    log_success "All dependencies satisfied"
}

start_infrastructure() {
    log_info "Starting test infrastructure..."

    cd "$PROJECT_ROOT"

    # Build and start containers
    COMPOSE_PROFILES=""
    if [ "$DEBUG" = true ]; then
        COMPOSE_PROFILES="--profile debug"
    fi

    docker compose -f "$COMPOSE_FILE" $COMPOSE_PROFILES up -d --build

    log_info "Waiting for services to be healthy..."

    # Wait for API to be ready
    local max_attempts=60
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        if curl -s -f "http://localhost:8081/health" > /dev/null 2>&1; then
            log_success "API is healthy"
            break
        fi
        attempt=$((attempt + 1))
        echo -ne "\r${CYAN}[INFO]${NC} Waiting for API... ($attempt/$max_attempts)"
        sleep 2
    done
    echo ""

    if [ $attempt -eq $max_attempts ]; then
        log_error "API failed to become healthy"
        docker compose -f "$COMPOSE_FILE" logs apex-api-test
        exit 1
    fi

    # Wait for database
    attempt=0
    while [ $attempt -lt 30 ]; do
        if docker compose -f "$COMPOSE_FILE" exec -T postgres-test pg_isready -U apex -d apex_test > /dev/null 2>&1; then
            log_success "Database is ready"
            break
        fi
        attempt=$((attempt + 1))
        sleep 1
    done

    # Wait for Redis
    attempt=0
    while [ $attempt -lt 30 ]; do
        if docker compose -f "$COMPOSE_FILE" exec -T redis-test redis-cli ping > /dev/null 2>&1; then
            log_success "Redis is ready"
            break
        fi
        attempt=$((attempt + 1))
        sleep 1
    done

    log_success "Test infrastructure is ready"
    echo ""
}

stop_infrastructure() {
    log_info "Stopping test infrastructure..."

    cd "$PROJECT_ROOT"
    docker compose -f "$COMPOSE_FILE" down -v --remove-orphans

    log_success "Test infrastructure stopped"
}

run_tests() {
    log_info "Running integration tests..."
    echo ""

    cd "$PROJECT_ROOT"

    # Build pytest command
    PYTEST_CMD="python -m pytest tests/integration/"

    # Add options
    if [ "$VERBOSE" = true ]; then
        PYTEST_CMD="$PYTEST_CMD -v"
    fi

    if [ "$COVERAGE" = true ]; then
        PYTEST_CMD="$PYTEST_CMD --cov=apex_sdk --cov-report=html --cov-report=term-missing"
    fi

    if [ "$PARALLEL" = true ]; then
        # Requires pytest-xdist
        PYTEST_CMD="$PYTEST_CMD -n auto"
    fi

    if [ -n "$FILTER" ]; then
        PYTEST_CMD="$PYTEST_CMD -k \"$FILTER\""
    fi

    if [ -n "$MARKERS" ]; then
        PYTEST_CMD="$PYTEST_CMD -m \"$MARKERS\""
    fi

    # Add standard options
    PYTEST_CMD="$PYTEST_CMD --tb=short --strict-markers"

    # Run tests
    log_info "Executing: $PYTEST_CMD"
    echo ""

    # Add SDK to path
    export PYTHONPATH="$PROJECT_ROOT/sdk/python:$PYTHONPATH"

    # Run with eval to handle quoted arguments
    eval $PYTEST_CMD
    TEST_RESULT=$?

    return $TEST_RESULT
}

show_logs() {
    log_info "Showing service logs..."
    docker compose -f "$COMPOSE_FILE" logs --tail=50
}

# ══════════════════════════════════════════════════════════════════════════════
# Argument Parsing
# ══════════════════════════════════════════════════════════════════════════════

while [[ $# -gt 0 ]]; do
    case $1 in
        --setup-only)
            SETUP_ONLY=true
            shift
            ;;
        --teardown-only)
            TEARDOWN_ONLY=true
            shift
            ;;
        --no-teardown)
            NO_TEARDOWN=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        --coverage)
            COVERAGE=true
            shift
            ;;
        -p|--parallel)
            PARALLEL=true
            shift
            ;;
        -k|--filter)
            FILTER="$2"
            shift 2
            ;;
        -m|--markers)
            MARKERS="$2"
            shift 2
            ;;
        --debug)
            DEBUG=true
            shift
            ;;
        -h|--help)
            print_help
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            print_help
            exit 1
            ;;
    esac
done

# ══════════════════════════════════════════════════════════════════════════════
# Main Execution
# ══════════════════════════════════════════════════════════════════════════════

print_header

# Handle teardown-only
if [ "$TEARDOWN_ONLY" = true ]; then
    stop_infrastructure
    exit 0
fi

# Check dependencies
check_dependencies

# Start infrastructure
start_infrastructure

# Handle setup-only
if [ "$SETUP_ONLY" = true ]; then
    echo ""
    log_success "Infrastructure is running. Use the following to connect:"
    echo ""
    echo -e "  ${CYAN}API:${NC}      $TEST_API_URL"
    echo -e "  ${CYAN}WebSocket:${NC} $TEST_WS_URL"
    echo -e "  ${CYAN}Database:${NC}  $TEST_DB_URL"
    echo -e "  ${CYAN}Redis:${NC}     $TEST_REDIS_URL"
    echo ""
    echo -e "To run tests manually:"
    echo -e "  ${YELLOW}PYTHONPATH=sdk/python pytest tests/integration/ -v${NC}"
    echo ""
    echo -e "To stop infrastructure:"
    echo -e "  ${YELLOW}$0 --teardown-only${NC}"
    exit 0
fi

# Run tests
TEST_RESULT=0
run_tests || TEST_RESULT=$?

# Show summary
echo ""
echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                         Test Summary                               ${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════════${NC}"

if [ $TEST_RESULT -eq 0 ]; then
    echo -e "  Result: ${GREEN}PASSED${NC}"
else
    echo -e "  Result: ${RED}FAILED${NC}"

    # Show logs on failure
    echo ""
    log_warning "Showing recent service logs..."
    show_logs
fi

# Teardown unless --no-teardown
if [ "$NO_TEARDOWN" = false ]; then
    echo ""
    stop_infrastructure
else
    echo ""
    log_info "Infrastructure is still running (--no-teardown)"
    log_info "Run '$0 --teardown-only' to stop"
fi

exit $TEST_RESULT
