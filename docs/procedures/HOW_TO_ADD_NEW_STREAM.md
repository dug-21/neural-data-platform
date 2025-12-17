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

## Checklist

Before deploying a new stream:

- [ ] Stream ID follows kebab-case format (3-64 chars)
- [ ] All field names follow snake_case format
- [ ] At least one field defined
- [ ] At least one source defined
- [ ] Field types match expected data
- [ ] Range constraints are valid (min < max)
- [ ] Source topics/endpoints are correct
- [ ] Config loaded into etcd successfully
- [ ] Data source is publishing to correct topic
- [ ] Verified data appears in storage (when implemented)

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
