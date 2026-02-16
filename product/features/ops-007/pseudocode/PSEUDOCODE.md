# ops-007: Pseudocode

All pseudocode is bash/shell. No Rust code in this feature.

## 1. run-testbed.sh (Entry Point)

```bash
#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

source "$SCRIPT_DIR/lib/prep.sh"
source "$SCRIPT_DIR/lib/inject.sh"
source "$SCRIPT_DIR/lib/assert.sh"

# --- Argument Parsing ---
TESTBED_TYPE="${1:?Usage: run-testbed.sh <smoke|regression|stress|feature> [options]}"
shift

NO_CLEAN=false
VERBOSE=false
TIMEOUT=120
FEATURE_PATH=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-clean)   NO_CLEAN=true; shift ;;
        --verbose)    VERBOSE=true; shift ;;
        --timeout)    TIMEOUT="$2"; shift 2 ;;
        --feature)    FEATURE_PATH="$2"; shift 2 ;;
        *)            echo "Unknown option: $1"; exit 1 ;;
    esac
done

# --- Resolve Testbed Path ---
case "$TESTBED_TYPE" in
    smoke|regression|stress)
        TESTBED_DIR="$SCRIPT_DIR/testbeds/$TESTBED_TYPE"
        ;;
    feature)
        TESTBED_DIR="${FEATURE_PATH:?--feature <path> required for feature testbed}"
        ;;
    *)
        echo "Unknown testbed type: $TESTBED_TYPE"
        exit 1
        ;;
esac

# --- Resolve Compose Files ---
COMPOSE_BASE="$REPO_ROOT/docker-compose.integration.yml"
COMPOSE_CMD="-f $COMPOSE_BASE"
if [ -f "$TESTBED_DIR/compose-override.yml" ]; then
    COMPOSE_CMD="$COMPOSE_CMD -f $TESTBED_DIR/compose-override.yml"
fi

export COMPOSE_CMD TESTBED_DIR REPO_ROOT VERBOSE

# --- Execute Pipeline ---
START_TIME=$(date +%s)

log_info "=== Testbed: $TESTBED_TYPE ==="

# Phase 1: Prep
if [ "$NO_CLEAN" = false ]; then
    prep_clean_slate "$COMPOSE_CMD"
fi
prep_wait_healthy
prep_sync_configs
prep_apply_manifest "$TESTBED_DIR/manifest.json"

# Phase 2: Inject (if testbed has injection config)
if [ -f "$TESTBED_DIR/inject.conf" ]; then
    source "$TESTBED_DIR/inject.conf"  # sets INJECT_TOPIC, INJECT_TEMPLATE, etc.
    inject_messages "$INJECT_TOPIC" "$INJECT_TEMPLATE" "$INJECT_COUNT" "$INJECT_RATE"
    log_info "Waiting for pipeline processing..."
    sleep "${INJECT_SETTLE_TIME:-10}"
elif [ "$TESTBED_TYPE" = "smoke" ] || [ "$TESTBED_TYPE" = "stress" ]; then
    # Default injection for smoke/stress
    inject_messages \
        "airgradient/readings/test-sensor-001" \
        "$SCRIPT_DIR/fixtures/mqtt/airgradient.jsonl" \
        "${INJECT_COUNT:-10}" \
        "${INJECT_RATE:-1}"
    sleep 10
fi

# Phase 3: Validate
log_info "Running validation..."
source "$TESTBED_DIR/validate.sh"

# Phase 4: Report
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
log_info "=== Duration: ${DURATION}s ==="
assert_summary

exit $?
```

## 2. lib/prep.sh (Environment Preparation)

```bash
#!/bin/bash
# Sourced by run-testbed.sh -- not executed directly

DEPLOY_SCRIPT="$REPO_ROOT/deploy/pi/deploy.sh"
TIMESCALE_CONTAINER="ndp-timescaledb-integration"
ETCD_CONTAINER="ndp-etcd-integration"
MOSQUITTO_CONTAINER="ndp-mosquitto-integration"
PG_USER="ndp"
PG_DB="ndp"

log_info() { echo "[INFO] $(date +%H:%M:%S) $*"; }
log_error() { echo "[ERROR] $(date +%H:%M:%S) $*" >&2; }

prep_clean_slate() {
    local compose_cmd="$1"
    log_info "Tearing down integration stack (volumes included)..."
    docker compose $compose_cmd down -v --remove-orphans 2>/dev/null || true

    log_info "Starting fresh integration stack..."
    docker compose $compose_cmd up -d
}

prep_wait_healthy() {
    local timeout="${1:-60}"
    local elapsed=0

    log_info "Waiting for services to be healthy (timeout: ${timeout}s)..."

    while [ $elapsed -lt $timeout ]; do
        local all_healthy=true

        # Check TimescaleDB
        if ! docker exec "$TIMESCALE_CONTAINER" pg_isready -U "$PG_USER" -d "$PG_DB" &>/dev/null; then
            all_healthy=false
        fi

        # Check etcd
        if ! docker exec "$ETCD_CONTAINER" etcdctl endpoint health &>/dev/null; then
            all_healthy=false
        fi

        # Check mosquitto
        if ! docker inspect --format='{{.State.Health.Status}}' "$MOSQUITTO_CONTAINER" 2>/dev/null | grep -q "healthy"; then
            # Mosquitto may not have healthcheck -- check running state
            if ! docker inspect --format='{{.State.Running}}' "$MOSQUITTO_CONTAINER" 2>/dev/null | grep -q "true"; then
                all_healthy=false
            fi
        fi

        if [ "$all_healthy" = true ]; then
            log_info "All services healthy after ${elapsed}s"
            return 0
        fi

        sleep 2
        elapsed=$((elapsed + 2))
    done

    log_error "Services not healthy after ${timeout}s"
    return 1
}

prep_sync_configs() {
    log_info "Syncing integration configs..."
    DEPLOY_ENV=integration "$DEPLOY_SCRIPT" sync
}

prep_apply_manifest() {
    local manifest="$1"
    if [ ! -f "$manifest" ]; then
        log_error "Manifest not found: $manifest"
        return 1
    fi
    log_info "Applying manifest: $manifest"
    DEPLOY_ENV=integration "$DEPLOY_SCRIPT" apply "$manifest"
}
```

## 3. lib/inject.sh (MQTT Data Injection)

```bash
#!/bin/bash
# Sourced by run-testbed.sh -- not executed directly

inject_messages() {
    local topic="$1"
    local template="$2"
    local count="${3:-10}"
    local rate="${4:-1}"

    if [ ! -f "$template" ]; then
        log_error "Template not found: $template"
        return 1
    fi

    local delay=$(echo "scale=3; 1/$rate" | bc)
    local line_count=$(wc -l < "$template")

    log_info "Injecting $count messages to $topic at ${rate} msg/sec..."

    for i in $(seq 1 "$count"); do
        local line_idx=$(( (i - 1) % line_count + 1 ))
        local msg=$(sed -n "${line_idx}p" "$template")

        # Randomize numeric values within template ranges
        msg=$(randomize_message "$msg")

        docker exec "$MOSQUITTO_CONTAINER" mosquitto_pub \
            -t "$topic" \
            -m "$msg" \
            -q 1

        if [ "$i" -lt "$count" ]; then
            sleep "$delay"
        fi
    done

    log_info "Injection complete: $count messages sent"
}

randomize_message() {
    local msg="$1"
    # Replace placeholder tokens with random values
    # {{RAND_INT:min:max}} -> random integer in [min, max]
    # {{RAND_FLOAT:min:max:decimals}} -> random float
    # {{TIMESTAMP}} -> current ISO timestamp
    # {{SERIAL:prefix}} -> prefix + random hex

    msg=$(echo "$msg" | sed "s/{{TIMESTAMP}}/$(date -u +%Y-%m-%dT%H:%M:%SZ)/g")

    # Process RAND_INT tokens
    while echo "$msg" | grep -q '{{RAND_INT:[0-9]*:[0-9]*}}'; do
        local token=$(echo "$msg" | grep -o '{{RAND_INT:[0-9]*:[0-9]*}}' | head -1)
        local min=$(echo "$token" | cut -d: -f2)
        local max=$(echo "$token" | cut -d: -f3 | tr -d '}}')
        local val=$(( RANDOM % (max - min + 1) + min ))
        msg=$(echo "$msg" | sed "s|$token|$val|")
    done

    # Process SERIAL tokens
    while echo "$msg" | grep -q '{{SERIAL:[^}]*}}'; do
        local token=$(echo "$msg" | grep -o '{{SERIAL:[^}]*}}' | head -1)
        local prefix=$(echo "$token" | cut -d: -f2 | tr -d '}}')
        local hex=$(openssl rand -hex 4)
        msg=$(echo "$msg" | sed "s|$token|${prefix}${hex}|")
    done

    echo "$msg"
}
```

## 4. lib/assert.sh (Validation Helpers)

```bash
#!/bin/bash
# Sourced by run-testbed.sh -- not executed directly

ASSERT_PASS=0
ASSERT_FAIL=0

assert_silver_rows() {
    local table="$1" min_count="$2"
    local count
    count=$(docker exec "$TIMESCALE_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
        "SELECT COUNT(*) FROM $table" 2>/dev/null) || count=0
    count=$(echo "$count" | tr -d '[:space:]')

    if [ "$count" -ge "$min_count" ]; then
        echo "PASS: $table has $count rows (>= $min_count)"
        ((ASSERT_PASS++))
        return 0
    else
        echo "FAIL: $table has $count rows (expected >= $min_count)"
        ((ASSERT_FAIL++))
        return 1
    fi
}

assert_bronze_wal_exists() {
    local stream="$1"
    local app_container="${2:-ndp-air-quality-app-integration}"

    # WAL files are inside the app container's data volume
    if docker exec "$app_container" test -d "/data/bronze/$stream/wal" 2>/dev/null; then
        echo "PASS: Bronze WAL directory exists for $stream"
        ((ASSERT_PASS++))
        return 0
    else
        echo "FAIL: Bronze WAL directory missing for $stream"
        ((ASSERT_FAIL++))
        return 1
    fi
}

assert_service_healthy() {
    local service="$1"
    local status
    status=$(docker inspect --format='{{.State.Health.Status}}' "$service" 2>/dev/null) || status="not-found"

    if [ "$status" = "healthy" ]; then
        echo "PASS: $service is healthy"
        ((ASSERT_PASS++))
        return 0
    else
        # Fallback: check running state for services without healthcheck
        local running
        running=$(docker inspect --format='{{.State.Running}}' "$service" 2>/dev/null) || running="false"
        if [ "$running" = "true" ]; then
            echo "PASS: $service is running (no healthcheck configured)"
            ((ASSERT_PASS++))
            return 0
        fi
        echo "FAIL: $service status: $status"
        ((ASSERT_FAIL++))
        return 1
    fi
}

assert_etcd_key() {
    local key="$1"
    local value
    value=$(docker exec "$ETCD_CONTAINER" etcdctl get "$key" --print-value-only 2>/dev/null)

    if [ -n "$value" ]; then
        echo "PASS: etcd key exists: $key"
        ((ASSERT_PASS++))
        return 0
    else
        echo "FAIL: etcd key missing: $key"
        ((ASSERT_FAIL++))
        return 1
    fi
}

assert_embedding_exists() {
    local domain="$1"
    local count
    count=$(docker exec "$TIMESCALE_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -tAc \
        "SELECT COUNT(*) FROM intelligence.${domain}_embeddings" 2>/dev/null) || count=0
    count=$(echo "$count" | tr -d '[:space:]')

    if [ "$count" -gt 0 ]; then
        echo "PASS: Embeddings exist for $domain ($count rows)"
        ((ASSERT_PASS++))
        return 0
    else
        echo "FAIL: No embeddings for $domain"
        ((ASSERT_FAIL++))
        return 1
    fi
}

assert_container_rss_below() {
    local container="$1" max_mb="$2"
    local rss_bytes
    rss_bytes=$(docker stats --no-stream --format '{{.MemUsage}}' "$container" 2>/dev/null | awk '{print $1}')

    # Parse memory value (handles MiB, GiB suffixes)
    local rss_mb
    if echo "$rss_bytes" | grep -qi "gib"; then
        rss_mb=$(echo "$rss_bytes" | sed 's/[^0-9.]//g' | awk '{printf "%.0f", $1 * 1024}')
    else
        rss_mb=$(echo "$rss_bytes" | sed 's/[^0-9.]//g' | awk '{printf "%.0f", $1}')
    fi

    if [ "$rss_mb" -le "$max_mb" ]; then
        echo "PASS: $container RSS ${rss_mb}MB <= ${max_mb}MB"
        ((ASSERT_PASS++))
        return 0
    else
        echo "FAIL: $container RSS ${rss_mb}MB > ${max_mb}MB"
        ((ASSERT_FAIL++))
        return 1
    fi
}

assert_summary() {
    echo ""
    echo "=============================="
    echo "Results: $ASSERT_PASS passed, $ASSERT_FAIL failed"
    echo "=============================="
    [ "$ASSERT_FAIL" -eq 0 ] && return 0 || return 1
}
```

## 5. deploy.sh etcd Sync Fix (WS1-02)

```bash
# Location: deploy/pi/deploy.sh, inside sync_domains_to_data_dictionary()
# ADD after existing TimescaleDB sync logic:

sync_domains_to_data_dictionary() {
    # ... existing TimescaleDB sync code ...

    # NEW: Also push domain config to etcd (intelligence daemon reads from etcd)
    log_info "Syncing domain configs to etcd..."
    for domain_dir in "$CONFIG_DOMAINS_DIR"/*/; do
        [ -d "$domain_dir" ] || continue
        local domain_id=$(basename "$domain_dir")
        local domain_config="$domain_dir/domain.json"

        if [ -f "$domain_config" ]; then
            log_info "  etcd put: /domains/${domain_id}/config"
            docker exec -i "$ETCD_CONTAINER" etcdctl put \
                "/domains/${domain_id}/config" \
                "$(cat "$domain_config")" || {
                log_warn "Failed to sync $domain_id to etcd (non-fatal)"
            }
        fi
    done
}
```

## 6. deploy.sh Gold DDL Config Path Fix (WS1-03)

Four hardcoded `--config-dir "$REPO_ROOT/config/base"` references need the same fix.
deploy.sh uses `ndp gold` (the ndp CLI), not the legacy `ndp-gold-ddl` binary.

```bash
# All 4 locations use the same substitution:
# BEFORE (hardcoded):
--config-dir "$REPO_ROOT/config/base"
# AFTER (environment-aware):
--config-dir "$(dirname "$CONFIG_STREAMS_DIR")"

# Location 1: handle_gold_table(), line ~1976 (stream DDL)
"$ndp_tool" gold "$action" --stream "$stream_id" \
    --config-dir "$(dirname "$CONFIG_STREAMS_DIR")" \
    --db-url "$db_url" --db-timeout 10

# Location 2: handle_domain(), line ~2094 (aligned views)
"$ndp_tool" gold "$gold_verb" --domain "$domain_id" \
    --config-dir "$(dirname "$CONFIG_STREAMS_DIR")"

# Location 3: handle_domain(), line ~2120 (events DDL)
"$ndp_tool" gold generate --domain "$domain_id" --events \
    --config-dir "$(dirname "$CONFIG_STREAMS_DIR")"

# Location 4: handle_domain(), line ~2139 (intelligence DDL)
"$ndp_tool" gold intelligence schema --domain "$domain_id" \
    --config-dir "$(dirname "$CONFIG_STREAMS_DIR")"
```

## 7. Smoke Testbed validate.sh

```bash
#!/bin/bash
# tests/integration/testbeds/smoke/validate.sh
# Sourced by run-testbed.sh -- lib/assert.sh already loaded

log_info "Smoke validation: checking data pipeline..."

# Service health
assert_service_healthy "$TIMESCALE_CONTAINER"
assert_service_healthy "$ETCD_CONTAINER"
assert_service_healthy "$MOSQUITTO_CONTAINER"

# Config propagation
assert_etcd_key "/domains/indoor-air-quality/config"

# Data flow: MQTT -> Bronze -> Silver
assert_silver_rows "air_quality_readings" 1

# Report
assert_summary
```

## 8. MQTT Message Template (airgradient.jsonl)

```jsonl
{"wifi":-{{RAND_INT:30:80}},"serialno":"{{SERIAL:test-}}","rco2":{{RAND_INT:400:2000}},"pm02":{{RAND_INT:1:50}},"atmp":{{RAND_INT:18:30}},"rhum":{{RAND_INT:30:70}}}
{"wifi":-{{RAND_INT:30:80}},"serialno":"{{SERIAL:test-}}","rco2":{{RAND_INT:400:2000}},"pm02":{{RAND_INT:1:50}},"atmp":{{RAND_INT:18:30}},"rhum":{{RAND_INT:30:70}}}
{"wifi":-{{RAND_INT:30:80}},"serialno":"{{SERIAL:test-}}","rco2":{{RAND_INT:400:2000}},"pm02":{{RAND_INT:1:50}},"atmp":{{RAND_INT:18:30}},"rhum":{{RAND_INT:30:70}}}
{"wifi":-{{RAND_INT:30:80}},"serialno":"{{SERIAL:test-}}","rco2":null,"pm02":{{RAND_INT:1:50}},"atmp":null,"rhum":{{RAND_INT:30:70}}}
{"wifi":-{{RAND_INT:30:80}},"serialno":"{{SERIAL:test-}}","rco2":{{RAND_INT:400:2000}},"pm02":{{RAND_INT:1:50}},"atmp":{{RAND_INT:18:30}},"rhum":null}
```
