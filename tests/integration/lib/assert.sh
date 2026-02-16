#!/usr/bin/env bash
# ops-007: Integration test assertion library (ADR-007-007, Pattern ID 23)
#
# Uniform assert_* contract:
#   - Each function prints PASS/FAIL with description
#   - Tracks pass/fail counts internally
#   - assert_summary prints totals and returns exit code (0=all pass, 1=any fail)
#
# Usage:
#   source "$(dirname "$0")/../../lib/assert.sh"
#   assert_silver_rows "air_quality_observations" 5
#   assert_etcd_key "/domains/indoor-air-quality/config"
#   assert_summary

set -euo pipefail

# Internal counters
_ASSERT_PASS=0
_ASSERT_FAIL=0
_ASSERT_RESULTS=()

ASSERT_TIMESCALEDB_CONTAINER="${ASSERT_TIMESCALEDB_CONTAINER:-integration-timescaledb}"
ASSERT_ETCD_CONTAINER="${ASSERT_ETCD_CONTAINER:-integration-etcd}"
ASSERT_MOSQUITTO_CONTAINER="${ASSERT_MOSQUITTO_CONTAINER:-integration-mosquitto}"

# Record a pass
_assert_pass() {
    local desc="$1"
    _ASSERT_PASS=$((_ASSERT_PASS + 1))
    _ASSERT_RESULTS+=("PASS: $desc")
    echo "  PASS: $desc"
}

# Record a fail
_assert_fail() {
    local desc="$1"
    local detail="${2:-}"
    _ASSERT_FAIL=$((_ASSERT_FAIL + 1))
    if [ -n "$detail" ]; then
        _ASSERT_RESULTS+=("FAIL: $desc ($detail)")
        echo "  FAIL: $desc ($detail)"
    else
        _ASSERT_RESULTS+=("FAIL: $desc")
        echo "  FAIL: $desc"
    fi
}

# Assert that a Silver table has at least min_count rows.
#
# Args: $1 = table name (without schema prefix), $2 = minimum row count
# Example: assert_silver_rows "air_quality_observations" 5
assert_silver_rows() {
    local table="$1"
    local min_count="${2:-1}"
    local desc="Silver table silver.$table has >= $min_count rows"

    local actual
    actual=$(docker exec "$ASSERT_TIMESCALEDB_CONTAINER" \
        psql -U postgres -d ndp -t -A -c \
        "SELECT COUNT(*) FROM silver.$table;" 2>/dev/null) || {
        _assert_fail "$desc" "query failed"
        return 0
    }

    # Trim whitespace
    actual=$(echo "$actual" | tr -d '[:space:]')

    if [ "$actual" -ge "$min_count" ] 2>/dev/null; then
        _assert_pass "$desc (actual: $actual)"
    else
        _assert_fail "$desc" "actual: $actual"
    fi
}

# Assert that Bronze data exists for a stream (Parquet dir OR WAL entries).
#
# Checks two locations:
#   1. /data/raw/$stream/ — Parquet directory (exists after day rollover)
#   2. /data/bronze_wal.log — WAL file entries for this stream's source
#
# Args: $1 = stream name, $2 = container (default: integration-air-quality)
# Example: assert_bronze_wal_exists "air-quality"
assert_bronze_wal_exists() {
    local stream="$1"
    local container="${2:-integration-air-quality}"
    local desc="Bronze data exists for stream $stream"

    # Check Parquet directory first (post-rollover)
    if docker exec "$container" \
        ls "/data/raw/$stream/" >/dev/null 2>&1; then
        _assert_pass "$desc (parquet dir)"
        return 0
    fi

    # Check WAL for entries from this stream's source
    if docker exec "$container" \
        grep -q "\"source_id\":\"${stream}-Mqtt\"\|\"source_id\":\"${stream}-Http\"" \
        /data/bronze_wal.log 2>/dev/null; then
        _assert_pass "$desc (WAL entries)"
        return 0
    fi

    _assert_fail "$desc" "no parquet dir or WAL entries"
}

# Assert that a Docker service is healthy.
#
# Args: $1 = container name
# Example: assert_service_healthy "integration-timescaledb"
assert_service_healthy() {
    local container="$1"
    local desc="Container $container is healthy"

    local health
    health=$(docker inspect --format='{{.State.Health.Status}}' "$container" 2>/dev/null) || {
        _assert_fail "$desc" "container not found"
        return 0
    }

    if [ "$health" = "healthy" ]; then
        _assert_pass "$desc"
    else
        _assert_fail "$desc" "status: $health"
    fi
}

# Assert that an etcd key exists and has a non-empty value.
#
# Args: $1 = etcd key path
# Example: assert_etcd_key "/domains/indoor-air-quality/config"
assert_etcd_key() {
    local key="$1"
    local desc="etcd key exists: $key"

    local value
    value=$(docker exec "$ASSERT_ETCD_CONTAINER" \
        etcdctl get "$key" --print-value-only 2>/dev/null) || {
        _assert_fail "$desc" "etcdctl get failed"
        return 0
    }

    if [ -n "$value" ]; then
        _assert_pass "$desc"
    else
        _assert_fail "$desc" "key empty or missing"
    fi
}

# Assert that intelligence embeddings exist for a domain.
#
# Args: $1 = domain ID
# Example: assert_embedding_exists "indoor-air-quality"
assert_embedding_exists() {
    local domain="$1"
    # Intelligence tables use underscores: indoor_air_quality
    local table_domain
    table_domain=$(echo "$domain" | tr '-' '_')
    local desc="Embeddings exist for domain $domain"

    local count
    count=$(docker exec "$ASSERT_TIMESCALEDB_CONTAINER" \
        psql -U postgres -d ndp -t -A -c \
        "SELECT COUNT(*) FROM intelligence.${table_domain}_embeddings;" 2>/dev/null) || {
        _assert_fail "$desc" "query failed (table may not exist)"
        return 0
    }

    count=$(echo "$count" | tr -d '[:space:]')

    if [ "$count" -gt 0 ] 2>/dev/null; then
        _assert_pass "$desc (count: $count)"
    else
        _assert_fail "$desc" "count: $count"
    fi
}

# Assert that a container's RSS memory is below a threshold.
#
# Args: $1 = container name, $2 = max RSS in MiB
# Example: assert_container_rss_below "integration-air-quality" 256
assert_container_rss_below() {
    local container="$1"
    local max_mb="$2"
    local desc="Container $container RSS below ${max_mb} MiB"

    # docker stats returns memory usage in human-readable format
    local mem_usage
    mem_usage=$(docker stats --no-stream --format '{{.MemUsage}}' "$container" 2>/dev/null) || {
        _assert_fail "$desc" "docker stats failed"
        return 0
    }

    # Parse the current usage (before the / separator)
    # Format examples: "23.5MiB / 256MiB" or "1.2GiB / 4GiB"
    local current
    current=$(echo "$mem_usage" | awk -F'/' '{print $1}' | tr -d '[:space:]')

    local value unit
    value=$(echo "$current" | sed 's/[A-Za-z]*$//')
    unit=$(echo "$current" | sed 's/^[0-9.]*//')

    # Convert to MiB
    local mb
    case "$unit" in
        MiB) mb="$value" ;;
        GiB) mb=$(awk "BEGIN { printf \"%.1f\", $value * 1024 }") ;;
        KiB) mb=$(awk "BEGIN { printf \"%.1f\", $value / 1024 }") ;;
        *) _assert_fail "$desc" "unknown unit: $unit"; return 0 ;;
    esac

    if awk "BEGIN { exit !($mb < $max_mb) }"; then
        _assert_pass "$desc (actual: ${mb} MiB)"
    else
        _assert_fail "$desc" "actual: ${mb} MiB"
    fi
}

# Assert that a Gold continuous aggregate or materialized view exists.
#
# Args: $1 = view/table name (in gold schema)
# Example: assert_gold_object_exists "air_quality_observations_hourly"
assert_gold_object_exists() {
    local name="$1"
    local desc="Gold object exists: gold.$name"

    local exists
    exists=$(docker exec "$ASSERT_TIMESCALEDB_CONTAINER" \
        psql -U postgres -d ndp -t -A -c \
        "SELECT EXISTS(
            SELECT 1 FROM information_schema.tables
            WHERE table_schema = 'gold' AND table_name = '$name'
            UNION
            SELECT 1 FROM pg_matviews
            WHERE schemaname = 'gold' AND matviewname = '$name'
        );" 2>/dev/null) || {
        _assert_fail "$desc" "query failed"
        return 0
    }

    exists=$(echo "$exists" | tr -d '[:space:]')

    if [ "$exists" = "t" ]; then
        _assert_pass "$desc"
    else
        _assert_fail "$desc" "not found"
    fi
}

# Print assertion summary and return appropriate exit code.
#
# Returns: 0 if all assertions passed, 1 if any failed
assert_summary() {
    local total=$((_ASSERT_PASS + _ASSERT_FAIL))
    echo ""
    echo "=========================================="
    echo "  Assertion Summary"
    echo "=========================================="
    echo "  Total:  $total"
    echo "  Passed: $_ASSERT_PASS"
    echo "  Failed: $_ASSERT_FAIL"
    echo "=========================================="

    if [ $_ASSERT_FAIL -gt 0 ]; then
        echo ""
        echo "Failed assertions:"
        for result in "${_ASSERT_RESULTS[@]}"; do
            if [[ "$result" == FAIL:* ]]; then
                echo "  $result"
            fi
        done
        return 1
    fi

    return 0
}
