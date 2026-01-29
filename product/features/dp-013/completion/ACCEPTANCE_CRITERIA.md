# dp-013: CSV Source Type & Dimension Tables - Acceptance Criteria

**SPARC Phase**: Completion
**Version**: 1.0.0
**Last Updated**: 2026-01-29
**Status**: Ready for Implementation

---

## Final Acceptance Criteria

This document maps each acceptance criterion from the SCOPE.md and SPECIFICATION.md to verification methods and done definitions. Use this as the authoritative checklist during implementation and QA.

---

## Part 1: CSV Source Type

### AC-1.1: Source Type Recognition

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.1 |
| **Criterion** | `source.type: csv` recognized in stream configuration files |
| **Test Method** | Unit |
| **Test Case** | `test_validate_config_csv_source_type` |
| **Verification Steps** | 1. Create stream config YAML with `source.type: csv` 2. Parse config using StreamConfig parser 3. Verify `source_type` enum equals `SourceType::Csv` |
| **Done Definition** | Config parser returns `SourceType::Csv` without error; invalid source types rejected with descriptive error |

### AC-1.2: CSV Adapter Implementation

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.2 |
| **Criterion** | CSV adapter implemented following existing Source trait pattern |
| **Test Method** | Unit + Integration |
| **Test Case** | `test_csv_adapter_implements_source_trait` |
| **Verification Steps** | 1. Verify `CsvSource` implements `RawSource` trait 2. Verify `fetch()` method returns `Vec<RawDataPoint>` 3. Test with valid CSV file |
| **Done Definition** | CsvSource compiles, implements RawSource, can be instantiated and fetched from |

### AC-1.3: Timestamp Field Extraction

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.3 |
| **Criterion** | `timestamp_field` configuration extracts correct column |
| **Test Method** | Unit |
| **Test Case** | `test_timestamp_field_extracted` |
| **Verification Steps** | 1. Create CSV with column named per config 2. Parse with timestamp_field set 3. Verify timestamp value matches expected |
| **Done Definition** | Timestamp extracted from correct column; missing column produces clear error |

### AC-1.4: Timestamp Format - ISO8601

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.3 |
| **Criterion** | `timestamp_format: iso8601` parses ISO8601 timestamps correctly |
| **Test Method** | Unit |
| **Test Case** | `test_timestamp_iso8601_parsed` |
| **Verification Steps** | 1. Parse `2024-01-15T10:30:00Z` 2. Parse `2024-01-15T10:30:00.123Z` (with millis) 3. Parse `2024-01-15T10:30:00-05:00` (with timezone) |
| **Done Definition** | All ISO8601 variants parse to correct DateTime<Utc> |

### AC-1.5: Timestamp Format - Epoch

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.3 |
| **Criterion** | `timestamp_format: epoch_seconds` and `epoch_millis` parse correctly |
| **Test Method** | Unit |
| **Test Cases** | `test_timestamp_epoch_seconds_parsed`, `test_timestamp_epoch_millis_parsed` |
| **Verification Steps** | 1. Parse `1705315800` as epoch_seconds 2. Parse `1705315800000` as epoch_millis 3. Verify both produce same DateTime |
| **Done Definition** | Epoch timestamps convert to correct DateTime<Utc> |

### AC-1.6: Timestamp Format - Custom

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.3 |
| **Criterion** | Custom strftime format patterns supported |
| **Test Method** | Unit |
| **Test Case** | `test_timestamp_custom_format_parsed` |
| **Verification Steps** | 1. Configure `timestamp_format: "%Y/%m/%d %H:%M"` 2. Parse `2024/01/15 10:30` 3. Verify correct DateTime |
| **Done Definition** | Custom strftime patterns parse correctly; invalid patterns produce clear error |

### AC-1.7: Column Mapping via entity_schemas

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.4 |
| **Criterion** | Column mapping uses existing `entity_schemas` pattern |
| **Test Method** | Unit |
| **Test Case** | `test_entity_schema_column_mapping` |
| **Verification Steps** | 1. Configure `source_field: temp_c` mapping to `name: temperature` 2. Parse CSV with `temp_c` column 3. Verify RawDataPoint contains `temperature` field |
| **Done Definition** | Source columns map to target names per entity_schemas; unmapped columns preserved in raw_payload |

### AC-1.8: Bronze Parquet Output

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.5 |
| **Criterion** | Data lands in Bronze Parquet with same format as HTTP/MQTT sources |
| **Test Method** | Integration |
| **Test Case** | `test_csv_source_creates_parquet` |
| **Verification Steps** | 1. Ingest CSV to Bronze 2. Read resulting Parquet 3. Verify schema matches MQTT-sourced Parquet 4. Verify `raw_payload` contains full CSV row as JSON |
| **Done Definition** | Parquet files created with identical schema to existing sources; readable by Bronze ETL |

### AC-1.9: Silver ETL Compatibility

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.5 |
| **Criterion** | Normal ETL promotes CSV-sourced data to Silver |
| **Test Method** | Integration |
| **Test Case** | `test_csv_data_promoted_to_silver` |
| **Verification Steps** | 1. Ingest CSV to Bronze 2. Run silver-etl 3. Verify data in Silver table with correct types |
| **Done Definition** | CSV-sourced Bronze data promoted to Silver identically to other sources |

### AC-1.10: Error Handling - Skip Mode

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.8, FR-5.3 |
| **Criterion** | Invalid rows logged and skipped when `on_error: skip` |
| **Test Method** | Unit + Integration |
| **Test Case** | `test_invalid_row_skipped_with_log` |
| **Verification Steps** | 1. Create CSV with malformed row in middle 2. Set `on_error: skip` 3. Run ingest 4. Verify good rows processed, bad row logged with line number |
| **Done Definition** | Processing continues past errors; error log includes line number and field context |

### AC-1.11: Error Handling - Abort Mode

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-1.8 |
| **Criterion** | Processing stops on first error when `on_error: abort` |
| **Test Method** | Unit |
| **Test Case** | `test_on_error_abort_stops` |
| **Verification Steps** | 1. Create CSV with malformed row 2. Set `on_error: abort` 3. Run ingest 4. Verify processing stops, error reported |
| **Done Definition** | First error stops processing; partial data written up to last complete batch |

### AC-1.12: CLI Stream Ingest

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.1 (implied for streams) |
| **Criterion** | `ndp stream ingest <stream_id>` triggers CSV ingest |
| **Test Method** | Integration |
| **Test Case** | `test_cli_stream_ingest_csv` |
| **Verification Steps** | 1. Create stream config with `source.type: csv` 2. Run `ndp stream ingest historical-aq` 3. Verify data in Bronze |
| **Done Definition** | CLI command executes, returns summary stats, data appears in Bronze |

---

## Part 2: Dimension Table Configs

### AC-2.1: Dimension Config Schema

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.2 |
| **Criterion** | Dimension config schema defined with required sections |
| **Test Method** | Unit |
| **Test Case** | `test_dimension_config_deserializes` |
| **Verification Steps** | 1. Create YAML with dimension_id, target, source, schema, load 2. Deserialize to DimensionConfig struct 3. Verify all fields populated |
| **Done Definition** | DimensionConfig struct parses valid YAML; missing required fields produce validation error |

### AC-2.2: Config File Discovery

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.1 |
| **Criterion** | Config files discovered in `config/base/dimensions/*.yaml` |
| **Test Method** | Integration |
| **Test Case** | `test_dimension_configs_loaded_from_dir` |
| **Verification Steps** | 1. Place multiple .yaml files in dimensions/ 2. Call config discovery 3. Verify all files found |
| **Done Definition** | All .yaml files in directory discovered; non-.yaml files ignored |

### AC-2.3: CSV Source for Dimensions

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.1 |
| **Criterion** | CSV source type for dimensions (path, delimiter, encoding) |
| **Test Method** | Unit + Integration |
| **Test Case** | `test_dimension_csv_source_parsed` |
| **Verification Steps** | 1. Configure dimension with `source.type: csv` 2. Set path, delimiter, encoding 3. Load dimension 4. Verify CSV read correctly |
| **Done Definition** | Dimension CSV parsed with configured settings; defaults work (comma, utf-8) |

### AC-2.4: Schema Validation - Required Fields

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.3 |
| **Criterion** | Schema validation enforces required fields |
| **Test Method** | Unit |
| **Test Case** | `test_dimension_required_field_enforced` |
| **Verification Steps** | 1. Create CSV missing required column 2. Attempt load 3. Verify validation error before any DB operation |
| **Done Definition** | Missing required field produces clear error with field name; no database changes |

### AC-2.5: Schema Validation - Data Types

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.3 |
| **Criterion** | Schema validation validates data types |
| **Test Method** | Unit |
| **Test Case** | `test_dimension_type_validation` |
| **Verification Steps** | 1. Create CSV with string in float field 2. Attempt load 3. Verify type conversion error with context |
| **Done Definition** | Type mismatches produce error with field name, expected type, and actual value |

### AC-2.6: Truncate and Load Strategy

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.4, NFR-2.1 |
| **Criterion** | `truncate_and_load` executes DELETE + INSERT in transaction |
| **Test Method** | Integration |
| **Test Case** | `test_truncate_and_load_replaces_data` |
| **Verification Steps** | 1. Load dimension with 10 rows 2. Load again with 5 different rows 3. Verify table has exactly 5 rows (old removed) |
| **Done Definition** | Table contains only new data; old data completely removed; atomic transaction |

### AC-2.7: Upsert Strategy

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.5 |
| **Criterion** | `upsert` executes INSERT ON CONFLICT UPDATE |
| **Test Method** | Integration |
| **Test Cases** | `test_upsert_updates_or_inserts`, `test_dimension_upsert_mixed_insert_update` |
| **Verification Steps** | 1. Load dimension with rows A, B 2. Upsert with modified A and new C 3. Verify A updated, B unchanged, C added |
| **Done Definition** | Existing rows updated by primary key; new rows inserted; unmatched rows preserved |

### AC-2.8: Transaction Rollback

| Field | Value |
|-------|-------|
| **Requirement ID** | NFR-2.2 |
| **Criterion** | Database state rolls back on failure |
| **Test Method** | Integration |
| **Test Case** | `test_dimension_truncate_load_atomic` |
| **Verification Steps** | 1. Load dimension successfully 2. Attempt load with error-producing CSV 3. Verify original data preserved |
| **Done Definition** | Failed load leaves table in original state; no partial data |

### AC-2.9: Auto-Create Table

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-2.6 |
| **Criterion** | Silver table auto-created from dimension schema if not exists |
| **Test Method** | Integration |
| **Test Case** | `test_dimension_table_created_if_missing` |
| **Verification Steps** | 1. Ensure target table does not exist 2. Run dimension sync 3. Verify table created with correct schema |
| **Done Definition** | CREATE TABLE IF NOT EXISTS executes; columns match schema; primary key set |

### AC-2.10: deploy.sh Integration

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-4.1 |
| **Criterion** | `deploy.sh sync` processes dimension configs |
| **Test Method** | Integration |
| **Test Case** | `test_deploy_sync_loads_dimensions` |
| **Verification Steps** | 1. Place dimension configs in config/base/dimensions/ 2. Run `./deploy.sh sync` 3. Verify dimensions loaded to Silver |
| **Done Definition** | All enabled dimensions synced; summary shows success/failure counts |

---

## Part 3: CLI

### AC-3.1: Dimension List Command

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.1 |
| **Criterion** | `ndp dimension list` shows configured dimensions |
| **Test Method** | Integration |
| **Test Case** | `test_cli_dimension_list_output` |
| **Verification Steps** | 1. Configure multiple dimensions 2. Run `ndp dimension list` 3. Verify all dimensions displayed with status |
| **Done Definition** | Output shows dimension_id, status (enabled/disabled), target table, last sync time |

### AC-3.2: Dimension Sync Specific

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.2 |
| **Criterion** | `ndp dimension sync <id>` loads specific dimension |
| **Test Method** | Integration |
| **Test Case** | `test_cli_dimension_sync_single` |
| **Verification Steps** | 1. Configure multiple dimensions 2. Run `ndp dimension sync entity-context` 3. Verify only entity-context loaded |
| **Done Definition** | Only specified dimension synced; others unchanged |

### AC-3.3: Dimension Sync All

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.3 |
| **Criterion** | `ndp dimension sync --all` loads all enabled dimensions |
| **Test Method** | Integration |
| **Test Case** | `test_cli_dimension_sync_all` |
| **Verification Steps** | 1. Configure multiple dimensions (some enabled, some disabled) 2. Run `ndp dimension sync --all` 3. Verify all enabled dimensions loaded |
| **Done Definition** | All enabled dimensions synced; disabled dimensions skipped |

### AC-3.4: Dry Run Mode

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.4 |
| **Criterion** | `--dry-run` validates without side effects |
| **Test Method** | Integration |
| **Test Case** | `test_cli_dimension_dry_run_no_changes` |
| **Verification Steps** | 1. Run `ndp dimension sync entity-context --dry-run` 2. Query database 3. Verify no changes made |
| **Done Definition** | Validation runs; output shows what would happen; database unchanged |

### AC-3.5: Summary Output

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.5 |
| **Criterion** | Summary output shows rows processed, loaded, errors |
| **Test Method** | Unit + Integration |
| **Test Case** | `test_cli_output_format` |
| **Verification Steps** | 1. Run dimension sync 2. Capture output 3. Verify summary line format |
| **Done Definition** | Output includes: "Rows: N processed, N loaded, N errors. Duration: Xs" |

### AC-3.6: Exit Code Success

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.6 |
| **Criterion** | Exit code 0 on success |
| **Test Method** | Integration |
| **Test Case** | `test_cli_exit_code_success` |
| **Verification Steps** | 1. Run successful dimension sync 2. Check exit code |
| **Done Definition** | Exit code is 0 |

### AC-3.7: Exit Code Failure

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-3.6 |
| **Criterion** | Exit code non-zero on failure |
| **Test Method** | Integration |
| **Test Case** | `test_cli_exit_code_failure` |
| **Verification Steps** | 1. Run dimension sync with invalid config 2. Check exit code |
| **Done Definition** | Exit code > 0; specific codes per error type (1-5 as documented) |

---

## Part 4: Error Handling

### AC-4.1: Malformed CSV Error

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-5.1 |
| **Criterion** | Parse error with line number for malformed CSV |
| **Test Method** | Unit |
| **Test Case** | `test_malformed_csv_parse_error` |
| **Verification Steps** | 1. Create CSV with unclosed quote on line 47 2. Attempt parse 3. Verify error includes "line 47" |
| **Done Definition** | Error message includes: file path, line number, error description |

### AC-4.2: Missing Required Columns

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-5.2 |
| **Criterion** | Validation error lists missing columns |
| **Test Method** | Unit |
| **Test Case** | `test_missing_column_validation_error` |
| **Verification Steps** | 1. Create CSV missing `ndp_id` column 2. Attempt dimension load 3. Verify error lists missing column name |
| **Done Definition** | Error message includes: "Missing required column(s): ndp_id, category" |

### AC-4.3: Type Conversion Errors

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-5.3 |
| **Criterion** | Type conversion errors include context |
| **Test Method** | Unit |
| **Test Case** | `test_type_conversion_error_handling` |
| **Verification Steps** | 1. Create CSV with "abc" in float field 2. Attempt parse 3. Verify error shows column, value, expected type |
| **Done Definition** | Error: "Type conversion failed: field 'temperature', value 'abc', expected float" |

### AC-4.4: File Not Found

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-5.4 |
| **Criterion** | File not found error shows full path |
| **Test Method** | Unit |
| **Test Case** | `test_file_not_found_error` |
| **Verification Steps** | 1. Configure non-existent file path 2. Attempt load 3. Verify error includes full path |
| **Done Definition** | Error: "File not found: /full/path/to/missing.csv" |

### AC-4.5: Empty File Warning

| Field | Value |
|-------|-------|
| **Requirement ID** | FR-5.5 |
| **Criterion** | Empty file produces warning, not error |
| **Test Method** | Unit |
| **Test Case** | `test_empty_file_warning` |
| **Verification Steps** | 1. Create CSV with header only (no data rows) 2. Run dimension load 3. Verify warning logged, exit 0 |
| **Done Definition** | Warning logged: "Empty file: no data rows"; load completes with 0 rows; exit code 0 |

---

## Implementation Checklist

Use this checklist to track implementation progress. Mark items complete only when associated acceptance criteria pass.

### Core Types

- [ ] Extend `SourceType` enum with `Csv` variant (`core/src/types/stream_config.rs`)
- [ ] Add `CsvConfig` struct for CSV source configuration
- [ ] Create `DimensionConfig` struct (`core/src/types/dimension_config.rs`)
- [ ] Create `DimensionTarget`, `DimensionSource`, `DimensionSchema` structs
- [ ] Create `LoadStrategy` enum (TruncateAndLoad, Upsert)

### CSV Source Adapter

- [ ] Implement `CsvSource` struct
- [ ] Implement `RawSource` trait for `CsvSource`
- [ ] Implement timestamp parsing (iso8601, epoch_seconds, epoch_millis, custom)
- [ ] Implement column mapping via entity_schemas
- [ ] Implement error handling (skip/abort modes)
- [ ] Integrate with SourceManager dispatch

### Dimension Loader

- [ ] Implement `DimensionLoader` struct
- [ ] Implement CSV reader for dimensions
- [ ] Implement schema validation
- [ ] Implement `truncate_and_load` strategy
- [ ] Implement `upsert` strategy
- [ ] Implement auto-create table from schema
- [ ] Implement transaction wrapping

### CLI Commands

- [ ] Add `ndp dimension list` command
- [ ] Add `ndp dimension sync <id>` command
- [ ] Add `ndp dimension sync --all` command
- [ ] Add `--dry-run` flag
- [ ] Add `ndp stream ingest <stream_id>` command
- [ ] Implement summary output formatting
- [ ] Implement exit codes (0-5 per spec)

### deploy.sh Integration

- [ ] Add `sync_dimensions()` function to deploy.sh
- [ ] Add TimescaleDB readiness check
- [ ] Add dimension sync summary to deploy output

### Tests

- [ ] Unit tests for CSV parsing (all edge cases)
- [ ] Unit tests for timestamp parsing (all formats)
- [ ] Unit tests for column mapping
- [ ] Unit tests for schema validation
- [ ] Unit tests for load strategy logic
- [ ] Integration tests for CSV -> Bronze flow
- [ ] Integration tests for Bronze -> Silver ETL
- [ ] Integration tests for dimension truncate_and_load
- [ ] Integration tests for dimension upsert
- [ ] Integration tests for CLI commands
- [ ] Integration tests for deploy.sh sync
- [ ] Error scenario tests (malformed, missing, type errors)
- [ ] Test fixtures created

### Documentation

- [ ] README updates for CSV source type
- [ ] README updates for dimension tables
- [ ] CLI help text for new commands
- [ ] Example configs in config/base/dimensions/

### Initial Deliverable

- [ ] Entity Context dimension config created (`config/base/dimensions/entity_context.yaml`)
- [ ] Entity Context CSV data file created (`config/dimensions/entity_context.csv`)
- [ ] Entity Context table created in Silver
- [ ] gold.events_with_context view created
- [ ] Entity Context working for air-012 Home Assistant integration

---

## Definition of Done

dp-013 is considered **COMPLETE** when ALL of the following are true:

### Functional Completeness

1. **All acceptance criteria verified** - Every AC in this document has passed its verification steps
2. **Unit tests passing** - `cargo test -p silver-etl --lib` passes with >80% coverage on CSV parsing
3. **Integration tests passing** - `cargo test -p silver-etl --test integration_tests -- --ignored` passes
4. **CLI functional** - All `ndp dimension` and `ndp stream ingest` commands work as specified

### Deployment Verification

5. **deploy.sh sync includes dimensions** - Running `./deploy.sh sync` processes dimension configs
6. **Entity Context dimension working** - `silver.entity_context` table populated from CSV
7. **gold.events_with_context view works** - Query returns enriched events for air-012

### Documentation and Quality

8. **Patterns saved to AgentDB** - New patterns discovered during implementation stored via `/save-pattern`
9. **STATUS.md updated** - All SPARC phases marked complete
10. **COMPLETION.md summary** - Summary of implementation with any deviations from spec

### Sign-off

| Verification | Verified By | Date |
|--------------|-------------|------|
| All unit tests pass | | |
| All integration tests pass | | |
| CLI commands functional | | |
| deploy.sh sync works | | |
| Entity Context dimension loaded | | |
| gold view returns data | | |
| Documentation updated | | |
| Patterns saved | | |

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-01-29 | ndp-scrum-master | Initial acceptance criteria |
