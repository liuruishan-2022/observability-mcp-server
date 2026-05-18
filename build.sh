#!/bin/bash
# Build script for observability-mcp-server Docker image
# Version: v0.1.0
# Tag format: v0.1.0-<commit_hash>

set -e

# Configuration
VERSION="v0.1.0"
IMAGE_NAME="observability-mcp-server"
REGISTRY="xwharbor.wxchina.com/cpaas/component/"
DOCKERFILE="./Dockerfile"

# Get the short commit hash (first 10 characters)
if git rev-parse --git-dir > /dev/null 2>&1; then
    COMMIT_HASH=$(git rev-parse --short=10 HEAD 2>/dev/null || echo "unknown")
else
    COMMIT_HASH="unknown"
fi

# Construct the full image tag
FULL_TAG="${VERSION}-${COMMIT_HASH}"
FULL_IMAGE="${REGISTRY}${IMAGE_NAME}:${FULL_TAG}"

echo "=================================="
echo "Building observability-mcp-server"
echo "=================================="
echo "Version: ${VERSION}"
echo "Commit Hash: ${COMMIT_HASH}"
echo "Image Tag: ${FULL_IMAGE}"
echo "=================================="

# Step 1: Build the Rust binary
echo ""
echo "Step 1: Building Rust binary..."
cargo build --release -p observability-mcp-tools

# Step 2: Build Docker image
echo ""
echo "Step 2: Building Docker image..."
docker build \
    -t "${FULL_IMAGE}" \
    -f "${DOCKERFILE}" .

# Step 3: Push Docker image
echo ""
echo "Step 3: Pushing Docker image..."
docker push "${FULL_IMAGE}"

echo ""
echo "=================================="
echo "Build Complete!"
echo "=================================="
echo ""
echo "Image Tag:"
echo "  ${FULL_IMAGE}"
echo ""
echo "To run the container:"
echo "  docker run -d -p 3013:3013 --env-file .env ${FULL_IMAGE}"
echo "=================================="

echo ""
echo "Done!"
