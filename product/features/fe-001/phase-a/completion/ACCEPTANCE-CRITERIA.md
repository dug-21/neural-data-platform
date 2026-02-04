# FE-001 Phase A: Architecture Foundation - Acceptance Criteria

> **Phase:** A (Architecture Foundation)
> **Version:** 1.0
> **Created:** 2026-02-04
> **Last Updated:** 2026-02-04

---

## Overview

This document defines the acceptance criteria that must be satisfied before Phase A is considered complete. Each criterion follows the Given-When-Then format with specific verification commands.

---

## Feature Acceptance Criteria

### AC-A-01: Gold ETL JSON Schema Validates Correctly (v11-A01)

**Given:** A stream configuration file with a `gold_etl` section
**When:** The configuration is validated against `gold-etl.schema.json`
**Then:** Valid configurations pass, invalid configurations fail with descriptive errors

**Verification:**
```bash
# Test valid configuration passes
ndp-validate --schema config/schemas/gold-etl.schema.json \
  --config tests/fixtures/configs/valid/air_quality_gold_etl.json
# Expected: Exit code 0, no errors

# Test invalid configuration fails with helpful error
ndp-validate --schema config/schemas/gold-etl.schema.json \
  --config tests/fixtures/configs/invalid/unknown_metric.json
# Expected: Exit code non-zero, error mentions "unknown metric"
```

**Acceptance Checklist:**
- [ ] `config/schemas/gold-etl.schema.json` file exists
- [ ] Schema validates all valid test fixtures without error
- [ ] Schema rejects all invalid test fixtures with descriptive messages
- [ ] Schema supports all required metric types: mean, std, min, max, count, p95, p99
- [ ] Schema supports all granularity formats: "1 hour", "1 day", "15 minutes"
- [ ] Schema validates field references exist in stream fields

**Owner:** ndp-architect

---

### AC-A-02: Gold DDL Tool Generates Valid SQL (v11-A02)

**Given:** A valid stream configuration with `gold_etl` section enabled
**When:** Running `ndp-gold-ddl generate --stream <stream-id>`
**Then:** Valid TimescaleDB continuous aggregate SQL is produced

**Verification:**
```bash
# Generate SQL for air-quality stream
ndp-gold-ddl generate --stream air-quality --output /tmp/gold_ddl.sql

# Verify SQL contains expected elements
grep -q "CREATE MATERIALIZED VIEW" /tmp/gold_ddl.sql
grep -q "gold.air_quality_hourly" /tmp/gold_ddl.sql
grep -q "WITH (timescaledb.continuous)" /tmp/gold_ddl.sql
grep -q "time_bucket" /tmp/gold_ddl.sql

# Verify SQL syntax is valid (dry run against TimescaleDB)
cat /tmp/gold_ddl.sql | docker exec -i timescaledb psql -U postgres -d ndp -f -
# Expected: No syntax errors (objects may not exist)
```

**Acceptance Checklist:**
- [ ] `tools/ndp-gold-ddl/` directory exists with Cargo.toml
- [ ] `ndp-gold-ddl generate --stream air-quality` produces SQL to stdout
- [ ] Generated SQL includes `CREATE MATERIALIZED VIEW ... WITH (timescaledb.continuous)`
- [ ] Generated SQL includes `time_bucket()` function call
- [ ] Generated SQL includes all configured metrics (AVG, STDDEV, MIN, MAX, PERCENTILE)
- [ ] Generated SQL groups by bucket and entity (ndp_id)
- [ ] CLI supports `--action sync|recreate` modes
- [ ] CLI returns exit code 0 on success, non-zero on failure
- [ ] Errors are written to stderr, SQL to stdout

**Owner:** ndp-rust-dev

---

### AC-A-03: Alignment JSON Schema Validates Domain Configuration (v11-A03)

**Given:** A domain configuration file with alignment settings
**When:** The configuration is validated against `domain.schema.json`
**Then:** Valid domain configurations pass, invalid configurations fail

**Verification:**
```bash
# Validate domain configuration
ndp-validate --schema config/schemas/domain.schema.json \
  --config config/domains/indoor-air-quality/domain.yaml
# Expected: Exit code 0

# Test invalid domain config
ndp-validate --schema config/schemas/domain.schema.json \
  --config tests/fixtures/configs/invalid/domain_missing_streams.yaml
# Expected: Exit code non-zero, error mentions required streams
```

**Acceptance Checklist:**
- [ ] `config/schemas/domain.schema.json` file exists
- [ ] Schema validates stream references within domain
- [ ] Schema validates alignment configuration (granularity, join_strategy)
- [ ] Schema validates null_handling options (by_stream_type, preserve_null, coalesce)
- [ ] Schema validates stream role assignments (primary, context, actuator)
- [ ] Error messages identify which stream reference is invalid

**Owner:** ndp-architect

---

### AC-A-04: Alignment Interpreter Generates Aligned View SQL (v11-A04)

**Given:** A valid domain configuration with 3+ streams
**When:** Running `ndp-gold-ddl generate --domain <domain-id>`
**Then:** Valid FULL OUTER JOIN SQL is produced for the aligned view

**Verification:**
```bash
# Generate aligned view SQL
ndp-gold-ddl generate --domain indoor-air-quality --output /tmp/aligned.sql

# Verify SQL structure
grep -q "CREATE MATERIALIZED VIEW" /tmp/aligned.sql
grep -q "gold.indoor_air_quality_aligned" /tmp/aligned.sql
grep -q "FULL OUTER JOIN" /tmp/aligned.sql

# Count JOIN clauses (should be N-1 for N streams)
join_count=$(grep -c "FULL OUTER JOIN" /tmp/aligned.sql)
echo "JOIN count: $join_count"
# Expected: 2 for 3 streams (air-quality, outdoor-weather, home-assistant-state)
```

**Acceptance Checklist:**
- [ ] `ndp-gold-ddl generate --domain indoor-air-quality` produces SQL
- [ ] Generated SQL creates `gold.indoor_air_quality_aligned` view
- [ ] Generated SQL uses FULL OUTER JOIN strategy
- [ ] Generated SQL includes correct JOIN conditions on bucket
- [ ] Column names are aliased with stream prefix (e.g., `indoor_pm25`, `outdoor_temp`)
- [ ] NULL handling follows ADR-FE001-004 (by_stream_type default)

**Owner:** ndp-rust-dev

---

### AC-A-05: Objectives JSON Schema Validates Objective Definitions (v11-A05)

**Given:** A domain configuration with objectives section
**When:** The objectives section is validated against schema
**Then:** Valid objectives pass, invalid objectives fail with descriptive errors

**Verification:**
```bash
# Objectives are embedded in domain.yaml, validated together
ndp-validate --schema config/schemas/domain.schema.json \
  --config config/domains/indoor-air-quality/domain.yaml
# Expected: Exit code 0 (includes objectives validation)

# Test invalid objective
ndp-validate --schema config/schemas/domain.schema.json \
  --config tests/fixtures/configs/invalid/objective_bad_condition.yaml
# Expected: Error mentions invalid condition type
```

**Acceptance Checklist:**
- [ ] Objectives section validates within domain.schema.json
- [ ] Schema validates condition types: <, <=, >, >=, ==, between
- [ ] Schema validates threshold is numeric
- [ ] Schema validates metric reference exists in target stream
- [ ] Schema validates priority values (high, medium, low)
- [ ] Schema validates unit is optional string

**Owner:** ndp-architect

---

### AC-A-06: Feature Type Registry Is Extensible (v11-A06)

**Given:** The feature type registry with base types (lag, rolling, trend)
**When:** A new feature type trait is implemented
**Then:** The new feature type is recognized by the DDL generator

**Verification:**
```bash
# Run unit tests for feature registry
cargo test -p ndp-gold-ddl feature_registry

# Verify base feature types exist
cargo test -p ndp-gold-ddl test_lag_feature_type_registered
cargo test -p ndp-gold-ddl test_rolling_feature_type_registered
cargo test -p ndp-gold-ddl test_trend_feature_type_registered
```

**Acceptance Checklist:**
- [ ] `tools/ndp-gold-ddl/src/registry/` module exists
- [ ] `FeatureType` trait defined with `generate_sql()` method
- [ ] `LagFeatureType` implements trait for lag features
- [ ] `RollingFeatureType` implements trait for rolling window features
- [ ] `TrendFeatureType` implements trait for trend computation
- [ ] Registry lookup by feature type name works
- [ ] Unit tests demonstrate trait implementation for new type

**Owner:** ndp-rust-dev

---

## Integration Acceptance Criteria

### AC-A-INT-01: deploy.sh Handles gold-table Declarations

**Given:** A manifest with `gold-table` declaration type
**When:** Running `deploy.sh apply <manifest>`
**Then:** deploy.sh invokes ndp-gold-ddl with correct arguments

**Verification:**
```bash
# Create test manifest
cat > /tmp/test-manifest.json << 'EOF'
{
  "version": "test-phase-a",
  "declarations": {
    "gold-tables": [
      { "stream_id": "air-quality", "action": "sync" }
    ]
  }
}
EOF

# Dry run deploy
DEPLOY_ENV=integration deploy/pi/deploy.sh apply /tmp/test-manifest.json --dry-run 2>&1 | tee /tmp/deploy.log

# Verify ndp-gold-ddl is called
grep -q "ndp-gold-ddl" /tmp/deploy.log
# Expected: Log shows ndp-gold-ddl invocation
```

**Acceptance Checklist:**
- [ ] `deploy/pi/deploy.sh` contains `handle_gold_table()` function
- [ ] Function calls `ndp-gold-ddl generate --stream <stream-id>`
- [ ] Function passes `--action sync|recreate` from manifest
- [ ] Function handles errors and returns appropriate exit code
- [ ] Dry-run mode shows what would be executed

**Owner:** ndp-rust-dev

---

### AC-A-INT-02: ndp-validate Integrates Gold-Specific Validation

**Given:** A stream configuration with gold_etl section
**When:** Running ndp-validate semantic validation
**Then:** Gold-specific validation rules are applied

**Verification:**
```bash
# Test semantic validation catches invalid field reference
cat > /tmp/bad_gold.json << 'EOF'
{
  "stream_id": "test",
  "fields": [{"name": "pm25", "type": "float"}],
  "gold_etl": {
    "enabled": true,
    "aggregates": {
      "fields": {
        "nonexistent_field": { "metrics": ["mean"] }
      }
    }
  }
}
EOF

ndp-validate semantic --config /tmp/bad_gold.json
# Expected: Error code 400 (InvalidGoldField)
```

**Acceptance Checklist:**
- [ ] Error code 400 (InvalidGoldField) implemented
- [ ] Error code 401 (InvalidStreamType) implemented
- [ ] Error code 403 (InvalidAggregateMetric) implemented
- [ ] Error code 405 (InvalidFeatureType) implemented
- [ ] Error code 406 (InvalidGranularity) implemented
- [ ] All error codes produce helpful messages

**Owner:** ndp-rust-dev

---

### AC-A-INT-03: Two-Layer Validation Pipeline Works for Gold Configs

**Given:** Stream config with gold_etl section
**When:** Running full validation pipeline (schema + semantic)
**Then:** Both layers execute and report combined results

**Verification:**
```bash
# Run full validation
ndp-validate --config config/base/streams/air-quality/config.yaml --semantic
# Expected: Both schema and semantic validation pass

# Test semantic-only errors
ndp-validate --config tests/fixtures/configs/semantic_invalid_gold.yaml --semantic
# Expected: Schema passes, semantic fails with specific code
```

**Acceptance Checklist:**
- [ ] Schema validation runs first
- [ ] Semantic validation runs if schema passes
- [ ] Combined error report shows both layers
- [ ] Exit code reflects worst error

**Owner:** ndp-rust-dev

---

## Unit Test Acceptance Criteria

### AC-A-UNIT-01: Continuous Aggregate Generator Unit Tests Pass

**Given:** Unit test suite for continuous aggregate generator
**When:** Running `cargo test -p ndp-gold-ddl generators::continuous_aggregate`
**Then:** All tests pass with 90%+ line coverage

**Verification:**
```bash
# Run unit tests
cargo test -p ndp-gold-ddl generators::continuous_aggregate -- --nocapture

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin -p ndp-gold-ddl --out Html -- generators::continuous_aggregate
# Expected: Line coverage > 90%
```

**Acceptance Checklist:**
- [ ] Test: valid config generates SQL - PASSES
- [ ] Test: empty fields returns error - PASSES
- [ ] Test: unknown metric type returns error - PASSES
- [ ] Test: generated SQL contains time_bucket - PASSES
- [ ] Test: generated SQL groups by ndp_id - PASSES
- [ ] Test: generated SQL idempotent (CREATE OR REPLACE pattern) - PASSES
- [ ] Line coverage > 90%

**Owner:** ndp-tester

---

### AC-A-UNIT-02: Aligned View Generator Unit Tests Pass

**Given:** Unit test suite for aligned view generator
**When:** Running `cargo test -p ndp-gold-ddl generators::aligned_view`
**Then:** All tests pass

**Verification:**
```bash
cargo test -p ndp-gold-ddl generators::aligned_view -- --nocapture
```

**Acceptance Checklist:**
- [ ] Test: 2 streams generates 1 JOIN - PASSES
- [ ] Test: 3 streams generates 2 JOINs - PASSES
- [ ] Test: column aliases use stream prefix - PASSES
- [ ] Test: NULL handling respects stream_type - PASSES
- [ ] Test: observation stream preserves NULL - PASSES
- [ ] Test: state_event stream uses COALESCE - PASSES

**Owner:** ndp-tester

---

### AC-A-UNIT-03: Feature Registry Unit Tests Pass

**Given:** Unit test suite for feature type registry
**When:** Running `cargo test -p ndp-gold-ddl registry`
**Then:** All tests pass

**Verification:**
```bash
cargo test -p ndp-gold-ddl registry -- --nocapture
```

**Acceptance Checklist:**
- [ ] Test: lag feature registered - PASSES
- [ ] Test: rolling feature registered - PASSES
- [ ] Test: trend feature registered - PASSES
- [ ] Test: lookup by name works - PASSES
- [ ] Test: unknown type returns error - PASSES
- [ ] Test: custom type can be registered - PASSES

**Owner:** ndp-tester

---

## Performance Acceptance Criteria

### AC-A-PERF-01: DDL Generation Completes in Acceptable Time

**Given:** A stream configuration with full gold_etl section
**When:** Running ndp-gold-ddl generate
**Then:** Generation completes within 2 seconds

**Verification:**
```bash
time ndp-gold-ddl generate --stream air-quality > /dev/null
# Expected: real < 2.0s
```

**Acceptance Checklist:**
- [ ] Single stream DDL generation < 2 seconds
- [ ] Domain DDL generation (3 streams) < 5 seconds
- [ ] Memory usage < 50 MB during generation

**Owner:** ndp-tester

---

## Exit Criteria Summary

Phase A is complete when ALL of the following are true:

### Schema Deliverables
- [ ] AC-A-01: gold-etl.schema.json validates correctly
- [ ] AC-A-03: domain.schema.json validates correctly
- [ ] AC-A-05: Objectives schema validates within domain config

### Tool Deliverables
- [ ] AC-A-02: ndp-gold-ddl generates valid continuous aggregate SQL
- [ ] AC-A-04: ndp-gold-ddl generates valid aligned view SQL
- [ ] AC-A-06: Feature type registry is extensible via traits

### Integration Deliverables
- [ ] AC-A-INT-01: deploy.sh handles gold-table declarations
- [ ] AC-A-INT-02: ndp-validate includes Gold-specific validation
- [ ] AC-A-INT-03: Two-layer validation pipeline works

### Test Deliverables
- [ ] AC-A-UNIT-01: Continuous aggregate generator tests pass (90%+ coverage)
- [ ] AC-A-UNIT-02: Aligned view generator tests pass
- [ ] AC-A-UNIT-03: Feature registry tests pass

### Performance Deliverables
- [ ] AC-A-PERF-01: DDL generation completes within time budget

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| ndp-architect | | | |
| ndp-rust-dev | | | |
| ndp-tester | | | |

---

*Acceptance Criteria created: 2026-02-04 by ndp-tester*
