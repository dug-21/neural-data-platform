# dp-023: config Test Plan (NWS Forecast Stream Config)

## Config Validation Tests

### Test: nws_forecast_config_validates

```bash
# After adding silver_etl and stream_type, config must pass validation
ndp validate --config config/base/streams/nws-forecast-hourly/config.json
# Expected: PASS
```

### Test: nws_forecast_parser_has_detailed_forecast

```bash
# Verify detailedForecast is in parser element_mappings
jq '.sources[0].parser.array_config.element_mappings[] | select(.metric_name == "detailed_forecast")' \
    config/base/streams/nws-forecast-hourly/config.json
# Expected: Returns the mapping object
```

### Test: nws_forecast_has_stream_type

```bash
# Verify stream_type field exists
jq '.stream_type' config/base/streams/nws-forecast-hourly/config.json
# Expected: "forecast"
```

### Test: nws_forecast_silver_etl_complete

```bash
# Verify silver_etl has all required sections
jq '.silver_etl | keys' config/base/streams/nws-forecast-hourly/config.json
# Expected: ["deduplication", "dq_output", "dq_rules", "enabled", "field_mappings",
#            "identity_fields", "target_table", "timestamp"]
```

### Test: nws_forecast_text_field_mappings

```bash
# Verify text field_mappings exist
jq '.silver_etl.field_mappings[] | select(.type == "text") | .target_column' \
    config/base/streams/nws-forecast-hourly/config.json
# Expected: "short_forecast" and "detailed_forecast"
```

### Test: nws_forecast_numeric_field_mappings_preserved

```bash
# Verify numeric field_mappings exist alongside text
jq '.silver_etl.field_mappings[] | select(.type == "double_precision") | .target_column' \
    config/base/streams/nws-forecast-hourly/config.json
# Expected: temperature_f, dewpoint_c, relative_humidity, wind_speed_mph,
#           wind_direction_deg, probability_of_precipitation, forecast_issue_time
```

## Summary

| Test | Type | AC Mapping | Priority |
|------|------|-----------|----------|
| nws_forecast_config_validates | CLI | AC-01, AC-04 | High |
| nws_forecast_parser_has_detailed_forecast | Shell | AC-05 | High |
| nws_forecast_has_stream_type | Shell | AC-04 | Medium |
| nws_forecast_silver_etl_complete | Shell | AC-04 | Medium |
| nws_forecast_text_field_mappings | Shell | AC-01 | High |
| nws_forecast_numeric_field_mappings_preserved | Shell | AC-04, AC-08 | High |
