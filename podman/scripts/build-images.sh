#!/bin/bash
# Build container images for Neural Trader
# This script builds all necessary images using Podman

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Base directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo -e "${BLUE}Building container images for Neural Trader...${NC}"

# Function to build an image
build_image() {
    local image_name=$1
    local dockerfile=$2
    local context=${3:-${PROJECT_ROOT}}
    local extra_args=${4:-}
    
    echo -e "${BLUE}Building ${image_name}...${NC}"
    
    # Check if Dockerfile exists
    if [[ ! -f "${dockerfile}" ]]; then
        echo -e "${RED}Error: Dockerfile not found: ${dockerfile}${NC}"
        return 1
    fi
    
    # Build the image
    if podman build \
        --tag "localhost/${image_name}:latest" \
        --file "${dockerfile}" \
        ${extra_args} \
        "${context}"; then
        echo -e "${GREEN}Successfully built ${image_name}${NC}"
    else
        echo -e "${RED}Failed to build ${image_name}${NC}"
        return 1
    fi
}

# Build TimescaleDB image
build_image \
    "neural-trader-timescaledb" \
    "${PROJECT_ROOT}/docker/timescaledb/Dockerfile" \
    "${PROJECT_ROOT}" \
    "--build-arg POSTGRES_VERSION=15"

# Build Redis image
build_image \
    "neural-trader-redis" \
    "${PROJECT_ROOT}/docker/redis/Dockerfile" \
    "${PROJECT_ROOT}"

# Build Data Ingestion image
build_image \
    "neural-trader-data-ingestion" \
    "${PROJECT_ROOT}/docker/data-ingestion/Dockerfile" \
    "${PROJECT_ROOT}"

# Build main Neural Trader image
build_image \
    "neural-trader" \
    "${PROJECT_ROOT}/Dockerfile" \
    "${PROJECT_ROOT}" \
    "--target production"

# List built images
echo -e "${BLUE}Built images:${NC}"
podman images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.Created}}" | \
    grep -E "(REPOSITORY|localhost/neural-trader)" || echo "No images found"

echo -e "${GREEN}All images built successfully!${NC}"

# Optional: Save images for distribution
if [[ "${1:-}" == "--save" ]]; then
    echo -e "${BLUE}Saving images to tar files...${NC}"
    mkdir -p "${PROJECT_ROOT}/podman/images"
    
    for image in neural-trader-timescaledb neural-trader-redis neural-trader-data-ingestion neural-trader; do
        echo -e "${BLUE}Saving ${image}...${NC}"
        podman save \
            --output "${PROJECT_ROOT}/podman/images/${image}.tar" \
            "localhost/${image}:latest"
    done
    
    echo -e "${GREEN}Images saved to ${PROJECT_ROOT}/podman/images/${NC}"
fi