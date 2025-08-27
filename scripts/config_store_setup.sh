#!/bin/bash
set -euo pipefail

# Neural Trader Config Store Setup Script
# Automates the complete setup and migration process

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration
REDIS_URL="${REDIS_URL:-redis://localhost:6379}"
CONFIG_SEED_FILE="${PROJECT_ROOT}/config/config_store_seed.json"
MIGRATION_REPORT="${PROJECT_ROOT}/scripts/migration_report.json"
DRY_RUN="${DRY_RUN:-false}"
SKIP_BUILD="${SKIP_BUILD:-false}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_dependencies() {
    log_info "Checking dependencies..."
    
    # Check if Python is available
    if ! command -v python3 &> /dev/null; then
        log_error "Python 3 is required but not installed"
        exit 1
    fi
    
    # Check if Docker and Docker Compose are available
    if ! command -v docker &> /dev/null; then
        log_error "Docker is required but not installed"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_error "Docker Compose is required but not installed"
        exit 1
    fi
    
    # Install Python dependencies
    if [ -f "${SCRIPT_DIR}/requirements.txt" ]; then
        log_info "Installing Python dependencies..."
        pip3 install -r "${SCRIPT_DIR}/requirements.txt" --quiet
    fi
    
    log_success "Dependencies check passed"
}

wait_for_redis() {
    log_info "Waiting for Redis to be ready..."
    local max_attempts=30
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if python3 -c "import redis; redis.from_url('$REDIS_URL').ping()" 2>/dev/null; then
            log_success "Redis is ready"
            return 0
        fi
        
        log_info "Waiting for Redis... (attempt $attempt/$max_attempts)"
        sleep 2
        ((attempt++))
    done
    
    log_error "Redis failed to become ready after $max_attempts attempts"
    return 1
}

build_services() {
    if [ "$SKIP_BUILD" = "true" ]; then
        log_info "Skipping build (SKIP_BUILD=true)"
        return 0
    fi
    
    log_info "Building config-store service..."
    cd "$PROJECT_ROOT"
    
    # Build config-store service
    docker-compose build config-store
    
    log_success "Config-store service built successfully"
}

start_infrastructure() {
    log_info "Starting infrastructure services..."
    cd "$PROJECT_ROOT"
    
    # Start Redis first
    docker-compose up -d redis
    
    # Wait for Redis to be ready
    if ! wait_for_redis; then
        log_error "Failed to start Redis"
        exit 1
    fi
    
    # Start config-store service
    docker-compose up -d config-store
    
    # Wait for config-store to be ready
    log_info "Waiting for config-store to be ready..."
    local max_attempts=30
    local attempt=1
    
    while [ $attempt -le $max_attempts ]; do
        if docker-compose exec -T config-store grpc_health_probe -addr=localhost:8003 2>/dev/null; then
            log_success "Config-store is ready"
            break
        fi
        
        if [ $attempt -eq $max_attempts ]; then
            log_error "Config-store failed to become ready"
            docker-compose logs config-store
            exit 1
        fi
        
        log_info "Waiting for config-store... (attempt $attempt/$max_attempts)"
        sleep 2
        ((attempt++))
    done
}

run_migration() {
    log_info "Running configuration migration..."
    cd "$SCRIPT_DIR"
    
    local dry_run_flag=""
    if [ "$DRY_RUN" = "true" ]; then
        dry_run_flag="--dry-run"
        log_warning "Running in DRY RUN mode - no changes will be made"
    fi
    
    # Run migration script
    python3 migrate_config.py \
        --redis-url "$REDIS_URL" \
        --seed-file "$CONFIG_SEED_FILE" \
        --report-file "$MIGRATION_REPORT" \
        $dry_run_flag
    
    if [ $? -eq 0 ]; then
        log_success "Migration completed successfully"
        
        # Show migration report
        if [ -f "$MIGRATION_REPORT" ]; then
            log_info "Migration report:"
            python3 -c "
import json
with open('$MIGRATION_REPORT', 'r') as f:
    report = json.load(f)
    print(f\"  Migrated namespaces: {report.get('total_namespaces', 0)}\")
    print(f\"  Validation passed: {report.get('validation_passed', False)}\")
    if report.get('errors'):
        print(f\"  Errors: {len(report['errors'])}\")
        for error in report['errors']:
            print(f\"    - {error}\")
"
        fi
    else
        log_error "Migration failed"
        exit 1
    fi
}

verify_setup() {
    log_info "Verifying setup..."
    
    # Check Redis keys
    log_info "Checking Redis for configuration keys..."
    local key_count=$(python3 -c "
import redis
r = redis.from_url('$REDIS_URL')
keys = r.keys('config::*')
print(len(keys))
")
    
    if [ "$key_count" -gt 0 ]; then
        log_success "Found $key_count configuration namespaces in Redis"
    else
        log_warning "No configuration keys found in Redis"
    fi
    
    # Check config-store service health
    log_info "Checking config-store service health..."
    if docker-compose exec -T config-store grpc_health_probe -addr=localhost:8003 2>/dev/null; then
        log_success "Config-store service is healthy"
    else
        log_error "Config-store service health check failed"
        return 1
    fi
    
    # Verify configuration loading
    log_info "Testing configuration retrieval..."
    local test_result=$(python3 -c "
import redis
import json
r = redis.from_url('$REDIS_URL')
try:
    config = r.hget('config::neural-trading/data-ingestion', 'data')
    if config:
        data = json.loads(config)
        print('SUCCESS' if 'sources' in data else 'MISSING_SOURCES')
    else:
        print('NO_CONFIG')
except Exception as e:
    print(f'ERROR: {e}')
")
    
    if [ "$test_result" = "SUCCESS" ]; then
        log_success "Configuration retrieval test passed"
    else
        log_error "Configuration retrieval test failed: $test_result"
        return 1
    fi
    
    log_success "Setup verification completed"
}

cleanup() {
    if [ "${1:-}" = "error" ]; then
        log_error "Setup failed. Cleaning up..."
        cd "$PROJECT_ROOT"
        docker-compose stop config-store redis 2>/dev/null || true
    fi
}

show_usage() {
    cat << EOF
Neural Trader Config Store Setup

USAGE:
    $0 [OPTIONS]

OPTIONS:
    --dry-run           Run migration in dry-run mode (no changes made)
    --skip-build        Skip building Docker images
    --redis-url URL     Redis connection URL (default: redis://localhost:6379)
    --help              Show this help message

ENVIRONMENT VARIABLES:
    DRY_RUN            Set to 'true' for dry-run mode
    SKIP_BUILD         Set to 'true' to skip building Docker images
    REDIS_URL          Redis connection URL

EXAMPLES:
    # Full setup with build
    $0
    
    # Dry run to preview changes
    $0 --dry-run
    
    # Skip building images
    $0 --skip-build
    
    # Custom Redis URL
    $0 --redis-url redis://custom-redis:6379

EOF
}

main() {
    local start_time=$(date +%s)
    
    log_info "Starting Neural Trader Config Store Setup"
    log_info "Project root: $PROJECT_ROOT"
    log_info "Redis URL: $REDIS_URL"
    
    # Set up error handling
    trap 'cleanup error' ERR
    trap 'cleanup' EXIT
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dry-run)
                DRY_RUN="true"
                shift
                ;;
            --skip-build)
                SKIP_BUILD="true"
                shift
                ;;
            --redis-url)
                REDIS_URL="$2"
                shift 2
                ;;
            --help)
                show_usage
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
    
    # Execute setup steps
    check_dependencies
    build_services
    start_infrastructure
    run_migration
    verify_setup
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log_success "Config Store setup completed successfully in ${duration}s"
    
    if [ "$DRY_RUN" != "true" ]; then
        log_info "Next steps:"
        log_info "  1. Update service configurations to use config-store"
        log_info "  2. Restart services with new configuration"
        log_info "  3. Monitor config-store metrics and logs"
        log_info ""
        log_info "Config-store endpoints:"
        log_info "  gRPC API: localhost:8003"
        log_info "  Metrics:  localhost:9094"
        log_info ""
        log_info "Configuration files:"
        log_info "  Seed data: $CONFIG_SEED_FILE"
        log_info "  Migration report: $MIGRATION_REPORT"
    else
        log_info "Dry run completed. Review the migration report and run without --dry-run to apply changes."
    fi
}

# Check if script is being sourced or executed
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi