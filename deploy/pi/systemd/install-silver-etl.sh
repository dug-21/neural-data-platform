#!/bin/bash
# install-silver-etl.sh - Install Silver ETL systemd timer
#
# Usage: sudo ./install-silver-etl.sh
#
# This script installs the silver-etl systemd service and timer units,
# enabling hourly Bronze to Silver ETL transformation.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_FILE="silver-etl.service"
TIMER_FILE="silver-etl.timer"
SYSTEMD_DIR="/etc/systemd/system"

echo "=== Silver ETL Timer Installation ==="
echo ""

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    echo "Error: This script must be run as root (use sudo)"
    exit 1
fi

# Verify source files exist
if [[ ! -f "${SCRIPT_DIR}/${SERVICE_FILE}" ]]; then
    echo "Error: ${SERVICE_FILE} not found in ${SCRIPT_DIR}"
    exit 1
fi

if [[ ! -f "${SCRIPT_DIR}/${TIMER_FILE}" ]]; then
    echo "Error: ${TIMER_FILE} not found in ${SCRIPT_DIR}"
    exit 1
fi

# Copy unit files to systemd directory
echo "Copying unit files to ${SYSTEMD_DIR}..."
cp "${SCRIPT_DIR}/${SERVICE_FILE}" "${SYSTEMD_DIR}/"
cp "${SCRIPT_DIR}/${TIMER_FILE}" "${SYSTEMD_DIR}/"

# Set correct permissions
chmod 644 "${SYSTEMD_DIR}/${SERVICE_FILE}"
chmod 644 "${SYSTEMD_DIR}/${TIMER_FILE}"

# Reload systemd daemon
echo "Reloading systemd daemon..."
systemctl daemon-reload

# Enable the timer (starts on boot)
echo "Enabling ${TIMER_FILE}..."
systemctl enable "${TIMER_FILE}"

# Start the timer
echo "Starting ${TIMER_FILE}..."
systemctl start "${TIMER_FILE}"

echo ""
echo "=== Installation Complete ==="
echo ""

# Verify timer is active
echo "Verifying timer status..."
systemctl status "${TIMER_FILE}" --no-pager || true

echo ""
echo "=== Next Scheduled Run ==="
systemctl list-timers "${TIMER_FILE}" --no-pager

echo ""
echo "=== Useful Commands ==="
echo "  View timer status:     systemctl status silver-etl.timer"
echo "  View service status:   systemctl status silver-etl.service"
echo "  View logs:             journalctl -u silver-etl.service -f"
echo "  Run ETL manually:      systemctl start silver-etl.service"
echo "  Stop timer:            systemctl stop silver-etl.timer"
echo "  Disable timer:         systemctl disable silver-etl.timer"
