#!/bin/bash
# Raspberry Pi 5 Setup Script
# Prepares Pi 5 for running air-quality-app in production
# Usage: sudo ./scripts/setup-pi5.sh

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Please run as root (sudo)${NC}"
    exit 1
fi

# Check if running on ARM64
if [ "$(uname -m)" != "aarch64" ]; then
    echo -e "${YELLOW}Warning: This script is designed for ARM64 (Pi 5)${NC}"
    read -p "Continue anyway? (yes/no): " confirm
    if [ "$confirm" != "yes" ]; then
        exit 1
    fi
fi

echo -e "${GREEN}=== Raspberry Pi 5 Setup for Neural Data Platform ===${NC}"
echo ""

# Update system
echo -e "${BLUE}[1/8] Updating system...${NC}"
apt-get update
apt-get upgrade -y

# Install Docker if not present
if ! command -v docker &> /dev/null; then
    echo -e "${BLUE}[2/8] Installing Docker...${NC}"
    curl -fsSL https://get.docker.com -o get-docker.sh
    sh get-docker.sh
    rm get-docker.sh

    # Add pi user to docker group
    usermod -aG docker pi || true

    # Enable Docker service
    systemctl enable docker
    systemctl start docker
else
    echo -e "${GREEN}[2/8] Docker already installed${NC}"
fi

# Install Docker Compose
if ! docker compose version &> /dev/null; then
    echo -e "${BLUE}[3/8] Installing Docker Compose...${NC}"
    apt-get install -y docker-compose-plugin
else
    echo -e "${GREEN}[3/8] Docker Compose already installed${NC}"
fi

# Create directory structure
echo -e "${BLUE}[4/8] Creating directory structure...${NC}"
mkdir -p /opt/neural/{data/{air-quality,mosquitto},logs/mosquitto,models,config}
chown -R 1000:1000 /opt/neural

# Install system utilities
echo -e "${BLUE}[5/8] Installing utilities...${NC}"
apt-get install -y \
    curl \
    wget \
    git \
    vim \
    htop \
    iotop \
    mosquitto-clients \
    net-tools \
    jq

# Configure system limits
echo -e "${BLUE}[6/8] Configuring system limits...${NC}"
cat > /etc/security/limits.d/neural.conf <<EOF
# Neural Data Platform limits
*    soft    nofile    65536
*    hard    nofile    65536
*    soft    nproc     4096
*    hard    nproc     4096
EOF

# Enable memory cgroup
echo -e "${BLUE}[7/8] Enabling memory cgroup...${NC}"
if ! grep -q "cgroup_enable=memory" /boot/cmdline.txt; then
    cp /boot/cmdline.txt /boot/cmdline.txt.backup
    sed -i 's/$/ cgroup_enable=memory cgroup_memory=1/' /boot/cmdline.txt
    echo -e "${YELLOW}Note: Reboot required for cgroup changes${NC}"
fi

# Create systemd service for auto-start
echo -e "${BLUE}[8/8] Creating systemd service...${NC}"
cat > /etc/systemd/system/neural-air-quality.service <<EOF
[Unit]
Description=Neural Air Quality Application
Requires=docker.service
After=docker.service network-online.target
Wants=network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=/opt/neural
ExecStart=/usr/bin/docker compose -f /opt/neural/docker-compose.prod.yml up -d
ExecStop=/usr/bin/docker compose -f /opt/neural/docker-compose.prod.yml down
TimeoutStartSec=300
TimeoutStopSec=120
Restart=on-failure
RestartSec=30

[Install]
WantedBy=multi-user.target
EOF

# Don't enable service yet - user needs to configure first
# systemctl enable neural-air-quality

# Performance tuning for Pi 5
echo -e "${BLUE}Applying performance tuning...${NC}"
cat > /etc/sysctl.d/99-neural.conf <<EOF
# Neural Data Platform tuning for Pi 5
vm.swappiness=10
vm.vfs_cache_pressure=50
net.core.somaxconn=1024
net.ipv4.tcp_max_syn_backlog=2048
EOF
sysctl -p /etc/sysctl.d/99-neural.conf

# Create helper scripts
echo -e "${BLUE}Creating helper scripts...${NC}"

cat > /usr/local/bin/neural-status <<'EOF'
#!/bin/bash
echo "=== Neural Air Quality Status ==="
docker compose -f /opt/neural/docker-compose.prod.yml ps
echo ""
echo "=== Resource Usage ==="
docker stats --no-stream
echo ""
echo "=== System Info ==="
vcgencmd measure_temp
free -h
EOF
chmod +x /usr/local/bin/neural-status

cat > /usr/local/bin/neural-logs <<'EOF'
#!/bin/bash
docker compose -f /opt/neural/docker-compose.prod.yml logs -f "${1:-air-quality-app}"
EOF
chmod +x /usr/local/bin/neural-logs

cat > /usr/local/bin/neural-restart <<'EOF'
#!/bin/bash
docker compose -f /opt/neural/docker-compose.prod.yml restart air-quality-app
EOF
chmod +x /usr/local/bin/neural-restart

# Print summary
echo ""
echo -e "${GREEN}=== Setup Complete! ===${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Copy docker-compose.prod.yml to /opt/neural/"
echo "2. Copy config files to /opt/neural/config/"
echo "3. Copy mosquitto.conf to /opt/neural/mosquitto/config/"
echo "4. Login to GitHub Container Registry:"
echo "   docker login ghcr.io -u YOUR_USERNAME"
echo "5. Pull the image:"
echo "   docker pull ghcr.io/neural-data-platform/air-quality:latest"
echo "6. Start the service:"
echo "   sudo systemctl start neural-air-quality"
echo "7. Enable auto-start:"
echo "   sudo systemctl enable neural-air-quality"
echo ""
echo -e "${YELLOW}Helper commands:${NC}"
echo "  neural-status  - Show service status and resource usage"
echo "  neural-logs    - View application logs"
echo "  neural-restart - Restart application"
echo ""
echo -e "${YELLOW}Manual deployment:${NC}"
echo "  cd /opt/neural"
echo "  docker compose -f docker-compose.prod.yml up -d"
echo ""
echo -e "${RED}Important: Reboot required for cgroup changes!${NC}"
echo "  sudo reboot"
echo ""
