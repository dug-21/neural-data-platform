# Implementation Launch Prompt: dp-023

## Proposed Prompt
> Implement dp-023: Text Field Pipeline (Bronze through Gold)
> GitHub Issue: #37
> Brief: product/features/dp-023/IMPLEMENTATION-BRIEF.md
> Pattern IDs from planning: 23 (ADR-001 jsonb coercion), 24 (ADR-002 timescale binding), 25 (ADR-003 gold text view), 26 (ADR-004 NWS forecast config), 27 (ADR-005 validation), 28 (ADR-006 data dictionary)
> Constraints: ARM64, config-driven, no hardcoded DDL, no DuckDB/Polars, no text processing (fe-005), apps/silver-etl/ is deprecated
> Wave structure: Wave 1 (Silver core: jsonb coercion + TimescaleOutput binding), Wave 2 (Config + DDL + Gold text view), Wave 3 (Validation + dictionary + integration tests)

## Reminders for User
- Review ALIGNMENT-REPORT.md -- no variances requiring approval (all PASS, Self-Learning is WARN-expected)
- Verify acceptance criteria in SCOPE.md
- Existing ADR pattern IDs 17-22 from prior planning also exist but are superseded by 23-28

## Gotchas Discovered During Planning
- coerce_to_type() wildcard `_ => Ok(value.clone())` accidentally handles jsonb -- explicit branch needed for correctness and testability
- TimescaleOutput uses raw SQL string substitution (build_raw_query), not parameterized queries -- JSONB needs `::jsonb` cast in the SQL template, not at the param level
- NWS forecast config has NO silver_etl section and NO stream_type field -- both must be added (not just silver_etl)
- detailedForecast is not in parser element_mappings -- must be added alongside silver_etl
- Gold text view uses unpivoted schema (source_stream, field_name, value) -- NOT one column per text field
- Data dictionary sync already handles text/jsonb types -- zero changes needed there
- DDL generator map_type() already handles text/jsonb -- zero changes needed there
- The Gold text view is a VIEW, not MATERIALIZED VIEW -- important for deploy.sh Phase 6 (no refresh policy needed)
