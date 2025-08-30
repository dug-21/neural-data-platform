#!/bin/bash
# Developer Environment Setup Script for Neural Trader V2
# Optimized with parallel processing for faster setup

set -e

# Configuration
PROJECT_ROOT=${PROJECT_ROOT:-/workspaces/neural-trader}
PARALLEL_JOBS=${PARALLEL_JOBS:-4}
SKIP_DOCKER=${SKIP_DOCKER:-false}
SKIP_RUST=${SKIP_RUST:-false}
SKIP_PYTHON=${SKIP_PYTHON:-false}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_success() { echo -e "${CYAN}[✓]${NC} $1"; }
log_parallel() { echo -e "${MAGENTA}[PARALLEL]${NC} $1"; }

# Track setup progress
declare -A setup_status

# Check system requirements
check_requirements() {
    log_step "Checking system requirements..."
    
    local missing_deps=()
    
    # Check for essential tools
    command -v git >/dev/null 2>&1 || missing_deps+=("git")
    command -v docker >/dev/null 2>&1 || missing_deps+=("docker")
    command -v docker-compose >/dev/null 2>&1 || missing_deps+=("docker-compose")
    command -v make >/dev/null 2>&1 || missing_deps+=("make")
    command -v curl >/dev/null 2>&1 || missing_deps+=("curl")
    command -v jq >/dev/null 2>&1 || missing_deps+=("jq")
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        log_error "Missing dependencies: ${missing_deps[*]}"
        log_info "Please install missing dependencies and retry."
        return 1
    fi
    
    log_success "All system requirements met"
    setup_status[requirements]="complete"
    return 0
}

# Setup Rust environment
setup_rust() {
    if [ "$SKIP_RUST" = "true" ]; then
        log_info "Skipping Rust setup (SKIP_RUST=true)"
        return 0
    fi
    
    log_step "Setting up Rust environment..."
    
    if ! command -v cargo >/dev/null 2>&1; then
        log_info "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    
    # Install required Rust tools in parallel
    log_parallel "Installing Rust tools in parallel..."
    (
        cargo install cargo-watch 2>&1 | grep -v "already installed" || true
    ) &
    (
        cargo install cargo-tarpaulin 2>&1 | grep -v "already installed" || true
    ) &
    (
        cargo install cargo-audit 2>&1 | grep -v "already installed" || true
    ) &
    (
        rustup component add clippy rustfmt 2>&1 || true
    ) &
    
    wait
    
    log_success "Rust environment ready"
    setup_status[rust]="complete"
}

# Setup Python environment
setup_python() {
    if [ "$SKIP_PYTHON" = "true" ]; then
        log_info "Skipping Python setup (SKIP_PYTHON=true)"
        return 0
    fi
    
    log_step "Setting up Python environment..."
    
    # Create virtual environment
    if [ ! -d "$PROJECT_ROOT/venv" ]; then
        python3 -m venv "$PROJECT_ROOT/venv"
    fi
    
    # Activate and install dependencies
    source "$PROJECT_ROOT/venv/bin/activate"
    
    log_parallel "Installing Python packages in parallel..."
    
    # Create requirements if not exists
    cat > /tmp/requirements.txt << EOF
pytest==7.4.0
pytest-asyncio==0.21.0
pytest-cov==4.1.0
numpy==1.24.3
pandas==2.0.3
redis==4.6.0
protobuf==4.23.4
grpcio==1.56.0
grpcio-tools==1.56.0
pyyaml==6.0.1
jsonschema==4.18.0
psycopg2-binary==2.9.6
faker==19.2.0
EOF
    
    pip install --upgrade pip setuptools wheel
    pip install -r /tmp/requirements.txt
    
    log_success "Python environment ready"
    setup_status[python]="complete"
}

# Setup Docker environment
setup_docker() {
    if [ "$SKIP_DOCKER" = "true" ]; then
        log_info "Skipping Docker setup (SKIP_DOCKER=true)"
        return 0
    fi
    
    log_step "Setting up Docker environment..."
    
    # Ensure Docker is running
    if ! docker info >/dev/null 2>&1; then
        log_error "Docker is not running. Please start Docker and retry."
        return 1
    fi
    
    # Create Docker network
    docker network create neural-trader-v2 2>/dev/null || true
    
    # Pull base images in parallel
    log_parallel "Pulling Docker images in parallel..."
    (
        docker pull rust:1.70-alpine 2>&1 | grep -v "Pull complete" || true
    ) &
    (
        docker pull python:3.11-slim 2>&1 | grep -v "Pull complete" || true
    ) &
    (
        docker pull timescale/timescaledb:latest-pg15 2>&1 | grep -v "Pull complete" || true
    ) &
    (
        docker pull redis:7-alpine 2>&1 | grep -v "Pull complete" || true
    ) &
    
    wait
    
    log_success "Docker environment ready"
    setup_status[docker]="complete"
}

# Create project structure
create_project_structure() {
    log_step "Creating project structure..."
    
    # Create directories in parallel
    log_parallel "Creating directories..."
    
    local dirs=(
        "$PROJECT_ROOT/configs/dev"
        "$PROJECT_ROOT/configs/test"
        "$PROJECT_ROOT/configs/schemas"
        "$PROJECT_ROOT/logs"
        "$PROJECT_ROOT/metrics/baseline"
        "$PROJECT_ROOT/metrics/drift"
        "$PROJECT_ROOT/data/synthetic"
        "$PROJECT_ROOT/.vscode"
        "$PROJECT_ROOT/proto"
    )
    
    for dir in "${dirs[@]}"; do
        mkdir -p "$dir" &
    done
    
    wait
    
    log_success "Project structure created"
    setup_status[structure]="complete"
}

# Setup Git hooks
setup_git_hooks() {
    log_step "Setting up Git hooks..."
    
    # Create pre-commit hook
    cat > "$PROJECT_ROOT/.git/hooks/pre-commit" << 'EOF'
#!/bin/bash
# Pre-commit hook for Neural Trader V2

# Run Rust formatting check
if [ -d "v2" ]; then
    cargo fmt --all -- --check
fi

# Run Python linting
if [ -f ".flake8" ]; then
    flake8 tests/ scripts/
fi

# Check for secrets
if grep -r "api_key\|secret\|password\|token" --include="*.yaml" --include="*.yml" configs/ | grep -v "example\|template"; then
    echo "WARNING: Possible secrets detected in configuration files!"
    exit 1
fi

exit 0
EOF
    
    chmod +x "$PROJECT_ROOT/.git/hooks/pre-commit"
    
    log_success "Git hooks configured"
    setup_status[git_hooks]="complete"
}

# Initialize databases
init_databases() {
    log_step "Initializing databases..."
    
    # Start database containers
    docker-compose -f docker-compose.v2.yml up -d timescaledb redis 2>/dev/null
    
    # Wait for databases to be ready
    log_info "Waiting for databases to start..."
    sleep 5
    
    # Initialize TimescaleDB
    if [ -f "$PROJECT_ROOT/scripts/v2/init-db.sql" ]; then
        PGPASSWORD=postgres psql -h localhost -U postgres -d postgres -f "$PROJECT_ROOT/scripts/v2/init-db.sql" 2>/dev/null || true
    fi
    
    log_success "Databases initialized"
    setup_status[databases]="complete"
}

# Build services in parallel
build_services() {
    log_step "Building services..."
    
    log_parallel "Building all services in parallel (this may take a few minutes)..."
    
    # Build Rust services in parallel
    (
        cd "$PROJECT_ROOT/v2/config-store" && cargo build --release 2>&1 | tail -5
    ) &
    
    (
        cd "$PROJECT_ROOT/v2/data-ingestion" && cargo build --release 2>&1 | tail -5
    ) &
    
    (
        cd "$PROJECT_ROOT/v2/data-staging" && cargo build --release 2>&1 | tail -5
    ) &
    
    (
        cd "$PROJECT_ROOT/v2/neural-trading" && cargo build --release 2>&1 | tail -5
    ) &
    
    wait
    
    # Build Docker images in parallel
    log_parallel "Building Docker images..."
    
    make -f Makefile.v2 v2-build-parallel 2>&1 | grep "Successfully" || true
    
    log_success "All services built"
    setup_status[build]="complete"
}

# Setup environment variables
setup_environment() {
    log_step "Setting up environment variables..."
    
    # Create .env file if not exists
    if [ ! -f "$PROJECT_ROOT/.env" ]; then
        cat > "$PROJECT_ROOT/.env" << EOF
# Neural Trader V2 Development Environment

# Service Configuration
CONFIG_REPO_URL=https://github.com/your-org/neural-trader-configs.git
CONFIG_BRANCH=main
ENVIRONMENT=dev

# Database Configuration
DB_HOST=localhost
DB_PORT=5432
DB_NAME=neural_trader_v2
DB_USER=postgres
DB_PASSWORD=postgres

# Redis Configuration
REDIS_URL=redis://localhost:6379
REDIS_STREAM_PREFIX=neural-trader

# Service Ports
CONFIG_STORE_PORT=50050
DATA_INGESTION_PORT=50051
DATA_STAGING_PORT=50052
NEURAL_ML_OPS_PORT=50053
NEURAL_TRADING_PORT=50054

# Development Settings
LOG_LEVEL=debug
ENABLE_HOT_RELOAD=true
ENABLE_PROFILING=false

# API Keys (use test keys for development)
POLYGON_API_KEY=test_key
ALPHA_VANTAGE_API_KEY=test_key
EOF
    fi
    
    log_success "Environment variables configured"
    setup_status[environment]="complete"
}

# Create helper scripts
create_helper_scripts() {
    log_step "Creating helper scripts..."
    
    # Parallel creation of helper scripts
    (
        cat > "$PROJECT_ROOT/scripts/v2/dev-up.sh" << 'EOF'
#!/bin/bash
# Start development environment
echo "Starting Neural Trader V2 development environment..."
docker-compose -f docker-compose.v2.yml up -d
echo "Services started. View logs with: ./scripts/v2/dev-logs.sh"
EOF
        chmod +x "$PROJECT_ROOT/scripts/v2/dev-up.sh"
    ) &
    
    (
        cat > "$PROJECT_ROOT/scripts/v2/dev-down.sh" << 'EOF'
#!/bin/bash
# Stop development environment
echo "Stopping Neural Trader V2 development environment..."
docker-compose -f docker-compose.v2.yml down
echo "Services stopped."
EOF
        chmod +x "$PROJECT_ROOT/scripts/v2/dev-down.sh"
    ) &
    
    (
        cat > "$PROJECT_ROOT/scripts/v2/dev-logs.sh" << 'EOF'
#!/bin/bash
# View development logs
SERVICE=${1:-}
if [ -z "$SERVICE" ]; then
    docker-compose -f docker-compose.v2.yml logs -f --tail=100
else
    docker-compose -f docker-compose.v2.yml logs -f --tail=100 "$SERVICE"
fi
EOF
        chmod +x "$PROJECT_ROOT/scripts/v2/dev-logs.sh"
    ) &
    
    (
        cat > "$PROJECT_ROOT/scripts/v2/dev-restart.sh" << 'EOF'
#!/bin/bash
# Restart specific service
SERVICE=$1
if [ -z "$SERVICE" ]; then
    echo "Usage: $0 <service-name>"
    exit 1
fi
docker-compose -f docker-compose.v2.yml restart "$SERVICE"
echo "$SERVICE restarted"
EOF
        chmod +x "$PROJECT_ROOT/scripts/v2/dev-restart.sh"
    ) &
    
    wait
    
    log_success "Helper scripts created"
    setup_status[scripts]="complete"
}

# Generate setup report
generate_report() {
    log_step "Generating setup report..."
    
    local report_file="$PROJECT_ROOT/setup-report.txt"
    
    cat > "$report_file" << EOF
========================================
Neural Trader V2 Development Setup Report
========================================
Date: $(date)
User: $(whoami)
System: $(uname -a)

Setup Status
------------
EOF
    
    for component in requirements rust python docker structure git_hooks databases environment scripts; do
        local status="${setup_status[$component]:-pending}"
        echo "✓ $component: $status" >> "$report_file"
    done
    
    cat >> "$report_file" << EOF

Quick Start Commands
--------------------
1. Start services:    ./scripts/v2/dev-up.sh
2. View logs:         ./scripts/v2/dev-logs.sh
3. Run tests:         make v2-test-module MODULE=data-ingestion
4. Stop services:     ./scripts/v2/dev-down.sh

VS Code Integration
-------------------
Open VS Code: code $PROJECT_ROOT
Install recommended extensions when prompted

Environment Variables
---------------------
Configuration: $PROJECT_ROOT/.env
Secrets: Use environment-specific .env files

Documentation
-------------
README: $PROJECT_ROOT/README.md
Architecture: $PROJECT_ROOT/docs/architecture/
API Docs: $PROJECT_ROOT/docs/api/

Support
-------
Issues: https://github.com/your-org/neural-trader/issues
Wiki: https://github.com/your-org/neural-trader/wiki

Next Steps
----------
1. Configure your IDE with the project
2. Run the test pipeline: make v2-test
3. Try the example workflows in docs/examples/
4. Join the development chat channel

EOF
    
    log_info "Setup report saved to: $report_file"
    cat "$report_file"
}

# Main setup function
main() {
    log_info "🚀 Starting Neural Trader V2 Developer Setup..."
    log_info "Using $PARALLEL_JOBS parallel jobs for faster setup"
    
    local start_time=$(date +%s)
    
    # Run setup steps
    check_requirements || exit 1
    
    # Run parallel setup tasks
    log_parallel "Running setup tasks in parallel..."
    
    create_project_structure &
    setup_git_hooks &
    setup_environment &
    
    wait
    
    # Language-specific setup in parallel
    setup_rust &
    setup_python &
    setup_docker &
    
    wait
    
    # Sequential tasks that depend on previous steps
    init_databases
    create_helper_scripts
    
    # Optional build step
    if [ "${BUILD_SERVICES:-false}" = "true" ]; then
        build_services
    else
        log_info "Skipping service build (set BUILD_SERVICES=true to build)"
    fi
    
    # Generate report
    generate_report
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log_success "✨ Development environment setup complete in ${duration} seconds!"
    log_info "Run './scripts/v2/dev-up.sh' to start the development environment"
}

# Run main function
main "$@"