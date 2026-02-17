#!/usr/bin/env bash
# BUG-007: Validate MQTT pipeline still processes after cached_points drain fix
# Sourced by run-testbed.sh (assert.sh already loaded)
#
# Validates:
#   - Core services healthy after rebuild
#   - MQTT messages reach Silver layer (no regression from fix)
#   - Bronze WAL populated (EventBus data flow intact)
#   - Container did not crash (restart count = 0)

echo "Running BUG-007 regression validations..."

# Service health
assert_service_healthy "integration-mosquitto"
assert_service_healthy "integration-etcd"
assert_service_healthy "integration-timescaledb"
assert_service_healthy "integration-air-quality"

# etcd domain config loaded
assert_etcd_key "/domains/indoor-air-quality/config"

# Silver layer data — proves full pipeline: MQTT → EventBus → Bronze → Silver
assert_silver_rows "air_quality_observations" 1

# Bronze WAL populated — proves EventBus → BronzeSubscriber path works
assert_bronze_wal_exists "air-quality"

# No crash — the fix must not break the app
RESTART_COUNT=$(docker inspect --format='{{.RestartCount}}' "integration-air-quality" 2>/dev/null || echo "0")
if [ "$RESTART_COUNT" = "0" ]; then
    _assert_pass "Container restart count = 0"
else
    _assert_fail "Container restart count = 0" "restarted ${RESTART_COUNT} times"
fi

echo "BUG-007 regression validations complete."
