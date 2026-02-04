# SPEC-D02: Basic Feature Computation (v11-008)

**Feature ID**: v11-008
**Feature Name**: Basic Feature Computation
**Priority**: Medium
**Created**: 2026-02-04
**Status**: Draft

---

## 1. Overview

### 1.1 User Story

> As a **data scientist**, I want the Gold layer to compute standard aggregate statistics (mean, std, min, max, count, percentiles) for each configured field, so that I can analyze time-series patterns without writing custom SQL.

### 1.2 Goal

Provide config-driven computation of basic aggregate features for Gold layer continuous aggregates. These features are the building blocks for V1.2 pattern detection.

### 1.3 Scope

| In Scope | Out of Scope |
|----------|--------------|
| Mean, std, min, max, count | Linear regression (v11-trend) |
| Percentiles (p95, p99) | Lag features (v11-009) |
| Config-driven field selection | Rolling windows (v11-009) |
| Multiple granularities | Cross-stream features |

---

## 2. Functional Requirements

### 2.1 Aggregate Metrics (FR-D02-AGG)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D02-AGG-001 | Compute arithmetic mean for numeric fields | P0 | `AVG()` used; NULL-safe |
| FR-D02-AGG-002 | Compute standard deviation for numeric fields | P0 | `STDDEV()` used; NULL-safe |
| FR-D02-AGG-003 | Compute minimum value for numeric fields | P0 | `MIN()` used |
| FR-D02-AGG-004 | Compute maximum value for numeric fields | P0 | `MAX()` used |
| FR-D02-AGG-005 | Compute sample count per bucket | P0 | `COUNT(*)` or `COUNT(field)` |
| FR-D02-AGG-006 | Compute p95 percentile for numeric fields | P1 | `PERCENTILE_CONT(0.95)` used |
| FR-D02-AGG-007 | Compute p99 percentile for numeric fields | P1 | `PERCENTILE_CONT(0.99)` used |

### 2.2 Configuration (FR-D02-CFG)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D02-CFG-001 | Support default_metrics for all fields | P0 | Config parsed correctly |
| FR-D02-CFG-002 | Support per-field metric override | P0 | Field-specific metrics used |
| FR-D02-CFG-003 | Support multiple granularities | P1 | Separate views per granularity |
| FR-D02-CFG-004 | Validate metric names against allowed list | P0 | Invalid metrics rejected |

### 2.3 SQL Generation (FR-D02-SQL)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D02-SQL-001 | Generate valid TimescaleDB continuous aggregate SQL | P0 | SQL executes without error |
| FR-D02-SQL-002 | Use time_bucket() for temporal grouping | P0 | Correct interval used |
| FR-D02-SQL-003 | Include ndp_id in GROUP BY | P0 | Entity-level aggregation |
| FR-D02-SQL-004 | Handle NULL values appropriately | P0 | NULLs excluded from aggregates |

---

## 3. Non-Functional Requirements

### 3.1 Performance (NFR-D02-PERF)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D02-PERF-001 | Aggregate computation per bucket | < 10ms | Query plan analysis |
| NFR-D02-PERF-002 | Refresh policy execution | < 30 seconds | TimescaleDB job metrics |
| NFR-D02-PERF-003 | Memory during percentile computation | < 50MB | Pi monitoring |

### 3.2 Accuracy (NFR-D02-ACC)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D02-ACC-001 | Mean computation | Exact (no approximation) | Unit test comparison |
| NFR-D02-ACC-002 | Std deviation | Sample stddev (n-1) | Verify formula |
| NFR-D02-ACC-003 | Percentile computation | Linear interpolation | Verify method |

---

## 4. Acceptance Criteria (Gherkin)

### 4.1 Basic Aggregates

```gherkin
Feature: Basic Aggregate Computation

  Scenario: Compute mean for PM2.5
    Given air-quality Silver table has data for the last hour
    And gold_etl config specifies mean metric for pm25 field
    When the continuous aggregate refreshes
    Then gold.air_quality_hourly should have pm25_mean column
    And pm25_mean should equal AVG(pm25) from Silver for that hour

  Scenario: Compute standard deviation
    Given air-quality Silver table has data with variance
    And gold_etl config specifies std metric for pm25 field
    When the continuous aggregate refreshes
    Then gold.air_quality_hourly should have pm25_std column
    And pm25_std should be the sample standard deviation

  Scenario: Compute percentiles
    Given air-quality Silver table has at least 100 samples in one hour
    And gold_etl config specifies p95 and p99 metrics for pm25 field
    When the continuous aggregate refreshes
    Then gold.air_quality_hourly should have pm25_p95 and pm25_p99 columns
    And pm25_p95 should be less than or equal to pm25_p99
```

### 4.2 Configuration

```gherkin
Feature: Feature Configuration

  Scenario: Use default metrics for all fields
    Given gold_etl config has default_metrics: [mean, std, min, max, count]
    And fields section lists pm25 and co2 without per-field metrics
    When DDL is generated
    Then both pm25 and co2 should have all 5 default metrics

  Scenario: Override metrics for specific field
    Given gold_etl config has default_metrics: [mean, std]
    And pm25 field specifies metrics: [mean, std, min, max, p95]
    And co2 field has no per-field metrics
    When DDL is generated
    Then pm25 should have 5 metrics (override)
    And co2 should have 2 metrics (default)

  Scenario: Reject invalid metric name
    Given gold_etl config specifies metric "average" (invalid)
    When config validation runs
    Then validation should fail with helpful error message
    And error should list valid metric names
```

### 4.3 NULL Handling

```gherkin
Feature: NULL Handling in Aggregates

  Scenario: NULL values excluded from mean
    Given Silver table has 10 rows for one hour
    And 3 of those rows have NULL pm25
    When continuous aggregate computes pm25_mean
    Then pm25_mean should be average of 7 non-NULL values
    And sample_count should indicate total rows (10)

  Scenario: All NULL values in bucket
    Given Silver table has 5 rows for one hour
    And all 5 rows have NULL pm25
    When continuous aggregate computes pm25_mean
    Then pm25_mean should be NULL
    And pm25_std should be NULL
    And sample_count should be 5
```

---

## 5. Configuration Schema

### 5.1 Aggregates Section

```json
{
  "gold_etl": {
    "enabled": true,
    "aggregates": {
      "granularities": ["1 hour", "1 day"],
      "default_metrics": ["mean", "std", "min", "max", "count"],
      "fields": {
        "pm25": {
          "metrics": ["mean", "std", "min", "max", "p95", "p99"],
          "description": "PM2.5 with percentiles for outlier detection"
        },
        "co2": {
          "metrics": ["mean", "std", "min", "max"],
          "description": "CO2 standard aggregates"
        },
        "temperature_c": {
          "description": "Uses default metrics"
        }
      }
    }
  }
}
```

### 5.2 Allowed Metric Values

| Metric | SQL Function | Description |
|--------|--------------|-------------|
| `mean` | `AVG(field)` | Arithmetic mean |
| `std` | `STDDEV(field)` | Sample standard deviation |
| `min` | `MIN(field)` | Minimum value |
| `max` | `MAX(field)` | Maximum value |
| `count` | `COUNT(field)` | Non-NULL count for specific field |
| `count_all` | `COUNT(*)` | Total row count |
| `p95` | `PERCENTILE_CONT(0.95)` | 95th percentile |
| `p99` | `PERCENTILE_CONT(0.99)` | 99th percentile |
| `sum` | `SUM(field)` | Sum (for additive metrics) |

---

## 6. Generated SQL Examples

### 6.1 Basic Continuous Aggregate

```sql
-- Generated from gold_etl config
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,

    -- PM2.5 aggregates (with percentiles)
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    MIN(pm25) AS pm25_min,
    MAX(pm25) AS pm25_max,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) AS pm25_p95,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY pm25) AS pm25_p99,

    -- CO2 aggregates (default metrics)
    AVG(co2) AS co2_mean,
    STDDEV(co2) AS co2_std,
    MIN(co2) AS co2_min,
    MAX(co2) AS co2_max,

    -- Temperature (default metrics)
    AVG(temperature_c) AS temperature_c_mean,
    STDDEV(temperature_c) AS temperature_c_std,
    MIN(temperature_c) AS temperature_c_min,
    MAX(temperature_c) AS temperature_c_max,

    -- Sample count
    COUNT(*) AS sample_count

FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;
```

### 6.2 Refresh Policy

```sql
-- Add continuous aggregate policy
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

### 6.3 Multiple Granularities

```sql
-- Hourly aggregate (primary)
CREATE MATERIALIZED VIEW gold.air_quality_hourly ...

-- Daily aggregate (rollup)
CREATE MATERIALIZED VIEW gold.air_quality_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', observation_time) AS bucket,
    ndp_id,
    AVG(pm25) AS pm25_mean,
    ...
FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;
```

---

## 7. Column Naming Convention

### 7.1 Pattern

```
{field}_{metric}
```

### 7.2 Examples

| Field | Metric | Column Name |
|-------|--------|-------------|
| pm25 | mean | `pm25_mean` |
| pm25 | std | `pm25_std` |
| pm25 | p95 | `pm25_p95` |
| co2 | mean | `co2_mean` |
| temperature_c | min | `temperature_c_min` |
| (n/a) | count_all | `sample_count` |

### 7.3 Reserved Column Names

| Column | Purpose |
|--------|---------|
| `bucket` | Time bucket (from time_bucket()) |
| `ndp_id` | Entity identifier |
| `sample_count` | Total samples in bucket |

---

## 8. London TDD Interfaces

### 8.1 Interface: Metric Registry

```rust
/// Registry of supported aggregate metrics
pub struct MetricRegistry {
    metrics: HashMap<String, MetricDefinition>,
}

pub struct MetricDefinition {
    pub name: String,
    pub sql_template: String,  // e.g., "AVG({field})"
    pub description: String,
    pub requires_ordered: bool, // True for percentiles
}

impl MetricRegistry {
    pub fn default() -> Self;
    pub fn get(&self, name: &str) -> Option<&MetricDefinition>;
    pub fn is_valid(&self, name: &str) -> bool;
    pub fn generate_sql(&self, metric: &str, field: &str) -> Result<String, MetricError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_mean_metric_generates_avg() {
        let registry = MetricRegistry::default();
        let sql = registry.generate_sql("mean", "pm25").unwrap();
        assert_eq!(sql, "AVG(pm25) AS pm25_mean");
    }

    #[test]
    fn test_p95_metric_generates_percentile() {
        let registry = MetricRegistry::default();
        let sql = registry.generate_sql("p95", "pm25").unwrap();
        assert!(sql.contains("PERCENTILE_CONT(0.95)"));
        assert!(sql.contains("ORDER BY pm25"));
    }

    #[test]
    fn test_invalid_metric_rejected() {
        let registry = MetricRegistry::default();
        let result = registry.generate_sql("average", "pm25");
        assert!(result.is_err());
    }
}
```

### 8.2 Interface: Aggregate Generator

```rust
/// Generate aggregate columns from gold_etl config
pub struct AggregateGenerator {
    metric_registry: MetricRegistry,
}

impl AggregateGenerator {
    pub fn new() -> Self;

    /// Generate SELECT columns for all configured fields and metrics
    pub fn generate_columns(
        &self,
        config: &GoldEtlConfig
    ) -> Result<Vec<String>, GeneratorError>;

    /// Generate complete continuous aggregate SQL
    pub fn generate_continuous_aggregate(
        &self,
        config: &GoldEtlConfig,
        stream_id: &str,
        silver_table: &str,
        granularity: &str
    ) -> Result<String, GeneratorError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_generates_columns_with_default_metrics() {
        // GIVEN config with default_metrics and fields without overrides
        let config = GoldEtlConfig {
            aggregates: Aggregates {
                default_metrics: vec!["mean".into(), "std".into()],
                fields: hashmap! {
                    "pm25".into() => FieldConfig { metrics: None, .. },
                    "co2".into() => FieldConfig { metrics: None, .. },
                },
                ..
            },
            ..
        };

        let generator = AggregateGenerator::new();
        let columns = generator.generate_columns(&config).unwrap();

        // THEN all fields get default metrics
        assert!(columns.contains(&"AVG(pm25) AS pm25_mean".into()));
        assert!(columns.contains(&"STDDEV(pm25) AS pm25_std".into()));
        assert!(columns.contains(&"AVG(co2) AS co2_mean".into()));
        assert!(columns.contains(&"STDDEV(co2) AS co2_std".into()));
    }

    #[test]
    fn test_field_metrics_override_defaults() {
        // GIVEN config with field-specific metrics
        let config = GoldEtlConfig {
            aggregates: Aggregates {
                default_metrics: vec!["mean".into()],
                fields: hashmap! {
                    "pm25".into() => FieldConfig {
                        metrics: Some(vec!["mean".into(), "p95".into()]),
                        ..
                    },
                },
                ..
            },
            ..
        };

        let generator = AggregateGenerator::new();
        let columns = generator.generate_columns(&config).unwrap();

        // THEN pm25 uses overridden metrics
        assert!(columns.contains(&"AVG(pm25) AS pm25_mean".into()));
        assert!(columns.iter().any(|c| c.contains("PERCENTILE_CONT(0.95)")));
    }
}
```

---

## 9. Error Handling

### 9.1 Validation Errors

| Error Code | Description | Resolution |
|------------|-------------|------------|
| `E-AGG-001` | Invalid metric name | Use allowed metric names |
| `E-AGG-002` | Field not in Silver schema | Check field name spelling |
| `E-AGG-003` | Invalid granularity format | Use format "N unit" (e.g., "1 hour") |
| `E-AGG-004` | Empty fields configuration | Configure at least one field |

### 9.2 Runtime Errors

| Error | Likely Cause | Resolution |
|-------|--------------|------------|
| Percentile OOM | Too many samples in bucket | Reduce bucket size or add WHERE clause |
| Aggregate returns NULL | All values in bucket are NULL | Expected behavior; handle in queries |

---

## 10. References

- [SCOPE.md](../../SCOPE.md) - Feature v11-008 definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [TimescaleDB Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)
- [PostgreSQL Aggregate Functions](https://www.postgresql.org/docs/current/functions-aggregate.html)

---

*Specification created: 2026-02-04*
