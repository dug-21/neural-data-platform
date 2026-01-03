# dp-005: Bronze MCP Server - Test Fixtures

## Overview

This document specifies the test data, mock responses, and fixture files required for testing the Bronze MCP Server. All fixtures are designed to support both unit tests (via mocks) and integration tests (via actual files).

---

## 1. Sample Parquet Files

### Location

```
/core/ndp-mcp-server/tests/fixtures/
├── air-quality/
│   └── year=2026/month=01/day=03/data.parquet
├── outdoor-weather/
│   └── year=2026/month=01/day=03/data.parquet
├── sparse-stream/
│   └── year=2026/month=01/day=03/data.parquet
└── empty-stream/
    └── (no files)
```

### 1.1 air-quality.parquet

**Schema** (Bronze envelope):
```
timestamp: INT64 (microseconds since epoch)
source_id: STRING
ndp_id: STRING (nullable)
context: JSON STRING (nullable)
raw_payload: JSON STRING
year: INT32
month: INT32
day: INT32
```

**Sample Rows** (10 rows):
```json
[
  {
    "timestamp": 1767452639760716,
    "source_id": "air-quality-Mqtt",
    "ndp_id": "airgradient-office-001",
    "context": {"location": {"path": "office", "type": "indoor", "floor": 2}},
    "raw_payload": {
      "serialno": "ecda3b123456",
      "wifi": -62,
      "pm02": 8,
      "rco2": 742,
      "atmp": 22.7,
      "rhum": 45,
      "tvoc_index": 105,
      "nox_index": 12
    }
  },
  {
    "timestamp": 1767452039760716,
    "source_id": "air-quality-Mqtt",
    "ndp_id": "airgradient-office-001",
    "context": {"location": {"path": "office", "type": "indoor", "floor": 2}},
    "raw_payload": {
      "serialno": "ecda3b123456",
      "wifi": -58,
      "pm02": 12,
      "rco2": 856,
      "atmp": 22.5,
      "rhum": 46,
      "tvoc_index": 112,
      "nox_index": 15
    }
  },
  // ... 8 more rows with varying values
]
```

**File Size**: ~7KB (10 rows)

**Parquet Generation Code**:
```rust
use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;

fn generate_air_quality_fixture() -> Result<(), Box<dyn Error>> {
    let schema = Schema::new(vec![
        Field::new("timestamp", DataType::Int64, false),
        Field::new("source_id", DataType::Utf8, false),
        Field::new("ndp_id", DataType::Utf8, true),
        Field::new("context", DataType::Utf8, true),
        Field::new("raw_payload", DataType::Utf8, false),
        Field::new("year", DataType::Int32, false),
        Field::new("month", DataType::Int32, false),
        Field::new("day", DataType::Int32, false),
    ]);

    // ... build arrays and write to file
}
```

---

### 1.2 outdoor-weather.parquet

**Sample Rows** (10 rows):
```json
[
  {
    "timestamp": 1767452639760716,
    "source_id": "outdoor-weather-Http",
    "ndp_id": "weather-owm-002",
    "context": {
      "location": {
        "coordinates": [29.95838, -81.30878],
        "path": "beachhouse",
        "type": "outdoor"
      }
    },
    "raw_payload": {
      "coord": {"lon": -81.3088, "lat": 29.9584},
      "weather": [{"id": 803, "main": "Clouds", "description": "broken clouds", "icon": "04d"}],
      "base": "stations",
      "main": {
        "temp": 19.72,
        "feels_like": 19.55,
        "temp_min": 18.33,
        "temp_max": 21.11,
        "pressure": 1015,
        "humidity": 76,
        "sea_level": 1015,
        "grnd_level": 1014
      },
      "visibility": 10000,
      "wind": {"speed": 5.66, "deg": 220, "gust": 8.23},
      "clouds": {"all": 75},
      "dt": 1767452400,
      "sys": {"type": 2, "id": 2041216, "country": "US", "sunrise": 1767437256, "sunset": 1767475432},
      "timezone": -18000,
      "id": 4164138,
      "name": "Jacksonville Beach",
      "cod": 200
    }
  },
  // ... 9 more rows
]
```

**Notes**:
- `raw_payload` has nested structure (`main.temp`, `wind.speed`)
- Used for testing `describe_schema` mode=source
- Used for testing `validate_config` nested field detection

---

### 1.3 sparse-stream.parquet

**Purpose**: Test TC-SD-031 (n > available rows)

**Content**: Only 3 rows

```json
[
  {"timestamp": 1767452639760716, "source_id": "sparse-Http", "raw_payload": {"value": 1}},
  {"timestamp": 1767452039760716, "source_id": "sparse-Http", "raw_payload": {"value": 2}},
  {"timestamp": 1767451439760716, "source_id": "sparse-Http", "raw_payload": {"value": 3}}
]
```

---

## 2. Mock etcd Responses

### 2.1 Stream Configuration: air-quality

**etcd Keys**:
```
/streams/air-quality/stream_id → "air-quality"
/streams/air-quality/description → "AirGradient sensor readings from MQTT"
/streams/air-quality/version → "1.0.0"
/streams/air-quality/enabled → true
/streams/air-quality/sources/0/type → "mqtt"
/streams/air-quality/sources/0/params/topic → "airgradient/readings/+"
/streams/air-quality/sources/0/parser/type → "flat_json"
/streams/air-quality/sources/0/parser/field_mappings/0/source_path → "pm02"
/streams/air-quality/sources/0/parser/field_mappings/0/target_field → "pm25"
/streams/air-quality/sources/0/parser/field_mappings/1/source_path → "rco2"
/streams/air-quality/sources/0/parser/field_mappings/1/target_field → "co2"
/streams/air-quality/sources/0/parser/field_mappings/2/source_path → "atmp"
/streams/air-quality/sources/0/parser/field_mappings/2/target_field → "temperature"
/streams/air-quality/sources/0/parser/field_mappings/3/source_path → "rhum"
/streams/air-quality/sources/0/parser/field_mappings/3/target_field → "humidity"
/streams/air-quality/entity_schemas/0/schema_name → "airgradient"
/streams/air-quality/entity_schemas/0/attributes/0/name → "pm25"
/streams/air-quality/entity_schemas/0/attributes/0/type → "float"
/streams/air-quality/entity_schemas/0/attributes/0/unit → "ug/m3"
/streams/air-quality/entity_schemas/0/attributes/0/nullable → false
/streams/air-quality/entity_schemas/0/attributes/1/name → "co2"
/streams/air-quality/entity_schemas/0/attributes/1/type → "float"
/streams/air-quality/entity_schemas/0/attributes/1/unit → "ppm"
/streams/air-quality/entity_schemas/0/attributes/2/name → "temperature"
/streams/air-quality/entity_schemas/0/attributes/2/type → "float"
/streams/air-quality/entity_schemas/0/attributes/2/unit → "celsius"
/streams/air-quality/entity_schemas/0/attributes/3/name → "humidity"
/streams/air-quality/entity_schemas/0/attributes/3/type → "float"
/streams/air-quality/entity_schemas/0/attributes/3/unit → "percent"
```

**Parsed StreamConfig**:
```rust
StreamConfig {
    stream_id: "air-quality".to_string(),
    description: "AirGradient sensor readings from MQTT".to_string(),
    version: "1.0.0".to_string(),
    enabled: true,
    sources: vec![SourceConfig {
        source_type: "mqtt".to_string(),
        parser: ParserConfig {
            parser_type: "flat_json".to_string(),
            field_mappings: vec![
                FieldMapping { source_path: "pm02", target_field: "pm25" },
                FieldMapping { source_path: "rco2", target_field: "co2" },
                FieldMapping { source_path: "atmp", target_field: "temperature" },
                FieldMapping { source_path: "rhum", target_field: "humidity" },
            ],
        },
    }],
    entity_schemas: vec![EntitySchema {
        schema_name: "airgradient".to_string(),
        attributes: vec![
            Attribute { name: "pm25", attr_type: "float", unit: "ug/m3", nullable: false },
            Attribute { name: "co2", attr_type: "float", unit: "ppm", nullable: true },
            Attribute { name: "temperature", attr_type: "float", unit: "celsius", nullable: true },
            Attribute { name: "humidity", attr_type: "float", unit: "percent", nullable: true },
        ],
    }],
}
```

---

### 2.2 Stream Configuration: outdoor-weather

**Key Differences from air-quality**:
- Uses `http_poll` source type
- Uses `json_path` parser type
- Has nested source paths (`main.temp`, `wind.speed`)

**etcd Keys** (abbreviated):
```
/streams/outdoor-weather/stream_id → "outdoor-weather"
/streams/outdoor-weather/description → "Outdoor weather data from OpenWeatherMap"
/streams/outdoor-weather/enabled → true
/streams/outdoor-weather/sources/0/type → "http_poll"
/streams/outdoor-weather/sources/0/parser/type → "json_path"
/streams/outdoor-weather/sources/0/parser/field_mappings/0/source_path → "main.temp"
/streams/outdoor-weather/sources/0/parser/field_mappings/0/target_field → "temperature"
/streams/outdoor-weather/sources/0/parser/field_mappings/0/unit → "celsius"
/streams/outdoor-weather/sources/0/parser/field_mappings/1/source_path → "main.humidity"
/streams/outdoor-weather/sources/0/parser/field_mappings/1/target_field → "humidity"
/streams/outdoor-weather/sources/0/parser/field_mappings/2/source_path → "wind.speed"
/streams/outdoor-weather/sources/0/parser/field_mappings/2/target_field → "wind_speed"
/streams/outdoor-weather/entity_schemas/0/schema_name → "nws-weather"
/streams/outdoor-weather/entity_schemas/0/attributes/0/name → "temperature"
/streams/outdoor-weather/entity_schemas/0/attributes/1/name → "humidity"
/streams/outdoor-weather/entity_schemas/0/attributes/2/name → "wind_speed"
/streams/outdoor-weather/entity_schemas/0/attributes/3/name → "rain_1h"
/streams/outdoor-weather/entity_schemas/0/attributes/4/name → "snow_1h"
```

**Notes**:
- `rain_1h` and `snow_1h` in entity_schemas but NOT mapped from source
- Used for testing gap_analysis in describe_schema mode=all

---

### 2.3 Stream Configuration: nws-forecast-hourly

**Purpose**: Test disabled stream, no data file

```
/streams/nws-forecast-hourly/stream_id → "nws-forecast-hourly"
/streams/nws-forecast-hourly/description → "NWS hourly forecast data"
/streams/nws-forecast-hourly/enabled → false
/streams/nws-forecast-hourly/version → "1.0.0"
/streams/nws-forecast-hourly/sources/0/type → "http_poll"
```

---

### 2.4 Mock ConfigStore Implementation

```rust
pub struct MockConfigStore {
    configs: HashMap<String, StreamConfig>,
}

impl MockConfigStore {
    pub fn with_test_data() -> Self {
        let mut configs = HashMap::new();
        configs.insert("air-quality".to_string(), Self::air_quality_config());
        configs.insert("outdoor-weather".to_string(), Self::outdoor_weather_config());
        configs.insert("nws-forecast-hourly".to_string(), Self::nws_forecast_config());
        Self { configs }
    }

    fn air_quality_config() -> StreamConfig {
        StreamConfig {
            stream_id: "air-quality".to_string(),
            description: "AirGradient sensor readings from MQTT".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            sources: vec![SourceConfig {
                source_type: "mqtt".to_string(),
                parser: ParserConfig {
                    parser_type: "flat_json".to_string(),
                    field_mappings: vec![
                        FieldMapping::new("pm02", "pm25"),
                        FieldMapping::new("rco2", "co2"),
                        FieldMapping::new("atmp", "temperature"),
                        FieldMapping::new("rhum", "humidity"),
                    ],
                },
            }],
            entity_schemas: vec![EntitySchema {
                schema_name: "airgradient".to_string(),
                attributes: vec![
                    Attribute::float("pm25", "ug/m3", false),
                    Attribute::float("co2", "ppm", true),
                    Attribute::float("temperature", "celsius", true),
                    Attribute::float("humidity", "percent", true),
                ],
            }],
        }
    }

    // ... similar for other configs
}

#[async_trait]
impl ConfigStore for MockConfigStore {
    async fn get_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError> {
        self.configs.get(stream_id)
            .cloned()
            .ok_or_else(|| ConfigError::StreamNotFound(stream_id.to_string()))
    }

    async fn list_stream_ids(&self) -> Result<Vec<String>, ConfigError> {
        Ok(self.configs.keys().cloned().collect())
    }

    async fn health_check(&self) -> Result<(), ConfigError> {
        Ok(())
    }
}
```

---

## 3. Expected Tool Outputs

### 3.1 list_streams Expected Output

```json
{
  "success": true,
  "streams": [
    {
      "stream_id": "air-quality",
      "description": "AirGradient sensor readings from MQTT",
      "enabled": true,
      "version": "1.0.0",
      "sources": ["mqtt"],
      "storage": {
        "latest_partition": "year=2026/month=01/day=03",
        "file_size_bytes": 7310,
        "file_modified": "2026-01-03T14:54:00Z"
      }
    },
    {
      "stream_id": "outdoor-weather",
      "description": "Outdoor weather data from OpenWeatherMap",
      "enabled": true,
      "version": "1.0.0",
      "sources": ["http_poll"],
      "storage": {
        "latest_partition": "year=2026/month=01/day=03",
        "file_size_bytes": 12450,
        "file_modified": "2026-01-03T15:02:00Z"
      }
    },
    {
      "stream_id": "nws-forecast-hourly",
      "description": "NWS hourly forecast data",
      "enabled": false,
      "version": "1.0.0",
      "sources": ["http_poll"],
      "storage": null
    }
  ]
}
```

---

### 3.2 describe_schema Expected Outputs

**Mode: source (air-quality)**:
```json
{
  "success": true,
  "stream_id": "air-quality",
  "mode": "source",
  "raw_payload_structure": {
    "keys": ["serialno", "wifi", "pm02", "rco2", "atmp", "rhum", "tvoc_index", "nox_index"]
  },
  "parser_type": "flat_json",
  "field_mappings": [
    {"source_path": "pm02", "target_field": "pm25"},
    {"source_path": "rco2", "target_field": "co2"},
    {"source_path": "atmp", "target_field": "temperature"},
    {"source_path": "rhum", "target_field": "humidity"}
  ],
  "unmapped_source_fields": ["serialno", "wifi", "tvoc_index", "nox_index"],
  "file_analyzed": "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet"
}
```

**Mode: target (air-quality)**:
```json
{
  "success": true,
  "stream_id": "air-quality",
  "mode": "target",
  "entity_schema": "airgradient",
  "attributes": [
    {"name": "pm25", "type": "float", "unit": "ug/m3", "nullable": false},
    {"name": "co2", "type": "float", "unit": "ppm", "nullable": true},
    {"name": "temperature", "type": "float", "unit": "celsius", "nullable": true},
    {"name": "humidity", "type": "float", "unit": "percent", "nullable": true}
  ]
}
```

**Mode: all (outdoor-weather with gaps)**:
```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "all",
  "source": {
    "raw_payload_structure": {
      "keys": ["coord", "weather", "base", "main", "visibility", "wind", "clouds", "dt", "sys", "timezone", "id", "name", "cod"],
      "nested": {
        "main": ["temp", "feels_like", "temp_min", "temp_max", "pressure", "humidity", "sea_level", "grnd_level"],
        "wind": ["speed", "deg", "gust"],
        "coord": ["lon", "lat"],
        "clouds": ["all"],
        "sys": ["type", "id", "country", "sunrise", "sunset"]
      }
    },
    "parser_type": "json_path",
    "field_mappings": [
      {"source_path": "main.temp", "target_field": "temperature", "unit": "celsius"},
      {"source_path": "main.humidity", "target_field": "humidity", "unit": "percent"},
      {"source_path": "wind.speed", "target_field": "wind_speed", "unit": "m/s"}
    ]
  },
  "target": {
    "entity_schema": "nws-weather",
    "attributes": [
      {"name": "temperature", "type": "float", "unit": "celsius", "nullable": false},
      {"name": "humidity", "type": "float", "unit": "percent", "nullable": true},
      {"name": "wind_speed", "type": "float", "unit": "m/s", "nullable": true},
      {"name": "rain_1h", "type": "float", "unit": "mm", "nullable": true},
      {"name": "snow_1h", "type": "float", "unit": "mm", "nullable": true}
    ]
  },
  "gap_analysis": {
    "unmapped_source_fields": ["coord", "weather", "base", "main.feels_like", "main.temp_min", "main.temp_max", "main.pressure", "main.sea_level", "main.grnd_level", "visibility", "wind.deg", "wind.gust", "clouds.all", "dt", "sys", "timezone", "id", "name", "cod"],
    "target_fields_without_mapping": ["rain_1h", "snow_1h"]
  }
}
```

---

### 3.3 validate_config Expected Outputs

**Perfect match (hypothetical simple-weather)**:
```json
{
  "success": true,
  "stream_id": "simple-weather",
  "entity_schema": "simple-weather",
  "validation": {
    "status": "match",
    "config_fields": ["temperature", "humidity"],
    "raw_payload_fields": ["temperature", "humidity"],
    "analysis": {
      "in_config_not_in_payload": [],
      "in_payload_not_in_config": [],
      "matching": ["temperature", "humidity"]
    }
  }
}
```

**Nested mismatch (outdoor-weather)**:
```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "entity_schema": "nws-weather",
  "validation": {
    "status": "mismatch",
    "config_fields": ["temperature", "humidity", "wind_speed", "rain_1h", "snow_1h"],
    "raw_payload_fields": ["coord", "weather", "base", "main", "visibility", "wind", "clouds", "dt", "sys", "timezone", "id", "name", "cod"],
    "analysis": {
      "in_config_not_in_payload": ["temperature", "humidity", "wind_speed", "rain_1h", "snow_1h"],
      "in_payload_not_in_config": ["coord", "weather", "base", "main", "visibility", "wind", "clouds", "dt", "sys", "timezone", "id", "name", "cod"],
      "matching": []
    },
    "notes": "Config uses flattened field names; raw_payload preserves source structure (main.temp, wind.speed). Mapping happens in Silver layer via field_mappings."
  }
}
```

---

### 3.4 sample_data Expected Output

**Standard case (n=3)**:
```json
{
  "success": true,
  "stream_id": "air-quality",
  "row_count": 3,
  "rows": [
    {
      "timestamp": 1767452639760716,
      "source_id": "air-quality-Mqtt",
      "ndp_id": "airgradient-office-001",
      "context": {"location": {"path": "office", "type": "indoor", "floor": 2}},
      "raw_payload": {
        "serialno": "ecda3b123456",
        "wifi": -62,
        "pm02": 8,
        "rco2": 742,
        "atmp": 22.7,
        "rhum": 45,
        "tvoc_index": 105,
        "nox_index": 12
      }
    },
    {
      "timestamp": 1767452039760716,
      "source_id": "air-quality-Mqtt",
      "ndp_id": "airgradient-office-001",
      "context": {"location": {"path": "office", "type": "indoor", "floor": 2}},
      "raw_payload": {
        "serialno": "ecda3b123456",
        "wifi": -58,
        "pm02": 12,
        "rco2": 856,
        "atmp": 22.5,
        "rhum": 46,
        "tvoc_index": 112,
        "nox_index": 15
      }
    },
    {
      "timestamp": 1767451439760716,
      "source_id": "air-quality-Mqtt",
      "ndp_id": "airgradient-office-001",
      "context": {"location": {"path": "office", "type": "indoor", "floor": 2}},
      "raw_payload": {
        "serialno": "ecda3b123456",
        "wifi": -65,
        "pm02": 6,
        "rco2": 698,
        "atmp": 23.1,
        "rhum": 44,
        "tvoc_index": 98,
        "nox_index": 10
      }
    }
  ],
  "source_file": "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet"
}
```

---

## 4. Test Helper Functions

### 4.1 Fixture Generation

```rust
// /core/ndp-mcp-server/tests/helpers/fixtures.rs

use chrono::{TimeZone, Utc};
use serde_json::json;

/// Generate a test RawDataPoint for air-quality stream
pub fn air_quality_point(offset_minutes: i64, pm25: f64, co2: f64) -> RawDataPoint {
    let timestamp = Utc.with_ymd_and_hms(2026, 1, 3, 14, 54, 0)
        .unwrap()
        .checked_sub_signed(chrono::Duration::minutes(offset_minutes))
        .unwrap();

    RawDataPoint {
        timestamp,
        source_id: "air-quality-Mqtt".to_string(),
        ndp_id: Some("airgradient-office-001".to_string()),
        context: Some(json!({
            "location": {"path": "office", "type": "indoor", "floor": 2}
        })),
        raw_payload: json!({
            "serialno": "ecda3b123456",
            "wifi": -60,
            "pm02": pm25,
            "rco2": co2,
            "atmp": 22.5,
            "rhum": 45,
            "tvoc_index": 100,
            "nox_index": 12
        }),
    }
}

/// Generate a test RawDataPoint for outdoor-weather stream
pub fn outdoor_weather_point(offset_minutes: i64, temp: f64, humidity: f64) -> RawDataPoint {
    let timestamp = Utc.with_ymd_and_hms(2026, 1, 3, 15, 2, 0)
        .unwrap()
        .checked_sub_signed(chrono::Duration::minutes(offset_minutes))
        .unwrap();

    RawDataPoint {
        timestamp,
        source_id: "outdoor-weather-Http".to_string(),
        ndp_id: Some("weather-owm-002".to_string()),
        context: Some(json!({
            "location": {
                "coordinates": [29.95838, -81.30878],
                "path": "beachhouse",
                "type": "outdoor"
            }
        })),
        raw_payload: json!({
            "coord": {"lon": -81.3088, "lat": 29.9584},
            "weather": [{"id": 803, "main": "Clouds", "description": "broken clouds"}],
            "base": "stations",
            "main": {
                "temp": temp,
                "feels_like": temp - 0.5,
                "humidity": humidity,
                "pressure": 1015
            },
            "wind": {"speed": 5.66, "deg": 220, "gust": 8.23},
            "visibility": 10000,
            "dt": 1767452400,
            "name": "Jacksonville Beach",
            "cod": 200
        }),
    }
}
```

### 4.2 Parquet File Generation

```rust
// /core/ndp-mcp-server/tests/helpers/parquet.rs

use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

pub fn create_test_parquet(
    path: &Path,
    points: Vec<RawDataPoint>,
) -> Result<(), Box<dyn std::error::Error>> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("timestamp", DataType::Int64, false),
        Field::new("source_id", DataType::Utf8, false),
        Field::new("ndp_id", DataType::Utf8, true),
        Field::new("context", DataType::Utf8, true),
        Field::new("raw_payload", DataType::Utf8, false),
        Field::new("year", DataType::Int32, false),
        Field::new("month", DataType::Int32, false),
        Field::new("day", DataType::Int32, false),
    ]));

    let timestamps: Vec<i64> = points.iter()
        .map(|p| p.timestamp.timestamp_micros())
        .collect();
    let source_ids: Vec<&str> = points.iter()
        .map(|p| p.source_id.as_str())
        .collect();
    let ndp_ids: Vec<Option<&str>> = points.iter()
        .map(|p| p.ndp_id.as_deref())
        .collect();
    let contexts: Vec<Option<String>> = points.iter()
        .map(|p| p.context.as_ref().map(|c| c.to_string()))
        .collect();
    let payloads: Vec<String> = points.iter()
        .map(|p| p.raw_payload.to_string())
        .collect();

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(timestamps)),
            Arc::new(StringArray::from(source_ids)),
            Arc::new(StringArray::from(ndp_ids)),
            Arc::new(StringArray::from(contexts.iter().map(|c| c.as_deref()).collect::<Vec<_>>())),
            Arc::new(StringArray::from(payloads.iter().map(|s| s.as_str()).collect::<Vec<_>>())),
            Arc::new(Int32Array::from(vec![2026; points.len()])),
            Arc::new(Int32Array::from(vec![1; points.len()])),
            Arc::new(Int32Array::from(vec![3; points.len()])),
        ],
    )?;

    let file = File::create(path)?;
    let mut writer = ArrowWriter::try_new(file, schema, None)?;
    writer.write(&batch)?;
    writer.close()?;

    Ok(())
}
```

### 4.3 etcd Test Setup

```rust
// /core/ndp-mcp-server/tests/helpers/etcd.rs

use etcd_client::Client;
use std::collections::HashMap;

pub async fn seed_test_config(
    client: &mut Client,
    stream_id: &str,
    config: &StreamConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = format!("/streams/{}", stream_id);

    // Flatten config to etcd keys
    let kvs = flatten_config(&prefix, config);
    for (key, value) in kvs {
        client.put(key, value, None).await?;
    }

    Ok(())
}

pub async fn cleanup_test_config(
    client: &mut Client,
    stream_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = format!("/streams/{}/", stream_id);
    client.delete(prefix, Some(DeleteOptions::new().with_prefix())).await?;
    Ok(())
}

fn flatten_config(prefix: &str, config: &StreamConfig) -> Vec<(String, String)> {
    let mut kvs = Vec::new();
    kvs.push((format!("{}/stream_id", prefix), config.stream_id.clone()));
    kvs.push((format!("{}/description", prefix), config.description.clone()));
    kvs.push((format!("{}/enabled", prefix), config.enabled.to_string()));
    kvs.push((format!("{}/version", prefix), config.version.clone()));
    // ... flatten sources, entity_schemas, etc.
    kvs
}
```

---

## 5. Fixture File Locations Summary

| Fixture | Path | Purpose |
|---------|------|---------|
| air-quality.parquet | `tests/fixtures/air-quality/year=2026/month=01/day=03/data.parquet` | Standard Bronze data |
| outdoor-weather.parquet | `tests/fixtures/outdoor-weather/year=2026/month=01/day=03/data.parquet` | Nested JSON payload |
| sparse-stream.parquet | `tests/fixtures/sparse-stream/year=2026/month=01/day=03/data.parquet` | Only 3 rows |
| MockConfigStore | `tests/helpers/mock_config.rs` | In-memory config mocks |
| Parquet generator | `tests/helpers/parquet.rs` | Generate test Parquet files |
| etcd seeder | `tests/helpers/etcd.rs` | Seed/cleanup test etcd |

---

## Related Documents

- `test-plan.md` - Overall testing strategy
- `test-cases.md` - Detailed test case specifications
- `/core/src/types/raw_data_point.rs` - Bronze envelope schema definition
