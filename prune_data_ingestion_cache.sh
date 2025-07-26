#!/bin/bash
# Prune only data-ingestion related build cache

echo "Removing data-ingestion image and its build cache..."

# Stop and remove any data-ingestion containers
docker-compose -f docker-compose.prod.yml stop data-ingestion
docker-compose -f docker-compose.prod.yml rm -f data-ingestion

# Remove the data-ingestion image
docker images | grep "data-ingestion" | awk '{print $3}' | xargs -r docker rmi -f

# Remove any dangling images that might be related
docker image prune -f

# Clear buildx cache for recent builds (last hour)
docker buildx prune --filter "until=1h" -f

echo "Cache cleared. Now rebuilding data-ingestion without BuildKit..."

# Rebuild just data-ingestion without BuildKit
DOCKER_BUILDKIT=0 docker-compose -f docker-compose.prod.yml build data-ingestion

echo "Done! If successful, other services should still have their cache intact."