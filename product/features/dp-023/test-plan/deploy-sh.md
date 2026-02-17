# dp-023: deploy-sh Test Plan

## DDL Generator Tests

### Test: ddl_generator_text_column

```bash
# Generate Silver DDL for NWS forecast hourly config
# Expected: DDL includes "short_forecast TEXT" and "detailed_forecast TEXT"
source deploy/pi/ddl-generator.sh
output=$(generate_silver_ddl "nws-forecast-hourly" "full" 2>&1)
echo "$output" | grep -q "short_forecast TEXT"
echo "$output" | grep -q "detailed_forecast TEXT"
```

### Test: ddl_generator_mixed_columns

```bash
# Expected: DDL includes both numeric and text columns in same table
echo "$output" | grep -q "temperature_f DOUBLE PRECISION"
echo "$output" | grep -q "short_forecast TEXT"
```

### Test: ddl_generator_jsonb_column

```bash
# If a stream has jsonb field_mapping, verify JSONB column in DDL
# Setup: Stream config with type="jsonb" field
output=$(generate_silver_ddl "test-stream-with-jsonb" "full" 2>&1)
echo "$output" | grep -q "JSONB"
```

### Test: ddl_generator_existing_streams_unchanged

```bash
# Generate DDL for existing numeric-only streams
# Expected: Output identical to before dp-023 changes
for stream in air-quality indoor-air-quality; do
    output=$(generate_silver_ddl "$stream" "full" 2>&1)
    # Verify no TEXT or JSONB columns in numeric-only streams
    echo "$output" | grep -qv "TEXT" || echo "UNEXPECTED: TEXT in $stream DDL"
done
```

## Data Dictionary Sync Tests

### Test: dictionary_sync_text_columns

```bash
# After running sync on NWS forecast config, verify silver_columns entries
# Expected: short_forecast (TEXT) and detailed_forecast (TEXT) in silver_columns
psql -c "SELECT column_name, data_type FROM data_dictionary.silver_columns
          WHERE table_name = 'silver.nws_forecast_hourly'
          AND data_type = 'TEXT'"
# Should return 2 rows: short_forecast, detailed_forecast
```

### Test: dictionary_sync_lineage_entries

```bash
# Verify lineage entries for text fields
psql -c "SELECT silver_column, source_path FROM data_dictionary.silver_lineage
          WHERE silver_table = 'silver.nws_forecast_hourly'
          AND silver_column IN ('short_forecast', 'detailed_forecast')"
# Should return 2 rows with correct source_path values
```

### Test: dictionary_sync_existing_unchanged

```bash
# Verify existing silver_columns entries for numeric streams are unchanged
psql -c "SELECT COUNT(*) FROM data_dictionary.silver_columns
          WHERE table_name LIKE 'silver.air_quality%'
          AND data_type = 'DOUBLE PRECISION'"
# Count should match pre-dp-023 value
```

## Deploy.sh Phase 6 Tests

### Test: gold_text_view_created

```bash
# After deploy.sh Phase 6, verify Gold text view exists
psql -c "SELECT * FROM gold.indoor_air_quality_text LIMIT 1"
# Should succeed (empty result OK if no data yet)
```

### Test: gold_text_view_is_view_not_matview

```bash
# Verify it's a regular VIEW, not MATERIALIZED VIEW
psql -c "SELECT table_type FROM information_schema.views
          WHERE table_schema = 'gold' AND table_name = 'indoor_air_quality_text'"
# Should return 'VIEW'
```

## Summary

| Test | Type | AC Mapping | Priority |
|------|------|-----------|----------|
| ddl_generator_text_column | Shell | AC-03 | High |
| ddl_generator_mixed_columns | Shell | AC-04 | High |
| ddl_generator_jsonb_column | Shell | AC-02 | Medium |
| ddl_generator_existing_streams_unchanged | Regression | AC-08 | Critical |
| dictionary_sync_text_columns | Integration | AC-09 | High |
| dictionary_sync_lineage_entries | Integration | AC-09 | Medium |
| dictionary_sync_existing_unchanged | Regression | AC-08 | High |
| gold_text_view_created | Integration | AC-06 | High |
| gold_text_view_is_view_not_matview | Integration | AC-06 | Medium |
