# DP-004: Bronze Layer Raw JSON Schema

## Summary

Redesign Bronze layer (Parquet) to store raw JSON payloads instead of parsed, typed metrics. This enables source resilience, replay capability, and support for non-numeric data types.

## Motivation

1. **Data Loss Prevention**: Current parsing transforms data at ingestion, losing original values
2. **Source Resilience**: Format changes break parsers; raw storage defers parsing to Silver ETL
3. **Replay Capability**: Cannot reprocess historical data with current schema
4. **Type Flexibility**: Non-numeric data (text states, booleans) not supported

## Goals

- Store exact source payloads in Bronze layer
- Preserve platform metadata (ndp_id, context) as separate columns
- Move metric extraction to Silver layer ETL

## Non-Goals

- Silver layer ETL implementation (separate feature)
- Migration of existing Parquet files
- Changes to Grafana dashboard queries (deferred)

## Key Decisions

- [ADR-001: Bronze Raw JSON Schema](./architecture/ADR-001-bronze-raw-json-schema.md)

## Success Criteria

1. New `RawDataPoint` type implemented in core
2. Parquet storage writes 5-column schema
3. Sources emit raw payloads without parsing
5. Unit tests for new schema
