#!/bin/bash
# ══════════════════════════════════════════════════════════════════════════════
# Project Apex - Database Migration Script
# ══════════════════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
DATABASE_URL="${DATABASE_URL:-postgres://apex:apex_secret@localhost:5432/apex}"

echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}              Project Apex - Database Migrations                ${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
MIGRATIONS_DIR="$PROJECT_ROOT/src/backend/core/migrations"

# Check if sqlx-cli is installed
if ! command -v sqlx &> /dev/null; then
    echo -e "${YELLOW}Installing sqlx-cli...${NC}"
    cargo install sqlx-cli --no-default-features --features postgres
fi

# Parse command
COMMAND="${1:-run}"

case "$COMMAND" in
    run)
        echo -e "${YELLOW}▶ Running pending migrations...${NC}"
        echo ""
        cd "$PROJECT_ROOT/src/backend/core"
        sqlx migrate run --database-url "$DATABASE_URL"
        echo ""
        echo -e "${GREEN}✓ Migrations complete${NC}"
        ;;

    revert)
        echo -e "${YELLOW}▶ Reverting last migration...${NC}"
        echo ""
        cd "$PROJECT_ROOT/src/backend/core"
        sqlx migrate revert --database-url "$DATABASE_URL"
        echo ""
        echo -e "${GREEN}✓ Migration reverted${NC}"
        ;;

    status)
        echo -e "${YELLOW}▶ Migration status:${NC}"
        echo ""
        cd "$PROJECT_ROOT/src/backend/core"
        sqlx migrate info --database-url "$DATABASE_URL"
        ;;

    create)
        if [ -z "$2" ]; then
            echo -e "${RED}Error: Migration name required${NC}"
            echo "Usage: $0 create <migration_name>"
            exit 1
        fi
        echo -e "${YELLOW}▶ Creating new migration: $2${NC}"
        echo ""
        cd "$PROJECT_ROOT/src/backend/core"
        sqlx migrate add "$2"
        echo ""
        echo -e "${GREEN}✓ Migration created in $MIGRATIONS_DIR${NC}"
        ;;

    prepare)
        echo -e "${YELLOW}▶ Preparing offline query data...${NC}"
        echo ""
        cd "$PROJECT_ROOT/src/backend/core"
        cargo sqlx prepare --database-url "$DATABASE_URL"
        echo ""
        echo -e "${GREEN}✓ SQLx offline data prepared in .sqlx/${NC}"
        ;;

    reset)
        echo -e "${RED}⚠ WARNING: This will drop and recreate the database!${NC}"
        read -p "Are you sure? (y/N) " -n 1 -r
        echo ""
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${YELLOW}▶ Resetting database...${NC}"
            cd "$PROJECT_ROOT/src/backend/core"
            sqlx database drop --database-url "$DATABASE_URL" -y || true
            sqlx database create --database-url "$DATABASE_URL"
            sqlx migrate run --database-url "$DATABASE_URL"
            echo ""
            echo -e "${GREEN}✓ Database reset complete${NC}"
        else
            echo "Cancelled."
        fi
        ;;

    *)
        echo "Usage: $0 {run|revert|status|create|prepare|reset}"
        echo ""
        echo "Commands:"
        echo "  run      - Run all pending migrations"
        echo "  revert   - Revert the last migration"
        echo "  status   - Show migration status"
        echo "  create   - Create a new migration file"
        echo "  prepare  - Prepare SQLx offline query data"
        echo "  reset    - Drop and recreate database (DESTRUCTIVE)"
        exit 1
        ;;
esac
