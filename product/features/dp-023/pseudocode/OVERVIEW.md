# dp-023: Pseudocode Overview

## Component Interaction

```
                    ┌─────────────────────────────────────────────────────┐
                    │  config/base/streams/nws-forecast-hourly/config.json │
                    │  (stream_type, silver_etl with text field_mappings)  │
                    └──────────┬──────────────────────┬───────────────────┘
                               │                      │
                    ┌──────────▼──────────┐ ┌────────▼────────────────┐
                    │  ndp-validate        │ │  deploy/pi/             │
                    │  (schema validation) │ │  ddl-generator.sh       │
                    │                      │ │  (Silver DDL with TEXT) │
                    └──────────────────────┘ │  deploy.sh              │
                                             │  (dictionary sync)      │
                                             └────────┬───────────────┘
                                                      │
                    ┌─────────────────────────────────▼───────────────────┐
                    │  core/src/silver/transform.rs                        │
                    │  coerce_to_type() — jsonb branch                    │
                    │                                                      │
                    │  core/src/silver/outputs/timescale.rs                │
                    │  build_upsert_query() — jsonb cast                  │
                    │  write() — text/jsonb param binding                  │
                    └─────────────────────────────────┬───────────────────┘
                                                      │
                                                      │ INSERT INTO silver.nws_forecast_hourly
                                                      │ (text + numeric columns)
                                                      ▼
                    ┌─────────────────────────────────────────────────────┐
                    │  TimescaleDB Silver Layer                            │
                    │  silver.nws_forecast_hourly                         │
                    │  (observation_time, ndp_id, temperature_f, ...,     │
                    │   short_forecast TEXT, detailed_forecast TEXT)       │
                    └─────────────────────────────────┬───────────────────┘
                                                      │
                    ┌─────────────────────────────────▼───────────────────┐
                    │  crates/ndp-lib/src/gold/generators/text_view.rs    │
                    │  TextViewGenerator — per-domain VIEW                │
                    │  gold.indoor_air_quality_text                        │
                    └─────────────────────────────────────────────────────┘
```

## Data Flow

1. **Bronze**: NWS API -> parser -> RawDataPoint (shortForecast + detailedForecast in raw_payload JSON)
2. **Silver Transform**: EventBus -> SilverSubscriber -> `transform_to_silver()` -> `apply_field_mapping()` -> `coerce_to_type("text")` / `coerce_to_type("jsonb")`
3. **Silver Output**: `TimescaleOutput.write()` -> `build_upsert_query()` (with `::jsonb` cast for jsonb columns) -> `build_raw_query()` -> PostgreSQL INSERT
4. **Gold**: `TextViewGenerator` reads domain config -> generates `CREATE OR REPLACE VIEW gold.{domain}_text` -> DISTINCT ON query across text-bearing Silver tables

## Component Files

| Component | Pseudocode | What It Covers |
|-----------|-----------|----------------|
| platform-core | pseudocode/platform-core.md | Silver transform jsonb branch, TimescaleOutput binding |
| ndp-lib | pseudocode/ndp-lib.md | Gold TextViewGenerator |
| deploy-sh | pseudocode/deploy-sh.md | DDL generator verification, deploy.sh Gold text integration, dictionary sync |
| ndp-validate | pseudocode/ndp-validate.md | Schema validation for text/jsonb types |
| config | pseudocode/config.md | NWS forecast stream config changes |

## Integration Surfaces

| Surface | From | To | Data |
|---------|------|----|------|
| Stream config -> Silver subscriber | config JSON | SilverSubscriber | field_mappings with type="text"/"jsonb" |
| Silver subscriber -> TimescaleDB | TimescaleOutput | PostgreSQL | INSERT with TEXT/JSONB columns |
| Stream config -> DDL generator | config JSON | ddl-generator.sh | map_type() for TEXT/JSONB DDL |
| Stream config -> Gold generator | domain config | TextViewGenerator | text field discovery for VIEW generation |
| Stream config -> dictionary sync | config JSON | deploy.sh | silver_columns with TEXT/JSONB data_type |
| Stream config -> ndp validate | config JSON | ndp-validate | field_mappings type acceptance |
