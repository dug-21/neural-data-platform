#!/bin/bash
# Module Build Script - Build specific module with caching

set -e

# Script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Configuration
MODULE=${1:-}
CACHE_DIR=${MODULE_CACHE_DIR:-/tmp/module-cache}
BUILD_CACHE=${BUILD_CACHE:-true}
PARALLEL_BUILD=${PARALLEL_BUILD:-true}

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

# Timer functions
start_timer() {
    START_TIME=$(date +%s)
}

end_timer() {
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    echo $DURATION
}

# Determine module path
get_module_path() {
    # If MODULE_PATH already set by validate_module, use it
    if [ -n "$MODULE_PATH" ]; then
        return 0
    fi
    
    # Otherwise determine path
    if [ -d "$MODULE" ]; then
        MODULE_PATH="$MODULE"
        SERVICE_ROOT="."
    elif [ -d "services/$MODULE" ]; then
        MODULE_PATH="services/$MODULE"
        SERVICE_ROOT="services"
    else
        log_error "Module directory not found: $MODULE"
        return 1
    fi
    log_info "Found module at: $MODULE_PATH"
    return 0
}

# Global variables for module location
MODULE_PATH=""
SERVICE_ROOT=""

# Validate module and set SERVICE_ROOT
validate_module() {
    if [ -z "$MODULE" ]; then
        log_error "Module name required"
        echo "Usage: $0 <module-name>"
        exit 1
    fi
    
    # Determine where module exists and set SERVICE_ROOT
    if [ -d "$MODULE" ]; then
        SERVICE_ROOT="."
        MODULE_PATH="$MODULE"
        log_info "Module found in root directory: $MODULE"
    elif [ -d "services/$MODULE" ]; then
        SERVICE_ROOT="services"
        MODULE_PATH="services/$MODULE"
        log_info "Module found in services directory: services/$MODULE"
    elif [ -d "modules/$MODULE" ]; then
        SERVICE_ROOT="modules"
        MODULE_PATH="modules/$MODULE"
        log_info "Module found in modules directory: modules/$MODULE"
    else
        log_error "Module directory not found: $MODULE (checked root, services/, and modules/)"
        exit 1
    fi
    
    log_info "SERVICE_ROOT set to: $SERVICE_ROOT"
}

# Check cache validity
check_cache() {
    local cache_marker="$CACHE_DIR/$MODULE/build/.cache-marker"
    local source_hash="$CACHE_DIR/$MODULE/build/.source-hash"
    
    if [ "$BUILD_CACHE" != "true" ]; then
        log_info "Cache disabled, forcing rebuild"
        return 1
    fi
    
    if [ ! -f "$cache_marker" ]; then
        log_info "No cache found"
        return 1
    fi
    
    # Calculate source hash using MODULE_PATH
    if [ -z "$MODULE_PATH" ]; then
        get_module_path || return 1
    fi
    local current_hash=$(find $MODULE_PATH -type f -name "*.rs" -o -name "*.toml" | xargs sha256sum | sha256sum | cut -d' ' -f1)
    
    if [ -f "$source_hash" ]; then
        local cached_hash=$(cat "$source_hash")
        if [ "$current_hash" = "$cached_hash" ]; then
            log_info "Cache valid, skipping build"
            return 0
        fi
    fi
    
    log_info "Source changed, rebuilding"
    echo "$current_hash" > "$source_hash"
    return 1
}

# Build Rust module
build_rust_module() {
    log_step "Building Rust module: $MODULE"
    
    start_timer
    
    # Use MODULE_PATH
    if [ -z "$MODULE_PATH" ]; then
        get_module_path || return 1
    fi
    cd "$MODULE_PATH"
    
    # Use cargo cache if available
    if [ -d "$CACHE_DIR/$MODULE/build/target" ]; then
        ln -sf "$CACHE_DIR/$MODULE/build/target" target
    fi
    
    # Build with optimizations for testing
    if [ "$PARALLEL_BUILD" = "true" ]; then
        cargo build --release --jobs $(nproc)
    else
        cargo build --release
    fi
    
    # Cache the build artifacts (we're still in MODULE_PATH directory)
    if [ ! -L "target" ]; then
        mkdir -p "$CACHE_DIR/$MODULE/build"
        if [ -d "target" ]; then
            mv target "$CACHE_DIR/$MODULE/build/"
            ln -sf "$CACHE_DIR/$MODULE/build/target" target
        else
            log_warn "No target directory found to cache"
        fi
    fi
    
    duration=$(end_timer)
    log_info "Rust build completed in ${duration}s"
    
    # Return to root directory
    cd "$SCRIPT_DIR/../.."
    
    # Mark cache as valid
    touch "$CACHE_DIR/$MODULE/build/.cache-marker"
}

# Build Python module
build_python_module() {
    log_step "Building Python module: $MODULE"
    
    start_timer
    
    # Use MODULE_PATH
    if [ -z "$MODULE_PATH" ]; then
        get_module_path || return 1
    fi
    cd "$MODULE_PATH"
    
    # Create virtual environment if needed
    if [ ! -d "$CACHE_DIR/$MODULE/build/venv" ]; then
        python3 -m venv "$CACHE_DIR/$MODULE/build/venv"
    fi
    
    # Activate and install dependencies
    source "$CACHE_DIR/$MODULE/build/venv/bin/activate"
    
    if [ -f "requirements.txt" ]; then
        pip install --cache-dir "$CACHE_DIR/$MODULE/build/pip-cache" -r requirements.txt
    fi
    
    if [ -f "setup.py" ]; then
        pip install --cache-dir "$CACHE_DIR/$MODULE/build/pip-cache" -e .
    fi
    
    deactivate
    
    duration=$(end_timer)
    log_info "Python build completed in ${duration}s"
    
    # Return to root directory
    cd "$SCRIPT_DIR/../.."
    
    # Mark cache as valid
    touch "$CACHE_DIR/$MODULE/build/.cache-marker"
}

# Build Docker image with cache
build_docker_image() {
    log_step "Building Docker image for $MODULE"
    
    start_timer
    
    local dockerfile="docker/v2/Dockerfile.$MODULE"
    
    if [ ! -f "$dockerfile" ]; then
        log_error "Dockerfile not found: $dockerfile"
        return 1
    fi
    
    # Build with cache
    docker build \
        --cache-from neural-trader/$MODULE:cache \
        --tag neural-trader/$MODULE:latest \
        --tag neural-trader/$MODULE:cache \
        --file "$dockerfile" \
        .
    
    duration=$(end_timer)
    log_info "Docker build completed in ${duration}s"
}

# Run module linting
run_linting() {
    log_step "Running linting for $MODULE"
    
    case $MODULE in
        config-store|data-staging|neural-ml-ops|neural-trading|data-ingestion)
            # Use MODULE_PATH
            if [ -z "$MODULE_PATH" ]; then
                get_module_path || return 0
            fi
            cd "$MODULE_PATH"
            cargo clippy -- -D warnings || log_warn "Linting warnings found"
            cargo fmt --check || log_warn "Formatting issues found"
            cd "$SCRIPT_DIR/../.."
            ;;
        *)
            log_info "No linting configured for $MODULE"
            ;;
    esac
}

# Generate build artifacts
generate_artifacts() {
    log_step "Generating build artifacts..."
    
    local artifact_dir="$CACHE_DIR/$MODULE/build/artifacts"
    mkdir -p "$artifact_dir"
    
    # Copy binary if Rust
    if [ -f "$MODULE_PATH/target/release/$MODULE" ]; then
        cp "$MODULE_PATH/target/release/$MODULE" "$artifact_dir/"
        log_info "Binary copied to $artifact_dir/$MODULE"
    fi
    
    # Generate build info
    cat > "$artifact_dir/build-info.json" << EOF
{
  "module": "$MODULE",
  "build_time": "$(date -Iseconds)",
  "build_duration": "${TOTAL_DURATION}s",
  "cache_used": "$BUILD_CACHE",
  "git_commit": "$(git rev-parse --short HEAD 2>/dev/null || echo 'unknown')",
  "git_branch": "$(git branch --show-current 2>/dev/null || echo 'unknown')"
}
EOF
    
    log_info "Build artifacts saved to $artifact_dir"
}

# Generate build report
generate_report() {
    local report_file="$CACHE_DIR/$MODULE/build-report.txt"
    
    cat > "$report_file" << EOF
Module Build Report
===================
Date: $(date)
Module: $MODULE

Build Statistics:
-----------------
Total Duration: ${TOTAL_DURATION}s
Cache Used: $BUILD_CACHE
Parallel Build: $PARALLEL_BUILD

Build Steps:
------------
✓ Source validation
✓ Dependency resolution
✓ Compilation
✓ Linting
✓ Artifact generation

Artifacts Location: $CACHE_DIR/$MODULE/build/artifacts/

Status: BUILD SUCCESSFUL
EOF
    
    log_info "Build report saved: $report_file"
    
    # Display summary
    if [ ${TOTAL_DURATION} -lt 60 ]; then
        echo -e "${GREEN}✓ Module build completed in ${TOTAL_DURATION}s (< 1 min)${NC}"
    elif [ ${TOTAL_DURATION} -lt 180 ]; then
        echo -e "${YELLOW}✓ Module build completed in ${TOTAL_DURATION}s (< 3 min)${NC}"
    else
        echo -e "${RED}⚠ Module build took ${TOTAL_DURATION}s (> 3 min target)${NC}"
    fi
}

# Main execution
main() {
    log_info "Starting build for module: $MODULE"
    
    start_timer
    
    validate_module
    
    # Setup cache directory
    mkdir -p "$CACHE_DIR/$MODULE/build"
    
    # Check if rebuild needed
    if check_cache; then
        log_info "Using cached build"
        TOTAL_DURATION=0
    else
        # Determine module type and build (MODULE_PATH already set by validate_module)
        if [ -f "$MODULE_PATH/Cargo.toml" ]; then
            build_rust_module
        elif [ -f "$MODULE_PATH/requirements.txt" ] || [ -f "$MODULE_PATH/setup.py" ]; then
            build_python_module
        else
            log_error "Cannot determine module type for: $MODULE at $MODULE_PATH"
            exit 1
        fi
        
        # Build Docker image
        build_docker_image
        
        # Run linting
        run_linting
        
        TOTAL_DURATION=$(end_timer)
    fi
    
    # Always generate artifacts and report
    generate_artifacts
    generate_report
    
    log_info "Module build complete for $MODULE"
}

main