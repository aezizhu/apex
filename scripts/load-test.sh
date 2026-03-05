#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Load Testing Script
# Uses wrk or hey for HTTP load testing
# ══════════════════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
API_URL="${API_URL:-http://localhost:8080}"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-100}"
THREADS="${THREADS:-4}"
RATE="${RATE:-1000}"

echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}              Project Apex - Load Testing Suite                 ${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "Target: ${GREEN}$API_URL${NC}"
echo -e "Duration: ${GREEN}$DURATION${NC}"
echo -e "Connections: ${GREEN}$CONNECTIONS${NC}"
echo ""

# Check for load testing tools
if command -v hey &> /dev/null; then
    TOOL="hey"
elif command -v wrk &> /dev/null; then
    TOOL="wrk"
elif command -v ab &> /dev/null; then
    TOOL="ab"
else
    echo -e "${RED}Error: No load testing tool found.${NC}"
    echo "Please install one of: hey, wrk, or ab (Apache Bench)"
    echo ""
    echo "Install hey:  go install github.com/rakyll/hey@latest"
    echo "Install wrk:  brew install wrk  (macOS)"
    exit 1
fi

echo -e "Using: ${GREEN}$TOOL${NC}"
echo ""

# ──────────────────────────────────────────────────────────────────────────────
# Test 1: Health Check Endpoint
# ──────────────────────────────────────────────────────────────────────────────
echo -e "${YELLOW}▶ Test 1: Health Check Endpoint${NC}"
echo "   GET $API_URL/health"
echo ""

if [ "$TOOL" = "hey" ]; then
    hey -z "$DURATION" -c "$CONNECTIONS" "$API_URL/health"
elif [ "$TOOL" = "wrk" ]; then
    wrk -t"$THREADS" -c"$CONNECTIONS" -d"$DURATION" "$API_URL/health"
else
    ab -t "${DURATION%s}" -c "$CONNECTIONS" "$API_URL/health"
fi

echo ""

# ──────────────────────────────────────────────────────────────────────────────
# Test 2: List Tasks Endpoint
# ──────────────────────────────────────────────────────────────────────────────
echo -e "${YELLOW}▶ Test 2: List Tasks Endpoint${NC}"
echo "   GET $API_URL/api/v1/tasks"
echo ""

if [ "$TOOL" = "hey" ]; then
    hey -z "$DURATION" -c "$CONNECTIONS" "$API_URL/api/v1/tasks"
elif [ "$TOOL" = "wrk" ]; then
    wrk -t"$THREADS" -c"$CONNECTIONS" -d"$DURATION" "$API_URL/api/v1/tasks"
else
    ab -t "${DURATION%s}" -c "$CONNECTIONS" "$API_URL/api/v1/tasks"
fi

echo ""

# ──────────────────────────────────────────────────────────────────────────────
# Test 3: List Agents Endpoint
# ──────────────────────────────────────────────────────────────────────────────
echo -e "${YELLOW}▶ Test 3: List Agents Endpoint${NC}"
echo "   GET $API_URL/api/v1/agents"
echo ""

if [ "$TOOL" = "hey" ]; then
    hey -z "$DURATION" -c "$CONNECTIONS" "$API_URL/api/v1/agents"
elif [ "$TOOL" = "wrk" ]; then
    wrk -t"$THREADS" -c"$CONNECTIONS" -d"$DURATION" "$API_URL/api/v1/agents"
else
    ab -t "${DURATION%s}" -c "$CONNECTIONS" "$API_URL/api/v1/agents"
fi

echo ""

# ──────────────────────────────────────────────────────────────────────────────
# Test 4: Metrics Endpoint
# ──────────────────────────────────────────────────────────────────────────────
echo -e "${YELLOW}▶ Test 4: Metrics Endpoint${NC}"
echo "   GET $API_URL/metrics"
echo ""

if [ "$TOOL" = "hey" ]; then
    hey -z "$DURATION" -c "$CONNECTIONS" "$API_URL/metrics"
elif [ "$TOOL" = "wrk" ]; then
    wrk -t"$THREADS" -c"$CONNECTIONS" -d"$DURATION" "$API_URL/metrics"
else
    ab -t "${DURATION%s}" -c "$CONNECTIONS" "$API_URL/metrics"
fi

echo ""

# ──────────────────────────────────────────────────────────────────────────────
# Test 5: POST Task (if hey is available)
# ──────────────────────────────────────────────────────────────────────────────
if [ "$TOOL" = "hey" ]; then
    echo -e "${YELLOW}▶ Test 5: Create Task (POST)${NC}"
    echo "   POST $API_URL/api/v1/tasks"
    echo ""

    PAYLOAD='{"name":"Load Test Task","instruction":"Test instruction","limits":{"token_limit":1000,"cost_limit":0.01}}'

    hey -z "$DURATION" -c "$CONNECTIONS" -m POST \
        -H "Content-Type: application/json" \
        -d "$PAYLOAD" \
        "$API_URL/api/v1/tasks"

    echo ""
fi

# ──────────────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────────────
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}                    Load Testing Complete                       ${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "${GREEN}Performance targets:${NC}"
echo "  - Health check: < 10ms P99"
echo "  - List endpoints: < 50ms P99"
echo "  - Create task: < 100ms P99"
echo "  - Throughput: > 1000 req/sec"
echo ""
