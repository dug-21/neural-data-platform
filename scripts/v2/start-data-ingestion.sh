#!/bin/bash
# Start data ingestion service for V2 development environment

set -e

echo "Starting data-ingestion service (V2 dev)..."

# In dev mode, we use synthetic data provider by default
# Real providers would be configured via config-store
DEFAULT_PROVIDER="${PRIMARY_PROVIDER:-synthetic}"
DEFAULT_SYMBOLS="${SYMBOLS:-AAPL,MSFT,GOOGL}"

# Wait for dependencies with timeout
wait_for_service() {
    local service=$1
    local port=$2
    local timeout=30
    
    echo "Waiting for $service on port $port..."
    while [ $timeout -gt 0 ]; do
        if nc -z $service $port 2>/dev/null; then
            echo "✓ $service is ready"
            return 0
        fi
        echo "  Waiting for $service... ($timeout seconds remaining)"
        sleep 2
        timeout=$((timeout - 2))
    done
    echo "✗ Timeout waiting for $service"
    return 1
}

# Check dependencies
wait_for_service redis 6379
wait_for_service timescaledb 5432

# Note: config-store might not be ready yet, but that's okay for dev
# The service should handle retries internally

echo "Dependencies ready. Starting Python data-ingestion service..."
echo "Provider: $DEFAULT_PROVIDER"
echo "Symbols: $DEFAULT_SYMBOLS"

# Convert comma-separated symbols to arguments
SYMBOL_ARGS=""
IFS=',' read -ra SYMBOL_ARRAY <<< "$DEFAULT_SYMBOLS"
for symbol in "${SYMBOL_ARRAY[@]}"; do
    SYMBOL_ARGS="$SYMBOL_ARGS --symbols $symbol"
done

# Start the service
cd /app
exec python -m data_ingestion.main start --providers $DEFAULT_PROVIDER $SYMBOL_ARGS "$@"