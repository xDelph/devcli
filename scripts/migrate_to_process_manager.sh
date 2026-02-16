#!/usr/bin/env bash
#
# Migration script to stop all apps managed by the old ProcessTracker
# and prepare for the new process-manager system.
#
# This script will:
# 1. Read all PID files from ~/.devcli/pids/
# 2. Stop each running process
# 3. Archive the old PID files
# 4. Clean up the directory
#
# Usage: ./migrate_to_process_manager.sh [--dry-run]

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PID_DIR="$HOME/.devcli/pids"
BACKUP_DIR="$HOME/.devcli/pids_backup_$(date +%Y%m%d_%H%M%S)"
DRY_RUN=false

# Parse arguments
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    echo -e "${YELLOW}Running in DRY-RUN mode - no changes will be made${NC}"
fi

# Check if PID directory exists
if [[ ! -d "$PID_DIR" ]]; then
    echo -e "${YELLOW}No PID directory found at $PID_DIR${NC}"
    echo "Nothing to migrate!"
    exit 0
fi

# Count JSON files
JSON_COUNT=$(find "$PID_DIR" -name "*.json" -type f 2>/dev/null | wc -l | tr -d ' ')

if [[ "$JSON_COUNT" -eq 0 ]]; then
    echo -e "${YELLOW}No PID files found in $PID_DIR${NC}"
    echo "Nothing to migrate!"
    exit 0
fi

echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  Process Manager Migration Script             ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}Found $JSON_COUNT PID file(s) to process${NC}"
echo ""

# Function to safely stop a process
stop_process() {
    local pid=$1
    local app_name=$2

    # Check if process is running
    if ! kill -0 "$pid" 2>/dev/null; then
        echo -e "  ${YELLOW}└─ Process $pid is not running (skipping)${NC}"
        return 0
    fi

    if [[ "$DRY_RUN" == true ]]; then
        echo -e "  ${YELLOW}└─ [DRY-RUN] Would stop process $pid${NC}"
        return 0
    fi

    # Try graceful shutdown first (SIGTERM)
    echo -e "  ${BLUE}└─ Sending SIGTERM to process $pid...${NC}"
    kill -15 "$pid" 2>/dev/null || true

    # Wait up to 5 seconds for graceful shutdown
    for i in {1..5}; do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo -e "  ${GREEN}└─ Process stopped gracefully${NC}"
            return 0
        fi
        sleep 1
    done

    # If still running, force kill
    if kill -0 "$pid" 2>/dev/null; then
        echo -e "  ${YELLOW}└─ Process still running, sending SIGKILL...${NC}"
        kill -9 "$pid" 2>/dev/null || true
        sleep 1

        if ! kill -0 "$pid" 2>/dev/null; then
            echo -e "  ${GREEN}└─ Process force stopped${NC}"
        else
            echo -e "  ${RED}└─ Failed to stop process $pid${NC}"
            return 1
        fi
    fi
}

# Process each PID file
STOPPED_COUNT=0
FAILED_COUNT=0

echo -e "${BLUE}Processing PID files...${NC}"
echo ""

while IFS= read -r -d '' pid_file; do
    filename=$(basename "$pid_file")

    # Skip the status notification file
    if [[ "$filename" == ".status_changed" ]]; then
        continue
    fi

    # Skip the monitor.json file
    if [[ "$filename" == "monitor.json" ]]; then
        echo -e "${BLUE}Skipping monitor daemon file: $filename${NC}"
        continue
    fi

    echo -e "${BLUE}Processing: $filename${NC}"

    # Extract PID and app name from JSON
    if ! pid=$(jq -r '.pid' "$pid_file" 2>/dev/null); then
        echo -e "  ${RED}└─ Failed to parse JSON${NC}"
        FAILED_COUNT=$((FAILED_COUNT + 1))
        continue
    fi

    app_name=$(jq -r '.app_name // "unknown"' "$pid_file" 2>/dev/null)
    project=$(jq -r '.project // "unknown"' "$pid_file" 2>/dev/null)

    echo -e "  ${BLUE}├─ App: $app_name (Project: $project)${NC}"
    echo -e "  ${BLUE}├─ PID: $pid${NC}"

    # Stop the process
    if stop_process "$pid" "$app_name"; then
        STOPPED_COUNT=$((STOPPED_COUNT + 1))
    else
        FAILED_COUNT=$((FAILED_COUNT + 1))
    fi

    echo ""
done < <(find "$PID_DIR" -name "*.json" -type f -print0)

# Create backup and clean up
if [[ "$DRY_RUN" == true ]]; then
    echo -e "${YELLOW}[DRY-RUN] Would create backup at: $BACKUP_DIR${NC}"
    echo -e "${YELLOW}[DRY-RUN] Would remove old PID files${NC}"
else
    echo -e "${BLUE}Creating backup of PID files...${NC}"
    mkdir -p "$BACKUP_DIR"
    cp -r "$PID_DIR"/* "$BACKUP_DIR/" 2>/dev/null || true
    echo -e "${GREEN}Backup created at: $BACKUP_DIR${NC}"
    echo ""

    echo -e "${BLUE}Cleaning up old PID files...${NC}"
    rm -f "$PID_DIR"/*.json
    echo -e "${GREEN}Old PID files removed${NC}"
fi

echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  Migration Summary                             ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}✓ Processes stopped: $STOPPED_COUNT${NC}"
if [[ "$FAILED_COUNT" -gt 0 ]]; then
    echo -e "${RED}✗ Failed operations: $FAILED_COUNT${NC}"
fi
echo ""

if [[ "$DRY_RUN" == true ]]; then
    echo -e "${YELLOW}This was a DRY-RUN. No changes were made.${NC}"
    echo -e "${YELLOW}Run without --dry-run to apply changes.${NC}"
else
    echo -e "${GREEN}Migration complete!${NC}"
    echo ""
    echo -e "${BLUE}Next steps:${NC}"
    echo "  1. Update devcli-core to use the process-manager crate"
    echo "  2. Rebuild and test the new system"
    echo "  3. Start your applications with the new process manager"
    echo ""
    echo -e "${YELLOW}Note: Backup of old PID files saved to:${NC}"
    echo "  $BACKUP_DIR"
fi
