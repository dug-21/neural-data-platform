# DP-011: Acceptance Criteria Checklist

**Feature ID**: dp-011
**Phase**: Refinement (SPARC R)
**Created**: 2026-01-16
**Purpose**: Verify feature completion against success criteria

---

## Overview

This checklist tracks acceptance criteria for ETL run statistics persistence. All items must be checked before the feature is considered complete.

---

## Acceptance Criteria

### AC-1: ETL Runs Persisted

**Requirement**: All ETL runs create database records in `silver.etl_runs`

| Criterion | Test | Status |
|-----------|------|--------|
| start_run creates record with status='running' | `test_start_run_creates_record` | [ ] |
| complete_run updates status to 'success' | `test_complete_run_updates_status` | [ ] |
| All fields populated correctly | `test_complete_run_sets_statistics` | [ ] |
| Records queryable via SQL | `test_persistence_roundtrip` | [ ] |

**Verification Query**:
```sql
SELECT COUNT(*) FROM silver.etl_runs;
-- Should return > 0 after daemon runs
```

---

### AC-2: All Streams Tracked

**Requirement**: Each enabled stream has run records in the database

| Criterion | Test | Status |
|-----------|------|--------|
| Every enabled stream gets start_run call | `test_daemon_persists_each_stream_run` | [ ] |
| Stream filter limits persistence to filtered stream | `test_stream_filter_with_persistence` | [ ] |
| Multiple streams in same cycle linked | `test_daemon_cycle_id_shared` | [ ] |

**Verification Query**:
```sql
SELECT DISTINCT stream_id FROM silver.etl_runs ORDER BY stream_id;
-- Should list all enabled streams
```

---

### AC-3: Success Runs Have Statistics

**Requirement**: Successful runs have status='success' and complete statistics

| Criterion | Test | Status |
|-----------|------|--------|
| status = 'success' on completion | `test_complete_run_updates_status` | [ ] |
| rows_processed populated | `test_complete_run_sets_statistics` | [ ] |
| rows_flagged populated | `test_complete_run_sets_statistics` | [ ] |
| duration_ms calculated | `test_complete_run_sets_statistics` | [ ] |
| watermark_after set | `test_complete_run_sets_statistics` | [ ] |
| completed_at timestamp set | `test_complete_run_updates_status` | [ ] |

**Verification Query**:
```sql
SELECT stream_id, status, rows_processed, rows_flagged, duration_ms,
       watermark_before, watermark_after, completed_at
FROM silver.etl_runs
WHERE status = 'success'
ORDER BY started_at DESC
LIMIT 5;
-- All columns should have values for successful runs
```

---

### AC-4: Failed Runs Have Error Details

**Requirement**: Failed runs have status='failed' and error_message populated

| Criterion | Test | Status |
|-----------|------|--------|
| status = 'failed' on error | `test_fail_run_records_error` | [ ] |
| error_message stored | `test_fail_run_records_error` | [ ] |
| error_context JSONB stored | `test_fail_run_stores_context` | [ ] |
| completed_at timestamp set | `test_fail_run_records_error` | [ ] |

**Verification Query**:
```sql
SELECT stream_id, status, error_message, error_context->>'stage' as stage
FROM silver.etl_runs
WHERE status = 'failed'
ORDER BY started_at DESC
LIMIT 5;
-- Should show error details for failed runs
```

---

### AC-5: Daemon Cycle ID Links Runs

**Requirement**: All runs from the same daemon cycle share daemon_cycle_id

| Criterion | Test | Status |
|-----------|------|--------|
| UUID generated once per cycle | `test_daemon_cycle_id_shared` | [ ] |
| Same UUID passed to all streams in cycle | `test_daemon_cycle_id_shared` | [ ] |
| Query by daemon_cycle_id returns all runs | `test_multiple_streams_same_cycle` | [ ] |

**Verification Query**:
```sql
SELECT daemon_cycle_id, COUNT(*) as run_count,
       array_agg(stream_id) as streams
FROM silver.etl_runs
WHERE daemon_cycle_id IS NOT NULL
GROUP BY daemon_cycle_id
ORDER BY MIN(started_at) DESC
LIMIT 5;
-- Each cycle should have multiple streams with same daemon_cycle_id
```

---

### AC-6: Persistence Failures Do Not Fail ETL

**Requirement**: ETL data processing succeeds even if persistence fails

| Criterion | Test | Status |
|-----------|------|--------|
| start_run failure logged but ETL continues | `test_persistence_failure_continues_etl` | [ ] |
| complete_run failure logged but cycle succeeds | `test_complete_run_failure_logged` | [ ] |
| Daemon cycle stats reflect ETL outcome, not persistence | `test_persistence_failure_continues_etl` | [ ] |
| No panic on persistence error | `test_persistence_failure_graceful` | [ ] |

**Verification**: Manual test by stopping TimescaleDB mid-ETL:
```bash
# 1. Start daemon
silver-etl daemon --interval 60

# 2. Stop TimescaleDB
docker stop timescaledb

# 3. Wait for ETL cycle

# 4. Check daemon logs - should show warning, not crash
# Expected: WARN ... "Failed to start run record"

# 5. Restart TimescaleDB
docker start timescaledb

# 6. Next cycle should persist normally
```

---

### AC-7: 30-Day Retention Policy

**Requirement**: Run records older than 30 days are automatically cleaned up

| Criterion | Test | Status |
|-----------|------|--------|
| Cleanup query deletes old records | `test_retention_cleanup` | [ ] |
| Records < 30 days preserved | `test_retention_cleanup` | [ ] |
| Cleanup is idempotent | `test_retention_cleanup` | [ ] |

**Verification** (after 30+ days or with backdated test data):
```sql
-- Check oldest record
SELECT MIN(created_at), MAX(created_at),
       NOW() - MIN(created_at) as oldest_age
FROM silver.etl_runs;
-- oldest_age should be < 30 days after cleanup runs
```

**Cleanup Command**:
```sql
DELETE FROM silver.etl_runs WHERE created_at < NOW() - INTERVAL '30 days';
```

---

### AC-8: MCP Can Query Records

**Requirement**: dp-010 MCP `etl_status` tool can query this table

| Criterion | Test | Status |
|-----------|------|--------|
| Table schema matches MCP tool expectations | Schema review | [ ] |
| Indexes support MCP query patterns | Schema review | [ ] |
| `etl_status` tool returns data | MCP tool integration test | [ ] |
| `etl_history` tool returns data | MCP tool integration test | [ ] |
| `data_freshness` tool can query runs | MCP tool integration test | [ ] |

**Verification**:
```bash
# Use MCP tool (when implemented)
echo '{"tool": "etl_status", "arguments": {"stream_id": "air-quality"}}' | \
  curl -X POST http://localhost:8080/mcp/tools/call -d @-

# Expected: JSON with last_run details from silver.etl_runs
```

---

## Test Execution Summary

### Unit Tests

| Test Suite | Count | Command |
|------------|-------|---------|
| Persistence | 11 | `cargo test --package silver-etl persistence::tests` |
| Daemon | 8 | `cargo test --package silver-etl daemon::tests` |
| **Total** | **19** | `cargo test --package silver-etl` |

### Integration Tests

| Test Suite | Count | Command |
|------------|-------|---------|
| DB Integration | 5 | `cargo test --package silver-etl -- --ignored` |

---

## Checklist Sign-Off

### Phase 1: Unit Tests Passing

- [ ] All unit tests pass: `cargo test --package silver-etl`
- [ ] No clippy warnings: `cargo clippy --package silver-etl`
- [ ] Code formatted: `cargo fmt --package silver-etl --check`

### Phase 2: Integration Tests Passing

- [ ] TimescaleDB running with migrations applied
- [ ] All integration tests pass: `cargo test --package silver-etl -- --ignored`

### Phase 3: Manual Verification

- [ ] Daemon runs and persists records to database
- [ ] Failed streams have error details
- [ ] MCP tools can query records (after dp-010 implementation)

### Phase 4: Documentation

- [ ] STATUS.md updated with completion
- [ ] Any new patterns saved to AgentDB via `save-pattern`
- [ ] Feedback recorded via `reflexion` skill

---

## Acceptance Criteria Matrix

| AC | Description | Unit | Integration | Manual |
|----|-------------|------|-------------|--------|
| AC-1 | ETL runs persisted | x | x | x |
| AC-2 | All streams tracked | x | x | x |
| AC-3 | Success stats complete | x | x | x |
| AC-4 | Failed runs have errors | x | x | |
| AC-5 | Cycle ID links runs | x | x | x |
| AC-6 | Persistence failure graceful | x | | x |
| AC-7 | Retention policy works | | x | |
| AC-8 | MCP queryable | | | x |

---

## Final Approval

| Reviewer | Date | Status |
|----------|------|--------|
| Tester (automated) | | [ ] Tests pass |
| Developer | | [ ] Code complete |
| Architect | | [ ] Design approved |
| Product | | [ ] Requirements met |

---

*Acceptance checklist created: 2026-01-16*
*Feature complete when all checkboxes are checked*
