#!/bin/bash
# Multi-architecture Docker build script
# Builds for linux/amd64 (Mac Intel, cloud) and linux/arm64 (Mac M-series, Pi 5)
# Usage: ./scripts/build-multiarch.sh [VERSION] [REGISTRY]

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
VERSION="${1:-latest}"
REGISTRY="${2:-ghcr.io/neural-data-platform}"
IMAGE_NAME="air-quality"
FULL_IMAGE="${REGISTRY}/${IMAGE_NAME}:${VERSION}"

echo -e "${GREEN}=== Multi-Architecture Docker Build ===${NC}"
echo "Version: ${VERSION}"
echo "Registry: ${REGISTRY}"
echo "Full image: ${FULL_IMAGE}"
echo ""

# Check if buildx is available
if ! docker buildx version &> /dev/null; then
    echo -e "${RED}Error: docker buildx is not available${NC}"
    echo "Please install Docker Desktop or enable buildx"
    exit 1
fi

# Check if builder exists, create if not
BUILDER_NAME="neural-multiarch-builder"
if ! docker buildx inspect "${BUILDER_NAME}" &> /dev/null; then
    echo -e "${YELLOW}Creating new buildx builder: ${BUILDER_NAME}${NC}"
    docker buildx create --name "${BUILDER_NAME}" --use --platform linux/amd64,linux/arm64
else
    echo -e "${GREEN}Using existing builder: ${BUILDER_NAME}${NC}"
    docker buildx use "${BUILDER_NAME}"
fi

# Ensure builder is running
docker buildx inspect --bootstrap

# Build for multiple architectures
echo -e "${GREEN}Building multi-arch image...${NC}"
docker buildx build \
    --platform linux/amd64,linux/arm64 \
    --tag "${FULL_IMAGE}" \
    --tag "${REGISTRY}/${IMAGE_NAME}:latest" \
    --build-arg BUILDKIT_INLINE_CACHE=1 \
    --cache-from "${REGISTRY}/${IMAGE_NAME}:latest" \
    --progress plain \
    --push \
    .

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Build successful!${NC}"
    echo ""
    echo "Images pushed:"
    echo "  - ${FULL_IMAGE}"
    echo "  - ${REGISTRY}/${IMAGE_NAME}:latest"
    echo ""
    echo "Supported platforms:"
    echo "  - linux/amd64 (Mac Intel, cloud servers)"
    echo "  - linux/arm64 (Mac M-series, Raspberry Pi 5)"
    echo ""
    echo "Pull command:"
    echo "  docker pull ${FULL_IMAGE}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi
