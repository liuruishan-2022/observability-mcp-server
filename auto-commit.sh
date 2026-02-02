#!/bin/bash
# Auto-commit script for manual changes
# Runs every 15 minutes to commit any manual changes

set -e

# Change to script directory
cd "$(dirname "$0")"

# Check if there are any changes
if git diff --quiet && git diff --cached --quiet; then
    echo "No changes to commit"
    exit 0
fi

# Add all changes
git add -A

# Commit with a timestamp message
COMMIT_MSG="Auto-commit: Manual changes at $(date '+%Y-%m-%d %H:%M:%S')

This is an automatic commit for manual changes made by the user.
Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"

git commit -m "$COMMIT_MSG"

echo "Auto-commit completed at $(date '+%Y-%m-%d %H:%M:%S')"
