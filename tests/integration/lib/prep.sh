#!/usr/bin/env bash
# ops-007: Environment preparation library (ADR-007-005, Pattern ID 21)
#
# Provides clean-slate reset, health polling, config sync, and manifest apply
# for integration testbeds. All test-only logic lives here, not in deploy.sh.
#
# Usage:
#   source "$(dirname "$0")/../lib/prep.sh"
#   prep_clean_slate
#   prep_wait_healthy
#   prep_sync_configs
#   prep_apply_manifest "$TESTBED_DIR/manifest.json"

set -euo pipefail

_PREP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${REPO_ROOT:-$(cd "$_PREP_DIR/../../.." && pwd)}"

COMPOSE_FILE="${COMPOSE_FILE:-$REPO_ROOT/docker-compose.integration.yml}"
DEPLOY_SCRIPT="${DEPLOY_SCRIPT:-$REPO_ROOT/deploy/pi/deploy.sh}"
PREP_HEALTH_TIMEOUT="${PREP_HEALTH_TIMEOUT:-120}"

# Build the base docker compose command.
# Args: compose override file paths (optional)
_compose_cmd() {
    local cmd="docker compose -f $COMPOSE_FILE"
    for override in "$@"; do
        if [ -f "$override" ]; then
            cmd="$cmd -f $override"
        fi
    done
    echo "$cmd"
}

# Clean slate: destroy all volumes and recreate containers (ADR-007-005).
# This guarantees a pristine database with no leftover state.
#
# Args: $@ = additional compose override files (optional)
prep_clean_slate() {
    local compose
    compose=$(_compose_cmd "$@")

    echo "Clean slate: tearing down integration environment..."
    $compose down -v --remove-orphans 2>/dev/null || true

    echo "Clean slate: starting integration environment..."
    if [ "${PREP_INTELLIGENCE:-false}" = "true" ]; then
        $compose --profile intelligence up -d
    else
        $compose up -d
    fi

    echo "Clean slate: environment restarted"
}

# Wait for all core services to be healthy.
#
# Args: $1 = timeout in seconds (default: PREP_HEALTH_TIMEOUT)
prep_wait_healthy() {
    local timeout="${1:-$PREP_HEALTH_TIMEOUT}"
    local services=(
        "integration-mosquitto"
        "integration-etcd"
        "integration-timescaledb"
    )

    echo "Waiting for services to be healthy (timeout: ${timeout}s)..."

    local start_time
    start_time=$(date +%s)

    for svc in "${services[@]}"; do
        echo -n "  Waiting for $svc..."
        while true; do
            local elapsed=$(( $(date +%s) - start_time ))
            if [ "$elapsed" -ge "$timeout" ]; then
                echo " TIMEOUT"
                echo "ERROR: Service $svc did not become healthy within ${timeout}s" >&2
                return 1
            fi

            local health
            health=$(docker inspect --format='{{.State.Health.Status}}' "$svc" 2>/dev/null) || {
                sleep 2
                continue
            }

            if [ "$health" = "healthy" ]; then
                echo " OK (${elapsed}s)"
                break
            fi

            sleep 2
        done
    done

    # Wait for air-quality-app separately (depends on other services, takes longer)
    echo -n "  Waiting for integration-air-quality..."
    while true; do
        local elapsed=$(( $(date +%s) - start_time ))
        if [ "$elapsed" -ge "$timeout" ]; then
            echo " TIMEOUT"
            echo "ERROR: integration-air-quality did not become healthy within ${timeout}s" >&2
            return 1
        fi

        local health
        health=$(docker inspect --format='{{.State.Health.Status}}' "integration-air-quality" 2>/dev/null) || {
            sleep 3
            continue
        }

        if [ "$health" = "healthy" ]; then
            echo " OK (${elapsed}s)"
            break
        fi

        sleep 3
    done

    # Optionally wait for intelligence
    if [ "${PREP_INTELLIGENCE:-false}" = "true" ]; then
        echo -n "  Waiting for integration-intelligence..."
        while true; do
            local elapsed=$(( $(date +%s) - start_time ))
            if [ "$elapsed" -ge "$timeout" ]; then
                echo " TIMEOUT (non-fatal for intelligence)"
                break
            fi

            local running
            running=$(docker inspect --format='{{.State.Status}}' "integration-intelligence" 2>/dev/null) || {
                sleep 3
                continue
            }

            if [ "$running" = "running" ]; then
                echo " RUNNING (${elapsed}s)"
                break
            fi

            sleep 3
        done
    fi

    echo "All services healthy."
    return 0
}

# Sync configs using deploy.sh sync-domains command.
# This pushes stream configs, domain configs, and data dictionary entries.
prep_sync_configs() {
    echo "Syncing configs via deploy.sh..."
    DEPLOY_ENV=integration POSTGRES_PASSWORD=postgres bash "$DEPLOY_SCRIPT" sync-domains || {
        echo "WARNING: sync-domains returned non-zero (may be expected if ndp CLI not built)" >&2
    }
    echo "Config sync complete."
}

# Apply a deployment manifest using deploy.sh.
#
# Args: $1 = path to manifest.json
prep_apply_manifest() {
    local manifest="$1"

    if [ ! -f "$manifest" ]; then
        echo "ERROR: Manifest not found: $manifest" >&2
        return 1
    fi

    echo "Applying manifest: $manifest"
    DEPLOY_ENV=integration POSTGRES_PASSWORD=postgres bash "$DEPLOY_SCRIPT" apply "$manifest" || {
        echo "ERROR: Manifest apply failed" >&2
        return 1
    }
    echo "Manifest applied successfully."
}
