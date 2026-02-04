#!/bin/bash
# Local Validation Script for Neural Data Platform
# Part of AIR-015: Configuration Lifecycle Improvements
#
# Runs quick validation checks suitable for pre-commit or local development.
# Target: < 60s for standard mode
#
# Usage: ./scripts/validate.sh [options]
#
# Modes:
#   --quick       Cargo check only (~15s)
#   (default)     Cargo check + tests (~45s)
#   --full        Check + tests + clippy + config validation (~90s)
#   --config-only Validate config files only (~5s)
#
# Git Hook Installation:
#   cp scripts/validate.sh .git/hooks/pre-commit
#   chmod +x .git/hooks/pre-commit
#
# Exit codes:
#   0 = Validation passed
#   1 = Validation failed

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Mode
MODE="standard"
START_TIME=$(date +%s)

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --quick|-q)
            MODE="quick"
            shift
            ;;
        --full|-f)
            MODE="full"
            shift
            ;;
        --config-only|-c)
            MODE="config"
            shift
            ;;
        --help|-h)
            echo "Local Validation Script"
            echo ""
            echo "Usage: $(basename "$0") [options]"
            echo ""
            echo "Options:"
            echo "  --quick, -q       Cargo check only (~15s)"
            echo "  --full, -f        Full validation with clippy (~90s)"
            echo "  --config-only, -c Config validation only (~5s)"
            echo "  --help, -h        Show this help"
            echo ""
            echo "Default (no options): cargo check + tests (~45s)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

log() { echo -e "${GREEN}[VALIDATE]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
fail() { echo -e "${RED}[FAIL]${NC} $1"; }
info() { echo -e "${BLUE}[INFO]${NC} $1"; }

show_duration() {
    local end_time=$(date +%s)
    local duration=$((end_time - START_TIME))
    echo ""
    if [ $duration -lt 60 ]; then
        log "Completed in ${duration}s"
    else
        log "Completed in $((duration / 60))m $((duration % 60))s"
    fi
}

# Track failures
FAILURES=0

run_check() {
    local name="$1"
    shift
    log "Running: $name"
    if ! "$@"; then
        fail "$name failed"
        FAILURES=$((FAILURES + 1))
        return 1
    fi
    return 0
}

# Validate config files
validate_configs() {
    log "Validating configuration files..."

    local config_errors=0

    # Check YAML syntax for stream configs
    if command -v python3 &> /dev/null; then
        for config in "$REPO_ROOT"/config/base/streams/*/config.yaml; do
            if [ -f "$config" ]; then
                if ! python3 -c "import yaml; yaml.safe_load(open('$config'))" 2>/dev/null; then
                    fail "Invalid YAML: $config"
                    config_errors=$((config_errors + 1))
                fi
            fi
        done

        # Also check integration configs if they exist
        if [ -d "$REPO_ROOT/config/integration/base/streams" ]; then
            for config in "$REPO_ROOT"/config/integration/base/streams/*/config.yaml; do
                if [ -f "$config" ]; then
                    if ! python3 -c "import yaml; yaml.safe_load(open('$config'))" 2>/dev/null; then
                        fail "Invalid YAML: $config"
                        config_errors=$((config_errors + 1))
                    fi
                fi
            done
        fi
    else
        warn "Python3 not available, skipping YAML validation"
    fi

    # Check JSON syntax for stream configs
    if command -v jq &> /dev/null; then
        for config in "$REPO_ROOT"/config/base/streams/*/config.json; do
            if [ -f "$config" ]; then
                if ! jq empty "$config" 2>/dev/null; then
                    fail "Invalid JSON: $config"
                    config_errors=$((config_errors + 1))
                fi
            fi
        done
    fi

    if [ $config_errors -gt 0 ]; then
        FAILURES=$((FAILURES + config_errors))
        return 1
    fi

    log "Config validation passed"
    return 0
}

# Main validation based on mode
cd "$REPO_ROOT"

log "Mode: $MODE"
echo ""

case "$MODE" in
    quick)
        run_check "cargo check" cargo check --all-targets
        ;;

    standard)
        run_check "cargo check" cargo check --all-targets
        run_check "cargo test" cargo test --lib --bins
        ;;

    full)
        validate_configs
        run_check "cargo check" cargo check --all-targets
        run_check "cargo test" cargo test --lib --bins
        run_check "cargo clippy" cargo clippy --all-targets -- -D warnings
        ;;

    config)
        validate_configs
        ;;
esac

show_duration

if [ $FAILURES -gt 0 ]; then
    echo ""
    fail "Validation failed with $FAILURES error(s)"
    exit 1
fi

echo ""
log "All validations passed"
exit 0
