#!/usr/bin/env bash
# ops-007: Integration testbed runner (ADR-007-001, Pattern ID 17)
#
# Entry point for all integration testbeds. Dispatches to testbed-specific
# config while orchestrating shared prep -> inject -> validate flow.
#
# Usage:
#   ./tests/integration/run-testbed.sh smoke
#   ./tests/integration/run-testbed.sh regression --intelligence
#   ./tests/integration/run-testbed.sh stress --timeout 1800
#   ./tests/integration/run-testbed.sh feature --path product/features/fe-005/testbed
#
# Exit codes:
#   0 = all validations passed
#   1 = validation failures or runtime error

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Source shared libraries
source "$SCRIPT_DIR/lib/prep.sh"
source "$SCRIPT_DIR/lib/inject.sh"
source "$SCRIPT_DIR/lib/assert.sh"

# Defaults
TESTBED_TYPE=""
INTELLIGENCE=false
TIMEOUT=120
INJECT_COUNT=10
INJECT_RATE=1
FEATURE_PATH=""
SKIP_CLEAN=false

usage() {
    echo "Usage: $0 <type> [options]"
    echo ""
    echo "Testbed types:"
    echo "  smoke       Minimal validation: 1 stream, basic assertions"
    echo "  regression  Full layer coverage: all streams, Gold, domains, intelligence"
    echo "  stress      Sustained load: RSS monitoring, growth rate checks"
    echo "  feature     Feature-specific: --path required"
    echo ""
    echo "Options:"
    echo "  --intelligence    Enable intelligence service profile"
    echo "  --timeout N       Health check timeout in seconds (default: 120)"
    echo "  --count N         Number of MQTT messages to inject (default: 10)"
    echo "  --rate N          Messages per second (default: 1)"
    echo "  --path DIR        Feature testbed directory (required for 'feature' type)"
    echo "  --skip-clean      Skip clean slate (use existing environment)"
    echo "  --help            Show this help"
    exit 0
}

# Parse arguments
if [ $# -lt 1 ]; then
    usage
fi

TESTBED_TYPE="$1"
shift

while [[ $# -gt 0 ]]; do
    case "$1" in
        --intelligence) INTELLIGENCE=true; shift ;;
        --timeout) TIMEOUT="$2"; shift 2 ;;
        --count) INJECT_COUNT="$2"; shift 2 ;;
        --rate) INJECT_RATE="$2"; shift 2 ;;
        --path) FEATURE_PATH="$2"; shift 2 ;;
        --skip-clean) SKIP_CLEAN=true; shift ;;
        --help) usage ;;
        *) echo "ERROR: Unknown option: $1" >&2; exit 1 ;;
    esac
done

# Resolve testbed directory
case "$TESTBED_TYPE" in
    smoke|regression|stress)
        TESTBED_DIR="$SCRIPT_DIR/testbeds/$TESTBED_TYPE"
        ;;
    feature)
        if [ -z "$FEATURE_PATH" ]; then
            echo "ERROR: --path required for feature testbed" >&2
            exit 1
        fi
        TESTBED_DIR="$REPO_ROOT/$FEATURE_PATH"
        ;;
    *)
        echo "ERROR: Unknown testbed type: $TESTBED_TYPE" >&2
        echo "Valid types: smoke, regression, stress, feature" >&2
        exit 1
        ;;
esac

if [ ! -d "$TESTBED_DIR" ]; then
    echo "ERROR: Testbed directory not found: $TESTBED_DIR" >&2
    exit 1
fi

# Export for sub-scripts
export REPO_ROOT
export TESTBED_TYPE
export TESTBED_DIR
export PREP_HEALTH_TIMEOUT="$TIMEOUT"
export PREP_INTELLIGENCE="$INTELLIGENCE"

# Banner
echo "============================================"
echo "  NDP Integration Testbed: $TESTBED_TYPE"
echo "============================================"
echo "  Directory: $TESTBED_DIR"
echo "  Intelligence: $INTELLIGENCE"
echo "  Timeout: ${TIMEOUT}s"
echo "  Inject: ${INJECT_COUNT} msgs @ ${INJECT_RATE}/sec"
echo "============================================"
echo ""

START_TIME=$(date +%s)

# Phase 1: Environment preparation
echo "--- Phase 1: Environment Preparation ---"
COMPOSE_OVERRIDE="$TESTBED_DIR/compose-override.yml"

if [ "$SKIP_CLEAN" = "false" ]; then
    prep_clean_slate "$COMPOSE_OVERRIDE"
else
    echo "Skipping clean slate (--skip-clean)"
fi

prep_wait_healthy "$TIMEOUT"
echo ""

# Phase 2: Config sync and manifest apply
echo "--- Phase 2: Configuration ---"
MANIFEST="$TESTBED_DIR/manifest.json"
if [ -f "$MANIFEST" ]; then
    prep_apply_manifest "$MANIFEST"
else
    echo "No manifest found, skipping apply"
fi
echo ""

# Phase 3: Data injection
echo "--- Phase 3: Data Injection ---"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
TEMPLATE="$FIXTURES_DIR/mqtt/airgradient.jsonl"

if [ -f "$TEMPLATE" ]; then
    inject_messages \
        --topic "airgradient/readings/test-sensor-001" \
        --template "$TEMPLATE" \
        --count "$INJECT_COUNT" \
        --rate "$INJECT_RATE"

    # Brief pause to let data flow through the pipeline
    echo "Waiting for data propagation..."
    sleep 5
else
    echo "No MQTT template found, skipping injection"
fi
echo ""

# Phase 4: Validation
echo "--- Phase 4: Validation ---"
VALIDATE_SCRIPT="$TESTBED_DIR/validate.sh"
if [ -f "$VALIDATE_SCRIPT" ] && [ -x "$VALIDATE_SCRIPT" ]; then
    source "$VALIDATE_SCRIPT"
else
    echo "WARNING: No validate.sh found in $TESTBED_DIR" >&2
fi

# Summary
END_TIME=$(date +%s)
ELAPSED=$(( END_TIME - START_TIME ))

echo ""
echo "============================================"
echo "  Testbed $TESTBED_TYPE completed in ${ELAPSED}s"
echo "============================================"

# Return assertion exit code
assert_summary
