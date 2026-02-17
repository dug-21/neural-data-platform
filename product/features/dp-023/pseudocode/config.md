# dp-023: config Pseudocode (NWS Forecast Stream Config)

## Component: config/base/streams/nws-forecast-hourly/config.json

### Change 1: Add stream_type field

**Location**: Top-level, after `stream_id` field

```json
{
    "stream_id": "nws-forecast-hourly",
    "stream_type": "forecast",
    ...
}
```

### Change 2: Add detailedForecast to parser element_mappings

**Location**: `sources[0].parser.array_config.element_mappings[]`
**Insert after**: The `shortForecast` mapping

```json
{
    "path": "detailedForecast",
    "metric_name": "detailed_forecast",
    "optional": true
}
```

### Change 3: Add silver_etl section

**Location**: Top-level, after `storage` section

```json
{
    "silver_etl": {
        "enabled": true,
        "target_table": "silver.nws_forecast_hourly",
        "timestamp": {
            "source_field": "timestamp",
            "target_field": "observation_time",
            "transform": "iso8601"
        },
        "identity_fields": [
            {
                "source": "ndp_id",
                "target": "ndp_id"
            }
        ],
        "deduplication": {
            "enabled": true,
            "strategy": "upsert",
            "key_columns": ["observation_time", "ndp_id"]
        },
        "field_mappings": [
            // Numeric fields
            { "source_path": "temperature", "target_column": "temperature_f",
              "type": "double_precision", "description": "Forecast temperature in Fahrenheit" },
            { "source_path": "dewpoint", "target_column": "dewpoint_c",
              "type": "double_precision", "description": "Forecast dew point in Celsius",
              "nullable": true },
            { "source_path": "relative_humidity", "target_column": "relative_humidity",
              "type": "double_precision", "description": "Relative humidity percentage",
              "nullable": true },
            { "source_path": "wind_speed", "target_column": "wind_speed_mph",
              "type": "double_precision", "description": "Wind speed in mph",
              "nullable": true },
            { "source_path": "wind_direction", "target_column": "wind_direction_deg",
              "type": "double_precision", "description": "Wind direction in degrees",
              "nullable": true },
            { "source_path": "probability_of_precipitation",
              "target_column": "probability_of_precipitation",
              "type": "double_precision", "description": "Precipitation probability %",
              "nullable": true },
            { "source_path": "forecast_issue_time", "target_column": "forecast_issue_time",
              "type": "double_precision",
              "description": "Forecast issue timestamp as epoch seconds",
              "nullable": true },

            // Text fields (NEW for dp-023)
            { "source_path": "short_forecast", "target_column": "short_forecast",
              "type": "text",
              "description": "Brief forecast description (e.g., Partly Cloudy)",
              "nullable": true },
            { "source_path": "detailed_forecast", "target_column": "detailed_forecast",
              "type": "text",
              "description": "Multi-sentence forecast narrative",
              "nullable": true }
        ],
        "dq_rules": [],
        "dq_output": {
            "enabled": false
        }
    }
}
```

### Design Notes

- **source_path matches metric_name** from parser element_mappings. The Silver subscriber reads from `record.fields` which are keyed by metric_name.
- **Text fields are nullable** because NWS API sometimes omits forecast text in edge cases.
- **No DQ rules for text** -- range_check and similar numeric rules do not apply. Text quality validation (if ever needed) is fe-005 territory.
- **No `valid_timestamp`** -- forecasts could use `startTime` as valid_timestamp for forecast-period alignment, but that is an enhancement beyond dp-023 scope.

## Summary of Changes

| File | Action | Description |
|------|--------|-------------|
| `config/base/streams/nws-forecast-hourly/config.json` | Modify | Add stream_type, detailedForecast parser mapping, silver_etl section |
