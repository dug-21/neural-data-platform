#!/bin/bash
# Clean up Docker resources and rebuild

echo "Cleaning up Docker resources..."

# Stop any running containers (except the dev container we're in)
docker ps -q | grep -v $(hostname) | xargs -r docker stop

# Remove stopped containers
docker container prune -f

# Remove unused images
docker image prune -a -f

# Clean build cache
docker builder prune -f

# Remove unused volumes
docker volume prune -f

echo "Docker cleanup complete. Disk usage after cleanup:"
docker system df

echo ""
echo "Attempting fresh build..."

# Try building with reduced parallelism and more verbose output
cd /workspaces/neural-trader
DOCKER_BUILDKIT=1 docker build \
  --progress=plain \
  --no-cache \
  -f docker/neural-trader.dockerfile \
  -t neural-trader:prod \
  .