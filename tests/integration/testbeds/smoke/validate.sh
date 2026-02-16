#!/usr/bin/env bash
# ops-007: Smoke testbed validation
# Sourced by run-testbed.sh (assert.sh already loaded)
#
# Validates:
#   - Core services are healthy
#   - etcd has domain config
#   - Silver table has rows from injected MQTT data
#   - Bronze WAL directory exists

echo "Running smoke validations..."

# Service health
assert_service_healthy "integration-mosquitto"
assert_service_healthy "integration-etcd"
assert_service_healthy "integration-timescaledb"
assert_service_healthy "integration-air-quality"

# etcd domain config (AC-03 verification)
assert_etcd_key "/domains/indoor-air-quality/config"

# Silver layer data (AC-05 verification)
assert_silver_rows "air_quality_observations" 1

# Bronze WAL (data was ingested)
assert_bronze_wal_exists "air-quality"

echo "Smoke validations complete."
