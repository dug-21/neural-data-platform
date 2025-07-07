#!/bin/bash
# Generate systemd unit files for Neural Trader pods
# This creates user-level systemd units for automatic startup

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Base directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSTEMD_DIR="${SCRIPT_DIR}/../systemd"
USER_SYSTEMD_DIR="${HOME}/.config/systemd/user"

echo -e "${BLUE}Generating systemd unit files for Neural Trader...${NC}"

# Create directories
mkdir -p "${SYSTEMD_DIR}"
mkdir -p "${USER_SYSTEMD_DIR}"

# Check if pods are running
if ! podman pod exists neural-trader-db 2>/dev/null; then
    echo -e "${RED}Error: Pods are not running. Start them first with podman-up.sh${NC}"
    exit 1
fi

# Generate systemd units for each pod
echo -e "${BLUE}Generating pod units...${NC}"

for pod in neural-trader-db neural-trader-cache neural-trader-app neural-trader-monitoring; do
    echo -e "${BLUE}Generating unit for pod: ${pod}${NC}"
    
    # Generate the systemd unit
    podman generate systemd \
        --new \
        --name \
        --pod-prefix="" \
        --container-prefix="" \
        --separator="-" \
        --restart-policy=on-failure \
        --restart-sec=30 \
        "${pod}" > "${SYSTEMD_DIR}/podman-${pod}.service"
    
    # Add dependencies between services
    case "${pod}" in
        neural-trader-cache)
            # Cache depends on database
            sed -i '/\[Unit\]/a After=podman-neural-trader-db.service\nRequires=podman-neural-trader-db.service' \
                "${SYSTEMD_DIR}/podman-${pod}.service"
            ;;
        neural-trader-app)
            # App depends on database and cache
            sed -i '/\[Unit\]/a After=podman-neural-trader-db.service podman-neural-trader-cache.service\nRequires=podman-neural-trader-db.service podman-neural-trader-cache.service' \
                "${SYSTEMD_DIR}/podman-${pod}.service"
            ;;
        neural-trader-monitoring)
            # Monitoring depends on app
            sed -i '/\[Unit\]/a After=podman-neural-trader-app.service\nRequires=podman-neural-trader-app.service' \
                "${SYSTEMD_DIR}/podman-${pod}.service"
            ;;
    esac
done

# Create a master target unit
cat > "${SYSTEMD_DIR}/neural-trader.target" <<EOF
[Unit]
Description=Neural Trader Application Stack
Documentation=https://github.com/yourusername/neural-trader
Requires=podman-neural-trader-db.service podman-neural-trader-cache.service podman-neural-trader-app.service podman-neural-trader-monitoring.service
After=network-online.target

[Install]
WantedBy=default.target
EOF

# Create a helper script for managing the services
cat > "${SYSTEMD_DIR}/neural-trader-systemctl.sh" <<'EOF'
#!/bin/bash
# Helper script for managing Neural Trader systemd services

set -euo pipefail

ACTION=${1:-status}

case "$ACTION" in
    start)
        systemctl --user start neural-trader.target
        ;;
    stop)
        systemctl --user stop neural-trader.target
        ;;
    restart)
        systemctl --user restart neural-trader.target
        ;;
    status)
        systemctl --user status neural-trader.target
        echo
        systemctl --user status podman-neural-trader-*.service
        ;;
    enable)
        systemctl --user enable neural-trader.target
        ;;
    disable)
        systemctl --user disable neural-trader.target
        ;;
    logs)
        journalctl --user -u neural-trader.target -u podman-neural-trader-*.service -f
        ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|enable|disable|logs}"
        exit 1
        ;;
esac
EOF

chmod +x "${SYSTEMD_DIR}/neural-trader-systemctl.sh"

# Copy units to user systemd directory
echo -e "${BLUE}Installing systemd units...${NC}"
cp "${SYSTEMD_DIR}"/*.service "${USER_SYSTEMD_DIR}/"
cp "${SYSTEMD_DIR}"/*.target "${USER_SYSTEMD_DIR}/"

# Reload systemd
echo -e "${BLUE}Reloading systemd daemon...${NC}"
systemctl --user daemon-reload

# Show installation status
echo -e "${GREEN}Systemd units generated successfully!${NC}"
echo
echo -e "${BLUE}Available commands:${NC}"
echo "  Start all services:    systemctl --user start neural-trader.target"
echo "  Stop all services:     systemctl --user stop neural-trader.target"
echo "  Enable on boot:        systemctl --user enable neural-trader.target"
echo "  View status:           systemctl --user status neural-trader.target"
echo "  View logs:             journalctl --user -u neural-trader.target -f"
echo
echo -e "${BLUE}Or use the helper script:${NC}"
echo "  ${SYSTEMD_DIR}/neural-trader-systemctl.sh {start|stop|restart|status|enable|disable|logs}"
echo
echo -e "${YELLOW}Note: These are user-level systemd units. They will start when you log in.${NC}"
echo -e "${YELLOW}To start on system boot, you'll need to enable lingering:${NC}"
echo "  sudo loginctl enable-linger $USER"

# Create a rootless helper script
cat > "${SCRIPT_DIR}/enable-rootless-autostart.sh" <<'EOF'
#!/bin/bash
# Enable rootless container autostart for Neural Trader

set -euo pipefail

echo "Enabling user lingering for rootless container autostart..."
sudo loginctl enable-linger $USER

echo "Enabling Neural Trader services..."
systemctl --user enable neural-trader.target

echo "Starting Neural Trader services..."
systemctl --user start neural-trader.target

echo "Done! Neural Trader will now start automatically on system boot."
echo "Even when you're not logged in."
EOF

chmod +x "${SCRIPT_DIR}/enable-rootless-autostart.sh"

echo
echo -e "${GREEN}To enable automatic startup on system boot (rootless), run:${NC}"
echo "  ${SCRIPT_DIR}/enable-rootless-autostart.sh"