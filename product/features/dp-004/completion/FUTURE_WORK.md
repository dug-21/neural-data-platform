# DP-004: Future Work

## Overview

This document captures work that is explicitly out of scope for dp-004 but is required for the complete Bronze/Silver/Gold architecture. These items should be tracked as separate features.

---

## Immediate Follow-On Features

### dp-005: Silver Layer ETL Pipeline

**Priority**: High
**Dependency**: dp-004 (Bronze Raw JSON Schema)
**Estimated Effort**: Large (2-3 weeks)

#### Scope

Build the ETL pipeline that transforms raw JSON from Bronze into typed, queryable data in Silver (TimescaleDB).

#### Key Components

1. **ETL Scheduler**
   - Batch or streaming approach decision
   - Configurable extraction schedules
   - Backfill capability for historical data

2. **Transformation Rules Engine**
   - JSON path extraction configuration
   - Type coercion (string -> numeric, etc.)
   - Field mapping per source type

3. **Silver Schema Design**
   - Extend existing TimescaleDB schema
   - Handle numeric, text, and event data types
   - Continuous aggregates for common queries

4. **Monitoring & Alerting**
   - ETL job success/failure tracking
   - Data freshness monitoring
   - Schema drift detection

#### Example Transformation

```yaml
# config/etl/airgradient.yaml
source_filter:
  source_id: "air-quality-Mqtt"

extractions:
  - target_table: silver.readings
    fields:
      - source_path: "$.pm02"
        target_column: value
        type: float
        metric_name: "pm02"
      - source_path: "$.rco2"
        target_column: value
        type: float
        metric_name: "rco2"
      - source_path: "$.atmp"
        target_column: value
        type: float
        metric_name: "atmp"
    key_fields:
      ndp_id: from_context
      location_id: "$.serialno"
```

#### ADR Topics

- [ ] Batch vs streaming ETL decision
- [ ] Error handling and dead letter queue
- [ ] Schema evolution strategy
- [ ] Backfill approach for existing data

---

### dp-006: Grafana Query Migration

**Priority**: Medium
**Dependency**: dp-005 (Silver ETL)
**Estimated Effort**: Medium (1-2 weeks)

#### Scope

Update Grafana dashboards to query Silver layer (TimescaleDB) instead of Bronze (Parquet/DuckDB).

#### Key Tasks

1. **Query Migration**
   - Identify all existing dashboard queries
   - Rewrite for TimescaleDB syntax
   - Test query performance

2. **Datasource Configuration**
   - Add TimescaleDB datasource to Grafana
   - Configure connection pooling
   - Set up query caching

3. **Dashboard Updates**
   - Update variable definitions
   - Adjust time range handling
   - Verify visualization compatibility

4. **Bronze Query Preservation**
   - Keep DuckDB datasource for debugging
   - Create "Raw Data Explorer" dashboard
   - Document when to use Bronze vs Silver

#### Migration Checklist

| Dashboard | Current Datasource | Target Datasource | Status |
|-----------|-------------------|-------------------|--------|
| Air Quality Overview | DuckDB | TimescaleDB | Pending |
| Room Comparison | DuckDB | TimescaleDB | Pending |
| Sensor Health | DuckDB | TimescaleDB | Pending |
| Raw Data Debug | DuckDB | DuckDB (keep) | N/A |

---

## Medium-Term Features

### dp-007: Historical Data Migration

**Priority**: Low (after dp-005 stable)
**Dependency**: dp-005 (Silver ETL)
**Estimated Effort**: Medium (1-2 weeks)

#### Scope

Migrate historical Bronze data (v1 schema) to Silver layer using new ETL pipeline.

#### Key Considerations

1. **Data Volume**
   - Estimate total historical records
   - Plan for incremental processing
   - Monitor storage during migration

2. **Validation**
   - Compare record counts pre/post migration
   - Spot-check value accuracy
   - Verify time range coverage

3. **Cleanup**
   - Archive v1 Parquet files after migration
   - Update backup procedures
   - Document what was migrated

#### Migration Strategy

```
Phase 1: Inventory
- Count records per source in v1 files
- Identify date ranges
- Estimate processing time

Phase 2: Pilot Migration
- Migrate 1 week of data
- Validate in Silver
- Adjust ETL rules if needed

Phase 3: Full Migration
- Process all historical data
- Run in parallel with live pipeline
- Monitor system resources

Phase 4: Verification
- Full count comparison
- Random sampling validation
- Dashboard verification

Phase 5: Cleanup
- Archive v1 files
- Update documentation
- Remove dual-read logic
```

---

### dp-008: Schema Registry

**Priority**: Medium
**Dependency**: dp-005
**Estimated Effort**: Medium (1-2 weeks)

#### Scope

Implement schema versioning and evolution tracking for ETL transformations.

#### Features

1. **Schema Versioning**
   - Track transformation rule versions
   - Link Bronze records to ETL version that processed them
   - Enable reprocessing with specific versions

2. **Schema Evolution**
   - Detect when source payloads change
   - Alert on new/removed fields
   - Suggest transformation updates

3. **Compatibility Checking**
   - Validate ETL rules against sample data
   - Test backward/forward compatibility
   - Dry-run transformations

---

## Long-Term Features

### fe-001: Feature Engineering Layer (Gold)

**Priority**: Low (after dp-005, dp-006)
**Dependency**: Silver layer populated
**Estimated Effort**: Large

#### Scope

Build aggregated features for ML model training and inference.

#### Example Features

| Feature | Input | Output | Window |
|---------|-------|--------|--------|
| pm25_hourly_avg | silver.readings | gold.features | 1 hour |
| co2_daily_max | silver.readings | gold.features | 1 day |
| window_open_pct | silver.events | gold.features | 1 day |
| temp_volatility | silver.readings | gold.features | 6 hours |

---

### ml-001: Air Quality Prediction

**Priority**: Future
**Dependency**: fe-001 (Feature Engineering)
**Estimated Effort**: Large

#### Scope

Train and deploy predictive models for air quality forecasting.

---

## Technical Debt

### TD-001: Parser Simplification

**Dependency**: dp-004 complete
**Effort**: Small

After dp-004, parsers no longer need to extract individual fields. Simplify to:
- Extract timestamp (if present in payload)
- Pass through raw JSON unchanged
- Remove field mapping logic

### TD-002: Legacy Schema Removal

**Dependency**: dp-007 (Historical Migration) complete
**Effort**: Small

Once all data migrated to v2 schema:
- Remove v1 schema support from Parquet reader
- Remove dual-write capability
- Simplify storage code

### TD-003: Test Fixture Updates

**Dependency**: dp-004 complete
**Effort**: Small

Update all test fixtures to use new v2 schema format:
- Update sample Parquet files
- Update mock server responses
- Update integration test expectations

---

## Decision Log

Decisions deferred from dp-004:

| Decision | Deferred To | Reason |
|----------|-------------|--------|
| Batch vs streaming ETL | dp-005 | Depends on Silver requirements |
| TimescaleDB vs alternatives | dp-005 | Already decided in dp-002 |
| Grafana query syntax | dp-006 | After Silver data available |
| Historical data handling | dp-007 | Non-urgent, can be done later |

---

## Dependencies Graph

```
dp-004 (Bronze Raw JSON)
    |
    v
dp-005 (Silver ETL) -----> dp-007 (Historical Migration)
    |                           |
    v                           v
dp-006 (Grafana Migration) --> TD-002 (Legacy Removal)
    |
    v
fe-001 (Gold Layer)
    |
    v
ml-001 (Predictions)
```

---

## Resource Estimates

| Feature | Developer Days | Priority | Quarter |
|---------|---------------|----------|---------|
| dp-005 | 10-15 | High | Q1 2026 |
| dp-006 | 5-10 | Medium | Q1 2026 |
| dp-007 | 5-10 | Low | Q2 2026 |
| dp-008 | 5-10 | Medium | Q2 2026 |
| fe-001 | 15-20 | Low | Q2-Q3 2026 |
| ml-001 | 20+ | Future | Q3-Q4 2026 |

---

## References

- [ADR-001: Bronze Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)
- [ADR-001 (dp-002): TimescaleDB Schema](../../dp-002/architecture/ADR-001-TIMESCALEDB-SCHEMA.md)
- [Medallion Architecture Overview](https://docs.databricks.com/lakehouse-architecture/medallion.html)

---

## Document Control

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-01 | ndp-scrum-master | Initial draft |
