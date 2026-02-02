#!/bin/bash
# Stop the auto-commit background task

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PID_FILE="$SCRIPT_DIR/.auto-commit.pid"

if [ ! -f "$PID_FILE" ]; then
    echo "Auto-commit task is not running"
    exit 0
fi

PID=$(cat "$PID_FILE")

if ps -p "$PID" > /dev/null 2>&1; then
    kill "$PID"
    echo "Auto-commit task stopped (PID: $PID)"
else
    echo "Process $PID not found"
fi

rm -f "$PID_FILE"
