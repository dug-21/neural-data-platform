#!/bin/bash
# Fix Docker unpacking issue - Solution 1: Disable buildkit

echo "Attempting build without BuildKit..."
cd /workspaces/neural-trader

# Disable BuildKit which can cause unpacking issues
export DOCKER_BUILDKIT=0

# Build with plain progress
docker build \
  --progress=plain \
  -f docker/production/images/neural-trader.dockerfile \
  -t neural-trader:prod \
  docker/production