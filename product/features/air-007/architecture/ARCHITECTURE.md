# AIR-007 Architecture - NWS Gridpoints Weather Data Expansion

**Version**: 1.0.0
**Date**: 2025-12-24
**Status**: Design Phase

---

## Executive Summary

AIR-007 extends the Neural Data Platform's weather data collection capabilities by adding two new NWS data streams: raw gridpoints forecast data (40+ fields) and station observations. This requires a new **ColumnOrientedParser** to handle NWS's unique JSON structure where each metric contains its own time-series array.

### Key Architectural Changes

1. **New Parser Type**: `ColumnOrientedParser` for NWS gridpoints format
2. **Two New Streams**: `nws-gridpoints-forecast` and `nws-station-observations`
3. **Reusable Design**: Parser architecture supports future Open-Meteo integration
4. **No Breaking Changes**: Extends existing `HttpPollingSource` and `Parser` trait

---

## System Context

### Current State (AIR-005)

The platform currently supports:
- **MQTT sources**: AirGradient sensors
- **HTTP polling sources**: OpenWeatherMap (weather and air quality)
- **Parsers**: `FlatJsonParser`, `JsonPathParser`, `ArrayIteratorParser`
- **Existing NWS stream**: `nws-forecast-hourly` (12 fields via `/forecast/hourly` endpoint)

### AIR-007 Additions

```
New NWS Data Sources
├── Raw Gridpoints API (/gridpoints/JAX/79,49)
│   ├── 40+ forecast fields (temperature, dewpoint, humidity, wind, clouds, visibility, etc.)
│   ├── Column-oriented JSON format (new challenge)
│   ├── Variable time intervals (PT1H, PT3H, PT6H)
│   └── Uses ColumnOrientedParser (NEW)
│
└── Station Observations API (/stations/KSGJ/observations/latest)
    ├── Current conditions (single object, not array)
    ├── Real-time ground truth data
    ├── ~20 minute delay (MADIS QC processing)
    └── Uses FlatJsonParser (existing)
```

---

## Data Flow Architecture

### Overall Ingestion Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         NWS API Endpoints                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  /gridpoints/JAX/80,50                                                  │
│  ├── Raw forecast (column-oriented JSON)                               │
│  ├── 40+ metrics, each with values[] array                             │
│  └── Variable time intervals (PT1H, PT3H, PT6H)                        │
│                                                                         │
│  /stations/KSGJ/observations/latest                                     │
│  ├── Current observations (flat JSON object)                           │
│  └── Single point per poll                                             │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ HTTPS poll every 1-6 hours
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        HttpPollingSource                                │
│                  (core/src/sources/http_poll.rs)                        │
│                                                                         │
│  - Handles HTTP requests, auth, retries                                │
│  - User-Agent: Required by NWS API                                     │
│  - Accept: "application/geo+json"                                      │
└────────────────────────────┬────────────────────────────────────────────┘
                             │ Raw JSON response
                             ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          Parser Layer                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌───────────────────────┐        ┌───────────────────────┐           │
│  │ ColumnOrientedParser  │        │    FlatJsonParser     │           │
│  │       (NEW)           │        │     (existing)        │           │
│  │                       │        │                       │           │
│  │ - Gridpoints format   │        │ - Station obs format  │           │
│  │ - Iterate columns     │        │ - Extract flat fields │           │
│  │ - Parse ISO 8601      │        │ - Single TimeSeriesPoint│          │
│  │ - Multiple points     │        │                       │           │
│  └───────┬───────────────┘        └───────┬───────────────┘           │
│          │                                │                            │
│          └────────────────┬───────────────┘                            │
│                           │ Vec<TimeSeriesPoint>                       │
└───────────────────────────┼────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                    mpsc::channel<TimeSeriesPoint>                       │
│                  (owned by IngestionCoordinator)                        │
│                         Buffer: 10000                                   │
└────────────────────────────┬────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         IngestionRouter                                 │
│                                                                         │
│  - Schema validation                                                    │
│  - Dead-letter queue for invalid points                                │
│  - Route by stream_id                                                   │
└────────────────────────────┬────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         StorageWriter                                   │
│                                                                         │
│  - Batch accumulation (100 points or 5s timeout)                       │
│  - Write to Parquet with WAL                                           │
└────────────────────────────┬────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                     Bronze Layer (Parquet Files)                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  /data/nws-gridpoints-forecast/2025-12-24_readings.parquet            │
│  /data/nws-station-observations/2025-12-24_readings.parquet           │
│                                                                         │
│  Fields: timestamp, location_id, metric_name, value, unit, tags        │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Parser Architecture

### Domain Adapter Pattern Integration

AIR-007 follows the established Domain Adapter Pattern:

```rust
// Port (Trait)
pub trait Parser: Send + Sync {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>;
    fn name(&self) -> &str;
    fn config(&self) -> &ParserConfig;
}

// NEW Adapter
pub struct ColumnOrientedParser {
    config: ParserConfig,
    column_mappings: Vec<ColumnMapping>,
}

impl Parser for ColumnOrientedParser {
    // Implementation details in ADR-001
}
```

### Parser Type Enumeration Extension

```rust
// core/src/parsers/config.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserType {
    FlatJson,           // Existing
    JsonPath,           // Existing
    ArrayIterator,      // Existing
    ColumnOriented,     // NEW for AIR-007
    Custom(String),     // Existing
}
```

### Column-Oriented JSON Format

**NWS Gridpoints Structure**:
```json
{
  "properties": {
    "temperature": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2025-12-24T12:00:00+00:00/PT1H", "value": 15.5},
        {"validTime": "2025-12-24T13:00:00+00:00/PT1H", "value": 16.1},
        ...
      ]
    },
    "dewpoint": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2025-12-24T12:00:00+00:00/PT1H", "value": 8.3},
        ...
      ]
    }
  }
}
```

**Parsing Strategy**:
1. Iterate over configured column paths (e.g., `properties.temperature.values`)
2. For each entry in `values[]`:
   - Extract timestamp from ISO 8601 interval (`2025-12-24T12:00:00+00:00/PT1H`)
   - Extract value
   - Create `TimeSeriesPoint` with metric name from column mapping
3. Return flattened `Vec<TimeSeriesPoint>`

---

## Stream Configurations

### Stream 1: nws-gridpoints-forecast

**Configuration File**: `config/base/streams/nws-gridpoints-forecast/config.yaml`

```yaml
stream_id: nws-gridpoints-forecast
description: Raw NWS gridpoint forecast data with 40+ fields
version: "1.0.0"
enabled: true
retention_days: 30
compression_after_days: 7
partitioning_strategy: daily

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 3600  # 1 hour (gridpoints update hourly)
    parser_name: nws_gridpoints
    endpoints:
      - endpoint_id: nws_jax_79_49_gridpoints
        location_id: ksgj_gridpoints
        url: "https://api.weather.gov/gridpoints/JAX/79,49"
        auth_type: none
        headers:
          User-Agent: "(neural-data-platform/1.0, YOUR_EMAIL)"
          Accept: "application/geo+json"
    parser:
      parser_type: column_oriented
      location_id_field: properties.gridId
      default_location_id: ksgj_gridpoints
      column_config:  # NEW for ColumnOrientedParser
        columns:
          - path: properties.temperature.values
            metric_name: temperature
            unit: celsius
          - path: properties.dewpoint.values
            metric_name: dewpoint
            unit: celsius
          - path: properties.relativeHumidity.values
            metric_name: relative_humidity
            unit: percent
          # ... 37+ more columns
```

### Stream 2: nws-station-observations

**Configuration File**: `config/base/streams/nws-station-observations/config.yaml`

```yaml
stream_id: nws-station-observations
description: Current NWS station observations (KSGJ)
version: "1.0.0"
enabled: true
retention_days: 90
compression_after_days: 7
partitioning_strategy: daily

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 900  # 15 minutes (observations update frequently)
    parser_name: nws_station_obs
    endpoints:
      - endpoint_id: nws_ksgj_obs
        location_id: ksgj_station
        url: "https://api.weather.gov/stations/KSGJ/observations/latest"
        auth_type: none
        headers:
          User-Agent: "(neural-data-platform/1.0, YOUR_EMAIL)"
          Accept: "application/geo+json"
    parser:
      parser_type: flat_json  # Uses existing parser
      location_id_field: properties.station
      default_location_id: ksgj_station
      field_mappings:
        - path: properties.temperature.value
          metric_name: temperature
          unit: celsius
        - path: properties.dewpoint.value
          metric_name: dewpoint
          unit: celsius
        # ... standard flat field mappings
```

---

## Integration Points

### IngestionCoordinator (ADR-001 Reference)

Per **ADR-001** from AIR-005, the `IngestionCoordinator` owns the master mpsc channel. AIR-007 sources integrate via:

```rust
// In IngestionCoordinator::start()
let sender_clone = self.sender.clone();

// SourceManager spawns HttpPollingSource for gridpoints
source_manager.spawn_source(
    "nws-gridpoints-forecast",
    stream_config,
    sender_clone,
    cancel_token.clone()
).await?;
```

**No changes to coordinator ownership model.**

### HttpPollingSource (Existing Adapter)

The existing `HttpPollingSource` (from AIR-005) handles HTTP mechanics:
- Request construction
- Authentication (User-Agent header required by NWS)
- Retry logic with exponential backoff
- Timeout handling

**AIR-007 only adds new parser, not new source type.**

### Parser Registration

Parsers are registered in the global `ParserRegistry` at startup:

```rust
// In main.rs or parser module initialization
let parser_registry = ParserRegistry::new();

// Existing parsers
parser_registry.register("flat_json", Arc::new(FlatJsonParser::new()));
parser_registry.register("array_iterator", Arc::new(ArrayIteratorParser::new()));

// NEW for AIR-007
parser_registry.register("nws_gridpoints", Arc::new(ColumnOrientedParser::new()));
```

---

## Component Dependencies

### New Dependencies

```
ColumnOrientedParser (NEW)
├── Depends on: Parser trait (core/src/parsers/traits.rs)
├── Depends on: ParserConfig (core/src/parsers/config.rs)
├── Depends on: TimeSeriesPoint (core/src/types/mod.rs)
├── Depends on: chrono (ISO 8601 parsing)
└── Depends on: serde_json (JSON navigation)
```

### Modified Components

```
ParserType enum (MODIFIED)
└── Location: core/src/parsers/config.rs
    └── Add: ColumnOriented variant

ParserConfig struct (EXTENDED)
└── Location: core/src/parsers/config.rs
    └── Add: column_config: Option<ColumnOrientedConfig>
```

### Unchanged Components (Reuse)

- `HttpPollingSource` - No changes
- `IngestionCoordinator` - No changes (ADR-001 ownership model)
- `IngestionRouter` - No changes (schema validation works as-is)
- `ParquetStore` - No changes (accepts TimeSeriesPoint vectors)
- `StreamRegistry` - No changes (YAML → etcd sync)

---

## Data Quality and Validation

### Schema Validation (IngestionRouter)

Each stream defines expected fields with validation rules:

```yaml
# nws-gridpoints-forecast/config.yaml
fields:
  - name: temperature
    type: float
    nullable: false
    unit: celsius
    range: [-50.0, 60.0]  # Realistic range
  - name: sky_cover
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]
```

**Router behavior**:
- Valid points → Parquet storage
- Invalid points → Dead-letter queue (logged, not stored)
- Out-of-range values → NULL (configurable)

### ISO 8601 Duration Parsing

NWS uses intervals like `2025-12-24T12:00:00+00:00/PT1H`:
- Start time: `2025-12-24T12:00:00+00:00`
- Duration: `PT1H` (1 hour)

**ColumnOrientedParser** extracts start time as the timestamp for each point.

---

## Cross-Cutting Concerns

### Error Handling

```rust
use crate::error::{CoreError, CoreResult};

impl Parser for ColumnOrientedParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>
    {
        // Use .map_err() for context
        let columns = payload
            .pointer("/properties")
            .ok_or(CoreError::ParseError("Missing properties".into()))?;

        // ... parsing logic
    }
}
```

### Logging

```rust
use tracing::{info, warn, debug};

debug!("Parsing column: {}", column_path);
warn!("Invalid timestamp format: {}", raw_value);
info!("Parsed {} points from {} columns", points.len(), column_count);
```

### Resource Constraints

**Target**: Raspberry Pi 5 (<2GB total memory)

- **Parser memory**: <10MB per parse operation
- **Buffer sizing**: 2500 points (NWS gridpoints can generate ~1000+ points per poll)
- **Async design**: Non-blocking parsing (use `tokio::spawn_blocking` if needed)

---

## Extension Points

### Future Open-Meteo Support

The `ColumnOrientedParser` is designed to be reusable for Open-Meteo's API, which uses a similar structure:

```json
{
  "hourly": {
    "time": ["2025-12-24T00:00", "2025-12-24T01:00", ...],
    "temperature_2m": [15.5, 16.1, ...],
    "cloud_cover": [75, 80, ...]
  }
}
```

**Adaptation strategy** (future work):
- Use `ColumnOrientedParser` with different `column_config`
- Map Open-Meteo's parallel arrays (shared `time[]`) to NWS's nested objects
- Minimal parser changes, primarily configuration-driven

---

## Architecture Decision Records

### ADR-001: Column-Oriented Parser Design
See: `ADR-001-column-oriented-parser.md`

**Decision**: Create new `ColumnOrientedParser` for NWS gridpoints format.

### ADR-002: Separate Streams Strategy
See: `ADR-002-nws-stream-strategy.md`

**Decision**: Use separate streams for gridpoints forecast and station observations.

---

## Performance Characteristics

### Expected Load

| Stream | Poll Interval | Points/Poll | Points/Day |
|--------|---------------|-------------|------------|
| nws-gridpoints-forecast | 1 hour | ~1000 | ~24,000 |
| nws-station-observations | 15 minutes | 1 | ~96 |

**Total**: ~24,100 points/day from NWS sources (negligible vs AirGradient sensors)

### Storage Requirements

- **Parquet compression ratio**: ~10:1 (typical for time-series)
- **Daily storage**: ~500 KB/day for gridpoints, ~10 KB/day for observations
- **30-day retention**: ~15 MB total (well within Pi constraints)

---

## Migration Path

### Phase 1: Parser Implementation
1. Implement `ColumnOrientedParser` in `core/src/parsers/column_oriented.rs`
2. Add `ColumnOriented` variant to `ParserType` enum
3. Extend `ParserConfig` with `column_config` field
4. Write unit tests with sample NWS JSON

### Phase 2: Stream Configuration
1. Create `config/base/streams/nws-gridpoints-forecast/config.yaml`
2. Create `config/base/streams/nws-station-observations/config.yaml`
3. Register parser in `main.rs` startup sequence
4. Sync configs to etcd via `ConfigSyncService`

### Phase 3: Integration Testing
1. Test gridpoints parsing with live NWS API
2. Verify station observations parsing
3. Check schema validation in `IngestionRouter`
4. Validate Parquet storage and WAL recovery

### Phase 4: Grafana Dashboards
1. Build gridpoint forecast dashboard (queries Parquet directly)
2. Build station observations dashboard (queries Parquet directly)
3. Add forecast vs observation comparison panel

---

## References

### Internal Documentation
- [Platform Architecture Overview](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [AIR-005 ADR Summary](/workspaces/neural-data-platform/docs/architecture/AIR-005_ADR_SUMMARY.md)
- [Parser Trait Definition](/workspaces/neural-data-platform/core/src/parsers/traits.rs)
- [AIR-007 Scope](/workspaces/neural-data-platform/product/features/air-007/SCOPE.md)

### External References
- NWS API Documentation: `product/research/weatherresources/NWS-COMPLETE-ANALYSIS.md`
- Weather Data Comparison: `product/research/weatherresources/COMPARISON.md`
- NWS Gridpoints Endpoint: `https://api.weather.gov/gridpoints/{wfo}/{x},{y}`
- NWS Observations Endpoint: `https://api.weather.gov/stations/{id}/observations/latest`

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-24 | Initial architecture design for AIR-007 |
