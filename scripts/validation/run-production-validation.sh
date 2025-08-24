#!/bin/bash
# Production Validation Runner Script
# ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS
#
# This script provides local execution of the Production Validation Framework
# Use before pushing to ensure CI/CD pipeline will pass

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
VALIDATION_BIN="$PROJECT_ROOT/target/release/production-validator"

# Default values
VALIDATOR=""
MODE="development"
FAIL_FAST=true
VERBOSE=false
REPORT_FORMAT="console"
OUTPUT_DIR="$PROJECT_ROOT/target/validation-results"

# Quality Gate Thresholds
MIN_COVERAGE_THRESHOLD=95
MAX_ALLOWED_TODOS=0
MAX_ALLOWED_STUBS=0

print_banner() {
    echo -e "${BLUE}${BOLD}"
    echo "=================================================================="
    echo "  Phase 3 Production Validation Framework"
    echo "  ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS"
    echo "=================================================================="
    echo -e "${NC}"
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "OPTIONS:"
    echo "  -v, --validator VALIDATOR    Run specific validator (code-completeness,"
    echo "                               interface-contract, test-coverage,"
    echo "                               performance-benchmark, security-standards, all)"
    echo "  -m, --mode MODE             Execution mode (development|staging|production)"
    echo "  -r, --report FORMAT         Report format (console|json|html|markdown)"
    echo "  -o, --output DIR            Output directory for reports"
    echo "  --no-fail-fast              Continue on first failure"
    echo "  --verbose                   Enable verbose output"
    echo "  --dry-run                   Show what would be executed without running"
    echo "  -h, --help                  Show this help message"
    echo ""
    echo "EXAMPLES:"
    echo "  $0 --validator=all --mode=production"
    echo "  $0 --validator=code-completeness --verbose"
    echo "  $0 --validator=test-coverage --report=html --output=./reports"
    echo ""
    echo "VALIDATORS:"
    echo "  code-completeness    - Check for TODOs, stubs, incomplete implementations"
    echo "  interface-contract   - Validate gRPC services and Redis Streams contracts"
    echo "  test-coverage       - Enforce 95% minimum coverage across all binaries"
    echo "  performance-benchmark - Validate SLA compliance for all binaries"
    echo "  security-standards  - OWASP and NIST security compliance checks"
    echo "  all                 - Run all validators in sequence"
}

log() {
    local level=$1
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    case $level in
        "INFO")
            echo -e "${BLUE}[$timestamp] [INFO]${NC} $message"
            ;;
        "SUCCESS")
            echo -e "${GREEN}[$timestamp] [SUCCESS]${NC} $message"
            ;;
        "WARNING")
            echo -e "${YELLOW}[$timestamp] [WARNING]${NC} $message"
            ;;
        "ERROR")
            echo -e "${RED}[$timestamp] [ERROR]${NC} $message"
            ;;
        "CRITICAL")
            echo -e "${RED}${BOLD}[$timestamp] [CRITICAL]${NC} $message"
            ;;
    esac
}

check_prerequisites() {
    log "INFO" "Checking prerequisites..."
    
    # Check if we're in the project root
    if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
        log "ERROR" "Not in Neural Trader project root directory"
        exit 1
    fi
    
    # Check Rust installation
    if ! command -v cargo &> /dev/null; then
        log "ERROR" "Cargo not found. Please install Rust toolchain"
        exit 1
    fi
    
    # Check Python installation (for data-ingestion binary)
    if ! command -v python3 &> /dev/null; then
        log "ERROR" "Python3 not found. Required for data-ingestion validation"
        exit 1
    fi
    
    # Check required tools
    local required_tools=("git" "jq")
    for tool in "${required_tools[@]}"; do
        if ! command -v "$tool" &> /dev/null; then
            log "ERROR" "Required tool '$tool' not found"
            exit 1
        fi
    done
    
    log "SUCCESS" "All prerequisites satisfied"
}

build_validator() {
    log "INFO" "Building production validator..."
    
    cd "$PROJECT_ROOT"
    
    if [ ! -f "$VALIDATION_BIN" ] || [ "src/orchestrator/validators" -nt "$VALIDATION_BIN" ]; then
        log "INFO" "Building validator binary..."
        cargo build --release --bin production-validator
        
        if [ $? -ne 0 ]; then
            log "CRITICAL" "Failed to build production validator"
            exit 1
        fi
        
        log "SUCCESS" "Production validator built successfully"
    else
        log "INFO" "Using existing validator binary"
    fi
}

prepare_environment() {
    log "INFO" "Preparing validation environment..."
    
    # Create output directory
    mkdir -p "$OUTPUT_DIR"
    
    # Set environment variables
    export RUST_BACKTRACE=1
    export CARGO_TERM_COLOR=always
    
    # Check git status
    if [ "$MODE" = "production" ]; then
        if [ -n "$(git status --porcelain)" ]; then
            log "WARNING" "Working directory has uncommitted changes"
            if [ "$FAIL_FAST" = true ]; then
                log "CRITICAL" "Production mode requires clean working directory"
                exit 1
            fi
        fi
    fi
    
    log "SUCCESS" "Environment prepared"
}

run_validator() {
    local validator_name="$1"
    log "INFO" "Running $validator_name validator..."
    
    local cmd_args=()
    cmd_args+=("--validator=$validator_name")
    cmd_args+=("--mode=$MODE")
    cmd_args+=("--report=$REPORT_FORMAT")
    cmd_args+=("--output=$OUTPUT_DIR")
    
    if [ "$FAIL_FAST" = true ]; then
        cmd_args+=("--fail-fast")
    fi
    
    if [ "$VERBOSE" = true ]; then
        cmd_args+=("--verbose")
    fi
    
    log "INFO" "Command: $VALIDATION_BIN ${cmd_args[*]}"
    
    if "$VALIDATION_BIN" "${cmd_args[@]}"; then
        log "SUCCESS" "$validator_name validation PASSED"
        return 0
    else
        log "CRITICAL" "$validator_name validation FAILED"
        
        if [ "$FAIL_FAST" = true ]; then
            log "ERROR" "Stopping execution due to --fail-fast"
            exit 1
        fi
        return 1
    fi
}

run_all_validators() {
    log "INFO" "Running all production validators..."
    
    local validators=("code-completeness" "interface-contract" "test-coverage" "performance-benchmark" "security-standards")
    local failed_validators=()
    
    for validator in "${validators[@]}"; do
        if ! run_validator "$validator"; then
            failed_validators+=("$validator")
        fi
        echo "" # Add spacing between validators
    done
    
    if [ ${#failed_validators[@]} -eq 0 ]; then
        log "SUCCESS" "ALL VALIDATORS PASSED - Code is production ready!"
        return 0
    else
        log "CRITICAL" "Failed validators: ${failed_validators[*]}"
        log "ERROR" "Code is NOT production ready"
        return 1
    fi
}

generate_summary_report() {
    log "INFO" "Generating validation summary report..."
    
    local report_file="$OUTPUT_DIR/validation-summary.md"
    local timestamp=$(date -u '+%Y-%m-%d %H:%M:%S UTC')
    local git_commit=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
    local git_branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
    
    cat > "$report_file" << EOF
# Phase 3 Production Validation Summary

**Generated:** $timestamp  
**Commit:** $git_commit  
**Branch:** $git_branch  
**Mode:** $MODE  
**Validator:** $VALIDATOR  

## 🎯 Validation Results

EOF
    
    # Add results from individual validator reports
    if [ -f "$OUTPUT_DIR/validation-results.json" ]; then
        local overall_status=$(jq -r '.overall_status // "unknown"' "$OUTPUT_DIR/validation-results.json")
        local passed_count=$(jq -r '.summary.passed_count // 0' "$OUTPUT_DIR/validation-results.json")
        local failed_count=$(jq -r '.summary.failed_count // 0' "$OUTPUT_DIR/validation-results.json")
        
        echo "**Overall Status:** $overall_status" >> "$report_file"
        echo "**Passed Validators:** $passed_count" >> "$report_file"
        echo "**Failed Validators:** $failed_count" >> "$report_file"
        echo "" >> "$report_file"
        
        if [ "$overall_status" = "PASSED" ]; then
            echo "### ✅ PRODUCTION DEPLOYMENT APPROVED" >> "$report_file"
            echo "All validation gates have passed. Code is ready for production deployment." >> "$report_file"
        else
            echo "### ❌ PRODUCTION DEPLOYMENT BLOCKED" >> "$report_file"
            echo "One or more validation gates failed. Address all issues before deployment." >> "$report_file"
        fi
    fi
    
    echo "" >> "$report_file"
    echo "---" >> "$report_file"
    echo "*Report generated by Phase 3 Production Validation Framework*" >> "$report_file"
    
    log "SUCCESS" "Summary report generated: $report_file"
}

cleanup() {
    log "INFO" "Cleaning up temporary files..."
    # Clean up any temporary files if needed
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--validator)
            VALIDATOR="$2"
            shift 2
            ;;
        -m|--mode)
            MODE="$2"
            shift 2
            ;;
        -r|--report)
            REPORT_FORMAT="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --no-fail-fast)
            FAIL_FAST=false
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            log "ERROR" "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# Validate arguments
if [ -z "$VALIDATOR" ]; then
    log "ERROR" "Validator is required. Use --validator or see --help"
    exit 1
fi

valid_validators=("code-completeness" "interface-contract" "test-coverage" "performance-benchmark" "security-standards" "all")
if [[ ! " ${valid_validators[*]} " =~ " ${VALIDATOR} " ]]; then
    log "ERROR" "Invalid validator: $VALIDATOR"
    log "INFO" "Valid validators: ${valid_validators[*]}"
    exit 1
fi

valid_modes=("development" "staging" "production")
if [[ ! " ${valid_modes[*]} " =~ " ${MODE} " ]]; then
    log "ERROR" "Invalid mode: $MODE"
    log "INFO" "Valid modes: ${valid_modes[*]}"
    exit 1
fi

# Main execution
main() {
    print_banner
    
    # Setup trap for cleanup
    trap cleanup EXIT
    
    check_prerequisites
    build_validator
    prepare_environment
    
    # Execute validation
    if [ "$VALIDATOR" = "all" ]; then
        if run_all_validators; then
            log "SUCCESS" "🚀 ALL VALIDATIONS PASSED - PRODUCTION READY!"
            exit_code=0
        else
            log "CRITICAL" "🚨 VALIDATION FAILURES - NOT PRODUCTION READY"
            exit_code=1
        fi
    else
        if run_validator "$VALIDATOR"; then
            log "SUCCESS" "🎯 $VALIDATOR validation PASSED"
            exit_code=0
        else
            log "CRITICAL" "❌ $VALIDATOR validation FAILED"
            exit_code=1
        fi
    fi
    
    generate_summary_report
    
    # Final status message
    if [ $exit_code -eq 0 ]; then
        echo -e "\n${GREEN}${BOLD}✅ VALIDATION SUCCESSFUL${NC}"
        if [ "$MODE" = "production" ]; then
            echo -e "${GREEN}🚀 Code approved for production deployment${NC}"
        fi
    else
        echo -e "\n${RED}${BOLD}❌ VALIDATION FAILED${NC}"
        echo -e "${RED}🔧 Fix all issues before proceeding${NC}"
        
        if [ "$MODE" = "production" ]; then
            echo -e "${RED}🚨 PRODUCTION DEPLOYMENT BLOCKED${NC}"
        fi
    fi
    
    exit $exit_code
}

# Execute main function
main "$@"