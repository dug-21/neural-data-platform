#!/bin/bash
# Custom entrypoint for Grafana that ensures dashboards are copied

# Copy dashboards from build-time location to volume
if [ -d "/tmp/dashboards" ]; then
    echo "Copying dashboards to volume..."
    mkdir -p /var/lib/grafana/dashboards
    cp -f /tmp/dashboards/*.json /var/lib/grafana/dashboards/ 2>/dev/null || true
    # Fix ownership - Grafana runs as UID 472
    chown -R 472:472 /var/lib/grafana/dashboards
    echo "Dashboards copied successfully"
fi

# Call the original Grafana entrypoint
exec /run.sh "$@"