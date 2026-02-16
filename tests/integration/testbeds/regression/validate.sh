#!/usr/bin/env bash
# ops-007: Regression testbed validation
# Sourced by run-testbed.sh (assert.sh already loaded)
#
# Full layer coverage: smoke checks + Gold objects + all Silver tables + intelligence

echo "Running regression validations..."

# --- Smoke checks (baseline) ---

# Service health
assert_service_healthy "integration-mosquitto"
assert_service_healthy "integration-etcd"
assert_service_healthy "integration-timescaledb"
assert_service_healthy "integration-air-quality"

# etcd domain config
assert_etcd_key "/domains/indoor-air-quality/config"

# Bronze WAL
assert_bronze_wal_exists "air-quality"

# --- Silver layer (all tables) ---

assert_silver_rows "air_quality_observations" 1

# --- Gold layer (continuous aggregates created by deploy.sh) ---
# These verify that Gold DDL generation worked with the correct config path (AC-04)
assert_gold_object_exists "air_quality_observations_hourly"
assert_gold_object_exists "air_quality_observations_daily"

# --- Intelligence layer (if enabled) ---
if [ "${PREP_INTELLIGENCE:-false}" = "true" ]; then
    assert_service_healthy "integration-intelligence" || true
    # Embeddings may not exist yet if warmup threshold not met
    # This is informational, not a hard failure for regression
    assert_embedding_exists "indoor-air-quality" || true
fi

echo "Regression validations complete."
