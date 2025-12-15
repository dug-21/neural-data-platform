# Docker Deployment Guide - Multi-Stream Air Quality Monitoring

## Overview

This guide covers the Docker deployment configuration for AIR-004 multi-stream support on Raspberry Pi 5.

## Architecture

The deployment consists of three core services:

1. **mosquitto** - MQTT broker for sensor data ingestion
2. **etcd** - Distributed configuration store for stream management
3. **air-quality-app** - Main application with multi-stream processing

## Memory Budget (Raspberry Pi 5 Constraints)

Total available: 896MB

| Service | Memory Limit | Purpose |
|---------|-------------|---------|
| mosquitto | 128MB | MQTT message broker |
| etcd | 256MB | Configuration store |
| air-quality-app | 512MB | Stream processing (baseline + 6-8 streams) |
| **Total** | **896MB** | Fits within Pi constraints |

### Memory Scaling Per Stream

Each additional stream consumes approximately 20-50MB depending on:
- Message throughput
- Batch buffer size
- Time series retention

**Recommended maximum: 6-8 concurrent streams**

## Docker Compose Configuration

### Location
`/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`

### Key Changes for Multi-Stream Support

#### 1. Port Exposure
```yaml
ports:
  - "8080:8080"     # HTTP API
  - "8081:8081"     # Webhook handler (NEW)
  - "9090:9090"     # Metrics
```

Port 8081 exposes the webhook handler for dynamic stream management.

#### 2. Volume Mounts
```yaml
volumes:
  - air-quality-data:/app/data
  - air-quality-streams:/app/data/streams  # NEW: Stream-specific storage
```

Separate volume for stream data provides:
- Isolation between streams
- Easier backup/restore
- Individual stream monitoring

#### 3. Environment Variables
```yaml
environment:
  # Existing configuration
  - RUST_LOG=info
  - STORAGE_PATH=/app/data
  - ETCD_ENDPOINT=http://etcd:2379
  - MQTT_BROKER_URL=mosquitto
  - MQTT_PORT=1883

  # NEW: Multi-stream configuration
  - ENABLE_MULTI_STREAM=true
  - MAX_CONCURRENT_STREAMS=8
  - WEBHOOK_ENABLED=true
  - WEBHOOK_PORT=8081
  - STREAM_CONFIG_PREFIX=/air-quality/streams
```

## Deployment Script Extensions

### Location
`/workspaces/neural-data-platform/deploy/pi/deploy.sh`

### New Functions

#### init_streams()
Initializes stream configurations in etcd on first deployment.

**Features:**
- Detects existing streams to prevent overwriting
- Prompts user before re-initialization
- Loads default stream configurations
- Sets global multi-stream settings

**Invoked by:**
- `./deploy.sh deploy` (full deployment)
- `./deploy.sh start` (service startup)
- `./deploy.sh update` (update deployment)
- `./deploy.sh init-streams` (manual initialization)

#### status() - Enhanced
Now includes stream status information:
- Number of configured streams
- Enabled/disabled state per stream
- Multi-stream global configuration

**Invoked by:**
- `./deploy.sh status`

### New Commands

```bash
# Initialize or re-initialize streams
./deploy.sh init-streams

# List all configured streams
./deploy.sh list-streams
```

## Stream Configuration Scripts

### Location
`/workspaces/neural-data-platform/deploy/pi/configs/streams/`

### Scripts

#### 1. init-streams.sh
Loads default stream configurations into etcd.

**Default streams:**
- `airgradient-001`: Primary office sensor (enabled)
- `airgradient-002`: Conference room sensor (disabled)

**Usage:**
```bash
./init-streams.sh [etcd_container_name]
```

**Configuration structure:**
```
/air-quality/streams/<stream_id>/
  ├── id                    # Stream identifier
  ├── name                  # Human-readable name
  ├── device_id             # AirGradient device ID
  ├── mqtt_topic            # MQTT topic pattern
  ├── location              # Physical location
  ├── description           # Stream description
  ├── enabled               # true/false
  ├── created_at            # ISO 8601 timestamp
  └── storage/
      ├── path              # /app/data/streams/<id>
      ├── retention_days    # 30
      └── compression       # true
```

#### 2. add-stream.sh
Adds a new stream configuration.

**Usage:**
```bash
./add-stream.sh <stream_id> <name> <device_id> <topic> <location> [description]
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

**Validation:**
- Stream ID must be lowercase alphanumeric with hyphens
- Checks for existing streams to prevent duplicates
- Prompts before overwriting

#### 3. list-streams.sh
Lists all configured streams with status.

**Usage:**
```bash
./list-streams.sh [etcd_container_name]
```

**Output:**
- Stream name, ID, device ID
- MQTT topic
- Physical location
- Enabled/disabled status
- Global multi-stream configuration

## Deployment Workflows

### Initial Deployment

```bash
cd /workspaces/neural-data-platform/deploy/pi

# Full deployment with stream initialization
./deploy.sh deploy
```

**Steps:**
1. Check prerequisites (Docker, Docker Compose)
2. Build Docker images
3. Start services (mosquitto, etcd, air-quality-app)
4. Wait for health checks
5. Sync base configuration to etcd
6. Initialize stream configurations
7. Display status

### Adding a New Sensor

```bash
# 1. Add stream configuration
cd /workspaces/neural-data-platform/deploy/pi/configs/streams
./add-stream.sh \
    airgradient-004 \
    "Basement Sensor" \
    "84fce612f5fa" \
    "airgradient/readings/84fce612f5fa" \
    "Basement"

# 2. Verify stream is active
./list-streams.sh

# 3. Check app logs for stream startup
docker logs -f air-quality-app
```

The app will automatically detect the new stream configuration and start processing data.

### Disabling a Stream

```bash
# Disable via etcd
docker exec etcd etcdctl put "/air-quality/streams/airgradient-002/enabled" "false"

# Verify
./list-streams.sh
```

The app will stop processing data from the disabled stream on next config reload.

### Monitoring Stream Data

```bash
# Check stream data directories
docker exec air-quality-app ls -lh /app/data/streams/

# Check data size for a specific stream
docker exec air-quality-app du -sh /app/data/streams/airgradient-001/

# Monitor app logs for stream activity
docker logs -f air-quality-app | grep "stream_id"
```

## Webhook API (Dynamic Management)

When `WEBHOOK_ENABLED=true`, the app exposes management endpoints on port 8081.

### Endpoints

#### POST /webhook/streams/add
Add a new stream dynamically.

```bash
curl -X POST http://localhost:8081/webhook/streams/add \
  -H "Content-Type: application/json" \
  -d '{
    "stream_id": "airgradient-005",
    "name": "Rooftop Sensor",
    "device_id": "84fce612f5fb",
    "mqtt_topic": "airgradient/readings/84fce612f5fb",
    "location": "Rooftop",
    "description": "Outdoor air quality monitoring"
  }'
```

#### PUT /webhook/streams/{id}/enable
Enable a stream.

```bash
curl -X PUT http://localhost:8081/webhook/streams/airgradient-002/enable
```

#### PUT /webhook/streams/{id}/disable
Disable a stream.

```bash
curl -X PUT http://localhost:8081/webhook/streams/airgradient-002/disable
```

#### GET /webhook/streams
List all streams.

```bash
curl http://localhost:8081/webhook/streams
```

## Health Monitoring

### Service Health Checks

```bash
# Check all services
./deploy.sh status

# Individual health checks
curl http://localhost:8080/health       # Air quality app
docker exec etcd etcdctl endpoint health # etcd
```

### Memory Monitoring

```bash
# Real-time stats for all services
docker stats

# Memory usage for air-quality-app only
docker stats air-quality-app --no-stream
```

**Warning thresholds:**
- App using >480MB: Consider reducing active streams
- App using >500MB: Approaching memory limit

### Stream Processing Verification

```bash
# Check app logs for stream processing
docker logs air-quality-app | grep "Processing message"

# Check parquet files are being created
docker exec air-quality-app ls -lt /app/data/streams/airgradient-001/ | head -10

# Verify MQTT subscriptions
docker exec mosquitto mosquitto_sub -t 'airgradient/readings/#' -v
```

## Troubleshooting

### Issue: Stream not receiving data

**Diagnosis:**
```bash
# 1. Check stream is enabled
docker exec etcd etcdctl get "/air-quality/streams/<id>/enabled"

# 2. Verify MQTT topic
docker exec etcd etcdctl get "/air-quality/streams/<id>/mqtt_topic"

# 3. Check MQTT messages arriving
docker exec mosquitto mosquitto_sub -t 'airgradient/readings/#' -C 5
```

**Resolution:**
- Enable stream if disabled
- Verify MQTT topic matches sensor configuration
- Check sensor network connectivity

### Issue: Memory limit exceeded

**Diagnosis:**
```bash
docker stats --no-stream
```

**Resolution:**
1. Reduce `MAX_CONCURRENT_STREAMS` environment variable
2. Disable unused streams
3. Reduce batch buffer sizes in config

### Issue: Stream configuration not loading

**Diagnosis:**
```bash
# Check etcd connectivity
docker exec air-quality-app ping etcd

# Verify stream configs in etcd
docker exec etcd etcdctl get --prefix "/air-quality/streams/" --keys-only

# Check app logs for config errors
docker logs air-quality-app | grep -i "config"
```

**Resolution:**
- Run `./deploy.sh init-streams` to re-initialize
- Verify ETCD_ENDPOINT environment variable
- Check etcd service is healthy

### Issue: Webhook endpoints not accessible

**Diagnosis:**
```bash
# Check port mapping
docker port air-quality-app 8081

# Test webhook endpoint
curl http://localhost:8081/webhook/streams
```

**Resolution:**
- Verify `WEBHOOK_ENABLED=true` in docker-compose.yml
- Check port 8081 is not blocked by firewall
- Restart air-quality-app service

## Upgrade Path

### From Single-Stream to Multi-Stream

1. **Backup existing data:**
   ```bash
   docker exec air-quality-app tar -czf /app/data/backup.tar.gz /app/data/*.parquet
   docker cp air-quality-app:/app/data/backup.tar.gz ./backup.tar.gz
   ```

2. **Update docker-compose.yml:**
   - Add new environment variables
   - Add stream volume mount
   - Expose webhook port

3. **Restart with new configuration:**
   ```bash
   ./deploy.sh stop
   ./deploy.sh start
   ```

4. **Initialize streams:**
   ```bash
   ./deploy.sh init-streams
   ```

5. **Migrate existing data (if needed):**
   - Map old data to a new stream ID
   - Move parquet files to stream directory

## Configuration Reference

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ENABLE_MULTI_STREAM` | `false` | Enable multi-stream processing |
| `MAX_CONCURRENT_STREAMS` | `8` | Maximum number of active streams |
| `WEBHOOK_ENABLED` | `false` | Enable webhook management API |
| `WEBHOOK_PORT` | `8081` | Webhook HTTP port |
| `STREAM_CONFIG_PREFIX` | `/air-quality/streams` | etcd prefix for stream configs |
| `ETCD_ENDPOINT` | `http://etcd:2379` | etcd connection endpoint |
| `MQTT_BROKER_URL` | `mosquitto` | MQTT broker hostname |
| `MQTT_PORT` | `1883` | MQTT broker port |

### Volume Mounts

| Container Path | Volume | Purpose |
|----------------|--------|---------|
| `/app/data` | `air-quality-data` | Base data directory |
| `/app/data/streams` | `air-quality-streams` | Stream-specific storage |
| `/etcd-data` | `etcd-data` | etcd persistent storage |
| `/mosquitto/data` | `mosquitto-data` | MQTT persistent state |

## Performance Optimization

### Batch Size Tuning

Edit stream storage config in etcd:
```bash
# Increase batch size for high-throughput streams
docker exec etcd etcdctl put \
    "/air-quality/streams/airgradient-001/storage/batch_size" "500"

# Decrease for low-latency requirements
docker exec etcd etcdctl put \
    "/air-quality/streams/airgradient-001/storage/batch_size" "100"
```

### Compression Settings

```bash
# Disable compression for faster writes (uses more disk space)
docker exec etcd etcdctl put \
    "/air-quality/streams/airgradient-001/storage/compression" "false"
```

### Memory Optimization

```bash
# Reduce concurrent streams
docker exec etcd etcdctl put \
    "/air-quality/multi_stream/max_concurrent_streams" "4"
```

## Security Considerations

### Network Isolation

The default network configuration uses Docker bridge networking with the custom network `neural-network`. Services communicate using container names.

**Hardening:**
- Limit external port exposure
- Use TLS for MQTT (mosquitto TLS config)
- Enable etcd authentication
- Add webhook authentication

### Data Protection

```bash
# Regular backups of etcd data
docker run --rm -v etcd-data:/etcd-data -v $(pwd):/backup alpine \
    tar -czf /backup/etcd-backup-$(date +%Y%m%d).tar.gz /etcd-data

# Stream data backups
docker run --rm -v air-quality-streams:/streams -v $(pwd):/backup alpine \
    tar -czf /backup/streams-backup-$(date +%Y%m%d).tar.gz /streams
```

## See Also

- [AIR-004 Architecture Overview](/workspaces/neural-data-platform/product/features/air-004/architecture/)
- [Stream Configuration Guide](/workspaces/neural-data-platform/deploy/pi/configs/streams/README.md)
- [Multi-Stream API Documentation](/workspaces/neural-data-platform/apps/air-quality-app/docs/multi-stream-api.md)
