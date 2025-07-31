#!/bin/bash
#
# Phase 3 Test Execution Script
# Runs Phase 3A (completion) tests first, then Phase 3B (integration) tests
#
# Usage: ./tests/run_phase3_tests.sh [options]
# Options:
#   --phase3a-only    Run only Phase 3A tests
#   --phase3b-only    Run only Phase 3B tests (requires 3A to have passed)
#   --verbose         Show detailed test output
#   --coverage        Generate coverage report

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PHASE3A_ONLY=false
PHASE3B_ONLY=false
VERBOSE=false
COVERAGE=false
PHASE3A_PASSED_MARKER=".phase3a_passed"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --phase3a-only)
            PHASE3A_ONLY=true
            shift
            ;;
        --phase3b-only)
            PHASE3B_ONLY=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --coverage)
            COVERAGE=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Helper functions
print_header() {
    echo -e "\n${BLUE}===================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}===================================================${NC}\n"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

# Check prerequisites
check_prerequisites() {
    print_header "Checking Prerequisites"
    
    # Check Rust toolchain
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo not found. Please install Rust toolchain."
        exit 1
    fi
    print_success "Rust toolchain found"
    
    # Check if in project root
    if [ ! -f "Cargo.toml" ]; then
        print_error "Not in project root. Please run from neural-trader directory."
        exit 1
    fi
    print_success "In project root"
    
    # Check if Phase 3B requested without 3A completion
    if [ "$PHASE3B_ONLY" = true ] && [ ! -f "$PHASE3A_PASSED_MARKER" ]; then
        print_error "Phase 3A must pass before running Phase 3B tests"
        print_info "Run without --phase3b-only to run both phases"
        exit 1
    fi
}

# Run Phase 3A tests
run_phase3a_tests() {
    print_header "Phase 3A: Implementation Completion Tests"
    
    local start_time=$(date +%s)
    local test_args=""
    
    if [ "$VERBOSE" = true ]; then
        test_args="-- --nocapture"
    fi
    
    # Module structure tests
    print_info "Testing module structure..."
    if cargo test --test phase3a_completion_tests module_validation $test_args; then
        print_success "Module structure validated"
    else
        print_error "Module structure validation failed"
        return 1
    fi
    
    # Compilation tests
    print_info "Testing compilation with features..."
    if cargo test --all-features --test phase3a_completion_tests compilation_tests $test_args; then
        print_success "Compilation tests passed"
    else
        print_error "Compilation tests failed"
        return 1
    fi
    
    # Performance channel tests
    print_info "Testing performance channel..."
    if cargo test --test phase3a_completion_tests performance_channel_tests $test_args; then
        print_success "Performance channel tests passed"
    else
        print_error "Performance channel tests failed"
        return 1
    fi
    
    # Training notification tests
    print_info "Testing training notification system..."
    if cargo test --test phase3a_completion_tests training_notification_tests $test_args; then
        print_success "Training notification tests passed"
    else
        print_error "Training notification tests failed"
        return 1
    fi
    
    # Integration readiness tests
    print_info "Testing integration readiness..."
    if cargo test --test phase3a_completion_tests integration_readiness_tests $test_args; then
        print_success "Integration readiness tests passed"
    else
        print_error "Integration readiness tests failed"
        return 1
    fi
    
    # Run comprehensive validation
    print_info "Running comprehensive Phase 3A validation..."
    if cargo test --test phase3a_completion_tests test_phase3a_complete_validation $test_args; then
        print_success "Phase 3A comprehensive validation passed"
    else
        print_error "Phase 3A comprehensive validation failed"
        return 1
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    print_success "Phase 3A completed in ${duration} seconds"
    
    # Mark Phase 3A as passed
    touch "$PHASE3A_PASSED_MARKER"
    
    return 0
}

# Run Phase 3B tests
run_phase3b_tests() {
    print_header "Phase 3B: System Integration Tests"
    
    local start_time=$(date +%s)
    local test_args=""
    
    if [ "$VERBOSE" = true ]; then
        test_args="-- --nocapture"
    fi
    
    # Market timing integration
    print_info "Testing market timing integration..."
    if cargo test --test phase3b_integration_tests market_timing_tests $test_args; then
        print_success "Market timing integration passed"
    else
        print_error "Market timing integration failed"
        return 1
    fi
    
    # Performance flow tests
    print_info "Testing performance event flow..."
    if cargo test --test phase3b_integration_tests performance_flow_tests $test_args; then
        print_success "Performance event flow tests passed"
    else
        print_error "Performance event flow tests failed"
        return 1
    fi
    
    # Training trigger tests
    print_info "Testing training triggers..."
    if cargo test --test phase3b_integration_tests training_trigger_tests $test_args; then
        print_success "Training trigger tests passed"
    else
        print_error "Training trigger tests failed"
        return 1
    fi
    
    # End-to-end tests
    print_info "Testing end-to-end system behavior..."
    if cargo test --test phase3b_integration_tests end_to_end_tests $test_args; then
        print_success "End-to-end tests passed"
    else
        print_error "End-to-end tests failed"
        return 1
    fi
    
    # Run comprehensive integration test
    print_info "Running comprehensive Phase 3B integration..."
    if cargo test --test phase3b_integration_tests test_phase3b_complete_integration $test_args; then
        print_success "Phase 3B comprehensive integration passed"
    else
        print_error "Phase 3B comprehensive integration failed"
        return 1
    fi
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    print_success "Phase 3B completed in ${duration} seconds"
    
    return 0
}

# Generate coverage report
generate_coverage_report() {
    print_header "Generating Coverage Report"
    
    if ! command -v cargo-tarpaulin &> /dev/null; then
        print_warning "cargo-tarpaulin not installed. Installing..."
        cargo install cargo-tarpaulin
    fi
    
    print_info "Running tests with coverage..."
    cargo tarpaulin \
        --out Html \
        --output-dir target/coverage \
        --exclude-files "tests/*" \
        --ignore-panics \
        --timeout 300 \
        --features "performance-monitoring health-monitoring fallback"
    
    print_success "Coverage report generated at target/coverage/tarpaulin-report.html"
}

# Main execution
main() {
    print_header "Neural Trader Phase 3 Test Suite"
    
    check_prerequisites
    
    local phase3a_passed=false
    local phase3b_passed=false
    local overall_start=$(date +%s)
    
    # Run Phase 3A if needed
    if [ "$PHASE3B_ONLY" = false ]; then
        if run_phase3a_tests; then
            phase3a_passed=true
        else
            print_error "Phase 3A failed - cannot proceed to Phase 3B"
            rm -f "$PHASE3A_PASSED_MARKER"
            exit 1
        fi
    else
        phase3a_passed=true  # Assume passed if only running 3B
    fi
    
    # Run Phase 3B if needed
    if [ "$PHASE3A_ONLY" = false ] && [ "$phase3a_passed" = true ]; then
        if run_phase3b_tests; then
            phase3b_passed=true
        else
            print_error "Phase 3B failed"
            exit 1
        fi
    fi
    
    # Generate coverage if requested
    if [ "$COVERAGE" = true ]; then
        generate_coverage_report
    fi
    
    local overall_end=$(date +%s)
    local overall_duration=$((overall_end - overall_start))
    
    # Final summary
    print_header "Test Execution Summary"
    
    if [ "$PHASE3B_ONLY" = false ]; then
        if [ "$phase3a_passed" = true ]; then
            print_success "Phase 3A: PASSED"
        else
            print_error "Phase 3A: FAILED"
        fi
    fi
    
    if [ "$PHASE3A_ONLY" = false ]; then
        if [ "$phase3b_passed" = true ]; then
            print_success "Phase 3B: PASSED"
        else
            print_error "Phase 3B: FAILED"
        fi
    fi
    
    print_info "Total execution time: ${overall_duration} seconds"
    
    if [ "$phase3a_passed" = true ] && [ "$phase3b_passed" = true ]; then
        print_success "🎉 All Phase 3 tests passed! System is ready for deployment."
    elif [ "$PHASE3A_ONLY" = true ] && [ "$phase3a_passed" = true ]; then
        print_success "🎉 Phase 3A complete! Ready for integration testing."
    elif [ "$PHASE3B_ONLY" = true ] && [ "$phase3b_passed" = true ]; then
        print_success "🎉 Phase 3B integration tests passed!"
    fi
}

# Run main
main