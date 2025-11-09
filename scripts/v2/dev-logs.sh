#!/bin/bash
# View development logs
SERVICE=${1:-}
if [ -z "$SERVICE" ]; then
    docker-compose -f docker-compose.v2.yml logs -f --tail=100
else
    docker-compose -f docker-compose.v2.yml logs -f --tail=100 "$SERVICE"
fi
