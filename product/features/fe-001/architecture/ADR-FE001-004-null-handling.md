# ADR-FE001-004: NULL Handling by Stream Type

**Status**: Accepted
**Date**: 2026-02-04
**Decision Makers**: NDP Architecture Team
**Feature**: FE-001 Gold Layer Foundation
**Parent ADRs**: ADR-FE001-002 (Domain-Centric Config), ADR-FE001-003 (Forecast Alignment)

---

## Context

### The Problem

Aligned views use FULL OUTER JOIN across streams, which produces NULLs where a stream has no data for a given time bucket:

```sql
-- Aligned view result
bucket              | indoor_pm25 | outdoor_temp | window_state
--------------------|-------------|--------------|-------------
2026-02-04 10:00    | 12.5        | 65.0         | closed
2026-02-04 11:00    | 15.2        | NULL         | NULL        -- sensor offline for outdoor, no state change
2026-02-04 12:00    | NULL        | 68.0         | open        -- indoor sensor gap
```

The question: How should NULLs be handled in the aligned view?

### Why This Matters for Correlation

V1.2 will compute correlations between streams. NULL handling affects correlation validity:

| Handling Strategy | Effect on Correlation |
|-------------------|----------------------|
| **Preserve NULL** | Correlation algorithms skip NULL pairs; reports "coverage %" |
| **Fill with Zero** | Fabricates data; can create false correlations |
| **Interpolate** | Fabricates data; synthetic values may correlate falsely |
| **Carry Forward (LOCF)** | Extends last known value; appropriate for some data types |

### Different Data Types Need Different Handling

Not all data is semantically equivalent:

| Data Type | NULL Meaning | Appropriate Handling |
|-----------|--------------|---------------------|
| **Sensor Reading** | Sensor was offline; we don't know the value | Preserve NULL |
| **State** | No state change; previous state still applies | Carry Forward |
| **Forecast** | No forecast available at that time | Preserve NULL |
| **Dimension** | Dimension unchanged; previous value applies | Carry Forward |

---

## Decision

**NULL handling depends on `stream_type`. The strategy is applied per-stream in the aligned view.**

### Strategy by Stream Type

| Stream Type | Strategy | Rationale |
|-------------|----------|-----------|
| `observation` | **Preserve NULL** | Missing sensor reading is not zero. Don't fabricate data. |
| `state_event` | **Carry Forward (LOCF)** | State persists until changed. Last known state IS current state. |
| `forecast` | **Preserve NULL** | If no forecast available, don't pretend there was one. |
| `dimension` | **Carry Forward** | Dimensions are slow-changing. Last value remains valid. |

### Implementation in Aligned View

```sql
CREATE VIEW gold.indoor_air_quality_aligned AS
SELECT
    bucket,

    -- Observations: preserve NULL (honest representation)
    aq.pm25_mean AS indoor_pm25,
    aq.co2_mean AS indoor_co2,
    ow.temp_mean AS outdoor_temp,

    -- State Events: carry forward (state persists until changed)
    COALESCE(
        se.window_state,
        LAG(se.window_state) IGNORE NULLS OVER (ORDER BY bucket)
    ) AS window_state,

    COALESCE(
        se.hvac_mode,
        LAG(se.hvac_mode) IGNORE NULLS OVER (ORDER BY bucket)
    ) AS hvac_mode,

    -- Forecasts: preserve NULL (don't fabricate predictions)
    f.temp_forecast,
    f.precip_probability

FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_weather_hourly ow ON aq.bucket = ow.bucket
FULL OUTER JOIN gold.state_events_hourly se ON aq.bucket = se.bucket
LEFT JOIN LATERAL (...) f ON TRUE;
```

### The Carry Forward Pattern

TimescaleDB/PostgreSQL supports `LAG() IGNORE NULLS`:

```sql
-- For state_event columns:
COALESCE(
    current_value,
    LAG(current_value) IGNORE NULLS OVER (ORDER BY bucket)
) AS filled_value
```

This means:
1. If current bucket has a value, use it
2. If current bucket is NULL, find the most recent non-NULL value
3. If no prior value exists, result is NULL (first bucket problem)

---

## Consequences

### Positive

1. **Data Integrity** - Observations aren't fabricated; NULLs honestly represent missing data
2. **State Correctness** - State columns reflect actual system state (persists until changed)
3. **Correlation Validity** - Analysts can trust that non-NULL values are real measurements
4. **Coverage Metrics** - Can report "85% coverage" based on NULL ratio
5. **Semantic Correctness** - Each stream type handled according to its meaning
6. **Consistent Pattern** - Same handling for all streams of same type

### Negative

1. **NULL-Aware Algorithms Required** - Correlation algorithms must handle NULLs gracefully
2. **Query Complexity** - Carry forward adds window functions to view definition
3. **Performance** - Window functions have computational cost
4. **First Bucket Problem** - If no prior state exists, LOCF returns NULL

### Neutral

1. **Configurable Per-Domain** - Domain config could override strategy if needed (future extension)
2. **View Materialization** - Aligned views may need to be materialized for performance

---

## Alternatives Considered

### Alternative 1: Always Preserve NULL

Preserve NULLs for all stream types, including state events.

**Rejected because:**
- State events semantically persist until changed
- A NULL for "window_state" at 11:00 doesn't mean "unknown" - it means "no change from 10:00"
- Would require analysts to manually implement LOCF every time they query state

### Alternative 2: Always Interpolate

Use linear interpolation for all NULLs.

**Rejected because:**
- Fabricates data points that weren't measured
- Can create false correlations (interpolated values may appear correlated)
- Not appropriate for discrete states (can't interpolate between "open" and "closed")
- Correlation analysis should know data was missing, not use synthetic values

### Alternative 3: Fill with Default Value

Fill NULLs with type-specific defaults (0 for numbers, empty string for text).

**Rejected because:**
- 0 is a valid measurement (0 ppm CO2 is meaningful, different from "unknown")
- Creates false signal in correlation (zero values correlate differently than missing)
- Loses information about data availability

### Alternative 4: User-Configurable Per-Column

Let config specify NULL handling per column.

**Rejected because:**
- Adds significant complexity to config
- Stream type semantically implies the right strategy
- Most use cases follow stream type convention
- Can add per-column override as future extension if needed

---

## Implementation

### ndp-gold-ddl Generator Logic

```rust
// In generators/aligned_view.rs
fn generate_column_expression(
    column: &ColumnDef,
    stream: &StreamRef,
    alias: &str,
) -> String {
    let base_expr = format!("{}.{}", alias, column.name);

    match stream.stream_type {
        StreamType::Observation | StreamType::Forecast => {
            // Preserve NULL
            base_expr
        }
        StreamType::StateEvent | StreamType::Dimension => {
            // Carry forward (LOCF)
            format!(
                "COALESCE({base}, LAG({base}) IGNORE NULLS OVER (ORDER BY bucket)) AS {name}",
                base = base_expr,
                name = column.name
            )
        }
    }
}
```

### Validation Rules

No specific validation rules needed - stream_type determines strategy automatically.

### V1.2 Correlation Implications

The V1.2 correlation engine must:

1. **Calculate Coverage** - Report percentage of time buckets with non-NULL values for each column pair
2. **Skip NULL Pairs** - Correlation algorithms should skip rows where either value is NULL
3. **Document Gaps** - Correlation results should include data coverage metrics

```python
# Future V1.2 correlation pattern
def correlate(df, col_a, col_b):
    # Drop rows where either is NULL
    valid_rows = df.dropna(subset=[col_a, col_b])
    coverage = len(valid_rows) / len(df)
    correlation = valid_rows[col_a].corr(valid_rows[col_b])
    return {
        'correlation': correlation,
        'coverage': coverage,
        'n_samples': len(valid_rows)
    }
```

---

## Related Decisions

- **Decision 10 (DECISIONS.md)**: NULL Handling in Aligned View by Stream Type - source decision
- **ADR-FE001-003**: Forecast Alignment - forecast stream handling
- **ADR-FE001-002**: Domain-Centric Configuration - where alignment is configured
- **Decision 8 (DECISIONS.md)**: Forecast alignment uses issued_at

---

## References

- `/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md` - Source decision (Decision 10)
- PostgreSQL documentation: Window functions with IGNORE NULLS
- TimescaleDB documentation: Time-series gap filling
- Statistical best practices: Handling missing data in correlation analysis

---

*Architecture decision created: 2026-02-04*
*Feature: FE-001 Gold Layer Foundation*
