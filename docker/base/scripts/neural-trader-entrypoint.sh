#!/bin/bash
set -euo pipefail

# Neural Trader Service Entrypoint Script
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
DEFAULT_CONFIG_PATH="/opt/neural-trader/config"
DEFAULT_LOG_PATH="/var/log/neural-trader"
DEFAULT_MODEL_PATH="/opt/neural-trader/models"
DEFAULT_DATA_PATH="/opt/neural-trader/data"

# Environment variables with defaults
CONFIG_PATH="${CONFIG_PATH:-$DEFAULT_CONFIG_PATH}"
LOG_PATH="${LOG_PATH:-$DEFAULT_LOG_PATH}"
MODEL_STORAGE_PATH="${MODEL_STORAGE_PATH:-$DEFAULT_MODEL_PATH}"
DATA_PATH="${DATA_PATH:-$DEFAULT_DATA_PATH}"
RUST_LOG="${RUST_LOG:-info}"
HEALTH_CHECK_PORT="${HEALTH_CHECK_PORT:-9092}"
API_PORT="${API_PORT:-8080}"

# Signal handlers for graceful shutdown
shutdown_handler() {
    log_info "Received shutdown signal, initiating graceful shutdown..."
    
    if [ -n "${NEURAL_TRADER_PID:-}" ]; then
        log_info "Sending SIGTERM to neural-trader process (PID: $NEURAL_TRADER_PID)"
        kill -TERM "$NEURAL_TRADER_PID" 2>/dev/null || true
        
        # Wait for graceful shutdown with timeout
        local timeout=30
        local count=0
        while kill -0 "$NEURAL_TRADER_PID" 2>/dev/null && [ $count -lt $timeout ]; do
            sleep 1
            count=$((count + 1))
            if [ $((count % 5)) -eq 0 ]; then
                log_info "Waiting for graceful shutdown... ($count/$timeout seconds)"
            fi
        done
        
        if kill -0 "$NEURAL_TRADER_PID" 2>/dev/null; then
            log_warn "Process did not shut down gracefully, sending SIGKILL"
            kill -KILL "$NEURAL_TRADER_PID" 2>/dev/null || true
        else
            log_success "Neural trader shut down gracefully"
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
    
    # Validate database URL format
    if [[ ! "$DATABASE_URL" =~ ^postgresql:// ]]; then
        log_error "DATABASE_URL must be a valid PostgreSQL connection string"
        exit 1
    fi
    
    # Validate Redis URL format
    if [[ ! "$REDIS_URL" =~ ^redis:// ]]; then
        log_error "REDIS_URL must be a valid Redis connection string"
        exit 1
    fi
    
    log_success "Environment validation passed"
}

validate_directories() {
    log_info "Validating directory structure..."
    
    local directories=(
        "$CONFIG_PATH"
        "$LOG_PATH"
        "$MODEL_STORAGE_PATH"
        "$DATA_PATH"
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
    db_host=$(echo "$DATABASE_URL" | sed -n 's/.*@\([^:]*\):.*/\1/p')
    db_port=$(echo "$DATABASE_URL" | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
    
    # Extract Redis connection details
    local redis_host
    local redis_port
    redis_host=$(echo "$REDIS_URL" | sed -n 's/.*\/\/\([^:]*\):.*/\1/p')
    redis_port=$(echo "$REDIS_URL" | sed -n 's/.*:\([0-9]*\)/\1/p')
    
    # Wait for database
    log_info "Waiting for TimescaleDB at $db_host:$db_port..."
    local timeout=60
    local count=0
    while ! nc -z "$db_host" "$db_port" 2>/dev/null && [ $count -lt $timeout ]; do
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
    while ! nc -z "$redis_host" "$redis_port" 2>/dev/null && [ $count -lt $timeout ]; do
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
    local runtime_config="$CONFIG_PATH/runtime.toml"
    cat > "$runtime_config" << EOF
[database]
url = "$DATABASE_URL"
max_connections = 10
connection_timeout = 30

[redis]
url = "$REDIS_URL"
max_connections = 10
connection_timeout = 10

[logging]
level = "$RUST_LOG"
path = "$LOG_PATH"

[storage]
model_path = "$MODEL_STORAGE_PATH"
data_path = "$DATA_PATH"

[server]
api_port = $API_PORT
health_port = $HEALTH_CHECK_PORT
shutdown_timeout = 30

[features]
neural_enabled = ${NEURAL_USE_REAL_MODELS:-true}
sector_models = ${ENABLE_SECTOR_MODELS:-true}
realtime_adaptation = ${ENABLE_REALTIME_ADAPTATION:-true}
autonomous_training = ${ENABLE_AUTONOMOUS_TRAINING:-false}

[training]
sample_threshold = ${TRAINING_SAMPLE_THRESHOLD:-1000}
history_days = ${TRAINING_HISTORY_DAYS:-90}
EOF
    
    log_success "Configuration setup completed"
}

perform_health_check() {
    log_info "Performing initial health check..."
    
    # Start neural-trader in background for health check
    neural-trader --config "$CONFIG_PATH/runtime.toml" &
    local health_pid=$!
    
    # Wait for service to start
    sleep 10
    
    # Check if process is still running
    if ! kill -0 $health_pid 2>/dev/null; then
        log_error "Neural trader failed to start"
        exit 1
    fi
    
    # Perform health check
    local health_url="http://localhost:$HEALTH_CHECK_PORT/health"
    local count=0
    local max_attempts=30
    
    while [ $count -lt $max_attempts ]; do
        if curl -sf "$health_url" >/dev/null 2>&1; then
            log_success "Health check passed"
            kill $health_pid 2>/dev/null || true
            wait $health_pid 2>/dev/null || true
            return 0
        fi
        
        sleep 2
        count=$((count + 1))
        
        if [ $((count % 5)) -eq 0 ]; then
            log_info "Health check attempt $count/$max_attempts..."
        fi
    done
    
    log_error "Health check failed after $max_attempts attempts"
    kill $health_pid 2>/dev/null || true
    exit 1
}

start_neural_trader() {
    log_info "Starting Neural Trader service..."
    
    # Build command arguments
    local cmd_args=(
        "--config" "$CONFIG_PATH/runtime.toml"
        "--log-level" "$RUST_LOG"
    )
    
    # Add optional feature flags
    if [ "${ENABLE_METRICS:-true}" = "true" ]; then
        cmd_args+=("--enable-metrics")
    fi
    
    if [ "${ENABLE_TRACING:-false}" = "true" ]; then
        cmd_args+=("--enable-tracing")
    fi
    
    # Start the service
    exec neural-trader "${cmd_args[@]}" &
    NEURAL_TRADER_PID=$!
    
    log_success "Neural Trader started with PID: $NEURAL_TRADER_PID"
    
    # Wait for the process
    wait $NEURAL_TRADER_PID
}

# Main execution flow
main() {
    log_info "Neural Trader service starting up..."
    log_info "Version: $(neural-trader --version 2>/dev/null || echo 'unknown')"
    log_info "Config path: $CONFIG_PATH"
    log_info "Log path: $LOG_PATH"
    log_info "Model path: $MODEL_STORAGE_PATH"
    
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
    start_neural_trader
}

# Handle different command types
case "${1:-}" in
    "neural-trader"|"")
        main
        ;;
    "bash"|"sh")
        exec "$@"
        ;;
    "--help"|"-h")
        echo "Neural Trader Docker Container"
        echo "Usage: $0 [neural-trader|bash|sh|--help]"
        echo ""
        echo "Environment Variables:"
        echo "  DATABASE_URL - PostgreSQL connection string (required)"
        echo "  REDIS_URL - Redis connection string (required)"
        echo "  RUST_LOG - Log level (default: info)"
        echo "  CONFIG_PATH - Configuration directory (default: /opt/neural-trader/config)"
        echo "  SKIP_HEALTH_CHECK - Skip initial health check (default: false)"
        exit 0
        ;;
    *)
        log_info "Executing custom command: $*"
        exec "$@"
        ;;
esac