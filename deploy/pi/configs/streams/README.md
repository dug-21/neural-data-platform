# Air Quality Stream Configuration

This directory contains scripts for managing multi-stream air quality monitoring configurations in etcd.

## Overview

The multi-stream architecture allows the air-quality-app to process data from multiple AirGradient sensors simultaneously. Each sensor is configured as a separate "stream" with its own:

- Device ID and MQTT topic
- Storage path
- Metadata (name, location, description)
- Enable/disable status

## Scripts

### init-streams.sh
Initializes default stream configurations in etcd.

```bash
./init-streams.sh [etcd_container_name]
```

**Default streams:**
- `airgradient-001`: Primary office sensor (enabled)
- `airgradient-002`: Conference room sensor (disabled by default)

Run this script after deploying the stack for the first time.

### add-stream.sh
Adds a new stream configuration.

```bash
./add-stream.sh <stream_id> <stream_name> <device_id> <mqtt_topic> <location> [description]
```

**Example:**
```bash
./add-stream.sh \
    airgradient-003 \
    "Lab Sensor" \
    "84fce612f5f9" \
    "airgradient/readings/84fce612f5f9" \
    "Research Lab" \
    "Lab air quality monitoring"
```

**Stream ID format:**
- Lowercase letters, numbers, and hyphens only
- Example: `airgradient-001`, `sensor-lab-01`

### list-streams.sh
Lists all configured streams and their status.

```bash
./list-streams.sh [etcd_container_name]
```

Displays:
- Stream ID, name, and location
- Device ID and MQTT topic
- Enable/disable status
- Global multi-stream configuration

## Stream Configuration Structure

Each stream is stored in etcd under `/air-quality/streams/<stream_id>/`:

```
/air-quality/streams/airgradient-001/
  ├── id                    # Stream identifier
  ├── name                  # Human-readable name
  ├── device_id             # AirGradient device ID
  ├── mqtt_topic            # MQTT topic to subscribe to
  ├── location              # Physical location
  ├── description           # Stream description
  ├── enabled               # true/false
  ├── created_at            # ISO 8601 timestamp
  └── storage/
      ├── path              # Data storage path
      ├── retention_days    # Data retention period
      └── compression       # Enable compression
```

## Global Multi-Stream Configuration

Global settings are stored under `/air-quality/multi_stream/`:

```
/air-quality/multi_stream/
  ├── enabled                   # Enable multi-stream mode
  ├── max_concurrent_streams    # Maximum number of active streams
  ├── webhook_enabled           # Enable webhook handler
  └── webhook_port              # Webhook port (default: 8081)
```

## Workflow

### Initial Setup
1. Deploy the Docker stack
2. Run `./init-streams.sh` to create default streams
3. Verify with `./list-streams.sh`

### Adding a New Sensor
1. Get the device ID from your AirGradient sensor
2. Run `./add-stream.sh` with sensor details
3. The stream will be automatically enabled
4. The app will start processing data from the new MQTT topic

### Enabling/Disabling Streams
```bash
# Disable a stream
docker exec etcd etcdctl put "/air-quality/streams/airgradient-002/enabled" "false"

# Enable a stream
docker exec etcd etcdctl put "/air-quality/streams/airgradient-002/enabled" "true"
```

### Viewing Stream Data
Stream data is stored in `/app/data/streams/<stream_id>/` within the container.

```bash
# List stream data directories
docker exec air-quality-app ls -lh /app/data/streams/

# Check data size for a specific stream
docker exec air-quality-app du -sh /app/data/streams/airgradient-001/
```

## Webhook Integration

When `webhook_enabled` is set to `true`, the app exposes a webhook endpoint on port 8081 for dynamic stream management:

- `POST /webhook/streams/add` - Add a new stream
- `PUT /webhook/streams/{id}/enable` - Enable a stream
- `PUT /webhook/streams/{id}/disable` - Disable a stream
- `GET /webhook/streams` - List all streams

Example webhook call:
```bash
curl -X POST http://localhost:8081/webhook/streams/add \
  -H "Content-Type: application/json" \
  -d '{
    "stream_id": "airgradient-004",
    "name": "Basement Sensor",
    "device_id": "84fce612f5fa",
    "mqtt_topic": "airgradient/readings/84fce612f5fa",
    "location": "Basement",
    "description": "Basement air quality monitoring"
  }'
```

## Memory Considerations

Each stream consumes approximately 20-50MB of memory depending on:
- Message throughput
- Batch buffer size
- Number of retained time series

With the Pi's 896MB budget:
- Base services: ~350MB (mosquitto + etcd)
- App baseline: ~200MB
- Available for streams: ~346MB
- **Recommended max streams: 6-8**

Monitor memory usage:
```bash
docker stats --no-stream air-quality-app
```

## Troubleshooting

### Stream not receiving data
1. Check MQTT topic is correct
2. Verify stream is enabled: `docker exec etcd etcdctl get "/air-quality/streams/<id>/enabled"`
3. Check app logs: `docker logs air-quality-app`

### Stream data not persisting
1. Verify volume mount: `docker inspect air-quality-app | grep Mounts`
2. Check storage path exists: `docker exec air-quality-app ls /app/data/streams/`

### etcd connection issues
1. Verify etcd is healthy: `docker exec etcd etcdctl endpoint health`
2. Check network connectivity: `docker exec air-quality-app ping etcd`

## See Also

- [AIR-004 Feature Documentation](/workspaces/neural-data-platform/product/features/air-004/)
- [Docker Deployment Guide](/workspaces/neural-data-platform/deploy/pi/README.md)
- [Air Quality App Documentation](/workspaces/neural-data-platform/apps/air-quality-app/README.md)
