# AIR-004: Generic Multi-Stream Data Platform - SPARC Completion (Pi Deployment Edition)

## Document Status

**Status**: CORRECTED for Raspberry Pi 5 Production Deployment
**Version**: 3.0.0
**Last Updated**: 2025-12-15
**Related Documents**:
- [Platform Architecture](/workspaces/neural-data-platform/product/features/air-004/architecture/PLATFORM_ARCHITECTURE.md)
- [AIR-003 Implementation](/workspaces/neural-data-platform/product/features/air-003/)
- [AIR-002 Configuration System](/workspaces/neural-data-platform/product/features/air-002/)

**CRITICAL CORRECTION**: This document reflects the ACTUAL Raspberry Pi 5 production deployment at `/workspaces/neural-data-platform/deploy/pi/`, NOT the theoretical `docker/production/` paths.

---

## Executive Summary

This document provides the complete integration and deployment plan for transforming the neural-data-platform from a single-stream air quality system into a generic multi-stream data platform. The design builds on **ACTUAL RASPBERRY PI 5 OPERATIONAL INFRASTRUCTURE** running on Ubuntu 25.04.

**Current Operational Baseline**:
- **Production Pi**: Raspberry Pi 5 deployment in `deploy/pi/`
- **Services**: mosquitto, etcd, air-quality-app (3 services)
- **Volumes**: air-quality-data, etcd-data, mosquitto-data, mosquitto-logs
- **Ports**: 1883 (MQTT), 2379 (etcd), 8080 (API), 9090 (metrics)
- **Deployment**: `deploy/pi/deploy.sh` with health checks and validation
- **Config Management**: `scripts/sync-config-to-etcd.sh production`

**Key Deliverables**:
1. Stream Registry in etcd with hot-reload capability
2. Generic ingestion coordinator supporting MQTT, HTTP polling, and webhooks
3. Dual-layer storage (Bronze Parquet + Silver TimescaleDB extension)
4. Stream-agnostic dashboards and monitoring
5. Migration path from single-stream to multi-stream with ROLLBACK capability

**Timeline**: 6 phases, 4-6 weeks total

---

## Current Operational Baseline

### Pi Production Infrastructure (deploy/pi/docker-compose.yml)

**Location**: `/workspaces/neural-data-platform/deploy/pi/`

**Services** (3 total):

1. **mosquitto** (eclipse-mosquitto:2.0)
   - Container: mqtt-broker
   - Ports: 1883 (MQTT), 9001 (WebSocket - optional)
   - Volumes: ./mosquitto/mosquitto.conf → /mosquitto/config/, mosquitto-data, mosquitto-logs
   - Health: mosquitto_sub test every 30s
   - Memory: 128MB limit
   - Network: neural-network (bridge)

2. **etcd** (quay.io/coreos/etcd:v3.5.11)
   - Container: etcd
   - Ports: 2379 (client)
   - Volume: etcd-data
   - Single-node cluster (etcd0)
   - Memory: 256MB limit
   - Quota: 512MB backend
   - Network: neural-network (bridge)

3. **air-quality-app** (neural-data-platform/air-quality-app:latest)
   - Container: air-quality-app
   - Build: Context ../.. (project root), Dockerfile
   - Ports: 8080 (HTTP API), 9090 (metrics)
   - Volumes: air-quality-data → /app/data
   - Environment:
     - RUST_LOG=info
     - STORAGE_PATH=/app/data
     - ETCD_ENDPOINT=http://etcd:2379
     - MQTT_BROKER_URL=mosquitto
     - MQTT_PORT=1883
   - Depends on: mosquitto (healthy), etcd (healthy)
   - Health: curl http://localhost:8080/health
   - Memory: 512MB limit
   - Network: neural-network (bridge)

**Network**: neural-network (bridge driver)

**Volumes**:
- mosquitto-data (local driver)
- mosquitto-logs (local driver)
- etcd-data (local driver)
- air-quality-data (local driver)

**Docker Compose Project Name**: `pi` (volumes prefixed with `pi_`)

---

### Pi Production Environment Constraints

**Hardware**:
- Platform: Raspberry Pi 5
- OS: Ubuntu 25.04 (Linux 6.12.54-linuxkit ARM64)
- CPU: ARM Cortex (limited cores)
- Memory: 4-8GB RAM total
- Storage: SD card or USB (limited I/O)

**Resource Limits**:
- mosquitto: 128MB memory (minimal broker)
- etcd: 256MB memory, 512MB quota
- air-quality-app: 512MB memory
- **Total Stack**: <1GB memory footprint

**Optimizations**:
- No Prometheus/Grafana on Pi (lightweight deployment)
- Config loaded from etcd (hot-reload capability)
- Parquet files stored locally (/app/data)
- No TimescaleDB on Pi (Bronze layer only)
- Health checks with minimal overhead
- Service dependencies to ensure startup order

**Network**:
- Localhost binding for security
- Docker bridge network (neural-network)
- External access via Pi IP address

---

### Deployment Process (deploy/pi/deploy.sh)

**Script Location**: `/workspaces/neural-data-platform/deploy/pi/deploy.sh`

**Commands**:
```bash
./deploy.sh         # Full deploy (build + start)
./deploy.sh start   # Start services
./deploy.sh stop    # Stop services
./deploy.sh logs    # View logs
./deploy.sh status  # Check status
./deploy.sh update  # Pull latest and rebuild
./deploy.sh build   # Build images only
./deploy.sh sync    # Sync config to etcd
```

**Deployment Steps**:
1. **Prerequisites Check**: Docker, docker compose availability
2. **Build**: `docker compose build --progress=plain` (15-30 min first run)
3. **Start**: `docker compose up -d`
4. **Health Wait**: 10 second delay for services to initialize
5. **Config Sync**: Execute `scripts/sync-config-to-etcd.sh production` via etcd container
6. **Status Display**: Service status, health checks, data volume size, useful URLs

**Config Sync Process**:
```bash
# Wait for etcd to be ready
until docker exec etcd etcdctl endpoint health >/dev/null 2>&1; do
    sleep 2
done

# Run sync script from repo root
cd ../..
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production
cd deploy/pi
```

**Health Checks**:
- MQTT Broker: TCP connection test (mosquitto_sub)
- etcd: `etcdctl endpoint health`
- Air Quality: HTTP GET `/health` endpoint

**Status Display**:
- Service status: `docker compose ps`
- Data volume: `docker exec air-quality-app du -sh /app/data`
- Useful URLs: API, metrics, MQTT broker (using Pi IP)

---

### Configuration Management

**Base Config**: `/workspaces/neural-data-platform/config/base/air-quality.yaml`
**Production Overlay**: `/workspaces/neural-data-platform/config/overlays/production/overrides.yaml`

**Sync Script**: `/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh`

**Usage**:
```bash
# Sync production config to etcd
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production

# Config hierarchy in etcd:
# /config/air-quality (base config + production overrides merged)
```

**Air-Quality App Config Loading**:
1. Check etcd endpoint: `http://etcd:2379`
2. Fetch `/config/air-quality` from etcd
3. Merge with environment variables
4. Watch for changes (hot-reload if enabled)

---

## Table of Contents

1. [Integration Roadmap](#1-integration-roadmap)
2. [Infrastructure Changes](#2-infrastructure-changes)
3. [Deployment Strategy](#3-deployment-strategy)
4. [Operational Runbook](#4-operational-runbook)
5. [Rollback Procedures](#5-rollback-procedures)
6. [Future Extensions](#6-future-extensions)

---

## 1. Integration Roadmap

### 1.1 Migration Philosophy

**Core Principle**: Verify existing Pi deployment, then extend incrementally

- **BEFORE ANY CHANGES**: Verify current Pi stack works (mosquitto + etcd + air-quality-app)
- Preserve existing air-quality-app functionality throughout migration
- Introduce new components alongside existing ones (feature flags)
- Enable gradual activation with rollback to current working state
- Work within Pi resource constraints (memory, CPU, I/O)

**Risk Mitigation**:
- Phase 1 is VERIFICATION ONLY (no code changes)
- All phases include rollback procedures to restore current state
- Incremental deployment with health monitoring
- Resource usage monitoring to prevent Pi overload

---

### 1.2 Phase-by-Phase Integration Plan

#### Phase 1: Baseline Verification and Documentation (Week 1)

**Objective**: Verify existing Pi deployment BEFORE making any changes

**CRITICAL**: This phase is VERIFICATION ONLY. No code changes, no infrastructure changes.

**Tasks**:

1. **Verify Pi Air-Quality Stack** (1 day)
   ```bash
   # Location: /workspaces/neural-data-platform/deploy/pi

   # Start Pi stack
   ./deploy.sh start

   # Verify all services healthy
   docker compose ps
   # Expected: mqtt-broker, etcd, air-quality-app all "Up (healthy)"

   # Test MQTT ingestion
   docker exec mqtt-broker mosquitto_pub -h localhost -p 1883 \
     -t "airgradient/test/measures" \
     -m '{"pm25":12.3,"co2":650,"temperature":22.1,"humidity":45.2}'

   # Verify data storage (Parquet files)
   docker exec air-quality-app ls -lh /app/data/

   # Test API endpoints
   PI_IP=$(hostname -I | awk '{print $1}')
   curl http://${PI_IP}:8080/health
   curl http://${PI_IP}:8080/api/v1/air-quality/latest

   # Test metrics
   curl http://${PI_IP}:9090/metrics | grep air_quality

   # Verify etcd connectivity and config
   docker exec etcd etcdctl endpoint health
   docker exec etcd etcdctl get /config/air-quality

   # Check resource usage
   docker stats --no-stream
   # Verify: mosquitto <128MB, etcd <256MB, air-quality-app <512MB
   ```

2. **Document Current Data Flows** (4 hours)
   - Air-Quality: MQTT (AirGradient) → air-quality-app → Parquet → /app/data volume → pi_air-quality-data

3. **Document Current Configuration Hierarchy** (4 hours)
   - Base: config/base/air-quality.yaml
   - Overlay: config/overlays/production/overrides.yaml
   - Sync: scripts/sync-config-to-etcd.sh production
   - Storage: etcd @ /config/air-quality
   - Consumption: air-quality-app reads from etcd

4. **Identify Integration Points** (1 day)
   - Shared Network: neural-network (bridge)
   - Volume Strategy: Named volumes (pi_ prefix)
   - Config Store: etcd service (already running)
   - MQTT Broker: mosquitto service (already running)
   - Data Storage: /app/data in air-quality-app container

5. **Create Baseline Snapshot** (2 hours)
   ```bash
   # Backup current configurations
   mkdir -p /workspaces/neural-data-platform/product/features/air-004/pi-baseline-snapshot

   # Docker configs
   cp deploy/pi/docker-compose.yml product/features/air-004/pi-baseline-snapshot/
   cp deploy/pi/deploy.sh product/features/air-004/pi-baseline-snapshot/
   cp -r deploy/pi/mosquitto product/features/air-004/pi-baseline-snapshot/

   # Application configs
   cp -r config/base config/overlays product/features/air-004/pi-baseline-snapshot/

   # etcd snapshot
   docker exec etcd etcdctl snapshot save /tmp/etcd-snapshot.db
   docker cp etcd:/tmp/etcd-snapshot.db product/features/air-004/pi-baseline-snapshot/

   # Data volume inspection (record size, not content)
   docker exec air-quality-app du -sh /app/data > product/features/air-004/pi-baseline-snapshot/data-volume-size.txt
   docker exec air-quality-app find /app/data -name "*.parquet" | wc -l > product/features/air-004/pi-baseline-snapshot/parquet-file-count.txt

   # Tag current state in git
   git add product/features/air-004/pi-baseline-snapshot/
   git commit -m "docs(air-004): Pi baseline snapshot before multi-stream migration"
   git tag air-004-pi-baseline-pre-migration
   ```

**Dependencies**: None (verification only)

**Deliverables**:
- `/workspaces/neural-data-platform/product/features/air-004/pi-baseline-snapshot/` (configs, etcd snapshot)
- `/workspaces/neural-data-platform/product/features/air-004/pi-verification-report.md` (test results)
- Git tag: `air-004-pi-baseline-pre-migration`

**Success Criteria**:
- All Pi services pass health checks
- MQTT → Parquet pipeline verified working
- etcd config loading verified
- Resource usage within limits (mosquitto <128MB, etcd <256MB, app <512MB)

**Failure Criteria**:
- ANY service fails health check → STOP, fix existing issues before proceeding
- MQTT ingestion broken → STOP, fix air-quality-app
- etcd connection issues → STOP, fix etcd service
- Resource usage exceeds limits → STOP, investigate memory leak

---

#### Phase 2: Stream Registry in etcd (Week 1-2)

**Objective**: Implement stream registry schema in existing etcd service

**Tasks**:

1. **Define Stream Registry Schema** (4 hours)
   ```yaml
   # Location: /workspaces/neural-data-platform/deploy/pi/configs/streams/air-quality/config.yaml

   stream_id: air-quality
   description: Indoor air quality measurements from AirGradient sensors
   enabled: true
   retention_days: 365
   compression_after_days: 7
   tags:
     - environmental
     - indoor
     - health
   alert_thresholds:
     pm25_unhealthy: 35.0
     co2_high: 1000
   ```

   ```yaml
   # Location: /workspaces/neural-data-platform/deploy/pi/configs/streams/air-quality/schema.yaml

   fields:
     - name: pm25
       type: float
       unit: µg/m³
       nullable: false
       range: [0, 1000]

     - name: co2
       type: int
       unit: ppm
       nullable: false
       range: [400, 5000]

     - name: temperature
       type: float
       unit: celsius
       nullable: true
       range: [-50, 100]

     - name: humidity
       type: float
       unit: percent
       nullable: true
       range: [0, 100]

   indexes:
     - fields: [location_id, timestamp]
       order: desc
     - fields: [sensor_id, timestamp]
       order: desc
   ```

   ```yaml
   # Location: /workspaces/neural-data-platform/deploy/pi/configs/streams/air-quality/sources.yaml

   sources:
     - id: mqtt-airgradient
       type: mqtt
       enabled: true
       config:
         topics:
           - airgradient/+/measures
         qos: 1
         transform: airgradient_v1
       health_check:
         enabled: true
         interval_seconds: 60
   ```

2. **Create Stream Registry Loader Script** (1 day)
   ```bash
   # Location: /workspaces/neural-data-platform/deploy/pi/scripts/load-stream-configs.sh

   #!/bin/bash
   # Load stream configurations into etcd (Pi deployment)

   set -e

   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
   PI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
   ETCD_CONTAINER="etcd"
   CONFIG_DIR="$PI_DIR/configs/streams"

   echo "Loading stream configurations from $CONFIG_DIR to etcd"

   # Check if etcd container is running
   if ! docker ps | grep -q "$ETCD_CONTAINER"; then
       echo "ERROR: etcd container is not running"
       echo "Run: cd $PI_DIR && ./deploy.sh start"
       exit 1
   fi

   # Function to load YAML file into etcd
   load_yaml_to_etcd() {
       local key=$1
       local file=$2

       if [ ! -f "$file" ]; then
           echo "WARNING: File not found: $file"
           return 1
       fi

       echo "  Loading $key from $(basename "$file")"
       docker exec "$ETCD_CONTAINER" sh -c "etcdctl put '$key' '$(cat "$file")'"
   }

   # Iterate over stream directories
   for stream_dir in "$CONFIG_DIR"/*; do
       if [ ! -d "$stream_dir" ]; then
           continue
       fi

       stream_id=$(basename "$stream_dir")
       echo "Loading stream: $stream_id"

       # Load config
       [ -f "$stream_dir/config.yaml" ] && \
         load_yaml_to_etcd "/streams/$stream_id/config" "$stream_dir/config.yaml"

       # Load schema
       [ -f "$stream_dir/schema.yaml" ] && \
         load_yaml_to_etcd "/streams/$stream_id/schema" "$stream_dir/schema.yaml"

       # Load sources
       [ -f "$stream_dir/sources.yaml" ] && \
         load_yaml_to_etcd "/streams/$stream_id/sources" "$stream_dir/sources.yaml"
   done

   echo ""
   echo "Stream configurations loaded successfully"
   echo ""
   echo "Registered streams:"
   docker exec "$ETCD_CONTAINER" etcdctl get /streams/ --prefix --keys-only | \
       grep config | sed 's|/streams/||' | sed 's|/config||' | sort -u
   ```

3. **Load Streams into etcd** (30 mins)
   ```bash
   # Make script executable
   chmod +x /workspaces/neural-data-platform/deploy/pi/scripts/load-stream-configs.sh

   # Run loader
   cd /workspaces/neural-data-platform/deploy/pi
   ./scripts/load-stream-configs.sh

   # Verify streams loaded
   docker exec etcd etcdctl get /streams/air-quality/config
   docker exec etcd etcdctl get /streams/air-quality/schema
   docker exec etcd etcdctl get /streams/air-quality/sources
   ```

4. **Update air-quality-app to Support Stream Registry** (2 days)
   - Modify Rust code to read stream configs from `/streams/` prefix in etcd
   - Add stream registry watcher (optional hot-reload)
   - Maintain backward compatibility with existing `/config/air-quality` path
   - Feature flag: `ENABLE_STREAM_REGISTRY` (default: false)

5. **Test Stream Registry** (1 day)
   ```bash
   # Enable stream registry feature flag
   cd /workspaces/neural-data-platform/deploy/pi

   # Option 1: Environment variable in docker-compose.yml
   # Add to air-quality-app service:
   # environment:
   #   - ENABLE_STREAM_REGISTRY=true

   # Option 2: Update config in etcd
   docker exec etcd etcdctl put /config/air-quality/features/stream_registry true

   # Restart air-quality-app
   docker compose restart air-quality-app

   # Watch logs for stream registry initialization
   docker logs -f air-quality-app | grep -i "stream\|registry"
   # Expected: "Stream registry enabled", "Loaded stream: air-quality"

   # Verify stream registration
   curl http://localhost:8080/api/v1/streams
   # Expected: [{"id": "air-quality", "enabled": true, ...}]
   ```

**Integration Points**:
- EXISTING: etcd service (reused)
- EXISTING: neural-network (bridge network)
- NEW: Stream registry schema in etcd
- NEW: air-quality-app stream registry client

**Deliverables**:
- `deploy/pi/configs/streams/air-quality/{config,schema,sources}.yaml`
- `deploy/pi/scripts/load-stream-configs.sh`
- Updated air-quality-app code (stream registry support)
- Stream registry test results

**Validation**:
```bash
# Comprehensive validation
cd /workspaces/neural-data-platform/deploy/pi

# Load streams
./scripts/load-stream-configs.sh

# Verify etcd contains streams
docker exec etcd etcdctl get /streams/ --prefix --keys-only | wc -l
# Expected: >= 3 (config, schema, sources for air-quality)

# Test API endpoints
curl http://localhost:8080/api/v1/streams | jq '.'
curl http://localhost:8080/api/v1/streams/air-quality | jq '.'

# Resource usage check
docker stats --no-stream
# Verify no significant increase in memory
```

**Rollback Procedure**:
```bash
# If stream registry causes issues, disable feature flag

# Option 1: Quick disable via config
docker exec etcd etcdctl put /config/air-quality/features/stream_registry false
docker compose restart air-quality-app

# Option 2: Environment variable override
docker compose stop air-quality-app
# Edit docker-compose.yml: Set ENABLE_STREAM_REGISTRY=false or remove
docker compose up -d air-quality-app

# Verify stream registry disabled
docker logs air-quality-app | grep "stream registry"
# Expected: "Stream registry disabled" or no stream registry logs

# Clean up stream registry data (optional)
docker exec etcd etcdctl del /streams/ --prefix
```

---

#### Phase 3: Multi-Source Ingestion Support (Week 2-3)

**Objective**: Add HTTP polling and webhook sources alongside MQTT

**Tasks**:

1. **Implement Generic Ingestion Coordinator** (3 days)
   - Abstract source types: MQTT, HTTP Polling, Webhook
   - Pluggable source adapters
   - Unified message queue
   - Source health monitoring

2. **Add HTTP Polling Source** (2 days)
   ```yaml
   # Example: Weather API polling
   # Location: deploy/pi/configs/streams/weather/sources.yaml

   sources:
     - id: http-openweather
       type: http_poll
       enabled: true
       config:
         url: https://api.openweathermap.org/data/2.5/weather?q=Location&appid=${API_KEY}
         method: GET
         interval_seconds: 300  # Poll every 5 minutes
         timeout_seconds: 10
         headers:
           User-Agent: neural-data-platform/1.0
         transform: openweather_v1
       health_check:
         enabled: true
         interval_seconds: 600
   ```

3. **Add Webhook Source** (2 days)
   ```yaml
   # Example: Third-party webhook
   # Location: deploy/pi/configs/streams/events/sources.yaml

   sources:
     - id: webhook-github
       type: webhook
       enabled: true
       config:
         path: /webhooks/github
         method: POST
         authentication:
           type: hmac_sha256
           secret_key: ${WEBHOOK_SECRET}
         transform: github_webhook_v1
       health_check:
         enabled: false  # Webhooks are passive
   ```

4. **Update docker-compose.yml for Webhook Port** (1 hour)
   ```yaml
   # Location: /workspaces/neural-data-platform/deploy/pi/docker-compose.yml

   services:
     air-quality-app:
       # ... existing config ...
       ports:
         - "8080:8080"     # HTTP API
         - "9090:9090"     # Metrics
         - "8081:8081"     # NEW: Webhook ingestion endpoint
   ```

5. **Test Multi-Source Ingestion** (1 day)
   ```bash
   # Test MQTT source (existing)
   docker exec mqtt-broker mosquitto_pub -h localhost -p 1883 \
     -t "airgradient/test/measures" \
     -m '{"pm25":15.0,"co2":700,"temperature":21.0,"humidity":45.0}'

   # Test HTTP polling source
   # (Configured to poll external API automatically)
   docker logs air-quality-app | grep "http_poll"
   # Expected: "Polling http-openweather: success"

   # Test webhook source
   PI_IP=$(hostname -I | awk '{print $1}')
   curl -X POST http://${PI_IP}:8081/webhooks/github \
     -H "Content-Type: application/json" \
     -H "X-Hub-Signature-256: sha256=..." \
     -d '{"event":"push","repository":"test"}'

   # Verify all sources active
   curl http://localhost:8080/api/v1/sources | jq '.'
   # Expected: [{"id":"mqtt-airgradient","status":"active"}, {"id":"http-openweather","status":"active"}, ...]
   ```

**Deliverables**:
- Generic ingestion coordinator in air-quality-app
- HTTP polling source adapter
- Webhook source adapter
- Updated docker-compose.yml (webhook port)
- Multi-source test results

**Validation**:
```bash
# Verify all source types working
curl http://localhost:8080/api/v1/sources | jq '.[] | {id, type, status}'

# Check resource usage (should still be within Pi limits)
docker stats --no-stream

# Verify data from multiple sources
curl http://localhost:8080/api/v1/streams/air-quality/latest
curl http://localhost:8080/api/v1/streams/weather/latest
```

**Rollback Procedure**:
```bash
# Disable specific sources via etcd
docker exec etcd etcdctl put /streams/weather/sources/http-openweather/enabled false
docker exec etcd etcdctl put /streams/events/sources/webhook-github/enabled false

# Restart to apply changes
docker compose restart air-quality-app

# If webhook port causes issues, remove from docker-compose.yml
# Edit deploy/pi/docker-compose.yml: Remove port 8081
docker compose up -d air-quality-app
```

---

#### Phase 4: Bronze-Only Multi-Stream Storage (Week 3)

**Objective**: Store multiple stream types in Parquet (Bronze layer) with stream isolation

**Tasks**:

1. **Implement Stream-Isolated Parquet Storage** (2 days)
   ```
   /app/data/
   ├── air-quality/
   │   ├── 2025-12-15_00.parquet
   │   └── 2025-12-15_01.parquet
   ├── weather/
   │   ├── 2025-12-15_00.parquet
   │   └── 2025-12-15_01.parquet
   └── events/
       ├── 2025-12-15_00.parquet
       └── 2025-12-15_01.parquet
   ```

2. **Update Parquet Writer for Schema Flexibility** (2 days)
   - Dynamic schema from stream registry
   - Schema validation against stream definition
   - Backward compatibility with single-stream storage

3. **Test Multi-Stream Parquet Storage** (1 day)
   ```bash
   # Verify stream isolation
   docker exec air-quality-app find /app/data -type d
   # Expected: /app/data/air-quality, /app/data/weather, /app/data/events

   # Check Parquet files per stream
   docker exec air-quality-app find /app/data/air-quality -name "*.parquet" | wc -l
   docker exec air-quality-app find /app/data/weather -name "*.parquet" | wc -l

   # Verify schemas match stream definitions
   # (Use Parquet tools or API endpoint)
   curl http://localhost:8080/api/v1/streams/air-quality/schema
   ```

**Deliverables**:
- Stream-isolated Parquet storage
- Dynamic schema Parquet writer
- Multi-stream storage test results

**Validation**:
```bash
# Data volume structure check
docker exec air-quality-app tree /app/data -L 2

# Schema validation
for stream in air-quality weather events; do
  echo "Checking schema for $stream"
  curl http://localhost:8080/api/v1/streams/$stream/schema | jq '.fields[] | .name'
done

# Resource usage (disk space on Pi)
docker exec air-quality-app du -sh /app/data/*
```

**Rollback Procedure**:
```bash
# Disable non-air-quality streams
docker exec etcd etcdctl put /streams/weather/enabled false
docker exec etcd etcdctl put /streams/events/enabled false
docker compose restart air-quality-app

# Move non-air-quality data to backup (optional)
docker exec air-quality-app mkdir -p /app/data/backup
docker exec air-quality-app mv /app/data/weather /app/data/backup/
docker exec air-quality-app mv /app/data/events /app/data/backup/
```

---

#### Phase 5: Optional TimescaleDB Silver Layer (Week 4 - FUTURE)

**Objective**: Add TimescaleDB service to Pi stack for Silver layer (queryable SQL)

**CAUTION**: TimescaleDB adds significant resource overhead. Only proceed if Pi has sufficient resources.

**Pre-Requisites**:
- Pi has >= 2GB free memory
- Fast storage (USB SSD, not SD card)
- Low-latency network for remote queries

**Tasks**:

1. **Add TimescaleDB Service to docker-compose.yml** (1 day)
   ```yaml
   # Location: /workspaces/neural-data-platform/deploy/pi/docker-compose.yml

   services:
     # ... existing services ...

     timescaledb:
       image: timescale/timescaledb:latest-pg14
       container_name: timescaledb
       ports:
         - "127.0.0.1:5432:5432"  # Localhost only
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
             memory: 1G  # Significant overhead

   volumes:
     # ... existing volumes ...
     timescaledb-data:
       driver: local
   ```

2. **Apply TimescaleDB Migration for Multi-Stream Schema** (1 day)
   - Similar to Phase 4 in original document
   - Create hypertables per stream type
   - Set up compression and retention policies

3. **Enable Dual-Write in air-quality-app** (2 days)
   - Bronze (Parquet) + Silver (TimescaleDB)
   - Async writes to avoid blocking
   - Error handling: log and continue

4. **Test Dual-Write and Query Performance** (1 day)
   ```bash
   # Verify both storage layers
   docker exec air-quality-app ls -lh /app/data/air-quality/
   docker exec timescaledb psql -U neural -d neural_data -c \
     "SELECT COUNT(*) FROM air_quality_measurements;"

   # Query performance test
   docker exec timescaledb psql -U neural -d neural_data -c \
     "SELECT * FROM air_quality_measurements ORDER BY timestamp DESC LIMIT 10;"
   ```

**Deliverables**:
- Updated docker-compose.yml (TimescaleDB service)
- TimescaleDB migration scripts
- Dual-write implementation in air-quality-app
- Performance test results

**Validation**:
```bash
# Resource check (CRITICAL on Pi)
docker stats --no-stream
# Ensure total memory < 75% of Pi capacity

# Data consistency check
# Compare record counts between Parquet and TimescaleDB
```

**Rollback Procedure**:
```bash
# If TimescaleDB overloads Pi, disable Silver layer

# Stop TimescaleDB service
docker compose stop timescaledb

# Disable dual-write
docker exec etcd etcdctl put /config/air-quality/storage/silver/enabled false
docker compose restart air-quality-app

# Remove TimescaleDB service from docker-compose.yml (optional)
# Remove timescaledb service definition
docker compose up -d

# Clean up TimescaleDB volume (optional)
docker volume rm pi_timescaledb-data
```

---

#### Phase 6: Stream-Agnostic Dashboards (Week 4-5 - FUTURE)

**Objective**: Generic monitoring and visualization for all stream types

**Tasks**:

1. **Add Grafana Service to Pi Stack** (1 day)
   ```yaml
   # Location: /workspaces/neural-data-platform/deploy/pi/docker-compose.yml

   services:
     grafana:
       image: grafana/grafana:latest
       container_name: grafana
       ports:
         - "3000:3000"
       environment:
         - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD}
       volumes:
         - grafana-data:/var/lib/grafana
         - ./grafana/provisioning:/etc/grafana/provisioning
       networks:
         - neural-network
       depends_on:
         - timescaledb  # If using TimescaleDB
       deploy:
         resources:
           limits:
             memory: 256M

   volumes:
     grafana-data:
       driver: local
   ```

2. **Create Generic Stream Dashboards** (2 days)
   - Template dashboard for any stream type
   - Dynamic panel generation based on stream schema
   - Alert rules from stream alert_thresholds

3. **Test Dashboard Auto-Generation** (1 day)
   ```bash
   # Access Grafana
   PI_IP=$(hostname -I | awk '{print $1}')
   echo "Grafana URL: http://${PI_IP}:3000"

   # Verify dashboards exist for all streams
   curl -u admin:${GRAFANA_PASSWORD} http://localhost:3000/api/dashboards/db/air-quality
   curl -u admin:${GRAFANA_PASSWORD} http://localhost:3000/api/dashboards/db/weather
   ```

**Deliverables**:
- Grafana service in docker-compose.yml
- Generic dashboard templates
- Dashboard provisioning configs

**Validation**:
```bash
# Resource check
docker stats --no-stream

# Dashboard accessibility test
curl -u admin:${GRAFANA_PASSWORD} http://localhost:3000/api/search | jq '.'
```

**Rollback Procedure**:
```bash
# Stop Grafana if resource constraints
docker compose stop grafana

# Or remove from docker-compose.yml
# Remove grafana service definition
docker compose up -d
```

---

## 4. Operational Runbook

### 4.1 Daily Operations

**Deployment**:
```bash
cd /workspaces/neural-data-platform/deploy/pi
./deploy.sh start
```

**Monitoring**:
```bash
# Check service health
./deploy.sh status

# View logs
./deploy.sh logs

# Check resource usage
docker stats --no-stream
```

**Configuration Updates**:
```bash
# Update config files
vim /workspaces/neural-data-platform/config/overlays/production/overrides.yaml

# Sync to etcd
cd /workspaces/neural-data-platform
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production

# Restart services if needed
cd deploy/pi
docker compose restart air-quality-app
```

**Stream Management**:
```bash
# Add new stream
mkdir -p deploy/pi/configs/streams/new-stream
vim deploy/pi/configs/streams/new-stream/{config,schema,sources}.yaml

# Load to etcd
deploy/pi/scripts/load-stream-configs.sh

# Verify stream registered
curl http://localhost:8080/api/v1/streams
```

---

### 4.2 Troubleshooting

**Service Not Starting**:
```bash
# Check logs
docker logs air-quality-app
docker logs etcd
docker logs mqtt-broker

# Check dependencies
docker compose ps

# Rebuild if needed
./deploy.sh build
```

**High Resource Usage**:
```bash
# Identify culprit
docker stats --no-stream

# Check disk space
docker exec air-quality-app df -h /app/data

# Clean old Parquet files
docker exec air-quality-app find /app/data -name "*.parquet" -mtime +30 -delete
```

**MQTT Ingestion Failure**:
```bash
# Check MQTT broker
docker exec mqtt-broker mosquitto_sub -t "#" -v

# Check air-quality-app MQTT connection
docker logs air-quality-app | grep -i mqtt

# Restart mosquitto
docker compose restart mosquitto
```

**etcd Connection Issues**:
```bash
# Check etcd health
docker exec etcd etcdctl endpoint health

# Check etcd data
docker exec etcd etcdctl get / --prefix --keys-only

# Restore from snapshot if needed
docker cp product/features/air-004/pi-baseline-snapshot/etcd-snapshot.db etcd:/tmp/
docker exec etcd etcdctl snapshot restore /tmp/etcd-snapshot.db
```

---

## 5. Rollback Procedures

### 5.1 Complete Rollback to Baseline

**Scenario**: All multi-stream changes need to be reverted

```bash
# Stop current services
cd /workspaces/neural-data-platform/deploy/pi
docker compose down

# Restore baseline docker-compose.yml
cp /workspaces/neural-data-platform/product/features/air-004/pi-baseline-snapshot/docker-compose.yml \
   /workspaces/neural-data-platform/deploy/pi/

# Restore baseline deploy.sh
cp /workspaces/neural-data-platform/product/features/air-004/pi-baseline-snapshot/deploy.sh \
   /workspaces/neural-data-platform/deploy/pi/

# Restore etcd snapshot
docker compose up -d etcd
sleep 10
docker cp product/features/air-004/pi-baseline-snapshot/etcd-snapshot.db etcd:/tmp/
docker exec etcd sh -c "etcdctl snapshot restore /tmp/etcd-snapshot.db --data-dir=/etcd-data-restore"
docker compose stop etcd
docker exec etcd mv /etcd-data /etcd-data-old
docker exec etcd mv /etcd-data-restore /etcd-data
docker compose up -d etcd

# Restart all services
./deploy.sh start

# Verify baseline restored
./deploy.sh status
docker compose ps
# Expected: mqtt-broker, etcd, air-quality-app (3 services)
```

---

### 5.2 Partial Rollback (Per Phase)

**Phase 2 Rollback** (Stream Registry):
```bash
# Disable stream registry
docker exec etcd etcdctl del /streams/ --prefix
docker exec etcd etcdctl put /config/air-quality/features/stream_registry false
docker compose restart air-quality-app
```

**Phase 3 Rollback** (Multi-Source):
```bash
# Disable HTTP polling and webhook sources
docker exec etcd etcdctl put /streams/weather/enabled false
docker exec etcd etcdctl put /streams/events/enabled false

# Remove webhook port from docker-compose.yml
# Edit deploy/pi/docker-compose.yml, remove port 8081
docker compose up -d air-quality-app
```

**Phase 4 Rollback** (Multi-Stream Storage):
```bash
# Keep only air-quality stream
docker exec etcd etcdctl put /streams/weather/enabled false
docker exec etcd etcdctl put /streams/events/enabled false
docker compose restart air-quality-app

# Backup non-air-quality data
docker exec air-quality-app mkdir -p /app/data/backup
docker exec air-quality-app mv /app/data/weather /app/data/backup/
docker exec air-quality-app mv /app/data/events /app/data/backup/
```

**Phase 5 Rollback** (TimescaleDB):
```bash
# Stop TimescaleDB
docker compose stop timescaledb

# Disable dual-write
docker exec etcd etcdctl put /config/air-quality/storage/silver/enabled false
docker compose restart air-quality-app

# Remove from docker-compose.yml (optional)
# Edit deploy/pi/docker-compose.yml, remove timescaledb service
docker compose up -d
```

---

## 6. Future Extensions

### 6.1 Remote TimescaleDB

**Scenario**: Pi storage limited, use remote TimescaleDB server

```yaml
# air-quality-app configuration
storage:
  silver:
    enabled: true
    host: remote-timescaledb.example.com  # Remote server
    port: 5432
    database: neural_data
    user: neural
    password: ${REMOTE_DB_PASSWORD}
```

---

### 6.2 Edge Aggregation

**Scenario**: Multiple Pi devices, central aggregator

```
Pi-1 (Site A) ─┐
Pi-2 (Site B) ─┼─> Central Aggregator (TimescaleDB + Grafana)
Pi-3 (Site C) ─┘
```

Each Pi runs Bronze-only (Parquet), central aggregator collects and merges.

---

### 6.3 Stream Correlation

**Scenario**: Correlate air quality with weather data

```sql
-- TimescaleDB query
SELECT
  a.timestamp,
  a.pm25,
  w.temperature,
  w.humidity
FROM air_quality_measurements a
JOIN weather_measurements w ON
  time_bucket('5 minutes', a.timestamp) = time_bucket('5 minutes', w.timestamp)
WHERE a.timestamp > NOW() - INTERVAL '1 day'
ORDER BY a.timestamp DESC;
```

---

## Appendix A: File Locations Reference

**Pi Deployment Files**:
- Deployment: `/workspaces/neural-data-platform/deploy/pi/`
- Docker Compose: `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`
- Deploy Script: `/workspaces/neural-data-platform/deploy/pi/deploy.sh`
- Mosquitto Config: `/workspaces/neural-data-platform/deploy/pi/mosquitto/mosquitto.conf`
- Stream Configs: `/workspaces/neural-data-platform/deploy/pi/configs/streams/`

**Configuration Files**:
- Base Config: `/workspaces/neural-data-platform/config/base/air-quality.yaml`
- Production Overlay: `/workspaces/neural-data-platform/config/overlays/production/overrides.yaml`
- Sync Script: `/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh`

**Baseline Snapshot**:
- Snapshot Location: `/workspaces/neural-data-platform/product/features/air-004/pi-baseline-snapshot/`

**Docker Volumes**:
- air-quality-data: Parquet files (Bronze layer)
- etcd-data: etcd persistent storage
- mosquitto-data: MQTT broker persistence
- mosquitto-logs: MQTT broker logs

**Container Names**:
- mqtt-broker: Mosquitto MQTT broker
- etcd: Configuration store
- air-quality-app: Main application

---

## Appendix B: Resource Usage Guidelines

**Baseline (3 services)**:
- mosquitto: ~50MB actual, 128MB limit
- etcd: ~100MB actual, 256MB limit
- air-quality-app: ~200MB actual, 512MB limit
- **Total**: ~350MB actual, ~900MB limits

**With Multi-Stream (3 services + overhead)**:
- mosquitto: ~80MB (more topics)
- etcd: ~150MB (stream registry)
- air-quality-app: ~300MB (multiple sources)
- **Total**: ~530MB actual

**With TimescaleDB (4 services)**:
- Add timescaledb: ~600MB actual, 1GB limit
- **Total**: ~1.1GB actual, ~2GB limits
- **Recommendation**: Pi with >=4GB RAM

**With Grafana (5 services)**:
- Add grafana: ~150MB actual, 256MB limit
- **Total**: ~1.25GB actual, ~2.3GB limits
- **Recommendation**: Pi with >=4GB RAM, fast storage

---

## Appendix C: Network Ports Reference

**Default Ports**:
- 1883: MQTT broker (mosquitto)
- 2379: etcd client API
- 8080: Air-quality-app HTTP API
- 8081: Webhook ingestion (Phase 3)
- 9090: Air-quality-app metrics (Prometheus)
- 5432: TimescaleDB PostgreSQL (Phase 5, localhost only)
- 3000: Grafana UI (Phase 6)

**Security Notes**:
- All ports bound to Pi IP address (accessible on local network)
- TimescaleDB bound to 127.0.0.1 (localhost only)
- Use firewall rules to restrict external access
- Consider VPN for remote access

---

## Document Version History

**v3.0.0** (2025-12-15):
- CORRECTED all references to use actual Pi deployment paths (`deploy/pi/`)
- Removed incorrect `docker/production/` references
- Added Pi-specific resource constraints and optimizations
- Updated deployment procedures to match `deploy/pi/deploy.sh`
- Added Pi environment details (Raspberry Pi 5, Ubuntu 25.04)
- Corrected volume names to use `pi_` prefix
- Updated service names to match actual Pi deployment (mqtt-broker, etcd, air-quality-app)
- Added Pi production environment constraints section
- Ensured rollback procedures work with Pi deployment structure

**v2.0.0** (Previous):
- Revised to reflect operational infrastructure (incorrect paths)

**v1.0.0** (Original):
- Initial theoretical design

---

**END OF DOCUMENT**
