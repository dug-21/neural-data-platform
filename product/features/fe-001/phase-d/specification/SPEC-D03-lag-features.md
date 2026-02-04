# SPEC-D03: Lag Feature Computation (v11-009)

**Feature ID**: v11-009
**Feature Name**: Lag Feature Computation
**Priority**: Medium
**Created**: 2026-02-04
**Status**: Draft

---

## 1. Overview

### 1.1 User Story

> As a **data scientist**, I want the Gold layer to provide time-lagged values of key metrics (e.g., PM2.5 at t-1h, t-6h, t-24h), so that I can build temporal models without complex self-joins.

### 1.2 Goal

Enable config-driven generation of lag features that capture historical values at fixed time offsets. These features are essential for V1.2 pattern detection (Granger causality, lead-lag analysis).

### 1.3 Scope

| In Scope | Out of Scope |
|----------|--------------|
| Fixed lag offsets (t-1h, t-6h, t-24h) | Dynamic lag windows |
| Lag features in aligned view | Lag features in per-stream aggregates |
| NULL handling for boundary conditions | Interpolation of missing values |
| Config-driven field selection | Automatic lag selection |

---

## 2. Functional Requirements

### 2.1 Lag Computation (FR-D03-LAG)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D03-LAG-001 | Compute value at t - N hours for configured fields | P0 | LAG() window function used |
| FR-D03-LAG-002 | Support multiple lag offsets per field | P0 | 1h, 6h, 24h configurable |
| FR-D03-LAG-003 | Return NULL when historical value unavailable | P0 | Edge cases handled |
| FR-D03-LAG-004 | Lag computed on hourly aggregates (not raw) | P0 | Uses Gold hourly buckets |

### 2.2 Configuration (FR-D03-CFG)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D03-CFG-001 | Support lag configuration in gold_etl.features section | P0 | Config parsed correctly |
| FR-D03-CFG-002 | Allow enabling/disabling lag features | P0 | enabled: true/false works |
| FR-D03-CFG-003 | Configure lag offsets in hours | P0 | lags_hours array supported |
| FR-D03-CFG-004 | Configure fields for lag computation | P0 | fields array supported |

### 2.3 Aligned View Integration (FR-D03-ALIGN)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D03-ALIGN-001 | Lag features included in domain aligned view | P0 | Columns present |
| FR-D03-ALIGN-002 | Lag column names follow naming convention | P0 | `{alias}_{field}_lag_{N}h` |
| FR-D03-ALIGN-003 | Lag computed within aligned view context | P1 | Not pre-computed separately |

---

## 3. Non-Functional Requirements

### 3.1 Performance (NFR-D03-PERF)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D03-PERF-001 | Lag computation overhead | < 5% additional query time | EXPLAIN ANALYZE |
| NFR-D03-PERF-002 | Aligned view with lags (30 days) | < 150ms | Query timing |
| NFR-D03-PERF-003 | Memory for window function | < 10MB additional | Query plan analysis |

### 3.2 Data Quality (NFR-D03-DQ)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D03-DQ-001 | Lag accuracy | Exact (no approximation) | Unit test |
| NFR-D03-DQ-002 | NULL for missing lags | 100% consistent | Edge case tests |
| NFR-D03-DQ-003 | Entity isolation | Lags only within same ndp_id | PARTITION BY verification |

---

## 4. Acceptance Criteria (Gherkin)

### 4.1 Basic Lag Computation

```gherkin
Feature: Lag Feature Computation

  Scenario: Compute 1-hour lag for PM2.5
    Given gold.air_quality_hourly has data for hours 10:00, 11:00, 12:00
    And the 11:00 bucket has pm25_mean = 15.5
    When lag features are computed for the 12:00 bucket
    Then indoor_pm25_lag_1h should equal 15.5

  Scenario: Compute 24-hour lag
    Given gold.air_quality_hourly has data for the last 48 hours
    And the bucket at NOW - 24h has pm25_mean = 20.0
    When lag features are computed for the current hour
    Then indoor_pm25_lag_24h should equal 20.0

  Scenario: NULL lag when history unavailable
    Given gold.air_quality_hourly has only 2 hours of data
    When lag features are computed with lags_hours: [24]
    Then indoor_pm25_lag_24h should be NULL for all rows
```

### 4.2 Entity Isolation

```gherkin
Feature: Entity-Isolated Lag Computation

  Scenario: Lags computed per entity
    Given two sensors (sensor-A, sensor-B) have data
    And at 11:00 sensor-A had pm25_mean = 10.0
    And at 11:00 sensor-B had pm25_mean = 30.0
    When lag features are computed for 12:00
    Then sensor-A's indoor_pm25_lag_1h should be 10.0
    And sensor-B's indoor_pm25_lag_1h should be 30.0
    And lags should NOT cross between sensors
```

### 4.3 Configuration

```gherkin
Feature: Lag Feature Configuration

  Scenario: Configure multiple lag offsets
    Given gold_etl config has lag.lags_hours: [1, 6, 24]
    And lag.fields: [pm25, co2]
    When aligned view is generated
    Then view should have columns:
      | indoor_pm25_lag_1h  |
      | indoor_pm25_lag_6h  |
      | indoor_pm25_lag_24h |
      | indoor_co2_lag_1h   |
      | indoor_co2_lag_6h   |
      | indoor_co2_lag_24h  |

  Scenario: Disable lag features
    Given gold_etl config has lag.enabled: false
    When aligned view is generated
    Then view should NOT have any _lag_ columns
```

---

## 5. Configuration Schema

### 5.1 Lag Features Section

```json
{
  "gold_etl": {
    "features": {
      "lag": {
        "enabled": true,
        "lags_hours": [1, 6, 24],
        "fields": ["pm25", "co2"],
        "description": "Time-lagged values for correlation analysis"
      }
    }
  }
}
```

### 5.2 Schema Definition

```json
{
  "type": "object",
  "properties": {
    "lag": {
      "type": "object",
      "properties": {
        "enabled": {
          "type": "boolean",
          "default": false,
          "description": "Enable lag feature computation"
        },
        "lags_hours": {
          "type": "array",
          "items": {
            "type": "integer",
            "minimum": 1,
            "maximum": 168
          },
          "default": [1, 6, 24],
          "description": "Lag offsets in hours"
        },
        "fields": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Fields to compute lags for"
        }
      },
      "required": ["enabled"]
    }
  }
}
```

---

## 6. Generated SQL Examples

### 6.1 Lag Computation in Aligned View

```sql
-- Aligned view with lag features
CREATE VIEW gold.indoor_air_quality_aligned AS
WITH base AS (
    SELECT
        COALESCE(aq.bucket, ow.bucket, se.bucket) AS bucket,

        -- Indoor air quality
        aq.ndp_id AS indoor_ndp_id,
        aq.pm25_mean AS indoor_pm25,
        aq.co2_mean AS indoor_co2,

        -- Outdoor weather
        ow.temp_mean AS outdoor_temp,

        -- State events
        se.window_state

    FROM gold.air_quality_hourly aq
    FULL OUTER JOIN gold.outdoor_weather_hourly ow ON aq.bucket = ow.bucket
    FULL OUTER JOIN gold.state_events_hourly se ON aq.bucket = se.bucket
)
SELECT
    bucket,
    indoor_ndp_id,
    indoor_pm25,
    indoor_co2,
    outdoor_temp,
    window_state,

    -- Lag features for indoor PM2.5
    LAG(indoor_pm25, 1) OVER w AS indoor_pm25_lag_1h,
    LAG(indoor_pm25, 6) OVER w AS indoor_pm25_lag_6h,
    LAG(indoor_pm25, 24) OVER w AS indoor_pm25_lag_24h,

    -- Lag features for indoor CO2
    LAG(indoor_co2, 1) OVER w AS indoor_co2_lag_1h,
    LAG(indoor_co2, 6) OVER w AS indoor_co2_lag_6h,
    LAG(indoor_co2, 24) OVER w AS indoor_co2_lag_24h

FROM base
WINDOW w AS (PARTITION BY indoor_ndp_id ORDER BY bucket);
```

### 6.2 Lag Offset Calculation

For hourly buckets, the LAG offset N corresponds directly to hours:

| lags_hours | LAG Offset | Meaning |
|------------|------------|---------|
| 1 | `LAG(field, 1)` | 1 hour ago |
| 6 | `LAG(field, 6)` | 6 hours ago |
| 24 | `LAG(field, 24)` | 24 hours (1 day) ago |
| 168 | `LAG(field, 168)` | 168 hours (1 week) ago |

### 6.3 Handling Missing Buckets

If hourly buckets have gaps (missing data), LAG still works correctly:

```sql
-- LAG with explicit NULL handling
LAG(indoor_pm25, 1) OVER (
    PARTITION BY indoor_ndp_id
    ORDER BY bucket
    -- Note: No ROWS BETWEEN needed; gaps are preserved
) AS indoor_pm25_lag_1h
```

**Important**: If bucket 11:00 is missing, the lag for 12:00 will be from 10:00, not NULL. This is the expected SQL LAG behavior. For strict "exactly N hours ago" semantics, we would need a different approach (self-join).

---

## 7. Column Naming Convention

### 7.1 Pattern

```
{stream_alias}_{field}_lag_{N}h
```

### 7.2 Examples

| Stream Alias | Field | Lag Hours | Column Name |
|--------------|-------|-----------|-------------|
| indoor | pm25 | 1 | `indoor_pm25_lag_1h` |
| indoor | pm25 | 6 | `indoor_pm25_lag_6h` |
| indoor | pm25 | 24 | `indoor_pm25_lag_24h` |
| indoor | co2 | 1 | `indoor_co2_lag_1h` |
| outdoor | temp | 6 | `outdoor_temp_lag_6h` |
| outdoor_aqi | pm25 | 24 | `outdoor_aqi_pm25_lag_24h` |

---

## 8. Edge Cases

### 8.1 Boundary Conditions

| Condition | Behavior | Example |
|-----------|----------|---------|
| First N rows | LAG returns NULL | First 24 rows have NULL for lag_24h |
| Missing bucket | LAG skips to previous existing | Gap at 11:00 means 12:00 lag is from 10:00 |
| New entity | LAG resets per partition | New sensor starts with NULL lags |
| No data | All lags NULL | Empty period produces NULL features |

### 8.2 NULL Propagation

```sql
-- If indoor_pm25 is NULL at t-1h, the lag is also NULL
LAG(indoor_pm25, 1) OVER w  -- Returns NULL if source value was NULL
```

This is expected behavior. Pattern detection algorithms must handle NULL lag values.

---

## 9. London TDD Interfaces

### 9.1 Interface: Lag Feature Generator

```rust
/// Generate lag feature columns for aligned view
pub struct LagFeatureGenerator;

impl LagFeatureGenerator {
    pub fn new() -> Self;

    /// Generate LAG column expressions for a stream
    pub fn generate_columns(
        &self,
        stream_alias: &str,
        config: &LagConfig
    ) -> Result<Vec<String>, GeneratorError>;

    /// Generate WINDOW clause for lag computation
    pub fn generate_window_clause(
        &self,
        partition_column: &str
    ) -> String;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_generates_lag_columns() {
        // GIVEN lag config
        let config = LagConfig {
            enabled: true,
            lags_hours: vec![1, 6, 24],
            fields: vec!["pm25".into(), "co2".into()],
        };

        let generator = LagFeatureGenerator::new();

        // WHEN generating columns
        let columns = generator.generate_columns("indoor", &config).unwrap();

        // THEN 6 lag columns generated (2 fields x 3 lags)
        assert_eq!(columns.len(), 6);
        assert!(columns.contains(&"LAG(indoor_pm25, 1) OVER w AS indoor_pm25_lag_1h".into()));
        assert!(columns.contains(&"LAG(indoor_co2, 24) OVER w AS indoor_co2_lag_24h".into()));
    }

    #[test]
    fn test_disabled_returns_empty() {
        let config = LagConfig {
            enabled: false,
            ..Default::default()
        };

        let generator = LagFeatureGenerator::new();
        let columns = generator.generate_columns("indoor", &config).unwrap();

        assert!(columns.is_empty());
    }

    #[test]
    fn test_window_clause_partitions_by_entity() {
        let generator = LagFeatureGenerator::new();
        let clause = generator.generate_window_clause("indoor_ndp_id");

        assert!(clause.contains("PARTITION BY indoor_ndp_id"));
        assert!(clause.contains("ORDER BY bucket"));
    }
}
```

### 9.2 Interface: Aligned View with Lags

```rust
/// Integration test for aligned view with lag features
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_aligned_view_includes_lag_columns() {
        // GIVEN domain config with lag features enabled
        let domain = DomainConfig {
            streams: vec![
                StreamRef { stream_id: "air-quality".into(), alias: "indoor".into() },
            ],
            ..Default::default()
        };

        // AND stream config with lag features
        let stream_config = GoldEtlConfig {
            features: Features {
                lag: Some(LagConfig {
                    enabled: true,
                    lags_hours: vec![1, 24],
                    fields: vec!["pm25".into()],
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let generator = AlignedViewGenerator::new();

        // WHEN generating aligned view
        let sql = generator.generate(&domain, &[stream_config]).unwrap();

        // THEN SQL includes lag columns
        assert!(sql.contains("indoor_pm25_lag_1h"));
        assert!(sql.contains("indoor_pm25_lag_24h"));
        assert!(sql.contains("PARTITION BY"));
        assert!(sql.contains("ORDER BY bucket"));
    }
}
```

---

## 10. Performance Considerations

### 10.1 Query Plan Impact

LAG window functions add minimal overhead when:
- Partition column (ndp_id) is indexed
- ORDER BY column (bucket) is the time index
- Window frame is default (unbounded preceding)

### 10.2 Materialization Strategy

**Option A: Compute lags in view (recommended for V1.1)**
- Lags computed at query time
- Always up-to-date
- Slight query overhead

**Option B: Pre-compute lags in continuous aggregate (future)**
- Faster queries
- Requires continuous aggregate refresh
- More complex DDL

V1.1 uses Option A. If performance becomes an issue, Option B can be evaluated for V1.2.

---

## 11. Future Considerations

### 11.1 Rolling Window Features (v11-009 extension)

Rolling windows (e.g., rolling 4-hour mean) are related but distinct:

```json
{
  "features": {
    "rolling": {
      "enabled": true,
      "windows": ["4 hours", "24 hours"],
      "stats": ["mean", "std"],
      "fields": ["pm25"]
    }
  }
}
```

Rolling features use different SQL (frame specification):

```sql
AVG(indoor_pm25) OVER (
    PARTITION BY indoor_ndp_id
    ORDER BY bucket
    ROWS BETWEEN 3 PRECEDING AND CURRENT ROW  -- 4-hour rolling
) AS indoor_pm25_roll_4h_mean
```

### 11.2 Strict Time-Offset Lags

If "exactly 24 hours ago" semantics are needed (vs "24 buckets ago"):

```sql
-- Self-join approach for strict offset
SELECT
    current.bucket,
    current.indoor_pm25,
    lagged.indoor_pm25 AS indoor_pm25_lag_24h
FROM aligned_view current
LEFT JOIN aligned_view lagged
    ON lagged.bucket = current.bucket - INTERVAL '24 hours'
    AND lagged.indoor_ndp_id = current.indoor_ndp_id
```

This is more expensive but handles missing buckets differently. Deferred to V1.2 if needed.

---

## 12. References

- [SCOPE.md](../../SCOPE.md) - Feature v11-009 definition
- [PostgreSQL Window Functions](https://www.postgresql.org/docs/current/tutorial-window.html)
- [LAG Function Documentation](https://www.postgresql.org/docs/current/functions-window.html)

---

*Specification created: 2026-02-04*
