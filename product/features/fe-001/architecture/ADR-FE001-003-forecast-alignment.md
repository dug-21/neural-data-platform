# ADR-FE001-003: Forecast Streams Align on issued_at

**Status**: Accepted
**Date**: 2026-02-04
**Decision Makers**: NDP Architecture Team
**Feature**: FE-001 Gold Layer Foundation
**Parent ADRs**: ADR-FE001-002 (Domain-Centric Config)

---

## Context

### The Problem

Forecast streams (e.g., NWS weather forecasts) have two distinct timestamps:

| Timestamp | Meaning | Example |
|-----------|---------|---------|
| `issued_at` | When the forecast was published/available | 2026-02-04 10:00 |
| `valid_time` | The future hour being predicted | 2026-02-04 14:00 |

When creating aligned views that join forecasts with observations for correlation analysis, the join key determines what the data means.

### The Scenario

```
NWS Forecast issued at 10:00 AM:
  - valid_time=14:00: temp=75F
  - valid_time=15:00: temp=78F

Observation at 14:00:
  - indoor CO2 = 850 ppm
  - user opened window at 14:15
```

### Wrong Approach: Join on valid_time

```sql
-- WRONG: Joins forecast.valid_time = observation.time
SELECT
    o.time,
    o.indoor_co2,
    f.temp AS forecast_temp
FROM observations o
JOIN forecasts f ON f.valid_time = o.time
```

This shows "what was predicted FOR 14:00" - but that prediction was made hours earlier. The user at 14:00 couldn't have acted on information they received at 10:00 specifically because it predicted 14:00.

**Problem**: Cannot establish causality. User decisions at 14:00 weren't influenced by the specific `valid_time=14:00` prediction - they were influenced by whatever forecast was available at decision time.

### Correct Approach: Join on issued_at

```sql
-- CORRECT: Joins on latest available forecast at observation time
SELECT
    o.time,
    o.indoor_co2,
    f.temp AS forecast_temp  -- Temp from forecast AVAILABLE at observation time
FROM observations o
LEFT JOIN LATERAL (
    SELECT * FROM forecasts f
    WHERE f.issued_at <= o.time
    ORDER BY f.issued_at DESC
    LIMIT 1
) f ON TRUE
```

This shows "what forecast was AVAILABLE when the observation was recorded" - the information the user could have seen when making decisions.

---

## Decision

**All `forecast` type streams align on `issued_at`, not `valid_time`.**

### Stream Type Classification

Gold layer introduces `stream_type` as a required field:

| Stream Type | Alignment Key | Rationale |
|-------------|---------------|-----------|
| `observation` | `time` | Direct measurement at a point in time |
| `state_event` | `time` | State changed at a point in time |
| `forecast` | `issued_at` | Information was available at this time |
| `dimension` | N/A | Slow-changing, joined by ID not time |

### Implementation

In the aligned view DDL generation, forecast streams use LATERAL JOIN:

```sql
CREATE VIEW gold.indoor_air_quality_aligned AS
SELECT
    bucket,

    -- Observations: join on time = bucket
    aq.pm25_mean AS indoor_pm25,
    aq.co2_mean AS indoor_co2,

    -- Forecasts: join on issued_at <= bucket (most recent)
    forecast.temp_forecast,
    forecast.precip_probability

FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_weather_hourly ow ON aq.bucket = ow.bucket

-- Forecast streams use LATERAL join
LEFT JOIN LATERAL (
    SELECT
        temp AS temp_forecast,
        precip_probability
    FROM gold.nws_forecast_hourly f
    WHERE f.issued_at <= bucket
    ORDER BY f.issued_at DESC
    LIMIT 1
) forecast ON TRUE
```

### Config Expression

Domain config specifies which streams are forecasts:

```yaml
domain:
  id: indoor-air-quality
  streams:
    - stream_id: air-quality
      role: primary
      # No stream_type needed - inherited from stream config
    - stream_id: nws-forecast
      role: context
      # stream_type: forecast is in stream config
```

The `ndp-gold-ddl` tool reads `stream_type` from each referenced stream's config and generates the appropriate JOIN clause.

---

## Consequences

### Positive

1. **Causal Validity** - Aligned data represents information that was actually available at decision time
2. **V1.2 Correlation Ready** - Correlation analysis requires causal validity; this enables meaningful V1.2 pattern detection
3. **Human Intuition Match** - "What did I know when I made this decision?" is the natural question
4. **Forecast Accuracy Evaluation** - Can compare `forecast.temp` (what was predicted) with `observation.temp` (what happened)
5. **Consistent Pattern** - All forecast-type streams treated the same way automatically

### Negative

1. **Query Complexity** - LATERAL JOIN is more complex than simple equality join
2. **Performance** - LATERAL JOIN may be slower; mitigated by indexing `issued_at`
3. **Understanding Requirement** - Operators must understand the issued_at vs valid_time distinction
4. **Index Requirement** - Forecast tables need index on `issued_at` for efficient LATERAL lookups

### Neutral

1. **valid_time Still Available** - The forecast's `valid_time` is still in the data for analysis if needed
2. **Stream Config Change** - Requires `stream_type` field in stream configs (already planned)

---

## Alternatives Considered

### Alternative 1: Join on valid_time

Join forecasts on what they predicted for that time bucket.

**Rejected because:**
- Violates causality - correlates decisions with information that wasn't actionable at decision time
- Would show false correlations between "decisions at 14:00" and "prediction for 14:00 made at 10:00"
- Makes forecast evaluation impossible (can't compare prediction vs actual if joined on predicted time)

### Alternative 2: Include Both valid_time and issued_at Joins

Create two sets of columns: one joined on issued_at, one on valid_time.

**Rejected because:**
- Doubles number of forecast columns
- Confusing to analysts (which set to use?)
- Most use cases only need issued_at join
- valid_time join can be a separate ad-hoc query if needed

### Alternative 3: No Special Handling (Treat as Observation)

Join all streams on their primary timestamp regardless of type.

**Rejected because:**
- Forecasts are fundamentally different from observations
- `valid_time` is NOT when data was available; `issued_at` is
- Would require analysts to manually handle forecast joins correctly every time
- Error-prone approach that doesn't match platform's "configuration over code" philosophy

### Alternative 4: User-Configurable Join Strategy

Let domain config specify the join key per stream.

**Rejected because:**
- Adds unnecessary complexity
- `forecast` type semantically implies issued_at join
- Easy to misconfigure
- If different join strategy is ever needed, it's a new stream_type

---

## Implementation

### Stream Type Field

Add to stream config schema:

```json
{
  "stream_id": "nws-forecast",
  "stream_type": "forecast",
  "description": "NWS hourly weather forecasts",
  "fields": {
    "issued_at": { "type": "timestamp", "description": "When forecast was published" },
    "valid_time": { "type": "timestamp", "description": "Future time being predicted" },
    "temp": { "type": "float", "unit": "F" },
    "precip_probability": { "type": "float", "unit": "percent" }
  }
}
```

### Validation Rules

| Error Code | Rule |
|------------|------|
| 409 | Forecast stream missing `issued_at` field |
| 410 | Forecast stream missing `valid_time` field |

### ndp-gold-ddl Generator

```rust
// In generators/aligned_view.rs
fn generate_join_clause(stream: &StreamRef, alias: &str) -> String {
    match stream.stream_type {
        StreamType::Observation | StreamType::StateEvent => {
            // Standard time-bucket join
            format!("FULL OUTER JOIN gold.{}_hourly {} ON {}.bucket = bucket",
                stream.stream_id, alias, alias)
        }
        StreamType::Forecast => {
            // LATERAL join on issued_at
            format!(r#"
                LEFT JOIN LATERAL (
                    SELECT * FROM gold.{stream_id}_hourly f
                    WHERE f.issued_at <= bucket
                    ORDER BY f.issued_at DESC
                    LIMIT 1
                ) {alias} ON TRUE
            "#, stream_id = stream.stream_id, alias = alias)
        }
        StreamType::Dimension => {
            // Dimension joins are by ID, not time
            unimplemented!("Dimension joins handled separately")
        }
    }
}
```

### Index Requirement

For forecast tables, ensure `issued_at` is indexed:

```sql
-- Generated by ndp-gold-ddl for forecast streams
CREATE INDEX IF NOT EXISTS idx_{stream_id}_issued_at
    ON gold.{stream_id}_hourly (issued_at DESC);
```

---

## Related Decisions

- **Decision 8 (DECISIONS.md)**: Forecast Streams Align on issued_at - source decision
- **ADR-FE001-002**: Domain-Centric Configuration - domain alignment config
- **ADR-FE001-004**: NULL Handling by Stream Type - related stream_type handling
- **Decision 10 (DECISIONS.md)**: NULL Handling - affects alignment results

---

## References

- `/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md` - Source decision (Decision 8)
- `/workspaces/neural-data-platform/config/base/streams/nws-gridpoints-forecast/` - Example forecast stream
- TimescaleDB documentation: LATERAL joins for time-series correlation

---

*Architecture decision created: 2026-02-04*
*Feature: FE-001 Gold Layer Foundation*
