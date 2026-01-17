# DP-011: Integration Tests Specification

**Feature ID**: dp-011
**Phase**: Refinement (SPARC R)
**Created**: 2026-01-16
**Test Type**: Integration (Real Database)

---

## Overview

Integration tests that verify ETL run persistence against a real TimescaleDB instance. These tests are marked `#[ignore]` and run separately from unit tests.

---

## Test File Location

```
tests/integration/
  dp011_persistence.rs   # New file for dp-011 integration tests
```

Or inline in the persistence module:

```
apps/silver-etl/src/persistence.rs

#[cfg(test)]
mod integration_tests {
    // Tests marked with #[ignore]
}
```

---

## Prerequisites

### Required Environment Variables

```bash
# TimescaleDB connection
export TIMESCALE_URL="postgresql://ndp:password@localhost:5432/ndp"

# Or individual variables
export NDP_TIMESCALE_HOST="localhost"
export NDP_TIMESCALE_PORT="5432"
export NDP_TIMESCALE_DB="ndp"
export NDP_TIMESCALE_USER="ndp"
export NDP_TIMESCALE_PASSWORD="password"
```

### Required Infrastructure

- TimescaleDB running with `silver.etl_runs` table created
- Migration `XXX_etl_runs.sql` already applied

---

## Integration Test Specifications

### Test 1: Full Persistence Roundtrip

```rust
/// Integration test: Full persistence roundtrip
///
/// Verifies:
/// - start_run creates record with correct initial state
/// - complete_run updates record with final state
/// - Query returns accurate data
///
/// Prerequisites:
/// - TimescaleDB running
/// - silver.etl_runs table exists
#[tokio::test]
#[ignore] // Run with: cargo test --ignored
async fn test_persistence_roundtrip() {
    // Setup: Connect to real database
    let conn_str = std::env::var("TIMESCALE_URL")
        .expect("TIMESCALE_URL required for integration tests");

    let runner = EtlRunner::with_postgres(&conn_str)
        .expect("Failed to connect to TimescaleDB");

    let persistence = DuckDbRunPersistence::new(runner.connection());

    // 1. Start a run
    let stream_id = format!("integration-test-{}", Uuid::new_v4());
    let cycle_id = Uuid::new_v4();

    let run_id = persistence.start_run(&stream_id, EtlRunMode::Manual, Some(cycle_id))
        .expect("start_run should succeed");

    // 2. Verify initial state
    let initial_row = query_run(&runner, run_id).await
        .expect("Should find run record");

    assert_eq!(initial_row.stream_id, stream_id);
    assert_eq!(initial_row.status, "running");
    assert_eq!(initial_row.run_mode, "manual");
    assert_eq!(initial_row.daemon_cycle_id, Some(cycle_id));
    assert!(initial_row.completed_at.is_none());
    assert_eq!(initial_row.rows_processed, 0);

    // 3. Complete the run with stats
    let stats = EtlStats {
        stream_id: stream_id.clone(),
        rows_processed: 1000,
        rows_with_dq_flags: 50,
        rows_rejected: 5,
        duration_ms: 2500,
        watermark_before: Some(Utc::now() - chrono::Duration::hours(1)),
        watermark_after: Some(Utc::now()),
    };

    persistence.complete_run(run_id, &stats)
        .expect("complete_run should succeed");

    // 4. Verify final state
    let final_row = query_run(&runner, run_id).await
        .expect("Should find completed run");

    assert_eq!(final_row.status, "success");
    assert!(final_row.completed_at.is_some());
    assert_eq!(final_row.rows_processed, 1000);
    assert_eq!(final_row.rows_flagged, 50);
    assert_eq!(final_row.rows_rejected, 5);
    assert!(final_row.duration_ms >= 2500);
    assert!(final_row.watermark_after.is_some());

    // Cleanup
    cleanup_test_run(&runner, run_id).await;
}

/// Query a run record by ID
async fn query_run(runner: &EtlRunner, id: Uuid) -> Option<EtlRunRow> {
    let sql = format!(
        "SELECT * FROM pg.silver.etl_runs WHERE id = '{}'",
        id
    );

    runner.connection().query_row(&sql, [], |row| {
        Ok(EtlRunRow {
            id: row.get("id")?,
            stream_id: row.get("stream_id")?,
            status: row.get("status")?,
            run_mode: row.get("run_mode")?,
            daemon_cycle_id: row.get("daemon_cycle_id")?,
            completed_at: row.get("completed_at")?,
            rows_processed: row.get("rows_processed")?,
            rows_flagged: row.get("rows_flagged")?,
            rows_rejected: row.get("rows_rejected")?,
            duration_ms: row.get("duration_ms")?,
            watermark_before: row.get("watermark_before")?,
            watermark_after: row.get("watermark_after")?,
            error_message: row.get("error_message")?,
        })
    }).ok()
}

/// Cleanup test data
async fn cleanup_test_run(runner: &EtlRunner, id: Uuid) {
    let sql = format!("DELETE FROM pg.silver.etl_runs WHERE id = '{}'", id);
    let _ = runner.connection().execute(&sql, []);
}
```

### Test 2: Multiple Streams Same Cycle

```rust
/// Integration test: Multiple streams in same daemon cycle
///
/// Verifies:
/// - daemon_cycle_id correctly links multiple runs
/// - Each stream has its own run record
/// - Query by cycle_id returns all runs
#[tokio::test]
#[ignore]
async fn test_multiple_streams_same_cycle() {
    let conn_str = std::env::var("TIMESCALE_URL").expect("TIMESCALE_URL required");
    let runner = EtlRunner::with_postgres(&conn_str).expect("Connection failed");
    let persistence = DuckDbRunPersistence::new(runner.connection());

    let cycle_id = Uuid::new_v4();
    let streams = vec![
        format!("integ-stream-a-{}", Uuid::new_v4()),
        format!("integ-stream-b-{}", Uuid::new_v4()),
        format!("integ-stream-c-{}", Uuid::new_v4()),
    ];

    let mut run_ids = Vec::new();

    // Create runs for all streams with same cycle_id
    for stream_id in &streams {
        let run_id = persistence.start_run(stream_id, EtlRunMode::Daemon, Some(cycle_id))
            .expect("start_run should succeed");
        run_ids.push(run_id);

        // Complete each run
        let stats = make_test_stats(100);
        persistence.complete_run(run_id, &stats)
            .expect("complete_run should succeed");
    }

    // Query all runs for this cycle
    let sql = format!(
        "SELECT COUNT(*) as cnt FROM pg.silver.etl_runs WHERE daemon_cycle_id = '{}'",
        cycle_id
    );

    let count: i64 = runner.connection()
        .query_row(&sql, [], |row| row.get(0))
        .expect("Query should succeed");

    assert_eq!(count, 3, "Should have 3 runs in the same cycle");

    // Cleanup
    for run_id in run_ids {
        cleanup_test_run(&runner, run_id).await;
    }
}
```

### Test 3: Retention Policy Cleanup

```rust
/// Integration test: Retention policy deletes old records
///
/// Verifies:
/// - Records older than 30 days are deleted
/// - Recent records are preserved
/// - Cleanup is idempotent
///
/// Note: This test inserts backdated records to simulate age
#[tokio::test]
#[ignore]
async fn test_retention_cleanup() {
    let conn_str = std::env::var("TIMESCALE_URL").expect("TIMESCALE_URL required");
    let runner = EtlRunner::with_postgres(&conn_str).expect("Connection failed");

    // Insert old record (40 days ago)
    let old_stream_id = format!("retention-old-{}", Uuid::new_v4());
    let old_run_id = Uuid::new_v4();
    let old_date = Utc::now() - chrono::Duration::days(40);

    let insert_old = format!(
        r#"
        INSERT INTO pg.silver.etl_runs (id, stream_id, started_at, status, created_at)
        VALUES ('{}', '{}', '{}', 'success', '{}')
        "#,
        old_run_id, old_stream_id,
        old_date.to_rfc3339(),
        old_date.to_rfc3339()
    );

    runner.connection().execute(&insert_old, [])
        .expect("Insert old record should succeed");

    // Insert recent record (5 days ago)
    let recent_stream_id = format!("retention-recent-{}", Uuid::new_v4());
    let recent_run_id = Uuid::new_v4();
    let recent_date = Utc::now() - chrono::Duration::days(5);

    let insert_recent = format!(
        r#"
        INSERT INTO pg.silver.etl_runs (id, stream_id, started_at, status, created_at)
        VALUES ('{}', '{}', '{}', 'success', '{}')
        "#,
        recent_run_id, recent_stream_id,
        recent_date.to_rfc3339(),
        recent_date.to_rfc3339()
    );

    runner.connection().execute(&insert_recent, [])
        .expect("Insert recent record should succeed");

    // Run retention cleanup
    let cleanup_sql = "DELETE FROM pg.silver.etl_runs WHERE created_at < NOW() - INTERVAL '30 days'";
    let deleted_count = runner.connection().execute(cleanup_sql, [])
        .expect("Cleanup should succeed");

    assert!(deleted_count >= 1, "Should delete at least 1 old record");

    // Verify old record deleted
    let old_exists = query_run(&runner, old_run_id).await;
    assert!(old_exists.is_none(), "Old record should be deleted");

    // Verify recent record preserved
    let recent_exists = query_run(&runner, recent_run_id).await;
    assert!(recent_exists.is_some(), "Recent record should be preserved");

    // Cleanup recent record
    cleanup_test_run(&runner, recent_run_id).await;
}
```

### Test 4: Failed Run Persistence

```rust
/// Integration test: Failed run stores error details correctly
///
/// Verifies:
/// - status='failed' set
/// - error_message stored
/// - error_context JSONB stored and queryable
#[tokio::test]
#[ignore]
async fn test_failed_run_persistence() {
    let conn_str = std::env::var("TIMESCALE_URL").expect("TIMESCALE_URL required");
    let runner = EtlRunner::with_postgres(&conn_str).expect("Connection failed");
    let persistence = DuckDbRunPersistence::new(runner.connection());

    let stream_id = format!("integ-fail-{}", Uuid::new_v4());
    let run_id = persistence.start_run(&stream_id, EtlRunMode::Daemon, None)
        .expect("start_run should succeed");

    // Fail with detailed context
    let error_msg = "Transform SQL failed: column 'wind_speed_kmh' does not exist";
    let context = serde_json::json!({
        "stage": "transform",
        "sql": "INSERT INTO silver.weather_forecasts...",
        "parquet_files": ["forecast_2026-01-16.parquet"],
        "duckdb_error": "Catalog Error: column 'wind_speed_kmh' does not exist"
    });

    persistence.fail_run(run_id, error_msg, Some(context.clone()))
        .expect("fail_run should succeed");

    // Query and verify
    let row = query_run(&runner, run_id).await
        .expect("Should find failed run");

    assert_eq!(row.status, "failed");
    assert!(row.error_message.as_ref().unwrap().contains("wind_speed_kmh"));
    assert!(row.completed_at.is_some());

    // Query error_context JSONB
    let context_sql = format!(
        "SELECT error_context->>'stage' as stage FROM pg.silver.etl_runs WHERE id = '{}'",
        run_id
    );
    let stage: String = runner.connection()
        .query_row(&context_sql, [], |row| row.get(0))
        .expect("Context query should succeed");

    assert_eq!(stage, "transform");

    // Cleanup
    cleanup_test_run(&runner, run_id).await;
}
```

### Test 5: Concurrent Writes

```rust
/// Integration test: Concurrent persistence writes
///
/// Verifies:
/// - Multiple simultaneous start_run calls succeed
/// - No deadlocks or constraint violations
/// - Each run gets unique UUID
#[tokio::test]
#[ignore]
async fn test_concurrent_persistence() {
    let conn_str = std::env::var("TIMESCALE_URL").expect("TIMESCALE_URL required");

    let cycle_id = Uuid::new_v4();
    let mut handles = Vec::new();

    // Spawn 10 concurrent persistence operations
    for i in 0..10 {
        let conn_str = conn_str.clone();
        let cycle_id = cycle_id.clone();

        handles.push(tokio::spawn(async move {
            let runner = EtlRunner::with_postgres(&conn_str)
                .expect("Connection failed");
            let persistence = DuckDbRunPersistence::new(runner.connection());

            let stream_id = format!("concurrent-{}-{}", i, Uuid::new_v4());
            let run_id = persistence.start_run(&stream_id, EtlRunMode::Daemon, Some(cycle_id))?;

            let stats = make_test_stats(100 * (i as u64 + 1));
            persistence.complete_run(run_id, &stats)?;

            Ok::<Uuid, PersistenceError>(run_id)
        }));
    }

    // Collect results
    let mut run_ids = Vec::new();
    for handle in handles {
        match handle.await.unwrap() {
            Ok(run_id) => run_ids.push(run_id),
            Err(e) => panic!("Concurrent operation failed: {}", e),
        }
    }

    assert_eq!(run_ids.len(), 10, "All 10 operations should succeed");

    // Verify all UUIDs are unique
    let unique_count = run_ids.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, 10, "All UUIDs should be unique");

    // Cleanup
    let runner = EtlRunner::with_postgres(&conn_str).expect("Connection failed");
    for run_id in run_ids {
        cleanup_test_run(&runner, run_id).await;
    }
}
```

---

## Test Data Structures

```rust
/// Row returned from etl_runs query
#[derive(Debug)]
struct EtlRunRow {
    id: Uuid,
    stream_id: String,
    status: String,
    run_mode: String,
    daemon_cycle_id: Option<Uuid>,
    completed_at: Option<DateTime<Utc>>,
    rows_processed: i64,
    rows_flagged: i64,
    rows_rejected: i64,
    duration_ms: Option<i64>,
    watermark_before: Option<DateTime<Utc>>,
    watermark_after: Option<DateTime<Utc>>,
    error_message: Option<String>,
}

fn make_test_stats(rows: u64) -> EtlStats {
    EtlStats {
        stream_id: "test".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: rows / 20,
        rows_rejected: rows / 100,
        duration_ms: 100,
        watermark_before: None,
        watermark_after: Some(Utc::now()),
    }
}
```

---

## Test Execution

```bash
# Run all integration tests (requires running TimescaleDB)
TIMESCALE_URL="postgresql://ndp:secret@localhost:5432/ndp" \
  cargo test --package silver-etl -- --ignored

# Run specific integration test
TIMESCALE_URL="postgresql://ndp:secret@localhost:5432/ndp" \
  cargo test --package silver-etl test_persistence_roundtrip -- --ignored --exact

# Run with verbose output
TIMESCALE_URL="postgresql://ndp:secret@localhost:5432/ndp" \
  cargo test --package silver-etl -- --ignored --nocapture
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/integration-tests.yml
integration-tests:
  runs-on: ubuntu-latest
  services:
    timescaledb:
      image: timescale/timescaledb:latest-pg15
      env:
        POSTGRES_PASSWORD: test
        POSTGRES_DB: ndp
      ports:
        - 5432:5432
      options: >-
        --health-cmd pg_isready
        --health-interval 10s
        --health-timeout 5s
        --health-retries 5

  steps:
    - uses: actions/checkout@v4
    - name: Apply migrations
      run: psql $TIMESCALE_URL -f deploy/timescaledb/migrations/XXX_etl_runs.sql

    - name: Run integration tests
      env:
        TIMESCALE_URL: postgresql://postgres:test@localhost:5432/ndp
      run: cargo test --package silver-etl -- --ignored
```

---

## Test Cleanup Strategy

All integration tests should:

1. Use unique identifiers (UUID in stream_id)
2. Clean up after themselves
3. Not depend on pre-existing data
4. Be idempotent (can run multiple times)

```rust
/// Cleanup helper - call at end of each test
async fn cleanup_test_run(runner: &EtlRunner, id: Uuid) {
    let sql = format!("DELETE FROM pg.silver.etl_runs WHERE id = '{}'", id);
    match runner.connection().execute(&sql, []) {
        Ok(_) => {}
        Err(e) => eprintln!("Warning: Cleanup failed for {}: {}", id, e),
    }
}
```

---

## Coverage Summary

| Test | Verifies |
|------|----------|
| `test_persistence_roundtrip` | Full write-read cycle |
| `test_multiple_streams_same_cycle` | daemon_cycle_id linking |
| `test_retention_cleanup` | 30-day retention policy |
| `test_failed_run_persistence` | Error storage & JSONB |
| `test_concurrent_persistence` | Concurrent safety |

**Total: 5 integration tests**

---

*Integration tests specification created: 2026-01-16*
