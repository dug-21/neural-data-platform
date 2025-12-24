# AIR-007: NWS Gridpoints Weather Data Expansion

## Overview

Expand NWS weather data collection to use the raw gridpoints API endpoint, enabling access to 40+ forecast fields and current observations that are not available through the simplified forecast/hourly endpoint.

## Problem Statement

The current `nws-forecast-hourly` stream uses the `/gridpoints/{wfo}/{x},{y}/forecast/hourly` endpoint which only provides 12 fields. Critical meteorological data like `skyCover`, `visibility`, `windGust`, `ceilingHeight`, and fire weather indices are missing but available in the raw gridpoints endpoint.

Additionally, we have no current observation data from NWS stations, which would provide real-time ground truth for sensor correlation.

## Goals

1. **Add NWS Raw Gridpoints Stream** - Capture 40+ forecast metrics from `/gridpoints/{wfo}/{x},{y}`
2. **Add NWS Station Observations Stream** - Capture current conditions from `/stations/{id}/observations`
3. **Build Generic Column-Oriented Parser** - Handle NWS's column-oriented JSON format (reusable for Open-Meteo)
4. **Create Weather Dashboards** - Visualize gridpoint forecast and observation data in Grafana

## Deliverables

### Stream Configurations
- [ ] `nws-gridpoints-forecast` - Raw gridpoint forecast data (40+ fields)
- [ ] `nws-station-observations` - Current observation data from weather stations

### Parser Implementation
- [ ] `ColumnOrientedParser` - New parser type for column-oriented JSON
- [ ] Support for NWS gridpoints format (metric → values[] arrays)
- [ ] Support for NWS ISO 8601 duration timestamps (PT1H, PT3H, etc.)
- [ ] Configurable field mappings

### Dashboards
- [ ] NWS Gridpoint Forecast Dashboard
- [ ] NWS Current Observations Dashboard
- [ ] Forecast vs Observations Comparison Panel

## Out of Scope

- Open-Meteo integration (future feature, but parser should support it)
- Historical backfill from NWS archive endpoint
- Multi-location support beyond Jacksonville area (can be added via config later)
- Air quality data (NWS doesn't provide this)

## Technical Constraints

- Parser must handle variable time intervals (PT1H, PT3H, PT6H durations)
- Station observations are single objects, not arrays
- Raw gridpoints updates every 1-6 hours depending on model
- Station observations have ~20 minute delay due to MADIS QC processing

## Success Criteria

1. Successfully ingest raw gridpoint forecast data with all 40+ fields
2. Successfully ingest station observation data
3. Dashboard displays cloud cover, visibility, and other fields missing from current implementation
4. Parser is generic enough to support future Open-Meteo integration
5. All existing streams continue to function (no regressions)

## Research References

- `product/research/weatherresources/NWS-COMPLETE-ANALYSIS.md`
- `product/research/weatherresources/COMPARISON.md`

## Target Location

- Grid Office: JAX (Jacksonville)
- Grid Coordinates: 79,49
- Nearest Station: KSGJ (NE Florida Regional Airport)
