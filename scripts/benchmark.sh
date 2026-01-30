#!/bin/bash
# ==============================================================================
# Project Apex - Comprehensive Benchmark Runner
#
# Orchestrates K6 and Locust load testing with various configurations.
# Generates reports and validates performance against thresholds.
#
# Usage:
#   ./scripts/benchmark.sh [command] [options]
#
# Commands:
#   all      - Run all benchmark tests
#   load     - Run K6 load test
#   stress   - Run K6 stress test
#   soak     - Run K6 soak test
#   locust   - Run Locust load test
#   quick    - Quick smoke test
#   report   - Generate benchmark report from results
#   help     - Show this help message
#
# Options:
#   --vus N         - Number of virtual users (default: varies by test)
#   --duration D    - Test duration (default: varies by test)
#   --host URL      - Target host URL (default: http://localhost:8080)
#   --output DIR    - Output directory for results
#   --no-thresholds - Disable threshold validation
#   --ci            - CI mode (fail on threshold violations)
# ==============================================================================

set -e

# ==============================================================================
# Configuration
# ==============================================================================

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BENCHMARKS_DIR="$PROJECT_ROOT/benchmarks"
K6_DIR="$BENCHMARKS_DIR/k6"
LOCUST_DIR="$BENCHMARKS_DIR/locust"
RESULTS_DIR="$BENCHMARKS_DIR/results"

# Default configuration
API_URL="${API_URL:-http://localhost:8080}"
WS_URL="${WS_URL:-ws://localhost:8080}"
AUTH_TOKEN="${AUTH_TOKEN:-}"

# Test defaults
DEFAULT_LOAD_VUS=50
DEFAULT_LOAD_DURATION="5m"
DEFAULT_STRESS_VUS=100
DEFAULT_STRESS_DURATION="10m"
DEFAULT_SOAK_VUS=50
DEFAULT_SOAK_DURATION="1h"
DEFAULT_LOCUST_USERS=50
DEFAULT_LOCUST_SPAWN_RATE=5
DEFAULT_LOCUST_DURATION="5m"

# Runtime options
VUS=""
DURATION=""
OUTPUT_DIR="$RESULTS_DIR"
ENABLE_THRESHOLDS=true
CI_MODE=false

# ==============================================================================
# Helper Functions
# ==============================================================================

print_header() {
    echo ""
    echo -e "${BLUE}╔══════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}  ${CYAN}$1${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_section() {
    echo ""
    echo -e "${PURPLE}▶ $1${NC}"
    echo ""
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${CYAN}ℹ $1${NC}"
}

check_dependencies() {
    local missing=()

    # Check for K6
    if ! command -v k6 &> /dev/null; then
        missing+=("k6")
    fi

    # Check for Locust (optional)
    if ! command -v locust &> /dev/null; then
        print_warning "Locust not installed (optional)"
    fi

    # Check for jq (optional, for report generation)
    if ! command -v jq &> /dev/null; then
        print_warning "jq not installed (optional, needed for report generation)"
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        print_error "Missing required dependencies: ${missing[*]}"
        echo ""
        echo "Install K6:"
        echo "  macOS:  brew install k6"
        echo "  Linux:  sudo apt-get install k6"
        echo "  Docker: docker pull grafana/k6"
        echo ""
        exit 1
    fi
}

check_api_health() {
    print_section "Checking API Health"

    local health_url="$API_URL/health"
    local max_attempts=5
    local attempt=1

    while [ $attempt -le $max_attempts ]; do
        if curl -s -f "$health_url" > /dev/null 2>&1; then
            print_success "API is healthy at $API_URL"
            return 0
        fi

        print_warning "Attempt $attempt/$max_attempts: API not responding..."
        sleep 2
        ((attempt++))
    done

    print_error "API health check failed after $max_attempts attempts"
    echo "  URL: $health_url"
    echo "  Please ensure the API is running and accessible."
    exit 1
}

create_results_dir() {
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local run_dir="$OUTPUT_DIR/run_$timestamp"

    mkdir -p "$run_dir"
    echo "$run_dir"
}

# ==============================================================================
# K6 Test Functions
# ==============================================================================

run_k6_load_test() {
    print_header "K6 Load Test"

    local vus=${VUS:-$DEFAULT_LOAD_VUS}
    local duration=${DURATION:-$DEFAULT_LOAD_DURATION}
    local run_dir=$(create_results_dir)

    print_info "Configuration:"
    echo "  Target:    $API_URL"
    echo "  VUs:       $vus"
    echo "  Duration:  $duration"
    echo "  Output:    $run_dir"
    echo ""

    local k6_args=(
        "run"
        "--out" "json=$run_dir/load-test-results.json"
    )

    if [ "$ENABLE_THRESHOLDS" = false ]; then
        k6_args+=("--no-thresholds")
    fi

    # Set environment variables
    export API_URL="$API_URL"
    export WS_URL="$WS_URL"
    export AUTH_TOKEN="$AUTH_TOKEN"

    print_section "Running K6 Load Test"

    if k6 "${k6_args[@]}" "$K6_DIR/load-test.js" 2>&1 | tee "$run_dir/load-test.log"; then
        print_success "Load test completed successfully"
        echo "  Results: $run_dir"
        return 0
    else
        print_error "Load test failed"
        if [ "$CI_MODE" = true ]; then
            exit 1
        fi
        return 1
    fi
}

run_k6_stress_test() {
    print_header "K6 Stress Test"

    local run_dir=$(create_results_dir)

    print_info "Configuration:"
    echo "  Target:    $API_URL"
    echo "  Output:    $run_dir"
    echo ""

    print_warning "Stress test will push the system beyond normal capacity."
    print_warning "Monitor system resources during this test."
    echo ""

    local k6_args=(
        "run"
        "--out" "json=$run_dir/stress-test-results.json"
    )

    if [ "$ENABLE_THRESHOLDS" = false ]; then
        k6_args+=("--no-thresholds")
    fi

    export API_URL="$API_URL"
    export WS_URL="$WS_URL"
    export AUTH_TOKEN="$AUTH_TOKEN"

    print_section "Running K6 Stress Test"

    if k6 "${k6_args[@]}" "$K6_DIR/stress-test.js" 2>&1 | tee "$run_dir/stress-test.log"; then
        print_success "Stress test completed"
        echo "  Results: $run_dir"
        return 0
    else
        print_error "Stress test failed or thresholds violated"
        if [ "$CI_MODE" = true ]; then
            exit 1
        fi
        return 1
    fi
}

run_k6_soak_test() {
    print_header "K6 Soak Test"

    local vus=${VUS:-$DEFAULT_SOAK_VUS}
    local duration=${DURATION:-$DEFAULT_SOAK_DURATION}
    local run_dir=$(create_results_dir)

    print_info "Configuration:"
    echo "  Target:    $API_URL"
    echo "  VUs:       $vus"
    echo "  Duration:  $duration"
    echo "  Output:    $run_dir"
    echo ""

    print_warning "Soak test is designed for extended duration."
    print_warning "Default duration: $DEFAULT_SOAK_DURATION"
    print_warning "Monitor memory and connection usage during this test."
    echo ""

    local k6_args=(
        "run"
        "--out" "json=$run_dir/soak-test-results.json"
    )

    if [ "$ENABLE_THRESHOLDS" = false ]; then
        k6_args+=("--no-thresholds")
    fi

    export API_URL="$API_URL"
    export WS_URL="$WS_URL"
    export AUTH_TOKEN="$AUTH_TOKEN"
    export SOAK_DURATION="$duration"
    export TARGET_VUS="$vus"

    print_section "Running K6 Soak Test"

    if k6 "${k6_args[@]}" "$K6_DIR/soak-test.js" 2>&1 | tee "$run_dir/soak-test.log"; then
        print_success "Soak test completed"
        echo "  Results: $run_dir"
        return 0
    else
        print_error "Soak test failed"
        if [ "$CI_MODE" = true ]; then
            exit 1
        fi
        return 1
    fi
}

# ==============================================================================
# Locust Test Functions
# ==============================================================================

run_locust_test() {
    print_header "Locust Load Test"

    if ! command -v locust &> /dev/null; then
        print_error "Locust is not installed"
        echo "  Install with: pip install locust"
        exit 1
    fi

    local users=${VUS:-$DEFAULT_LOCUST_USERS}
    local spawn_rate=${LOCUST_SPAWN_RATE:-$DEFAULT_LOCUST_SPAWN_RATE}
    local duration=${DURATION:-$DEFAULT_LOCUST_DURATION}
    local run_dir=$(create_results_dir)

    print_info "Configuration:"
    echo "  Target:      $API_URL"
    echo "  Users:       $users"
    echo "  Spawn Rate:  $spawn_rate"
    echo "  Duration:    $duration"
    echo "  Output:      $run_dir"
    echo ""

    # Check if we should run headless or with web UI
    if [ "$CI_MODE" = true ] || [ -n "$DURATION" ]; then
        print_section "Running Locust (Headless Mode)"

        locust -f "$LOCUST_DIR/locustfile.py" \
            --host="$API_URL" \
            --headless \
            --users "$users" \
            --spawn-rate "$spawn_rate" \
            --run-time "$duration" \
            --csv="$run_dir/locust" \
            --html="$run_dir/locust-report.html" \
            2>&1 | tee "$run_dir/locust.log"

        print_success "Locust test completed"
        echo "  Results: $run_dir"
        echo "  HTML Report: $run_dir/locust-report.html"
    else
        print_section "Starting Locust Web UI"
        print_info "Web UI will be available at http://localhost:8089"
        print_info "Press Ctrl+C to stop"
        echo ""

        locust -f "$LOCUST_DIR/locustfile.py" --host="$API_URL"
    fi
}

# ==============================================================================
# Quick Smoke Test
# ==============================================================================

run_quick_test() {
    print_header "Quick Smoke Test"

    print_info "Running a quick smoke test with minimal load"
    echo "  Target: $API_URL"
    echo "  VUs: 5"
    echo "  Duration: 30s"
    echo ""

    export API_URL="$API_URL"
    export WS_URL="$WS_URL"

    k6 run --vus 5 --duration 30s "$K6_DIR/load-test.js"

    print_success "Smoke test completed"
}

# ==============================================================================
# Run All Tests
# ==============================================================================

run_all_tests() {
    print_header "Running All Benchmark Tests"

    local start_time=$(date +%s)
    local failed_tests=()

    # Load Test
    if ! run_k6_load_test; then
        failed_tests+=("load")
    fi

    # Stress Test
    if ! run_k6_stress_test; then
        failed_tests+=("stress")
    fi

    # Note: Soak test is typically run separately due to duration
    print_warning "Soak test skipped (run separately with: ./scripts/benchmark.sh soak)"

    # Generate Report
    generate_report

    local end_time=$(date +%s)
    local duration=$((end_time - start_time))

    print_header "Benchmark Suite Complete"

    echo "  Total Duration: ${duration}s"
    echo "  Results: $RESULTS_DIR"

    if [ ${#failed_tests[@]} -ne 0 ]; then
        print_error "Failed tests: ${failed_tests[*]}"
        if [ "$CI_MODE" = true ]; then
            exit 1
        fi
    else
        print_success "All tests passed!"
    fi
}

# ==============================================================================
# Report Generation
# ==============================================================================

generate_report() {
    print_header "Generating Benchmark Report"

    if ! command -v jq &> /dev/null; then
        print_warning "jq is not installed, skipping detailed report generation"
        return 0
    fi

    local report_file="$RESULTS_DIR/benchmark-report.md"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')

    cat > "$report_file" << EOF
# Project Apex - Benchmark Report

Generated: $timestamp

## Configuration

- **Target API**: $API_URL
- **Environment**: ${ENVIRONMENT:-local}

## Summary

EOF

    # Parse most recent results if available
    local latest_load_result=$(ls -t "$RESULTS_DIR"/run_*/load-test-summary.json 2>/dev/null | head -1)
    local latest_stress_result=$(ls -t "$RESULTS_DIR"/run_*/stress-test-summary.json 2>/dev/null | head -1)
    local latest_soak_result=$(ls -t "$RESULTS_DIR"/run_*/soak-test-summary.json 2>/dev/null | head -1)

    if [ -f "$latest_load_result" ]; then
        echo "### Load Test Results" >> "$report_file"
        echo "" >> "$report_file"
        echo "| Metric | Value |" >> "$report_file"
        echo "|--------|-------|" >> "$report_file"
        jq -r '"| Total Requests | \(.metrics.http.requests) |"' "$latest_load_result" >> "$report_file"
        jq -r '"| Error Rate | \(.metrics.http.failures) |"' "$latest_load_result" >> "$report_file"
        jq -r '"| P50 Response Time | \(.metrics.http.duration.p50)ms |"' "$latest_load_result" >> "$report_file"
        jq -r '"| P95 Response Time | \(.metrics.http.duration.p95)ms |"' "$latest_load_result" >> "$report_file"
        jq -r '"| P99 Response Time | \(.metrics.http.duration.p99)ms |"' "$latest_load_result" >> "$report_file"
        echo "" >> "$report_file"
    fi

    if [ -f "$latest_stress_result" ]; then
        echo "### Stress Test Results" >> "$report_file"
        echo "" >> "$report_file"
        echo "| Metric | Value |" >> "$report_file"
        echo "|--------|-------|" >> "$report_file"
        jq -r '"| Breaking Point Analysis | \(.breakpointAnalysis) |"' "$latest_stress_result" >> "$report_file" 2>/dev/null || true
        jq -r '"| Total Requests | \(.metrics.http.totalRequests) |"' "$latest_stress_result" >> "$report_file"
        jq -r '"| Error Rate | \(.metrics.http.errorRate) |"' "$latest_stress_result" >> "$report_file"
        echo "" >> "$report_file"
    fi

    if [ -f "$latest_soak_result" ]; then
        echo "### Soak Test Results" >> "$report_file"
        echo "" >> "$report_file"
        echo "| Metric | Value |" >> "$report_file"
        echo "|--------|-------|" >> "$report_file"
        jq -r '"| Duration | \(.durationMinutes) minutes |"' "$latest_soak_result" >> "$report_file"
        jq -r '"| Total Requests | \(.metrics.http.totalRequests) |"' "$latest_soak_result" >> "$report_file"
        jq -r '"| Error Rate | \(.metrics.http.errorRate) |"' "$latest_soak_result" >> "$report_file"
        echo "" >> "$report_file"
        echo "Stability Analysis:" >> "$report_file"
        jq -r '.stabilityAnalysis[] | "- \(.)"' "$latest_soak_result" >> "$report_file" 2>/dev/null || true
        echo "" >> "$report_file"
    fi

    cat >> "$report_file" << EOF

## Performance Targets

| Endpoint | P50 Target | P95 Target | P99 Target | Status |
|----------|------------|------------|------------|--------|
| GET /health | < 5ms | < 10ms | < 20ms | - |
| GET /api/v1/tasks | < 20ms | < 50ms | < 100ms | - |
| POST /api/v1/tasks | < 50ms | < 100ms | < 200ms | - |
| GET /api/v1/agents | < 20ms | < 50ms | < 100ms | - |
| POST /api/v1/dags/{id}/execute | < 100ms | < 200ms | < 500ms | - |

---
*Report generated by Project Apex Benchmark Suite*
EOF

    print_success "Report generated: $report_file"
}

# ==============================================================================
# Help
# ==============================================================================

show_help() {
    cat << EOF
Project Apex - Benchmark Runner

Usage: ./scripts/benchmark.sh [command] [options]

Commands:
  all       Run all benchmark tests (load, stress)
  load      Run K6 load test
  stress    Run K6 stress test
  soak      Run K6 soak test (extended duration)
  locust    Run Locust load test (with web UI by default)
  quick     Quick smoke test (30 seconds)
  report    Generate benchmark report from existing results
  help      Show this help message

Options:
  --vus N           Number of virtual users
  --duration D      Test duration (e.g., 5m, 1h)
  --host URL        Target host URL (default: http://localhost:8080)
  --output DIR      Output directory for results
  --no-thresholds   Disable threshold validation
  --ci              CI mode (exit with error on threshold violations)

Environment Variables:
  API_URL           Target API base URL (default: http://localhost:8080)
  WS_URL            WebSocket URL (default: ws://localhost:8080)
  AUTH_TOKEN        Bearer token for authenticated endpoints
  ENVIRONMENT       Environment name for tagging (default: local)

Examples:
  # Run load test with default settings
  ./scripts/benchmark.sh load

  # Run stress test against production
  API_URL=https://api.example.com ./scripts/benchmark.sh stress

  # Run quick smoke test
  ./scripts/benchmark.sh quick

  # Run soak test for 2 hours
  ./scripts/benchmark.sh soak --duration 2h

  # Run Locust with web UI
  ./scripts/benchmark.sh locust

  # CI mode - fail on threshold violations
  ./scripts/benchmark.sh load --ci

  # Generate report from existing results
  ./scripts/benchmark.sh report
EOF
}

# ==============================================================================
# Argument Parsing
# ==============================================================================

parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --vus)
                VUS="$2"
                shift 2
                ;;
            --duration)
                DURATION="$2"
                shift 2
                ;;
            --host)
                API_URL="$2"
                shift 2
                ;;
            --output)
                OUTPUT_DIR="$2"
                shift 2
                ;;
            --no-thresholds)
                ENABLE_THRESHOLDS=false
                shift
                ;;
            --ci)
                CI_MODE=true
                shift
                ;;
            *)
                shift
                ;;
        esac
    done
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    local command="${1:-help}"
    shift || true

    parse_args "$@"

    # Create results directory
    mkdir -p "$RESULTS_DIR"

    case $command in
        all)
            check_dependencies
            check_api_health
            run_all_tests
            ;;
        load)
            check_dependencies
            check_api_health
            run_k6_load_test
            ;;
        stress)
            check_dependencies
            check_api_health
            run_k6_stress_test
            ;;
        soak)
            check_dependencies
            check_api_health
            run_k6_soak_test
            ;;
        locust)
            check_api_health
            run_locust_test
            ;;
        quick)
            check_dependencies
            check_api_health
            run_quick_test
            ;;
        report)
            generate_report
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            print_error "Unknown command: $command"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

main "$@"
