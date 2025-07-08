#!/bin/bash

# Stop Neural Trader containers running on external Docker

EXTERNAL_DOCKER_HOST="tcp://host.docker.internal:2375"
CONTEXT_DIR="/tmp/neural-trader-context"

# Allow user to override Docker host
if [ ! -z "$DOCKER_HOST_OVERRIDE" ]; then
    EXTERNAL_DOCKER_HOST="$DOCKER_HOST_OVERRIDE"
fi

echo "🛑 Stopping Neural Trader on External Docker"
echo "==========================================="
echo ""
echo "🔧 Using Docker host: $EXTERNAL_DOCKER_HOST"
echo ""

# Check if context directory exists
if [ ! -d "$CONTEXT_DIR" ]; then
    echo "⚠️  Context directory not found at $CONTEXT_DIR"
    echo "   Attempting to stop from current directory..."
    CONTEXT_DIR="."
fi

cd $CONTEXT_DIR

# Stop containers
echo "📦 Stopping all containers..."
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml down

# Stop dev tools if running
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml --profile dev-tools down 2>/dev/null

# Optional: Remove volumes
read -p "Would you like to remove data volumes as well? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🗑️  Removing volumes..."
    DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml down -v
fi

# Clean up context directory if not current directory
if [ "$CONTEXT_DIR" != "." ] && [ -d "$CONTEXT_DIR" ]; then
    echo "🧹 Cleaning up context directory..."
    rm -rf $CONTEXT_DIR
fi

echo ""
echo "✅ Neural Trader stopped successfully!"