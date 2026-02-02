# dp-020: Declarative Deploy - Test Cases

## Overview

This document provides detailed test scenarios for dp-020 (Declarative Deploy). Each test case includes setup, execution, verification, and expected outcomes.

---

## Test Configuration

### Test Environment

| Setting | Value |
|---------|-------|
| Docker Compose | `docker-compose.integration.yml` |
| DEPLOY_ENV | `integration` |
| Test stream prefix | `_test-dp020` |
| Test table prefix | `silver._test_dp020_` |

### Test Stream Config Template

```json
{
  "stream_id": "_test-dp020-{scenario}",
  "description": "Test stream for dp-020 scenario {scenario}",
  "enabled": true,
  "version": "1.0.0",
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_{scenario}",
    "timestamp": {
      "source_field": "timestamp",
      "target_field": "timestamp",
      "transform": "microseconds_to_timestamp"
    },
    "field_mappings": []
  }
}
```

---

## T1: New Stream Creates Silver Table

### Purpose
Verify that declaring a new stream with `silver-table` action generates and executes CREATE TABLE DDL.

### Setup

1. Create test stream configuration:

```bash
mkdir -p config/base/streams/_test-dp020-t1
cat > config/base/streams/_test-dp020-t1/config.json << 'EOF'
{
  "stream_id": "_test-dp020-t1",
  "description": "Test stream T1 - CREATE TABLE",
  "enabled": true,
  "version": "1.0.0",
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_t1",
    "timestamp": {
      "source_field": "timestamp",
      "target_field": "timestamp",
      "transform": "microseconds_to_timestamp"
    },
    "field_mappings": [
      {
        "target_column": "pm25",
        "source_path": "raw_payload.pm25",
        "type": "float",
        "nullable": true,
        "description": "Particulate matter 2.5um"
      },
      {
        "target_column": "temperature",
        "source_path": "raw_payload.temperature",
        "type": "float",
        "nullable": true,
        "description": "Temperature in Celsius"
      }
    ]
  }
}
EOF
```

2. Create manifest:

```bash
mkdir -p .deploy
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "_test-dp020-t1", "action": "create"},
    {"type": "silver-table", "stream_id": "_test-dp020-t1", "action": "sync"}
  ]
}
EOF
```

### Execution

```bash
DEPLOY_ENV=integration ./deploy.sh apply
```

### Verification

```bash
# Check table exists with correct columns
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT column_name, data_type, is_nullable
  FROM information_schema.columns
  WHERE table_schema = 'silver'
    AND table_name = '_test_dp020_t1'
  ORDER BY ordinal_position;
"
```

### Expected Output

| column_name | data_type | is_nullable |
|-------------|-----------|-------------|
| timestamp | timestamp with time zone | NO |
| ndp_id | text | NO |
| pm25 | double precision | YES |
| temperature | double precision | YES |
| dq_flags | ARRAY | YES |
| _bronze_id | uuid | YES |
| _ingested_at | timestamp with time zone | YES |

### Cleanup

```bash
rm -rf config/base/streams/_test-dp020-t1
docker exec integration-timescaledb psql -U postgres -d ndp -c "DROP TABLE IF EXISTS silver._test_dp020_t1 CASCADE;"
```

---

## T2: Add Field Mapping Creates Column

### Purpose
Verify that adding a new `field_mapping` to an existing stream's config generates ADD COLUMN DDL.

### Precondition
T1 completed successfully (table exists with pm25, temperature columns).

### Setup

1. Update stream config with new field:

```bash
cat > config/base/streams/_test-dp020-t1/config.json << 'EOF'
{
  "stream_id": "_test-dp020-t1",
  "description": "Test stream T1 - ADD COLUMN",
  "enabled": true,
  "version": "1.1.0",
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_t1",
    "timestamp": {
      "source_field": "timestamp",
      "target_field": "timestamp",
      "transform": "microseconds_to_timestamp"
    },
    "field_mappings": [
      {
        "target_column": "pm25",
        "source_path": "raw_payload.pm25",
        "type": "float",
        "nullable": true
      },
      {
        "target_column": "temperature",
        "source_path": "raw_payload.temperature",
        "type": "float",
        "nullable": true
      },
      {
        "target_column": "humidity",
        "source_path": "raw_payload.humidity",
        "type": "float",
        "nullable": true,
        "description": "Relative humidity percentage"
      }
    ]
  }
}
EOF
```

2. Create manifest:

```bash
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "_test-dp020-t1", "action": "update"},
    {"type": "silver-table", "stream_id": "_test-dp020-t1", "action": "sync"}
  ]
}
EOF
```

### Execution

```bash
DEPLOY_ENV=integration ./deploy.sh apply
```

### Verification

```bash
# Check humidity column exists
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT column_name, data_type
  FROM information_schema.columns
  WHERE table_schema = 'silver'
    AND table_name = '_test_dp020_t1'
    AND column_name = 'humidity';
"
```

### Expected Output

| column_name | data_type |
|-------------|-----------|
| humidity | double precision |

### Additional Verification

```bash
# Verify existing data not affected (if any)
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT COUNT(*) as row_count FROM silver._test_dp020_t1;
"
```

---

## T3: Idempotent Execution

### Purpose
Verify that running `deploy.sh apply` multiple times produces no errors and no duplicate objects.

### Setup
Use existing T1/T2 configuration.

### Execution

```bash
# First run
DEPLOY_ENV=integration ./deploy.sh apply
RESULT_1=$?

# Second run (idempotent)
DEPLOY_ENV=integration ./deploy.sh apply
RESULT_2=$?

# Third run (idempotent)
DEPLOY_ENV=integration ./deploy.sh apply
RESULT_3=$?
```

### Verification

```bash
# All runs should succeed
echo "Run 1 exit code: $RESULT_1"
echo "Run 2 exit code: $RESULT_2"
echo "Run 3 exit code: $RESULT_3"

# Table should exist exactly once
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT COUNT(*)
  FROM information_schema.tables
  WHERE table_schema = 'silver'
    AND table_name = '_test_dp020_t1';
"

# Index should exist exactly once
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT COUNT(*)
  FROM pg_indexes
  WHERE schemaname = 'silver'
    AND tablename = '_test_dp020_t1'
    AND indexname LIKE 'idx_%';
"
```

### Expected Output

- All exit codes = 0
- Table count = 1
- Index count >= 2 (time_id composite, dq_flags GIN)

---

## T4: Type Mapping Accuracy

### Purpose
Verify each supported config type maps to the correct PostgreSQL type.

### Setup

```bash
mkdir -p config/base/streams/_test-dp020-t4
cat > config/base/streams/_test-dp020-t4/config.json << 'EOF'
{
  "stream_id": "_test-dp020-t4",
  "description": "Test stream T4 - Type Mapping",
  "enabled": true,
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_t4_types",
    "timestamp": {
      "source_field": "ts",
      "target_field": "timestamp",
      "transform": "iso8601"
    },
    "field_mappings": [
      {"target_column": "col_float", "source_path": "$.float", "type": "float"},
      {"target_column": "col_double", "source_path": "$.double", "type": "double_precision"},
      {"target_column": "col_int", "source_path": "$.int", "type": "integer"},
      {"target_column": "col_smallint", "source_path": "$.small", "type": "smallint"},
      {"target_column": "col_bigint", "source_path": "$.big", "type": "bigint"},
      {"target_column": "col_text", "source_path": "$.text", "type": "text"},
      {"target_column": "col_bool", "source_path": "$.bool", "type": "boolean"},
      {"target_column": "col_ts", "source_path": "$.ts", "type": "timestamptz"},
      {"target_column": "col_json", "source_path": "$.json", "type": "jsonb"}
    ]
  }
}
EOF

cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "_test-dp020-t4", "action": "create"},
    {"type": "silver-table", "stream_id": "_test-dp020-t4", "action": "sync"}
  ]
}
EOF
```

### Execution

```bash
DEPLOY_ENV=integration ./deploy.sh apply
```

### Verification

```bash
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT column_name, data_type
  FROM information_schema.columns
  WHERE table_schema = 'silver'
    AND table_name = '_test_dp020_t4_types'
    AND column_name LIKE 'col_%'
  ORDER BY column_name;
"
```

### Expected Output

| column_name | data_type |
|-------------|-----------|
| col_bigint | bigint |
| col_bool | boolean |
| col_double | double precision |
| col_float | double precision |
| col_int | integer |
| col_json | jsonb |
| col_smallint | smallint |
| col_text | text |
| col_ts | timestamp with time zone |

---

## T5: Indexes Created

### Purpose
Verify standard indexes are created on Silver tables.

### Precondition
T1 table exists.

### Verification

```bash
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT indexname, indexdef
  FROM pg_indexes
  WHERE schemaname = 'silver'
    AND tablename = '_test_dp020_t1'
  ORDER BY indexname;
"
```

### Expected Indexes

| Index Pattern | Purpose |
|---------------|---------|
| `idx_*_time_id` | Composite index on (timestamp, ndp_id) |
| `idx_*_dq_flags` | GIN index on dq_flags array |
| `*_pkey` or hypertable index | Primary key / chunk index |

---

## T6: Hypertable Conversion

### Purpose
Verify Silver tables are converted to TimescaleDB hypertables.

### Precondition
T1 table exists.

### Verification

```bash
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT hypertable_schema, hypertable_name, num_dimensions, compression_enabled
  FROM timescaledb_information.hypertables
  WHERE hypertable_name = '_test_dp020_t1';
"
```

### Expected Output

| hypertable_schema | hypertable_name | num_dimensions | compression_enabled |
|-------------------|-----------------|----------------|---------------------|
| silver | _test_dp020_t1 | 1 | t |

---

## T7: Compression Policy

### Purpose
Verify compression policy is applied to hypertables.

### Precondition
T1 table exists as hypertable.

### Verification

```bash
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT j.hypertable_name, j.schedule_interval, j.config
  FROM timescaledb_information.jobs j
  WHERE j.proc_name = 'policy_compression'
    AND j.hypertable_name = '_test_dp020_t1';
"
```

### Expected Output

- `schedule_interval` is set (e.g., 12 hours)
- `config->compress_after` = '7 days' (or configured value)

---

## T8: Retention Policy

### Purpose
Verify retention policy is applied to hypertables.

### Precondition
T1 table exists as hypertable.

### Verification

```bash
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT j.hypertable_name, j.schedule_interval, j.config->>'drop_after' as retention_days
  FROM timescaledb_information.jobs j
  WHERE j.proc_name = 'policy_retention'
    AND j.hypertable_name = '_test_dp020_t1';
"
```

### Expected Output

| hypertable_name | schedule_interval | retention_days |
|-----------------|-------------------|----------------|
| _test_dp020_t1 | 1 day | 90 days |

---

## T9: Permissions

### Purpose
Verify database roles have correct permissions on Silver tables.

### Precondition
T1 table exists. Roles `ndp_app` and `grafana_reader` exist.

### Setup (if roles don't exist)

```bash
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  CREATE ROLE ndp_app WITH LOGIN PASSWORD 'ndp_app';
  CREATE ROLE grafana_reader WITH LOGIN PASSWORD 'grafana_reader';
"
```

### Verification

```bash
# ndp_app can SELECT and INSERT
docker exec integration-timescaledb psql -U ndp_app -d ndp -c "
  SELECT 1 FROM silver._test_dp020_t1 LIMIT 1;
"

# grafana_reader can SELECT only
docker exec integration-timescaledb psql -U grafana_reader -d ndp -c "
  SELECT 1 FROM silver._test_dp020_t1 LIMIT 1;
"

# grafana_reader cannot INSERT (should fail)
docker exec integration-timescaledb psql -U grafana_reader -d ndp -c "
  INSERT INTO silver._test_dp020_t1 (timestamp, ndp_id) VALUES (NOW(), 'test');
" 2>&1 | grep -q "permission denied" && echo "PASS: grafana_reader cannot INSERT"
```

### Expected Output

- ndp_app SELECT succeeds
- grafana_reader SELECT succeeds
- grafana_reader INSERT fails with "permission denied"

---

## T10: Device State Files

### Purpose
Verify deployment tracking files are created in `/var/ndp/`.

### Setup
Run any deploy.sh apply command.

### Verification

```bash
# In integration environment, state is in container or local
# For local testing:

# Check deployed-version (should be git commit SHA)
if [ -f /var/ndp/deployed-version ]; then
  DEPLOYED_VERSION=$(cat /var/ndp/deployed-version)
  CURRENT_SHA=$(git rev-parse HEAD)
  [ "$DEPLOYED_VERSION" = "$CURRENT_SHA" ] && echo "PASS: deployed-version matches HEAD"
fi

# Check deployed-at (should be ISO timestamp)
if [ -f /var/ndp/deployed-at ]; then
  DEPLOYED_AT=$(cat /var/ndp/deployed-at)
  echo "Deployed at: $DEPLOYED_AT"
  # Verify format is ISO 8601
  echo "$DEPLOYED_AT" | grep -qE "^[0-9]{4}-[0-9]{2}-[0-9]{2}T" && echo "PASS: deployed-at is ISO format"
fi

# Check manifest-applied (should be hash of manifest)
if [ -f /var/ndp/manifest-applied ]; then
  MANIFEST_HASH=$(cat /var/ndp/manifest-applied)
  CURRENT_HASH=$(sha256sum .deploy/manifest.json | cut -d' ' -f1)
  [ "$MANIFEST_HASH" = "$CURRENT_HASH" ] && echo "PASS: manifest-applied hash matches"
fi
```

### Expected Output

- `/var/ndp/deployed-version` exists and contains git SHA
- `/var/ndp/deployed-at` exists and contains ISO timestamp
- `/var/ndp/manifest-applied` exists and contains manifest hash

---

## Container Declaration Tests

### T11: Container Build

### Purpose
Verify that declaring a container build action triggers Docker image rebuild.

### Setup

1. Ensure air-quality-app Dockerfile exists and container can be built
2. Record image timestamp before test

### Execution

```bash
# Get image timestamp before
BEFORE=$(docker inspect --format='{{.Created}}' ndp/air-quality-app:integration 2>/dev/null || echo "none")

# Create manifest with container build
mkdir -p .deploy
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "container", "target": "air-quality-app", "action": "build"}
  ]
}
EOF

# Execute
DEPLOY_ENV=integration ./deploy.sh apply

# Get image timestamp after
AFTER=$(docker inspect --format='{{.Created}}' ndp/air-quality-app:integration)
```

### Verification

```bash
# Compare timestamps
if [ "$BEFORE" != "$AFTER" ]; then
    echo "PASS: Container image was rebuilt"
else
    echo "FAIL: Container image was not rebuilt"
fi
```

### Expected Output

- Image timestamp changes after apply
- Build output shows successful image creation
- No errors in deploy output

---

### T12: Container Restart

### Purpose
Verify that declaring a container restart action restarts the running container.

### Precondition
Container `integration-air-quality` must be running.

### Setup

```bash
# Ensure container is running
docker ps | grep integration-air-quality || echo "Container not running"

# Get container start time before
BEFORE=$(docker inspect --format='{{.State.StartedAt}}' integration-air-quality)
```

### Execution

```bash
# Small delay to ensure timestamp difference
sleep 2

# Create manifest with container restart
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "container", "target": "air-quality-app", "action": "restart"}
  ]
}
EOF

# Execute
DEPLOY_ENV=integration ./deploy.sh apply

# Get container start time after
AFTER=$(docker inspect --format='{{.State.StartedAt}}' integration-air-quality)
```

### Verification

```bash
# Compare start times
if [ "$BEFORE" != "$AFTER" ]; then
    echo "PASS: Container was restarted"
else
    echo "FAIL: Container was not restarted"
fi
```

### Expected Output

- Container StartedAt timestamp changes after apply
- Container status is "running" after restart
- No errors in deploy output

---

### T13: Build with no_cache

### Purpose
Verify that `no_cache: true` option forces a full Docker image rebuild without using layer cache.

### Setup

```bash
# Ensure a cached image exists first (run T11 if not)
docker images | grep "ndp/air-quality-app" || echo "No cached image"
```

### Execution

```bash
# Create manifest with no_cache build
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "container", "target": "air-quality-app", "action": "build", "no_cache": true}
  ]
}
EOF

# Execute and capture output
DEPLOY_ENV=integration ./deploy.sh apply 2>&1 | tee /tmp/build-output.log
```

### Verification

```bash
# Check that build output shows actual build steps (not "Using cache")
if grep -q "Step [0-9]*/[0-9]*" /tmp/build-output.log && ! grep -q "Using cache" /tmp/build-output.log; then
    echo "PASS: Build ran without cache"
else
    # Alternative: newer Docker buildx output
    if grep -q "CACHED" /tmp/build-output.log; then
        echo "FAIL: Build used cache despite no_cache flag"
    else
        echo "PASS: Build appears to have run without cache"
    fi
fi
```

### Expected Output

- Build output shows "Step X/Y" without "Using cache" indicators
- Image timestamp is updated
- Full rebuild takes longer than cached build

---

### T14: Container Health After Restart

### Purpose
Verify that container health status returns to "healthy" after a restart operation.

### Precondition
Container must have HEALTHCHECK defined in Dockerfile.

### Setup

```bash
# Ensure container is running and healthy before test
docker inspect --format='{{.State.Health.Status}}' integration-air-quality
```

### Execution

```bash
# Create manifest with restart
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "container", "target": "air-quality-app", "action": "restart"}
  ]
}
EOF

# Execute
DEPLOY_ENV=integration ./deploy.sh apply

# Wait for health check (with timeout)
TIMEOUT=30
ELAPSED=0
while [ $ELAPSED -lt $TIMEOUT ]; do
    STATUS=$(docker inspect --format='{{.State.Health.Status}}' integration-air-quality 2>/dev/null || echo "unknown")
    if [ "$STATUS" = "healthy" ]; then
        break
    fi
    sleep 2
    ELAPSED=$((ELAPSED + 2))
done
```

### Verification

```bash
STATUS=$(docker inspect --format='{{.State.Health.Status}}' integration-air-quality)
if [ "$STATUS" = "healthy" ]; then
    echo "PASS: Container is healthy after restart"
else
    echo "FAIL: Container health status is '$STATUS', expected 'healthy'"
fi
```

### Expected Output

- Health status transitions from "starting" to "healthy"
- Verification completes within timeout (30s)
- No health check failures in container logs

---

## Error Cases

### E1: Invalid Manifest

```bash
# Create malformed manifest
cat > .deploy/manifest.json << 'EOF'
{ "invalid": "structure", "no_changes": true }
EOF

DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE=$?

[ $EXIT_CODE -ne 0 ] && echo "PASS: Invalid manifest rejected"
```

### E2: Missing Stream Config

```bash
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "nonexistent-stream", "action": "create"}
  ]
}
EOF

DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE=$?

[ $EXIT_CODE -ne 0 ] && echo "PASS: Missing stream config detected"
```

### E3: Database Connection Failure

```bash
# Stop TimescaleDB
docker stop integration-timescaledb

DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE=$?

[ $EXIT_CODE -ne 0 ] && echo "PASS: DB connection failure handled"

# Restart
docker start integration-timescaledb
sleep 5
```

### E4: Invalid Type in Config

```bash
mkdir -p config/base/streams/_test-dp020-e4
cat > config/base/streams/_test-dp020-e4/config.json << 'EOF'
{
  "stream_id": "_test-dp020-e4",
  "enabled": true,
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_e4",
    "field_mappings": [
      {"target_column": "bad_col", "source_path": "$.bad", "type": "invalid_type_xyz"}
    ]
  }
}
EOF

cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "silver-table", "stream_id": "_test-dp020-e4", "action": "sync"}
  ]
}
EOF

DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE=$?

# Should either fail validation or use TEXT as fallback
```

---

## Full Test Cleanup

```bash
# Remove all test stream configs
rm -rf config/base/streams/_test-dp020*

# Drop all test tables
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  DO \$\$
  DECLARE
    r RECORD;
  BEGIN
    FOR r IN SELECT tablename FROM pg_tables
             WHERE schemaname = 'silver'
               AND tablename LIKE '_test_dp020%'
    LOOP
      EXECUTE 'DROP TABLE IF EXISTS silver.' || quote_ident(r.tablename) || ' CASCADE';
    END LOOP;
  END \$\$;
"

# Clear manifest
rm -f .deploy/manifest.json

# Clear device state (if testing locally)
sudo rm -f /var/ndp/deployed-version /var/ndp/deployed-at /var/ndp/manifest-applied

echo "Cleanup complete"
```

---

## Test Summary Matrix

| ID | Scenario | Declaration Types | Priority | Automated |
|----|----------|------------------|----------|-----------|
| T1 | New stream -> CREATE TABLE | stream, silver-table | Critical | Yes |
| T2 | Add field -> ADD COLUMN | stream, silver-table | Critical | Yes |
| T3 | Idempotent execution | all | Critical | Yes |
| T4 | Type mapping accuracy | silver-table | Critical | Yes |
| T5 | Indexes created | silver-table | High | Yes |
| T6 | Hypertable conversion | silver-table | High | Yes |
| T7 | Compression policy | silver-table | High | Yes |
| T8 | Retention policy | silver-table | High | Yes |
| T9 | Permissions | silver-table | Medium | Yes |
| T10 | Device state files | all | High | Yes |
| T11 | Container build | container | High | Yes |
| T12 | Container restart | container | High | Yes |
| T13 | Build with no_cache | container | Medium | Yes |
| T14 | Container health after restart | container | High | Yes |
| E1 | Invalid manifest | - | High | Yes |
| E2 | Missing stream config | stream | High | Yes |
| E3 | Database connection failure | - | High | Yes |
| E4 | Invalid type in config | silver-table | Medium | Yes |

---

*Test Cases created: 2026-02-02*
*SPARC Phase: Refinement (R)*
*Author: ndp-tester agent*
