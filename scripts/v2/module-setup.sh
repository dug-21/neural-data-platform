#!/bin/bash
# Module Setup Script - Prepare environment for module testing

set -e

# Configuration
MODULE=${1:-}
ENV=${CONFIG_ENV:-dev}
CACHE_DIR=${CACHE_DIR:-/tmp/module-cache}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

# Module dependency matrix
declare -A MODULE_DEPS
MODULE_DEPS["config-store"]="redis"
MODULE_DEPS["data-ingestion"]="redis config-store"
MODULE_DEPS["data-staging"]="redis timescaledb config-store"
MODULE_DEPS["neural-ml-ops"]="config-store data-staging"
MODULE_DEPS["neural-trading"]="config-store neural-ml-ops"

# Module service ports
declare -A MODULE_PORTS
MODULE_PORTS["config-store"]="50051"
MODULE_PORTS["data-ingestion"]="8081"
MODULE_PORTS["data-staging"]="50052"
MODULE_PORTS["neural-ml-ops"]="50053"
MODULE_PORTS["neural-trading"]="50054"

# Validate module name
validate_module() {
    if [ -z "$MODULE" ]; then
        log_error "Module name required"
        echo "Usage: $0 <module-name>"
        echo "Available modules: config-store, data-ingestion, data-staging, neural-ml-ops, neural-trading"
        exit 1
    fi
    
    if [ -z "${MODULE_DEPS[$MODULE]+x}" ]; then
        log_error "Unknown module: $MODULE"
        exit 1
    fi
}

# Setup cache directory
setup_cache() {
    log_step "Setting up cache directory..."
    mkdir -p "$CACHE_DIR/$MODULE"
    
    # Create module-specific directories
    mkdir -p "$CACHE_DIR/$MODULE/build"
    mkdir -p "$CACHE_DIR/$MODULE/test"
    mkdir -p "$CACHE_DIR/$MODULE/coverage"
    
    log_info "Cache directory ready: $CACHE_DIR/$MODULE"
}

# Start minimal dependencies
start_dependencies() {
    local deps="${MODULE_DEPS[$MODULE]}"
    
    if [ -z "$deps" ]; then
        log_info "No dependencies for $MODULE"
        return 0
    fi
    
    log_step "Starting dependencies for $MODULE: $deps"
    
    for dep in $deps; do
        case $dep in
            redis)
                if ! docker ps | grep -q neural-redis; then
                    log_info "Starting Redis..."
                    docker-compose -f docker-compose.v2.yml up -d redis
                    sleep 3
                fi
                ;;
            timescaledb)
                if ! docker ps | grep -q neural-timescale; then
                    log_info "Starting TimescaleDB..."
                    docker-compose -f docker-compose.v2.yml up -d timescaledb
                    sleep 5
                fi
                ;;
            config-store)
                if ! docker ps | grep -q neural-config-store; then
                    log_info "Starting config-store..."
                    docker-compose -f docker-compose.v2.yml up -d config-store
                    sleep 3
                fi
                ;;
            data-staging)
                if ! docker ps | grep -q neural-data-staging; then
                    log_info "Starting data-staging..."
                    docker-compose -f docker-compose.v2.yml up -d data-staging
                    sleep 3
                fi
                ;;
            *)
                log_warn "Unknown dependency: $dep"
                ;;
        esac
    done
    
    log_info "All dependencies started"
}

# Create test environment file
create_test_env() {
    log_step "Creating test environment..."
    
    local env_file="$CACHE_DIR/$MODULE/.env.test"
    
    cat > "$env_file" << EOF
# Module test environment
MODULE_NAME=$MODULE
MODULE_PORT=${MODULE_PORTS[$MODULE]}
CONFIG_ENV=test
LOG_LEVEL=debug
REDIS_URL=redis://localhost:6379
POSTGRES_URL=postgresql://postgres:postgres@localhost:5432/neural_trader_test
CONFIG_STORE_URL=localhost:50051
TEST_TIMEOUT=60
COVERAGE_THRESHOLD=70
EOF
    
    log_info "Test environment created: $env_file"
    echo "$env_file"
}

# Setup test fixtures
setup_fixtures() {
    log_step "Setting up test fixtures..."
    
    local fixture_dir="$CACHE_DIR/$MODULE/fixtures"
    mkdir -p "$fixture_dir"
    
    # Create module-specific fixtures
    case $MODULE in
        config-store)
            cat > "$fixture_dir/test-config.yaml" << EOF
service:
  name: test-service
  version: 1.0.0
server:
  port: 8080
EOF
            ;;
        data-ingestion)
            cat > "$fixture_dir/test-market-data.json" << EOF
{
  "symbol": "TEST",
  "timestamp": "2024-01-01T00:00:00Z",
  "price": 100.00,
  "volume": 1000
}
EOF
            ;;
        data-staging)
            cat > "$fixture_dir/test-features.json" << EOF
{
  "symbol": "TEST",
  "features": {
    "sma_20": 99.5,
    "rsi": 55.0,
    "volume_avg": 1000000
  }
}
EOF
            ;;
        neural-ml-ops)
            cat > "$fixture_dir/test-model.json" << EOF
{
  "model_name": "test-model",
  "version": "1.0.0",
  "type": "regression",
  "accuracy": 0.85
}
EOF
            ;;
        neural-trading)
            cat > "$fixture_dir/test-signal.json" << EOF
{
  "symbol": "TEST",
  "signal": "BUY",
  "confidence": 0.75,
  "timestamp": "2024-01-01T00:00:00Z"
}
EOF
            ;;
    esac
    
    log_info "Test fixtures created in $fixture_dir"
}

# Check service health
check_health() {
    log_step "Checking service health..."
    
    local port="${MODULE_PORTS[$MODULE]}"
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if nc -z localhost "$port" 2>/dev/null; then
            log_info "$MODULE is healthy on port $port"
            return 0
        fi
        
        attempt=$((attempt + 1))
        sleep 1
    done
    
    log_error "$MODULE failed to become healthy"
    return 1
}

# Generate setup report
generate_report() {
    local report_file="$CACHE_DIR/$MODULE/setup-report.txt"
    
    cat > "$report_file" << EOF
Module Setup Report
===================
Date: $(date)
Module: $MODULE
Environment: $ENV

Setup Steps Completed:
----------------------
✓ Cache directory created
✓ Dependencies started: ${MODULE_DEPS[$MODULE]:-none}
✓ Test environment configured
✓ Test fixtures prepared
✓ Service port: ${MODULE_PORTS[$MODULE]}

Cache Location: $CACHE_DIR/$MODULE
Environment File: $CACHE_DIR/$MODULE/.env.test
Fixtures: $CACHE_DIR/$MODULE/fixtures/

Status: READY FOR TESTING
EOF
    
    log_info "Setup report saved: $report_file"
    cat "$report_file"
}

# Cleanup function
cleanup() {
    if [ "$?" -ne 0 ]; then
        log_error "Setup failed, cleaning up..."
        # Don't stop dependencies on failure - might be needed for debugging
    fi
}

trap cleanup EXIT

# Main execution
main() {
    log_info "Starting module setup for: $MODULE"
    
    validate_module
    setup_cache
    start_dependencies
    local env_file=$(create_test_env)
    setup_fixtures
    
    # Export environment for subsequent scripts
    export MODULE_ENV_FILE="$env_file"
    export MODULE_CACHE_DIR="$CACHE_DIR/$MODULE"
    export MODULE_FIXTURES_DIR="$CACHE_DIR/$MODULE/fixtures"
    
    generate_report
    
    log_info "Module setup complete for $MODULE"
    log_info "Ready for testing with minimal dependencies"
}

main