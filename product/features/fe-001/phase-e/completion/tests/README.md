# Phase E Acceptance Tests

> **Test Approach:** London TDD (Outside-In)
> **Status:** Tests written FIRST - will FAIL until implementation complete
> **Created:** 2026-02-05

---

## Overview

These acceptance tests define the expected behavior for Phase E: Unified Event Abstraction. Following London TDD methodology, these tests are written **before** implementation and will initially fail.

---

## Test Files

| File | Covers | Acceptance Criteria |
|------|--------|---------------------|
| `acceptance_events_hypertable.sql` | Events hypertable schema, indexes, retention | AC-E02-001, FR-E02-001-009 |
| `acceptance_threshold_crossings.sql` | Threshold crossing detection, all conditions | AC-E-01, AC-E-02, AC-E-04 |
| `acceptance_unified_events.sql` | Unified view, hourly CA, V1.2 patterns | AC-E-03, AC-E-05, AC-E-06 |
| `acceptance_detection_job.sql` | Detection job scheduling, idempotency | FR-E01-009, AC-E-INT-01 |

---

## Running Tests

### Prerequisites

1. TimescaleDB running (via Docker or local)
2. Database `ndp` exists
3. User has CREATE/SELECT permissions on gold schema

### Execute All Tests

```bash
# From project root
DEPLOY_ENV=integration deploy/pi/deploy.sh test-phase-e

# Or run directly with psql
docker exec timescaledb psql -U postgres -d ndp \
  -f product/features/fe-001/phase-e/completion/tests/acceptance_events_hypertable.sql

docker exec timescaledb psql -U postgres -d ndp \
  -f product/features/fe-001/phase-e/completion/tests/acceptance_threshold_crossings.sql

docker exec timescaledb psql -U postgres -d ndp \
  -f product/features/fe-001/phase-e/completion/tests/acceptance_unified_events.sql

docker exec timescaledb psql -U postgres -d ndp \
  -f product/features/fe-001/phase-e/completion/tests/acceptance_detection_job.sql
```

### Run Individual Test File

```bash
# Events hypertable tests
docker exec timescaledb psql -U postgres -d ndp \
  -f /path/to/acceptance_events_hypertable.sql 2>&1 | grep -E "(PASS|FAIL|SKIP)"
```

---

## Expected Test Results

### Before Implementation (London TDD)

All tests should **FAIL** with messages like:
- `FAIL: gold.events is not a hypertable (or does not exist)`
- `FAIL: gold.events_unified view does not exist`
- `FAIL: detect_events job is not scheduled`

### After Implementation

All tests should **PASS**:
```
PASS: AC-E02-001-a gold.events exists as hypertable
PASS: AC-E02-001-b gold.events has 7-day chunk interval
PASS: AC-E02-001-c gold.events has all required columns
...
```

---

## Acceptance Criteria Mapping

### AC-E-01: Threshold Crossing Generator Works

| Test ID | Description | File |
|---------|-------------|------|
| AC-E-01-001 | Rising crossing detected | acceptance_threshold_crossings.sql |
| AC-E-01-002 | Falling crossing detected | acceptance_threshold_crossings.sql |
| AC-E-01-003 | No spurious crossings | acceptance_threshold_crossings.sql |

### AC-E-02: All Condition Types Supported

| Test ID | Description | File |
|---------|-------------|------|
| AC-E-02-001 | Condition `<` | acceptance_threshold_crossings.sql |
| AC-E-02-002 | Condition `<=` | acceptance_threshold_crossings.sql |
| AC-E-02-003 | Condition `>` | acceptance_threshold_crossings.sql |
| AC-E-02-004 | Condition `>=` | acceptance_threshold_crossings.sql |
| AC-E-02-005 | Condition `between` (entering) | acceptance_threshold_crossings.sql |
| AC-E-02-006 | Condition `between` (exiting low) | acceptance_threshold_crossings.sql |
| AC-E-02-007 | Condition `between` (exiting high) | acceptance_threshold_crossings.sql |

### AC-E-03: Unified Events View

| Test ID | Description | File |
|---------|-------------|------|
| AC-E-03-001 | View exists | acceptance_unified_events.sql |
| AC-E-03-002 | State transitions included | acceptance_unified_events.sql |
| AC-E-03-003 | Threshold crossings included | acceptance_unified_events.sql |
| AC-E-03-004 | Consistent schema | acceptance_unified_events.sql |

### AC-E-04: Event Schema Contract

| Test ID | Description | File |
|---------|-------------|------|
| AC-E-04-001 | Required columns exist | acceptance_threshold_crossings.sql |
| AC-E-04-002 | crossing_direction is TEXT | acceptance_threshold_crossings.sql |
| AC-E-04-003 | threshold_value is DOUBLE PRECISION | acceptance_threshold_crossings.sql |

### AC-E-05: Hourly Event Aggregate

| Test ID | Description | File |
|---------|-------------|------|
| AC-E-05-001 | CA exists | acceptance_unified_events.sql |
| AC-E-05-002 | bucket column | acceptance_unified_events.sql |
| AC-E-05-003 | total_events column | acceptance_unified_events.sql |
| AC-E-05-004 | state_transition_count | acceptance_unified_events.sql |
| AC-E-05-005 | threshold_crossing_count | acceptance_unified_events.sql |
| AC-E-05-006 | Joinable with aligned view | acceptance_unified_events.sql |

### AC-E-06: V1.2 Query Patterns

| Test ID | Description | File |
|---------|-------------|------|
| AC-E-06-001 | Time range query | acceptance_unified_events.sql |
| AC-E-06-002 | Event type filter | acceptance_unified_events.sql |
| AC-E-06-003 | Objective ID filter | acceptance_unified_events.sql |
| AC-E-06-004 | Aligned view join | acceptance_unified_events.sql |
| AC-E-06-005 | Context query | acceptance_unified_events.sql |
| AC-E-06-006 | Direction filter | acceptance_unified_events.sql |

---

## Test Categories

### Schema Tests
- Verify tables/views/CAs exist
- Verify column types and constraints
- Verify indexes exist

### Behavioral Tests
- Verify detection logic (requires test data)
- Verify query patterns work
- Verify joins work

### Performance Tests
- Verify indexes are used (EXPLAIN ANALYZE)
- Query execution time (< 100ms for 30-day range)

---

## Adding New Tests

When adding new tests, follow this pattern:

```sql
-- Test: AC-XX-NNN Description
DO $$
DECLARE
    result BOOLEAN;
BEGIN
    -- Arrange: Set up test state (if needed)

    -- Act: Execute the query/operation
    SELECT EXISTS (...) INTO result;

    -- Assert: Check the result
    IF NOT result THEN
        RAISE EXCEPTION 'FAIL: AC-XX-NNN Description of failure';
    END IF;

    RAISE NOTICE 'PASS: AC-XX-NNN Description';
END $$;
```

---

## References

- [ACCEPTANCE-CRITERIA.md](../ACCEPTANCE-CRITERIA.md) - Full acceptance criteria
- [TEST-PLAN.md](../../refinement/TEST-PLAN.md) - Test plan and strategy
- [SPEC-E01-threshold-crossings.md](../../specification/SPEC-E01-threshold-crossings.md) - Threshold spec
- [SPEC-E02-unified-events-view.md](../../specification/SPEC-E02-unified-events-view.md) - Events spec

---

*Tests created: 2026-02-05 by ndp-tester*
