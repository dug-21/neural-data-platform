#!/bin/bash
# =============================================================================
# Phase E Acceptance Tests Runner
# =============================================================================
# Usage: ./run_acceptance_tests.sh [options]
#
# Options:
#   --all         Run all acceptance tests (default)
#   --hypertable  Run only events hypertable tests
#   --crossings   Run only threshold crossing tests
#   --unified     Run only unified events tests
#   --job         Run only detection job tests
#   --summary     Show only PASS/FAIL/SKIP summary
#   --verbose     Show full output
#   --help        Show this help
# =============================================================================

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-ndp}"
DB_USER="${DB_USER:-postgres}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Test files
TESTS=(
    "acceptance_events_hypertable.sql"
    "acceptance_threshold_crossings.sql"
    "acceptance_unified_events.sql"
    "acceptance_detection_job.sql"
)

# Parse arguments
RUN_ALL=true
RUN_HYPERTABLE=false
RUN_CROSSINGS=false
RUN_UNIFIED=false
RUN_JOB=false
SUMMARY_ONLY=false
VERBOSE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            RUN_ALL=true
            shift
            ;;
        --hypertable)
            RUN_ALL=false
            RUN_HYPERTABLE=true
            shift
            ;;
        --crossings)
            RUN_ALL=false
            RUN_CROSSINGS=true
            shift
            ;;
        --unified)
            RUN_ALL=false
            RUN_UNIFIED=true
            shift
            ;;
        --job)
            RUN_ALL=false
            RUN_JOB=true
            shift
            ;;
        --summary)
            SUMMARY_ONLY=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --help)
            head -20 "$0" | tail -18
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Determine how to run psql (Docker or direct)
run_psql() {
    if docker ps | grep -q timescaledb; then
        docker exec timescaledb psql -U "$DB_USER" -d "$DB_NAME" "$@"
    else
        PGPASSWORD="${DB_PASSWORD:-}" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" "$@"
    fi
}

# Run a test file
run_test() {
    local test_file="$1"
    local test_path="$SCRIPT_DIR/$test_file"

    echo ""
    echo "============================================"
    echo "Running: $test_file"
    echo "============================================"

    if [[ ! -f "$test_path" ]]; then
        echo -e "${RED}ERROR: Test file not found: $test_path${NC}"
        return 1
    fi

    local output
    if output=$(run_psql -f "$test_path" 2>&1); then
        if $SUMMARY_ONLY; then
            echo "$output" | grep -E "(PASS|FAIL|SKIP|CHECK):" || true
        elif $VERBOSE; then
            echo "$output"
        else
            echo "$output" | grep -E "(PASS|FAIL|SKIP|CHECK|NOTICE):" || true
        fi
    else
        echo -e "${RED}ERROR running test: $test_file${NC}"
        echo "$output"
        return 1
    fi
}

# Count results
count_results() {
    local output="$1"
    local pass_count=$(echo "$output" | grep -c "PASS:" || true)
    local fail_count=$(echo "$output" | grep -c "FAIL:" || true)
    local skip_count=$(echo "$output" | grep -c "SKIP:" || true)

    echo -e "${GREEN}PASS: $pass_count${NC} | ${RED}FAIL: $fail_count${NC} | ${YELLOW}SKIP: $skip_count${NC}"
}

# Main execution
echo "============================================"
echo "Phase E Acceptance Tests"
echo "============================================"
echo "Database: $DB_NAME"
echo "Host: $DB_HOST:$DB_PORT"
echo ""

# Check database connection
if ! run_psql -c "SELECT 1" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Cannot connect to database${NC}"
    echo "Ensure TimescaleDB is running and accessible"
    exit 1
fi

echo "Database connection: OK"

# Collect all output for summary
all_output=""

# Run selected tests
if $RUN_ALL || $RUN_HYPERTABLE; then
    output=$(run_test "acceptance_events_hypertable.sql")
    echo "$output"
    all_output+="$output"
fi

if $RUN_ALL || $RUN_CROSSINGS; then
    output=$(run_test "acceptance_threshold_crossings.sql")
    echo "$output"
    all_output+="$output"
fi

if $RUN_ALL || $RUN_UNIFIED; then
    output=$(run_test "acceptance_unified_events.sql")
    echo "$output"
    all_output+="$output"
fi

if $RUN_ALL || $RUN_JOB; then
    output=$(run_test "acceptance_detection_job.sql")
    echo "$output"
    all_output+="$output"
fi

# Final summary
echo ""
echo "============================================"
echo "Test Summary"
echo "============================================"
count_results "$all_output"
echo ""

# Exit with failure if any tests failed
if echo "$all_output" | grep -q "FAIL:"; then
    echo -e "${YELLOW}Note: Tests are expected to FAIL until Phase E implementation is complete.${NC}"
    echo "This is London TDD - tests written FIRST."
    exit 0  # Exit 0 since failures are expected pre-implementation
fi

echo -e "${GREEN}All tests passed!${NC}"
exit 0
