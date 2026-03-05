#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Container Entrypoint Script
# ══════════════════════════════════════════════════════════════════════════════
#
# This script provides:
#   - Proper signal handling (SIGTERM, SIGINT)
#   - Environment validation
#   - Health check endpoints
#   - Graceful shutdown
#   - Pre-flight checks
#
# Usage:
#   ENTRYPOINT ["/app/entrypoint.sh"]
#   CMD ["your-application", "--args"]
#
# ══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────

# Colors for output (disable if not a TTY)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Default values
SHUTDOWN_TIMEOUT="${SHUTDOWN_TIMEOUT:-30}"
HEALTH_CHECK_PORT="${HEALTH_CHECK_PORT:-8081}"
ENABLE_HEALTH_SERVER="${ENABLE_HEALTH_SERVER:-false}"

# PID of the main process
MAIN_PID=""

# ─────────────────────────────────────────────────────────────────────────────
# Logging Functions
# ─────────────────────────────────────────────────────────────────────────────

log_info() {
    echo -e "${BLUE}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*" >&2
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*" >&2
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*"
}

# ─────────────────────────────────────────────────────────────────────────────
# Signal Handling
# ─────────────────────────────────────────────────────────────────────────────

# Handle shutdown signals
shutdown() {
    local signal=$1
    log_info "Received $signal signal, initiating graceful shutdown..."

    if [ -n "$MAIN_PID" ] && kill -0 "$MAIN_PID" 2>/dev/null; then
        log_info "Sending SIGTERM to main process (PID: $MAIN_PID)..."
        kill -TERM "$MAIN_PID" 2>/dev/null || true

        # Wait for graceful shutdown with timeout
        local count=0
        while kill -0 "$MAIN_PID" 2>/dev/null && [ $count -lt $SHUTDOWN_TIMEOUT ]; do
            sleep 1
            count=$((count + 1))
            if [ $((count % 5)) -eq 0 ]; then
                log_info "Waiting for process to terminate... ($count/${SHUTDOWN_TIMEOUT}s)"
            fi
        done

        # Force kill if still running
        if kill -0 "$MAIN_PID" 2>/dev/null; then
            log_warn "Process did not terminate gracefully, sending SIGKILL..."
            kill -KILL "$MAIN_PID" 2>/dev/null || true
        else
            log_success "Process terminated gracefully"
        fi
    fi

    log_info "Shutdown complete"
    exit 0
}

# Set up signal traps
trap 'shutdown SIGTERM' SIGTERM
trap 'shutdown SIGINT' SIGINT
trap 'shutdown SIGHUP' SIGHUP

# ─────────────────────────────────────────────────────────────────────────────
# Health Check Server (Optional)
# ─────────────────────────────────────────────────────────────────────────────

start_health_server() {
    if [ "$ENABLE_HEALTH_SERVER" = "true" ]; then
        log_info "Starting health check server on port $HEALTH_CHECK_PORT..."

        # Simple health check server using netcat (if available)
        if command -v nc &> /dev/null; then
            while true; do
                echo -e "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"healthy\",\"timestamp\":\"$(date -Iseconds)\"}" | \
                    nc -l -p "$HEALTH_CHECK_PORT" -q 1 2>/dev/null || true
            done &
            log_success "Health server started"
        else
            log_warn "netcat not available, health server disabled"
        fi
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Pre-flight Checks
# ─────────────────────────────────────────────────────────────────────────────

check_required_env() {
    local missing=()

    # Check for required environment variables (customize as needed)
    # Example: Add required variables to this array
    local required_vars=()

    # Parse REQUIRED_ENV_VARS if set (comma-separated list)
    if [ -n "${REQUIRED_ENV_VARS:-}" ]; then
        IFS=',' read -ra required_vars <<< "$REQUIRED_ENV_VARS"
    fi

    for var in "${required_vars[@]}"; do
        var=$(echo "$var" | xargs)  # Trim whitespace
        if [ -z "${!var:-}" ]; then
            missing+=("$var")
        fi
    done

    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing required environment variables: ${missing[*]}"
        exit 1
    fi
}

wait_for_service() {
    local host=$1
    local port=$2
    local timeout=${3:-30}
    local service_name=${4:-"$host:$port"}

    log_info "Waiting for $service_name to be available..."

    local count=0
    while ! nc -z "$host" "$port" 2>/dev/null; do
        count=$((count + 1))
        if [ $count -ge $timeout ]; then
            log_error "Timeout waiting for $service_name"
            return 1
        fi
        sleep 1
    done

    log_success "$service_name is available"
    return 0
}

check_dependencies() {
    # Wait for PostgreSQL if DATABASE_URL is set
    if [ -n "${DATABASE_URL:-}" ]; then
        # Extract host and port from DATABASE_URL
        local db_host db_port
        db_host=$(echo "$DATABASE_URL" | sed -n 's|.*@\([^:/]*\).*|\1|p')
        db_port=$(echo "$DATABASE_URL" | sed -n 's|.*:\([0-9]*\)/.*|\1|p')
        db_port=${db_port:-5432}

        if [ -n "$db_host" ]; then
            wait_for_service "$db_host" "$db_port" 60 "PostgreSQL" || exit 1
        fi
    fi

    # Wait for Redis if REDIS_URL is set
    if [ -n "${REDIS_URL:-}" ]; then
        local redis_host redis_port
        redis_host=$(echo "$REDIS_URL" | sed -n 's|redis://\([^:/]*\).*|\1|p')
        redis_port=$(echo "$REDIS_URL" | sed -n 's|.*:\([0-9]*\)$|\1|p')
        redis_port=${redis_port:-6379}

        if [ -n "$redis_host" ]; then
            wait_for_service "$redis_host" "$redis_port" 30 "Redis" || exit 1
        fi
    fi

    # Wait for additional services if WAIT_FOR_SERVICES is set
    # Format: "host1:port1,host2:port2"
    if [ -n "${WAIT_FOR_SERVICES:-}" ]; then
        IFS=',' read -ra services <<< "$WAIT_FOR_SERVICES"
        for service in "${services[@]}"; do
            local svc_host svc_port
            svc_host=$(echo "$service" | cut -d: -f1)
            svc_port=$(echo "$service" | cut -d: -f2)
            wait_for_service "$svc_host" "$svc_port" 30 || exit 1
        done
    fi
}

run_init_scripts() {
    local init_dir="/docker-entrypoint-init.d"

    if [ -d "$init_dir" ]; then
        log_info "Running initialization scripts from $init_dir..."

        for script in "$init_dir"/*; do
            if [ -f "$script" ] && [ -x "$script" ]; then
                log_info "Executing: $script"
                "$script" || {
                    log_error "Init script failed: $script"
                    exit 1
                }
            fi
        done

        log_success "All initialization scripts completed"
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# Main Execution
# ─────────────────────────────────────────────────────────────────────────────

main() {
    log_info "Starting Apex container entrypoint..."
    log_info "Container started at: $(date -Iseconds)"

    # Print environment info (non-sensitive)
    log_info "Environment: ${APEX_ENVIRONMENT:-development}"
    log_info "Log level: ${APEX_LOG_LEVEL:-INFO}"

    # Run pre-flight checks
    check_required_env
    check_dependencies
    run_init_scripts

    # Start health server if enabled
    start_health_server

    # Execute the main command
    if [ $# -eq 0 ]; then
        log_error "No command specified"
        exit 1
    fi

    log_info "Executing: $*"

    # Run the command in the background to properly handle signals
    "$@" &
    MAIN_PID=$!

    log_info "Main process started with PID: $MAIN_PID"

    # Wait for the main process
    wait "$MAIN_PID"
    EXIT_CODE=$?

    log_info "Main process exited with code: $EXIT_CODE"
    exit $EXIT_CODE
}

# Run main function with all arguments
main "$@"
