#!/bin/bash
# Module Integration Script - Run complete module pipeline

set -e

# Configuration
MODULE=${1:-}
SKIP_BUILD=${SKIP_BUILD:-false}
SKIP_TESTS=${SKIP_TESTS:-false}
CACHE_DIR=${MODULE_CACHE_DIR:-/tmp/module-cache}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_phase() { echo -e "${MAGENTA}[PHASE]${NC} $1"; }

# Timer functions
start_timer() {
    START_TIME=$(date +%s)
}

end_timer() {
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    echo $DURATION
}

# Validate module
validate_module() {
    if [ -z "$MODULE" ]; then
        log_error "Module name required"
        echo "Usage: $0 <module-name>"
        exit 1
    fi
}

# Run module pipeline phases
run_setup_phase() {
    log_phase "1/4 SETUP - Preparing environment"
    start_timer
    
    ./scripts/v2/module-setup.sh "$MODULE"
    local result=$?
    
    SETUP_DURATION=$(end_timer)
    log_info "Setup completed in ${SETUP_DURATION}s"
    
    return $result
}

run_build_phase() {
    if [ "$SKIP_BUILD" = "true" ]; then
        log_info "Skipping build phase"
        BUILD_DURATION=0
        return 0
    fi
    
    log_phase "2/4 BUILD - Compiling module"
    start_timer
    
    ./scripts/v2/module-build.sh "$MODULE"
    local result=$?
    
    BUILD_DURATION=$(end_timer)
    log_info "Build completed in ${BUILD_DURATION}s"
    
    return $result
}

run_test_phase() {
    if [ "$SKIP_TESTS" = "true" ]; then
        log_info "Skipping test phase"
        TEST_DURATION=0
        return 0
    fi
    
    log_phase "3/4 TEST - Running tests"
    start_timer
    
    ./scripts/v2/module-test.sh "$MODULE" all
    local result=$?
    
    TEST_DURATION=$(end_timer)
    log_info "Tests completed in ${TEST_DURATION}s"
    
    return $result
}

run_report_phase() {
    log_phase "4/4 REPORT - Generating reports"
    start_timer
    
    ./scripts/v2/module-report.sh "$MODULE"
    local result=$?
    
    REPORT_DURATION=$(end_timer)
    log_info "Reports generated in ${REPORT_DURATION}s"
    
    return $result
}

# Generate integration summary
generate_summary() {
    local total_duration=$((SETUP_DURATION + BUILD_DURATION + TEST_DURATION + REPORT_DURATION))
    local summary_file="$CACHE_DIR/$MODULE/integration-summary.txt"
    
    cat > "$summary_file" << EOF
================================================================================
                        MODULE PIPELINE INTEGRATION SUMMARY
================================================================================
Module: $MODULE
Date: $(date)

Pipeline Phases:
----------------
1. SETUP:  ${SETUP_DURATION}s ${SETUP_STATUS}
2. BUILD:  ${BUILD_DURATION}s ${BUILD_STATUS}
3. TEST:   ${TEST_DURATION}s ${TEST_STATUS}
4. REPORT: ${REPORT_DURATION}s ${REPORT_STATUS}

Total Duration: ${total_duration}s
Target: < 180s (3 minutes)

Performance Analysis:
---------------------
EOF
    
    if [ $total_duration -lt 180 ]; then
        echo "✅ PASSED: Pipeline completed within 3-minute target!" >> "$summary_file"
        echo -e "${GREEN}✅ EXCELLENT: Module pipeline completed in ${total_duration}s (< 3 min)${NC}"
    elif [ $total_duration -lt 240 ]; then
        echo "⚠️  WARNING: Pipeline exceeded target by $((total_duration - 180))s" >> "$summary_file"
        echo -e "${YELLOW}⚠️  WARNING: Module pipeline took ${total_duration}s (> 3 min target)${NC}"
    else
        echo "❌ FAILED: Pipeline significantly exceeded target" >> "$summary_file"
        echo -e "${RED}❌ FAILED: Module pipeline took ${total_duration}s (way over 3 min target)${NC}"
    fi
    
    cat >> "$summary_file" << EOF

Artifacts Generated:
--------------------
✓ Setup Report:    $CACHE_DIR/$MODULE/setup-report.txt
✓ Build Artifacts: $CACHE_DIR/$MODULE/build/artifacts/
✓ Test Results:    $CACHE_DIR/$MODULE/test/
✓ Coverage Report: $CACHE_DIR/$MODULE/coverage/
✓ Final Report:    $CACHE_DIR/$MODULE/report/

Next Steps:
-----------
1. Review test coverage at: $CACHE_DIR/$MODULE/coverage/index.html
2. Check detailed reports in: $CACHE_DIR/$MODULE/report/
3. Deploy if all checks passed

================================================================================
EOF
    
    log_info "Integration summary saved: $summary_file"
    cat "$summary_file"
}

# Error handler
handle_error() {
    local phase=$1
    local status_var="${phase}_STATUS"
    eval "$status_var='❌ FAILED'"
    
    log_error "$phase phase failed"
    
    # Continue with report generation even on failure
    if [ "$phase" != "REPORT" ]; then
        run_report_phase
    fi
    
    generate_summary
    exit 1
}

# Main execution
main() {
    echo -e "${MAGENTA}========================================${NC}"
    echo -e "${MAGENTA}    MODULE PIPELINE INTEGRATION${NC}"
    echo -e "${MAGENTA}========================================${NC}"
    log_info "Starting integrated pipeline for: $MODULE"
    
    validate_module
    
    # Initialize status indicators
    SETUP_STATUS="✅ PASSED"
    BUILD_STATUS="✅ PASSED"
    TEST_STATUS="✅ PASSED"
    REPORT_STATUS="✅ PASSED"
    
    # Run pipeline phases
    run_setup_phase || handle_error "SETUP"
    run_build_phase || handle_error "BUILD"
    run_test_phase || handle_error "TEST"
    run_report_phase || handle_error "REPORT"
    
    # Generate final summary
    generate_summary
    
    log_info "Module pipeline integration complete for $MODULE"
    
    # Return success
    exit 0
}

main