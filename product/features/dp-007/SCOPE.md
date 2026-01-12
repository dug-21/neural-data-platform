# DP-007: Pre-Transform Parser Integration for Silver ETL

## Summary
Integrate the existing `ColumnOrientedParser` from neural-core into silver-etl to enable pre-transformation of columnar array data (like NWS gridpoints forecasts) before DuckDB SQL processing.

## Problem Statement
The NWS gridpoints forecast data has a columnar array structure where each metric (temperature, windSpeed, etc.) contains arrays of `{validTime, value}` pairs. The current silver-etl uses DuckDB SQL which cannot easily handle this structure without complex UNNEST operations.

The `ColumnOrientedParser` in neural-core already solves this problem for Bronze ingestion - it explodes arrays into individual `TimeSeriesPoint` objects. However, Bronze stores the raw JSON, not the parsed points.

## Proposed Solution
Add a `pre_transform` configuration option to `silver_etl` that:
1. Parses `raw_payload` through `ColumnOrientedParser` before DuckDB processing
2. Outputs flattened rows with one row per metric per validTime
3. Allows standard DuckDB SQL to process the flattened data

## Data Flow

```
Current (doesn't work for arrays):
  Bronze Parquet (raw JSON with arrays)
       ↓
  DuckDB SQL (json_extract_path)  ← FAILS: can't handle arrays
       ↓
  Silver TimescaleDB

Proposed (with pre-transform):
  Bronze Parquet (raw JSON with arrays)
       ↓
  [Rust Pre-Transform - ColumnOrientedParser]  ← NEW
       ↓
  Flattened temp table (one row per metric per validTime)
       ↓
  DuckDB SQL (standard field extraction)
       ↓
  Silver TimescaleDB
```

## Target Streams
- `nws-gridpoints-forecast` (primary use case)
- Potentially other columnar data sources in the future

## Success Criteria
1. `ColumnOrientedParser` integrated into silver-etl crate
2. New `pre_transform` config section in `SilverEtlConfig`
3. `nws-gridpoints-forecast` successfully loads to Silver with `enabled: true`
4. Forecast data queryable in TimescaleDB with issue_time and valid_time
5. All existing streams continue to work without pre-transform

## Out of Scope
- New parser implementations (reuse existing)
- Changes to Bronze layer
- Changes to other streams' configurations (except nws-gridpoints-forecast)

## Dependencies
- DP-006: Config-Driven Silver ETL (completed)
- neural-core parsers module
- Existing `ColumnOrientedParser` implementation

## References
- `core/src/parsers/column_oriented.rs` - Existing parser implementation
- `core/src/parsers/config.rs` - Parser configuration types
- `apps/silver-etl/src/` - Silver ETL application
- `config/base/streams/nws-gridpoints-forecast/config.yaml` - Target stream config
