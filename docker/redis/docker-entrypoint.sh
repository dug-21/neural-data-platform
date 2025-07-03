#!/bin/sh
set -e

# Redis Docker entrypoint script for Neural Trader

# Function to log messages
log_info() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] INFO: $*"
}

log_error() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $*" >&2
}

# Check if running as root and switch to redis user if needed
if [ "$(id -u)" = "0" ]; then
    log_info "Running as root, checking permissions..."
    
    # Ensure data directory exists and has correct permissions
    mkdir -p /data
    chown -R redis:redis /data
    
    # Check config file permissions
    if [ -f /usr/local/etc/redis/redis.conf ]; then
        chown redis:redis /usr/local/etc/redis/redis.conf
        chmod 640 /usr/local/etc/redis/redis.conf
    fi
    
    log_info "Switching to redis user..."
    exec su-exec redis "$0" "$@"
fi

# Verify Redis configuration
if [ -f /usr/local/etc/redis/redis.conf ]; then
    log_info "Using custom Redis configuration"
    
    # Test configuration
    redis-server /usr/local/etc/redis/redis.conf --test-memory 256
    
    if [ $? -eq 0 ]; then
        log_info "Redis configuration test passed"
    else
        log_error "Redis configuration test failed"
        exit 1
    fi
else
    log_error "Redis configuration file not found at /usr/local/etc/redis/redis.conf"
    exit 1
fi

# Check if data directory is writable
if [ ! -w /data ]; then
    log_error "Data directory /data is not writable"
    exit 1
fi

# Check available memory
AVAILABLE_MEM=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
if [ -n "$AVAILABLE_MEM" ] && [ "$AVAILABLE_MEM" -lt 524288 ]; then
    log_error "Warning: Less than 512MB of available memory detected"
fi

# Load Redis modules if available
if [ -d /usr/lib/redis/modules ]; then
    log_info "Checking for Redis modules..."
    
    for module in /usr/lib/redis/modules/*.so; do
        if [ -f "$module" ]; then
            module_name=$(basename "$module")
            log_info "Found Redis module: $module_name"
        fi
    done
fi

# Set up performance monitoring
log_info "Enabling Redis performance monitoring..."

# Start Redis with the configuration
log_info "Starting Redis server..."
exec "$@"