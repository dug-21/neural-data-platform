#!/bin/bash
# Stop development environment
echo "Stopping Neural Trader V2 development environment..."
docker-compose -f docker-compose.v2.yml down
echo "Services stopped."
