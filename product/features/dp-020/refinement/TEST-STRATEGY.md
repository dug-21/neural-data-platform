# dp-020: Declarative Deploy - Test Strategy

## Overview

This document defines the comprehensive test strategy for dp-020 (Declarative Deploy). The testing approach follows NDP's established London School TDD patterns with behavior verification, focusing on ensuring the declarative manifest system correctly generates DDL, orchestrates deployments, and tracks device state.

---

## 1. Test Goals

1. **DDL Generation Accuracy** - Verify generated SQL matches expected output for all config types
2. **Idempotent Execution** - Confirm `deploy.sh apply` can run multiple times safely
3. **Schema Evolution** - Validate ADD COLUMN works for existing tables
4. **Correct Ordering** - Dependencies resolved automatically (migrations -> tables -> data)
5. **State Tracking** - Device state files updated correctly
6. **Error Handling** - Graceful failure with rollback where applicable

---

## 2. Test Pyramid

```
                    /\
                   /  \
                  / E2E \           T10+ (2 tests)
                 /-------\          Full deploy.sh apply workflow
                / Integr. \         T1-T9 (10 tests)
               /-----------\        Docker compose environment
              /    Unit     \       DDL-xxx (15+ tests)
             /---------------\      Isolated DDL generation
```

### Test Categories

| Level | Infrastructure | Run Command | Coverage Target |
|-------|---------------|-------------|-----------------|
| Unit | None | `cargo test` | 90%+ DDL generation |
| Integration | Docker (docker-compose.integration.yml) | `./integration-test-dp020.sh` | All declaration types |
| E2E | Full Pi simulation | Manual or CI | Happy path + rollback |

---

## 3. Test Environment

### Infrastructure

| Component | Container Name | Port | Purpose |
|-----------|---------------|------|---------|
| etcd | integration-etcd | 2379 | Configuration store |
| TimescaleDB | integration-timescaledb | 5432 | Silver layer database |
| MQTT | integration-mosquitto | 1883 | Message broker |
| MCP Server | integration-mcp-server | 9100 | MCP interface |

### Environment Variables

```bash
DEPLOY_ENV=integration          # Use integration compose file
ETCD_ENDPOINT=http://localhost:2379
TIMESCALE_URL=postgresql://postgres:postgres@localhost:5432/ndp
```

### Test Data Isolation

Test configurations use underscore prefix to exclude from production:

```
config/base/streams/_test-dp020/config.json    # Test stream config
.deploy/manifest.json                           # Test manifest (overwritten per test)
```

---

## 4. Unit Tests - DDL Generation

### 4.1 CREATE TABLE Generation

**Location**: `tools/ndp-ddl-gen/src/generator_test.rs` (or inline in deploy.sh tests)

#### DDL-001: Basic CREATE TABLE from field_mappings

| Field | Value |
|-------|-------|
| **Test ID** | DDL-001 |
| **Description** | Generate CREATE TABLE with columns from field_mappings |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_generate_create_table_basic() {
    // Arrange
    let config = silver_etl_config(
        "silver.test_readings",
        vec![
            field_mapping("pm25", "raw_payload.pm25", "float"),
            field_mapping("temperature", "raw_payload.temp", "float"),
        ]
    );

    // Act
    let ddl = generate_create_table(&config);

    // Assert
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS silver.test_readings"));
    assert!(ddl.contains("pm25 DOUBLE PRECISION"));
    assert!(ddl.contains("temperature DOUBLE PRECISION"));
    // Standard columns
    assert!(ddl.contains("timestamp TIMESTAMPTZ NOT NULL"));
    assert!(ddl.contains("ndp_id TEXT NOT NULL"));
    assert!(ddl.contains("dq_flags TEXT[]"));
}
```

#### DDL-002: Type Mapping Accuracy

| Field | Value |
|-------|-------|
| **Test ID** | DDL-002 |
| **Description** | Verify all config types map to correct PostgreSQL types |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_type_mapping_accuracy() {
    let mappings = vec![
        ("float", "DOUBLE PRECISION"),
        ("double_precision", "DOUBLE PRECISION"),
        ("int", "INTEGER"),
        ("integer", "INTEGER"),
        ("smallint", "SMALLINT"),
        ("bigint", "BIGINT"),
        ("text", "TEXT"),
        ("boolean", "BOOLEAN"),
        ("bool", "BOOLEAN"),
        ("timestamptz", "TIMESTAMPTZ"),
        ("jsonb", "JSONB"),
    ];

    for (config_type, pg_type) in mappings {
        let config = silver_etl_config_with_type("test_col", config_type);
        let ddl = generate_create_table(&config);
        assert!(
            ddl.contains(&format!("test_col {}", pg_type)),
            "Expected {} -> {}, DDL: {}",
            config_type, pg_type, ddl
        );
    }
}
```

#### DDL-003: Index Generation

| Field | Value |
|-------|-------|
| **Test ID** | DDL-003 |
| **Description** | Generate standard indexes (timestamp+ndp_id, dq_flags GIN) |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_index_generation() {
    let config = silver_etl_config("silver.test_readings", vec![]);

    let ddl = generate_indexes(&config);

    // Composite index on time+id
    assert!(ddl.contains("CREATE INDEX IF NOT EXISTS idx_test_readings_time_id"));
    assert!(ddl.contains("ON silver.test_readings (timestamp, ndp_id)"));

    // GIN index on dq_flags array
    assert!(ddl.contains("CREATE INDEX IF NOT EXISTS idx_test_readings_dq_flags"));
    assert!(ddl.contains("USING GIN (dq_flags)"));
}
```

#### DDL-004: Hypertable Conversion

| Field | Value |
|-------|-------|
| **Test ID** | DDL-004 |
| **Description** | Generate TimescaleDB hypertable conversion |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_hypertable_generation() {
    let config = silver_etl_config("silver.test_readings", vec![]);

    let ddl = generate_hypertable(&config);

    assert!(ddl.contains("SELECT create_hypertable('silver.test_readings', 'timestamp'"));
    assert!(ddl.contains("chunk_time_interval => INTERVAL '1 day'"));
    assert!(ddl.contains("if_not_exists => TRUE"));
}
```

#### DDL-005: Compression Policy

| Field | Value |
|-------|-------|
| **Test ID** | DDL-005 |
| **Description** | Generate compression policy with correct interval |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_compression_policy_generation() {
    let config = silver_etl_config("silver.test_readings", vec![]);

    let ddl = generate_compression_policy(&config);

    assert!(ddl.contains("SELECT add_compression_policy('silver.test_readings'"));
    assert!(ddl.contains("INTERVAL '7 days'"));
    assert!(ddl.contains("if_not_exists => TRUE"));
}
```

#### DDL-006: Retention Policy

| Field | Value |
|-------|-------|
| **Test ID** | DDL-006 |
| **Description** | Generate retention policy from config |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_retention_policy_generation() {
    let config = silver_etl_config_with_retention("silver.test_readings", 90);

    let ddl = generate_retention_policy(&config);

    assert!(ddl.contains("SELECT add_retention_policy('silver.test_readings'"));
    assert!(ddl.contains("INTERVAL '90 days'"));
    assert!(ddl.contains("if_not_exists => TRUE"));
}
```

#### DDL-007: Permissions

| Field | Value |
|-------|-------|
| **Test ID** | DDL-007 |
| **Description** | Grant permissions to standard roles |
| **Type** | Unit |
| **Priority** | Medium |

```rust
#[test]
fn test_permissions_generation() {
    let config = silver_etl_config("silver.test_readings", vec![]);

    let ddl = generate_permissions(&config);

    assert!(ddl.contains("GRANT SELECT, INSERT ON silver.test_readings TO ndp_app"));
    assert!(ddl.contains("GRANT SELECT ON silver.test_readings TO grafana_reader"));
}
```

### 4.2 ADD COLUMN Generation

#### DDL-008: ADD COLUMN for New Field

| Field | Value |
|-------|-------|
| **Test ID** | DDL-008 |
| **Description** | Generate ADD COLUMN wrapped in IF NOT EXISTS check |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_add_column_generation() {
    let ddl = generate_add_column(
        "silver.test_readings",
        "humidity",
        "DOUBLE PRECISION"
    );

    assert!(ddl.contains("DO $$"));
    assert!(ddl.contains("IF NOT EXISTS"));
    assert!(ddl.contains("information_schema.columns"));
    assert!(ddl.contains("table_name = 'test_readings'"));
    assert!(ddl.contains("column_name = 'humidity'"));
    assert!(ddl.contains("ALTER TABLE silver.test_readings ADD COLUMN humidity DOUBLE PRECISION"));
}
```

#### DDL-009: ADD COLUMN Multiple Fields

| Field | Value |
|-------|-------|
| **Test ID** | DDL-009 |
| **Description** | Generate multiple ADD COLUMN statements |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_add_multiple_columns() {
    let new_fields = vec![
        ("humidity", "DOUBLE PRECISION"),
        ("pressure", "DOUBLE PRECISION"),
        ("wind_speed", "DOUBLE PRECISION"),
    ];

    let ddl = generate_add_columns("silver.test_readings", new_fields);

    assert!(ddl.matches("ADD COLUMN").count() == 3);
    assert!(ddl.contains("humidity DOUBLE PRECISION"));
    assert!(ddl.contains("pressure DOUBLE PRECISION"));
    assert!(ddl.contains("wind_speed DOUBLE PRECISION"));
}
```

### 4.3 Manifest Parsing

#### DDL-010: Manifest Schema Validation

| Field | Value |
|-------|-------|
| **Test ID** | DDL-010 |
| **Description** | Validate manifest structure and required fields |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_manifest_parsing_valid() {
    let manifest = r#"{
        "version": "1.0",
        "changes": [
            {"type": "stream", "id": "air-quality", "action": "create"},
            {"type": "silver-table", "stream_id": "air-quality", "action": "sync"}
        ]
    }"#;

    let result: Result<Manifest, _> = serde_json::from_str(manifest);
    assert!(result.is_ok());

    let parsed = result.unwrap();
    assert_eq!(parsed.version, "1.0");
    assert_eq!(parsed.changes.len(), 2);
}
```

#### DDL-011: Invalid Manifest Rejected

| Field | Value |
|-------|-------|
| **Test ID** | DDL-011 |
| **Description** | Invalid manifest structure produces clear error |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_manifest_invalid_type_rejected() {
    let manifest = r#"{
        "version": "1.0",
        "changes": [
            {"type": "unknown-action", "id": "test"}
        ]
    }"#;

    let result: Result<Manifest, _> = serde_json::from_str(manifest);
    assert!(result.is_err());
}
```

---

## 5. Integration Tests

All integration tests require `docker-compose.integration.yml` to be running.

### Test Matrix

| ID | Scenario | Declaration Types | Verification |
|----|----------|------------------|--------------|
| T1 | New stream -> CREATE TABLE | stream, silver-table | `\d silver.{table}` shows all columns |
| T2 | Add field_mapping -> ADD COLUMN | stream, silver-table | Column added to existing table |
| T3 | Idempotent execution | all | Second run succeeds, no errors |
| T4 | Type mapping accuracy | silver-table | Each type maps correctly |
| T5 | Indexes created | silver-table | `\di silver.*` shows indexes |
| T6 | Hypertable conversion | silver-table | `timescaledb_information.hypertables` |
| T7 | Compression policy | silver-table | `timescaledb_information.jobs` |
| T8 | Retention policy | silver-table | `timescaledb_information.jobs` |
| T9 | Permissions | silver-table | Role can SELECT/INSERT |
| T10 | Device state files | all | `/var/ndp/deployed-*` exists |

### T1: New Stream Creates Silver Table

```bash
# Setup: Create test stream config
mkdir -p config/base/streams/_test-dp020
cat > config/base/streams/_test-dp020/config.json << 'EOF'
{
  "stream_id": "_test-dp020",
  "description": "Test stream for dp-020",
  "enabled": true,
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_readings",
    "field_mappings": [
      {"target_column": "test_value", "source_path": "raw_payload.value", "type": "float"}
    ]
  }
}
EOF

# Create manifest
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "_test-dp020", "action": "create"},
    {"type": "silver-table", "stream_id": "_test-dp020", "action": "sync"}
  ]
}
EOF

# Execute
DEPLOY_ENV=integration ./deploy.sh apply

# Verify
docker exec integration-timescaledb psql -U postgres -d ndp -c "\d silver._test_dp020_readings"
# Expected: Table exists with columns: timestamp, ndp_id, test_value, dq_flags, _bronze_id, _ingested_at
```

### T2: Add Column to Existing Table

```bash
# Setup: Modify config to add new field
cat > config/base/streams/_test-dp020/config.json << 'EOF'
{
  "stream_id": "_test-dp020",
  "description": "Test stream for dp-020 - updated",
  "enabled": true,
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_readings",
    "field_mappings": [
      {"target_column": "test_value", "source_path": "raw_payload.value", "type": "float"},
      {"target_column": "new_field", "source_path": "raw_payload.new", "type": "float"}
    ]
  }
}
EOF

# Create manifest
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "_test-dp020", "action": "update"},
    {"type": "silver-table", "stream_id": "_test-dp020", "action": "sync"}
  ]
}
EOF

# Execute
DEPLOY_ENV=integration ./deploy.sh apply

# Verify
docker exec integration-timescaledb psql -U postgres -d ndp -c "\d silver._test_dp020_readings" | grep new_field
# Expected: new_field column exists
```

### T3: Idempotent Execution

```bash
# Execute twice
DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE_1=$?

DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE_2=$?

# Verify
[ $EXIT_CODE_1 -eq 0 ] && [ $EXIT_CODE_2 -eq 0 ]
# Expected: Both runs succeed with exit code 0
```

### T4: Type Mapping Verification

```bash
# Setup: Config with all supported types
cat > config/base/streams/_test-dp020-types/config.json << 'EOF'
{
  "stream_id": "_test-dp020-types",
  "enabled": true,
  "silver_etl": {
    "enabled": true,
    "target_table": "silver._test_dp020_types",
    "field_mappings": [
      {"target_column": "col_float", "source_path": "$.float", "type": "float"},
      {"target_column": "col_int", "source_path": "$.int", "type": "integer"},
      {"target_column": "col_smallint", "source_path": "$.small", "type": "smallint"},
      {"target_column": "col_bigint", "source_path": "$.big", "type": "bigint"},
      {"target_column": "col_text", "source_path": "$.text", "type": "text"},
      {"target_column": "col_bool", "source_path": "$.bool", "type": "boolean"},
      {"target_column": "col_json", "source_path": "$.json", "type": "jsonb"}
    ]
  }
}
EOF

# Execute and verify each type
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT column_name, data_type
  FROM information_schema.columns
  WHERE table_schema = 'silver'
    AND table_name = '_test_dp020_types'
  ORDER BY column_name;
"
```

### T5-T9: Database Object Verification

```bash
# T5: Indexes
docker exec integration-timescaledb psql -U postgres -d ndp -c "\di silver.*_test_dp020*"

# T6: Hypertable
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT hypertable_name, num_dimensions
  FROM timescaledb_information.hypertables
  WHERE hypertable_name LIKE '_test_dp020%';
"

# T7: Compression policy
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT hypertable_name, schedule_interval
  FROM timescaledb_information.jobs
  WHERE proc_name = 'policy_compression'
    AND hypertable_name LIKE '_test_dp020%';
"

# T8: Retention policy
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  SELECT hypertable_name, config->'drop_after' as retention
  FROM timescaledb_information.jobs
  WHERE proc_name = 'policy_retention'
    AND hypertable_name LIKE '_test_dp020%';
"

# T9: Permissions
docker exec integration-timescaledb psql -U ndp_app -d ndp -c "
  SELECT 1 FROM silver._test_dp020_readings LIMIT 1;
"
```

### T10: Device State Files

```bash
# After deploy.sh apply:
[ -f /var/ndp/deployed-version ] && echo "deployed-version exists"
[ -f /var/ndp/deployed-at ] && echo "deployed-at exists"
[ -f /var/ndp/manifest-applied ] && echo "manifest-applied exists"

# Verify content
cat /var/ndp/deployed-version  # Should be git commit SHA
cat /var/ndp/deployed-at       # Should be ISO timestamp
```

---

## 5.1 Container Declaration Tests

Container declarations manage Docker image builds and container lifecycle operations.

### Test Matrix - Container Operations

| ID | Scenario | Declaration Type | Verification |
|----|----------|-----------------|--------------|
| T11 | Container build | container build | Docker image timestamp newer than before |
| T12 | Container restart | container restart | Container StartedAt newer than before |
| T13 | Build with no_cache | container build (no_cache: true) | Build output shows "Step X/Y" (not cached) |
| T14 | Container health after restart | container restart | Health status returns "healthy" |

### Container Build Verification Approach

**Objective**: Verify `deploy.sh apply` triggers Docker image rebuild when manifest declares container build.

**Verification Steps**:
1. Record image creation timestamp before: `docker inspect --format='{{.Created}}' ndp/air-quality-app:integration`
2. Apply manifest with container build declaration
3. Record image creation timestamp after
4. Assert: after timestamp > before timestamp

**Key Considerations**:
- Build may be slow (30-120 seconds)
- Test must handle "image not found" on first run
- `no_cache` option forces full rebuild without layer cache

### Container Restart Verification Approach

**Objective**: Verify `deploy.sh apply` restarts running containers when manifest declares restart.

**Verification Steps**:
1. Record container start time before: `docker inspect --format='{{.State.StartedAt}}' integration-air-quality`
2. Apply manifest with container restart declaration
3. Record container start time after
4. Assert: after start time > before start time

**Key Considerations**:
- Small delay (2s) needed between measurements to ensure distinguishable timestamps
- Container must be running before test
- Restart preserves volumes but refreshes container state

### Health Check Verification Approach

**Objective**: Verify container health status returns to "healthy" after restart.

**Verification Steps**:
1. Apply manifest with restart declaration
2. Wait for health check interval (may need polling with timeout)
3. Query health status: `docker inspect --format='{{.State.Health.Status}}' integration-air-quality`
4. Assert: status == "healthy"

**Key Considerations**:
- Health check may have start_period before first check
- Polling with timeout recommended (30s max)
- Container must have HEALTHCHECK defined in Dockerfile

---

## 6. Error Handling Tests

### E1: Invalid Manifest

```bash
# Create invalid manifest
cat > .deploy/manifest.json << 'EOF'
{ "invalid": "manifest" }
EOF

DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE=$?

[ $EXIT_CODE -ne 0 ]  # Should fail
# Verify: Error message mentions manifest validation
```

### E2: Database Connection Failure

```bash
# Stop TimescaleDB
docker stop integration-timescaledb

DEPLOY_ENV=integration ./deploy.sh apply
EXIT_CODE=$?

[ $EXIT_CODE -ne 0 ]  # Should fail
# Verify: Error message mentions database connection

# Restart for other tests
docker start integration-timescaledb
```

### E3: Partial Failure Rollback

```bash
# Create config that will fail mid-apply (e.g., invalid type)
# Verify: No partial state left behind
# Verify: Error message indicates what failed
```

---

## 7. Test Execution

### Running Integration Tests

```bash
# Start integration environment
./scripts/integration-test.sh start

# Run dp-020 integration tests
./scripts/integration-test-dp020.sh

# Clean up
./scripts/integration-test.sh clean
```

### Integration Test Script Location

`/workspaces/neural-data-platform/scripts/integration-test-dp020.sh`

### CI Configuration

```yaml
# .github/workflows/dp-020-tests.yml
jobs:
  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Start integration environment
        run: ./scripts/integration-test.sh start

      - name: Wait for services
        run: sleep 30

      - name: Run dp-020 tests
        run: ./scripts/integration-test-dp020.sh

      - name: Cleanup
        if: always()
        run: ./scripts/integration-test.sh clean
```

---

## 8. Test Data Management

### Test Stream Configs

All test configs use underscore prefix (`_test-dp020`) to:
- Exclude from production deployments
- Allow easy cleanup
- Avoid conflict with real streams

### Cleanup Procedure

```bash
# Remove test configs
rm -rf config/base/streams/_test-dp020*

# Remove test tables
docker exec integration-timescaledb psql -U postgres -d ndp -c "
  DROP TABLE IF EXISTS silver._test_dp020_readings CASCADE;
  DROP TABLE IF EXISTS silver._test_dp020_types CASCADE;
"

# Clear manifest
rm -f .deploy/manifest.json
```

---

## 9. Maturity Opportunities

### Reusable Test Patterns

1. **Test manifest template** - Create `tests/fixtures/manifests/` with common patterns
2. **DDL assertion helpers** - Functions to verify DDL contains expected clauses
3. **Database state snapshot** - Before/after comparison for idempotency tests

### Dry-Run Mode Testing

Future enhancement: `deploy.sh apply --dry-run` that:
- Generates all DDL
- Validates manifest
- Shows what would change
- Does NOT execute

Test pattern:
```bash
DEPLOY_ENV=integration ./deploy.sh apply --dry-run > /tmp/ddl-output.sql
# Verify DDL content
# Verify database NOT modified
```

### Golden File Testing

Store expected DDL output in `tests/fixtures/expected-ddl/` and compare:
```bash
./deploy.sh generate-ddl _test-dp020 > /tmp/actual.sql
diff tests/fixtures/expected-ddl/test-dp020.sql /tmp/actual.sql
```

---

## 10. Test Checklist

Before marking dp-020 testing complete:

- [ ] Unit tests for CREATE TABLE generation
- [ ] Unit tests for type mapping (all types)
- [ ] Unit tests for index generation
- [ ] Unit tests for hypertable conversion
- [ ] Unit tests for policy generation (compression, retention)
- [ ] Unit tests for permissions generation
- [ ] Unit tests for ADD COLUMN generation
- [ ] Unit tests for manifest parsing
- [ ] Integration test T1: New stream creates table
- [ ] Integration test T2: Add column to existing table
- [ ] Integration test T3: Idempotent execution
- [ ] Integration test T4: Type mapping accuracy
- [ ] Integration test T5: Indexes created
- [ ] Integration test T6: Hypertable conversion
- [ ] Integration test T7: Compression policy
- [ ] Integration test T8: Retention policy
- [ ] Integration test T9: Permissions work
- [ ] Integration test T10: Device state files
- [ ] Integration test T11: Container build
- [ ] Integration test T12: Container restart
- [ ] Integration test T13: Build with no_cache
- [ ] Integration test T14: Container health after restart
- [ ] Error handling tests (invalid manifest, DB down)
- [ ] Cleanup procedure documented and tested
- [ ] integration-test-dp020.sh script created and working

---

## 11. References

- [dp-020 SCOPE.md](../SCOPE.md) - Feature requirements
- [AIR-005-TEST-DESIGN.md](/workspaces/neural-data-platform/docs/testing/AIR-005-TEST-DESIGN.md) - London TDD patterns
- [dp-018 TEST-STRATEGY.md](/workspaces/neural-data-platform/product/features/dp-018/specification/TEST-STRATEGY.md) - JSON config testing
- [integration-test.sh](/workspaces/neural-data-platform/scripts/integration-test.sh) - Integration environment script

---

*Test Strategy created: 2026-02-02*
*SPARC Phase: Refinement (R)*
*Author: ndp-tester agent*
