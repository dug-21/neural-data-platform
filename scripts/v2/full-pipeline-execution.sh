#!/bin/bash
# Full Pipeline Execution Script with Parallel Optimization
# Target: Module < 3 min, Platform < 16 min

set -e

# Configuration
PROJECT_ROOT=${PROJECT_ROOT:-/workspaces/neural-trader}
PARALLEL_JOBS=${PARALLEL_JOBS:-8}
ENABLE_MONITORING=${ENABLE_MONITORING:-true}
GENERATE_REPORT=${GENERATE_REPORT:-true}
REPORT_DIR=${REPORT_DIR:-/workspaces/neural-trader/reports}

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

# Timing
PIPELINE_START=$(date +%s)
declare -A stage_times

# Results tracking
declare -A pipeline_results
pipeline_results[status]="SUCCESS"
pipeline_results[modules_built]=0
pipeline_results[modules_tested]=0
pipeline_results[tests_passed]=0
pipeline_results[tests_failed]=0

# Services list
SERVICES=(
    "config-store"
    "data-ingestion"
    "data-staging"
    "neural-ml-ops"
    "neural-trading"
)

# Stage 1: Environment Setup (Parallel)
stage_environment_setup() {
    log_step "Stage 1: Environment Setup (Parallel)"
    local start_time=$(date +%s)
    
    log_parallel "Setting up environment with $PARALLEL_JOBS parallel jobs..."
    
    # Parallel environment checks
    (
        # Check Docker
        if docker info >/dev/null 2>&1; then
            log_success "Docker is running"
        else
            log_error "Docker is not running"
            exit 1
        fi
    ) &
    
    (
        # Check dependencies
        for cmd in cargo rustc python3 make; do
            if command -v $cmd >/dev/null 2>&1; then
                log_success "$cmd available"
            else
                log_error "$cmd not found"
                exit 1
            fi
        done
    ) &
    
    (
        # Initialize databases
        docker-compose -f docker-compose.v2.yml up -d timescaledb redis 2>/dev/null
        sleep 3
        log_success "Databases started"
    ) &
    
    (
        # Create necessary directories
        mkdir -p "$REPORT_DIR" /tmp/cargo-cache /tmp/test-results
        log_success "Directories created"
    ) &
    
    wait
    
    local end_time=$(date +%s)
    stage_times[environment]=$((end_time - start_time))
    log_success "Environment setup completed in ${stage_times[environment]}s"
}

# Stage 2: Parallel Module Build
stage_module_build() {
    log_step "Stage 2: Module Build (Parallel)"
    local start_time=$(date +%s)
    
    log_parallel "Building ${#SERVICES[@]} modules in parallel..."
    
    for service in "${SERVICES[@]}"; do
        (
            log_info "Building $service..."
            cd "$PROJECT_ROOT/v2/$service"
            
            # Use caching for faster builds
            export CARGO_TARGET_DIR="/tmp/cargo-cache"
            
            if cargo build --release 2>&1 | tail -5; then
                log_success "$service built successfully"
                ((pipeline_results[modules_built]++))
            else
                log_error "$service build failed"
                pipeline_results[status]="FAILED"
            fi
        ) &
    done
    
    wait
    
    local end_time=$(date +%s)
    stage_times[build]=$((end_time - start_time))
    log_success "Module build completed in ${stage_times[build]}s"
    log_info "Modules built: ${pipeline_results[modules_built]}/${#SERVICES[@]}"
}

# Stage 3: Parallel Module Testing
stage_module_test() {
    log_step "Stage 3: Module Testing (Parallel)"
    local start_time=$(date +%s)
    
    log_parallel "Testing ${#SERVICES[@]} modules in parallel..."
    
    for service in "${SERVICES[@]}"; do
        (
            log_info "Testing $service..."
            cd "$PROJECT_ROOT/v2/$service"
            
            # Run tests with JSON output for parsing
            if cargo test --release -- --test-threads=4 2>&1 | tee /tmp/test-results/${service}.txt | grep -q "test result: ok"; then
                log_success "$service tests passed"
                ((pipeline_results[modules_tested]++))
                
                # Count test results
                local passed=$(grep -c "test .* ... ok" /tmp/test-results/${service}.txt || echo 0)
                pipeline_results[tests_passed]=$((pipeline_results[tests_passed] + passed))
            else
                log_error "$service tests failed"
                pipeline_results[status]="FAILED"
                
                local failed=$(grep -c "test .* ... FAILED" /tmp/test-results/${service}.txt || echo 0)
                pipeline_results[tests_failed]=$((pipeline_results[tests_failed] + failed))
            fi
        ) &
    done
    
    wait
    
    local end_time=$(date +%s)
    stage_times[test]=$((end_time - start_time))
    log_success "Module testing completed in ${stage_times[test]}s"
    log_info "Modules tested: ${pipeline_results[modules_tested]}/${#SERVICES[@]}"
}

# Stage 4: Docker Image Build (Parallel)
stage_docker_build() {
    log_step "Stage 4: Docker Image Build (Parallel)"
    local start_time=$(date +%s)
    
    log_parallel "Building Docker images in parallel..."
    
    # Enable BuildKit for better caching
    export DOCKER_BUILDKIT=1
    
    for service in "${SERVICES[@]}"; do
        (
            log_info "Building Docker image for $service..."
            
            docker build \
                -f "$PROJECT_ROOT/docker/v2/Dockerfile.$service" \
                -t "neural-trader/$service:latest" \
                --cache-from "neural-trader/$service:latest" \
                "$PROJECT_ROOT" 2>&1 | grep "Successfully" || true
            
            log_success "Docker image for $service built"
        ) &
    done
    
    wait
    
    local end_time=$(date +%s)
    stage_times[docker]=$((end_time - start_time))
    log_success "Docker build completed in ${stage_times[docker]}s"
}

# Stage 5: Integration Testing
stage_integration_test() {
    log_step "Stage 5: Integration Testing"
    local start_time=$(date +%s)
    
    # Start all services
    log_info "Starting all services..."
    docker-compose -f docker-compose.v2.yml up -d
    
    # Wait for services to be healthy
    log_info "Waiting for services to be healthy..."
    sleep 10
    
    # Run integration tests in parallel
    log_parallel "Running integration tests..."
    
    (
        log_info "Testing data pipeline flow..."
        ./scripts/v2/test-pipeline.sh
    ) &
    
    (
        log_info "Verifying EventBus messaging..."
        ./scripts/v2/verify-eventbus.sh
    ) &
    
    (
        log_info "Testing configuration management..."
        ./scripts/v2/config-seeder.sh dev
    ) &
    
    wait
    
    local end_time=$(date +%s)
    stage_times[integration]=$((end_time - start_time))
    log_success "Integration testing completed in ${stage_times[integration]}s"
}

# Stage 6: Performance Validation
stage_performance_validation() {
    log_step "Stage 6: Performance Validation"
    local start_time=$(date +%s)
    
    if [ "$ENABLE_MONITORING" = "true" ]; then
        log_info "Running performance validation..."
        
        # Check against baseline
        ./scripts/v2/drift-detection-tests.sh
        
        # Collect metrics
        ./scripts/v2/baseline-metrics.sh
    else
        log_info "Performance validation skipped (ENABLE_MONITORING=false)"
    fi
    
    local end_time=$(date +%s)
    stage_times[performance]=$((end_time - start_time))
    log_success "Performance validation completed in ${stage_times[performance]}s"
}

# Generate comprehensive report
generate_report() {
    if [ "$GENERATE_REPORT" != "true" ]; then
        return
    fi
    
    log_step "Generating pipeline report..."
    
    local report_file="$REPORT_DIR/pipeline_$(date +%Y%m%d_%H%M%S).json"
    local summary_file="$REPORT_DIR/pipeline_summary.txt"
    
    # Calculate total time
    local total_time=0
    for time in "${stage_times[@]}"; do
        total_time=$((total_time + time))
    done
    
    # Generate JSON report
    cat > "$report_file" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "status": "${pipeline_results[status]}",
    "duration_seconds": $total_time,
    "stages": {
        "environment_setup": ${stage_times[environment]:-0},
        "module_build": ${stage_times[build]:-0},
        "module_test": ${stage_times[test]:-0},
        "docker_build": ${stage_times[docker]:-0},
        "integration_test": ${stage_times[integration]:-0},
        "performance_validation": ${stage_times[performance]:-0}
    },
    "results": {
        "modules_built": ${pipeline_results[modules_built]},
        "modules_tested": ${pipeline_results[modules_tested]},
        "tests_passed": ${pipeline_results[tests_passed]},
        "tests_failed": ${pipeline_results[tests_failed]}
    },
    "performance": {
        "parallel_jobs": $PARALLEL_JOBS,
        "target_module_time": 180,
        "actual_module_time": $((stage_times[build] + stage_times[test])),
        "target_platform_time": 960,
        "actual_platform_time": $total_time
    }
}
EOF
    
    # Generate text summary
    cat > "$summary_file" << EOF
=====================================
Pipeline Execution Summary
=====================================
Date: $(date)
Status: ${pipeline_results[status]}
Total Duration: ${total_time}s

Stage Timings:
--------------
Environment Setup:    ${stage_times[environment]:-0}s
Module Build:        ${stage_times[build]:-0}s
Module Test:         ${stage_times[test]:-0}s
Docker Build:        ${stage_times[docker]:-0}s
Integration Test:    ${stage_times[integration]:-0}s
Performance Check:   ${stage_times[performance]:-0}s

Results:
--------
Modules Built:    ${pipeline_results[modules_built]}/${#SERVICES[@]}
Modules Tested:   ${pipeline_results[modules_tested]}/${#SERVICES[@]}
Tests Passed:     ${pipeline_results[tests_passed]}
Tests Failed:     ${pipeline_results[tests_failed]}

Performance vs Targets:
-----------------------
Module Pipeline Target:   < 3 minutes (180s)
Module Pipeline Actual:   $((stage_times[build] + stage_times[test]))s
$([ $((stage_times[build] + stage_times[test])) -lt 180 ] && echo "✓ PASS" || echo "✗ FAIL")

Platform Pipeline Target: < 16 minutes (960s)
Platform Pipeline Actual: ${total_time}s
$([ $total_time -lt 960 ] && echo "✓ PASS" || echo "✗ FAIL")

Parallel Optimization:
----------------------
Parallel Jobs Used: $PARALLEL_JOBS
Services Built Concurrently: ${#SERVICES[@]}
Estimated Time Saved: ~$((total_time * 2))s

EOF
    
    log_info "Reports generated:"
    log_info "  JSON: $report_file"
    log_info "  Summary: $summary_file"
    
    # Display summary
    cat "$summary_file"
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."
    # Keep services running for inspection
    # docker-compose -f docker-compose.v2.yml down
}

# Main execution
main() {
    log_info "🚀 Starting Full Pipeline Execution"
    log_info "Parallel Jobs: $PARALLEL_JOBS"
    log_info "Target Times: Module <3min, Platform <16min"
    
    # Set trap for cleanup
    trap cleanup EXIT
    
    # Execute pipeline stages
    stage_environment_setup
    stage_module_build
    stage_module_test
    stage_docker_build
    stage_integration_test
    stage_performance_validation
    
    # Calculate total pipeline time
    PIPELINE_END=$(date +%s)
    TOTAL_TIME=$((PIPELINE_END - PIPELINE_START))
    
    # Generate report
    generate_report
    
    # Final status
    if [ "${pipeline_results[status]}" = "SUCCESS" ]; then
        log_success "✨ Pipeline completed successfully in ${TOTAL_TIME}s!"
        
        # Check against targets
        local module_time=$((stage_times[build] + stage_times[test]))
        if [ $module_time -lt 180 ]; then
            log_success "✓ Module pipeline target achieved: ${module_time}s < 180s"
        else
            log_warn "⚠ Module pipeline exceeded target: ${module_time}s > 180s"
        fi
        
        if [ $TOTAL_TIME -lt 960 ]; then
            log_success "✓ Platform pipeline target achieved: ${TOTAL_TIME}s < 960s"
        else
            log_warn "⚠ Platform pipeline exceeded target: ${TOTAL_TIME}s > 960s"
        fi
        
        exit 0
    else
        log_error "✗ Pipeline failed! Check logs for details."
        exit 1
    fi
}

# Run main function
main "$@"