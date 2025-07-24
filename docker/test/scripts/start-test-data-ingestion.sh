#!/bin/bash
set -e

echo "Starting Neural Trader Data Ingestion - Test Mode"
echo "================================================"

# Wait for dependencies
echo "Waiting for database to be ready..."
while ! pg_isready -h ${TIMESCALE_HOST:-timescaledb-test} -p ${TIMESCALE_PORT:-5432} -U ${TIMESCALE_USER:-test_user} -d ${TIMESCALE_DATABASE:-neural_trader_test}; do
    echo "Waiting for TimescaleDB..."
    sleep 2
done

echo "Waiting for Redis to be ready..."
while ! redis-cli -h ${REDIS_HOST:-redis-test} -p ${REDIS_PORT:-6379} ping > /dev/null 2>&1; do
    echo "Waiting for Redis..."
    sleep 2
done

echo "Dependencies are ready!"

# Set test-specific environment variables
export TESTING_MODE=true
export MOCK_DATA_ENABLED=true
export LOG_LEVEL=DEBUG
export PYTHONPATH=/app

# Parse symbols from environment or use defaults
export SYMBOLS=${SYMBOLS:-"AAPL,MSFT,GOOGL"}
echo "Trading symbols: $SYMBOLS"

# Set update interval for testing (faster than production)
export UPDATE_INTERVAL=${UPDATE_INTERVAL:-5}
echo "Update interval: ${UPDATE_INTERVAL}s"

# Enable test metrics and monitoring
export METRICS_ENABLED=true
export METRICS_PORT=9090
export HEALTH_CHECK_PORT=8001

# Run database initialization if needed
echo "Initializing test database..."
python3 -c "
import sys
sys.path.append('/app')
from database.init import initialize_test_database
initialize_test_database()
" || echo "Database initialization skipped or failed (continuing anyway)"

# Start the data ingestion service with test configuration
echo "Starting data ingestion service in test mode..."
exec python3 -m main \
    --config test_config.py \
    --symbols "$SYMBOLS" \
    --update-interval "$UPDATE_INTERVAL" \
    --test-mode \
    --mock-providers \
    --log-level DEBUG