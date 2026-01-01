# AIR-011: Eliminate Duplicative Parser Processing

## Problem Statement

The current ingestion pipeline has a critical performance issue causing Pi lockups after several hours:

1. **Double Polling**: Two separate polling loops run simultaneously:
   - Source internal `polling_loop()` via `source.start()` - parses JSON into TimeSeriesPoints
   - SourceManager loop calling `fetch_raw_batch()` - stores raw JSON (actual Bronze layer path)

2. **Wasted Parser Work**: Parsers (JsonPath, ArrayIterator, ColumnOriented, FlatJson) are:
   - Created for each source
   - Invoked on every poll cycle
   - Parsing ~100KB JSON responses into 1000+ TimeSeriesPoints
   - Sending points to internal channels that are NEVER consumed
   - Causing memory pressure and eventual lockup

3. **Bronze Layer Reality**: The Bronze layer stores ENTIRE API responses raw - parsing is unnecessary.

## Objective

Eliminate parser processing from the ingestion path while preserving parsers for future Silver layer ETL.

## Success Criteria

- [ ] No parser code executed during HTTP polling/ingestion
- [ ] Single polling loop per source (no double polling)
- [ ] Parsers archived but accessible for Silver ETL
- [ ] Pi runs stable for 24+ hours without lockup
- [ ] Memory usage stable (no accumulation in unused channels)

## Out of Scope

- Silver layer ETL implementation (future DP-00x feature)
- New parser development
- Changes to Bronze storage format

## Stakeholders

- Platform stability
- Future ETL development
