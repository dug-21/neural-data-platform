#!/bin/bash
set -euo pipefail

# Model Manager Service Entrypoint Script
# Handles ML model lifecycle, storage, and serving

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
DEFAULT_CONFIG_PATH="/opt/model-manager/config"
DEFAULT_LOG_PATH="/var/log/model-manager"
DEFAULT_MODEL_PATH="/opt/model-manager/models"
DEFAULT_CACHE_PATH="/opt/model-manager/cache"
DEFAULT_TEMP_PATH="/opt/model-manager/temp"

# Environment variables with defaults
CONFIG_PATH="${CONFIG_PATH:-$DEFAULT_CONFIG_PATH}"
LOG_PATH="${LOG_PATH:-$DEFAULT_LOG_PATH}"
MODEL_STORAGE_PATH="${MODEL_STORAGE_PATH:-$DEFAULT_MODEL_PATH}"
CACHE_PATH="${CACHE_PATH:-$DEFAULT_CACHE_PATH}"
TEMP_PATH="${TEMP_PATH:-$DEFAULT_TEMP_PATH}"
RUST_LOG="${RUST_LOG:-info}"
API_PORT="${API_PORT:-8002}"
METRICS_PORT="${METRICS_PORT:-9093}"

# Model management configuration
MODEL_STORAGE_BACKEND="${MODEL_STORAGE_BACKEND:-filesystem}"
MODEL_VERSIONING="${MODEL_VERSIONING:-true}"
MODEL_COMPRESSION="${MODEL_COMPRESSION:-true}"
AUTO_CLEANUP="${AUTO_CLEANUP:-true}"
CLEANUP_RETENTION_DAYS="${CLEANUP_RETENTION_DAYS:-30}"

# Signal handlers for graceful shutdown
shutdown_handler() {
    log_info "Received shutdown signal, initiating graceful shutdown..."
    
    if [ -n "${MODEL_MANAGER_PID:-}" ]; then
        log_info "Sending SIGTERM to model-manager process (PID: $MODEL_MANAGER_PID)"
        kill -TERM "$MODEL_MANAGER_PID" 2>/dev/null || true
        
        # Wait for graceful shutdown with timeout
        local timeout=45  # Longer timeout for model operations
        local count=0
        while kill -0 "$MODEL_MANAGER_PID" 2>/dev/null && [ $count -lt $timeout ]; do
            sleep 1
            count=$((count + 1))
            if [ $((count % 5)) -eq 0 ]; then
                log_info "Waiting for graceful shutdown... ($count/$timeout seconds)"
            fi
        done
        
        if kill -0 "$MODEL_MANAGER_PID" 2>/dev/null; then
            log_warn "Process did not shut down gracefully, sending SIGKILL"
            kill -KILL "$MODEL_MANAGER_PID" 2>/dev/null || true
        else
            log_success "Model manager shut down gracefully"
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
    
    # Validate model storage backend
    case "$MODEL_STORAGE_BACKEND" in
        "filesystem"|"s3"|"gcs"|"azure")
            log_info "Using $MODEL_STORAGE_BACKEND storage backend"
            ;;
        *)
            log_error "Invalid model storage backend: $MODEL_STORAGE_BACKEND"
            exit 1
            ;;
    esac
    
    log_success "Environment validation passed"
}

validate_directories() {
    log_info "Validating directory structure..."
    
    local directories=(
        "$CONFIG_PATH"
        "$LOG_PATH"
        "$MODEL_STORAGE_PATH"
        "$CACHE_PATH"
        "$TEMP_PATH"
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
    
    # Create model subdirectories
    local model_subdirs=(
        "$MODEL_STORAGE_PATH/active"
        "$MODEL_STORAGE_PATH/archive"
        "$MODEL_STORAGE_PATH/templates"
        "$MODEL_STORAGE_PATH/experiments"
    )
    
    for dir in "${model_subdirs[@]}"; do
        mkdir -p "$dir" || {
            log_error "Failed to create model directory $dir"
            exit 1
        }
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
backend = "$MODEL_STORAGE_BACKEND"
model_path = "$MODEL_STORAGE_PATH"
cache_path = "$CACHE_PATH"
temp_path = "$TEMP_PATH"
versioning = $MODEL_VERSIONING
compression = $MODEL_COMPRESSION

[server]
api_port = $API_PORT
metrics_port = $METRICS_PORT
shutdown_timeout = 45

[model_management]
auto_cleanup = $AUTO_CLEANUP
retention_days = $CLEANUP_RETENTION_DAYS
max_versions_per_model = 10
compression_level = 6

[performance]
max_concurrent_loads = 3
cache_size_mb = 1024
preload_popular_models = true

[monitoring]
metrics_enabled = true
health_check_interval = 30
model_performance_tracking = true

[ml_frameworks]
pytorch_enabled = true
onnx_enabled = true
sklearn_enabled = true
custom_models_enabled = true

[security]
model_validation = true
signature_verification = false
access_logging = true
EOF
    
    log_success "Configuration setup completed"
}

initialize_model_storage() {
    log_info "Initializing model storage..."
    
    # Create model registry database
    local registry_file="$MODEL_STORAGE_PATH/model_registry.json"
    if [ ! -f "$registry_file" ]; then
        cat > "$registry_file" << EOF
{
  "version": "1.0",
  "models": {},
  "metadata": {
    "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "storage_backend": "$MODEL_STORAGE_BACKEND",
    "storage_path": "$MODEL_STORAGE_PATH"
  }
}
EOF
        log_info "Created model registry at $registry_file"
    fi
    
    # Create model template if none exists
    local template_dir="$MODEL_STORAGE_PATH/templates"
    if [ ! "$(ls -A "$template_dir" 2>/dev/null)" ]; then
        log_info "Creating default model templates..."
        mkdir -p "$template_dir/neural_trader_base"
        cat > "$template_dir/neural_trader_base/config.json" << EOF
{
  "name": "neural_trader_base",
  "type": "mlp",
  "version": "1.0.0",
  "input_features": 10,
  "hidden_layers": [64, 32, 16],
  "output_size": 1,
  "activation": "relu",
  "framework": "pytorch"
}
EOF
    fi
    
    log_success "Model storage initialized"
}

perform_health_check() {
    log_info "Performing initial health check..."
    
    # Check Python ML libraries
    python3 -c "
import sys
try:
    import numpy as np
    import torch
    import sklearn
    import onnx
    import onnxruntime
    print('All ML libraries loaded successfully')
except ImportError as e:
    print(f'ML library import failed: {e}')
    sys.exit(1)
" || {
        log_error "ML libraries health check failed"
        exit 1
    }
    
    # Test model storage access
    if [ ! -w "$MODEL_STORAGE_PATH" ]; then
        log_error "Model storage path not writable"
        exit 1
    fi
    
    # Test temporary space
    local test_file="$TEMP_PATH/health_check_test"
    echo "test" > "$test_file" && rm "$test_file" || {
        log_error "Temporary storage health check failed"
        exit 1
    }
    
    log_success "Health checks passed"
}

cleanup_old_models() {
    if [ "$AUTO_CLEANUP" = "true" ]; then
        log_info "Performing model cleanup (retention: $CLEANUP_RETENTION_DAYS days)..."
        
        # Clean up old temporary files
        find "$TEMP_PATH" -type f -mtime +1 -delete 2>/dev/null || true
        
        # Clean up old cached models
        find "$CACHE_PATH" -type f -mtime +"$CLEANUP_RETENTION_DAYS" -delete 2>/dev/null || true
        
        # Archive old experiment models
        find "$MODEL_STORAGE_PATH/experiments" -type f -mtime +"$CLEANUP_RETENTION_DAYS" -exec mv {} "$MODEL_STORAGE_PATH/archive/" \; 2>/dev/null || true
        
        log_info "Model cleanup completed"
    fi
}

start_model_manager() {
    log_info "Starting Model Manager service..."
    
    # Build command arguments
    local cmd_args=(
        "--config" "$CONFIG_PATH/runtime.toml"
        "--log-level" "$RUST_LOG"
    )
    
    # Add optional feature flags
    if [ "${ENABLE_METRICS:-true}" = "true" ]; then
        cmd_args+=("--enable-metrics")
    fi
    
    if [ "${ENABLE_MODEL_VALIDATION:-true}" = "true" ]; then
        cmd_args+=("--enable-validation")
    fi
    
    # Start the service
    exec model-manager "${cmd_args[@]}" &
    MODEL_MANAGER_PID=$!
    
    log_success "Model Manager started with PID: $MODEL_MANAGER_PID"
    
    # Wait for the process
    wait $MODEL_MANAGER_PID
}

# Main execution flow
main() {
    log_info "Model Manager service starting up..."
    log_info "Storage backend: $MODEL_STORAGE_BACKEND"
    log_info "Model path: $MODEL_STORAGE_PATH"
    log_info "Cache path: $CACHE_PATH"
    log_info "Versioning: $MODEL_VERSIONING"
    log_info "Auto cleanup: $AUTO_CLEANUP"
    
    # Validation phase
    validate_environment
    validate_directories
    
    # Setup phase
    wait_for_dependencies
    setup_configuration
    initialize_model_storage
    
    # Health check phase (if not in development mode)
    if [ "${SKIP_HEALTH_CHECK:-false}" != "true" ]; then
        perform_health_check
    fi
    
    # Cleanup phase
    cleanup_old_models
    
    # Start the service
    start_model_manager
}

# Handle different command types
case "${1:-}" in
    "model-manager"|"")
        main
        ;;
    "bash"|"sh")
        exec "$@"
        ;;
    "cleanup")
        log_info "Running model cleanup..."
        cleanup_old_models
        ;;
    "validate")
        log_info "Running validation checks..."
        validate_environment
        validate_directories
        perform_health_check
        log_success "All validations passed"
        ;;
    "--help"|"-h")
        echo "Model Manager Docker Container"
        echo "Usage: $0 [model-manager|cleanup|validate|bash|sh|--help]"
        echo ""
        echo "Environment Variables:"
        echo "  DATABASE_URL - PostgreSQL connection string (required)"
        echo "  REDIS_URL - Redis connection string (required)"
        echo "  MODEL_STORAGE_BACKEND - Storage backend (default: filesystem)"
        echo "  MODEL_VERSIONING - Enable model versioning (default: true)"
        echo "  AUTO_CLEANUP - Enable automatic cleanup (default: true)"
        echo "  CLEANUP_RETENTION_DAYS - Model retention days (default: 30)"
        echo "  RUST_LOG - Log level (default: info)"
        echo "  SKIP_HEALTH_CHECK - Skip initial health check (default: false)"
        exit 0
        ;;
    *)
        log_info "Executing custom command: $*"
        exec "$@"
        ;;
esac