#!/bin/bash
# Start the auto-commit background task

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PID_FILE="$SCRIPT_DIR/.auto-commit.pid"
LOG_FILE="$SCRIPT_DIR/.auto-commit.log"

# Check if already running
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if ps -p "$OLD_PID" > /dev/null 2>&1; then
        echo "Auto-commit task is already running (PID: $OLD_PID)"
        exit 1
    else
        rm -f "$PID_FILE"
    fi
fi

# Start background loop
(
    while true; do
        sleep 900  # 15 minutes = 900 seconds
        "$SCRIPT_DIR/auto-commit.sh" >> "$LOG_FILE" 2>&1
    done
) &

PID=$!
echo $PID > "$PID_FILE"

echo "Auto-commit task started (PID: $PID)"
echo "Log file: $LOG_FILE"
echo "To stop: kill $PID"
