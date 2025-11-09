#!/bin/bash
# Restart specific service
SERVICE=$1
if [ -z "$SERVICE" ]; then
    echo "Usage: $0 <service-name>"
    exit 1
fi
docker-compose -f docker-compose.v2.yml restart "$SERVICE"
echo "$SERVICE restarted"
