# ADR-001: Column-Oriented Parser Design

## Status
**Proposed** (2025-12-24)

---

## Context

The National Weather Service (NWS) raw gridpoints API uses a **column-oriented JSON structure** where each meteorological metric contains its own time-series array:

```json
{
  "properties": {
    "temperature": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2025-12-24T12:00:00+00:00/PT1H", "value": 15.5},
        {"validTime": "2025-12-24T13:00:00+00:00/PT1H", "value": 16.1}
      ]
    },
    "dewpoint": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2025-12-24T12:00:00+00:00/PT1H", "value": 8.3},
        {"validTime": "2025-12-24T13:00:00+00:00/PT1H", "value": 9.1}
      ]
    }
    // ... 38+ more metrics
  }
}
```

### Problem

Our existing parsers expect **row-oriented data**:
- **`FlatJsonParser`**: Single object with fields at same level
- **`ArrayIteratorParser`**: Array of objects, each with all metrics
- **`JsonPathParser`**: Extract specific fields from known paths

None can handle **one array per metric** with variable time intervals.

### Business Requirement

To access critical NWS fields (sky cover, visibility, wind gust, ceiling height, fire weather indices), we must parse the raw gridpoints endpoint. This data is essential for:
1. Complete weather monitoring (40+ fields vs current 12)
2. Aviation safety metrics (ceiling, visibility)
3. Fire weather prediction (indices, relative humidity)
4. Future Open-Meteo integration (similar column structure)

---

## Decision

**Create a new `ColumnOrientedParser` that:**

1. **Iterates over configured column paths** (e.g., `properties.temperature.values`)
2. **Extracts timestamp and value from each entry** in the values array
3. **Handles ISO 8601 duration format** (`2025-12-24T12:00:00+00:00/PT1H`)
4. **Produces one `TimeSeriesPoint` per value entry**
5. **Reuses existing `Parser` trait** (no new abstractions)

### Implementation Strategy

```rust
// core/src/parsers/column_oriented.rs

pub struct ColumnOrientedParser {
    config: ParserConfig,
    column_mappings: Vec<ColumnMapping>,
}

pub struct ColumnMapping {
    /// JSON path to values array (e.g., "properties.temperature.values")
    pub path: String,
    /// Metric name for TimeSeriesPoint
    pub metric_name: String,
    /// Unit for the metric
    pub unit: Option<String>,
    /// Whether this column is optional (skip if missing)
    pub optional: bool,
}

impl Parser for ColumnOrientedParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>
    {
        let mut points = Vec::new();

        for mapping in &self.column_mappings {
            // Navigate to values array
            let values_array = payload.pointer(&mapping.path)
                .and_then(|v| v.as_array());

            if values_array.is_none() {
                if mapping.optional {
                    continue; // Skip missing optional columns
                } else {
                    return Err(CoreError::ParseError(
                        format!("Required column missing: {}", mapping.path)
                    ));
                }
            }

            // Extract each (timestamp, value) pair
            for entry in values_array.unwrap() {
                let valid_time = entry["validTime"].as_str()
                    .ok_or_else(|| CoreError::ParseError("Missing validTime".into()))?;

                let timestamp = parse_iso8601_interval(valid_time)?;

                let value = entry["value"].as_f64()
                    .ok_or_else(|| CoreError::ParseError("Missing value".into()))?;

                points.push(TimeSeriesPoint {
                    timestamp,
                    location_id: self.config.default_location_id.clone().unwrap(),
                    metric_name: mapping.metric_name.clone(),
                    value,
                    unit: mapping.unit.clone(),
                    tags: self.config.default_tags.clone(),
                });
            }
        }

        Ok(points)
    }

    fn name(&self) -> &str {
        "column_oriented"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
}

/// Parse ISO 8601 interval to extract start timestamp
/// Example: "2025-12-24T12:00:00+00:00/PT1H" → 2025-12-24T12:00:00
fn parse_iso8601_interval(interval: &str) -> CoreResult<DateTime<Utc>> {
    let parts: Vec<&str> = interval.split('/').collect();
    if parts.is_empty() {
        return Err(CoreError::ParseError("Invalid interval format".into()));
    }

    DateTime::parse_from_rfc3339(parts[0])
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| CoreError::ParseError(format!("Invalid timestamp: {}", e)))
}
```

### Configuration Schema

```yaml
# In stream config
parser:
  parser_type: column_oriented
  location_id_field: properties.gridId
  default_location_id: ksgj_gridpoints
  column_config:
    columns:
      - path: properties.temperature.values
        metric_name: temperature
        unit: celsius
        optional: false
      - path: properties.dewpoint.values
        metric_name: dewpoint
        unit: celsius
        optional: true
      - path: properties.skyCover.values
        metric_name: sky_cover
        unit: percent
        optional: true
      # ... 37+ more columns
```

---

## Rationale

### 1. **Reusability**
- **Open-Meteo support**: Open-Meteo also uses column-oriented JSON (parallel arrays)
- **Generic design**: Not NWS-specific, works for any "metric → values[]" structure
- **Future-proof**: New weather APIs can use same parser

### 2. **Clean Separation of Concerns**
- **HttpPollingSource**: Handles HTTP mechanics (auth, retry, timeout)
- **ColumnOrientedParser**: Handles JSON structure transformation
- **IngestionRouter**: Handles schema validation
- Each component has single responsibility

### 3. **Configuration-Driven**
- **No code changes** to add/remove metrics
- **Stream owners** can configure fields via YAML
- **Easy testing**: Mock JSON with different column sets

### 4. **Minimal Complexity**
- **No new abstractions**: Implements existing `Parser` trait
- **Simple algorithm**: Nested iteration (columns → values → points)
- **Standard library**: Uses `serde_json` and `chrono` (already dependencies)

---

## Consequences

### Positive

✅ **Extensibility**: Easy to add new column-oriented data sources (Open-Meteo, ECMWF)
✅ **Testability**: Unit tests with sample JSON, no HTTP mocking needed
✅ **Maintainability**: Parser logic isolated in single module
✅ **Performance**: Efficient (single-pass iteration, no backtracking)
✅ **Type Safety**: Rust's ownership model prevents data races

### Negative

⚠️ **Configuration Complexity**: Stream configs become longer (40+ column mappings)
⚠️ **Memory Usage**: Must allocate `Vec<TimeSeriesPoint>` for all columns upfront
⚠️ **Error Handling**: Partial failures (some columns succeed, others fail) need strategy
⚠️ **Timestamp Alignment**: Different metrics may have different time intervals (PT1H vs PT3H)

### Mitigation Strategies

**Configuration Complexity**:
- Use YAML anchors for repeated patterns
- Provide template configs for common use cases
- Auto-generate configs from API schemas (future tool)

**Memory Usage**:
- Limit buffer sizes (2500 points = ~200 KB)
- Stream processing (iterate columns, flush points incrementally)
- Monitor memory usage in production

**Error Handling**:
- Mark columns as `optional: true` to allow partial success
- Log warnings for missing optional columns
- Fail fast on required columns

**Timestamp Alignment**:
- Store raw intervals (no interpolation)
- Let Grafana queries handle alignment via time_bucket()
- Document interval variability in stream schema

---

## Alternatives Considered

### Alternative 1: Pre-Transform Middleware

**Approach**: Add middleware layer to transform column-oriented JSON to row-oriented before parsing.

```rust
// REJECTED EXAMPLE
fn column_to_row_transformer(json: Value) -> Value {
    // Transform NWS format to ArrayIterator-compatible format
    // ...
}

let transformed = column_to_row_transformer(raw_json);
array_iterator_parser.parse(&transformed, timestamp)?;
```

**Rejected because**:
- ❌ Adds latency (two-pass processing)
- ❌ Increases memory usage (duplicate JSON trees)
- ❌ Hides complexity (debugging harder)
- ❌ Not reusable (Open-Meteo has different structure)

### Alternative 2: NWS-Specific Custom Parser

**Approach**: Hardcode NWS gridpoints parsing logic without generic column support.

```rust
// REJECTED EXAMPLE
pub struct NwsGridpointsParser {
    // Hardcoded NWS field names
}

impl Parser for NwsGridpointsParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        let temp = payload.pointer("/properties/temperature/values")?;
        let dewpoint = payload.pointer("/properties/dewpoint/values")?;
        // ... hardcoded for 40 fields
    }
}
```

**Rejected because**:
- ❌ Low flexibility (can't add/remove fields via config)
- ❌ Not reusable for Open-Meteo
- ❌ Duplicate code for each weather API
- ❌ Violates Domain Adapter Pattern (adapter should be generic)

### Alternative 3: Extend ArrayIteratorParser

**Approach**: Add "column mode" flag to existing `ArrayIteratorParser`.

```rust
// REJECTED EXAMPLE
pub struct ArrayIteratorConfig {
    pub array_path: String,
    pub column_mode: bool,  // NEW flag
    // ...
}
```

**Rejected because**:
- ❌ Conflates two different data structures (rows vs columns)
- ❌ Complex branching logic (if column_mode { ... } else { ... })
- ❌ Harder to test (combinatorial explosion of modes)
- ❌ Violates Single Responsibility Principle

---

## Implementation Impact

### Files Created

```
core/src/parsers/
├── column_oriented.rs          (NEW - ~300 LOC)
└── tests/
    └── column_oriented_test.rs (NEW - ~150 LOC)
```

### Files Modified

```
core/src/parsers/
├── config.rs                   (MODIFIED - add ColumnOriented variant)
├── mod.rs                      (MODIFIED - export new parser)
└── traits.rs                   (NO CHANGE)

apps/air-quality-app/src/
└── main.rs                     (MODIFIED - register parser in ParserRegistry)
```

### Configuration Changes

```
config/base/streams/
├── nws-gridpoints-forecast/
│   └── config.yaml             (NEW - stream configuration)
└── nws-station-observations/
    └── config.yaml             (NEW - uses FlatJsonParser)
```

### Testing Strategy

**Unit Tests**:
```rust
#[test]
fn test_parse_nws_gridpoints() {
    let sample_json = include_str!("../../test_data/nws_gridpoints_sample.json");
    let parser = ColumnOrientedParser::new(config);
    let points = parser.parse(&json, Utc::now()).unwrap();

    assert_eq!(points.len(), 40); // 1 value per column
    assert_eq!(points[0].metric_name, "temperature");
    assert_eq!(points[0].value, 15.5);
}

#[test]
fn test_missing_optional_column() {
    let json = json!({
        "properties": {
            "temperature": {"values": [{"validTime": "...", "value": 15.5}]}
            // Missing dewpoint (optional)
        }
    });

    let result = parser.parse(&json, Utc::now());
    assert!(result.is_ok()); // Should succeed (dewpoint optional)
}

#[test]
fn test_missing_required_column() {
    let json = json!({
        "properties": {
            "dewpoint": {"values": [...]}
            // Missing temperature (required)
        }
    });

    let result = parser.parse(&json, Utc::now());
    assert!(result.is_err()); // Should fail (temperature required)
}
```

**Integration Tests**:
- Live NWS API polling (with retries)
- End-to-end: HTTP fetch → parse → validate → store → query
- Performance test: 1000+ points per parse (<100ms)

---

## Migration Path

### Phase 1: Implementation (No Deployment)
1. Create `ColumnOrientedParser` in feature branch
2. Write unit tests with sample NWS JSON
3. Add to `ParserType` enum
4. Register in test harness (not production)

### Phase 2: Configuration (Stream Disabled)
1. Create `nws-gridpoints-forecast/config.yaml` with `enabled: false`
2. Sync to etcd (via ConfigSyncService)
3. Verify no runtime changes (stream disabled)

### Phase 3: Limited Rollout
1. Enable stream on single Pi (staging)
2. Monitor memory usage, parse latency, error rates
3. Validate Parquet storage correctness
4. Check dashboard queries work

### Phase 4: Full Deployment
1. Enable stream on all production Pi devices
2. Create Grafana dashboards
3. Document new metrics in data catalog
4. Train users on new weather data

---

## Open Questions

### Q1: How to handle metrics with different time intervals?

**Example**: Temperature updates every PT1H (1 hour), but probabilityOfPrecipitation updates every PT3H (3 hours).

**Answer**: Store each point with its actual timestamp. Do not interpolate or align. Grafana queries handle alignment via time_bucket().

```sql
-- Analytics query with 1-hour bucketing
SELECT
  time_bucket(INTERVAL '1 hour', timestamp) AS hour,
  AVG(value) FILTER (WHERE metric_name = 'temperature') AS avg_temp,
  AVG(value) FILTER (WHERE metric_name = 'probability_of_precipitation') AS avg_precip_prob
FROM read_parquet('/data/data/nws-gridpoints-forecast/**/*.parquet')
GROUP BY hour;
```

### Q2: Should parser validate value ranges?

**Example**: Temperature = 500°C (clearly invalid).

**Answer**: No. Parser extracts data as-is. `IngestionRouter` performs schema validation (range checks). Separation of concerns.

### Q3: How to test with real NWS API (rate limits)?

**Answer**:
1. **Unit tests**: Use static JSON samples (no HTTP)
2. **Integration tests**: Run nightly with real API (low frequency)
3. **CI/CD**: Skip live API tests (use mocks)
4. **Staging**: Enable on single Pi with real API

### Q4: What if NWS changes JSON structure?

**Answer**:
- **Version detection**: Check API response version field
- **Graceful degradation**: Parse available columns, log warnings
- **Monitoring**: Alert on parse error rate spikes
- **Config updates**: Update column paths via YAML (no code deploy)

---

## Success Criteria

This ADR is considered successful when:

1. ✅ **Parser implemented**: `ColumnOrientedParser` passes all unit tests
2. ✅ **NWS integration works**: Live API polling produces valid `TimeSeriesPoint` vectors
3. ✅ **Storage validated**: Parquet files contain expected columns
4. ✅ **Dashboard functional**: Grafana displays sky cover, visibility, etc.
5. ✅ **Performance acceptable**: Parse latency <100ms for 1000+ points
6. ✅ **No regressions**: Existing streams continue to function

---

## References

### Internal
- [AIR-007 Architecture](/workspaces/neural-data-platform/product/features/air-007/architecture/ARCHITECTURE.md)
- [Parser Trait Definition](/workspaces/neural-data-platform/core/src/parsers/traits.rs)
- [AIR-005 ADR Summary](/workspaces/neural-data-platform/docs/architecture/AIR-005_ADR_SUMMARY.md)

### External
- [NWS API Documentation](https://www.weather.gov/documentation/services-web-api)
- [ISO 8601 Duration Format](https://en.wikipedia.org/wiki/ISO_8601#Durations)
- [Open-Meteo API](https://open-meteo.com/en/docs) (future integration reference)

---

## Approval

| Role | Name | Date | Status |
|------|------|------|--------|
| System Architect | ndp-architect | 2025-12-24 | ✅ Proposed |
| Implementation Lead | (TBD) | - | Pending |
| Tech Lead | (TBD) | - | Pending |

---

## Revision History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-24 | Initial ADR for column-oriented parser |
