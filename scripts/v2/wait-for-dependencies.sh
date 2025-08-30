#!/bin/bash
# Wait for service dependencies to be ready

set -e

# Default timeout
TIMEOUT=${TIMEOUT:-60}
WAIT_INTERVAL=${WAIT_INTERVAL:-2}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Wait for Redis
wait_for_redis() {
    local host=${REDIS_HOST:-redis}
    local port=${REDIS_PORT:-6379}
    local elapsed=0

    log_info "Waiting for Redis at ${host}:${port}..."
    
    while ! redis-cli -h $host -p $port ping > /dev/null 2>&1; do
        if [ $elapsed -ge $TIMEOUT ]; then
            log_error "Timeout waiting for Redis"
            exit 1
        fi
        
        sleep $WAIT_INTERVAL
        elapsed=$((elapsed + WAIT_INTERVAL))
        echo -n "."
    done
    
    log_info "Redis is ready!"
}

# Wait for PostgreSQL/TimescaleDB
wait_for_postgres() {
    local host=${POSTGRES_HOST:-timescaledb}
    local port=${POSTGRES_PORT:-5432}
    local user=${POSTGRES_USER:-postgres}
    local elapsed=0

    log_info "Waiting for PostgreSQL at ${host}:${port}..."
    
    while ! pg_isready -h $host -p $port -U $user > /dev/null 2>&1; do
        if [ $elapsed -ge $TIMEOUT ]; then
            log_error "Timeout waiting for PostgreSQL"
            exit 1
        fi
        
        sleep $WAIT_INTERVAL
        elapsed=$((elapsed + WAIT_INTERVAL))
        echo -n "."
    done
    
    log_info "PostgreSQL is ready!"
}

# Wait for gRPC service
wait_for_grpc() {
    local service=$1
    local host=$2
    local port=$3
    local elapsed=0

    log_info "Waiting for gRPC service ${service} at ${host}:${port}..."
    
    while ! grpc_health_probe -addr="${host}:${port}" > /dev/null 2>&1; do
        if [ $elapsed -ge $TIMEOUT ]; then
            log_error "Timeout waiting for ${service}"
            exit 1
        fi
        
        sleep $WAIT_INTERVAL
        elapsed=$((elapsed + WAIT_INTERVAL))
        echo -n "."
    done
    
    log_info "${service} is ready!"
}

# Wait for HTTP health endpoint
wait_for_http() {
    local service=$1
    local url=$2
    local elapsed=0

    log_info "Waiting for HTTP service ${service} at ${url}..."
    
    while ! curl -f -s $url > /dev/null 2>&1; do
        if [ $elapsed -ge $TIMEOUT ]; then
            log_error "Timeout waiting for ${service}"
            exit 1
        fi
        
        sleep $WAIT_INTERVAL
        elapsed=$((elapsed + WAIT_INTERVAL))
        echo -n "."
    done
    
    log_info "${service} is ready!"
}

# Main execution
main() {
    local service=${1:-all}
    
    case $service in
        redis)
            wait_for_redis
            ;;
        postgres|timescaledb)
            wait_for_postgres
            ;;
        config-store)
            wait_for_grpc "config-store" "${CONFIG_STORE_HOST:-config-store}" "${CONFIG_STORE_PORT:-50051}"
            ;;
        data-ingestion)
            wait_for_http "data-ingestion" "http://${DATA_INGESTION_HOST:-data-ingestion}:8081/health"
            ;;
        data-staging)
            wait_for_grpc "data-staging" "${DATA_STAGING_HOST:-data-staging}" "${DATA_STAGING_PORT:-50052}"
            ;;
        neural-ml-ops)
            wait_for_grpc "neural-ml-ops" "${NEURAL_ML_OPS_HOST:-neural-ml-ops}" "${NEURAL_ML_OPS_PORT:-50053}"
            ;;
        neural-trading)
            wait_for_grpc "neural-trading" "${NEURAL_TRADING_HOST:-neural-trading}" "${NEURAL_TRADING_PORT:-50054}"
            ;;
        infrastructure)
            wait_for_redis
            wait_for_postgres
            ;;
        all)
            wait_for_redis
            wait_for_postgres
            wait_for_grpc "config-store" "${CONFIG_STORE_HOST:-config-store}" "${CONFIG_STORE_PORT:-50051}"
            ;;
        *)
            log_error "Unknown service: $service"
            echo "Usage: $0 [redis|postgres|config-store|data-ingestion|data-staging|neural-ml-ops|neural-trading|infrastructure|all]"
            exit 1
            ;;
    esac
    
    log_info "All dependencies are ready!"
}

main "$@"