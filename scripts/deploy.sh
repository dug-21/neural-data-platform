#!/bin/bash

# Neural Trader Deployment Script
# Usage: ./scripts/deploy.sh [environment] [version]
# Example: ./scripts/deploy.sh production v1.2.3

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
ENVIRONMENT=${1:-staging}
VERSION=${2:-latest}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configuration
COMPOSE_PROJECT_NAME="neural_trader"
BACKUP_BEFORE_DEPLOY=true
HEALTH_CHECK_TIMEOUT=300
ROLLBACK_ON_FAILURE=true

# Logging
LOG_DIR="$PROJECT_ROOT/logs"
mkdir -p "$LOG_DIR"
DEPLOY_LOG="$LOG_DIR/deploy-$(date +%Y%m%d-%H%M%S).log"

# Function to log messages
log() {
    local level=$1
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo -e "${timestamp} [${level}] ${message}" | tee -a "$DEPLOY_LOG"
}

# Function to log with color
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*" | tee -a "$DEPLOY_LOG"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*" | tee -a "$DEPLOY_LOG"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $*" | tee -a "$DEPLOY_LOG"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*" | tee -a "$DEPLOY_LOG"
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    if ! command_exists docker; then
        log_error "Docker is not installed"
        exit 1
    fi
    
    if ! command_exists docker-compose; then
        log_error "Docker Compose is not installed"
        exit 1
    fi
    
    # Check if Docker daemon is running
    if ! docker info >/dev/null 2>&1; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    # Check if environment configuration exists
    if [[ ! -f "$PROJECT_ROOT/docker-compose.${ENVIRONMENT}.yml" ]]; then
        log_error "Environment configuration not found: docker-compose.${ENVIRONMENT}.yml"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Function to validate environment
validate_environment() {
    log_info "Validating environment configuration..."
    
    case "$ENVIRONMENT" in
        development|dev)
            COMPOSE_FILES="-f docker-compose.yml -f docker-compose.dev.yml"
            ;;
        staging|stage)
            COMPOSE_FILES="-f docker-compose.yml -f docker-compose.staging.yml"
            ;;
        production|prod)
            COMPOSE_FILES="-f docker-compose.yml -f docker-compose.secure.yml"
            ;;
        *)
            log_error "Invalid environment: $ENVIRONMENT"
            log_error "Valid environments: development, staging, production"
            exit 1
            ;;
    esac
    
    log_success "Environment validation passed"
}

# Function to run security checks
run_security_checks() {
    log_info "Running security checks..."
    
    # Check if .env file exists in repository
    if [[ -f "$PROJECT_ROOT/.env" ]] && git ls-files --error-unmatch "$PROJECT_ROOT/.env" >/dev/null 2>&1; then
        log_error ".env file found in version control - this is a security risk!"
        log_error "Please remove .env from version control and add to .gitignore"
        exit 1
    fi
    
    # Check for hardcoded secrets in docker-compose files
    if grep -r "password.*=" "$PROJECT_ROOT"/docker-compose*.yml | grep -v "PASSWORD_FILE" | grep -v "example" >/dev/null 2>&1; then
        log_warning "Potential hardcoded passwords found in docker-compose files"
        log_warning "Consider using environment variables or secrets management"
    fi
    
    # Check if secrets directory exists for production
    if [[ "$ENVIRONMENT" == "production" ]] && [[ ! -d "$PROJECT_ROOT/secrets" ]]; then
        log_error "Secrets directory not found for production deployment"
        log_error "Please create secrets directory and add required secret files"
        exit 1
    fi
    
    log_success "Security checks passed"
}

# Function to create backup
create_backup() {
    if [[ "$BACKUP_BEFORE_DEPLOY" != "true" ]]; then
        return 0
    fi
    
    log_info "Creating backup before deployment..."
    
    local backup_dir="$PROJECT_ROOT/backups/pre-deploy"
    mkdir -p "$backup_dir"
    
    local backup_file="$backup_dir/backup-$(date +%Y%m%d-%H%M%S).sql"
    
    # Create database backup
    docker-compose $COMPOSE_FILES exec -T timescaledb pg_dump -U neural_trader -d neural_trader_db > "$backup_file" 2>/dev/null || {
        log_warning "Database backup failed - continuing with deployment"
    }
    
    # Create Redis backup
    docker-compose $COMPOSE_FILES exec -T redis redis-cli BGSAVE >/dev/null 2>&1 || {
        log_warning "Redis backup failed - continuing with deployment"
    }
    
    log_success "Backup created: $backup_file"
}

# Function to build images
build_images() {
    log_info "Building Docker images for version $VERSION..."
    
    cd "$PROJECT_ROOT"
    
    # Build with caching
    docker-compose $COMPOSE_FILES build --pull --parallel || {
        log_error "Image build failed"
        exit 1
    }
    
    # Tag images with version
    docker tag "${COMPOSE_PROJECT_NAME}_neural-trader:latest" "${COMPOSE_PROJECT_NAME}_neural-trader:$VERSION"
    docker tag "${COMPOSE_PROJECT_NAME}_data-ingestion:latest" "${COMPOSE_PROJECT_NAME}_data-ingestion:$VERSION"
    
    log_success "Images built successfully"
}

# Function to deploy services
deploy_services() {
    log_info "Deploying services..."
    
    cd "$PROJECT_ROOT"
    
    # Stop existing services gracefully
    docker-compose $COMPOSE_FILES down --timeout 30 || {
        log_warning "Graceful shutdown failed, forcing stop"
        docker-compose $COMPOSE_FILES down --timeout 5
    }
    
    # Start services
    docker-compose $COMPOSE_FILES up -d --remove-orphans || {
        log_error "Service deployment failed"
        if [[ "$ROLLBACK_ON_FAILURE" == "true" ]]; then
            log_info "Attempting rollback..."
            rollback_deployment
        fi
        exit 1
    }
    
    log_success "Services deployed successfully"
}

# Function to run health checks
run_health_checks() {
    log_info "Running health checks..."
    
    local timeout=$HEALTH_CHECK_TIMEOUT
    local start_time=$(date +%s)
    
    while [[ $(($(date +%s) - start_time)) -lt $timeout ]]; do
        local healthy=true
        
        # Check each service
        for service in timescaledb redis neural-trader data-ingestion; do
            if ! docker-compose $COMPOSE_FILES ps "$service" | grep -q "healthy\|Up"; then
                healthy=false
                break
            fi
        done
        
        if [[ "$healthy" == "true" ]]; then
            log_success "All services are healthy"
            return 0
        fi
        
        log_info "Waiting for services to become healthy..."
        sleep 10
    done
    
    log_error "Health check timeout - services are not healthy"
    
    # Show service status
    docker-compose $COMPOSE_FILES ps
    
    # Show logs for debugging
    docker-compose $COMPOSE_FILES logs --tail=50
    
    if [[ "$ROLLBACK_ON_FAILURE" == "true" ]]; then
        log_info "Attempting rollback..."
        rollback_deployment
    fi
    
    return 1
}

# Function to rollback deployment
rollback_deployment() {
    log_warning "Rolling back deployment..."
    
    # Try to restore from backup
    local backup_dir="$PROJECT_ROOT/backups/pre-deploy"
    local latest_backup=$(ls -t "$backup_dir"/backup-*.sql 2>/dev/null | head -1)
    
    if [[ -n "$latest_backup" ]]; then
        log_info "Restoring from backup: $latest_backup"
        docker-compose $COMPOSE_FILES exec -T timescaledb psql -U neural_trader -d neural_trader_db < "$latest_backup" || {
            log_error "Backup restore failed"
        }
    fi
    
    log_warning "Rollback completed"
}

# Function to run smoke tests
run_smoke_tests() {
    log_info "Running smoke tests..."
    
    local base_url="http://localhost:3030"
    
    # Test health endpoint
    if ! curl -f "$base_url/health" >/dev/null 2>&1; then
        log_error "Health endpoint test failed"
        return 1
    fi
    
    # Test metrics endpoint
    if ! curl -f "$base_url/metrics" >/dev/null 2>&1; then
        log_error "Metrics endpoint test failed"
        return 1
    fi
    
    # Test database connection
    if ! docker-compose $COMPOSE_FILES exec -T timescaledb pg_isready -U neural_trader >/dev/null 2>&1; then
        log_error "Database connection test failed"
        return 1
    fi
    
    # Test Redis connection
    if ! docker-compose $COMPOSE_FILES exec -T redis redis-cli ping | grep -q "PONG"; then
        log_error "Redis connection test failed"
        return 1
    fi
    
    log_success "Smoke tests passed"
}

# Function to cleanup old images
cleanup_old_images() {
    log_info "Cleaning up old images..."
    
    # Remove dangling images
    docker image prune -f >/dev/null 2>&1 || true
    
    # Remove old tagged images (keep last 5)
    docker images "${COMPOSE_PROJECT_NAME}_neural-trader" --format "table {{.Tag}}" | tail -n +6 | xargs -r docker rmi >/dev/null 2>&1 || true
    
    log_success "Cleanup completed"
}

# Function to send deployment notification
send_notification() {
    local status=$1
    local message="Neural Trader deployment to $ENVIRONMENT: $status (version $VERSION)"
    
    # Send to webhook if configured
    if [[ -n "${WEBHOOK_URL:-}" ]]; then
        curl -X POST -H "Content-Type: application/json" \
            -d "{\"text\":\"$message\"}" \
            "$WEBHOOK_URL" >/dev/null 2>&1 || true
    fi
    
    # Send email if configured
    if [[ -n "${ALERT_EMAIL:-}" ]]; then
        echo "$message" | mail -s "Neural Trader Deployment $status" "$ALERT_EMAIL" >/dev/null 2>&1 || true
    fi
}

# Function to display usage
usage() {
    cat << EOF
Usage: $0 [environment] [version]

Arguments:
    environment    Target environment (development, staging, production) [default: staging]
    version        Version tag for deployment [default: latest]

Examples:
    $0 staging v1.2.3
    $0 production v1.2.3
    $0 development latest

Environment variables:
    BACKUP_BEFORE_DEPLOY    Create backup before deployment [default: true]
    HEALTH_CHECK_TIMEOUT    Health check timeout in seconds [default: 300]
    ROLLBACK_ON_FAILURE     Rollback on deployment failure [default: true]
    WEBHOOK_URL             Webhook URL for notifications
    ALERT_EMAIL             Email address for notifications

EOF
}

# Main deployment function
main() {
    log_info "Starting Neural Trader deployment"
    log_info "Environment: $ENVIRONMENT"
    log_info "Version: $VERSION"
    log_info "Timestamp: $(date)"
    
    # Check if help requested
    if [[ "$1" == "-h" ]] || [[ "$1" == "--help" ]]; then
        usage
        exit 0
    fi
    
    # Run deployment steps
    check_prerequisites
    validate_environment
    run_security_checks
    create_backup
    build_images
    deploy_services
    
    # Wait for services to start
    sleep 10
    
    run_health_checks
    run_smoke_tests
    cleanup_old_images
    
    log_success "Deployment completed successfully!"
    log_info "Deployment log: $DEPLOY_LOG"
    
    # Send success notification
    send_notification "SUCCESS"
    
    # Display service status
    echo ""
    log_info "Service status:"
    docker-compose $COMPOSE_FILES ps
    
    echo ""
    log_info "Access URLs:"
    case "$ENVIRONMENT" in
        production)
            echo "  - Application: https://neuraltrader.com"
            echo "  - Monitoring: https://monitoring.neuraltrader.com"
            ;;
        staging)
            echo "  - Application: https://staging.neuraltrader.com"
            echo "  - Monitoring: https://staging-monitoring.neuraltrader.com"
            ;;
        *)
            echo "  - Application: http://localhost:3030"
            echo "  - Grafana: http://localhost:3000"
            echo "  - Prometheus: http://localhost:9090"
            ;;
    esac
}

# Error handling
trap 'log_error "Deployment failed at line $LINENO"; send_notification "FAILED"; exit 1' ERR

# Run main function
main "$@"