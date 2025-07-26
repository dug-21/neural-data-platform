#!/bin/bash
# Alternative fix for Docker unpacking issue

echo "Alternative solution: Clear builder cache and use legacy builder"

# Stop any running containers
docker-compose -f docker-compose.prod.yml down

# Clear the builder cache
docker builder prune -af

# Remove any dangling images
docker image prune -f

# Try building with the legacy builder explicitly
DOCKER_BUILDKIT=0 docker-compose -f docker-compose.prod.yml build --no-cache data-ingestion

echo "If this works, you can build other services with:"
echo "DOCKER_BUILDKIT=0 docker-compose -f docker-compose.prod.yml build"