# ADR-002: NWS Stream Strategy - Separate vs Combined Streams

## Status
**Proposed** (2025-12-24)

---

## Context

AIR-007 introduces two NWS data sources:

1. **Raw Gridpoints Forecast** (`/gridpoints/JAX/79,49`)
   - 40+ forecast fields
   - Column-oriented JSON (metric → values[] arrays)
   - Updates every 1-6 hours (model-dependent)
   - Forecast horizon: 156 hours (6.5 days)

2. **Station Observations** (`/stations/KSGJ/observations/latest`)
   - Current conditions (single object)
   - Flat JSON structure
   - Updates every 15-20 minutes
   - Real-time ground truth data

### Question

Should these be:
- **Option A**: Separate streams (`nws-gridpoints-forecast` + `nws-station-observations`)
- **Option B**: Combined stream (`nws-weather-all`)
- **Option C**: Multiple streams by field category (`nws-temperature`, `nws-wind`, `nws-visibility`)

---

## Decision

**Use separate streams (Option A):**
- `nws-gridpoints-forecast` - Column-oriented forecast data
- `nws-station-observations` - Single-object current conditions

Each stream has its own:
- Stream ID and configuration
- Poll interval (1 hour vs 15 minutes)
- Parser type (`ColumnOrientedParser` vs `FlatJsonParser`)
- Retention policy (30 days vs 90 days)
- Storage location (`/data/nws-gridpoints-forecast/` vs `/data/nws-station-observations/`)

---

## Rationale

### 1. **Different Update Frequencies**

| Stream | Poll Interval | Reason |
|--------|---------------|--------|
| Gridpoints Forecast | 1 hour | Model updates hourly, faster polling wastes API quota |
| Station Observations | 15 minutes | Real-time data, 20-minute MADIS delay justifies 15-min polling |

**Impact if combined**:
- Combined stream would need 15-minute polling (wasteful for forecasts)
- OR 1-hour polling (stale observations)
- Cannot optimize per-source polling frequency

### 2. **Different Data Structures**

| Stream | Structure | Parser |
|--------|-----------|--------|
| Gridpoints | Column-oriented (40 arrays) | `ColumnOrientedParser` |
| Observations | Flat object (1 point) | `FlatJsonParser` |

**Impact if combined**:
- Parser would need conditional logic (if forecast { ... } else { ... })
- Harder to test (two code paths)
- Violates Single Responsibility Principle

### 3. **Different Retention Policies**

| Stream | Retention | Rationale |
|--------|-----------|-----------|
| Gridpoints Forecast | 30 days | Forecasts are short-lived, historical value limited |
| Station Observations | 90 days | Real-time data useful for long-term analysis |

**Impact if combined**:
- Cannot apply different retention rules
- Either waste storage (long retention for forecasts) OR lose data (short retention for observations)

### 4. **Cleaner Dashboard Queries**

**Separate streams**:
```sql
-- Query only forecast data
SELECT * FROM bronze_nws_gridpoints_forecast
WHERE metric_name = 'sky_cover'
  AND timestamp >= NOW() - INTERVAL '7 days';

-- Query only observations
SELECT * FROM bronze_nws_station_observations
WHERE metric_name = 'temperature'
  AND timestamp >= NOW() - INTERVAL '24 hours';
```

**Combined stream** (hypothetical):
```sql
-- Must filter by source type in every query
SELECT * FROM bronze_nws_weather_all
WHERE metric_name = 'sky_cover'
  AND source_type = 'forecast'  -- Extra filtering needed
  AND timestamp >= NOW() - INTERVAL '7 days';
```

### 5. **Independent Failure Domains**

**Separate streams**:
- If gridpoints API fails, observations continue
- If observations API fails, forecasts continue
- Each stream can have independent error handling

**Combined stream**:
- Single parse failure affects entire stream
- Harder to diagnose which source failed
- Error logging becomes complex

### 6. **Schema Evolution Flexibility**

**Separate streams**:
- Add/remove forecast fields without affecting observations
- Change observation fields without affecting forecasts
- Independent versioning (gridpoints v2.0, observations v1.1)

**Combined stream**:
- Schema changes affect both data types
- Version conflicts (if APIs version independently)
- Migration complexity

---

## Consequences

### Positive

✅ **Optimized Polling**: Each stream polls at appropriate frequency
✅ **Simple Parsers**: One parser per data structure (clean separation)
✅ **Flexible Retention**: Different retention rules for different data
✅ **Easier Debugging**: Clear stream boundaries for error tracing
✅ **Independent Scaling**: Can disable/enable streams separately
✅ **Clean Analytics**: Queries don't need source type filtering

### Negative

⚠️ **More Configuration**: Two stream config files instead of one
⚠️ **More Parquet Files**: Two storage locations instead of one
⚠️ **JOIN Complexity**: Comparing forecast vs observation requires JOIN

### Mitigation Strategies

**Configuration Overhead**:
- Use YAML templates for common config sections
- Document stream relationships in README
- Automate stream creation via script

**Storage Management**:
- Grafana DuckDB plugin handles multi-file queries efficiently
- Parquet partitioning keeps files organized
- Minimal overhead (2 directories vs 1)

**JOIN Queries**:
- Use Grafana panel with JOIN query against Parquet files
- Example:
  ```sql
  SELECT
    f.timestamp AS forecast_time,
    f.value AS forecast_temp,
    o.value AS actual_temp,
    (o.value - f.value) AS error
  FROM read_parquet('/data/data/nws-gridpoints-forecast/**/*.parquet') f
  JOIN read_parquet('/data/data/nws-station-observations/**/*.parquet') o
    ON time_bucket(INTERVAL '1 hour', to_timestamp(f.timestamp/1000000)) =
       time_bucket(INTERVAL '1 hour', to_timestamp(o.timestamp/1000000))
  WHERE f.metric = 'temperature'
    AND o.metric = 'temperature';
  ```

---

## Alternatives Considered

### Alternative 1: Combined Stream (Option B)

**Approach**: Single stream `nws-weather-all` with both forecast and observation data.

```yaml
# REJECTED EXAMPLE
stream_id: nws-weather-all
sources:
  - type: http_poll
    endpoints:
      - url: https://api.weather.gov/gridpoints/JAX/79,49
        parser: column_oriented
      - url: https://api.weather.gov/stations/KSGJ/observations/latest
        parser: flat_json
```

**Rejected because**:
- ❌ Cannot optimize poll intervals per source
- ❌ Parser logic becomes complex (multi-format support)
- ❌ Single failure point (one parse error affects all)
- ❌ Retention policy conflict
- ❌ Dashboard queries need extra filtering

### Alternative 2: Field Category Streams (Option C)

**Approach**: Create streams by field type (temperature, wind, visibility, etc.).

```yaml
# REJECTED EXAMPLE
stream_id: nws-temperature
sources:
  - gridpoints: properties.temperature.values
  - observations: properties.temperature.value

stream_id: nws-wind
sources:
  - gridpoints: properties.windSpeed.values
  - observations: properties.windSpeed.value
```

**Rejected because**:
- ❌ Over-engineering (40+ streams for gridpoints fields)
- ❌ Duplicate HTTP requests (poll same API 40 times)
- ❌ Complex configuration management
- ❌ Inefficient storage (40x Parquet overhead)
- ❌ Analytics nightmare (JOIN 40 streams)

### Alternative 3: Source-Type Tags (Hybrid)

**Approach**: Single stream with `source_type` tag (`forecast` vs `observation`).

```yaml
# REJECTED EXAMPLE
stream_id: nws-weather
default_tags:
  source_type: forecast  # or observation
```

**Rejected because**:
- ❌ Still requires conditional parser logic
- ❌ Poll interval conflict remains
- ❌ Retention policy conflict remains
- ❌ Dashboard queries still need filtering
- ⚠️ Provides no benefits over separate streams

---

## Implementation Impact

### Configuration Files Created

```
config/base/streams/
├── nws-gridpoints-forecast/
│   └── config.yaml             # NEW - gridpoints stream
└── nws-station-observations/
    └── config.yaml             # NEW - observations stream
```

### Storage Layout

```
/data/
├── nws-gridpoints-forecast/
│   ├── 2025-12-24_readings.parquet
│   ├── 2025-12-25_readings.parquet
│   └── ...
└── nws-station-observations/
    ├── 2025-12-24_readings.parquet
    ├── 2025-12-25_readings.parquet
    └── ...
```

### Analytics Queries

**Forecast-only query**:
```sql
SELECT
  metric_name,
  AVG(value) AS avg_value
FROM bronze_nws_gridpoints_forecast
WHERE timestamp >= NOW() - INTERVAL '7 days'
GROUP BY metric_name;
```

**Observation-only query**:
```sql
SELECT
  timestamp,
  value AS current_temp
FROM bronze_nws_station_observations
WHERE metric_name = 'temperature'
  AND timestamp >= NOW() - INTERVAL '24 hours'
ORDER BY timestamp DESC
LIMIT 1;
```

**Forecast accuracy analysis** (Grafana query):
```sql
SELECT
  time_bucket(INTERVAL '1 hour', to_timestamp(f.timestamp/1000000)) AS hour,
  AVG(f.value) AS forecast_temp,
  AVG(o.value) AS actual_temp,
  AVG(ABS(f.value - o.value)) AS abs_error
FROM read_parquet('/data/data/nws-gridpoints-forecast/**/*.parquet') f
INNER JOIN read_parquet('/data/data/nws-station-observations/**/*.parquet') o
  ON time_bucket(INTERVAL '1 hour', to_timestamp(f.timestamp/1000000)) =
     time_bucket(INTERVAL '1 hour', to_timestamp(o.timestamp/1000000))
WHERE f.metric = 'temperature'
  AND o.metric = 'temperature'
GROUP BY 1
ORDER BY 1;
```

---

## Validation Strategy

### Test Cases

**TC-1: Independent Polling**
- Start both streams
- Verify gridpoints polls every 1 hour
- Verify observations polls every 15 minutes
- Check no interference between streams

**TC-2: Independent Failure**
- Simulate gridpoints API failure
- Verify observations continue normally
- Check error logs only mention gridpoints stream

**TC-3: Different Parsers**
- Send malformed JSON to gridpoints endpoint
- Verify `ColumnOrientedParser` error
- Verify observations parser unaffected

**TC-4: Storage Separation**
- Poll both streams for 24 hours
- Verify Parquet files in separate directories
- Check no cross-contamination of data

**TC-5: Analytics Queries**
- Query gridpoints data (forecast)
- Query observations data (actual)
- JOIN for forecast accuracy
- Verify query performance (<1 second)

---

## Migration Path

### Phase 1: Create Configurations (Streams Disabled)
```bash
# Create stream configs with enabled: false
config/base/streams/nws-gridpoints-forecast/config.yaml
config/base/streams/nws-station-observations/config.yaml
```

### Phase 2: Enable Gridpoints Stream
```yaml
# nws-gridpoints-forecast/config.yaml
enabled: true
```
- Test polling, parsing, storage
- Validate Parquet schema
- Check dashboard queries

### Phase 3: Enable Observations Stream
```yaml
# nws-station-observations/config.yaml
enabled: true
```
- Test independent operation
- Verify no interference with gridpoints
- Validate cross-stream queries

### Phase 4: Create Grafana Dashboards
- Build gridpoint forecast dashboard (queries Parquet directly)
- Build station observations dashboard (queries Parquet directly)
- Build forecast vs observations comparison dashboard

---

## Open Questions

### Q1: Should we add a third stream for NWS alerts?

**Answer**: Out of scope for AIR-007, but architecture supports it. Create `nws-alerts` stream in future feature (AIR-008?).

### Q2: How to handle duplicate data if switching between endpoints?

**Example**: Gridpoints also contains current conditions (overlaps with observations).

**Answer**:
- Separate streams ensure no duplicates (different stream IDs)
- If needed, use Grafana query to deduplicate (prefer observations as "ground truth")
- Document preference in data dictionary

### Q3: Should forecasts and observations share location_id?

**Answer**: No. Use distinct location IDs:
- Gridpoints: `ksgj_gridpoints` (forecast grid point)
- Observations: `ksgj_station` (physical station)
- Rationale: Different spatial resolution (grid cell vs point)

### Q4: What if NWS combines APIs in future?

**Answer**:
- Create new stream `nws-combined-v2`
- Deprecate old streams gradually
- Use stream versioning to track migration
- Backward compatibility not required (Bronze layer raw data)

---

## Success Criteria

This ADR is considered successful when:

1. ✅ **Independent Operation**: Both streams poll and store data without interference
2. ✅ **Optimized Polling**: Gridpoints 1-hour interval, observations 15-minute interval
3. ✅ **Clean Queries**: Dashboard queries work without source type filtering
4. ✅ **Forecast Accuracy**: Can JOIN streams to calculate prediction error
5. ✅ **No Regressions**: Existing streams (outdoor-weather, etc.) unaffected

---

## References

### Internal
- [AIR-007 Architecture](/workspaces/neural-data-platform/product/features/air-007/architecture/ARCHITECTURE.md)
- [ADR-001: Column-Oriented Parser](/workspaces/neural-data-platform/product/features/air-007/architecture/ADR-001-column-oriented-parser.md)
- [Platform Architecture Overview](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)

### External
- [NWS Gridpoints API](https://api.weather.gov/gridpoints/JAX/79,49)
- [NWS Station Observations API](https://api.weather.gov/stations/KSGJ/observations/latest)
- [NWS API Documentation](https://www.weather.gov/documentation/services-web-api)

---

## Approval

| Role | Name | Date | Status |
|------|------|------|--------|
| System Architect | ndp-architect | 2025-12-24 | ✅ Proposed |
| Data Engineer | (TBD) | - | Pending |
| Tech Lead | (TBD) | - | Pending |

---

## Revision History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-24 | Initial ADR for NWS stream separation strategy |
