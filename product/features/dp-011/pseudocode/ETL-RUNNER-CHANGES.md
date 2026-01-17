# ETL Runner Changes Pseudocode

**Feature**: dp-011 - Silver ETL Run Statistics Persistence
**Component**: Modifications to EtlRunner and CLI
**Author**: ndp-rust-dev
**Created**: 2026-01-16

---

## Overview

This document defines modifications to support persistence in non-daemon contexts (CLI manual runs, backfill operations). The daemon handles its own persistence (see DAEMON-INTEGRATION.md), but CLI commands also need tracking.

---

## Design Decision: Where to Add Persistence

### Option A: Inside EtlRunner (Tight Coupling)

```
# EtlRunner::run_etl gains persistence internally

STRUCT EtlRunner:
    conn: Connection
    pg_conn_str: Option<String>
    persistence: Option<Box<dyn EtlRunPersistence>>  # NEW
    ...

FUNCTION run_etl(...) -> Result<EtlStats, EtlError>:
    # Start run record
    run_id = self.persistence?.start_run(...)

    # Execute ETL
    result = self.execute_etl_internal(...)

    # Record result
    MATCH result:
        Ok(stats) => self.persistence?.complete_run(run_id, &stats)
        Err(e) => self.persistence?.fail_run(run_id, &e)

    result
```

**Pros**: Simple API, automatic tracking
**Cons**:
- Tight coupling
- Can't opt out of persistence
- Testing requires persistence setup

### Option B: Wrapper Function (Loose Coupling) - PREFERRED

```
# Separate function wraps EtlRunner for persistence

FUNCTION run_etl_with_persistence(
    runner: &EtlRunner,
    persistence: &dyn EtlRunPersistence,
    config: &SilverEtlConfig,
    stream_id: &str,
    bronze_path: &str,
    run_mode: EtlRunMode
) -> Result<EtlStats, EtlError>
```

**Pros**:
- EtlRunner stays focused on transformation
- Persistence is opt-in
- Easy to test each part independently
- Matches daemon's approach

---

## Wrapper Function Implementation

### Function Signature

```
FUNCTION run_etl_with_persistence(
    runner: &EtlRunner,
    persistence: &dyn EtlRunPersistence,
    config: &SilverEtlConfig,
    stream_id: &str,
    bronze_path: &str,
    run_mode: EtlRunMode
) -> Result<EtlStats, EtlError>:

    # 1. Start run record
    run_id = persistence.start_run(stream_id, run_mode, None)
        .map_err(|e| {
            warn!(
                stream_id = %stream_id,
                error = %e,
                "Failed to start run record, proceeding without tracking"
            )
            e
        })
        .ok()  # Convert to Option - don't fail ETL if persistence fails

    # 2. Execute ETL
    etl_result = runner.run_etl(config, stream_id, bronze_path)

    # 3. Record result based on outcome
    MATCH &etl_result:

        Ok(stats):
            # Record successful completion
            IF let Some(id) = run_id:
                IF let Err(e) = persistence.complete_run(id, stats):
                    warn!(
                        run_id = %id,
                        error = %e,
                        "Failed to record successful run"
                    )

        Err(etl_error):
            # Record failure with context
            IF let Some(id) = run_id:
                context = build_error_context(etl_error, config, bronze_path)
                IF let Err(e) = persistence.fail_run(id, &etl_error.to_string(), Some(context)):
                    warn!(
                        run_id = %id,
                        error = %e,
                        "Failed to record failed run"
                    )

    # 4. Return original result (pass through)
    etl_result
```

### Error Context Builder

```
FUNCTION build_error_context(
    error: &EtlError,
    config: &SilverEtlConfig,
    bronze_path: &str
) -> serde_json::Value:

    # Determine error stage from error variant
    stage = MATCH error:
        EtlError::ParquetResolution { .. } => "resolve_files"
        EtlError::Watermark { .. } => "get_watermark"
        EtlError::SqlGeneration(_) => "generate_sql"
        EtlError::SqlExecution(_) => "execute_sql"
        EtlError::Config(_) => "configuration"
        EtlError::PostgresExtension(_) => "postgres_extension"
        EtlError::PostgresAttach(_) => "postgres_attach"
        _ => "unknown"

    # Build context JSON
    RETURN json!({
        "stage": stage,
        "target_table": config.target_table,
        "bronze_path": bronze_path,
        "incremental_enabled": config.incremental.enabled,
        "error_variant": format!("{:?}", error)
    })
```

---

## CLI Integration

### Modified Run Command

```
# In apps/silver-etl/src/main.rs or cli.rs

FUNCTION handle_run_command(
    stream_id: &str,
    bronze_path: &str,
    dry_run: bool
) -> Result<(), Error>:

    # 1. Load configuration
    config = load_stream_config(stream_id).await?

    # 2. Create ETL runner
    runner = EtlRunner::from_env()?

    # 3. Handle dry-run (no persistence needed)
    IF dry_run:
        sql = runner.dry_run(&config, stream_id, bronze_path)?
        println!("Generated SQL:\n{}", sql)
        RETURN Ok(())

    # 4. Create persistence (for real runs)
    persistence = create_persistence_from_env()?

    # 5. Execute with persistence tracking
    stats = run_etl_with_persistence(
        &runner,
        &persistence,
        &config,
        stream_id,
        bronze_path,
        EtlRunMode::Manual
    )?

    # 6. Print results
    println!("ETL completed:")
    println!("  Stream: {}", stats.stream_id)
    println!("  Rows processed: {}", stats.rows_processed)
    println!("  Rows flagged: {}", stats.rows_with_dq_flags)
    println!("  Rows rejected: {}", stats.rows_rejected)
    println!("  Duration: {}ms", stats.duration_ms)

    Ok(())
```

### Backfill Command

```
FUNCTION handle_backfill_command(
    stream_id: &str,
    bronze_path: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>
) -> Result<(), Error>:

    # 1. Load configuration
    config = load_stream_config(stream_id).await?

    # 2. Create modified config for backfill
    backfill_config = modify_config_for_backfill(&config, start_time, end_time)

    # 3. Create ETL runner and persistence
    runner = EtlRunner::from_env()?
    persistence = create_persistence_from_env()?

    # 4. Execute with backfill mode tracking
    stats = run_etl_with_persistence(
        &runner,
        &persistence,
        &backfill_config,
        stream_id,
        bronze_path,
        EtlRunMode::Backfill  # Different run_mode
    )?

    println!("Backfill completed for {} -> {}", start_time, end_time)
    println!("  Rows processed: {}", stats.rows_processed)

    Ok(())
```

---

## Helper Functions

### Create Persistence from Environment

```
FUNCTION create_persistence_from_env() -> Result<DuckDbRunPersistence, Error>:

    # Get connection string from env
    pg_conn_str = get_postgres_connection_string()?

    # Create DuckDB connection with postgres extension
    conn = Connection::open_in_memory()?
    conn.execute_batch("INSTALL postgres; LOAD postgres;")?
    conn.execute_batch(&format!(
        "ATTACH '{}' AS pg (TYPE postgres)",
        pg_conn_str
    ))?

    # Create persistence layer
    persistence = DuckDbRunPersistence::new(&conn)?

    RETURN Ok(persistence)
```

### Modify Config for Backfill

```
FUNCTION modify_config_for_backfill(
    config: &SilverEtlConfig,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>
) -> SilverEtlConfig:

    # Clone config
    mut backfill_config = config.clone()

    # Override incremental settings for bounded backfill
    backfill_config.incremental = IncrementalConfig {
        enabled: true,
        watermark_column: config.incremental.watermark_column.clone(),
        lag_interval: "0 seconds".to_string(),  # No lag for backfill
    }

    # Add time bounds (if supported by ETL)
    backfill_config.backfill_bounds = Some(BackfillBounds {
        start_time,
        end_time,
    })

    RETURN backfill_config
```

---

## EtlRunner Optional Enhancements

### Add run_id to EtlStats (Optional)

```
# Useful for correlation in logs

STRUCT EtlStats:
    pub run_id: Option<UUID>          # NEW: Set by wrapper
    pub stream_id: String
    pub rows_processed: u64
    ...
```

### Add Timing Points (Optional)

```
# More granular timing for debugging

STRUCT EtlStats:
    ...
    pub timing: Option<EtlTiming>

STRUCT EtlTiming:
    pub resolve_files_ms: u64
    pub get_watermark_ms: u64
    pub generate_sql_ms: u64
    pub execute_sql_ms: u64
    pub post_dq_query_ms: u64
```

---

## Testing Strategy

### Unit Test: Wrapper Function Success Path

```
#[test]
fn test_run_etl_with_persistence_success():

    # Arrange
    config = create_test_config()
    mut mock_runner = MockEtlRunner::new()  # Would need trait extraction
    mut mock_persistence = MockEtlRunPersistence::new()

    run_id = Uuid::new_v4()

    mock_persistence.expect_start_run()
        .returning(move |_, _, _| Ok(run_id))

    mock_runner.expect_run_etl()
        .returning(|_, _, _| Ok(make_test_stats()))

    mock_persistence.expect_complete_run()
        .with(eq(run_id), always())
        .returning(|_, _| Ok(()))

    # Act
    result = run_etl_with_persistence(
        &mock_runner,
        &mock_persistence,
        &config,
        "test-stream",
        "/data/raw",
        EtlRunMode::Manual
    )

    # Assert
    assert!(result.is_ok())
```

### Unit Test: Wrapper Function Failure Path

```
#[test]
fn test_run_etl_with_persistence_failure():

    # Arrange
    config = create_test_config()
    mut mock_runner = MockEtlRunner::new()
    mut mock_persistence = MockEtlRunPersistence::new()

    run_id = Uuid::new_v4()

    mock_persistence.expect_start_run()
        .returning(move |_, _, _| Ok(run_id))

    # ETL fails
    mock_runner.expect_run_etl()
        .returning(|_, _, _| Err(EtlError::SqlExecution("Syntax error".to_string())))

    # Should call fail_run
    mock_persistence.expect_fail_run()
        .with(eq(run_id), always(), always())
        .returning(|_, _, _| Ok(()))

    # Should NOT call complete_run
    mock_persistence.expect_complete_run().times(0)

    # Act
    result = run_etl_with_persistence(...)

    # Assert - Error is propagated
    assert!(result.is_err())
    assert!(matches!(result.unwrap_err(), EtlError::SqlExecution(_)))
```

### Unit Test: Persistence Failure Doesn't Block ETL

```
#[test]
fn test_etl_succeeds_when_persistence_fails():

    # Arrange
    mut mock_runner = MockEtlRunner::new()
    mut mock_persistence = MockEtlRunPersistence::new()

    # Persistence fails
    mock_persistence.expect_start_run()
        .returning(|_, _, _| Err(PersistenceError::ConnectionError("DB down".to_string())))

    # ETL succeeds
    mock_runner.expect_run_etl()
        .returning(|_, _, _| Ok(make_test_stats()))

    # Act
    result = run_etl_with_persistence(...)

    # Assert - ETL result is returned despite persistence failure
    assert!(result.is_ok())
```

---

## File Organization

```
apps/silver-etl/src/
    etl.rs              # EtlRunner (unchanged core logic)
    persistence.rs      # NEW: EtlRunPersistence trait and impl
    daemon.rs           # Modified: Uses persistence
    run_with_tracking.rs # NEW: run_etl_with_persistence function
    cli.rs or main.rs   # Modified: CLI commands use tracking
```

---

## Summary of Changes

| File | Change |
|------|--------|
| `etl.rs` | No changes to EtlRunner core |
| `persistence.rs` | New file with trait and DuckDb impl |
| `daemon.rs` | Add persistence parameter, track runs |
| `run_with_tracking.rs` | New wrapper function |
| `main.rs` / `cli.rs` | Use wrapper for CLI commands |

---

*Pseudocode created: 2026-01-16*
*Next: SQL-STATEMENTS.md*
