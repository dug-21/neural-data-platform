# How to Add a New Data Stream

**Document Type**: Procedure
**Version**: 1.0.0
**Last Updated**: 2025-12-16
**Applies To**: Neural Data Platform v1.x

---

## Overview

This guide explains how to add a new data stream to the Neural Data Platform. A "stream" is a logical grouping of related time-series data with its own schema, sources, and storage configuration (e.g., "air-quality", "weather", "home-events").

### Prerequisites

- Running Pi deployment with etcd
- Understanding of YAML configuration
- Access to the data source you want to ingest

### Time Estimate

- **Simple Stream** (single MQTT source, simple schema): 30 minutes
- **Complex Stream** (multiple sources, validation rules): 1-2 hours

---

## Architecture Context

### Stream Registry

Streams are defined in etcd under the `/streams/{stream-id}/` prefix:

```
/streams/
├── air-quality/
│   ├── config        → Stream metadata (retention, compression)
│   ├── schema        → Field definitions (currently embedded in config)
│   └── sources       → Source configurations (MQTT, HTTP, etc.)
├── weather/
│   ├── config
│   └── sources
└── home-events/
    ├── config
    └── sources
```

### StreamConfig Structure

```rust
pub struct StreamConfig {
    pub stream_id: String,           // Unique ID (kebab-case)
    pub description: String,         // Human-readable
    pub version: String,             // Semver
    pub enabled: bool,
    pub retention_days: u32,
    pub compression_after_days: u32,
    pub partitioning_strategy: String,
    pub fields: Vec<SchemaField>,    // Schema
    pub sources: Vec<SourceConfig>,  // Data sources
}
```

---

## Step-by-Step Procedure

### Step 1: Design Your Schema

Determine what fields your stream will have:

| Field | Type | Unit | Required | Range |
|-------|------|------|----------|-------|
| temperature | float | celsius | Yes | [-50, 100] |
| humidity | float | percent | Yes | [0, 100] |
| pressure | float | hPa | No | [800, 1200] |
| conditions | string | - | No | - |

### Step 2: Create Stream Configuration Directory

Create the stream config directory in the GitOps structure:

```bash
# From repository root
mkdir -p config/base/streams/weather
```

### Step 3: Create config.yaml

**File**: `config/base/streams/weather/config.yaml`

```yaml
stream_id: weather
description: Outdoor weather conditions from weather station
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: temperature
    type: float
    unit: celsius
    nullable: false
    range: [-50, 100]
    display_precision: 1
    description: Outdoor temperature

  - name: humidity
    type: float
    unit: percent
    nullable: false
    range: [0, 100]
    display_precision: 0
    description: Relative humidity

  - name: pressure
    type: float
    unit: hPa
    nullable: true
    range: [800, 1200]
    display_precision: 1
    description: Atmospheric pressure

  - name: wind_speed
    type: float
    unit: m/s
    nullable: true
    range: [0, 100]
    display_precision: 1

  - name: conditions
    type: string
    nullable: true
    description: Weather conditions text

sources:
  - type: mqtt
    enabled: true
    topic: weather/station/+
    qos: 1
```

### Step 4: Validate Stream ID Format

Stream IDs must follow these rules:
- **Length**: 3-64 characters
- **Format**: kebab-case (lowercase letters, digits, hyphens)
- **Start**: Must start with a lowercase letter
- **Examples**: `air-quality`, `home-events`, `power-usage`, `sensor-data-1`

Invalid examples:
- `AirQuality` (uppercase)
- `air_quality` (underscore)
- `ab` (too short)
- `2stream` (starts with digit)

### Step 5: Validate Field Names

Field names must follow these rules:
- **Length**: 1-64 characters
- **Format**: snake_case (lowercase letters, digits, underscores)
- **Start**: Must start with a lowercase letter
- **Examples**: `pm25`, `temperature`, `event_type`, `sensor_id`

### Step 6: Create Stream Configuration Directory (GitOps Pattern)

Stream configurations are managed via GitOps YAML files and automatically synced to etcd.

**Create configuration in the GitOps structure:**

```bash
# From repository root
mkdir -p config/base/streams/weather
```

**File location:** `config/base/streams/weather/config.yaml`

> **Note**: The configuration file should follow the same schema as shown in Step 3.

### Step 7: Sync Configuration to etcd

The platform uses a GitOps sync mechanism. Configurations are automatically synced from YAML files to etcd.

**Manual sync (development/testing):**
```bash
# From repository root
cd /workspaces/neural-data-platform

# Sync all stream configurations to etcd
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production
```

**Via deployment script:**
```bash
# From deployment directory
cd deploy/pi

# Sync configurations
./deploy.sh sync

# Or initialize streams during first deployment
./deploy.sh init-streams
```

**Automatic sync:**
- On application startup, `ConfigSyncService` automatically discovers and syncs all YAML configs in `config/base/streams/`
- Configs are validated before being stored in etcd
- Invalid configs are skipped with warnings logged

### Step 8: Verify Stream Registration

```bash
# List all registered streams (keys are flattened)
docker exec etcd etcdctl get --prefix /streams/ --keys-only

# Check specific stream configuration
docker exec etcd etcdctl get --prefix /streams/weather/ --keys-only

# Verify via deployment script
cd deploy/pi && ./deploy.sh list-streams

# Check application logs for sync confirmation
docker logs air-quality-app 2>&1 | grep -i "synced\|stream"
```

**Expected log output on successful sync:**
```
INFO  Synced 3 stream configs to registry
INFO  Registered streams: ["air-quality", "outdoor-weather", "outdoor-air-quality"]
```

### Step 9: Configure Data Source

For MQTT sources, ensure your data publisher uses the correct topic:

```python
# Example: Weather station publishing
import paho.mqtt.client as mqtt
import json

client = mqtt.Client()
client.connect("mosquitto", 1883)

data = {
    "temperature": 22.5,
    "humidity": 65.0,
    "pressure": 1013.25,
    "conditions": "partly cloudy"
}

client.publish("weather/station/outdoor", json.dumps(data))
```

### Step 10: Verify Data Flow

```bash
# Subscribe to MQTT topic to see incoming data
docker exec mqtt-broker mosquitto_sub -t "weather/station/#" -v

# Check storage (if stream handling is implemented)
docker exec air-quality-app ls -la /app/data/weather/

# Check application logs
docker logs air-quality-app | grep -i weather
```

---

## Stream Configuration Reference

### Complete StreamConfig Example

```yaml
# Full configuration with all options
stream_id: home-events
description: Discrete home activity events for correlation analysis
version: "1.0.0"
enabled: true

# Storage settings
retention_days: 730        # 2 years
compression_after_days: 30
partitioning_strategy: daily

# Schema definition
fields:
  - name: event_type
    type: string
    nullable: false
    description: Type of event (window_state, hvac_mode, occupancy)

  - name: target
    type: string
    nullable: false
    description: Target of event (front_window, living_room, etc)

  - name: state
    type: string
    nullable: true
    description: New state value

  - name: previous_state
    type: string
    nullable: true
    description: Previous state (for transitions)

  - name: metadata
    type: json
    nullable: true
    description: Additional event-specific metadata

# Data sources
sources:
  # MQTT source for automated events
  - type: mqtt
    enabled: true
    topic: home/events/#
    qos: 1

  # Webhook for manual event logging
  - type: webhook
    enabled: true
    path: /api/events
    auth:
      type: bearer
      token_env: EVENTS_API_TOKEN

# Storage overrides (optional)
storage:
  batch_size: 50
  batch_timeout_secs: 10
  buffer_capacity: 500
```

### Field Types

| Type | Description | Supports Range | Supports Precision |
|------|-------------|----------------|-------------------|
| `float` | Floating point number | Yes | Yes |
| `int` | Integer | Yes | No |
| `string` | Text | No | No |
| `bool` | Boolean | No | No |
| `json` | JSON object/array | No | No |

### Source Types

| Type | Pattern | Use Case |
|------|---------|----------|
| `mqtt` | Push | Sensors, IoT devices |
| `http_poll` | Poll | External APIs |
| `webhook` | Push | Manual triggers, integrations |
| `file_watch` | Trigger | CSV imports, log files |

### Parser Types

Parsers extract data from different JSON structures. Choose based on your data format:

| Parser Type | When to Use | Data Structure | Example |
|-------------|-------------|----------------|---------|
| `flat_json` | Simple MQTT/flat JSON | Single observation, no nesting | AirGradient sensors |
| `json_path` | Nested JSON, single observation | API response with nested fields | OpenWeatherMap current weather |
| `array_iterator` | JSON arrays, multiple observations | API returns array of time periods | NWS hourly forecast (156 periods) |

#### flat_json Parser

**Use for:** Simple JSON payloads without nesting (typical MQTT sensor data)

**Configuration:**
```yaml
parser:
  parser_type: flat_json
  location_id_field: serialno  # Field containing location identifier
  skip_fields: [metadata1, metadata2]  # Fields to exclude from storage
  default_tags:
    source: mqtt
```

**Example Input:**
```json
{"serialno": "84fce612f684", "pm25": 12.3, "co2": 654, "temperature": 22.5}
```

**Output:** Direct 1:1 mapping from JSON keys to metrics

#### json_path Parser

**Use for:** Nested JSON structures with a single observation

**Configuration:**
```yaml
parser:
  parser_type: json_path
  location_id_field: name  # JSONPath for location
  timestamp_field: properties.timestamp  # Optional: extract timestamp from response
  timestamp_format: iso8601
  field_mappings:
    - path: main.temp  # JSONPath expression
      metric_name: temperature
      unit: celsius
    - path: list[0].components.pm2_5  # Supports array indexing
      metric_name: pm2_5
      unit: ug/m3
```

**Example Input:**
```json
{
  "name": "St Augustine",
  "main": {"temp": 24.5, "pressure": 1013},
  "wind": {"speed": 3.2, "deg": 180}
}
```

**Output:** One TimeSeriesPoint with extracted nested fields

#### array_iterator Parser

**Use for:** API responses containing arrays of observations/forecasts

**Configuration:**
```yaml
parser:
  parser_type: array_iterator
  array_path: properties.periods  # JSONPath to array
  timestamp_field: startTime  # Extract timestamp from each element
  timestamp_format: iso8601
  metadata_tags:  # Response-level tags applied to all points
    - path: properties.generatedAt
      tag_name: forecast_generated_at
      value_type: timestamp
  element_mappings:  # Extract these fields from each array element
    - path: temperature
      metric_name: temperature
      unit: fahrenheit
    - path: windSpeed
      metric_name: wind_speed
      string_parse:  # Parse numbers from strings
        pattern: "^(\\d+)\\s*(?:to\\s*(\\d+)\\s*)?mph$"
        capture_group: 1
      unit: mph
    - path: windDirection
      metric_name: wind_direction
      enum_map:  # Map categorical to numeric
        N: 0
        NE: 45
        E: 90
        SE: 135
        S: 180
        SW: 225
        W: 270
        NW: 315
      unit: degrees
```

**Example Input:**
```json
{
  "properties": {
    "generatedAt": "2025-12-21T14:30:00+00:00",
    "periods": [
      {
        "startTime": "2025-12-21T15:00:00+00:00",
        "temperature": 72,
        "windSpeed": "5 to 10 mph",
        "windDirection": "SE"
      },
      {
        "startTime": "2025-12-21T16:00:00+00:00",
        "temperature": 73,
        "windSpeed": "10 mph",
        "windDirection": "S"
      }
    ]
  }
}
```

**Output:** 2 TimeSeriesPoints (one per array element), each with:
- `timestamp` from element's `startTime`
- `forecast_generated_at` tag from response root
- Parsed metrics from element fields

**Key Features:**
- **Array Iteration:** Processes each array element independently
- **Per-Element Timestamps:** Uses element's timestamp field (not poll time)
- **Metadata Tags:** Response-level tags attached to all points
- **String Parsing:** Extract numbers from text ("15 mph" → 15.0)
- **Enum Mapping:** Convert categories to numbers ("NE" → 45.0)

### Transform Types

Parsers support transforms to handle non-standard field formats:

#### string_parse Transform

**Use for:** Extracting numeric values from string fields

**Configuration:**
```yaml
element_mappings:
  - path: windSpeed
    metric_name: wind_speed
    string_parse:
      pattern: "^(\\d+)\\s*(?:to\\s*(\\d+)\\s*)?mph$"  # Regex with capture groups
      capture_group: 1  # Which group to extract (1-indexed)
      fallback_value: null  # Value if parse fails
    unit: mph
```

**Examples:**
- `"15 mph"` → `15.0`
- `"10 to 20 mph"` → `10.0` (first capture group)
- `"Variable"` → `null` (parse failure, logged as warning)

#### enum_map Transform

**Use for:** Converting categorical values to numeric

**Configuration:**
```yaml
element_mappings:
  - path: windDirection
    metric_name: wind_direction
    enum_map:
      N: 0
      NNE: 22.5
      NE: 45
      ENE: 67.5
      E: 90
      ESE: 112.5
      SE: 135
      SSE: 157.5
      S: 180
      SSW: 202.5
      SW: 225
      WSW: 247.5
      W: 270
      WNW: 292.5
      NW: 315
      NNW: 337.5
    unit: degrees
```

**Examples:**
- `"NE"` → `45.0`
- `"S"` → `180.0`
- `"UNKNOWN"` → `null` (unmapped value, logged as warning)

**Note:** Matching is case-insensitive

#### unit_conversion Transform

**Use for:** Converting between unit systems (future implementation)

**Example Configuration:**
```yaml
element_mappings:
  - path: temperature
    metric_name: temperature
    unit_conversion:
      from: fahrenheit
      to: celsius
    unit: celsius
```

**Note:** Currently, unit conversions happen in the Silver layer (TimescaleDB). This transform type is reserved for future Bronze-layer conversion needs.

---

## Real-World Example: NWS Hourly Forecast Integration

This complete example shows how to integrate the National Weather Service hourly forecast API, which returns 156 forecast periods in a single response.

### Challenge

The NWS API returns:
- Array of 156 forecast periods (6.5 days)
- Each period has its own timestamp (`startTime`)
- Wind speed as text: `"5 to 10 mph"`, `"10 mph"`, `"Variable"`
- Wind direction as cardinal: `"N"`, `"SE"`, `"WSW"`
- Forecast metadata: `generatedAt`, `updateTime`

### Complete Stream Configuration

**File:** `config/base/streams/nws-forecast-hourly/config.yaml`

```yaml
stream_id: nws-forecast-hourly
description: Hourly weather forecast from NWS gridpoint forecast
version: "1.0.0"
enabled: true
retention_days: 30
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: temperature
    type: float
    nullable: false
    unit: fahrenheit
    range: [-50.0, 130.0]
  - name: dewpoint
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
  - name: relative_humidity
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]
  - name: wind_speed
    type: float
    nullable: true
    unit: mph
    range: [0.0, 200.0]
  - name: wind_direction
    type: float
    nullable: true
    unit: degrees
    range: [0.0, 360.0]
  - name: probability_of_precipitation
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 3600  # 1 hour
    timeout_secs: 30
    retry_attempts: 3
    retry_delay_secs: 5
    parser_name: nws_forecast_hourly
    endpoints:
      - endpoint_id: nws_jax_79_49_forecast
        location_id: ksgj
        url: "https://api.weather.gov/gridpoints/JAX/79,49/forecast/hourly"
        auth_type: none
        headers:
          User-Agent: "(neural-data-platform, contact@example.com)"
          Accept: "application/geo+json"

    # Parser configuration
    parser:
      parser_type: array_iterator

      # Location from response
      location_id_field: properties.gridId
      default_location_id: ksgj

      # Array extraction
      array_path: properties.periods

      # Timestamp from each element
      timestamp_field: startTime
      timestamp_format: iso8601

      # Response-level metadata (applied to all 156 points)
      metadata_tags:
        - path: properties.generatedAt
          tag_name: forecast_generated_at
          value_type: timestamp
        - path: properties.updateTime
          tag_name: forecast_update_time
          value_type: timestamp

      # Static tags
      default_tags:
        source: nws
        api: forecast_hourly
        stream_id: nws-forecast-hourly
        grid_office: JAX
        grid_x: "79"
        grid_y: "49"

      # Extract these fields from each array element
      element_mappings:
        # Simple numeric field
        - path: temperature
          metric_name: temperature
          unit: fahrenheit

        # Nested numeric field
        - path: dewpoint.value
          metric_name: dewpoint
          unit: celsius

        - path: relativeHumidity.value
          metric_name: relative_humidity
          unit: percent

        # String parsing: "5 to 10 mph" → 5.0
        - path: windSpeed
          metric_name: wind_speed
          string_parse:
            pattern: "^(\\d+)\\s*(?:to\\s*(\\d+)\\s*)?mph$"
            capture_group: 1
            fallback_value: null
          unit: mph

        # Enum mapping: "SE" → 135.0
        - path: windDirection
          metric_name: wind_direction
          enum_map:
            N: 0
            NNE: 22.5
            NE: 45
            ENE: 67.5
            E: 90
            ESE: 112.5
            SE: 135
            SSE: 157.5
            S: 180
            SSW: 202.5
            SW: 225
            WSW: 247.5
            W: 270
            WNW: 292.5
            NW: 315
            NNW: 337.5
          unit: degrees

        - path: probabilityOfPrecipitation.value
          metric_name: probability_of_precipitation
          unit: percent

storage:
  batch_size: 156  # One poll = 156 forecast periods
  batch_timeout_secs: 60
  buffer_capacity: 200
```

### What Happens When This Runs

1. **Poll:** Every hour, HTTP poller fetches forecast from NWS
2. **Parse:** `array_iterator` parser extracts `properties.periods` array (156 elements)
3. **Iterate:** For each of 156 periods:
   - Extract `startTime` as timestamp
   - Parse `windSpeed` text to number
   - Map `windDirection` cardinal to degrees
   - Extract other numeric fields
   - Attach metadata tags (`forecast_generated_at`, `forecast_update_time`)
4. **Output:** 156 × 6 = **936 TimeSeriesPoints** per poll
5. **Storage:** Batch written to Parquet in tall format

### Example API Response (Abbreviated)

```json
{
  "properties": {
    "generatedAt": "2025-12-21T14:30:00+00:00",
    "updateTime": "2025-12-21T14:25:00+00:00",
    "gridId": "JAX",
    "periods": [
      {
        "number": 1,
        "startTime": "2025-12-21T15:00:00+00:00",
        "temperature": 72,
        "dewpoint": {"value": 18.3, "unitCode": "wmoUnit:degC"},
        "relativeHumidity": {"value": 65, "unitCode": "wmoUnit:percent"},
        "windSpeed": "5 to 10 mph",
        "windDirection": "SE",
        "probabilityOfPrecipitation": {"value": 20, "unitCode": "wmoUnit:percent"}
      },
      {
        "number": 2,
        "startTime": "2025-12-21T16:00:00+00:00",
        "temperature": 73,
        "dewpoint": {"value": 18.9, "unitCode": "wmoUnit:degC"},
        "relativeHumidity": {"value": 63, "unitCode": "wmoUnit:percent"},
        "windSpeed": "10 mph",
        "windDirection": "S",
        "probabilityOfPrecipitation": {"value": 10, "unitCode": "wmoUnit:percent"}
      }
      // ... 154 more periods
    ]
  }
}
```

### Example Parsed Output (First Period)

```
TimeSeriesPoint {
  location_id: "ksgj",
  timestamp: 2025-12-21T15:00:00Z,  // From period.startTime
  metrics: {
    "temperature": 72.0,
    "dewpoint": 18.3,
    "relative_humidity": 65.0,
    "wind_speed": 5.0,  // Parsed from "5 to 10 mph"
    "wind_direction": 135.0,  // Mapped from "SE"
    "probability_of_precipitation": 20.0
  },
  tags: {
    "source": "nws",
    "api": "forecast_hourly",
    "stream_id": "nws-forecast-hourly",
    "grid_office": "JAX",
    "grid_x": "79",
    "grid_y": "49",
    "forecast_generated_at": "2025-12-21T14:30:00Z",  // From response root
    "forecast_update_time": "2025-12-21T14:25:00Z"    // From response root
  }
}
```

### Key Takeaways

1. **One API call → Many points:** Array iteration multiplies data points
2. **Element timestamps:** Each forecast period uses its own `startTime`
3. **Metadata propagation:** Response-level tags attached to all points
4. **String parsing:** Handles NWS text formats automatically
5. **Enum mapping:** Converts human-readable values to numeric
6. **Forecast tracking:** `forecast_generated_at` tag enables forecast verification

### Buffer Capacity for Array Sources

**Critical:** Array iterator sources generate many points per poll. Default `buffer_capacity` of 1000 may overflow.

```yaml
sources:
  - type: http_poll
    buffer_capacity: 2500  # NWS: 156 periods × 7 metrics = 1092 points
```

**Sizing formula:** `buffer_capacity = array_length × metrics_count × 2.5`

The 2.5x multiplier handles concurrent polls at startup (initial poll + background loop first tick).

---

## Multi-Stream Architecture (Future)

When the full IngestionCoordinator is implemented:

```
┌─────────────────────────────────────────────────────────────┐
│                   Stream Registry (etcd)                     │
│  /streams/air-quality/config                                │
│  /streams/weather/config                                    │
│  /streams/home-events/config                                │
└─────────────────────────────────────────────────────────────┘
                           │
                           │ watch
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                  IngestionCoordinator                        │
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │ air-quality │  │   weather   │  │ home-events │        │
│  │   sources   │  │   sources   │  │   sources   │        │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │
│         └────────────────┼────────────────┘                 │
│                          ▼                                   │
│                  IngestionRouter                             │
│                  (validate + route)                          │
│         ┌────────────────┼────────────────┐                 │
│         ▼                ▼                ▼                 │
│    StorageWriter   StorageWriter   StorageWriter            │
│    (air-quality)     (weather)    (home-events)             │
└─────────────────────────────────────────────────────────────┘
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
    /data/air-quality/ /data/weather/ /data/home-events/
```

---

## Timestamp Extraction

By default, parsers use `Utc::now()` as the timestamp for ingested data. However, many APIs provide their own timestamps that should be preserved.

### When to Extract Timestamps from Response

**Use response timestamps when:**
- API provides observation time (NWS observations: actual measurement time)
- API provides forecast valid time (NWS forecast: when forecast applies)
- Time accuracy matters for analysis
- Comparing multiple sources with different polling intervals

**Use current time when:**
- Source doesn't provide timestamps (MQTT sensors)
- Real-time streaming data
- Timestamp represents "data received" not "data observed"

### Configuration

#### Single Observation Timestamp

For APIs that return one observation with a timestamp field:

```yaml
parser:
  parser_type: json_path
  timestamp_field: properties.timestamp  # JSONPath to timestamp field
  timestamp_format: iso8601  # or unix, unix_ms
  field_mappings:
    # ... field mappings
```

**Example:** NWS observations use `properties.timestamp` from the observation

#### Per-Element Timestamp (Array Iterator)

For APIs that return arrays where each element has its own timestamp:

```yaml
parser:
  parser_type: array_iterator
  array_path: properties.periods
  timestamp_field: startTime  # Field within each array element
  timestamp_format: iso8601
  element_mappings:
    # ... field mappings
```

**Example:** NWS forecast periods each have `startTime` for when that forecast applies

### Supported Timestamp Formats

| Format | Example | Use Case |
|--------|---------|----------|
| `iso8601` | `2025-12-21T14:53:00+00:00` | Most APIs (recommended) |
| `unix` | `1703167980` | Unix epoch seconds |
| `unix_ms` | `1703167980000` | Unix epoch milliseconds |
| `rfc3339` | `2025-12-21T14:53:00Z` | Alternative ISO format |

### Fallback Behavior

If timestamp extraction fails:
1. Parser logs warning with details
2. Falls back to `Utc::now()`
3. Data point is NOT dropped
4. Counter `timestamp_extraction_failures_total{stream}` incremented

### Metadata Tags

Attach response-level metadata as tags on all generated points:

```yaml
parser:
  metadata_tags:
    - path: properties.generatedAt  # JSONPath in response root
      tag_name: forecast_generated_at  # Tag name in TimeSeriesPoint
      value_type: timestamp  # timestamp, string, number

    - path: properties.updateTime
      tag_name: forecast_update_time
      value_type: timestamp

    - path: properties.gridId
      tag_name: grid_id
      value_type: string
```

**Use cases:**
- Track when forecast was generated vs when it applies
- Record API version or data quality indicators
- Store location metadata from response root

**Note:** Metadata tags apply to ALL points generated from the response. For array iterators, this means the same metadata tags appear on all 156+ forecast points.

---

## Best Practices

### Collect All Available Information

**Guiding principle:** Capture everything the source provides. Storage is cheap; missing historical data is expensive.

**Why:**
- Future analysis without re-polling historical data
- Lead time calculations (forecast issue time vs valid time)
- Data quality monitoring (compare predictions to actuals)
- ML feature engineering flexibility

**How:**

1. **Extract document-level metadata** as tags or metrics:
```yaml
metadata_tags:
  - path: properties.generatedAt
    tag_name: forecast_generated_at
metadata_metrics:
  - path: properties.updateTime
    metric_name: forecast_issue_time
    value_type: timestamp
```

2. **Map all meaningful fields**, even if not immediately needed:
```yaml
element_mappings:
  - path: shortForecast     # Text has value for analysis
    metric_name: short_forecast
```

3. **Document everything** in the schema with units and descriptions

**Anti-pattern:** "We only need temperature, skip the rest"
**Best practice:** "Capture everything; filter at query time"

---

## Checklist

Before deploying a new stream:

### Configuration Validation
- [ ] Stream ID follows kebab-case format (3-64 chars)
- [ ] All field names follow snake_case format
- [ ] At least one field defined
- [ ] At least one source defined
- [ ] Field types match expected data
- [ ] Range constraints are valid (min < max)
- [ ] Source topics/endpoints are correct

### Parser Configuration
- [ ] Parser type matches data structure:
  - `flat_json` for simple MQTT/flat JSON
  - `json_path` for nested JSON, single observation
  - `array_iterator` for JSON arrays with multiple observations
- [ ] `timestamp_field` configured if using API timestamps
- [ ] `timestamp_format` specified correctly (iso8601, unix, etc.)
- [ ] `array_path` configured for array_iterator parsers
- [ ] `metadata_tags` configured if tracking response-level metadata
- [ ] All available source fields mapped (collect everything)

### Buffer Capacity (for array_iterator sources)
- [ ] Calculate expected points: `array_length × metrics_count`
- [ ] Set `buffer_capacity` to at least 2.5x expected points
- [ ] Example: NWS (156 periods × 7 metrics = 1092) → buffer_capacity: 2500

### Transform Configuration (if needed)
- [ ] `string_parse` patterns tested for text fields
- [ ] `enum_map` includes all expected categorical values
- [ ] Transform fallback values appropriate (null vs 0)
- [ ] Unit conversions documented (if manual conversion needed)

### Deployment & Verification
- [ ] Config loaded into etcd successfully
- [ ] Data source is publishing to correct topic/endpoint
- [ ] Verified data appears in storage (when implemented)
- [ ] Parser produces expected number of points
- [ ] Timestamps extracted correctly (not using poll time when shouldn't)
- [ ] Metadata tags present on generated points
- [ ] Log "forwarded X points" matches "Polled endpoint - X points" (no data loss)

---

## Troubleshooting

### Stream Not Loading

1. Check YAML syntax: `yq eval '.' config.yaml`
2. Verify etcd is running: `docker exec etcd etcdctl endpoint health`
3. Check loader script output for errors

### Validation Errors

Common validation errors:
- "Invalid stream ID" - Use kebab-case, 3-64 chars
- "Invalid field name" - Use snake_case
- "No fields" - Add at least one field
- "No sources" - Add at least one source
- "Range invalid" - Ensure min < max

### Data Not Being Ingested

1. Check MQTT topic matches source config
2. Verify JSON structure matches schema fields
3. Check application logs for parsing errors
4. Ensure stream is enabled (`enabled: true`)

### Storage Issues

1. Check disk space: `df -h`
2. Verify storage path exists
3. Check file permissions
4. Review ParquetStore logs

---

## References

- [StreamConfig Type](../../core/src/types/stream_config.rs) - Full type definition
- [Stream Registry](../../config-client/src/stream/registry.rs) - Registry implementation
- [PLATFORM_ARCHITECTURE.md](../../product/features/air-004/architecture/PLATFORM_ARCHITECTURE.md) - Architecture overview
- [COMPLETION-PI-CORRECTED.md](../../product/features/air-004/completion/COMPLETION-PI-CORRECTED.md) - Deployment guide
