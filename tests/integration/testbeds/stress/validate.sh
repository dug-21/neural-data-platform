#!/usr/bin/env bash
# ops-007: Stress testbed validation (Pattern ID 18: memory-management-dual-strategy)
# Sourced by run-testbed.sh (assert.sh already loaded)
#
# Validates RSS stays within bounds during sustained load.
# BUG-004/BUG-005 regression: RSS must not grow unbounded.
#
# Default: 10 msg/sec for 30 minutes (configurable via runner --count and --rate)
# RSS threshold: 256 MiB for air-quality-app (matches BUG-004 fix target)

STRESS_RSS_MAX="${STRESS_RSS_MAX:-256}"
STRESS_SAMPLE_INTERVAL="${STRESS_SAMPLE_INTERVAL:-30}"
STRESS_DURATION="${STRESS_DURATION:-1800}"

echo "Running stress validations..."
echo "  RSS max: ${STRESS_RSS_MAX} MiB"
echo "  Sample interval: ${STRESS_SAMPLE_INTERVAL}s"
echo "  Duration: ${STRESS_DURATION}s"

# Service health (baseline)
assert_service_healthy "integration-mosquitto"
assert_service_healthy "integration-etcd"
assert_service_healthy "integration-timescaledb"
assert_service_healthy "integration-air-quality"

# Silver data exists
assert_silver_rows "air_quality_observations" 1

# RSS monitoring over time
# The runner already injected data; now we sample RSS periodically
# to verify no memory growth
echo ""
echo "RSS monitoring (sampling every ${STRESS_SAMPLE_INTERVAL}s)..."

STRESS_START=$(date +%s)
RSS_SAMPLES=()
MAX_RSS_SEEN=0

while true; do
    local_elapsed=$(( $(date +%s) - STRESS_START ))
    if [ "$local_elapsed" -ge "$STRESS_DURATION" ]; then
        break
    fi

    # Sample RSS
    mem_usage=$(docker stats --no-stream --format '{{.MemUsage}}' "integration-air-quality" 2>/dev/null) || {
        echo "  WARNING: docker stats failed at ${local_elapsed}s"
        sleep "$STRESS_SAMPLE_INTERVAL"
        continue
    }

    current=$(echo "$mem_usage" | awk -F'/' '{print $1}' | tr -d '[:space:]')
    value=$(echo "$current" | sed 's/[A-Za-z]*$//')
    unit=$(echo "$current" | sed 's/^[0-9.]*//')

    case "$unit" in
        MiB) mb="$value" ;;
        GiB) mb=$(awk "BEGIN { printf \"%.1f\", $value * 1024 }") ;;
        KiB) mb=$(awk "BEGIN { printf \"%.1f\", $value / 1024 }") ;;
        *) mb="0" ;;
    esac

    RSS_SAMPLES+=("$mb")
    echo "  [${local_elapsed}s] RSS: ${mb} MiB"

    # Track max
    if awk "BEGIN { exit !($mb > $MAX_RSS_SEEN) }"; then
        MAX_RSS_SEEN="$mb"
    fi

    # Early exit if RSS exceeds limit
    if awk "BEGIN { exit !($mb > $STRESS_RSS_MAX) }"; then
        echo "  RSS EXCEEDED LIMIT at ${local_elapsed}s: ${mb} MiB > ${STRESS_RSS_MAX} MiB"
        break
    fi

    sleep "$STRESS_SAMPLE_INTERVAL"
done

echo ""
echo "Max RSS observed: ${MAX_RSS_SEEN} MiB (limit: ${STRESS_RSS_MAX} MiB)"

# Final RSS assertion
assert_container_rss_below "integration-air-quality" "$STRESS_RSS_MAX"

# Growth rate check: compare first and last samples
if [ ${#RSS_SAMPLES[@]} -ge 2 ]; then
    first_rss="${RSS_SAMPLES[0]}"
    last_rss="${RSS_SAMPLES[${#RSS_SAMPLES[@]}-1]}"
    growth=$(awk "BEGIN { printf \"%.1f\", $last_rss - $first_rss }")
    echo "  RSS growth: ${growth} MiB (first: ${first_rss}, last: ${last_rss})"

    # Growth should be minimal (< 50 MiB over the test duration)
    if awk "BEGIN { exit !($growth < 50) }"; then
        echo "  PASS: RSS growth within acceptable bounds"
    else
        echo "  WARNING: RSS growth may indicate a leak: ${growth} MiB"
    fi
fi

echo "Stress validations complete."
