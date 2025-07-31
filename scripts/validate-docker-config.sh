#!/bin/bash
# Docker Configuration Validation Script
# Validates that Docker Compose configuration is correct for external mounts

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Docker Configuration Validation${NC}"
echo "================================"
echo

# Check if we're in the right directory
if [[ ! -f "docker-compose.yml" ]]; then
    echo -e "${RED}Error: docker-compose.yml not found. Run this script from the project root.${NC}"
    exit 1
fi

# Check if .env file exists
if [[ ! -f ".env" ]]; then
    echo -e "${YELLOW}Warning: .env file not found. Using defaults.${NC}"
    echo "Run 'cp .env.example .env' to create one."
    echo
fi

# Validate docker-compose syntax
echo -e "${BLUE}Validating Docker Compose syntax...${NC}"
if docker-compose config > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Docker Compose configuration is valid${NC}"
elif docker compose config > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Docker Compose configuration is valid${NC}"
else
    echo -e "${RED}✗ Docker Compose configuration is invalid${NC}"
    echo "Running docker-compose config to show errors:"
    docker-compose config 2>&1 || docker compose config 2>&1
    exit 1
fi

# Check environment variables for external mounts
echo -e "${BLUE}Checking external mount configuration...${NC}"

# Source .env file if it exists
if [[ -f ".env" ]]; then
    source .env
fi

# Check key variables
EXTERNAL_DATA_ENABLED=${EXTERNAL_DATA_ENABLED:-false}
EXTERNAL_MOUNT_PATH=${EXTERNAL_MOUNT_PATH:-/mnt/external}

echo "External data enabled: $EXTERNAL_DATA_ENABLED"
echo "External mount path: $EXTERNAL_MOUNT_PATH"

if [[ "$EXTERNAL_DATA_ENABLED" == "true" ]]; then
    echo -e "${BLUE}Validating external mount path...${NC}"
    
    if [[ -d "$EXTERNAL_MOUNT_PATH" ]]; then
        echo -e "${GREEN}✓ External mount path exists: $EXTERNAL_MOUNT_PATH${NC}"
        
        # Check if path is readable
        if [[ -r "$EXTERNAL_MOUNT_PATH" ]]; then
            echo -e "${GREEN}✓ External mount path is readable${NC}"
            
            # Show some contents
            echo "Sample contents:"
            ls -la "$EXTERNAL_MOUNT_PATH" | head -5
        else
            echo -e "${YELLOW}⚠ External mount path exists but is not readable${NC}"
            echo "You may need to adjust permissions"
        fi
    else
        echo -e "${YELLOW}⚠ External mount path does not exist: $EXTERNAL_MOUNT_PATH${NC}"
        echo "This is not necessarily an error if the drive will be mounted later"
    fi
else
    echo -e "${BLUE}External data is disabled${NC}"
fi

echo

# Test docker-compose configuration with dry run
echo -e "${BLUE}Testing service configuration...${NC}"

# Get the compose command
if command -v docker-compose &> /dev/null; then
    COMPOSE_CMD="docker-compose"
else
    COMPOSE_CMD="docker compose"
fi

# Test configuration for each service
echo "Testing main services configuration:"

services=("timescaledb" "redis" "data-ingestion" "neural-trader")

for service in "${services[@]}"; do
    echo -n "  $service: "
    if $COMPOSE_CMD config --services | grep -q "^$service$"; then
        echo -e "${GREEN}✓ configured${NC}"
    else
        echo -e "${RED}✗ not found${NC}"
    fi
done

echo

# Validate volume mounts
echo -e "${BLUE}Validating volume mounts...${NC}"

# Extract volume information
volumes_info=$($COMPOSE_CMD config --format json | python3 -c "
import json, sys
try:
    config = json.load(sys.stdin)
    services = config.get('services', {})
    for service_name, service_config in services.items():
        volumes = service_config.get('volumes', [])
        for volume in volumes:
            if isinstance(volume, str) and '/mnt/external-data' in volume:
                print(f'{service_name}: {volume}')
            elif isinstance(volume, dict) and volume.get('target') == '/mnt/external-data':
                print(f'{service_name}: {volume.get(\"source\", \"unknown\")} -> {volume.get(\"target\", \"unknown\")}')
except Exception as e:
    print(f'Error parsing config: {e}')
")

if [[ -n "$volumes_info" ]]; then
    echo "External volume mounts found:"
    echo "$volumes_info"
else
    echo "No external volume mounts configured (this is normal if external data is disabled)"
fi

echo

# Check Docker daemon access
echo -e "${BLUE}Checking Docker daemon access...${NC}"

if docker info > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Docker daemon is accessible${NC}"
    
    # Show Docker version info
    echo "Docker version: $(docker --version)"
    echo "Docker Compose version: $($COMPOSE_CMD version --short 2>/dev/null || echo 'Unknown')"
else
    echo -e "${RED}✗ Cannot access Docker daemon${NC}"
    echo "Make sure Docker is running and you have permission to access it"
    echo "Try: sudo usermod -aG docker \$USER && newgrp docker"
    exit 1
fi

echo

# Check system resources
echo -e "${BLUE}Checking system resources...${NC}"

# Check available disk space
available_space=$(df -h . | awk 'NR==2 {print $4}')
echo "Available disk space: $available_space"

# Check memory
if command -v free &> /dev/null; then
    total_mem=$(free -h | awk '/^Mem:/ {print $2}')
    echo "Total system memory: $total_mem"
fi

# Warn about resource requirements
echo -e "${YELLOW}Note: Neural Trader requires significant resources:${NC}"
echo "  - Recommended RAM: 8GB+ (16GB+ for full stack)"
echo "  - Recommended disk space: 10GB+ free"
echo "  - For external drives: ensure sufficient space for historical data"

echo

# Test network connectivity (if services are running)
echo -e "${BLUE}Checking if services are currently running...${NC}"

if $COMPOSE_CMD ps --services --filter "status=running" 2>/dev/null | grep -q .; then
    echo "Running services found:"
    $COMPOSE_CMD ps --format table
    
    echo
    echo -e "${BLUE}Testing service connectivity...${NC}"
    
    # Test database connection
    if $COMPOSE_CMD ps --services --filter "status=running" | grep -q "timescaledb"; then
        echo -n "TimescaleDB connection: "
        if docker exec neural_trader_timescaledb pg_isready -U ${POSTGRES_USER:-neural_trader} > /dev/null 2>&1; then
            echo -e "${GREEN}✓ healthy${NC}"
        else
            echo -e "${RED}✗ not responding${NC}"
        fi
    fi
    
    # Test Redis connection
    if $COMPOSE_CMD ps --services --filter "status=running" | grep -q "redis"; then
        echo -n "Redis connection: "
        if docker exec neural_trader_redis redis-cli ping 2>/dev/null | grep -q "PONG"; then
            echo -e "${GREEN}✓ healthy${NC}"
        else
            echo -e "${RED}✗ not responding${NC}"
        fi
    fi
    
else
    echo "No services are currently running"
    echo "Start services with: $COMPOSE_CMD up -d"
fi

echo

# Final summary
echo -e "${GREEN}Validation completed!${NC}"
echo
echo -e "${BLUE}Summary:${NC}"
echo "  - Docker Compose configuration: Valid"
echo "  - External data enabled: $EXTERNAL_DATA_ENABLED"
if [[ "$EXTERNAL_DATA_ENABLED" == "true" ]]; then
    echo "  - External mount path: $EXTERNAL_MOUNT_PATH"
fi
echo
echo -e "${BLUE}Next steps:${NC}"
if [[ "$EXTERNAL_DATA_ENABLED" == "true" ]]; then
    echo "1. Ensure your external drive is mounted at: $EXTERNAL_MOUNT_PATH"
    echo "2. Verify file permissions allow Docker to read the data"
    echo "3. Start services: $COMPOSE_CMD up -d"
    echo "4. Check external data access: docker exec neural_trader_data_ingestion ls -la /mnt/external-data"
else
    echo "1. To enable external data, set EXTERNAL_DATA_ENABLED=true in .env"
    echo "2. Set EXTERNAL_MOUNT_PATH to your external drive path"
    echo "3. Run this validation script again"
    echo "4. Start services: $COMPOSE_CMD up -d"
fi

echo
echo "For more help, see: docs/docker-external-mount-guide.md"