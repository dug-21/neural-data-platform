# DP-011: TDD Strategy for ETL Run Statistics Persistence

**Feature ID**: dp-011
**Phase**: Refinement (SPARC R)
**Created**: 2026-01-16
**Testing Approach**: London School TDD with mockall

---

## Overview

This document defines the Test-Driven Development strategy for implementing ETL run statistics persistence in the silver-etl application. Following London School TDD principles, we focus on behavior verification through mock-driven testing.

---

## London School TDD Principles for dp-011

### 1. Outside-In Development

Start from the **daemon layer** (highest level) and work down to the **persistence layer**:

```
DaemonRunner (orchestrates cycles)
    |
    v
EtlExecutor (runs ETL per stream)
    |
    v
EtlRunPersistence (writes to database)
    |
    v
silver.etl_runs (TimescaleDB table)
```

### 2. Mock Boundaries

| Component | Mock Strategy | Why |
|-----------|---------------|-----|
| `EtlExecutor` | Existing `MockEtlExecutor` | Already has `#[cfg_attr(test, mockall::automock)]` |
| `EtlRunPersistence` | New `MockEtlRunPersistence` | Isolate database writes |
| `DuckDB Connection` | Mock via trait | Avoid real database in unit tests |
| `Uuid::new_v4()` | Don't mock | Deterministic enough, use returned value |
| `Utc::now()` | Mock via injection | Control timestamps in tests |

### 3. Behavior Verification Focus

Test **what interactions happen**, not just return values:

```rust
// WRONG: Just checking return value
assert!(result.is_ok());

// RIGHT: Verify persistence was called with correct data
mock_persistence.expect_start_run()
    .with(eq("air-quality"), eq(EtlRunMode::Daemon), predicate::always())
    .times(1)
    .returning(|_, _, _| Ok(Uuid::new_v4()));
```

### 4. Contract-First Design

Define the `EtlRunPersistence` trait contract BEFORE implementation:

```rust
/// Trait for ETL run persistence - enables mocking in tests
#[cfg_attr(test, mockall::automock)]
pub trait EtlRunPersistence: Send + Sync {
    /// Insert a new run record (status = 'running')
    /// Returns the UUID of the created run record
    fn start_run(
        &self,
        stream_id: &str,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError>;

    /// Update run with completion status and stats
    fn complete_run(&self, id: Uuid, stats: &EtlStats) -> Result<(), PersistenceError>;

    /// Update run with failure status and error
    fn fail_run(
        &self,
        id: Uuid,
        error: &str,
        context: Option<serde_json::Value>,
    ) -> Result<(), PersistenceError>;
}
```

---

## Test Pyramid

```
                    /\
                   /  \
                  / E2E \           1 test (full stack verification)
                 /------\
                /        \
               /  Integ   \         3 tests (real database)
              /------------\
             /              \
            /     Unit       \      15+ tests (mock-based)
           /------------------\
```

### Layer Breakdown

| Layer | Count | Database | Speed | Purpose |
|-------|-------|----------|-------|---------|
| **Unit** | 15+ | Mock | Fast (<1ms) | Behavior verification |
| **Integration** | 3 | Real TimescaleDB | Slow (~500ms) | Database contract verification |
| **E2E** | 1 | Real stack | Slowest | Full pipeline verification |

---

## Test Categories

### 1. Persistence Layer Unit Tests (Mock-Based)

Location: `apps/silver-etl/src/persistence.rs`

| Test | Verifies |
|------|----------|
| `test_start_run_creates_record` | INSERT with correct columns |
| `test_start_run_returns_uuid` | UUID generation and return |
| `test_complete_run_updates_status` | UPDATE with status='success' |
| `test_complete_run_sets_statistics` | Stats fields populated |
| `test_fail_run_records_error` | UPDATE with status='failed', error_message |
| `test_fail_run_stores_context` | error_context JSONB stored |
| `test_persistence_failure_graceful` | Errors don't propagate up |

### 2. Daemon Layer Unit Tests (Mock Persistence)

Location: `apps/silver-etl/src/daemon.rs`

| Test | Verifies |
|------|----------|
| `test_daemon_persists_each_stream_run` | start_run + complete_run per stream |
| `test_failed_stream_persists_error` | fail_run called on ETL failure |
| `test_daemon_cycle_id_shared` | All runs in cycle share UUID |
| `test_persistence_failure_continues_etl` | Daemon doesn't crash on persistence error |

### 3. Integration Tests (Real Database)

Location: `tests/integration/test_persistence.rs`

| Test | Verifies |
|------|----------|
| `test_persistence_roundtrip` | Full write + read cycle |
| `test_multiple_streams_same_cycle` | daemon_cycle_id links correctly |
| `test_retention_cleanup` | Old records deleted |

---

## Mock Setup Patterns

### Pattern 1: Basic MockEtlRunPersistence

```rust
use mockall::predicate::*;

fn setup_mock_persistence() -> MockEtlRunPersistence {
    let mut mock = MockEtlRunPersistence::new();

    // Default: All operations succeed
    mock.expect_start_run()
        .returning(|_, _, _| Ok(Uuid::new_v4()));
    mock.expect_complete_run()
        .returning(|_, _| Ok(()));
    mock.expect_fail_run()
        .returning(|_, _, _| Ok(()));

    mock
}
```

### Pattern 2: Verifying Call Arguments

```rust
#[test]
fn test_start_run_uses_correct_stream_id() {
    let mut mock = MockEtlRunPersistence::new();

    mock.expect_start_run()
        .with(
            eq("air-quality"),           // Exact stream_id match
            eq(EtlRunMode::Daemon),       // Exact mode match
            predicate::always()           // Any cycle_id
        )
        .times(1)                         // Exactly once
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    // ... exercise code ...
}
```

### Pattern 3: Verifying Call Order

```rust
#[test]
fn test_start_before_complete() {
    let mut mock = MockEtlRunPersistence::new();
    let mut seq = mockall::Sequence::new();

    mock.expect_start_run()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    mock.expect_complete_run()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| Ok(()));

    // ... exercise code ...
}
```

### Pattern 4: Simulating Persistence Failures

```rust
#[test]
fn test_daemon_continues_on_persistence_failure() {
    let mut mock = MockEtlRunPersistence::new();

    // First stream's persistence fails
    mock.expect_start_run()
        .times(1)
        .returning(|_, _, _| Err(PersistenceError::Database("Connection lost".into())));

    // Second stream should still be attempted
    mock.expect_start_run()
        .times(1)
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    // ... verify daemon processes both streams ...
}
```

---

## Error Handling Strategy

### Persistence Failures MUST NOT Fail ETL

Critical requirement: ETL data processing must succeed even if statistics persistence fails.

```rust
// In daemon.rs run_cycle()
match persistence.start_run(stream_id, mode, cycle_id) {
    Ok(run_id) => {
        // Track run_id for later completion
    }
    Err(e) => {
        warn!(%e, stream_id = %stream_id, "Failed to start run record");
        // Continue with ETL - don't propagate error
    }
}
```

### Test for This Behavior

```rust
#[tokio::test]
async fn test_etl_succeeds_when_persistence_fails() {
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    // Persistence always fails
    mock_persistence.expect_start_run()
        .returning(|_, _, _| Err(PersistenceError::Database("Error".into())));

    // But ETL should still run
    mock_executor.expect_run_stream()
        .returning(|_| Ok(make_stats(100, 0, 0)));

    let result = daemon.run_cycle();

    // ETL cycle should succeed despite persistence failure
    assert!(result.is_ok());
    assert_eq!(result.unwrap().streams_succeeded, 1);
}
```

---

## Dependencies

### Cargo.toml Additions

```toml
[dependencies]
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
mockall = "0.12"
```

### Existing Patterns to Follow

- `apps/silver-etl/src/daemon.rs`: `#[cfg_attr(test, mockall::automock)]` on `EtlExecutor`
- `core/ndp-mcp-server/src/storage/`: Trait + automock pattern for storage

---

## Test Execution

### Running Unit Tests

```bash
# All silver-etl tests
cargo test --package silver-etl

# Just persistence tests
cargo test --package silver-etl persistence::tests

# Just daemon tests with persistence
cargo test --package silver-etl daemon::tests

# With output
cargo test --package silver-etl -- --nocapture
```

### Running Integration Tests

```bash
# Requires running TimescaleDB
cargo test --package silver-etl --test test_persistence -- --ignored
```

---

## TDD Workflow

### Red-Green-Refactor Cycle

1. **RED**: Write failing test for new behavior
2. **GREEN**: Implement minimum code to pass
3. **REFACTOR**: Clean up without changing behavior

### Example Workflow

```
1. Write test: test_start_run_creates_record
   - Mock database connection
   - Assert INSERT SQL contains correct columns
   - Run test -> FAILS (no implementation)

2. Implement: start_run method
   - Write INSERT SQL
   - Execute via DuckDB connection
   - Return UUID
   - Run test -> PASSES

3. Refactor:
   - Extract SQL to const
   - Add error handling
   - Run test -> STILL PASSES
```

---

## Success Criteria Alignment

| SCOPE Criterion | Test Coverage |
|-----------------|---------------|
| ETL runs persisted | `test_start_run_creates_record`, `test_persistence_roundtrip` |
| All streams tracked | `test_daemon_persists_each_stream_run` |
| Errors captured | `test_fail_run_records_error`, `test_fail_run_stores_context` |
| MCP queryable | Integration test with real database |
| Retention working | `test_retention_cleanup` |

---

## References

- [dp-010 ETL-STATUS-SPEC.md](../../dp-010/specification/ETL-STATUS-SPEC.md) - Schema definition
- [daemon.rs tests](../../../../apps/silver-etl/src/daemon.rs) - Existing MockEtlExecutor pattern
- [AIR-005-TEST-DESIGN.md](../../../../docs/testing/AIR-005-TEST-DESIGN.md) - London TDD patterns
- [mcp-tool-testing-pattern](https://agentdb) - AgentDB stored pattern

---

*Strategy created: 2026-01-16*
*Ready for implementation: Yes*
