#!/bin/bash
# Main Pipeline Runner - Orchestrates complete CI/CD pipeline

set -e

# Configuration
PIPELINE_TYPE=${1:-module}
MODULE=${2:-}
ENV=${CONFIG_ENV:-dev}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_phase() { echo -e "${MAGENTA}[PHASE]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

# Timer
start_timer() { START_TIME=$(date +%s); }
end_timer() { echo $(($(date +%s) - START_TIME)); }

# Pipeline header
print_header() {
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}                    CI/CD PIPELINE EXECUTION                 ${NC}"
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    echo -e "Type: ${PIPELINE_TYPE^^}"
    echo -e "Module: ${MODULE:-ALL}"
    echo -e "Environment: ${ENV}"
    echo -e "Started: $(date '+%Y-%m-%d %H:%M:%S')"
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
}

# Module pipeline (3-minute target)
run_module_pipeline() {
    local module=$1
    
    if [ -z "$module" ]; then
        log_error "Module name required for module pipeline"
        exit 1
    fi
    
    log_phase "MODULE PIPELINE: $module"
    start_timer
    
    # Run integrated module pipeline
    ./scripts/v2/module-integration.sh "$module"
    local result=$?
    
    local duration=$(end_timer)
    
    if [ $duration -le 180 ]; then
        log_info "✅ Module pipeline completed in ${duration}s (< 3 min target)"
    else
        log_warn "⚠️ Module pipeline took ${duration}s (> 3 min target)"
    fi
    
    return $result
}

# Platform pipeline (16-minute target)
run_platform_pipeline() {
    log_phase "PLATFORM PIPELINE: All Services"
    start_timer
    
    local all_passed=true
    local modules=("config-store" "data-ingestion" "data-staging" "neural-ml-ops" "neural-trading")
    
    # Phase 1: Infrastructure
    log_step "1/5 Infrastructure Setup"
    docker-compose -f docker-compose.v2.yml up -d redis timescaledb || all_passed=false
    sleep 5
    
    # Phase 2: Core Services
    log_step "2/5 Core Services"
    docker-compose -f docker-compose.v2.yml up -d config-store || all_passed=false
    ./scripts/v2/wait-for-dependencies.sh config-store || all_passed=false
    
    # Phase 3: Build All
    log_step "3/5 Building All Services"
    for module in "${modules[@]}"; do
        log_info "Building $module..."
        ./scripts/v2/module-build.sh "$module" || all_passed=false
    done
    
    # Phase 4: Test All
    log_step "4/5 Testing All Services"
    for module in "${modules[@]}"; do
        log_info "Testing $module..."
        ./scripts/v2/module-test.sh "$module" unit || all_passed=false
    done
    
    # Phase 5: Integration Tests
    log_step "5/5 Running Integration Tests"
    docker-compose -f docker-compose.v2.yml up -d
    sleep 10
    
    # Run end-to-end tests
    if [ -f "tests/e2e/test_pipeline.py" ]; then
        python3 tests/e2e/test_pipeline.py || all_passed=false
    fi
    
    local duration=$(end_timer)
    
    if [ "$all_passed" = true ] && [ $duration -le 960 ]; then
        log_info "✅ Platform pipeline completed successfully in ${duration}s (< 16 min target)"
    elif [ "$all_passed" = true ]; then
        log_warn "⚠️ Platform pipeline took ${duration}s (> 16 min target)"
    else
        log_error "❌ Platform pipeline failed"
        return 1
    fi
    
    return 0
}

# Quick validation pipeline
run_validation_pipeline() {
    log_phase "VALIDATION PIPELINE"
    
    log_step "Checking Docker images..."
    docker images | grep neural-trader || log_warn "Some images missing"
    
    log_step "Checking service health..."
    ./scripts/v2/wait-for-dependencies.sh all
    
    log_step "Running drift detection..."
    ./scripts/v2/drift-detector.sh
    
    log_step "Generating reports..."
    ./scripts/v2/generate-reports.sh summary
    
    log_info "Validation complete"
}

# Retry logic wrapper
run_with_retry() {
    local max_attempts=3
    local attempt=1
    local command="$@"
    
    while [ $attempt -le $max_attempts ]; do
        log_info "Attempt $attempt/$max_attempts"
        
        if $command; then
            return 0
        fi
        
        if [ $attempt -lt $max_attempts ]; then
            log_warn "Command failed, retrying in 5 seconds..."
            sleep 5
        fi
        
        attempt=$((attempt + 1))
    done
    
    log_error "Command failed after $max_attempts attempts"
    return 1
}

# Error handling
handle_pipeline_error() {
    local phase=$1
    log_error "Pipeline failed at phase: $phase"
    
    # Generate error report
    cat > /tmp/pipeline-error.txt << EOF
Pipeline Error Report
=====================
Date: $(date)
Type: $PIPELINE_TYPE
Module: ${MODULE:-ALL}
Phase: $phase
Environment: $ENV

Error Details:
--------------
Check logs for more information.

Recovery Steps:
---------------
1. Check service logs: docker-compose logs
2. Verify dependencies: ./scripts/v2/wait-for-dependencies.sh all
3. Clean and retry: docker-compose down && docker-compose up
EOF
    
    log_error "Error report saved to /tmp/pipeline-error.txt"
    exit 1
}

# Generate final report
generate_pipeline_report() {
    local duration=$1
    local status=$2
    
    cat > /tmp/pipeline-report.txt << EOF
================================================================================
                          PIPELINE EXECUTION REPORT
================================================================================
Pipeline Type: $PIPELINE_TYPE
Module: ${MODULE:-ALL}
Environment: $ENV
Status: $status
Duration: ${duration}s

Execution Summary:
------------------
$([ "$PIPELINE_TYPE" = "module" ] && echo "Target: < 3 minutes (180s)" || echo "Target: < 16 minutes (960s)")
$([ $duration -le $([ "$PIPELINE_TYPE" = "module" ] && echo 180 || echo 960) ] && echo "✅ Met target" || echo "⚠️ Exceeded target")

Artifacts Generated:
--------------------
- Build artifacts: /tmp/module-cache/*/build/artifacts/
- Test results: /tmp/module-cache/*/test/
- Coverage reports: /tmp/module-cache/*/coverage/
- Pipeline logs: /tmp/pipeline.log

Next Steps:
-----------
$([ "$status" = "SUCCESS" ] && echo "1. Review test coverage
2. Check performance metrics
3. Deploy if approved" || echo "1. Check error logs
2. Fix failing tests
3. Re-run pipeline")

================================================================================
EOF
    
    log_info "Pipeline report saved to /tmp/pipeline-report.txt"
    cat /tmp/pipeline-report.txt
}

# Main execution
main() {
    print_header
    
    # Start logging
    exec 2>&1 | tee /tmp/pipeline.log
    
    start_timer
    PIPELINE_STATUS="SUCCESS"
    
    case $PIPELINE_TYPE in
        module)
            run_with_retry run_module_pipeline "$MODULE" || {
                PIPELINE_STATUS="FAILED"
                handle_pipeline_error "module"
            }
            ;;
        platform)
            run_with_retry run_platform_pipeline || {
                PIPELINE_STATUS="FAILED"
                handle_pipeline_error "platform"
            }
            ;;
        validation)
            run_validation_pipeline || {
                PIPELINE_STATUS="WARNING"
            }
            ;;
        *)
            log_error "Unknown pipeline type: $PIPELINE_TYPE"
            echo "Usage: $0 [module|platform|validation] [module-name]"
            exit 1
            ;;
    esac
    
    TOTAL_DURATION=$(end_timer)
    
    # Generate final report
    generate_pipeline_report $TOTAL_DURATION $PIPELINE_STATUS
    
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}                    PIPELINE EXECUTION COMPLETE              ${NC}"
    echo -e "${CYAN}════════════════════════════════════════════════════════════${NC}"
    
    [ "$PIPELINE_STATUS" = "SUCCESS" ] && exit 0 || exit 1
}

# Run pipeline
main