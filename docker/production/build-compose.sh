#!/bin/bash
# Build script using docker-compose

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}Building Neural Trader production images with docker-compose...${NC}"

cd "$SCRIPT_DIR"

# Build all images using docker-compose
echo -e "${YELLOW}Building all images in parallel...${NC}"
docker-compose -f docker-compose.build.yml build --parallel

echo -e "${GREEN}All images built successfully!${NC}"

# List built images
echo -e "${YELLOW}Built images:${NC}"
docker images | grep -E "(neural-trader|REPOSITORY)" | head -10

echo -e "${GREEN}Build complete!${NC}"
echo -e "To run: cd $SCRIPT_DIR && docker-compose -f docker-compose.prod.yml up -d"