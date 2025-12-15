# How to Add a New Data Stream

This guide explains how to add a new data stream using the GitOps YAML approach.

## Overview

Stream configurations are managed as YAML files in the GitOps structure:
- **Base configs**: `config/base/streams/{stream-id}/config.yaml`
- **Environment overlays**: `config/overlays/{env}/streams/{stream-id}/config.yaml`
- **Sync to etcd**: Automatic via `scripts/sync-config-to-etcd.sh`

## Quick Start

### 1. Create Stream YAML

```bash
mkdir -p config/base/streams/weather
```

Create `config/base/streams/weather/config.yaml`:

```yaml
# Weather Stream Configuration
stream_id: "weather"
description: "Local weather station readings"
version: "1.0.0"
enabled: true
retention_days: 365

# MQTT source configuration
mqtt:
  enabled: true
  broker_url: "mosquitto"
  port: 1883
  client_id: "weather-app"
  topic_pattern: "weather/readings/+"
  qos: 1

# Storage configuration
storage:
  batch_size: 50
  batch_timeout_secs: 10
```

### 2. Sync to etcd

```bash
# Development
./scripts/sync-config-to-etcd.sh development

# Production (on Pi)
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production
```

### 3. Verify

```bash
# Check keys were created
etcdctl get --prefix /streams/weather/
```

## GitOps Directory Structure

```
config/
├── base/
│   ├── air-quality/           # Existing app config
│   │   └── config.yaml
│   └── streams/               # Stream definitions
│       ├── air-quality/
│       │   └── config.yaml    # Air quality stream
│       └── weather/           # New weather stream
│           └── config.yaml
└── overlays/
    ├── development/
    │   └── streams/
    │       └── air-quality/
    │           └── config.yaml  # Dev overrides
    └── production/
        └── streams/
            └── air-quality/
                └── config.yaml  # Prod overrides
```

## Stream Config Schema

### Required Fields

```yaml
stream_id: "your-stream-name"     # Unique identifier (kebab-case)
description: "What this stream is"
version: "1.0.0"                   # Semver
enabled: true                       # Enable/disable stream
```

### MQTT Configuration

```yaml
mqtt:
  enabled: true
  broker_url: "mosquitto"          # Required
  port: 1883                       # Default: 1883
  client_id: "your-app"            # Default: air-quality-app
  topic_pattern: "your/topic/+"    # MQTT topic pattern
  qos: 1                           # 0, 1, or 2
  reconnect_delay_secs: 1          # Default: 1
  max_reconnect_delay_secs: 30     # Default: 30
  buffer_capacity: 1000            # Default: 1000
```

### Storage Configuration

```yaml
storage:
  batch_size: 100                  # Records per batch
  batch_timeout_secs: 5            # Flush timeout
  buffer_capacity: 1000            # Buffer size
```

### Data Fields (Optional)

```yaml
fields:
  temperature:
    type: "float"
    unit: "celsius"
    description: "Ambient temperature"
    nullable: true
  sensor_id:
    type: "string"
    nullable: false
```

## etcd Key Structure

The sync script flattens YAML to etcd keys:

```
/streams/air-quality/stream_id         → "air-quality"
/streams/air-quality/enabled           → true
/streams/air-quality/mqtt/broker_url   → "mosquitto"
/streams/air-quality/mqtt/port         → 1883
/streams/air-quality/mqtt/topic_pattern → "airgradient/readings/+"
/streams/air-quality/storage/batch_size → 100
```

## Environment Overrides

Create production-specific overrides:

```bash
mkdir -p config/overlays/production/streams/weather
```

`config/overlays/production/streams/weather/config.yaml`:
```yaml
# Production overrides (merged with base)
mqtt:
  broker_url: "mosquitto.internal"
  buffer_capacity: 2000

storage:
  batch_size: 500
```

## Deployment Workflow

### Local Development

```bash
# 1. Create/edit stream YAML
vim config/base/streams/weather/config.yaml

# 2. Sync to local etcd
./scripts/sync-config-to-etcd.sh development

# 3. Run app
cargo run --package air-quality-app
```

### Production (Pi)

```bash
# 1. Commit stream YAML to git
git add config/base/streams/weather/
git commit -m "Add weather stream configuration"
git push

# 2. On Pi: pull and sync
cd /path/to/neural-data-platform
git pull
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production

# 3. Restart app if needed
docker compose restart air-quality-app
```

## Adding New Source Types (Future)

Currently only MQTT is supported. When you need HTTP polling or webhooks:

1. **Implement the handler** in `core/src/sources/`
2. **Add YAML schema support** for the new source type
3. **Update stream_integration.rs** to handle the new type

The YAML structure is ready for extension:
```yaml
# Future: HTTP polling source
http:
  enabled: true
  url: "https://api.weather.com/..."
  interval_secs: 60
  auth_token: "${WEATHER_API_TOKEN}"
```

## Troubleshooting

### Stream not loading

```bash
# Check if keys exist in etcd
etcdctl get --prefix /streams/your-stream/

# Check sync script ran
./scripts/sync-config-to-etcd.sh development 2>&1 | grep your-stream

# Check app logs
docker logs air-quality-app | grep "stream"
```

### Config not updating

```bash
# Force re-sync
./scripts/sync-config-to-etcd.sh production

# Verify values in etcd
etcdctl get /streams/your-stream/mqtt/broker_url

# Restart app to pick up changes
docker compose restart air-quality-app
```

## Reference

- **Stream YAML**: `config/base/streams/air-quality/config.yaml`
- **Sync script**: `scripts/sync-config-to-etcd.sh`
- **App integration**: `apps/air-quality-app/src/stream_integration.rs`
- **Legacy config**: `apps/air-quality-app/src/config_etcd.rs`
