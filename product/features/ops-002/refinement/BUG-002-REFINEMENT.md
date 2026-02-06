# BUG-002 Refinement: Domain Objectives Sync London TDD Test Plan

> **Bug:** BUG-002 - Domain objectives sync not migrated to Rust toolchain
> **Feature:** ops-002
> **Phase:** Refinement (SPARC R)
> **Approach:** London School TDD - outside-in, mock collaborators, test behavior
> **Reference Implementation:** `crates/ndp-lib/src/dictionary/mod.rs` (20+ London TDD tests)
> **Target Module:** `crates/ndp-lib/src/domain/mod.rs`
> **Created:** 2026-02-06

---

## 1. Test-First Overview

London TDD dictates that we write failing tests FIRST describing the sync behavior for all four domain tables, then implement `sync_domains()` to make them pass. The tests define a contract: "given parsed `DomainSyncEntry` structs, the executed SQL must contain the correct parameterized statements in the correct order."

### Reference Pattern: dictionary/mod.rs

The dictionary sync tests establish the project pattern:

1. **MockDbClient** records all `execute()` and `batch_execute()` calls into a `Mutex<Vec<SqlCall>>`.
2. **Helper constructors** (`make_minimal_stream`, `make_field`, etc.) build minimal test fixtures.
3. **Assertions** verify SQL statement text, parameter counts, query ordering, and report counts.
4. **No real database** -- all unit tests run against the mock.

This refinement follows that pattern exactly for the domain sync operation.

### Target Tables (from `005_domain_objectives.sql`)

| Table | Sync Strategy | Key Columns |
|-------|--------------|-------------|
| `data_dictionary.domains` | UPSERT (ON CONFLICT) | domain_id, description, stream_count, config_path |
| `data_dictionary.domain_streams` | DELETE + INSERT per domain | domain_id, stream_id, alias, role |
| `data_dictionary.objectives` | DELETE + INSERT per domain | objective_id, domain_id, description, target_stream, target_metric, condition, threshold, threshold_upper, unit, priority |
| `data_dictionary.constraints` | DELETE + INSERT per domain | constraint_id, domain_id, description, constraint_stream, constraint_metric, condition, threshold, unit |

### Test Execution Order

| Step | Category | Count | Purpose | Run With |
|------|----------|-------|---------|----------|
| 1 | Unit Tests (sync logic) | 18 | Core sync behavior against MockDbClient | `cargo test -p ndp-lib` |
| 2 | ConfigLoader Tests | 5 | Domain config discovery and parsing | `cargo test -p ndp-lib` |
| 3 | Conversion Tests | 3 | DomainConfig to DomainSyncEntry conversion | `cargo test -p ndp-lib` |
| 4 | Integration Tests | 3 | Verify against real TimescaleDB | `cargo test -p ndp-lib -- --ignored` |
| 5 | CLI Tests | 1 | End-to-end `ndp domain sync` | `cargo test -p ndp-cli` |

---

## 2. Mock Infrastructure

### 2.1 MockDbClient (reuse dictionary pattern)

```rust
// crates/ndp-lib/src/domain/mod.rs (test module)

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SqlCall {
    query: String,
    params: Vec<String>,
}

struct MockDbClient {
    calls: Mutex<Vec<SqlCall>>,
}

impl MockDbClient {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<SqlCall> {
        self.calls.lock().unwrap().clone()
    }

    fn calls_starting_with(&self, prefix: &str) -> Vec<SqlCall> {
        self.calls()
            .into_iter()
            .filter(|c| c.query.starts_with(prefix))
            .collect()
    }

    fn query_strings(&self) -> Vec<String> {
        self.calls().iter().map(|c| c.query.clone()).collect()
    }
}

#[async_trait]
impl DbClient for MockDbClient {
    async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>> {
        let param_strs: Vec<String> = params.iter().map(|p| format!("{:?}", p)).collect();
        self.calls.lock().unwrap().push(SqlCall {
            query: query.to_string(),
            params: param_strs,
        });
        Ok(vec![])
    }

    async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64> {
        let param_strs: Vec<String> = params.iter().map(|p| format!("{:?}", p)).collect();
        self.calls.lock().unwrap().push(SqlCall {
            query: query.to_string(),
            params: param_strs,
        });
        Ok(1)
    }

    async fn batch_execute(&self, sql_text: &str) -> Result<()> {
        self.calls.lock().unwrap().push(SqlCall {
            query: sql_text.to_string(),
            params: vec![],
        });
        Ok(())
    }
}
```

### 2.2 MockDbClient with Error Injection

For test 17 (error handling), extend with a variant that fails on specific queries:

```rust
struct FailingMockDbClient {
    inner: MockDbClient,
    fail_on_domain: Option<String>,
}

impl FailingMockDbClient {
    fn new(fail_on_domain: &str) -> Self {
        Self {
            inner: MockDbClient::new(),
            fail_on_domain: Some(fail_on_domain.to_string()),
        }
    }
}

#[async_trait]
impl DbClient for FailingMockDbClient {
    async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64> {
        // Check if first param matches the fail domain
        if query.contains("data_dictionary.domains") {
            let param_strs: Vec<String> = params.iter().map(|p| format!("{:?}", p)).collect();
            if let Some(ref fail_id) = self.fail_on_domain {
                if param_strs.first().map_or(false, |p| p.contains(fail_id)) {
                    return Err(NdpLibError::Database("simulated failure".to_string()));
                }
            }
        }
        self.inner.execute(query, params).await
    }
    // delegate query() and batch_execute() to inner
}
```

### 2.3 Test Helpers

```rust
/// Minimal domain with no streams, objectives, or constraints.
fn make_minimal_domain(id: &str) -> DomainSyncEntry {
    DomainSyncEntry {
        domain_id: id.to_string(),
        description: "Test domain".to_string(),
        streams: vec![],
        objectives: vec![],
        constraints: vec![],
    }
}

/// Objective targeting a single metric with a standard comparison condition.
fn make_objective(id: &str, stream: &str, metric: &str) -> ObjectiveSyncEntry {
    ObjectiveSyncEntry {
        objective_id: id.to_string(),
        description: format!("Test objective for {}", metric),
        target_stream: stream.to_string(),
        target_metric: metric.to_string(),
        condition: "<".to_string(),
        threshold: 100.0,
        threshold_upper: None,
        unit: Some("units".to_string()),
        priority: "medium".to_string(),
    }
}

/// Stream mapping entry for a domain.
fn make_stream_mapping(stream_id: &str, alias: &str, role: &str) -> StreamMappingEntry {
    StreamMappingEntry {
        stream_id: stream_id.to_string(),
        alias: alias.to_string(),
        role: role.to_string(),
    }
}

/// Constraint entry for a domain.
fn make_constraint(id: &str, stream: &str, metric: &str) -> ConstraintSyncEntry {
    ConstraintSyncEntry {
        constraint_id: id.to_string(),
        description: format!("Test constraint for {}", metric),
        constraint_stream: stream.to_string(),
        constraint_metric: metric.to_string(),
        condition: ">".to_string(),
        threshold: 0.0,
        unit: Some("units".to_string()),
    }
}

fn opts() -> SyncOptions {
    SyncOptions { dry_run: false }
}
```

---

## 3. Type Definitions

These types live in `crates/ndp-lib/src/domain/types.rs`:

```rust
/// A domain ready for sync to data_dictionary tables.
/// Parsed from domain.json, converted by the caller.
#[derive(Debug, Clone)]
pub struct DomainSyncEntry {
    pub domain_id: String,
    pub description: String,
    pub streams: Vec<StreamMappingEntry>,
    pub objectives: Vec<ObjectiveSyncEntry>,
    pub constraints: Vec<ConstraintSyncEntry>,
}

/// A stream-to-domain mapping.
#[derive(Debug, Clone)]
pub struct StreamMappingEntry {
    pub stream_id: String,
    pub alias: String,
    pub role: String,
}

/// An objective to sync.
#[derive(Debug, Clone)]
pub struct ObjectiveSyncEntry {
    pub objective_id: String,
    pub description: String,
    pub target_stream: String,
    pub target_metric: String,
    pub condition: String,
    pub threshold: f64,
    pub threshold_upper: Option<f64>,
    pub unit: Option<String>,
    pub priority: String,
}

/// A constraint to sync.
#[derive(Debug, Clone)]
pub struct ConstraintSyncEntry {
    pub constraint_id: String,
    pub description: String,
    pub constraint_stream: String,
    pub constraint_metric: String,
    pub condition: String,
    pub threshold: f64,
    pub unit: Option<String>,
}
```

---

## 4. SQL Constants

These constants live in `crates/ndp-lib/src/domain/sql.rs`:

```rust
/// UPSERT domain into data_dictionary.domains.
/// $1=domain_id, $2=description, $3=stream_count, $4=config_path
pub const UPSERT_DOMAIN: &str =
    "INSERT INTO data_dictionary.domains (domain_id, description, stream_count, config_path) \
     VALUES ($1, $2, $3, $4) \
     ON CONFLICT (domain_id) DO UPDATE SET \
     description = EXCLUDED.description, \
     stream_count = EXCLUDED.stream_count, \
     config_path = EXCLUDED.config_path, \
     updated_at = NOW()";

/// DELETE domain_streams for a specific domain.
/// $1=domain_id
pub const DELETE_DOMAIN_STREAMS: &str =
    "DELETE FROM data_dictionary.domain_streams WHERE domain_id = $1";

/// INSERT a single domain_stream mapping.
/// $1=domain_id, $2=stream_id, $3=alias, $4=role
pub const INSERT_DOMAIN_STREAM: &str =
    "INSERT INTO data_dictionary.domain_streams (domain_id, stream_id, alias, role) \
     VALUES ($1, $2, $3, $4)";

/// DELETE objectives for a specific domain.
/// $1=domain_id
pub const DELETE_OBJECTIVES: &str =
    "DELETE FROM data_dictionary.objectives WHERE domain_id = $1";

/// INSERT a single objective.
/// $1=objective_id, $2=domain_id, $3=description, $4=target_stream,
/// $5=target_metric, $6=condition, $7=threshold, $8=threshold_upper,
/// $9=unit, $10=priority
pub const INSERT_OBJECTIVE: &str =
    "INSERT INTO data_dictionary.objectives \
     (objective_id, domain_id, description, target_stream, target_metric, \
      condition, threshold, threshold_upper, unit, priority) \
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)";

/// DELETE constraints for a specific domain.
/// $1=domain_id
pub const DELETE_CONSTRAINTS: &str =
    "DELETE FROM data_dictionary.constraints WHERE domain_id = $1";

/// INSERT a single constraint.
/// $1=constraint_id, $2=domain_id, $3=description, $4=constraint_stream,
/// $5=constraint_metric, $6=condition, $7=threshold, $8=unit
pub const INSERT_CONSTRAINT: &str =
    "INSERT INTO data_dictionary.constraints \
     (constraint_id, domain_id, description, constraint_stream, \
      constraint_metric, condition, threshold, unit) \
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)";
```

---

## 5. Unit Test Specifications (Tests 1-18)

All unit tests live in `crates/ndp-lib/src/domain/mod.rs` inside `#[cfg(test)] mod tests`.

### UT-001: test_sync_empty_domains

```
Behavior: Syncing an empty slice produces only transaction wrapping.
Setup:    Call sync_domains(&[], &db, &opts()).
Assert:   - report.items_processed == 0
          - report.items_created == 0
          - report.items_updated == 0
          - db.query_strings() contains "BEGIN"
          - db.query_strings() contains "COMMIT"
          - No INSERT or DELETE statements besides transaction control
Why:      Baseline: empty input should be harmless.
```

### UT-002: test_sync_single_domain_upsert

```
Behavior: One domain produces an UPSERT into data_dictionary.domains.
Setup:    let domain = make_minimal_domain("test-domain");
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - db.calls_starting_with("INSERT INTO data_dictionary.domains").len() == 1
          - The query contains "ON CONFLICT (domain_id) DO UPDATE"
          - The query contains "VALUES ($1, $2, $3, $4)"
Why:      Verifies the UPSERT pattern for the parent table.
          Must use ON CONFLICT for idempotent deploys.
```

### UT-003: test_sync_domain_streams_delete_insert

```
Behavior: Domain streams are synced via DELETE first, then INSERT per stream.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.streams = vec![
              make_stream_mapping("air-quality", "indoor", "primary"),
              make_stream_mapping("outdoor-weather", "outdoor", "context"),
          ];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - One DELETE FROM data_dictionary.domain_streams WHERE domain_id = $1
          - Two INSERT INTO data_dictionary.domain_streams statements
          - DELETE position < first INSERT position (ordering)
Why:      DELETE+INSERT per domain ensures stale stream mappings are removed.
          Ordering prevents FK violations.
```

### UT-004: test_sync_domain_streams_values

```
Behavior: INSERT domain_streams has correct $1-$4 params.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.streams = vec![
              make_stream_mapping("air-quality", "indoor", "primary"),
          ];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - INSERT query contains "VALUES ($1, $2, $3, $4)"
          - params.len() == 4
          - Params correspond to: domain_id, stream_id, alias, role
Why:      Validates parameter binding correctness.
          SQL injection prevention: no string concatenation.
```

### UT-005: test_sync_objectives_delete_insert

```
Behavior: Objectives are synced via DELETE first, then INSERT per objective.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.objectives = vec![
              make_objective("healthy_co2", "air-quality", "co2"),
              make_objective("healthy_pm25", "air-quality", "pm25"),
          ];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - One DELETE FROM data_dictionary.objectives WHERE domain_id = $1
          - Two INSERT INTO data_dictionary.objectives statements
          - DELETE position < first INSERT position
Why:      Same DELETE+INSERT pattern as streams.
          Ensures stale objectives are cleaned up.
```

### UT-006: test_sync_objective_values

```
Behavior: INSERT objective has correct 10 params including threshold as NUMERIC.
Setup:    let mut domain = make_minimal_domain("test-domain");
          let mut obj = make_objective("safe_voltage", "smart-meter", "voltage");
          obj.threshold = 240.0;
          obj.unit = Some("volts".to_string());
          obj.priority = "high".to_string();
          domain.objectives = vec![obj];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - INSERT query contains "$1, $2, $3, $4, $5, $6, $7, $8, $9, $10"
          - params.len() == 10
          - Parameter ordering matches:
            $1=objective_id, $2=domain_id, $3=description,
            $4=target_stream, $5=target_metric, $6=condition,
            $7=threshold (240.0), $8=threshold_upper (None/NULL),
            $9=unit ("volts"), $10=priority ("high")
Why:      Validates all 10 columns are parameterized correctly.
          threshold must be passed as f64 for NUMERIC column.
```

### UT-007: test_sync_objective_between_condition

```
Behavior: 'between' condition stores both threshold and threshold_upper.
Setup:    let mut domain = make_minimal_domain("test-domain");
          let mut obj = make_objective("comfort_range", "air-quality", "humidity_pct");
          obj.condition = "between".to_string();
          obj.threshold = 40.0;
          obj.threshold_upper = Some(60.0);
          domain.objectives = vec![obj];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - params[6] represents threshold (40.0) -- non-NULL
          - params[7] represents threshold_upper (60.0) -- non-NULL
          - Both values are present in the params vec
Why:      The 'between' condition is the only case where threshold_upper
          is non-NULL. The SQL schema allows NULL for single conditions.
          This test ensures the second bound is actually passed.
```

### UT-008: test_sync_objective_single_condition

```
Behavior: Non-between conditions pass NULL for threshold_upper.
Setup:    let mut domain = make_minimal_domain("test-domain");
          let obj = make_objective("healthy_co2", "air-quality", "co2");
          // condition defaults to "<", threshold_upper defaults to None
          domain.objectives = vec![obj];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - params.len() == 10
          - params[7] represents threshold_upper as None/NULL
            (debug format contains "None" or similar NULL indicator)
Why:      Complement to UT-007. Verifies the common case where only
          a single threshold is needed. threshold_upper must be NULL,
          not 0.0 or any default value.
```

### UT-009: test_sync_constraints_delete_insert

```
Behavior: Constraints are synced via DELETE first, then INSERT per constraint.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.constraints = vec![
              make_constraint("max_outdoor_aqi", "outdoor-air-quality", "aqi_pm25"),
              make_constraint("min_outdoor_temp", "outdoor-weather", "temperature_c"),
          ];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - One DELETE FROM data_dictionary.constraints WHERE domain_id = $1
          - Two INSERT INTO data_dictionary.constraints statements
          - DELETE position < first INSERT position
Why:      Validates the constraint sync follows the same DELETE+INSERT pattern.
```

### UT-010: test_sync_constraint_values

```
Behavior: INSERT constraint has correct 8 params.
Setup:    let mut domain = make_minimal_domain("test-domain");
          let mut con = make_constraint("max_aqi", "outdoor-aqi", "aqi_pm25");
          con.threshold = 150.0;
          con.unit = Some("AQI".to_string());
          domain.constraints = vec![con];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - INSERT query contains "$1, $2, $3, $4, $5, $6, $7, $8"
          - params.len() == 8
          - Parameter ordering matches:
            $1=constraint_id, $2=domain_id, $3=description,
            $4=constraint_stream, $5=constraint_metric, $6=condition,
            $7=threshold (150.0), $8=unit ("AQI")
Why:      Validates all 8 columns are parameterized.
          Mirrors UT-006 pattern for constraints.
```

### UT-011: test_sync_no_constraints

```
Behavior: Domain with no constraints produces no constraint SQL.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.objectives = vec![make_objective("obj1", "stream", "metric")];
          domain.constraints = vec![]; // explicitly empty
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - No DELETE FROM data_dictionary.constraints calls
          - No INSERT INTO data_dictionary.constraints calls
          - Objectives are still synced normally
Why:      The current indoor-air-quality domain has no constraints.
          The sync must handle this gracefully without unnecessary
          DELETE statements on an empty table.
          Alternatively, if the implementation always issues DELETE,
          verify only the DELETE exists with no INSERTs.
```

### UT-012: test_sync_transaction_wrapping

```
Behavior: First query is BEGIN, last query is COMMIT.
Setup:    let domain = make_minimal_domain("test-domain");
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - db.query_strings().first() == Some(&"BEGIN".to_string())
          - db.query_strings().last() == Some(&"COMMIT".to_string())
Why:      All mutations must be wrapped in a transaction.
          Matches dictionary sync behavior exactly.
```

### UT-013: test_sync_fk_ordering

```
Behavior: UPSERT domain before DELETE+INSERT children.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.streams = vec![make_stream_mapping("s1", "a1", "primary")];
          domain.objectives = vec![make_objective("o1", "s1", "m1")];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - Position of UPSERT domain < position of DELETE domain_streams
          - Position of UPSERT domain < position of DELETE objectives
          - Position of UPSERT domain < position of DELETE constraints
            (if constraints DELETE is issued)
Why:      domain_streams, objectives, and constraints all have FK
          references to domains(domain_id). The parent row must
          exist before children can reference it.
          ON DELETE CASCADE means deleting the parent would cascade,
          but we UPSERT (not DELETE) the parent, so children's DELETE
          is safe at any point after the parent exists.
```

### UT-014: test_sync_multi_domain

```
Behavior: Two domains each get independent UPSERT + children.
Setup:    let mut d1 = make_minimal_domain("domain-a");
          d1.streams = vec![make_stream_mapping("s1", "a1", "primary")];
          d1.objectives = vec![make_objective("o1", "s1", "m1")];

          let mut d2 = make_minimal_domain("domain-b");
          d2.streams = vec![make_stream_mapping("s2", "a2", "primary")];
          d2.objectives = vec![make_objective("o2", "s2", "m2")];

          Call sync_domains(&[d1, d2], &db, &opts()).
Assert:   - Two UPSERT INTO data_dictionary.domains calls
          - Two DELETE FROM data_dictionary.domain_streams calls
          - Two INSERT INTO data_dictionary.domain_streams calls
          - Two DELETE FROM data_dictionary.objectives calls
          - Two INSERT INTO data_dictionary.objectives calls
          - report.items_processed == 2
Why:      Verifies the sync loops correctly over multiple domains
          without cross-contamination.
```

### UT-015: test_sync_report_counts

```
Behavior: Report reflects correct items_processed, items_created, items_updated.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.streams = vec![
              make_stream_mapping("s1", "indoor", "primary"),
              make_stream_mapping("s2", "outdoor", "context"),
          ];
          domain.objectives = vec![
              make_objective("o1", "s1", "co2"),
              make_objective("o2", "s1", "pm25"),
              make_objective("o3", "s1", "temp"),
          ];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - report.entity == "domain"
          - report.items_processed == 1 (one domain)
          - report.items_created counts: 2 streams + 3 objectives = 5
            (or whatever the implementation counts; document the formula)
          - report.items_updated == 1 (the domain UPSERT)
          - report.errors.is_empty()
Why:      The SyncReport must accurately reflect what happened.
          The CLI prints these counts to the user.
```

### UT-016: test_dry_run_no_sql

```
Behavior: Dry run executes no SQL but returns accurate report counts.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.streams = vec![make_stream_mapping("s1", "a", "primary")];
          domain.objectives = vec![make_objective("o1", "s1", "m1")];

          let dry_opts = SyncOptions { dry_run: true };
          Call sync_domains(&[domain], &db, &dry_opts).
Assert:   - db.calls().is_empty() -- no SQL executed
          - report.items_processed == 1
          - report.items_created > 0
          - report.duration == Duration::ZERO (or near-zero)
Why:      Dry run is used by `ndp domain sync --dry-run` to preview
          what would happen. Must produce counts without side effects.
          Matches dictionary sync dry_run behavior.
```

### UT-017: test_sync_domain_error_collected

```
Behavior: DB error on one domain is collected in report.errors;
          other domains still processed.
Setup:    Use FailingMockDbClient configured to fail on "domain-b".
          let d1 = make_minimal_domain("domain-a");
          let d2 = make_minimal_domain("domain-b"); // will fail
          let d3 = make_minimal_domain("domain-c");
          Call sync_domains(&[d1, d2, d3], &db, &opts()).
Assert:   - report.errors.len() >= 1
          - report.errors[0].item contains "domain-b"
          - report.items_processed == 3 (all attempted)
          - domain-a and domain-c SQL still executed
Why:      One bad domain must not abort the entire sync.
          Matches dictionary sync error collection pattern.
```

### UT-018: test_sync_all_sql_parameterized

```
Behavior: All INSERT/UPDATE queries use $N params, no string concatenation.
Setup:    let mut domain = make_minimal_domain("test-domain");
          domain.streams = vec![make_stream_mapping("air-quality", "indoor", "primary")];
          domain.objectives = vec![make_objective("healthy_co2", "air-quality", "co2")];
          domain.constraints = vec![make_constraint("max_aqi", "outdoor-aqi", "aqi_pm25")];
          Call sync_domains(&[domain], &db, &opts()).
Assert:   - For every SqlCall where query starts with "INSERT" or "UPDATE":
            assert!(call.query.contains("$1"))
          - No INSERT/UPDATE query contains the literal string values
            "test-domain", "air-quality", "healthy_co2", "max_aqi"
            directly in the SQL text (they must only appear in params)
Why:      SQL injection prevention. All values must flow through
          parameterized queries, never string interpolation.
          This is a structural safety test.
```

---

## 6. ConfigLoader Test Specifications (Tests 19-23)

These tests validate that `FileSystemConfigLoader` can discover and parse domain configs from `config/domains/*/domain.json`. They live in `crates/ndp-lib/src/config.rs` tests or a dedicated test file.

### CL-001 (Test 19): test_discover_domain_ids

```
Behavior: Lists domain subdirectories that contain domain.json.
Setup:    Create a temp directory with:
          - domains/indoor-air-quality/domain.json  (valid)
          - domains/greenhouse-control/domain.json  (valid)
          - domains/empty-dir/                      (no domain.json -- skip)
Assert:   - Result is Ok with exactly 2 domain IDs
          - IDs are sorted: ["greenhouse-control", "indoor-air-quality"]
          - "empty-dir" is not included
Why:      Discovery must filter to directories that actually contain
          the expected config file, matching the stream discovery pattern.
```

### CL-002 (Test 20): test_load_domain_config

```
Behavior: Parses the real indoor-air-quality/domain.json.
Setup:    Use include_str!("../../../config/domains/indoor-air-quality/domain.json")
          to parse DomainConfig at compile time.
Assert:   - config.id == "indoor-air-quality"
          - config.description == "Maintain healthy indoor air quality"
          - config.streams.len() == 4
          - config.objectives.len() == 6
          - config.objectives[0].id == "healthy_co2"
          - config.objectives[0].target.threshold == 800.0
          - config.objectives[0].target.unit == Some("ppm")
          - config.objectives[0].priority == "high"
Why:      Proves the serde struct can parse the actual production config.
          Any schema drift between domain.json and DomainConfig is caught
          at compile time via include_str! + serde_json::from_str.
```

### CL-003 (Test 21): test_load_domain_configs_skips_invalid

```
Behavior: Invalid JSON domain config is skipped with warning, not fatal.
Setup:    Create a temp directory with:
          - domains/valid-domain/domain.json    (valid JSON)
          - domains/bad-domain/domain.json      (invalid JSON: "not json{")
Assert:   - Result is Ok with 1 domain config
          - The valid domain is returned
          - The bad domain is silently skipped (logged as warning)
Why:      One malformed config must not prevent syncing all other domains.
          Matches the stream loader's error handling behavior.
```

### CL-004 (Test 22): test_domain_config_no_objectives

```
Behavior: Domain with empty objectives[] parses OK.
Setup:    Parse JSON: { "id": "test", "description": "No objectives",
          "streams": [], "objectives": [] }
Assert:   - config.objectives.is_empty()
          - No parse error
Why:      Edge case: a domain might be defined with only stream mappings
          and no objectives yet. The parser must not require non-empty arrays.
```

### CL-005 (Test 23): test_domain_config_no_constraints

```
Behavior: Domain without constraints key parses OK (serde default).
Setup:    Parse the real indoor-air-quality/domain.json which has no
          "constraints" key.
Assert:   - config.constraints is empty (Vec default) or None
          - No parse error
Why:      The current domain.json has no constraints field.
          The struct must use #[serde(default)] so missing fields
          default to empty. This is a real-world case, not hypothetical.
```

---

## 7. Conversion Test Specifications (Tests 24-26)

These tests validate the conversion from `DomainConfig` (serde-parsed JSON) to `DomainSyncEntry` (sync-ready struct). They live in `crates/ndp-lib/src/domain/mod.rs` or a `convert.rs` submodule.

### CV-001 (Test 24): test_domain_config_to_sync_entry

```
Behavior: Full conversion from DomainConfig to DomainSyncEntry.
Setup:    Parse the real indoor-air-quality domain.json into DomainConfig.
          Convert to DomainSyncEntry.
Assert:   - entry.domain_id == "indoor-air-quality"
          - entry.description == "Maintain healthy indoor air quality"
          - entry.streams.len() == 4
          - entry.streams[0].stream_id == "air-quality"
          - entry.streams[0].alias == "indoor"
          - entry.streams[0].role == "primary"
          - entry.objectives.len() == 6
          - entry.objectives[0].objective_id == "healthy_co2"
          - entry.objectives[0].target_stream == "air-quality"
          - entry.objectives[0].target_metric == "co2"
          - entry.objectives[0].condition == "<"
          - entry.objectives[0].priority == "high"
          - entry.constraints.is_empty()
Why:      Validates the mapping layer between config format and sync format.
          If domain.json schema changes, this test catches the drift.
```

### CV-002 (Test 25): test_objective_threshold_numeric

```
Behavior: Threshold converts from JSON number to f64 correctly.
Setup:    Parse objective with threshold: 800 (integer in JSON).
          Convert to ObjectiveSyncEntry.
Assert:   - entry.threshold == 800.0 (f64)
          - The f64 value is exact (no floating point drift for this value)
Why:      JSON numbers can be integers or floats. The threshold column
          in PostgreSQL is NUMERIC. The conversion must handle both
          integer and float JSON values, producing f64 for the parameter.
```

### CV-003 (Test 26): test_objective_between_threshold

```
Behavior: Between condition extracts both threshold and threshold_upper.
Setup:    Create a JSON objective with:
          "condition": "between", "threshold": [40, 60]
          or "threshold": 40, "threshold_upper": 60
          (depending on chosen JSON schema)
          Convert to ObjectiveSyncEntry.
Assert:   - entry.condition == "between"
          - entry.threshold == 40.0
          - entry.threshold_upper == Some(60.0)
Why:      The 'between' condition is special because it requires two
          bounds. The conversion must extract both values correctly.
          This test documents the expected JSON format for between.
```

---

## 8. Integration Test Specifications (Tests 27-29)

Integration tests require `docker-compose.integration.yml` running TimescaleDB. They are marked `#[ignore]` and run with `cargo test -- --ignored`.

These live in `tests/integration/` or inline with `#[ignore]`.

### IT-001 (Test 27): test_integration_domain_sync

```
Behavior: Full sync against real TimescaleDB succeeds.
Setup:    - Connect to postgresql://postgres:postgres@localhost:5432/ndp
          - Ensure init-scripts/005_domain_objectives.sql has been run
          - Parse real indoor-air-quality domain.json
          - Convert to DomainSyncEntry
Assert:   - sync_domains() returns Ok
          - report.items_processed == 1
          - report.errors.is_empty()
          - SELECT count(*) FROM data_dictionary.domains == 1
          - SELECT count(*) FROM data_dictionary.domain_streams == 4
          - SELECT count(*) FROM data_dictionary.objectives == 6
          - SELECT count(*) FROM data_dictionary.constraints == 0
Teardown: DELETE FROM data_dictionary.domains WHERE domain_id = 'indoor-air-quality'
          (CASCADE will clean children)
Mark:     #[tokio::test] #[ignore]
Why:      End-to-end proof that generated SQL is valid and data lands correctly.
```

### IT-002 (Test 28): test_integration_domain_sync_idempotent

```
Behavior: Running sync twice produces the same result (UPSERT behavior).
Setup:    - Parse and sync indoor-air-quality domain
          - Sync again with same data
Assert:   - Both syncs return Ok
          - After second sync, row counts are unchanged:
            domains=1, domain_streams=4, objectives=6, constraints=0
          - No duplicate rows (PK constraints enforced)
Teardown: DELETE FROM data_dictionary.domains WHERE domain_id = 'indoor-air-quality'
Mark:     #[tokio::test] #[ignore]
Why:      deploy.sh runs on every deployment. Sync must be safe to
          execute repeatedly without data duplication.
```

### IT-003 (Test 29): test_integration_objectives_queryable

```
Behavior: After sync, objectives are queryable with correct values.
Setup:    - Sync indoor-air-quality domain
          - SELECT * FROM data_dictionary.objectives
            WHERE domain_id = 'indoor-air-quality'
            ORDER BY objective_id
Assert:   - 6 rows returned
          - Row for healthy_co2: target_stream='air-quality',
            target_metric='co2', condition='<', threshold=800,
            threshold_upper IS NULL, unit='ppm', priority='high'
          - Row for healthy_pm25: threshold=12, unit='ug/m3'
          - All audit columns (created_at, updated_at) are non-NULL
Teardown: DELETE FROM data_dictionary.domains WHERE domain_id = 'indoor-air-quality'
Mark:     #[tokio::test] #[ignore]
Why:      Validates that parameterized values land in the correct columns.
          Catches type mismatches (e.g., threshold passed as TEXT instead of NUMERIC).
```

---

## 9. CLI Test Specification (Test 30)

### CLI-001 (Test 30): test_cli_domain_sync_dry_run

```
Behavior: `ndp domain sync --dry-run` produces output without DB connection.
Setup:    Run the CLI binary with:
          ndp domain sync --dry-run \
            --config-dir /path/to/config/domains
          (or use Command::cargo_bin("ndp") for test harness)
Assert:   - Exit code 0
          - stdout contains "domain" (entity name)
          - stdout contains items_processed count
          - stdout contains "dry_run" or "DRY RUN" indicator
          - No database connection attempted (no connection error)
Why:      Users need to preview what sync would do before running it.
          The --dry-run flag must work without a running database.
```

---

## 10. Test File Organization

```
crates/ndp-lib/
  src/
    domain/
      mod.rs          # (NEW) sync_domains() + #[cfg(test)] mod tests (UT-001 through UT-018)
      types.rs        # (NEW) DomainSyncEntry, ObjectiveSyncEntry, etc.
      sql.rs          # (NEW) SQL constants (UPSERT_DOMAIN, INSERT_OBJECTIVE, etc.)
    config.rs         # (EXTEND) add load_domain_configs() + tests CL-001 through CL-005
    lib.rs            # (EXTEND) add `pub mod domain;`
  tests/
    integration/
      domain_sync.rs  # (NEW) IT-001 through IT-003

tools/ndp-cli/
  src/
    commands/
      domain.rs       # (NEW) `ndp domain sync` command
      mod.rs          # (EXTEND) add domain subcommand
  tests/
    cli_domain.rs     # (NEW) CLI-001
```

---

## 11. Implementation Order

The tests are written first; implementation follows to make them pass.

| Phase | Tests | Implementation | Depends On |
|-------|-------|----------------|------------|
| 1 | Define types (types.rs) | DomainSyncEntry, ObjectiveSyncEntry, StreamMappingEntry, ConstraintSyncEntry | None |
| 2 | Define SQL constants (sql.rs) | UPSERT_DOMAIN, INSERT_DOMAIN_STREAM, INSERT_OBJECTIVE, INSERT_CONSTRAINT, DELETEs | Phase 1 |
| 3 | Write UT-001 through UT-018 | Tests compile but fail (no sync_domains function yet) | Phase 1, 2 |
| 4 | Implement sync_domains() | Make UT-001 through UT-018 pass one by one | Phase 3 |
| 5 | Write CL-001 through CL-005 | Tests for config loading | Phase 1 |
| 6 | Extend ConfigLoader | Add load_domain_configs() to trait and FileSystemConfigLoader | Phase 5 |
| 7 | Write CV-001 through CV-003 | Tests for conversion | Phase 1, 6 |
| 8 | Implement conversion | DomainConfig -> DomainSyncEntry mapping | Phase 7 |
| 9 | Write CLI-001 | CLI test | Phase 4, 6, 8 |
| 10 | Implement CLI command | `ndp domain sync` wiring | Phase 9 |
| 11 | Write IT-001 through IT-003 | Integration tests (run when infra available) | Phase 4 |

---

## 12. Function Signature

The core sync function mirrors the dictionary sync signature:

```rust
/// Sync domain configurations to the data_dictionary tables.
///
/// Caller decides where configs come from (files, etcd, test fixtures).
/// This function takes parsed structs, not file paths.
///
/// # Arguments
/// * `domains` - parsed domain configurations
/// * `db` - database client (real or mock)
/// * `options` - sync options (dry_run, etc.)
///
/// # Returns
/// A `SyncReport` summarizing what was created, updated, or deleted.
pub async fn sync_domains(
    domains: &[DomainSyncEntry],
    db: &impl DbClient,
    options: &SyncOptions,
) -> Result<SyncReport>
```

Per-domain sync order within the transaction:

```
BEGIN
for each domain:
    1. UPSERT data_dictionary.domains          (parent must exist first)
    2. DELETE data_dictionary.domain_streams    (WHERE domain_id = $1)
    3. INSERT data_dictionary.domain_streams    (per stream)
    4. DELETE data_dictionary.objectives        (WHERE domain_id = $1)
    5. INSERT data_dictionary.objectives        (per objective)
    6. DELETE data_dictionary.constraints        (WHERE domain_id = $1) [skip if no constraints]
    7. INSERT data_dictionary.constraints        (per constraint) [skip if no constraints]
COMMIT
```

---

## 13. Completion Criteria

### 13.1 Unit Tests (Must Pass)

- [ ] UT-001: Empty domains produces BEGIN/COMMIT only
- [ ] UT-002: Single domain UPSERT with ON CONFLICT
- [ ] UT-003: Domain streams DELETE before INSERT
- [ ] UT-004: Domain streams correct $1-$4 params
- [ ] UT-005: Objectives DELETE before INSERT
- [ ] UT-006: Objective correct 10 params
- [ ] UT-007: Between condition: both thresholds non-NULL
- [ ] UT-008: Single condition: threshold_upper is NULL
- [ ] UT-009: Constraints DELETE before INSERT
- [ ] UT-010: Constraint correct 8 params
- [ ] UT-011: No constraints: no constraint SQL
- [ ] UT-012: Transaction wrapping (BEGIN/COMMIT)
- [ ] UT-013: FK ordering (UPSERT parent before children)
- [ ] UT-014: Multi-domain independent sync
- [ ] UT-015: Report counts accurate
- [ ] UT-016: Dry run no SQL
- [ ] UT-017: Error collection (one domain fails, others continue)
- [ ] UT-018: All SQL parameterized (no string concat)

### 13.2 ConfigLoader Tests (Must Pass)

- [ ] CL-001: Discover domain IDs from filesystem
- [ ] CL-002: Parse real indoor-air-quality domain.json
- [ ] CL-003: Skip invalid JSON gracefully
- [ ] CL-004: Empty objectives array parses OK
- [ ] CL-005: Missing constraints key uses serde default

### 13.3 Conversion Tests (Must Pass)

- [ ] CV-001: Full DomainConfig to DomainSyncEntry conversion
- [ ] CV-002: Threshold numeric conversion
- [ ] CV-003: Between condition threshold extraction

### 13.4 Integration Tests (Must Pass When Infrastructure Available)

- [ ] IT-001: Full sync against real TimescaleDB
- [ ] IT-002: Idempotent sync (run twice)
- [ ] IT-003: Objectives queryable with correct values

### 13.5 CLI Tests (Must Pass)

- [ ] CLI-001: `ndp domain sync --dry-run` works without DB

### 13.6 Existing Tests (Must Not Regress)

- [ ] All existing ndp-lib tests pass
- [ ] All 339 ndp-gold-ddl tests pass
- [ ] All 217 ndp-validate tests pass

---

## 14. Risk Assessment

| Risk | Mitigation |
|------|------------|
| Existing tests break from new module | New module is additive; no changes to existing code until ConfigLoader extension |
| DomainConfig serde struct doesn't match domain.json | CL-002 parses real config at compile time; any mismatch is a compile error |
| threshold as f64 loses precision for NUMERIC | All current thresholds are small integers/decimals; f64 is sufficient. Document limitation for future |
| constraints table unused (no constraints in config) | UT-011 explicitly tests empty constraints. Code handles both cases |
| Between condition JSON format ambiguous | CV-003 documents and tests the chosen format. Only one format supported |
| FailingMockDbClient adds test complexity | Keep it minimal; only used in UT-017. Delegate all other calls to inner MockDbClient |
| Integration tests flaky due to Docker timing | Use health checks, retry logic, and `#[ignore]` attribute. CI runs separately |
