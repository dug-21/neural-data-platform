#!/bin/bash
# Build docker-compose without BuildKit to avoid unpacking issues

echo "Building docker-compose.prod.yml without BuildKit..."

# Disable BuildKit
export DOCKER_BUILDKIT=0
export COMPOSE_DOCKER_CLI_BUILD=0

# Build all services
docker-compose -f docker-compose.prod.yml build --progress=plain

echo "Build complete. You can now run:"
echo "docker-compose -f docker-compose.prod.yml up -d"