# AIR-004: Multi-Stream Docker Deployment Plan

## Document Status
- **Status**: Planning
- **Version**: 1.0.0
- **Created**: 2025-12-15
- **Target Platform**: Raspberry Pi 5, Ubuntu 25.04, ARM64

## Executive Summary

This document provides the Docker deployment strategy for extending the existing Pi deployment (`deploy/pi/`) to support multi-stream data ingestion while maintaining backward compatibility with the current air-quality-only configuration.

**Critical Design Principle**: PRESERVE existing deployment, EXTEND for multi-stream.

## Current Baseline (deploy/pi/)

### Existing Services (3 total, ~350MB actual usage)
1. **mosquitto** (eclipse-mosquitto:2.0)
   - Memory: ~50MB actual, 128MB limit
   - Ports: 1883 (MQTT), 9001 (WebSocket)
   - Purpose: MQTT broker for AirGradient sensors

2. **etcd** (quay.io/coreos/etcd:v3.5.11)
   - Memory: ~100MB actual, 256MB limit
   - Port: 2379 (client API)
   - Purpose: Configuration store (existing pattern from AIR-003)

3. **air-quality-app** (neural-data-platform/air-quality-app:latest)
   - Memory: ~200MB actual, 512MB limit
   - Ports: 8080 (HTTP API), 9090 (metrics)
   - Purpose: MQTT ingestion → Parquet storage
   - Build time: 15-30 minutes (ARM64 Rust compilation)

### Existing Volumes
- `pi_air-quality-data`: Parquet files at `/app/data`
- `pi_etcd-data`: etcd persistence
- `pi_mosquitto-data`: MQTT broker data
- `pi_mosquitto-logs`: MQTT broker logs

### Existing Network
- `neural-network`: Docker bridge network

## Multi-Stream Extension Strategy

### Phase 1: Stream Registry Infrastructure (No Service Changes)

#### 1.1 Stream Configuration Directory Structure
Create stream configuration directories under `deploy/pi/`:

```
deploy/pi/
├── configs/
│   └── streams/
│       ├── air-quality/          # Existing stream (migrated config)
│       │   ├── config.yaml       # Stream metadata
│       │   ├── schema.yaml       # Data schema
│       │   └── sources.yaml      # Ingestion sources
│       ├── weather/              # NEW: Example second stream
│       │   ├── config.yaml
│       │   ├── schema.yaml
│       │   └── sources.yaml
│       └── home-events/          # NEW: Example third stream
│           ├── config.yaml
│           ├── schema.yaml
│           └── sources.yaml
├── scripts/
│   └── load-stream-configs.sh   # Load streams into etcd
├── docker-compose.yml            # MODIFIED: Add webhook port
├── deploy.sh                     # MODIFIED: Include stream config sync
└── mosquitto/
    └── mosquitto.conf
```

#### 1.2 Stream Configuration Templates

**`deploy/pi/configs/streams/air-quality/config.yaml`**:
```yaml
# Stream: Air Quality Monitoring
stream_id: air-quality
enabled: true
description: Indoor air quality measurements from AirGradient sensors

# Retention and storage policies
retention:
  days: 365                      # Keep data for 1 year
  compression_after_days: 7      # Compress Parquet after 7 days

# Alert thresholds
alert_thresholds:
  pm25_unhealthy: 35.0           # µg/m³
  pm25_very_unhealthy: 55.0
  co2_high: 1000                 # ppm
  co2_very_high: 2000

# Tags for categorization
tags:
  - environmental
  - indoor
  - health
```

**`deploy/pi/configs/streams/air-quality/schema.yaml`**:
```yaml
# Air Quality Stream Schema
fields:
  - name: pm25
    type: float
    unit: µg/m³
    nullable: false
    description: Particulate Matter 2.5µm concentration
    range: [0, 1000]

  - name: co2
    type: int
    unit: ppm
    nullable: false
    description: Carbon Dioxide concentration
    range: [400, 5000]

  - name: voc
    type: int
    unit: index
    nullable: true
    description: Volatile Organic Compounds index
    range: [0, 500]

  - name: temperature
    type: float
    unit: celsius
    nullable: true
    description: Ambient temperature
    range: [-50, 100]

  - name: humidity
    type: float
    unit: percent
    nullable: true
    description: Relative humidity
    range: [0, 100]

  - name: sensor_id
    type: string
    nullable: false
    description: AirGradient device identifier

  - name: location_id
    type: string
    nullable: true
    description: Logical location (e.g., "bedroom", "office")

# Indexes for TimescaleDB (Silver layer)
indexes:
  - fields: [location_id, timestamp]
    order: desc
  - fields: [sensor_id, timestamp]
    order: desc
```

**`deploy/pi/configs/streams/air-quality/sources.yaml`**:
```yaml
# Air Quality Data Sources
sources:
  - id: mqtt-airgradient
    type: mqtt
    enabled: true
    config:
      topics:
        - airgradient/+/measures     # AirGradient topic pattern
        - airgradient/readings/+     # Alternative pattern
      qos: 1
      transform: airgradient_v1      # Parser/transformer ID
    health_check:
      enabled: true
      interval_seconds: 60
      timeout_seconds: 5
```

**`deploy/pi/configs/streams/weather/config.yaml`**:
```yaml
# Stream: External Weather Data
stream_id: weather
enabled: false                      # Disabled by default, enable when ready
description: External weather data for correlation with air quality

retention:
  days: 365
  compression_after_days: 30

tags:
  - environmental
  - outdoor
  - weather
```

**`deploy/pi/configs/streams/weather/schema.yaml`**:
```yaml
# Weather Stream Schema
fields:
  - name: temperature
    type: float
    unit: celsius
    nullable: false
    range: [-50, 60]

  - name: humidity
    type: float
    unit: percent
    nullable: false
    range: [0, 100]

  - name: pressure
    type: float
    unit: hPa
    nullable: false
    range: [900, 1100]

  - name: wind_speed
    type: float
    unit: m/s
    nullable: true
    range: [0, 50]

  - name: precipitation
    type: float
    unit: mm
    nullable: true
    range: [0, 500]

indexes:
  - fields: [timestamp]
    order: desc
```

**`deploy/pi/configs/streams/weather/sources.yaml`**:
```yaml
# Weather Data Sources
sources:
  - id: http-openweather
    type: http_poll
    enabled: false                  # Requires API key configuration
    config:
      url: https://api.openweathermap.org/data/2.5/weather
      query_params:
        q: "YourCity,YourCountry"
        appid: "${OPENWEATHER_API_KEY}"
        units: metric
      method: GET
      interval_seconds: 300         # Poll every 5 minutes
      timeout_seconds: 10
      headers:
        User-Agent: neural-data-platform/1.0
      transform: openweather_v1     # Parser for OpenWeather API response
    health_check:
      enabled: true
      interval_seconds: 600
      timeout_seconds: 15
```

**`deploy/pi/configs/streams/home-events/config.yaml`**:
```yaml
# Stream: Home Automation Events
stream_id: home-events
enabled: false                      # Enable when home automation integrated
description: Events from home automation system (lights, switches, sensors)

retention:
  days: 90                          # Shorter retention for events
  compression_after_days: 7

tags:
  - home-automation
  - events
  - iot
```

**`deploy/pi/configs/streams/home-events/schema.yaml`**:
```yaml
# Home Events Stream Schema
fields:
  - name: event_type
    type: string
    nullable: false
    description: Type of event (state_change, trigger, etc.)

  - name: device_id
    type: string
    nullable: false
    description: Device identifier

  - name: device_type
    type: string
    nullable: false
    description: Device type (light, switch, motion_sensor, etc.)

  - name: state
    type: string
    nullable: false
    description: New state of device

  - name: previous_state
    type: string
    nullable: true
    description: Previous state of device

  - name: location
    type: string
    nullable: true
    description: Room/location of device

indexes:
  - fields: [device_id, timestamp]
    order: desc
  - fields: [event_type, timestamp]
    order: desc
```

**`deploy/pi/configs/streams/home-events/sources.yaml`**:
```yaml
# Home Events Data Sources
sources:
  - id: webhook-homebridge
    type: webhook
    enabled: false                  # Enable when webhook integration ready
    config:
      path: /webhooks/homebridge    # Webhook endpoint path
      method: POST
      authentication:
        type: hmac_sha256
        secret_key: "${WEBHOOK_SECRET}"
        header: X-Hub-Signature-256
      transform: homebridge_v1      # Parser for Homebridge webhook format
    health_check:
      enabled: false                # Webhooks are passive, no active health check

  - id: mqtt-home-events
    type: mqtt
    enabled: false
    config:
      topics:
        - home/events/#
        - homeassistant/+/state
      qos: 1
      transform: home_events_v1
    health_check:
      enabled: true
      interval_seconds: 120
```

#### 1.3 Stream Registry Loader Script

**`deploy/pi/scripts/load-stream-configs.sh`**:
```bash
#!/bin/bash
# Load stream configurations into etcd
# Usage: ./load-stream-configs.sh [stream_id]
#   No args: Load all streams
#   With stream_id: Load specific stream

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ETCD_CONTAINER="${ETCD_CONTAINER:-etcd}"
CONFIG_DIR="$PI_DIR/configs/streams"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log() { echo -e "${GREEN}[STREAMS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# Check if etcd container is running
if ! docker ps --format '{{.Names}}' | grep -q "^${ETCD_CONTAINER}$"; then
    error "etcd container is not running. Run: cd $PI_DIR && ./deploy.sh start"
fi

# Wait for etcd to be healthy
log "Waiting for etcd to be ready..."
MAX_RETRIES=30
RETRY=0
until docker exec "$ETCD_CONTAINER" etcdctl endpoint health >/dev/null 2>&1; do
    RETRY=$((RETRY+1))
    if [ $RETRY -ge $MAX_RETRIES ]; then
        error "etcd not healthy after $MAX_RETRIES retries"
    fi
    sleep 1
done
log "etcd is ready"

# Function to load YAML file into etcd
load_yaml_to_etcd() {
    local key=$1
    local file=$2

    if [ ! -f "$file" ]; then
        warn "File not found: $file"
        return 1
    fi

    log "  Loading $key"

    # Read file content and escape for etcdctl
    local content=$(cat "$file")

    # Use heredoc to handle multiline YAML
    docker exec -i "$ETCD_CONTAINER" sh -c "etcdctl put '$key' - <<'EOFYAML'
$content
EOFYAML"
}

# Function to load a single stream
load_stream() {
    local stream_dir=$1
    local stream_id=$(basename "$stream_dir")

    log "Loading stream: $stream_id"

    # Load config
    if [ -f "$stream_dir/config.yaml" ]; then
        load_yaml_to_etcd "/streams/$stream_id/config" "$stream_dir/config.yaml"
    else
        warn "No config.yaml for $stream_id"
    fi

    # Load schema
    if [ -f "$stream_dir/schema.yaml" ]; then
        load_yaml_to_etcd "/streams/$stream_id/schema" "$stream_dir/schema.yaml"
    else
        warn "No schema.yaml for $stream_id"
    fi

    # Load sources
    if [ -f "$stream_dir/sources.yaml" ]; then
        load_yaml_to_etcd "/streams/$stream_id/sources" "$stream_dir/sources.yaml"
    else
        warn "No sources.yaml for $stream_id"
    fi
}

# Main logic
if [ -z "$1" ]; then
    # Load all streams
    log "Loading all stream configurations from $CONFIG_DIR"

    for stream_dir in "$CONFIG_DIR"/*; do
        if [ ! -d "$stream_dir" ]; then
            continue
        fi
        load_stream "$stream_dir"
    done
else
    # Load specific stream
    STREAM_DIR="$CONFIG_DIR/$1"
    if [ ! -d "$STREAM_DIR" ]; then
        error "Stream directory not found: $STREAM_DIR"
    fi
    load_stream "$STREAM_DIR"
fi

echo ""
log "Stream configurations loaded successfully"
echo ""

# List registered streams
log "Registered streams:"
docker exec "$ETCD_CONTAINER" etcdctl get /streams/ --prefix --keys-only | \
    grep "/config" | sed 's|/streams/||' | sed 's|/config||' | sort -u | \
    while read stream_id; do
        # Get enabled status
        enabled=$(docker exec "$ETCD_CONTAINER" etcdctl get "/streams/$stream_id/config" | \
                  grep "^enabled:" | awk '{print $2}')
        if [ "$enabled" = "true" ]; then
            echo -e "  ${GREEN}✓${NC} $stream_id (enabled)"
        else
            echo -e "  ${YELLOW}○${NC} $stream_id (disabled)"
        fi
    done

echo ""
log "Stream configuration keys in etcd:"
docker exec "$ETCD_CONTAINER" etcdctl get /streams/ --prefix --keys-only | head -20
```

### Phase 2: Docker Compose Modifications

#### 2.1 Add Webhook Port to air-quality-app

**Changes to `deploy/pi/docker-compose.yml`**:

```yaml
  air-quality-app:
    # ... existing configuration ...
    ports:
      - "8080:8080"     # HTTP API (existing)
      - "9090:9090"     # Metrics (existing)
      - "8081:8081"     # NEW: Webhook ingestion endpoint
    # ... rest of configuration ...
```

**Rationale**:
- Port 8081 dedicated to webhook ingestion sources
- Separates webhook traffic from main API (8080)
- Allows different rate limiting and authentication policies

#### 2.2 Optional: TimescaleDB Service (Future Phase)

**NOT INCLUDED IN INITIAL DEPLOYMENT** - Add only when Silver layer needed:

```yaml
  # TimescaleDB - SQL-queryable Silver layer (OPTIONAL)
  # CAUTION: Adds ~600MB memory overhead
  # Only add if Pi has >= 2GB free memory
  timescaledb:
    image: timescale/timescaledb:latest-pg14
    container_name: timescaledb
    ports:
      - "127.0.0.1:5432:5432"      # Localhost only for security
    environment:
      - POSTGRES_DB=neural_data
      - POSTGRES_USER=neural
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
    volumes:
      - timescaledb-data:/var/lib/postgresql/data
    networks:
      - neural-network
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "pg_isready", "-U", "neural"]
      interval: 30s
      timeout: 10s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 1G               # Significant memory overhead

volumes:
  # ... existing volumes ...
  timescaledb-data:                # Add only if using TimescaleDB
    driver: local
```

### Phase 3: Deploy Script Enhancements

#### 3.1 Modified `deploy/pi/deploy.sh`

**Add stream configuration sync** after service startup:

```bash
# Add new function
load_streams() {
    log "Loading stream configurations to etcd..."

    if [ -f "$SCRIPT_DIR/scripts/load-stream-configs.sh" ]; then
        "$SCRIPT_DIR/scripts/load-stream-configs.sh"
    else
        warn "Stream loader script not found, skipping"
    fi
}

# Modify start() function
start() {
    log "Starting services..."
    docker compose up -d

    log "Waiting for services to be healthy..."
    sleep 10

    # Sync config after services are up (EXISTING)
    sync_config

    # NEW: Load stream configurations
    load_streams

    log "Services started successfully!"
    status
}

# Add new command: reload-streams
reload_streams() {
    log "Reloading stream configurations..."
    load_streams

    # Optionally restart air-quality-app to pick up changes
    if [ "$1" = "--restart" ]; then
        log "Restarting air-quality-app to apply changes..."
        docker compose restart air-quality-app
    else
        log "Changes loaded. Restart air-quality-app if hot-reload is disabled:"
        log "  docker compose restart air-quality-app"
    fi
}

# Modify main case statement
case "${1:-deploy}" in
    # ... existing commands ...
    reload-streams)
        load_streams
        ;;
    *)
        echo "Usage: $0 {deploy|start|stop|logs|status|update|build|sync|reload-streams}"
        exit 1
        ;;
esac
```

**New command**:
```bash
./deploy.sh reload-streams           # Reload stream configs without restart
./deploy.sh reload-streams --restart # Reload and restart app
```

### Phase 4: Application Code Changes (air-quality-app)

#### 4.1 Stream Registry Client Module

**New file: `apps/air-quality-app/src/streams/registry.rs`**:

```rust
// Stream Registry Client
// Reads stream configurations from etcd at /streams/{stream_id}/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub enabled: bool,
    pub description: String,
    pub retention: RetentionPolicy,
    pub alert_thresholds: Option<HashMap<String, f64>>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub days: u32,
    pub compression_after_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSchema {
    pub fields: Vec<SchemaField>,
    pub indexes: Vec<SchemaIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub unit: Option<String>,
    pub nullable: bool,
    pub description: Option<String>,
    pub range: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaIndex {
    pub fields: Vec<String>,
    pub order: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSources {
    pub sources: Vec<DataSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    pub id: String,
    #[serde(rename = "type")]
    pub source_type: String,
    pub enabled: bool,
    pub config: serde_json::Value,
    pub health_check: Option<HealthCheckConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub enabled: bool,
    pub interval_seconds: u64,
    pub timeout_seconds: Option<u64>,
}

// Stream Registry Client
pub struct StreamRegistry {
    etcd_client: etcd_client::Client,
}

impl StreamRegistry {
    pub async fn new(etcd_endpoint: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let etcd_client = etcd_client::Client::connect([etcd_endpoint], None).await?;
        Ok(Self { etcd_client })
    }

    pub async fn list_streams(&mut self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let response = self.etcd_client
            .get("/streams/", Some(etcd_client::GetOptions::new().with_prefix()))
            .await?;

        let mut stream_ids = std::collections::HashSet::new();
        for kv in response.kvs() {
            let key = kv.key_str()?;
            // Extract stream_id from /streams/{stream_id}/...
            if let Some(parts) = key.strip_prefix("/streams/") {
                if let Some(stream_id) = parts.split('/').next() {
                    stream_ids.insert(stream_id.to_string());
                }
            }
        }

        Ok(stream_ids.into_iter().collect())
    }

    pub async fn get_stream_config(
        &mut self,
        stream_id: &str,
    ) -> Result<StreamConfig, Box<dyn std::error::Error>> {
        let key = format!("/streams/{}/config", stream_id);
        let response = self.etcd_client.get(key, None).await?;

        if let Some(kv) = response.kvs().first() {
            let yaml_str = kv.value_str()?;
            let config: StreamConfig = serde_yaml::from_str(yaml_str)?;
            Ok(config)
        } else {
            Err(format!("Stream config not found: {}", stream_id).into())
        }
    }

    pub async fn get_stream_schema(
        &mut self,
        stream_id: &str,
    ) -> Result<StreamSchema, Box<dyn std::error::Error>> {
        let key = format!("/streams/{}/schema", stream_id);
        let response = self.etcd_client.get(key, None).await?;

        if let Some(kv) = response.kvs().first() {
            let yaml_str = kv.value_str()?;
            let schema: StreamSchema = serde_yaml::from_str(yaml_str)?;
            Ok(schema)
        } else {
            Err(format!("Stream schema not found: {}", stream_id).into())
        }
    }

    pub async fn get_stream_sources(
        &mut self,
        stream_id: &str,
    ) -> Result<StreamSources, Box<dyn std::error::Error>> {
        let key = format!("/streams/{}/sources", stream_id);
        let response = self.etcd_client.get(key, None).await?;

        if let Some(kv) = response.kvs().first() {
            let yaml_str = kv.value_str()?;
            let sources: StreamSources = serde_yaml::from_str(yaml_str)?;
            Ok(sources)
        } else {
            Err(format!("Stream sources not found: {}", stream_id).into())
        }
    }

    // Watch for stream configuration changes (hot-reload)
    pub async fn watch_streams(
        &mut self,
    ) -> Result<impl futures::Stream<Item = etcd_client::WatchResponse>, Box<dyn std::error::Error>> {
        let (watcher, stream) = self.etcd_client
            .watch("/streams/", Some(etcd_client::WatchOptions::new().with_prefix()))
            .await?;

        Ok(stream)
    }
}
```

#### 4.2 Feature Flag for Stream Registry

**Add to `apps/air-quality-app/src/config.rs`**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesConfig {
    pub stream_registry: bool,      // Enable multi-stream support
    pub hot_reload: bool,            // Watch etcd for config changes
    pub webhook_ingestion: bool,     // Enable webhook endpoints
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub mqtt: MqttConfig,
    pub storage: StorageConfig,
    pub features: FeaturesConfig,    // NEW
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            stream_registry: false,  // Disabled by default (backward compat)
            hot_reload: false,
            webhook_ingestion: false,
        }
    }
}
```

**Environment variable overrides**:
```bash
ENABLE_STREAM_REGISTRY=true
ENABLE_HOT_RELOAD=true
ENABLE_WEBHOOK_INGESTION=true
```

### Phase 5: Memory Budget Compliance

#### 5.1 Resource Allocation

**Baseline (Current)**:
- mosquitto: 128MB limit (~50MB actual)
- etcd: 256MB limit (~100MB actual)
- air-quality-app: 512MB limit (~200MB actual)
- **Total**: 896MB limit, ~350MB actual

**With Multi-Stream (Stream Registry + Webhook)**:
- mosquitto: 128MB limit (~80MB actual, more topics)
- etcd: 256MB limit (~150MB actual, stream registry data)
- air-quality-app: 512MB limit (~300MB actual, multiple sources)
- **Total**: 896MB limit, ~530MB actual

**Memory Budget**: 896MB limit, ~530MB actual → **COMPLIANT** (<1GB)

**With Optional TimescaleDB (Future)**:
- Add timescaledb: 1GB limit (~600MB actual)
- **Total**: 1.9GB limit, ~1.1GB actual
- **Recommendation**: Requires Pi with >=4GB RAM

#### 5.2 Memory Monitoring

Add to `deploy/pi/deploy.sh`:

```bash
check_resources() {
    log "Resource Usage:"
    docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}" | head -n 10

    # Warning if total memory usage > 800MB
    TOTAL_MEM=$(docker stats --no-stream --format "{{.MemUsage}}" | \
                awk '{split($1,a,"/"); sum+=a[1]} END {print sum}')

    if [ "${TOTAL_MEM%.*}" -gt 800 ]; then
        warn "Total memory usage exceeds 800MB. Monitor for OOM issues."
    fi
}

# Add to status() function
status() {
    # ... existing status checks ...

    echo ""
    check_resources
}
```

## Deployment Sequence

### Step 1: Create Stream Configuration Structure

```bash
cd /workspaces/neural-data-platform/deploy/pi

# Create directories
mkdir -p configs/streams/air-quality
mkdir -p configs/streams/weather
mkdir -p configs/streams/home-events
mkdir -p scripts

# Create stream configuration files (see templates above)
# - configs/streams/air-quality/{config,schema,sources}.yaml
# - configs/streams/weather/{config,schema,sources}.yaml
# - configs/streams/home-events/{config,schema,sources}.yaml

# Create stream loader script
# - scripts/load-stream-configs.sh

# Make script executable
chmod +x scripts/load-stream-configs.sh
```

### Step 2: Update Docker Compose

```bash
# Edit docker-compose.yml
# Add webhook port 8081 to air-quality-app service
# DO NOT add TimescaleDB yet (future phase)
```

### Step 3: Update Deploy Script

```bash
# Edit deploy.sh
# Add load_streams() function
# Add reload-streams command
# Integrate stream loading into start() function
```

### Step 4: Deploy and Verify

```bash
# Stop existing services
./deploy.sh stop

# Start with new configuration
./deploy.sh start

# Verify stream configurations loaded
docker exec etcd etcdctl get /streams/ --prefix --keys-only

# Check resource usage
docker stats --no-stream

# Test API endpoints
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/streams          # NEW: List streams
curl http://localhost:8080/api/v1/streams/air-quality  # NEW: Stream details
```

### Step 5: Backward Compatibility Verification

```bash
# Verify existing air-quality functionality still works
docker exec mqtt-broker mosquitto_pub -h localhost -p 1883 \
  -t "airgradient/test/measures" \
  -m '{"pm25":15.0,"co2":700,"temperature":21.0,"humidity":45.0}'

# Check Parquet files created
docker exec air-quality-app ls -lh /app/data/air-quality/

# Query API
curl http://localhost:8080/api/v1/air-quality/latest
```

## Rollback Procedures

### Rollback to Baseline (Pre-Multi-Stream)

```bash
# Stop services
cd /workspaces/neural-data-platform/deploy/pi
./deploy.sh stop

# Restore baseline docker-compose.yml (remove webhook port)
git checkout deploy/pi/docker-compose.yml

# Restore baseline deploy.sh (remove stream loading)
git checkout deploy/pi/deploy.sh

# Clear stream registry from etcd
docker compose up -d etcd
sleep 5
docker exec etcd etcdctl del /streams/ --prefix

# Restart services
./deploy.sh start

# Verify baseline operation
docker compose ps
curl http://localhost:8080/health
```

### Disable Specific Streams

```bash
# Disable weather stream
docker exec etcd etcdctl put /streams/weather/config/enabled false

# Disable home-events stream
docker exec etcd etcdctl put /streams/home-events/config/enabled false

# Restart app to apply
docker compose restart air-quality-app
```

### Remove Webhook Port

```bash
# Edit docker-compose.yml, remove port 8081
# Restart
docker compose up -d air-quality-app
```

## Verification Checklist

- [ ] Stream configuration directories created
- [ ] Stream loader script created and executable
- [ ] Docker compose includes webhook port (8081)
- [ ] Deploy script includes stream loading
- [ ] Services start successfully
- [ ] Stream configurations loaded to etcd
- [ ] Resource usage within limits (<1GB)
- [ ] Existing air-quality functionality works
- [ ] New API endpoints respond (`/api/v1/streams`)
- [ ] Rollback procedure tested

## Future Extensions

### Extension 1: Enable Weather Stream

```bash
# Configure OpenWeather API key
echo "export OPENWEATHER_API_KEY=your_key_here" >> ~/.bashrc
source ~/.bashrc

# Enable weather stream
docker exec etcd etcdctl put /streams/weather/config/enabled true
docker exec etcd etcdctl put /streams/weather/sources/http-openweather/enabled true

# Reload streams
cd /workspaces/neural-data-platform/deploy/pi
./deploy.sh reload-streams --restart

# Verify weather data ingestion
docker logs -f air-quality-app | grep weather
curl http://localhost:8080/api/v1/streams/weather/latest
```

### Extension 2: Enable Home Events Stream

```bash
# Configure webhook secret
echo "export WEBHOOK_SECRET=$(openssl rand -hex 32)" >> ~/.bashrc
source ~/.bashrc

# Enable home-events stream
docker exec etcd etcdctl put /streams/home-events/config/enabled true
docker exec etcd etcdctl put /streams/home-events/sources/webhook-homebridge/enabled true

# Reload streams
./deploy.sh reload-streams --restart

# Test webhook endpoint
PI_IP=$(hostname -I | awk '{print $1}')
curl -X POST http://${PI_IP}:8081/webhooks/homebridge \
  -H "Content-Type: application/json" \
  -d '{"event":"state_change","device":"living_room_light","state":"on"}'
```

### Extension 3: Add TimescaleDB (Silver Layer)

```bash
# Prerequisites check
free -h  # Ensure >=2GB free memory

# Add TimescaleDB service to docker-compose.yml
# (see Phase 2.2 above)

# Set database password
echo "export POSTGRES_PASSWORD=$(openssl rand -hex 16)" >> ~/.bashrc
source ~/.bashrc

# Deploy TimescaleDB
docker compose up -d timescaledb

# Wait for health check
docker logs -f timescaledb | grep "database system is ready"

# Apply schema migrations
# (Create hypertables, indexes, etc.)

# Enable dual-write in air-quality-app
docker exec etcd etcdctl put /config/air-quality/storage/silver/enabled true
docker compose restart air-quality-app
```

## File Locations Reference

**Stream Configurations**:
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/air-quality/`
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/weather/`
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/home-events/`

**Scripts**:
- `/workspaces/neural-data-platform/deploy/pi/scripts/load-stream-configs.sh`

**Docker Files**:
- `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`
- `/workspaces/neural-data-platform/deploy/pi/deploy.sh`

**Application Code** (changes needed):
- `/workspaces/neural-data-platform/apps/air-quality-app/src/streams/registry.rs` (NEW)
- `/workspaces/neural-data-platform/apps/air-quality-app/src/config.rs` (MODIFY)
- `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (MODIFY)

**etcd Keys**:
- `/streams/{stream_id}/config` - Stream metadata
- `/streams/{stream_id}/schema` - Data schema
- `/streams/{stream_id}/sources` - Ingestion sources

## Network Ports Reference

**Existing Ports**:
- 1883: MQTT broker (mosquitto)
- 2379: etcd client API
- 8080: Air-quality-app HTTP API
- 9090: Metrics (Prometheus)

**New Ports**:
- 8081: Webhook ingestion (air-quality-app)

**Future Ports** (optional):
- 5432: TimescaleDB PostgreSQL (localhost only)
- 3000: Grafana UI (if added)

## Support and Troubleshooting

**Issue**: Stream configurations not loading
```bash
# Check etcd connectivity
docker exec etcd etcdctl endpoint health

# Manually load streams
cd /workspaces/neural-data-platform/deploy/pi
./scripts/load-stream-configs.sh

# Verify loaded
docker exec etcd etcdctl get /streams/ --prefix --keys-only
```

**Issue**: Webhook endpoint not responding
```bash
# Check port exposed
docker compose ps air-quality-app
netstat -tuln | grep 8081

# Check firewall
sudo ufw status
sudo ufw allow 8081/tcp
```

**Issue**: Memory usage too high
```bash
# Check resource usage
docker stats --no-stream

# Disable optional streams
docker exec etcd etcdctl put /streams/weather/enabled false
docker exec etcd etcdctl put /streams/home-events/enabled false
docker compose restart air-quality-app
```

---

**END OF DEPLOYMENT PLAN**
