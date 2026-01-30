#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Devcontainer Post-Start Script
# Runs every time the container starts
# ══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

echo "═══════════════════════════════════════════════════════════════════════════"
echo "  Apex Development Container - Starting..."
echo "═══════════════════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────────────────────
# Wait for Services
# ─────────────────────────────────────────────────────────────────────────────

echo "[Services] Checking service availability..."

# Wait for PostgreSQL
echo -n "[Services] PostgreSQL..."
until pg_isready -h postgres -U apex -d apex -q 2>/dev/null; do
    echo -n "."
    sleep 1
done
echo " Ready!"

# Wait for Redis
echo -n "[Services] Redis..."
until redis-cli -h redis ping 2>/dev/null | grep -q PONG; do
    echo -n "."
    sleep 1
done
echo " Ready!"

# ─────────────────────────────────────────────────────────────────────────────
# Update Dependencies (if needed)
# ─────────────────────────────────────────────────────────────────────────────

cd /workspace

# Check if package.json has changed and update node_modules
if [ -f "src/frontend/package.json" ]; then
    cd src/frontend
    if [ "package.json" -nt "node_modules/.package-lock.json" ] 2>/dev/null; then
        echo "[Node.js] package.json changed, updating dependencies..."
        npm install --legacy-peer-deps 2>/dev/null || true
    fi
    cd /workspace
fi

# ─────────────────────────────────────────────────────────────────────────────
# Run Pending Migrations
# ─────────────────────────────────────────────────────────────────────────────

if command -v sqlx &>/dev/null; then
    cd /workspace/src/backend/core
    echo "[Database] Checking for pending migrations..."
    sqlx migrate run 2>/dev/null || true
    cd /workspace
fi

# ─────────────────────────────────────────────────────────────────────────────
# Environment Info
# ─────────────────────────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════════════════════════"
echo "  Development Environment Ready!"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""
echo "  Environment: ${APEX_ENVIRONMENT:-development}"
echo "  Log Level:   ${APEX_LOG_LEVEL:-DEBUG}"
echo ""
echo "  Service URLs:"
echo "    API:      http://localhost:8080"
echo "    Frontend: http://localhost:3000 (run: npm run dev)"
echo "    Jaeger:   http://localhost:16686"
echo "    Grafana:  http://localhost:3001"
echo ""
echo "═══════════════════════════════════════════════════════════════════════════"
