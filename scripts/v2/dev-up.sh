#!/bin/bash
# Start development environment
echo "Starting Neural Trader V2 development environment..."
docker-compose -f docker-compose.v2.yml up -d
echo "Services started. View logs with: ./scripts/v2/dev-logs.sh"
