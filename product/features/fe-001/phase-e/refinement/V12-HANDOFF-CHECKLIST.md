# Phase E: V1.2 Handoff Checklist

> **Phase:** E (Unified Event Abstraction)
> **Purpose:** Validate V1.1 completion and V1.2 readiness
> **Target:** V1.2 Pattern Detection Engine can begin immediately after checklist complete
> **Created:** 2026-02-04

---

## Overview

This checklist ensures V1.1 Gold Layer Foundation delivers everything V1.2 Pattern Detection Engine requires. V1.2 should be able to start development immediately after this checklist is verified.

**V1.2 Primary Consumer**: Pattern Detection Engine (Granger causality, correlation analysis)

---

## 1. Data Availability Checklist

### 1.1 Gold Layer Views

| Object | Schema | Required By V1.2 | Verified |
|--------|--------|------------------|----------|
| `gold.air_quality_hourly` | Continuous aggregate | Correlation scanning | [ ] |
| `gold.air_quality_daily` | Continuous aggregate | Daily patterns | [ ] |
| `gold.outdoor_weather_hourly` | Continuous aggregate | Context data | [ ] |
| `gold.state_events_hourly` | Continuous aggregate | Cause candidates | [ ] |
| `gold.outdoor_air_quality_hourly` | Continuous aggregate | Outdoor context | [ ] |
| `gold.indoor_air_quality_aligned` | Materialized view | All-stream join | [ ] |
| `gold.events_unified` | View | Event queries | [ ] |
| `gold.events_hourly` | Continuous aggregate | Event counts | [ ] |
| `gold.state_transitions` | View | Transition events | [ ] |
| `gold.threshold_crossings` | View | Crossing events | [ ] |

**Verification Query:**
```sql
SELECT schemaname, matviewname, ispopulated
FROM pg_matviews
WHERE schemaname = 'gold';

SELECT schemaname, viewname
FROM pg_views
WHERE schemaname = 'gold';
```

### 1.2 Minimum Data Volume

| Requirement | Minimum | Verification |
|-------------|---------|--------------|
| Aligned view rows | 720 (30 days * 24 hours) | `SELECT COUNT(*) FROM gold.indoor_air_quality_aligned WHERE bucket >= NOW() - INTERVAL '30 days'` |
| Events | 10+ | `SELECT COUNT(*) FROM gold.events_unified` |
| State transitions | 5+ | `SELECT COUNT(*) FROM gold.state_transitions WHERE is_actual_transition` |
| Streams in aligned | 4 | Column count check |

---

## 2. Schema Contract Checklist

### 2.1 Aligned View Schema

V1.2 expects these columns in `gold.indoor_air_quality_aligned`:

| Column Pattern | Type | Purpose | Verified |
|----------------|------|---------|----------|
| `bucket` | TIMESTAMPTZ | Time join key | [ ] |
| `indoor_pm25*` | FLOAT | Primary target metric | [ ] |
| `indoor_co2*` | FLOAT | Primary target metric | [ ] |
| `outdoor_temp*` | FLOAT | Context data | [ ] |
| `state_*` or `se_*` | Various | State metrics | [ ] |
| `oaq_*` | Various | Outdoor air quality | [ ] |

**Verification Query:**
```sql
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'gold'
  AND table_name = 'indoor_air_quality_aligned'
ORDER BY ordinal_position;
```

### 2.2 Unified Events Schema

V1.2 expects this exact schema for `gold.events_unified`:

| Column | Type | Nullable | Purpose | Verified |
|--------|------|----------|---------|----------|
| `event_id` | UUID | NO | Unique identifier | [ ] |
| `event_time` | TIMESTAMPTZ | NO | When event occurred | [ ] |
| `stream_id` | TEXT | NO | Source stream | [ ] |
| `entity_id` | TEXT | NO | Which entity (ndp_id) | [ ] |
| `event_type` | TEXT | NO | 'state_transition' or 'threshold_crossing' | [ ] |
| `details` | JSONB | NO | Type-specific payload | [ ] |

**Verification Query:**
```sql
SELECT column_name, data_type, is_nullable
FROM information_schema.columns
WHERE table_schema = 'gold'
  AND table_name = 'events_unified'
ORDER BY ordinal_position;
```

### 2.3 Event Details Schema

#### State Transition Details

```json
{
  "from_state": "off",
  "to_state": "on",
  "duration_in_previous_ms": 3600000
}
```

**Verification Query:**
```sql
SELECT details
FROM gold.events_unified
WHERE event_type = 'state_transition'
LIMIT 1;
```

#### Threshold Crossing Details

```json
{
  "metric": "co2",
  "threshold": 800,
  "direction": "rising",
  "value": 812,
  "previous_value": 795,
  "objective_id": "healthy_co2",
  "condition": "<"
}
```

**Verification Query:**
```sql
SELECT details
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
LIMIT 1;
```

---

## 3. Query Pattern Verification

### 3.1 Required V1.2 Query Patterns

V1.2 will execute these queries. All must work:

| Pattern | Query | Expected Result | Verified |
|---------|-------|-----------------|----------|
| Time range events | See below | Returns rows | [ ] |
| Events by type | See below | Returns rows | [ ] |
| Events + aligned join | See below | Returns rows | [ ] |
| Events by objective | See below | Returns rows (if crossings exist) | [ ] |

**Pattern 1: Time Range Events**
```sql
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '7 days'
ORDER BY event_time DESC
LIMIT 100;
```

**Pattern 2: Events by Type**
```sql
SELECT COUNT(*), event_type
FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '7 days'
GROUP BY event_type;
```

**Pattern 3: Events + Aligned Join**
```sql
SELECT
    a.bucket,
    a.indoor_pm25,
    a.indoor_co2,
    e.total_events,
    e.state_transition_count,
    e.threshold_crossing_count
FROM gold.indoor_air_quality_aligned a
LEFT JOIN gold.events_hourly e ON a.bucket = e.bucket
WHERE a.bucket >= NOW() - INTERVAL '30 days'
ORDER BY a.bucket DESC
LIMIT 720;
```

**Pattern 4: Events by Objective**
```sql
SELECT *
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
  AND details->>'objective_id' = 'healthy_co2'
ORDER BY event_time DESC;
```

### 3.2 Performance Requirements

| Query | Target | Verified |
|-------|--------|----------|
| Aligned view 30-day | < 100ms | [ ] |
| Events unified 30-day | < 100ms | [ ] |
| Events + aligned join | < 200ms | [ ] |

**Verification:**
```sql
EXPLAIN ANALYZE
SELECT * FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '30 days';

EXPLAIN ANALYZE
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '30 days';
```

---

## 4. Metadata Checklist

### 4.1 Data Dictionary

| Entry | Table | Required | Verified |
|-------|-------|----------|----------|
| Stream classifications | `data_dictionary.stream_classification` | Yes | [ ] |
| Gold tables | `data_dictionary.gold_tables` | Yes | [ ] |
| Gold columns | `data_dictionary.gold_columns` | Yes | [ ] |
| Objectives | `data_dictionary.objectives` | Yes | [ ] |
| Domains | `data_dictionary.domains` | Yes | [ ] |

**Verification Query:**
```sql
SELECT COUNT(*) FROM data_dictionary.stream_classification WHERE stream_type IS NOT NULL;
SELECT COUNT(*) FROM data_dictionary.gold_tables;
SELECT COUNT(*) FROM data_dictionary.gold_columns;
SELECT COUNT(*) FROM data_dictionary.objectives;
```

### 4.2 MCP Tool Availability

| Tool | Purpose | Verified |
|------|---------|----------|
| Query stream classification | Get stream types | [ ] |
| Query objectives | Get thresholds | [ ] |
| Query gold tables | Get available tables | [ ] |
| Query aligned view schema | Get column definitions | [ ] |

---

## 5. Observability Checklist

### 5.1 Monitoring Metrics

| Metric | Purpose | Available | Verified |
|--------|---------|-----------|----------|
| `gold_continuous_aggregate_refresh_duration` | Refresh health | Prometheus | [ ] |
| `gold_aligned_view_row_count` | Data volume | SQL query | [ ] |
| `gold_events_per_hour` | Event rate | SQL query | [ ] |
| `gold_threshold_crossings_per_day` | Crossing frequency | SQL query | [ ] |

### 5.2 Alerting (Optional - for V1.3)

| Alert | Condition | Status |
|-------|-----------|--------|
| Aggregate refresh failure | Job failed 3 times | Deferred |
| High crossing frequency | > 100/hour/objective | Deferred |
| Aligned view stale | No new rows in 2 hours | Deferred |

---

## 6. Documentation Checklist

### 6.1 Architecture Documents

| Document | Purpose | Up to Date | Verified |
|----------|---------|------------|----------|
| SCOPE.md | Feature requirements | Yes | [ ] |
| DECISIONS.md | ADRs | Yes | [ ] |
| ADR-FE001-001 | Gold DDL in Rust | Yes | [ ] |
| ADR-FE001-002 | Domain-centric config | Yes | [ ] |
| ADR-FE001-003 | Forecast alignment | Yes | [ ] |
| ADR-FE001-004 | NULL handling | Yes | [ ] |
| ADR-FE001-005 | Manifest idempotency | Yes | [ ] |

### 6.2 Schema Documentation

| Schema | Documented | Location |
|--------|------------|----------|
| gold_etl.schema.json | Yes | `config/schemas/` |
| alignment.schema.json | Yes | `config/schemas/` |
| objectives.schema.json | Yes | `config/schemas/` |

### 6.3 Usage Examples

| Example | Documented | Location |
|---------|------------|----------|
| Add new stream to Gold | Yes | VALIDATION-PROCEDURE.md |
| Add new objective | Yes | Domain config examples |
| Query aligned view | Yes | This document |
| Query events | Yes | This document |

---

## 7. Test Coverage Checklist

### 7.1 Test Suites Passing

| Suite | Tests | Passing | Verified |
|-------|-------|---------|----------|
| Phase A unit tests | ~30 | All | [ ] |
| Phase B unit tests | ~20 | All | [ ] |
| Phase C unit tests | ~25 | All | [ ] |
| Phase D validation | All | All | [ ] |
| Phase E unit tests | ~25 | All | [ ] |
| Integration tests | ~15 | All | [ ] |

**Verification:**
```bash
./scripts/test-all-phases.sh
```

### 7.2 Coverage Targets Met

| Component | Target | Actual | Verified |
|-----------|--------|--------|----------|
| continuous_aggregate.rs | 90% | __% | [ ] |
| aligned_view.rs | 85% | __% | [ ] |
| state_transitions.rs | 80% | __% | [ ] |
| events.rs | 85% | __% | [ ] |

---

## 8. Known Limitations (Document for V1.2)

### 8.1 Deferred Decisions

| Decision | Status | Impact on V1.2 |
|----------|--------|----------------|
| Threshold crossing deduplication | Deferred | V1.2 may see chattering |
| Hysteresis for crossings | Deferred | V1.2 handles raw crossings |
| Trend change events | V1.3 | Not available for V1.2 |
| Anomaly events | V1.3 | Not available for V1.2 |

### 8.2 Known Gaps

| Gap | Workaround | Fix Version |
|-----|------------|-------------|
| Forecast alignment not tested | Manual join on issued_at | V1.2 if needed |
| Event ID generation | UUID in SQL | Consider sequence |
| Large time range queries | May be slow | Indexing in V1.2 |

---

## 9. Handoff Meeting Agenda

### 9.1 V1.2 Team Walkthrough

1. **Aligned View Demo** (10 min)
   - Show schema
   - Demo queries
   - Explain NULL handling

2. **Events System Demo** (10 min)
   - Show unified events
   - Demo details JSONB
   - Explain event types

3. **MCP Tools Demo** (5 min)
   - Query objectives
   - Query classifications

4. **Architecture Review** (10 min)
   - Key ADRs
   - Extension patterns
   - Known limitations

5. **Q&A** (15 min)

### 9.2 Handoff Artifacts

| Artifact | Recipient | Delivered |
|----------|-----------|-----------|
| This checklist | V1.2 lead | [ ] |
| Query examples | V1.2 devs | [ ] |
| MCP tool docs | V1.2 devs | [ ] |
| Architecture docs | V1.2 architect | [ ] |

---

## 10. Sign-Off

### V1.1 Completion Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| V1.1 Lead | | | |
| ndp-architect | | | |
| ndp-tester | | | |

### V1.2 Acceptance Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| V1.2 Lead | | | |
| V1.2 Architect | | | |

---

## Verification Script

Save as `scripts/verify-v12-handoff.sh`:

```bash
#!/bin/bash
set -e

echo "=== V1.2 Handoff Verification ==="

echo "1. Checking Gold views..."
GOLD_VIEWS=$(dcx timescaledb psql -U postgres -d ndp -t -c "
SELECT COUNT(*)
FROM pg_matviews
WHERE schemaname = 'gold'"
)
echo "   Gold materialized views: $GOLD_VIEWS"

echo "2. Checking aligned view columns..."
ALIGNED_COLS=$(dcx timescaledb psql -U postgres -d ndp -t -c "
SELECT COUNT(*)
FROM information_schema.columns
WHERE table_schema = 'gold'
  AND table_name = 'indoor_air_quality_aligned'"
)
echo "   Aligned view columns: $ALIGNED_COLS"

echo "3. Checking events unified schema..."
dcx timescaledb psql -U postgres -d ndp -c "
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'gold'
  AND table_name = 'events_unified'
ORDER BY ordinal_position"

echo "4. Checking data dictionary entries..."
DD_STREAMS=$(dcx timescaledb psql -U postgres -d ndp -t -c "
SELECT COUNT(*) FROM data_dictionary.stream_classification")
DD_GOLD=$(dcx timescaledb psql -U postgres -d ndp -t -c "
SELECT COUNT(*) FROM data_dictionary.gold_tables")
DD_OBJ=$(dcx timescaledb psql -U postgres -d ndp -t -c "
SELECT COUNT(*) FROM data_dictionary.objectives")
echo "   Stream classifications: $DD_STREAMS"
echo "   Gold tables: $DD_GOLD"
echo "   Objectives: $DD_OBJ"

echo "5. Testing V1.2 query patterns..."
echo "   Pattern 1: Time range..."
time dcx timescaledb psql -U postgres -d ndp -c "
SELECT COUNT(*)
FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '30 days'" > /dev/null

echo "   Pattern 2: Aligned join..."
time dcx timescaledb psql -U postgres -d ndp -c "
SELECT COUNT(*)
FROM gold.indoor_air_quality_aligned a
LEFT JOIN gold.events_hourly e ON a.bucket = e.bucket
WHERE a.bucket >= NOW() - INTERVAL '30 days'" > /dev/null

echo ""
echo "=== Handoff Verification Complete ==="
```

---

## References

- [PHASE-E-OVERVIEW.md](../specification/PHASE-E-OVERVIEW.md) - Phase E specification
- [TEST-PLAN.md](./TEST-PLAN.md) - Phase E test plan
- [SCOPE.md](../../SCOPE.md) - V1.1 scope definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions

---

*V1.2 Handoff Checklist created: 2026-02-04*
