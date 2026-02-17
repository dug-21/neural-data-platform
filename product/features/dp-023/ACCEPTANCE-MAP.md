# dp-023 Acceptance Criteria Map

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | TEXT field mapping accepted | shell | `ndp validate --config config/base/streams/nws-forecast-hourly/config.json` exits 0 with type="text" field | PENDING |
| AC-02 | JSONB field mapping accepted | shell | `ndp validate` on a config with type="jsonb" field exits 0 | PENDING |
| AC-03 | Silver table has TEXT column | shell | `psql -c "\d silver.nws_forecast_hourly"` shows `short_forecast text` and `detailed_forecast text` | PENDING |
| AC-04 | Silver table has numeric + text | shell | Same `\d` output shows both `temperature_f double precision` and `short_forecast text` | PENDING |
| AC-05 | NWS text ingested | shell | `psql -c "SELECT detailed_forecast FROM silver.nws_forecast_hourly LIMIT 5"` returns text rows | PENDING |
| AC-06 | Gold text view exists | shell | `psql -c "SELECT * FROM gold.indoor_air_quality_text LIMIT 5"` succeeds | PENDING |
| AC-07 | Gold text view is config-driven | grep | `grep -r "TextViewGenerator" crates/ndp-lib/src/gold/generators/text_view.rs` finds config-driven generator; no hardcoded stream names in generated SQL | PENDING |
| AC-08 | Existing numeric streams unaffected | test | `cargo test -p platform-core` (908 tests pass) AND `cargo test -p ndp-lib` (606 tests pass) AND `ndp validate` on all existing configs | PENDING |
| AC-09 | Data dictionary updated | shell | `psql -c "SELECT column_name, data_type FROM data_dictionary.silver_columns WHERE table_name='silver.nws_forecast_hourly' AND data_type='TEXT'"` returns 2 rows | PENDING |
| AC-10 | Grafana queryable | manual | Open Grafana SQL explorer, run `SELECT * FROM gold.indoor_air_quality_text LIMIT 10`, verify text columns visible | PENDING |
