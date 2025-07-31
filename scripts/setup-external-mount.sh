#!/bin/bash
# Neural Trader External Drive Mount Setup Script
# This script helps configure external drive mounting for Docker containers

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ENV_FILE=".env"
COMPOSE_FILE="docker-compose.yml"
DOCS_URL="https://github.com/your-repo/neural-trader/blob/main/docs/docker-external-mount-guide.md"

echo -e "${BLUE}Neural Trader External Drive Mount Setup${NC}"
echo "======================================="
echo

# Check if running as root (not recommended)
if [[ $EUID -eq 0 ]]; then
    echo -e "${YELLOW}Warning: Running as root is not recommended for Docker operations${NC}"
    echo "Consider adding your user to the docker group instead:"
    echo "  sudo usermod -aG docker \$USER"
    echo
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check prerequisites
echo -e "${BLUE}Checking prerequisites...${NC}"

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Error: Docker is not installed${NC}"
    exit 1
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    echo -e "${RED}Error: Docker Compose is not installed${NC}"
    exit 1
fi

# Check if we're in the right directory
if [[ ! -f "$COMPOSE_FILE" ]]; then
    echo -e "${RED}Error: docker-compose.yml not found. Are you in the project root?${NC}"
    exit 1
fi

echo -e "${GREEN}Prerequisites check passed${NC}"
echo

# Function to detect platform
detect_platform() {
    case "$(uname -s)" in
        Linux*)     echo "linux";;
        Darwin*)    echo "macos";;
        CYGWIN*|MINGW*|MSYS*) echo "windows";;
        *)          echo "unknown";;
    esac
}

PLATFORM=$(detect_platform)
echo -e "${BLUE}Detected platform: $PLATFORM${NC}"
echo

# Function to list available drives/mount points
list_available_drives() {
    case $PLATFORM in
        linux)
            echo -e "${BLUE}Available mount points:${NC}"
            lsblk -f | grep -E "(ext[2-4]|ntfs|vfat|xfs)" || echo "No suitable drives found"
            echo
            echo -e "${BLUE}Currently mounted drives:${NC}"
            df -h | grep -E "^/dev/" | grep -v "tmpfs"
            echo
            echo -e "${BLUE}Common locations:${NC}"
            echo "  /media/\$USER/drive-name (auto-mounted USB drives)"
            echo "  /mnt/drive-name (manually mounted drives)"
            ;;
        macos)
            echo -e "${BLUE}Available volumes:${NC}"
            ls -la /Volumes/ 2>/dev/null || echo "No volumes found in /Volumes/"
            echo
            echo -e "${BLUE}Disk utility info:${NC}"
            diskutil list | grep -E "(external|USB)"
            ;;
        windows)
            echo -e "${BLUE}Available drives in WSL2:${NC}"
            ls -la /mnt/ 2>/dev/null || echo "No Windows drives found"
            echo
            echo -e "${BLUE}Note:${NC} Windows drives are typically mounted under /mnt/"
            echo "  Example: /mnt/d for D: drive"
            ;;
    esac
}

# Interactive setup
echo -e "${YELLOW}Interactive Setup${NC}"
echo "=================="
echo

# Ask if user wants to see available drives
read -p "Do you want to see available drives/mount points? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    list_available_drives
    echo
fi

# Get external mount path
while true; do
    echo -e "${BLUE}Enter the host path to your external drive:${NC}"
    case $PLATFORM in
        linux)
            echo "  Examples: /media/\$USER/TradingData or /mnt/external-drive"
            ;;
        macos)
            echo "  Examples: /Volumes/TradingData or /Volumes/USB-Drive"
            ;;
        windows)
            echo "  Examples: /mnt/d/TradingData or /mnt/e/data"
            ;;
    esac
    
    read -p "Path: " EXTERNAL_PATH
    
    # Validate path
    if [[ -z "$EXTERNAL_PATH" ]]; then
        echo -e "${RED}Error: Path cannot be empty${NC}"
        continue
    fi
    
    if [[ ! -d "$EXTERNAL_PATH" ]]; then
        echo -e "${YELLOW}Warning: Path '$EXTERNAL_PATH' does not exist${NC}"
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            continue
        fi
    else
        # Check if path is readable
        if [[ ! -r "$EXTERNAL_PATH" ]]; then
            echo -e "${YELLOW}Warning: Path '$EXTERNAL_PATH' is not readable${NC}"
            echo "You may need to adjust permissions or run with appropriate privileges"
        else
            echo -e "${GREEN}Path verified: $EXTERNAL_PATH${NC}"
        fi
    fi
    
    break
done

echo

# Get mount mode
echo -e "${BLUE}Select mount mode:${NC}"
echo "1) Read-only (recommended for safety)"
echo "2) Read-write (use with caution)"
read -p "Choice (1-2): " -n 1 -r
echo

case $REPLY in
    1|"")
        MOUNT_MODE="ro"
        echo -e "${GREEN}Selected: Read-only${NC}"
        ;;
    2)
        MOUNT_MODE="rw"
        echo -e "${YELLOW}Selected: Read-write (use with caution)${NC}"
        ;;
    *)
        echo -e "${YELLOW}Invalid choice, defaulting to read-only${NC}"
        MOUNT_MODE="ro"
        ;;
esac

echo

# Update .env file
echo -e "${BLUE}Updating .env file...${NC}"

# Create .env if it doesn't exist
if [[ ! -f "$ENV_FILE" ]]; then
    if [[ -f ".env.example" ]]; then
        cp ".env.example" "$ENV_FILE"
        echo -e "${GREEN}Created .env from .env.example${NC}"
    else
        touch "$ENV_FILE"
        echo -e "${GREEN}Created new .env file${NC}"
    fi
fi

# Update or add configuration
update_env_var() {
    local var_name="$1"
    local var_value="$2"
    local env_file="$3"
    
    if grep -q "^${var_name}=" "$env_file"; then
        # Update existing variable
        if [[ "$PLATFORM" == "macos" ]]; then
            sed -i '' "s|^${var_name}=.*|${var_name}=${var_value}|" "$env_file"
        else
            sed -i "s|^${var_name}=.*|${var_name}=${var_value}|" "$env_file"
        fi
    else
        # Add new variable
        echo "${var_name}=${var_value}" >> "$env_file"
    fi
}

# Update environment variables
update_env_var "EXTERNAL_DATA_ENABLED" "true" "$ENV_FILE"
update_env_var "EXTERNAL_MOUNT_PATH" "$EXTERNAL_PATH" "$ENV_FILE"
update_env_var "EXTERNAL_DATA_PATH" "/mnt/external-data" "$ENV_FILE"
update_env_var "EXTERNAL_MOUNT_MODE" "$MOUNT_MODE" "$ENV_FILE"

echo -e "${GREEN}Environment configuration updated${NC}"
echo

# Test configuration
echo -e "${BLUE}Testing configuration...${NC}"

# Check if docker-compose config is valid
if docker-compose config > /dev/null 2>&1 || docker compose config > /dev/null 2>&1; then
    echo -e "${GREEN}Docker Compose configuration is valid${NC}"
else
    echo -e "${RED}Error: Docker Compose configuration is invalid${NC}"
    echo "Please check your docker-compose.yml file"
    exit 1
fi

echo

# Display summary
echo -e "${GREEN}Setup Complete!${NC}"
echo "==============="
echo
echo -e "${BLUE}Configuration Summary:${NC}"
echo "  External Data Enabled: true"
echo "  Host Path: $EXTERNAL_PATH"
echo "  Container Path: /mnt/external-data"
echo "  Mount Mode: $MOUNT_MODE"
echo
echo -e "${BLUE}Next Steps:${NC}"
echo "1. Verify your external drive is mounted at: $EXTERNAL_PATH"
echo "2. Start the services with: docker-compose up -d"
echo "3. Verify the mount inside containers:"
echo "   docker exec neural_trader_data_ingestion ls -la /mnt/external-data"
echo
echo -e "${BLUE}Troubleshooting:${NC}"
echo "- If you encounter permission issues, see the troubleshooting guide:"
echo "  ${DOCS_URL}"
echo "- Check container logs: docker-compose logs -f"
echo

# Optional: Offer to start services
read -p "Do you want to start the services now? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${BLUE}Starting services...${NC}"
    if command -v docker-compose &> /dev/null; then
        docker-compose up -d
    else
        docker compose up -d
    fi
    
    echo
    echo -e "${GREEN}Services started!${NC}"
    echo "Check status with: docker-compose ps"
    echo "View logs with: docker-compose logs -f"
fi

echo
echo -e "${GREEN}Setup script completed successfully!${NC}"