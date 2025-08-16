#!/bin/bash
set -euo pipefail

# Data Ingestion Service Entrypoint Script
# Handles initialization, configuration, and graceful shutdown

# Color codes for logging
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

# Default configuration
DEFAULT_CONFIG_PATH="/opt/data-ingestion/config"
DEFAULT_LOG_PATH="/var/log/data-ingestion"
DEFAULT_DATA_PATH="/opt/data-ingestion/data"
DEFAULT_CACHE_PATH="/opt/data-ingestion/cache"

# Environment variables with defaults
CONFIG_PATH="${CONFIG_PATH:-$DEFAULT_CONFIG_PATH}"
LOG_PATH="${LOG_PATH:-$DEFAULT_LOG_PATH}"
DATA_PATH="${DATA_PATH:-$DEFAULT_DATA_PATH}"
CACHE_PATH="${CACHE_PATH:-$DEFAULT_CACHE_PATH}"
LOG_LEVEL="${LOG_LEVEL:-info}"
API_PORT="${API_PORT:-8001}"
METRICS_PORT="${METRICS_PORT:-9091}"

# Service configuration
PRIMARY_PROVIDER="${PRIMARY_PROVIDER:-alpaca}"
UPDATE_INTERVAL="${UPDATE_INTERVAL:-60s}"
SYMBOLS="${SYMBOLS:-AAPL,MSFT,GOOGL,AMZN,NVDA}"

# Signal handlers for graceful shutdown
shutdown_handler() {
    log_info "Received shutdown signal, initiating graceful shutdown..."
    
    if [ -n "${DATA_INGESTION_PID:-}" ]; then
        log_info "Sending SIGTERM to data-ingestion process (PID: $DATA_INGESTION_PID)"
        kill -TERM "$DATA_INGESTION_PID" 2>/dev/null || true
        
        # Wait for graceful shutdown with timeout
        local timeout=30
        local count=0
        while kill -0 "$DATA_INGESTION_PID" 2>/dev/null && [ $count -lt $timeout ]; do
            sleep 1
            count=$((count + 1))
            if [ $((count % 5)) -eq 0 ]; then
                log_info "Waiting for graceful shutdown... ($count/$timeout seconds)"
            fi
        done
        
        if kill -0 "$DATA_INGESTION_PID" 2>/dev/null; then
            log_warn "Process did not shut down gracefully, sending SIGKILL"
            kill -KILL "$DATA_INGESTION_PID" 2>/dev/null || true
        else
            log_success "Data ingestion service shut down gracefully"
        fi
    fi
    
    exit 0
}

# Set up signal traps
trap shutdown_handler SIGTERM SIGINT

# Validation functions
validate_environment() {
    log_info "Validating environment configuration..."
    
    # Check required environment variables
    local required_vars=(
        "DATABASE_URL"
        "REDIS_URL"
    )
    
    for var in "${required_vars[@]}"; do
        if [ -z "${!var:-}" ]; then
            log_error "Required environment variable $var is not set"
            exit 1
        fi
    done
    
    # Validate API keys based on primary provider
    case "$PRIMARY_PROVIDER" in
        "alpaca")
            if [ -z "${ALPACA_API_KEY:-}" ] || [ -z "${ALPACA_API_SECRET:-}" ]; then
                log_error "Alpaca API credentials not configured"
                exit 1
            fi
            ;;
        "polygon")
            if [ -z "${POLYGON_API_KEY:-}" ]; then
                log_error "Polygon API key not configured"
                exit 1
            fi
            ;;
        "finnhub")
            if [ -z "${FINNHUB_API_KEY:-}" ]; then
                log_error "Finnhub API key not configured"
                exit 1
            fi
            ;;
        *)
            log_warn "Unknown primary provider: $PRIMARY_PROVIDER"
            ;;
    esac
    
    log_success "Environment validation passed"
}

validate_directories() {
    log_info "Validating directory structure..."
    
    local directories=(
        "$CONFIG_PATH"
        "$LOG_PATH"
        "$DATA_PATH"
        "$CACHE_PATH"
    )
    
    for dir in "${directories[@]}"; do
        if [ ! -d "$dir" ]; then
            log_warn "Directory $dir does not exist, creating..."
            mkdir -p "$dir" || {
                log_error "Failed to create directory $dir"
                exit 1
            }
        fi
        
        # Check write permissions
        if [ ! -w "$dir" ]; then
            log_error "No write permission for directory $dir"
            exit 1
        fi
    done
    
    log_success "Directory validation passed"
}

wait_for_dependencies() {
    log_info "Waiting for dependencies to be ready..."
    
    # Extract database connection details
    local db_host
    local db_port
    if [[ "$DATABASE_URL" =~ postgresql://[^@]*@([^:]*):([0-9]*)/.*$ ]]; then
        db_host="${BASH_REMATCH[1]}"
        db_port="${BASH_REMATCH[2]}"
    else
        # Fallback parsing
        db_host=$(echo "$DATABASE_URL" | sed -n 's/.*@\([^:]*\):.*/\1/p')
        db_port=$(echo "$DATABASE_URL" | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
    fi
    
    # Extract Redis connection details
    local redis_host
    local redis_port
    if [[ "$REDIS_URL" =~ redis://([^:]*):([0-9]*) ]]; then
        redis_host="${BASH_REMATCH[1]}"
        redis_port="${BASH_REMATCH[2]}"
    else
        # Fallback parsing
        redis_host=$(echo "$REDIS_URL" | sed -n 's/.*\/\/\([^:]*\):.*/\1/p')
        redis_port=$(echo "$REDIS_URL" | sed -n 's/.*:\([0-9]*\)/\1/p')
    fi
    
    # Wait for database
    log_info "Waiting for TimescaleDB at $db_host:$db_port..."
    local timeout=60
    local count=0
    while ! timeout 5 bash -c "</dev/tcp/$db_host/$db_port" 2>/dev/null && [ $count -lt $timeout ]; do
        sleep 2
        count=$((count + 2))
        if [ $((count % 10)) -eq 0 ]; then
            log_info "Still waiting for database... ($count/$timeout seconds)"
        fi
    done
    
    if [ $count -ge $timeout ]; then
        log_error "Database connection timeout"
        exit 1
    fi
    
    # Wait for Redis
    log_info "Waiting for Redis at $redis_host:$redis_port..."
    count=0
    while ! timeout 5 bash -c "</dev/tcp/$redis_host/$redis_port" 2>/dev/null && [ $count -lt $timeout ]; do
        sleep 2
        count=$((count + 2))
        if [ $((count % 10)) -eq 0 ]; then
            log_info "Still waiting for Redis... ($count/$timeout seconds)"
        fi
    done
    
    if [ $count -ge $timeout ]; then
        log_error "Redis connection timeout"
        exit 1
    fi
    
    log_success "All dependencies are ready"
}

setup_configuration() {
    log_info "Setting up configuration..."
    
    # Create runtime configuration file
    local runtime_config="$CONFIG_PATH/runtime.yaml"
    cat > "$runtime_config" << EOF
database:
  url: "$DATABASE_URL"
  pool_size: 10
  timeout: 30

redis:
  url: "$REDIS_URL"
  pool_size: 10
  timeout: 10

logging:
  level: "$LOG_LEVEL"
  path: "$LOG_PATH"
  format: "json"

storage:
  data_path: "$DATA_PATH"
  cache_path: "$CACHE_PATH"

server:
  host: "0.0.0.0"
  port: $API_PORT
  metrics_port: $METRICS_PORT

providers:
  primary: "$PRIMARY_PROVIDER"
  symbols: $(echo "$SYMBOLS" | sed 's/,/", "/g' | sed 's/^/["/' | sed 's/$/"]/')
  update_interval: "$UPDATE_INTERVAL"
  
  alpaca:
    api_key: "${ALPACA_API_KEY:-}"
    api_secret: "${ALPACA_API_SECRET:-}"
    base_url: "https://paper-api.alpaca.markets"
    data_url: "https://data.alpaca.markets"
    websocket_enabled: ${ALPACA_WS_ENABLED:-false}
    
  polygon:
    api_key: "${POLYGON_API_KEY:-}"
    base_url: "https://api.polygon.io"
    
  finnhub:
    api_key: "${FINNHUB_API_KEY:-}"
    base_url: "https://finnhub.io/api/v1"
    
  alpha_vantage:
    api_key: "${ALPHA_VANTAGE_API_KEY:-}"
    base_url: "https://www.alphavantage.co"

features:
  rate_limiting: true
  circuit_breaker: true
  metrics: true
  health_checks: true
  data_validation: true
  
rate_limits:
  default: 100  # requests per minute
  polygon: 5    # requests per minute (free tier)
  alpha_vantage: 5  # requests per minute (free tier)
  
circuit_breaker:
  failure_threshold: 5
  recovery_timeout: 60
  half_open_max_calls: 3
EOF
    
    log_success "Configuration setup completed"
}

perform_health_check() {
    log_info "Performing initial health check..."
    
    # Test database connectivity
    python3 -c "
import asyncio
import asyncpg
import sys
import os

async def test_db():
    try:
        conn = await asyncpg.connect('$DATABASE_URL')
        result = await conn.fetchval('SELECT 1')
        await conn.close()
        return result == 1
    except Exception as e:
        print(f'Database connection failed: {e}')
        return False

if not asyncio.run(test_db()):
    sys.exit(1)
" || {
        log_error "Database health check failed"
        exit 1
    }
    
    # Test Redis connectivity
    python3 -c "
import redis
import sys
import os

try:
    r = redis.from_url('$REDIS_URL')
    r.ping()
    print('Redis connection successful')
except Exception as e:
    print(f'Redis connection failed: {e}')
    sys.exit(1)
" || {
        log_error "Redis health check failed"
        exit 1
    }
    
    log_success "Health checks passed"
}

start_data_ingestion() {
    log_info "Starting Data Ingestion service..."
    
    # Set Python environment
    export PYTHONPATH="/opt/data-ingestion/src:$PYTHONPATH"
    export PYTHONUNBUFFERED=1
    export PYTHONDONTWRITEBYTECODE=1
    
    # Change to source directory
    cd /opt/data-ingestion/src
    
    # Start the service
    exec python -m data_ingestion.main --config "$CONFIG_PATH/runtime.yaml" &
    DATA_INGESTION_PID=$!
    
    log_success "Data Ingestion service started with PID: $DATA_INGESTION_PID"
    
    # Wait for the process
    wait $DATA_INGESTION_PID
}

# Main execution flow
main() {
    log_info "Data Ingestion service starting up..."
    log_info "Primary provider: $PRIMARY_PROVIDER"
    log_info "Symbols: $SYMBOLS"
    log_info "Update interval: $UPDATE_INTERVAL"
    log_info "Config path: $CONFIG_PATH"
    log_info "Data path: $DATA_PATH"
    
    # Validation phase
    validate_environment
    validate_directories
    
    # Setup phase
    wait_for_dependencies
    setup_configuration
    
    # Health check phase (if not in development mode)
    if [ "${SKIP_HEALTH_CHECK:-false}" != "true" ]; then
        perform_health_check
    fi
    
    # Start the service
    start_data_ingestion
}

# Handle different command types
case "${1:-}" in
    "python"|"data-ingestion"|"")
        main
        ;;
    "bash"|"sh")
        exec "$@"
        ;;
    "test")
        log_info "Running test suite..."
        cd /opt/data-ingestion/src
        exec python -m pytest tests/ -v
        ;;
    "--help"|"-h")
        echo "Data Ingestion Docker Container"
        echo "Usage: $0 [python|data-ingestion|test|bash|sh|--help]"
        echo ""
        echo "Environment Variables:"
        echo "  DATABASE_URL - PostgreSQL connection string (required)"
        echo "  REDIS_URL - Redis connection string (required)"
        echo "  PRIMARY_PROVIDER - Primary data provider (default: alpaca)"
        echo "  SYMBOLS - Trading symbols comma-separated (default: AAPL,MSFT,GOOGL,AMZN,NVDA)"
        echo "  UPDATE_INTERVAL - Data update interval (default: 60s)"
        echo "  LOG_LEVEL - Log level (default: info)"
        echo "  SKIP_HEALTH_CHECK - Skip initial health check (default: false)"
        exit 0
        ;;
    *)
        log_info "Executing custom command: $*"
        exec "$@"
        ;;
esac