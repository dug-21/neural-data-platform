# AIR-004: Generic Multi-Stream Data Platform - SPARC Completion (REVISED)

## Document Status

**Status**: Revised to Reflect Current Infrastructure
**Version**: 2.0.0
**Last Updated**: 2025-12-15
**Related Documents**:
- [Platform Architecture](/workspaces/neural-data-platform/product/features/air-004/architecture/PLATFORM_ARCHITECTURE.md)
- [AIR-003 Implementation](/workspaces/neural-data-platform/product/features/air-003/)
- [AIR-002 Configuration System](/workspaces/neural-data-platform/product/features/air-002/)

**CRITICAL**: This document has been revised to reflect the ACTUAL operational deployment infrastructure, not theoretical designs.

---

## Executive Summary

This document provides the complete integration and deployment plan for transforming the neural-data-platform from a single-stream air quality system into a generic multi-stream data platform. The design builds on **EXISTING OPERATIONAL INFRASTRUCTURE** (current docker-compose deployments, TimescaleDB service, Prometheus/Grafana stack) while introducing new capabilities for heterogeneous data ingestion, multi-stream correlation, and predictive analytics.

**Current Operational Baseline**:
- Development: Mosquitto + etcd + air-quality-app stack (docker-compose.yml)
- Production: 11-service docker-compose stack (neural_trader_timescaledb, Redis, monitoring)
- Build/Deploy: build.sh (5 images) + deploy.sh (validation + health checks)

**Key Deliverables**:
1. Stream Registry in etcd with hot-reload capability
2. Generic ingestion coordinator supporting MQTT, HTTP polling, and webhooks
3. Dual-layer storage (Bronze Parquet + Silver TimescaleDB) using EXISTING neural_trader_timescaledb
4. Stream-agnostic dashboards and monitoring using EXISTING Grafana/Prometheus
5. Migration path from single-stream to multi-stream architecture with ROLLBACK to current working state

**Timeline**: 6 phases, 4-6 weeks total

---

## Current Operational Baseline

### Development Infrastructure (docker-compose.yml)

**Services**:
1. **mosquitto** (eclipse-mosquitto:2.0)
   - Ports: 1883 (MQTT), 9001 (WebSocket)
   - Volumes: mosquitto-data, mosquitto-logs
   - Config: ./mosquitto/config/mosquitto.conf
   - Health: mosquitto_sub test every 30s

2. **etcd** (quay.io/coreos/etcd:v3.5.11)
   - Ports: 2379 (client), 2380 (peer)
   - Volume: etcd-data
   - Single-node cluster (etcd0)

3. **air-quality-app** (built from Dockerfile)
   - Container: neural-air-quality
   - Ports: 8080 (HTTP API), 9090 (metrics)
   - Volumes: air-quality-data → /data, air-quality-models → /models
   - Config: /config/air-quality.yaml + /config/overrides.yaml
   - Environment: MQTT_BROKER_URL, ETCD_ENDPOINTS, DATA_DIR, MODELS_DIR

4. **prometheus** (prom/prometheus:latest) - Optional profile
   - Port: 9091:9090
   - Volume: prometheus-data

5. **grafana** (grafana/grafana:latest) - Optional profile
   - Port: 3000
   - Volume: grafana-data

**Network**: neural-network (bridge driver)

**Volumes**: mosquitto-data, mosquitto-logs, air-quality-data, air-quality-models, prometheus-data, grafana-data, etcd-data

---

### Production Infrastructure (docker/production/docker-compose.prod.yml)

**CRITICAL**: This is a SEPARATE neural-trader stack, NOT the air-quality infrastructure.

**Services** (11 total):

1. **timescaledb** (neural-trader/timescaledb:prod)
   - Container: neural_trader_timescaledb
   - Port: 127.0.0.1:5433:5432
   - Volume: timescaledb_data
   - Memory: 2GB limit, 1GB reservation
   - Network: neural_trader_internal

2. **redis** (redis:7-alpine)
   - Container: neural_trader_redis
   - LRU eviction policy, 512MB max memory
   - Volume: redis_data
   - Memory: 768M limit, 256M reservation

3. **neural-trader** (neural-trader:prod)
   - Container: neural_trader_app
   - Ports: 127.0.0.1:8080:8080 (MCP), 127.0.0.1:9092:9092 (metrics)
   - Volumes: neural_trader_models → /opt/neural-trader, neural_trader_logs
   - Memory: 4G limit, 2GB reservation, 2 CPUs
   - Networks: neural_trader_internal, monitoring

4. **data-ingestion** (neural-trader/data-ingestion:prod)
   - Container: neural_trader_data_ingestion
   - Port: 127.0.0.1:8002:8001
   - Volume: /Volumes/OneTouch/trader/polygon_data → /data
   - Memory: 2G limit, 1GB reservation, 1 CPU

5. **prometheus** (neural-trader/prometheus:prod)
   - Container: neural_trader_prometheus
   - Port: 127.0.0.1:9093:9090
   - Volume: prometheus_data
   - Retention: 30 days
   - Memory: 1G limit, 512M reservation

6. **grafana** (neural-trader/grafana:prod)
   - Container: neural_trader_grafana
   - Port: 127.0.0.1:3000:3000
   - Volume: grafana_data
   - Memory: 512M limit, 256M reservation
   - Networks: monitoring, neural_trader_internal (for TimescaleDB)

7. **postgres-exporter** (quay.io/prometheuscommunity/postgres-exporter:latest)
   - Metrics for TimescaleDB
   - Networks: neural_trader_internal, monitoring

8. **redis-exporter** (oliver006/redis_exporter:latest)
   - Metrics for Redis cache
   - Networks: neural_trader_internal, monitoring

9. **node-exporter** (prom/node-exporter:latest)
   - System metrics
   - Network: monitoring

**Networks**:
- neural_trader_internal: Backend services (TimescaleDB, Redis, app)
- monitoring: Isolated monitoring stack

**Volumes**:
- timescaledb_data, redis_data, prometheus_data, grafana_data
- neural_trader_models, neural_trader_logs, data_ingestion_logs
- neural_trader_persistence

**Build Process** (build.sh - DOES NOT EXIST):
- Expected images: neural-trader:prod, neural-trader/timescaledb:prod, neural-trader/prometheus:prod, neural-trader/grafana:prod, neural-trader/data-ingestion:prod

**Deployment Process** (/workspaces/neural-data-platform/scripts/deploy.sh):
1. Pre-deployment checks (Docker, docker-compose, config files)
2. Security validation (no .env in git, no hardcoded passwords)
3. Database backup (pg_dump, Redis BGSAVE)
4. Image build (docker-compose build --pull --parallel)
5. Service deployment (docker-compose down + up -d)
6. Health checks (300s timeout, per-service validation)
7. Smoke tests (health endpoint, metrics endpoint, DB connectivity)
8. Rollback on failure (restore from backup)

---

### Production Air Quality Infrastructure (docker-compose.prod.yml)

**CRITICAL**: This is the ACTUAL air-quality stack for Raspberry Pi 5 deployment.

**Services** (2 total):

1. **mosquitto** (eclipse-mosquitto:2.0)
   - Container: pi5-mosquitto
   - Port: 1883
   - Volumes: /opt/neural/data/mosquitto, /opt/neural/logs/mosquitto
   - Memory: 256M limit, 128M reservation, 0.5 CPU limit

2. **air-quality-app** (ghcr.io/neural-data-platform/air-quality:latest)
   - Container: pi5-air-quality
   - Ports: 8080 (API), 9090 (metrics)
   - Volumes: /opt/neural/data/air-quality → /data, /opt/neural/models → /models
   - Memory: 1792M limit (~1.75GB), 1024M reservation, 2 CPU limit
   - Environment: RAYON_NUM_THREADS=2, TOKIO_WORKER_THREADS=2 (Pi optimizations)

**Network**: pi5-neural-network (bridge driver)

**Missing Services**:
- NO etcd in production (config loaded from files only)
- NO Prometheus/Grafana in production (lightweight deployment)

---

## Integration Strategy: Merge Neural-Trader and Air-Quality Stacks

### Current State Analysis

**Two Independent Stacks**:
1. **Neural-Trader Stack** (Production): TimescaleDB + Redis + Prometheus + Grafana + 3 exporters
2. **Air-Quality Stack** (Development): Mosquitto + etcd + air-quality-app
3. **Air-Quality Stack** (Production Pi): Mosquitto + air-quality-app (minimal)

**Integration Approach**:
- **Target**: Merge air-quality services INTO neural-trader production stack
- **Rationale**: Reuse existing TimescaleDB (neural_trader_timescaledb), monitoring (Prometheus/Grafana), exporters
- **Benefit**: Single operational stack, shared monitoring, unified database

**New Production Topology** (Target):
```
docker/production/docker-compose.prod.yml (UNIFIED):
├── Infrastructure Layer
│   ├── timescaledb (EXISTING neural_trader_timescaledb) ← air-quality writes here
│   ├── redis (EXISTING neural_trader_redis) ← optional caching
│   ├── mosquitto (NEW - from air-quality stack)
│   └── etcd (NEW - from air-quality dev stack)
├── Application Layer
│   ├── neural-trader (EXISTING)
│   ├── data-ingestion (EXISTING)
│   └── air-quality-server (NEW - Rust binary)
├── Monitoring Layer (EXISTING)
│   ├── prometheus
│   ├── grafana
│   ├── postgres-exporter
│   ├── redis-exporter
│   └── node-exporter
└── Networks
    ├── neural_trader_internal (backend + air-quality)
    └── monitoring (isolated)
```

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

**Core Principle**: Verify existing functionality, then extend incrementally

- **BEFORE ANY CHANGES**: Verify current air-quality-app and neural-trader stacks work independently
- Preserve existing air-quality-app functionality throughout migration
- Introduce new components alongside existing ones (feature flags)
- Enable gradual activation with rollback to current working state
- Reuse EXISTING infrastructure (TimescaleDB, Prometheus, Grafana) instead of recreating

**Risk Mitigation**:
- Phase 1 is VERIFICATION ONLY (no code changes)
- All phases include rollback procedures to restore current state
- Blue-green deployment for zero-downtime migration
- Continuous health monitoring during migration

---

### 1.2 Phase-by-Phase Integration Plan

#### Phase 1: Baseline Verification and Documentation (Week 1)

**Objective**: Verify existing functionality BEFORE making any changes, document current operational state

**CRITICAL**: This phase is VERIFICATION ONLY. No code changes, no infrastructure changes.

**Tasks**:

1. **Verify Development Air-Quality Stack** (1 day)
   ```bash
   # Location: /workspaces/neural-data-platform

   # Start development stack
   docker-compose up -d

   # Verify all services healthy
   docker-compose ps
   # Expected: mosquitto, etcd, air-quality-app all "Up (healthy)"

   # Test MQTT ingestion
   mosquitto_pub -h localhost -p 1883 -t "airgradient/test/measures" \
     -m '{"pm25":12.3,"co2":650,"temperature":22.1,"humidity":45.2}'

   # Verify data storage (Parquet files)
   ls -lh /var/lib/docker/volumes/neural-data-platform_air-quality-data/_data/

   # Test API endpoints
   curl http://localhost:8080/health
   curl http://localhost:8080/api/v1/air-quality/latest

   # Test metrics
   curl http://localhost:9090/metrics | grep air_quality

   # Verify etcd connectivity
   docker exec etcd etcdctl endpoint health
   docker exec etcd etcdctl get /config/air-quality --prefix
   ```

2. **Verify Production Neural-Trader Stack** (1 day)
   ```bash
   # Location: /workspaces/neural-data-platform/docker/production

   # Check for build script (DOES NOT EXIST - needs creation)
   ls -la ../../scripts/build.sh

   # Verify deploy script exists
   ls -la ../../scripts/deploy.sh

   # Build production images (manual process)
   # TODO: Create build.sh script following deploy.sh expectations

   # Start production stack
   cd /workspaces/neural-data-platform/docker/production
   docker-compose -f docker-compose.prod.yml up -d

   # Verify all 11 services healthy
   docker-compose -f docker-compose.prod.yml ps
   # Expected: timescaledb, redis, neural-trader, data-ingestion, prometheus, grafana, 3 exporters

   # Test TimescaleDB connection
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c "\dt"

   # Test Redis
   docker exec neural_trader_redis redis-cli PING

   # Test Prometheus targets
   curl http://localhost:9093/api/v1/targets | jq '.data.activeTargets[] | {job:.labels.job, health:.health}'

   # Test Grafana datasources
   curl -u admin:${GRAFANA_ADMIN_PASSWORD} http://localhost:3000/api/datasources | jq '.[].name'
   ```

3. **Document Current Data Flows** (4 hours)
   - Air-Quality: MQTT → air-quality-app → Parquet (Bronze) → /data volume
   - Neural-Trader: Data-Ingestion → TimescaleDB → neural_trader_timescaledb volume
   - Monitoring: Apps → Prometheus → Grafana dashboards

4. **Document Current Configuration Hierarchy** (4 hours)
   - Air-Quality Dev: config/base/air-quality.yaml + config/overlays/development/overrides.yaml → etcd
   - Air-Quality Prod: config/base/air-quality.yaml + config/overlays/production/overrides.yaml (file-only, NO etcd)
   - Neural-Trader: Environment variables from .env → docker-compose.prod.yml

5. **Identify Integration Points** (1 day)
   - Shared TimescaleDB: neural_trader_timescaledb service (port 5433)
   - Shared Monitoring: Prometheus (port 9093), Grafana (port 3000)
   - Network Bridge: neural_trader_internal (backend) vs monitoring (isolated)
   - Volume Strategy: Named volumes vs bind mounts

6. **Create Baseline Snapshot** (2 hours)
   ```bash
   # Backup current configurations
   mkdir -p /workspaces/neural-data-platform/product/features/air-004/baseline-snapshot

   # Docker configs
   cp docker-compose.yml docker-compose.prod.yml baseline-snapshot/
   cp docker/production/docker-compose.prod.yml baseline-snapshot/docker-compose.prod.neural-trader.yml

   # Application configs
   cp -r config/base config/overlays baseline-snapshot/

   # Database schema (if TimescaleDB has air-quality data)
   docker exec neural_trader_timescaledb pg_dump -U ${POSTGRES_USER} -d ${POSTGRES_DB} --schema-only > baseline-snapshot/timescaledb-schema.sql

   # Tag current state in git
   git add baseline-snapshot/
   git commit -m "docs(air-004): baseline snapshot before multi-stream migration"
   git tag air-004-baseline-pre-migration
   ```

**Dependencies**: None (verification only)

**Deliverables**:
- `/workspaces/neural-data-platform/product/features/air-004/baseline-snapshot/` (configs, schemas, docker-compose files)
- `/workspaces/neural-data-platform/product/features/air-004/verification-report.md` (test results, screenshots, logs)
- Git tag: `air-004-baseline-pre-migration`

**Success Criteria**:
- All development services pass health checks
- All production services pass health checks
- MQTT → Parquet pipeline verified working
- Data-Ingestion → TimescaleDB pipeline verified working
- Prometheus scraping all targets
- Grafana dashboards rendering

**Failure Criteria**:
- ANY service fails health check → STOP, fix existing issues before proceeding
- MQTT ingestion broken → STOP, fix air-quality-app
- TimescaleDB connection issues → STOP, fix neural-trader stack

---

#### Phase 2: Add Mosquitto and etcd to Production Stack (Week 1-2)

**Objective**: Extend production docker-compose.prod.yml with mosquitto and etcd services

**Tasks**:

1. **Update Production Docker Compose** (4 hours)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/docker-compose.prod.yml

   services:
     # ADD: Mosquitto MQTT Broker
     mosquitto:
       image: eclipse-mosquitto:2.0
       container_name: neural_trader_mosquitto
       hostname: mosquitto
       restart: unless-stopped
       ports:
         - "127.0.0.1:1883:1883"  # MQTT - localhost only
         - "127.0.0.1:9001:9001"  # WebSocket - localhost only
       volumes:
         - ./configs/mosquitto/mosquitto.conf:/mosquitto/config/mosquitto.conf:ro
         - mosquitto_data:/mosquitto/data
         - mosquitto_logs:/mosquitto/log
       networks:
         - neural_trader_internal
       deploy:
         resources:
           limits:
             cpus: '0.5'
             memory: 256M
           reservations:
             cpus: '0.25'
             memory: 128M
       healthcheck:
         test: ["CMD", "mosquitto_sub", "-t", "$$SYS/#", "-C", "1", "-i", "healthcheck", "-W", "3"]
         interval: 30s
         timeout: 10s
         retries: 3
         start_period: 10s

     # ADD: etcd Configuration Store
     etcd:
       image: quay.io/coreos/etcd:v3.5.11
       container_name: neural_trader_etcd
       hostname: etcd
       restart: unless-stopped
       ports:
         - "127.0.0.1:2379:2379"  # Client - localhost only
       environment:
         - ETCD_NAME=etcd0
         - ETCD_DATA_DIR=/etcd-data
         - ETCD_LISTEN_CLIENT_URLS=http://0.0.0.0:2379
         - ETCD_ADVERTISE_CLIENT_URLS=http://etcd:2379
         - ETCD_LISTEN_PEER_URLS=http://0.0.0.0:2380
         - ETCD_INITIAL_ADVERTISE_PEER_URLS=http://etcd:2380
         - ETCD_INITIAL_CLUSTER=etcd0=http://etcd:2380
         - ETCD_INITIAL_CLUSTER_TOKEN=etcd-cluster-prod
         - ETCD_INITIAL_CLUSTER_STATE=new
       volumes:
         - etcd_data:/etcd-data
       networks:
         - neural_trader_internal
       deploy:
         resources:
           limits:
             cpus: '0.5'
             memory: 512M
           reservations:
             cpus: '0.25'
             memory: 256M
       healthcheck:
         test: ["CMD", "etcdctl", "endpoint", "health"]
         interval: 10s
         timeout: 5s
         retries: 5
         start_period: 10s

   volumes:
     # ADD new volumes
     mosquitto_data:
       driver: local
     mosquitto_logs:
       driver: local
     etcd_data:
       driver: local
   ```

2. **Create Mosquitto Production Config** (2 hours)
   ```bash
   # Location: /workspaces/neural-data-platform/docker/production/configs/mosquitto/mosquitto.conf

   mkdir -p /workspaces/neural-data-platform/docker/production/configs/mosquitto
   cat > /workspaces/neural-data-platform/docker/production/configs/mosquitto/mosquitto.conf << 'EOF'
   # Mosquitto Production Configuration
   # Optimized for neural-trader stack

   # Listeners
   listener 1883
   protocol mqtt

   listener 9001
   protocol websockets

   # Authentication (anonymous for internal Docker network)
   allow_anonymous true

   # Logging
   log_dest stdout
   log_type error
   log_type warning
   log_type notice
   log_timestamp true
   log_timestamp_format %Y-%m-%dT%H:%M:%S

   # Persistence
   persistence true
   persistence_location /mosquitto/data/
   autosave_interval 300

   # Performance
   max_queued_messages 1000
   max_inflight_messages 20
   max_keepalive 300

   # Memory limits
   memory_limit 209715200  # 200MB
   EOF
   ```

3. **Test New Services in Isolation** (1 day)
   ```bash
   # Start ONLY mosquitto and etcd (not full stack)
   cd /workspaces/neural-data-platform/docker/production
   docker-compose -f docker-compose.prod.yml up -d mosquitto etcd

   # Wait for health checks
   sleep 30
   docker-compose -f docker-compose.prod.yml ps mosquitto etcd

   # Test MQTT
   docker exec neural_trader_mosquitto mosquitto_sub -t "test/topic" -C 1 &
   docker exec neural_trader_mosquitto mosquitto_pub -t "test/topic" -m "hello"

   # Test etcd
   docker exec neural_trader_etcd etcdctl endpoint health
   docker exec neural_trader_etcd etcdctl put /test "value"
   docker exec neural_trader_etcd etcdctl get /test

   # Verify resource usage
   docker stats neural_trader_mosquitto neural_trader_etcd --no-stream
   ```

4. **Integration Test with Full Stack** (1 day)
   ```bash
   # Start full production stack with new services
   cd /workspaces/neural-data-platform/docker/production
   docker-compose -f docker-compose.prod.yml up -d

   # Verify all 13 services (11 existing + 2 new)
   docker-compose -f docker-compose.prod.yml ps

   # Check network connectivity (mosquitto → timescaledb)
   docker exec neural_trader_mosquitto ping -c 3 timescaledb

   # Check network connectivity (etcd → timescaledb)
   docker exec neural_trader_etcd ping -c 3 timescaledb

   # Run health check script
   /workspaces/neural-data-platform/scripts/deploy.sh
   ```

5. **Update .env.template** (1 hour)
   ```bash
   # Location: /workspaces/neural-data-platform/docker/production/.env.template

   # ADD: MQTT Configuration
   MQTT_BROKER_URL=mqtt://mosquitto:1883
   MQTT_CLIENT_ID=neural-trader-${HOSTNAME}

   # ADD: etcd Configuration
   ETCD_ENDPOINTS=http://etcd:2379
   ETCD_NAMESPACE=/neural-trader
   ```

**Integration Points**:
- EXISTING: neural_trader_internal network
- EXISTING: deploy.sh health check logic
- NEW: mosquitto and etcd health checks

**Deliverables**:
- Updated `docker/production/docker-compose.prod.yml` (mosquitto + etcd services)
- `docker/production/configs/mosquitto/mosquitto.conf`
- Updated `.env.template`
- Integration test results

**Validation**:
```bash
# Comprehensive validation
cd /workspaces/neural-data-platform/docker/production

# Health check all services
docker-compose -f docker-compose.prod.yml ps | grep -c "(healthy)"
# Expected: 13 (all services)

# Resource usage check
docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}"
# Expected: mosquitto < 256MB, etcd < 512MB

# Network connectivity matrix
for svc in mosquitto etcd timescaledb redis prometheus; do
  echo "Testing $svc connectivity:"
  docker exec neural_trader_mosquitto ping -c 1 $svc >/dev/null 2>&1 && echo "  ✓ mosquitto → $svc" || echo "  ✗ mosquitto → $svc"
done
```

**Rollback Procedure**:
```bash
# If Phase 2 fails, restore baseline
cd /workspaces/neural-data-platform/docker/production

# Restore original docker-compose.prod.yml
cp /workspaces/neural-data-platform/product/features/air-004/baseline-snapshot/docker-compose.prod.neural-trader.yml docker-compose.prod.yml

# Remove new volumes
docker-compose -f docker-compose.prod.yml down -v
docker volume rm neural-trader_mosquitto_data neural-trader_mosquitto_logs neural-trader_etcd_data

# Restart original stack
docker-compose -f docker-compose.prod.yml up -d

# Verify original functionality restored
docker-compose -f docker-compose.prod.yml ps
```

---

#### Phase 3: Build Air-Quality Server for Production (Week 2)

**Objective**: Create production-ready Rust binary and Docker image for air-quality-server

**Tasks**:

1. **Create Multi-Stage Dockerfile** (4 hours)
   ```dockerfile
   # Location: /workspaces/neural-data-platform/apps/air-quality-app/Dockerfile.prod

   # Builder stage
   FROM rust:1.75-slim as builder

   WORKDIR /build

   # Install build dependencies
   RUN apt-get update && apt-get install -y \
       pkg-config \
       libssl-dev \
       && rm -rf /var/lib/apt/lists/*

   # Copy workspace manifests
   COPY Cargo.toml Cargo.lock ./
   COPY neural-core ./neural-core
   COPY apps/air-quality-app ./apps/air-quality-app

   # Build release binary
   RUN cargo build --release --package air-quality-app --bin air-quality-server

   # Runtime stage
   FROM debian:bookworm-slim

   # Install runtime dependencies
   RUN apt-get update && apt-get install -y \
       ca-certificates \
       libssl3 \
       curl \
       && rm -rf /var/lib/apt/lists/*

   # Create app user
   RUN useradd -m -u 1000 -s /bin/bash neural

   # Copy binary from builder
   COPY --from=builder /build/target/release/air-quality-server /usr/local/bin/

   # Create directories
   RUN mkdir -p /app/data /app/models /app/config && \
       chown -R neural:neural /app

   # Switch to app user
   USER neural
   WORKDIR /app

   # Expose ports
   EXPOSE 8080 9090

   # Health check
   HEALTHCHECK --interval=30s --timeout=10s --retries=3 --start-period=10s \
     CMD curl -f http://localhost:8080/health || exit 1

   # Run binary
   CMD ["/usr/local/bin/air-quality-server"]
   ```

2. **Update build.sh Script** (CREATE NEW) (1 day)
   ```bash
   # Location: /workspaces/neural-data-platform/scripts/build.sh

   #!/bin/bash
   # Production Image Build Script
   # Builds all 6 production images: neural-trader stack (5) + air-quality-server (1)

   set -e

   PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
   cd "$PROJECT_ROOT"

   echo "Building production images..."

   # Build neural-trader images (existing)
   echo "1/6 Building neural-trader:prod..."
   docker build -t neural-trader:prod -f docker/production/neural-trader/Dockerfile .

   echo "2/6 Building neural-trader/timescaledb:prod..."
   docker build -t neural-trader/timescaledb:prod -f docker/production/timescaledb/Dockerfile .

   echo "3/6 Building neural-trader/prometheus:prod..."
   docker build -t neural-trader/prometheus:prod -f docker/production/prometheus/Dockerfile .

   echo "4/6 Building neural-trader/grafana:prod..."
   docker build -t neural-trader/grafana:prod -f docker/production/grafana/Dockerfile .

   echo "5/6 Building neural-trader/data-ingestion:prod..."
   docker build -t neural-trader/data-ingestion:prod -f docker/production/data-ingestion/Dockerfile .

   # Build air-quality-server image (NEW)
   echo "6/6 Building neural-trader/air-quality-server:prod..."
   docker build -t neural-trader/air-quality-server:prod -f apps/air-quality-app/Dockerfile.prod .

   echo "All images built successfully!"

   # List images
   docker images | grep "neural-trader"
   ```

3. **Add air-quality-server to docker-compose.prod.yml** (2 hours)
   ```yaml
   services:
     # ADD: Air Quality Server
     air-quality-server:
       image: neural-trader/air-quality-server:prod
       container_name: neural_trader_air_quality
       hostname: air-quality-server
       restart: unless-stopped
       depends_on:
         mosquitto:
           condition: service_healthy
         etcd:
           condition: service_healthy
         timescaledb:
           condition: service_healthy
       environment:
         - RUST_LOG=${AIR_QUALITY_LOG_LEVEL:-info}
         - MQTT_BROKER_URL=mqtt://mosquitto:1883
         - ETCD_ENDPOINTS=http://etcd:2379
         # TimescaleDB connection (EXISTING service)
         - TIMESCALE_HOST=timescaledb
         - TIMESCALE_PORT=5432
         - TIMESCALE_DB=${POSTGRES_DB}
         - TIMESCALE_USER=${POSTGRES_USER}
         - TIMESCALE_PASSWORD=${POSTGRES_PASSWORD}
         # Paths
         - DATA_DIR=/app/data
         - MODELS_DIR=/app/models
         - CONFIG_PATH=/app/config/air-quality.yaml
         # Feature flags
         - ENABLE_MULTI_STREAM=false  # Default: legacy mode
         - ENABLE_DUAL_WRITE=false    # Default: Parquet only
       volumes:
         - air-quality-data:/app/data
         - air-quality-models:/app/models
         # Config from production overlay
         - ./configs/air-quality/air-quality.yaml:/app/config/air-quality.yaml:ro
       networks:
         - neural_trader_internal
         - monitoring
       ports:
         - "127.0.0.1:8081:8080"  # API (8081 to avoid conflict with neural-trader:8080)
         - "127.0.0.1:9091:9090"  # Metrics (9091 to avoid conflict)
       deploy:
         resources:
           limits:
             cpus: '1.5'
             memory: 1536M  # 1.5GB
           reservations:
             cpus: '0.5'
             memory: 512M
       healthcheck:
         test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
         interval: 30s
         timeout: 10s
         retries: 3
         start_period: 30s

   volumes:
     air-quality-data:
       driver: local
     air-quality-models:
       driver: local
   ```

4. **Create Production Config** (2 hours)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/air-quality/air-quality.yaml

   ---
   # Air Quality Server Production Configuration
   # Loaded from file (NOT etcd in Phase 3)

   server:
     http_port: 8080
     metrics_port: 9090
     log_level: info

   mqtt:
     broker_url: mqtt://mosquitto:1883
     client_id: air-quality-server-prod
     topics:
       - airgradient/+/measures
     qos: 1
     reconnect_interval_ms: 5000

   storage:
     # Bronze layer (Parquet) - EXISTING functionality
     bronze:
       enabled: true
       path: /app/data
       format: parquet
       compression: snappy
       batch_size: 100
       batch_timeout_ms: 30000

     # Silver layer (TimescaleDB) - DISABLED until Phase 4
     silver:
       enabled: false
       host: timescaledb
       port: 5432
       database: ${POSTGRES_DB}
       user: ${POSTGRES_USER}
       password: ${POSTGRES_PASSWORD}
       pool_size: 10
       connection_timeout_ms: 5000

   stream_registry:
     # etcd-based registry - DISABLED until Phase 5
     enabled: false
     etcd_endpoints:
       - http://etcd:2379
     namespace: /streams
     watch_enabled: false
   ```

5. **Test Build and Deploy** (1 day)
   ```bash
   # Build all images
   cd /workspaces/neural-data-platform
   ./scripts/build.sh

   # Verify image created
   docker images neural-trader/air-quality-server:prod

   # Start production stack with air-quality-server
   cd docker/production
   docker-compose -f docker-compose.prod.yml up -d air-quality-server

   # Check logs
   docker logs -f neural_trader_air_quality

   # Verify health
   curl http://localhost:8081/health

   # Test MQTT ingestion
   docker exec neural_trader_mosquitto mosquitto_pub \
     -t "airgradient/test/measures" \
     -m '{"pm25":15.2,"co2":680,"temperature":21.5,"humidity":42.0}'

   # Verify Parquet file created
   docker exec neural_trader_air_quality ls -lh /app/data/

   # Verify metrics exposed
   curl http://localhost:9091/metrics | grep air_quality
   ```

6. **Add Prometheus Scrape Config** (1 hour)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/prometheus/prometheus.yml

   scrape_configs:
     # EXISTING scrape configs...

     # ADD: Air Quality Server metrics
     - job_name: 'air-quality-server'
       static_configs:
         - targets: ['air-quality-server:9090']
       scrape_interval: 15s
       scrape_timeout: 10s
   ```

**Deliverables**:
- `apps/air-quality-app/Dockerfile.prod` (multi-stage production Dockerfile)
- `scripts/build.sh` (6-image build script)
- Updated `docker/production/docker-compose.prod.yml` (air-quality-server service)
- `docker/production/configs/air-quality/air-quality.yaml` (production config)
- Updated `docker/production/configs/prometheus/prometheus.yml` (air-quality metrics)

**Validation**:
```bash
# Comprehensive validation
cd /workspaces/neural-data-platform/docker/production

# Build and deploy
cd .. && ./scripts/build.sh && cd docker/production
docker-compose -f docker-compose.prod.yml up -d

# Verify 14 services running (13 from Phase 2 + air-quality-server)
docker-compose -f docker-compose.prod.yml ps | grep -c "Up"

# Test air-quality-server endpoints
curl -f http://localhost:8081/health || echo "FAIL: health check"
curl -f http://localhost:8081/api/v1/air-quality/status || echo "FAIL: API"
curl -f http://localhost:9091/metrics | grep -q air_quality || echo "FAIL: metrics"

# Verify Prometheus scraping air-quality-server
curl http://localhost:9093/api/v1/targets | jq '.data.activeTargets[] | select(.labels.job=="air-quality-server")'

# Verify no writes to TimescaleDB yet (feature flag disabled)
docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c "\dt" | grep -c air_quality
# Expected: 0 (table doesn't exist yet)

# Verify Parquet files being created
docker exec neural_trader_air_quality find /app/data -name "*.parquet" -mmin -5
```

**Rollback Procedure**:
```bash
# Stop air-quality-server
docker-compose -f docker-compose.prod.yml stop air-quality-server
docker-compose -f docker-compose.prod.yml rm -f air-quality-server

# Restore docker-compose.prod.yml from Phase 2
git checkout HEAD~1 docker/production/docker-compose.prod.yml

# Restart remaining services
docker-compose -f docker-compose.prod.yml up -d

# Verify 13 services (without air-quality-server)
docker-compose -f docker-compose.prod.yml ps
```

---

#### Phase 4: Enable Dual-Write to TimescaleDB (Week 3)

**Objective**: Write air-quality data to BOTH Parquet (Bronze) AND TimescaleDB (Silver) using EXISTING neural_trader_timescaledb service

**Tasks**:

1. **Create TimescaleDB Migration for Air-Quality Tables** (1 day)
   ```sql
   -- Location: /workspaces/neural-data-platform/docker/production/configs/timescaledb/migrations/001_air_quality_schema.sql

   -- Initialize TimescaleDB extension (if not exists)
   CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

   -- Air Quality measurements table
   CREATE TABLE IF NOT EXISTS air_quality_measurements (
       timestamp TIMESTAMPTZ NOT NULL,
       location_id TEXT NOT NULL,
       sensor_id TEXT NOT NULL,
       pm25 REAL NOT NULL,
       pm10 REAL,
       co2 INTEGER NOT NULL,
       voc INTEGER,
       temperature REAL,
       humidity REAL,
       metadata JSONB,
       CONSTRAINT air_quality_pk PRIMARY KEY (timestamp, location_id, sensor_id)
   );

   -- Create hypertable (time-partitioned)
   SELECT create_hypertable(
       'air_quality_measurements',
       'timestamp',
       if_not_exists => TRUE,
       chunk_time_interval => INTERVAL '1 day'
   );

   -- Indexes for common query patterns
   CREATE INDEX IF NOT EXISTS idx_air_quality_location_time
       ON air_quality_measurements (location_id, timestamp DESC);

   CREATE INDEX IF NOT EXISTS idx_air_quality_sensor_time
       ON air_quality_measurements (sensor_id, timestamp DESC);

   CREATE INDEX IF NOT EXISTS idx_air_quality_co2
       ON air_quality_measurements (co2, timestamp DESC)
       WHERE co2 > 1000;  -- Index only high CO2 values

   CREATE INDEX IF NOT EXISTS idx_air_quality_pm25
       ON air_quality_measurements (pm25, timestamp DESC)
       WHERE pm25 > 35.0;  -- Index only unhealthy PM2.5 levels

   -- Continuous aggregates (5-minute rollups)
   CREATE MATERIALIZED VIEW IF NOT EXISTS mv_air_quality_5min
   WITH (timescaledb.continuous) AS
   SELECT
       time_bucket('5 minutes', timestamp) as bucket,
       location_id,
       sensor_id,
       COUNT(*) as reading_count,
       AVG(pm25) as pm25_avg,
       MAX(pm25) as pm25_max,
       MIN(pm25) as pm25_min,
       STDDEV(pm25) as pm25_stddev,
       AVG(co2) as co2_avg,
       MAX(co2) as co2_max,
       MIN(co2) as co2_min,
       AVG(temperature) as temp_avg,
       AVG(humidity) as humidity_avg,
       AVG(voc) as voc_avg
   FROM air_quality_measurements
   GROUP BY bucket, location_id, sensor_id
   WITH NO DATA;

   -- Continuous aggregates (1-hour rollups)
   CREATE MATERIALIZED VIEW IF NOT EXISTS mv_air_quality_1hr
   WITH (timescaledb.continuous) AS
   SELECT
       time_bucket('1 hour', timestamp) as bucket,
       location_id,
       COUNT(*) as reading_count,
       AVG(pm25) as pm25_avg,
       MAX(pm25) as pm25_max,
       MIN(pm25) as pm25_min,
       AVG(co2) as co2_avg,
       MAX(co2) as co2_max,
       AVG(temperature) as temp_avg,
       AVG(humidity) as humidity_avg
   FROM air_quality_measurements
   GROUP BY bucket, location_id
   WITH NO DATA;

   -- Refresh policies
   SELECT add_continuous_aggregate_policy('mv_air_quality_5min',
       start_offset => INTERVAL '1 hour',
       end_offset => INTERVAL '5 minutes',
       schedule_interval => INTERVAL '5 minutes',
       if_not_exists => TRUE
   );

   SELECT add_continuous_aggregate_policy('mv_air_quality_1hr',
       start_offset => INTERVAL '3 hours',
       end_offset => INTERVAL '1 hour',
       schedule_interval => INTERVAL '1 hour',
       if_not_exists => TRUE
   );

   -- Compression policy (compress data older than 7 days)
   ALTER TABLE air_quality_measurements SET (
       timescaledb.compress,
       timescaledb.compress_segmentby = 'location_id, sensor_id',
       timescaledb.compress_orderby = 'timestamp DESC'
   );

   SELECT add_compression_policy('air_quality_measurements',
       INTERVAL '7 days',
       if_not_exists => TRUE
   );

   -- Retention policy (delete data older than 365 days)
   SELECT add_retention_policy('air_quality_measurements',
       INTERVAL '365 days',
       if_not_exists => TRUE
   );

   -- Grant permissions to application user
   GRANT SELECT, INSERT ON air_quality_measurements TO ${POSTGRES_USER};
   GRANT SELECT ON mv_air_quality_5min, mv_air_quality_1hr TO ${POSTGRES_USER};

   -- Create helper function for latest readings
   CREATE OR REPLACE FUNCTION get_latest_air_quality_readings()
   RETURNS TABLE(
       location_id TEXT,
       sensor_id TEXT,
       latest_timestamp TIMESTAMPTZ,
       pm25 REAL,
       co2 INTEGER,
       temperature REAL,
       humidity REAL
   ) AS $$
   BEGIN
       RETURN QUERY
       SELECT DISTINCT ON (a.location_id, a.sensor_id)
           a.location_id,
           a.sensor_id,
           a.timestamp,
           a.pm25,
           a.co2,
           a.temperature,
           a.humidity
       FROM air_quality_measurements a
       ORDER BY a.location_id, a.sensor_id, a.timestamp DESC;
   END;
   $$ LANGUAGE plpgsql;
   ```

2. **Apply Migration to TimescaleDB** (1 hour)
   ```bash
   # Copy migration file into container
   docker cp /workspaces/neural-data-platform/docker/production/configs/timescaledb/migrations/001_air_quality_schema.sql \
       neural_trader_timescaledb:/tmp/

   # Apply migration
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -f /tmp/001_air_quality_schema.sql

   # Verify tables created
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c "\dt"
   # Expected output includes: air_quality_measurements

   # Verify hypertable
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c "SELECT * FROM timescaledb_information.hypertables WHERE hypertable_name='air_quality_measurements';"

   # Verify continuous aggregates
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c "SELECT * FROM timescaledb_information.continuous_aggregates;"
   ```

3. **Update air-quality.yaml to Enable Dual-Write** (1 hour)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/air-quality/air-quality.yaml

   storage:
     # Bronze layer (Parquet) - KEEP ENABLED
     bronze:
       enabled: true
       path: /app/data
       format: parquet
       compression: snappy
       batch_size: 100
       batch_timeout_ms: 30000

     # Silver layer (TimescaleDB) - ENABLE
     silver:
       enabled: true  # CHANGED from false
       host: timescaledb
       port: 5432
       database: ${POSTGRES_DB}
       user: ${POSTGRES_USER}
       password: ${POSTGRES_PASSWORD}
       table: air_quality_measurements
       pool_size: 10
       connection_timeout_ms: 5000
       batch_size: 50
       batch_timeout_ms: 15000
       # Error handling
       on_error: log_and_continue  # Don't block bronze writes if silver fails
       retry_attempts: 3
       retry_delay_ms: 1000
   ```

4. **Update Environment Variables** (30 mins)
   ```bash
   # Location: /workspaces/neural-data-platform/docker/production/.env

   # ADD: Enable dual-write feature flag
   ENABLE_DUAL_WRITE=true
   ```

5. **Reload Configuration (Hot-Reload via etcd OR Restart)** (15 mins)
   ```bash
   # Option 1: Hot-reload via etcd (if watch enabled)
   # NOTE: etcd watch not enabled in Phase 3, so use restart

   # Option 2: Restart air-quality-server
   cd /workspaces/neural-data-platform/docker/production
   docker-compose -f docker-compose.prod.yml restart air-quality-server

   # Watch logs for dual-write confirmation
   docker logs -f neural_trader_air_quality | grep -i "silver\|timescale"
   # Expected: "Silver layer (TimescaleDB) enabled"
   ```

6. **Verify Dual-Write Functionality** (1 day)
   ```bash
   # Publish test message to MQTT
   docker exec neural_trader_mosquitto mosquitto_pub \
     -t "airgradient/test-sensor/measures" \
     -m '{"pm25":18.5,"pm10":25.3,"co2":720,"voc":150,"temperature":22.3,"humidity":48.5,"location_id":"office","sensor_id":"test-sensor-001"}'

   # Wait for batch timeout (15 seconds)
   sleep 20

   # Verify Bronze layer (Parquet)
   docker exec neural_trader_air_quality find /app/data -name "*.parquet" -mmin -5 -exec ls -lh {} \;

   # Verify Silver layer (TimescaleDB)
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
     "SELECT COUNT(*) FROM air_quality_measurements WHERE timestamp > NOW() - INTERVAL '5 minutes';"
   # Expected: > 0

   # Query latest readings
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
     "SELECT * FROM get_latest_air_quality_readings();"

   # Verify continuous aggregates refreshed
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
     "SELECT * FROM mv_air_quality_5min ORDER BY bucket DESC LIMIT 5;"

   # Compare record counts (Bronze vs Silver)
   PARQUET_COUNT=$(docker exec neural_trader_air_quality find /app/data -name "*.parquet" -exec wc -l {} \; | awk '{sum+=$1} END {print sum}')
   TIMESCALE_COUNT=$(docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -t -c "SELECT COUNT(*) FROM air_quality_measurements;")

   echo "Parquet record count: $PARQUET_COUNT"
   echo "TimescaleDB record count: $TIMESCALE_COUNT"
   # Expected: Counts within 1% (due to batching)
   ```

7. **Monitor Write Latency and Errors** (1 day)
   ```bash
   # Check Prometheus metrics for write performance
   curl -s http://localhost:9093/api/v1/query \
     --data-urlencode 'query=air_quality_storage_write_duration_seconds{layer="silver"}' \
     | jq '.data.result[0].value[1]'

   # Check for write errors
   curl -s http://localhost:9093/api/v1/query \
     --data-urlencode 'query=air_quality_storage_write_errors_total{layer="silver"}' \
     | jq '.data.result[0].value[1]'

   # Check TimescaleDB connection pool
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
     "SELECT datname, numbackends, xact_commit, xact_rollback FROM pg_stat_database WHERE datname='${POSTGRES_DB}';"

   # Monitor compression and retention policies
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
     "SELECT * FROM timescaledb_information.jobs WHERE proc_name IN ('policy_compression', 'policy_retention');"
   ```

**Deliverables**:
- `docker/production/configs/timescaledb/migrations/001_air_quality_schema.sql`
- Updated `docker/production/configs/air-quality/air-quality.yaml` (silver.enabled=true)
- Updated `.env` (ENABLE_DUAL_WRITE=true)
- Dual-write verification report

**Validation Checklist**:
- [ ] TimescaleDB migration applied successfully
- [ ] air_quality_measurements table created
- [ ] Hypertable partitioning enabled
- [ ] Continuous aggregates created (5min, 1hr)
- [ ] Compression policy active (7 days)
- [ ] Retention policy active (365 days)
- [ ] Air-quality-server connects to TimescaleDB
- [ ] MQTT → Bronze (Parquet) still working
- [ ] MQTT → Silver (TimescaleDB) working
- [ ] Record counts match (Bronze ≈ Silver within 1%)
- [ ] Write latency < 100ms (p95)
- [ ] No write errors in Prometheus metrics
- [ ] Continuous aggregates refreshing on schedule

**Rollback Procedure**:
```bash
# If dual-write causes issues, disable Silver layer

# Option 1: Quick disable via config
docker exec neural_trader_air_quality sed -i 's/enabled: true/enabled: false/' /app/config/air-quality.yaml
docker-compose -f docker-compose.prod.yml restart air-quality-server

# Option 2: Environment variable override
docker-compose -f docker-compose.prod.yml stop air-quality-server
docker-compose -f docker-compose.prod.yml run -e ENABLE_DUAL_WRITE=false -d air-quality-server

# Verify Bronze-only mode restored
docker logs neural_trader_air_quality | grep "Silver layer"
# Expected: "Silver layer (TimescaleDB) disabled"

# TimescaleDB data preservation
# NOTE: Even if Silver writes disabled, data remains in TimescaleDB for rollback
# To fully rollback and drop air-quality tables:
docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
  "DROP TABLE IF EXISTS air_quality_measurements CASCADE;"
```

---

#### Phase 5: Stream Registry and Multi-Stream Support (Week 4)

**Objective**: Implement stream registry in etcd, enable hot-reload, prepare for multi-stream ingestion

**Tasks**:

1. **Define Stream Registry Schema** (4 hours)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/streams/air-quality/config.yaml

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
   # Location: /workspaces/neural-data-platform/docker/production/configs/streams/air-quality/schema.yaml

   fields:
     - name: pm25
       type: float
       unit: µg/m³
       nullable: false
       range: [0, 1000]
       description: PM2.5 particulate matter

     - name: pm10
       type: float
       unit: µg/m³
       nullable: true
       range: [0, 1000]
       description: PM10 particulate matter

     - name: co2
       type: int
       unit: ppm
       nullable: false
       range: [400, 5000]
       description: Carbon dioxide concentration

     - name: voc
       type: int
       unit: index
       nullable: true
       range: [0, 500]
       description: Volatile organic compounds index

     - name: temperature
       type: float
       unit: celsius
       nullable: true
       range: [-50, 100]
       description: Ambient temperature

     - name: humidity
       type: float
       unit: percent
       nullable: true
       range: [0, 100]
       description: Relative humidity

   indexes:
     - fields: [location_id, timestamp]
       order: desc
     - fields: [sensor_id, timestamp]
       order: desc

   continuous_aggregates:
     - name: mv_air_quality_5min
       interval: 5 minutes
       refresh_interval: 5 minutes
       retention: 90 days
     - name: mv_air_quality_1hr
       interval: 1 hour
       refresh_interval: 1 hour
       retention: 365 days
   ```

   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/streams/air-quality/sources.yaml

   sources:
     - id: mqtt-airgradient
       type: mqtt
       enabled: true
       config:
         topics:
           - airgradient/+/measures
         qos: 1
         transform: airgradient_v1  # Data transformation function
       health_check:
         enabled: true
         interval_seconds: 60
   ```

2. **Create Stream Registry Loader Script** (1 day)
   ```bash
   # Location: /workspaces/neural-data-platform/scripts/load-stream-configs.sh

   #!/bin/bash
   # Load stream configurations into etcd

   set -e

   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
   PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
   ETCD_CONTAINER="${ETCD_CONTAINER:-neural_trader_etcd}"
   ETCD_ENDPOINTS="${ETCD_ENDPOINTS:-http://etcd:2379}"
   CONFIG_DIR="$PROJECT_ROOT/docker/production/configs/streams"

   echo "Loading stream configurations from $CONFIG_DIR to etcd ($ETCD_CONTAINER)"

   # Check if etcd container is running
   if ! docker ps | grep -q "$ETCD_CONTAINER"; then
       echo "ERROR: etcd container ($ETCD_CONTAINER) is not running"
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
       docker exec "$ETCD_CONTAINER" etcdctl put "$key" "$(cat "$file")"
   }

   # Iterate over stream directories
   for stream_dir in "$CONFIG_DIR"/*; do
       if [ ! -d "$stream_dir" ]; then
           continue
       fi

       stream_id=$(basename "$stream_dir")
       echo "Loading stream: $stream_id"

       # Load config
       if [ -f "$stream_dir/config.yaml" ]; then
           load_yaml_to_etcd "/streams/$stream_id/config" "$stream_dir/config.yaml"
       fi

       # Load schema
       if [ -f "$stream_dir/schema.yaml" ]; then
           load_yaml_to_etcd "/streams/$stream_id/schema" "$stream_dir/schema.yaml"
       fi

       # Load sources
       if [ -f "$stream_dir/sources.yaml" ]; then
           load_yaml_to_etcd "/streams/$stream_id/sources" "$stream_dir/sources.yaml"
       fi
   done

   echo ""
   echo "Stream configurations loaded successfully"

   # Verify
   echo ""
   echo "Registered streams:"
   docker exec "$ETCD_CONTAINER" etcdctl get /streams/ --prefix --keys-only | \
       grep config | sed 's|/streams/||' | sed 's|/config||' | sort -u

   echo ""
   echo "Stream details:"
   for stream_id in $(docker exec "$ETCD_CONTAINER" etcdctl get /streams/ --prefix --keys-only | grep config | sed 's|/streams/||' | sed 's|/config||'); do
       echo "  $stream_id:"
       echo "    Config: $(docker exec "$ETCD_CONTAINER" etcdctl get "/streams/$stream_id/config" --print-value-only | head -1)"
       echo "    Schema: $(docker exec "$ETCD_CONTAINER" etcdctl get "/streams/$stream_id/schema" --print-value-only | grep -c "name:")] fields"
       echo "    Sources: $(docker exec "$ETCD_CONTAINER" etcdctl get "/streams/$stream_id/sources" --print-value-only | grep -c "id:")] sources"
   done
   ```

3. **Load Streams into etcd** (30 mins)
   ```bash
   # Make script executable
   chmod +x /workspaces/neural-data-platform/scripts/load-stream-configs.sh

   # Run loader
   cd /workspaces/neural-data-platform
   ./scripts/load-stream-configs.sh

   # Verify streams loaded
   docker exec neural_trader_etcd etcdctl get /streams/air-quality/config
   docker exec neural_trader_etcd etcdctl get /streams/air-quality/schema
   docker exec neural_trader_etcd etcdctl get /streams/air-quality/sources
   ```

4. **Enable Stream Registry in air-quality.yaml** (1 hour)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/air-quality/air-quality.yaml

   stream_registry:
     enabled: true  # CHANGED from false
     etcd_endpoints:
       - http://etcd:2379
     namespace: /streams
     watch_enabled: true  # Enable hot-reload
     watch_reconnect_interval_ms: 5000
   ```

5. **Restart air-quality-server and Verify Hot-Reload** (1 day)
   ```bash
   # Restart to enable stream registry
   docker-compose -f docker-compose.prod.yml restart air-quality-server

   # Watch logs for registry connection
   docker logs -f neural_trader_air_quality | grep -i "registry\|stream"
   # Expected: "Stream registry connected", "Watching streams: [air-quality]"

   # Test hot-reload: Update stream config in etcd
   docker exec neural_trader_etcd etcdctl put /streams/air-quality/config \
     "$(cat /workspaces/neural-data-platform/docker/production/configs/streams/air-quality/config.yaml | sed 's/retention_days: 365/retention_days: 730/')"

   # Watch logs for config reload
   docker logs -f neural_trader_air_quality | grep -i "reload\|updated"
   # Expected: "Stream config updated: air-quality", "Retention policy changed: 365d → 730d"

   # Verify retention policy updated in TimescaleDB
   docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
     "SELECT * FROM timescaledb_information.jobs WHERE proc_name='policy_retention';"
   ```

**Deliverables**:
- `docker/production/configs/streams/air-quality/` (config.yaml, schema.yaml, sources.yaml)
- `scripts/load-stream-configs.sh` (stream loader)
- Updated `docker/production/configs/air-quality/air-quality.yaml` (stream_registry.enabled=true)
- Hot-reload verification report

**Validation**:
```bash
# Comprehensive validation
cd /workspaces/neural-data-platform

# Load streams
./scripts/load-stream-configs.sh

# Verify etcd contains stream configs
docker exec neural_trader_etcd etcdctl get /streams/ --prefix --keys-only | wc -l
# Expected: 3 (config, schema, sources for air-quality)

# Restart air-quality-server
cd docker/production
docker-compose -f docker-compose.prod.yml restart air-quality-server

# Verify registry connection
docker logs neural_trader_air_quality 2>&1 | grep -c "Stream registry connected"
# Expected: 1

# Test hot-reload: Disable stream
docker exec neural_trader_etcd etcdctl put /streams/air-quality/config \
  "$(cat /workspaces/neural-data-platform/docker/production/configs/streams/air-quality/config.yaml | sed 's/enabled: true/enabled: false/')"

# Verify ingestion stopped
docker logs -f neural_trader_air_quality | grep "Stream disabled: air-quality"

# Re-enable stream
docker exec neural_trader_etcd etcdctl put /streams/air-quality/config \
  "$(cat /workspaces/neural-data-platform/docker/production/configs/streams/air-quality/config.yaml)"

# Verify ingestion resumed
docker logs -f neural_trader_air_quality | grep "Stream enabled: air-quality"
```

---

#### Phase 6: Grafana Dashboards and Monitoring (Week 5)

**Objective**: Create dashboards for air-quality data using EXISTING Grafana/Prometheus infrastructure

**Tasks**:

1. **Add TimescaleDB Datasource to Grafana** (1 hour)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/grafana/datasources/timescaledb.yaml

   apiVersion: 1

   datasources:
     - name: TimescaleDB (Air Quality)
       type: postgres
       access: proxy
       url: timescaledb:5432
       database: ${POSTGRES_DB}
       user: ${POSTGRES_USER}
       secureJsonData:
         password: ${POSTGRES_PASSWORD}
       jsonData:
         sslmode: disable
         postgresVersion: 1500  # PostgreSQL 15
         timescaledb: true
       editable: false
   ```

2. **Create Air Quality Overview Dashboard** (1 day)
   ```json
   # Location: /workspaces/neural-data-platform/docker/production/configs/grafana/dashboards/air-quality-overview.json

   {
     "dashboard": {
       "title": "Air Quality Overview",
       "tags": ["air-quality", "environmental"],
       "timezone": "browser",
       "panels": [
         {
           "title": "Latest PM2.5 by Location",
           "type": "stat",
           "datasource": "TimescaleDB (Air Quality)",
           "targets": [
             {
               "rawSql": "SELECT location_id, pm25, timestamp FROM get_latest_air_quality_readings() ORDER BY location_id"
             }
           ]
         },
         {
           "title": "CO2 Levels (Last 24h)",
           "type": "timeseries",
           "datasource": "TimescaleDB (Air Quality)",
           "targets": [
             {
               "rawSql": "SELECT bucket as time, location_id, co2_avg FROM mv_air_quality_5min WHERE bucket > NOW() - INTERVAL '24 hours' ORDER BY bucket"
             }
           ]
         },
         {
           "title": "PM2.5 Heatmap",
           "type": "heatmap",
           "datasource": "TimescaleDB (Air Quality)",
           "targets": [
             {
               "rawSql": "SELECT bucket as time, location_id, pm25_avg FROM mv_air_quality_1hr WHERE bucket > NOW() - INTERVAL '7 days' ORDER BY bucket"
             }
           ]
         },
         {
           "title": "Ingestion Rate (Records/sec)",
           "type": "timeseries",
           "datasource": "Prometheus",
           "targets": [
             {
               "expr": "rate(air_quality_ingestion_records_total{stream_id=\"air-quality\"}[5m])"
             }
           ]
         },
         {
           "title": "Storage Write Latency",
           "type": "timeseries",
           "datasource": "Prometheus",
           "targets": [
             {
               "expr": "histogram_quantile(0.95, air_quality_storage_write_duration_seconds{layer=\"silver\"})"
             }
           ]
         }
       ]
     }
   }
   ```

3. **Create Prometheus Alert Rules** (1 day)
   ```yaml
   # Location: /workspaces/neural-data-platform/docker/production/configs/prometheus/alerts/air_quality.yml

   groups:
     - name: air_quality_ingestion
       interval: 30s
       rules:
         - alert: AirQualityIngestionStopped
           expr: rate(air_quality_ingestion_records_total{stream_id="air-quality"}[5m]) == 0
           for: 10m
           labels:
             severity: critical
             component: air-quality-server
           annotations:
             summary: "Air quality data ingestion stopped"
             description: "No air quality data ingested for stream {{ $labels.stream_id }} in the last 10 minutes"

         - alert: AirQualityHighValidationErrorRate
           expr: rate(air_quality_validation_errors_total{stream_id="air-quality"}[5m]) > 0.1
           for: 5m
           labels:
             severity: warning
             component: air-quality-server
           annotations:
             summary: "High air quality validation error rate"
             description: "Validation error rate for {{ $labels.stream_id }} is {{ $value }} errors/sec"

         - alert: AirQualityTimescaleDBWriteFailure
           expr: rate(air_quality_storage_write_errors_total{layer="silver",stream_id="air-quality"}[5m]) > 0
           for: 2m
           labels:
             severity: critical
             component: air-quality-server
           annotations:
             summary: "TimescaleDB write failures detected"
             description: "Silver layer writes failing for {{ $labels.stream_id }}: {{ $value }} errors/sec"

         - alert: AirQualityUnhealthyPM25
           expr: avg_over_time((SELECT pm25_avg FROM mv_air_quality_5min WHERE bucket > NOW() - INTERVAL '15 minutes')[5m]) > 35
           for: 15m
           labels:
             severity: warning
             component: air-quality
             health_impact: moderate
           annotations:
             summary: "Unhealthy PM2.5 levels detected"
             description: "PM2.5 {{ $labels.location_id }} is {{ $value }} µg/m³ (unhealthy threshold: 35 µg/m³)"

         - alert: AirQualityHighCO2
           expr: avg_over_time((SELECT co2_avg FROM mv_air_quality_5min WHERE bucket > NOW() - INTERVAL '15 minutes')[5m]) > 1000
           for: 10m
           labels:
             severity: warning
             component: air-quality
             health_impact: low
           annotations:
             summary: "High CO2 levels detected"
             description: "CO2 {{ $labels.location_id }} is {{ $value }} ppm (recommended max: 1000 ppm)"
   ```

4. **Load Dashboards and Alerts** (1 hour)
   ```bash
   # Restart Grafana to load new datasource
   docker-compose -f docker-compose.prod.yml restart grafana

   # Wait for Grafana to start
   sleep 15

   # Verify datasource
   curl -u admin:${GRAFANA_ADMIN_PASSWORD} http://localhost:3000/api/datasources | \
       jq '.[] | select(.name=="TimescaleDB (Air Quality)")'

   # Reload Prometheus to load new alert rules
   docker-compose -f docker-compose.prod.yml exec prometheus curl -X POST http://localhost:9090/-/reload

   # Verify alert rules loaded
   curl http://localhost:9093/api/v1/rules | \
       jq '.data.groups[] | select(.name=="air_quality_ingestion")'
   ```

5. **Test Dashboards** (1 day)
   ```bash
   # Access Grafana
   open http://localhost:3000
   # Login: admin / ${GRAFANA_ADMIN_PASSWORD}

   # Navigate to Dashboards → Air Quality Overview
   # Verify panels render:
   # - Latest PM2.5 by Location (should show data)
   # - CO2 Levels (Last 24h) (should show time series)
   # - PM2.5 Heatmap (should show heatmap)
   # - Ingestion Rate (should show rate > 0)
   # - Storage Write Latency (should show p95 < 100ms)

   # Test TimescaleDB datasource query
   # Dashboards → Explore → TimescaleDB (Air Quality)
   # Run query:
   SELECT * FROM get_latest_air_quality_readings();
   # Expected: Rows with latest air quality data

   # Test alerts
   # Alerting → Alert rules → air_quality_ingestion
   # Verify rules: AirQualityIngestionStopped, AirQualityHighValidationErrorRate, etc.

   # Trigger test alert (stop ingestion)
   docker-compose -f docker-compose.prod.yml stop air-quality-server

   # Wait 10 minutes
   sleep 600

   # Check alerts firing
   curl http://localhost:9093/api/v1/alerts | jq '.data.alerts[] | select(.labels.alertname=="AirQualityIngestionStopped")'

   # Restore ingestion
   docker-compose -f docker-compose.prod.yml start air-quality-server
   ```

**Deliverables**:
- `docker/production/configs/grafana/datasources/timescaledb.yaml`
- `docker/production/configs/grafana/dashboards/air-quality-overview.json`
- `docker/production/configs/prometheus/alerts/air_quality.yml`
- Dashboard screenshots and alert test report

**Validation**:
```bash
# Comprehensive validation
cd /workspaces/neural-data-platform/docker/production

# Verify Grafana datasource
curl -u admin:${GRAFANA_ADMIN_PASSWORD} http://localhost:3000/api/datasources | \
    jq '.[] | select(.type=="postgres") | {name:.name, database:.database, url:.url}'

# Verify dashboard loaded
curl -u admin:${GRAFANA_ADMIN_PASSWORD} http://localhost:3000/api/search?query=air | \
    jq '.[] | {title:.title, uid:.uid}'

# Verify Prometheus alert rules
curl http://localhost:9093/api/v1/rules | \
    jq '.data.groups[] | select(.name=="air_quality_ingestion") | {name:.name, rules:(.rules | length)}'

# Test dashboard rendering (headless screenshot)
# Requires grafana-image-renderer plugin
curl -u admin:${GRAFANA_ADMIN_PASSWORD} \
    "http://localhost:3000/render/d/air-quality-overview?orgId=1&width=1920&height=1080" \
    -o /tmp/air-quality-dashboard.png

# Verify screenshot created
ls -lh /tmp/air-quality-dashboard.png
```

---

## 2. Infrastructure Changes Summary

### Unified Production Docker Compose (Final State)

**Services** (14 total):

1. **timescaledb** (neural-trader/timescaledb:prod) - EXISTING, extended with air-quality schema
2. **redis** (redis:7-alpine) - EXISTING
3. **mosquitto** (eclipse-mosquitto:2.0) - NEW
4. **etcd** (quay.io/coreos/etcd:v3.5.11) - NEW
5. **neural-trader** (neural-trader:prod) - EXISTING
6. **data-ingestion** (neural-trader/data-ingestion:prod) - EXISTING
7. **air-quality-server** (neural-trader/air-quality-server:prod) - NEW
8. **prometheus** (neural-trader/prometheus:prod) - EXISTING, extended with air-quality scrape
9. **grafana** (neural-trader/grafana:prod) - EXISTING, extended with air-quality datasource/dashboards
10. **postgres-exporter** - EXISTING
11. **redis-exporter** - EXISTING
12. **node-exporter** - EXISTING

**Networks**:
- neural_trader_internal: Backend services (TimescaleDB, Redis, Mosquitto, etcd, apps)
- monitoring: Isolated monitoring stack (Prometheus, Grafana, exporters)

**Volumes**:
- EXISTING: timescaledb_data, redis_data, prometheus_data, grafana_data, neural_trader_models, neural_trader_logs, data_ingestion_logs
- NEW: mosquitto_data, mosquitto_logs, etcd_data, air-quality-data, air-quality-models

**Configuration Structure**:
```
docker/production/configs/
├── mosquitto/
│   └── mosquitto.conf
├── timescaledb/
│   └── migrations/
│       └── 001_air_quality_schema.sql
├── prometheus/
│   ├── prometheus.yml (updated with air-quality scrape)
│   └── alerts/
│       └── air_quality.yml
├── grafana/
│   ├── datasources/
│   │   └── timescaledb.yaml
│   └── dashboards/
│       └── air-quality-overview.json
├── streams/
│   └── air-quality/
│       ├── config.yaml
│       ├── schema.yaml
│       └── sources.yaml
└── air-quality/
    └── air-quality.yaml
```

---

## 3. Deployment Strategy

### 3.1 Pre-Deployment Checklist

**Environment Validation**:
- [ ] Docker version ≥ 20.10
- [ ] docker-compose version ≥ 1.29
- [ ] .env file created from .env.template
- [ ] All API keys populated (15+ keys for neural-trader, optional for air-quality)
- [ ] Secrets directory exists (production only)
- [ ] Build script executable: `chmod +x scripts/build.sh`
- [ ] Deploy script executable: `chmod +x scripts/deploy.sh`
- [ ] Load-stream-configs script executable: `chmod +x scripts/load-stream-configs.sh`

**Baseline Verification** (CRITICAL):
- [ ] Current development stack verified (Phase 1, Task 1)
- [ ] Current production stack verified (Phase 1, Task 2)
- [ ] Baseline snapshot created (Phase 1, Task 6)
- [ ] Git tag created: `air-004-baseline-pre-migration`

**Resource Requirements**:
- [ ] Disk space: 20GB available (TimescaleDB data, Parquet files, Docker images)
- [ ] Memory: 8GB total (2GB TimescaleDB, 4GB neural-trader, 1.5GB air-quality, 1GB monitoring, 512MB overhead)
- [ ] CPU: 4 cores recommended (2 for neural-trader, 1.5 for air-quality, 0.5 for infrastructure)

**Backup Verification**:
- [ ] TimescaleDB backup script tested
- [ ] Parquet file backup location identified
- [ ] etcd snapshot capability verified
- [ ] Rollback procedure documented and understood

---

### 3.2 Deployment Steps (Using EXISTING deploy.sh)

**Step 1: Build Images**
```bash
cd /workspaces/neural-data-platform

# Run build script (creates 6 images)
./scripts/build.sh

# Verify images created
docker images | grep neural-trader
# Expected output:
# neural-trader:prod
# neural-trader/timescaledb:prod
# neural-trader/prometheus:prod
# neural-trader/grafana:prod
# neural-trader/data-ingestion:prod
# neural-trader/air-quality-server:prod
```

**Step 2: Run Deployment Script**
```bash
cd /workspaces/neural-data-platform

# Run deploy script (handles validation, backup, deployment, health checks)
./scripts/deploy.sh production latest

# Expected output:
# [INFO] Starting Neural Trader deployment
# [INFO] Environment: production
# [INFO] Version: latest
# [SUCCESS] Prerequisites check passed
# [SUCCESS] Environment validation passed
# [SUCCESS] Security checks passed
# [INFO] Creating backup before deployment...
# [SUCCESS] Backup created: backups/pre-deploy/backup-20251215-120000.sql
# [INFO] Building Docker images for version latest...
# [SUCCESS] Images built successfully
# [INFO] Deploying services...
# [SUCCESS] Services deployed successfully
# [INFO] Running health checks...
# [SUCCESS] All services are healthy
# [INFO] Running smoke tests...
# [SUCCESS] Smoke tests passed
# [SUCCESS] Deployment completed successfully!
```

**Step 3: Load Stream Configurations**
```bash
cd /workspaces/neural-data-platform

# Load streams into etcd
./scripts/load-stream-configs.sh

# Expected output:
# Loading stream configurations from docker/production/configs/streams to etcd (neural_trader_etcd)
# Loading stream: air-quality
#   Loading /streams/air-quality/config from config.yaml
#   Loading /streams/air-quality/schema from schema.yaml
#   Loading /streams/air-quality/sources from sources.yaml
# Stream configurations loaded successfully
#
# Registered streams:
#   air-quality
```

**Step 4: Apply TimescaleDB Migrations**
```bash
cd /workspaces/neural-data-platform

# Copy migration file
docker cp docker/production/configs/timescaledb/migrations/001_air_quality_schema.sql \
    neural_trader_timescaledb:/tmp/

# Apply migration
docker exec neural_trader_timescaledb \
    psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -f /tmp/001_air_quality_schema.sql

# Verify tables created
docker exec neural_trader_timescaledb \
    psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c "\dt air_quality*"
```

**Step 5: Verify Deployment**
```bash
cd /workspaces/neural-data-platform/docker/production

# Check all services healthy
docker-compose -f docker-compose.prod.yml ps

# Expected: 14 services, all "Up (healthy)"

# Test air-quality ingestion
docker exec neural_trader_mosquitto mosquitto_pub \
    -t "airgradient/test/measures" \
    -m '{"pm25":12.3,"co2":650,"temperature":22.1,"humidity":45.2,"location_id":"office","sensor_id":"test-001"}'

# Wait for batch timeout
sleep 20

# Verify data in TimescaleDB
docker exec neural_trader_timescaledb \
    psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
    "SELECT COUNT(*) FROM air_quality_measurements WHERE timestamp > NOW() - INTERVAL '5 minutes';"

# Verify data in Parquet
docker exec neural_trader_air_quality find /app/data -name "*.parquet" -mmin -5

# Check Grafana dashboard
open http://localhost:3000
# Navigate to Dashboards → Air Quality Overview
# Verify panels render with data

# Check Prometheus alerts
curl http://localhost:9093/api/v1/rules | jq '.data.groups[] | select(.name=="air_quality_ingestion")'
```

---

## 4. Operational Runbook

### 4.1 Daily Health Check

```bash
#!/bin/bash
# Location: /workspaces/neural-data-platform/scripts/health-check.sh

echo "=== Air Quality Platform Health Check ==="
echo ""

# 1. Docker services
echo "1. Checking Docker services..."
cd /workspaces/neural-data-platform/docker/production
UNHEALTHY=$(docker-compose -f docker-compose.prod.yml ps | grep -v "Up (healthy)" | grep -c "Up")
if [ "$UNHEALTHY" -gt 0 ]; then
    echo "  ✗ $UNHEALTHY services unhealthy"
    docker-compose -f docker-compose.prod.yml ps
else
    echo "  ✓ All 14 services healthy"
fi

# 2. TimescaleDB
echo "2. Checking TimescaleDB..."
docker exec neural_trader_timescaledb pg_isready -U ${POSTGRES_USER} >/dev/null 2>&1
if [ $? -eq 0 ]; then
    RECORDS=$(docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -t -c "SELECT COUNT(*) FROM air_quality_measurements;")
    echo "  ✓ TimescaleDB healthy ($RECORDS total records)"
else
    echo "  ✗ TimescaleDB unhealthy"
fi

# 3. etcd
echo "3. Checking etcd..."
docker exec neural_trader_etcd etcdctl endpoint health >/dev/null 2>&1
if [ $? -eq 0 ]; then
    STREAMS=$(docker exec neural_trader_etcd etcdctl get /streams/ --prefix --keys-only | grep -c config)
    echo "  ✓ etcd healthy ($STREAMS registered streams)"
else
    echo "  ✗ etcd unhealthy"
fi

# 4. Air Quality Ingestion
echo "4. Checking air quality ingestion..."
RECENT_RECORDS=$(docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -t -c \
    "SELECT COUNT(*) FROM air_quality_measurements WHERE timestamp > NOW() - INTERVAL '5 minutes';")
if [ "$RECENT_RECORDS" -gt 0 ]; then
    echo "  ✓ Ingestion active ($RECENT_RECORDS records in last 5 minutes)"
else
    echo "  ✗ No recent ingestion"
fi

# 5. Storage Health
echo "5. Checking storage..."
BRONZE_SIZE=$(docker exec neural_trader_air_quality du -sh /app/data 2>/dev/null | awk '{print $1}')
SILVER_SIZE=$(docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -t -c \
    "SELECT pg_size_pretty(pg_total_relation_size('air_quality_measurements'));")
echo "  ✓ Bronze layer: $BRONZE_SIZE"
echo "  ✓ Silver layer: $SILVER_SIZE"

# 6. Monitoring
echo "6. Checking monitoring..."
PROM_TARGETS=$(curl -s http://localhost:9093/api/v1/targets | jq -r '.data.activeTargets | length')
echo "  ✓ Prometheus scraping $PROM_TARGETS targets"

GRAFANA_STATUS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/health)
if [ "$GRAFANA_STATUS" == "200" ]; then
    echo "  ✓ Grafana healthy"
else
    echo "  ✗ Grafana unhealthy (HTTP $GRAFANA_STATUS)"
fi

echo ""
echo "=== Health Check Complete ==="
```

### 4.2 Common Maintenance Tasks

**Weekly Tasks**:
```bash
# Vacuum and analyze TimescaleDB
docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
    "VACUUM ANALYZE air_quality_measurements;"

# Check compression status
docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
    "SELECT * FROM timescaledb_information.compressed_chunk_stats WHERE hypertable_name='air_quality_measurements';"

# Prune Docker images
docker image prune -f
```

**Monthly Tasks**:
```bash
# Backup TimescaleDB
docker exec neural_trader_timescaledb pg_dump -U ${POSTGRES_USER} -d ${POSTGRES_DB} > \
    /workspaces/neural-data-platform/backups/monthly/air-quality-$(date +%Y%m).sql

# Backup etcd
docker exec neural_trader_etcd etcdctl snapshot save /tmp/etcd-backup-$(date +%Y%m).db
docker cp neural_trader_etcd:/tmp/etcd-backup-$(date +%Y%m).db \
    /workspaces/neural-data-platform/backups/monthly/

# Archive old Parquet files (>90 days)
docker exec neural_trader_air_quality find /app/data -name "*.parquet" -mtime +90 -exec gzip {} \;
```

---

## 5. Rollback Procedures

### 5.1 Complete Rollback to Baseline

**Scenario**: Multi-stream migration failed, restore to pre-migration state

```bash
#!/bin/bash
# Location: /workspaces/neural-data-platform/scripts/rollback-to-baseline.sh

set -e

echo "=== ROLLBACK TO BASELINE (Pre-AIR-004 Migration) ==="
echo "This will restore the system to the state before multi-stream migration."
read -p "Are you sure? (yes/no): " CONFIRM

if [ "$CONFIRM" != "yes" ]; then
    echo "Rollback cancelled."
    exit 1
fi

cd /workspaces/neural-data-platform

# 1. Stop all services
echo "1. Stopping all services..."
cd docker/production
docker-compose -f docker-compose.prod.yml down

# 2. Restore docker-compose.prod.yml to baseline
echo "2. Restoring docker-compose.prod.yml..."
cp ../../product/features/air-004/baseline-snapshot/docker-compose.prod.neural-trader.yml docker-compose.prod.yml

# 3. Remove air-quality volumes (CAUTION: Data loss)
echo "3. Removing air-quality volumes..."
docker volume rm -f neural-trader_air-quality-data neural-trader_air-quality-models
docker volume rm -f neural-trader_mosquitto_data neural-trader_mosquitto_logs
docker volume rm -f neural-trader_etcd_data

# 4. Restore TimescaleDB schema (remove air-quality tables)
echo "4. Restoring TimescaleDB schema..."
docker-compose -f docker-compose.prod.yml up -d timescaledb
sleep 10
docker exec neural_trader_timescaledb psql -U ${POSTGRES_USER} -d ${POSTGRES_DB} -c \
    "DROP TABLE IF EXISTS air_quality_measurements CASCADE;"

# 5. Restart original stack
echo "5. Restarting original stack..."
docker-compose -f docker-compose.prod.yml up -d

# 6. Verify health
echo "6. Verifying health..."
sleep 30
docker-compose -f docker-compose.prod.yml ps

echo ""
echo "=== Rollback Complete ==="
echo "Original neural-trader stack restored (11 services)."
echo "Air-quality-server and related services removed."
echo ""
echo "To restore air-quality development environment, use:"
echo "  cd /workspaces/neural-data-platform"
echo "  docker-compose up -d"
```

### 5.2 Partial Rollback (Disable Specific Features)

**Disable Dual-Write (Keep Bronze/Parquet Only)**:
```bash
# Stop air-quality-server
docker-compose -f docker-compose.prod.yml stop air-quality-server

# Update config to disable silver layer
docker exec neural_trader_air_quality sed -i 's/enabled: true/enabled: false/' /app/config/air-quality.yaml

# Restart
docker-compose -f docker-compose.prod.yml start air-quality-server

# Verify Bronze-only mode
docker logs neural_trader_air_quality | grep "Silver layer"
# Expected: "Silver layer (TimescaleDB) disabled"
```

**Disable Stream Registry (Use File Config Only)**:
```bash
# Update config to disable stream registry
docker exec neural_trader_air_quality sed -i 's/enabled: true/enabled: false/' /app/config/air-quality.yaml

# Restart
docker-compose -f docker-compose.prod.yml restart air-quality-server

# Verify file-based config
docker logs neural_trader_air_quality | grep "Stream registry"
# Expected: "Stream registry disabled, using file config"
```

---

## 6. Future Extensions

### 6.1 Adding New Streams (Weather, Home Events)

**Example: Weather Stream**

1. Create stream configs:
```yaml
# docker/production/configs/streams/weather/config.yaml
stream_id: weather
description: Outdoor weather conditions from OpenWeatherMap
enabled: true
retention_days: 365
```

```yaml
# docker/production/configs/streams/weather/schema.yaml
fields:
  - name: temperature
    type: float
    unit: celsius
    nullable: false
  - name: humidity
    type: float
    unit: percent
    nullable: false
  - name: pressure
    type: float
    unit: hPa
    nullable: true
```

```yaml
# docker/production/configs/streams/weather/sources.yaml
sources:
  - id: openweathermap-api
    type: http_poll
    enabled: true
    config:
      url: https://api.openweathermap.org/data/2.5/weather?q={LOCATION}&appid={API_KEY}
      interval_seconds: 300  # 5 minutes
      auth:
        type: query_param
        key: appid
```

2. Load into etcd:
```bash
./scripts/load-stream-configs.sh
```

3. Air-quality-server automatically:
- Detects new stream via etcd watch
- Creates TimescaleDB table from schema
- Spawns HTTP polling source
- Starts ingestion

**No code changes required** (once generic ingestion coordinator implemented in Phase 6 of original plan).

---

## Appendix A: File Locations Reference

### Configuration Files
- Air Quality Production Config: `/workspaces/neural-data-platform/docker/production/configs/air-quality/air-quality.yaml`
- Stream Configs: `/workspaces/neural-data-platform/docker/production/configs/streams/{stream-id}/`
- Mosquitto Config: `/workspaces/neural-data-platform/docker/production/configs/mosquitto/mosquitto.conf`
- Prometheus Config: `/workspaces/neural-data-platform/docker/production/configs/prometheus/prometheus.yml`
- Grafana Datasources: `/workspaces/neural-data-platform/docker/production/configs/grafana/datasources/`
- Grafana Dashboards: `/workspaces/neural-data-platform/docker/production/configs/grafana/dashboards/`

### Docker Compose Files
- Development: `/workspaces/neural-data-platform/docker-compose.yml`
- Production (Pi5): `/workspaces/neural-data-platform/docker-compose.prod.yml`
- Production (Neural-Trader): `/workspaces/neural-data-platform/docker/production/docker-compose.prod.yml`

### Scripts
- Build: `/workspaces/neural-data-platform/scripts/build.sh`
- Deploy: `/workspaces/neural-data-platform/scripts/deploy.sh`
- Load Streams: `/workspaces/neural-data-platform/scripts/load-stream-configs.sh`
- Health Check: `/workspaces/neural-data-platform/scripts/health-check.sh`
- Rollback: `/workspaces/neural-data-platform/scripts/rollback-to-baseline.sh`

### Data Volumes
- TimescaleDB: `neural_trader_timescaledb:/var/lib/postgresql/data` → `timescaledb_data` volume
- Parquet (Bronze): `neural_trader_air_quality:/app/data` → `air-quality-data` volume
- Models: `neural_trader_air_quality:/app/models` → `air-quality-models` volume
- etcd: `neural_trader_etcd:/etcd-data` → `etcd_data` volume
- Mosquitto: `neural_trader_mosquitto:/mosquitto/data` → `mosquitto_data` volume

### Dockerfile Locations
- Air Quality Server: `/workspaces/neural-data-platform/apps/air-quality-app/Dockerfile.prod`
- Neural Trader: `/workspaces/neural-data-platform/docker/production/neural-trader/Dockerfile`
- TimescaleDB: `/workspaces/neural-data-platform/docker/production/timescaledb/Dockerfile`
- Prometheus: `/workspaces/neural-data-platform/docker/production/prometheus/Dockerfile`
- Grafana: `/workspaces/neural-data-platform/docker/production/grafana/Dockerfile`
- Data Ingestion: `/workspaces/neural-data-platform/docker/production/data-ingestion/Dockerfile`

---

## Appendix B: Port Mapping Reference

### Development Environment
- Mosquitto MQTT: 1883
- Mosquitto WebSocket: 9001
- etcd Client: 2379
- etcd Peer: 2380
- Air Quality API: 8080
- Air Quality Metrics: 9090
- Prometheus: 9091 (mapped from container 9090)
- Grafana: 3000

### Production Environment (Neural-Trader Stack)
- TimescaleDB: 127.0.0.1:5433:5432 (localhost only)
- Mosquitto MQTT: 127.0.0.1:1883:1883 (localhost only)
- Mosquitto WebSocket: 127.0.0.1:9001:9001 (localhost only)
- etcd Client: 127.0.0.1:2379:2379 (localhost only)
- Neural-Trader API: 127.0.0.1:8080:8080 (localhost only)
- Neural-Trader Metrics: 127.0.0.1:9092:9092 (localhost only)
- Data Ingestion API: 127.0.0.1:8002:8001 (localhost only)
- Air Quality API: 127.0.0.1:8081:8080 (localhost only, avoids conflict)
- Air Quality Metrics: 127.0.0.1:9091:9090 (localhost only, avoids conflict)
- Prometheus: 127.0.0.1:9093:9090 (localhost only, avoids conflict)
- Grafana: 127.0.0.1:3000:3000 (localhost only)

**Note**: All production ports bound to 127.0.0.1 (localhost only) for security.

---

## Appendix C: Environment Variables Reference

### Required (.env)
```bash
# PostgreSQL/TimescaleDB
POSTGRES_USER=postgres
POSTGRES_PASSWORD=<secure-password>
POSTGRES_DB=neural_trader_db

# Grafana
GRAFANA_ADMIN_PASSWORD=<secure-password>

# Air Quality (Optional)
AIR_QUALITY_LOG_LEVEL=info
ENABLE_MULTI_STREAM=false
ENABLE_DUAL_WRITE=false

# Neural Trader (Existing)
LOG_LEVEL=info
NEURAL_USE_REAL_MODELS=false
ENABLE_SECTOR_MODELS=true
ENABLE_REALTIME_ADAPTATION=true
# ... (15+ additional API keys)
```

### Optional
```bash
# MQTT
MQTT_BROKER_URL=mqtt://mosquitto:1883

# etcd
ETCD_ENDPOINTS=http://etcd:2379

# Deployment
BACKUP_BEFORE_DEPLOY=true
HEALTH_CHECK_TIMEOUT=300
ROLLBACK_ON_FAILURE=true
WEBHOOK_URL=<notification-webhook>
ALERT_EMAIL=<alert-email>
```

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-15 | Claude Code | Initial completion plan (theoretical) |
| 2.0.0 | 2025-12-15 | Claude Code | **REVISED to reflect ACTUAL infrastructure**: Updated deployment topology, added current operational baseline, revised phases to use existing TimescaleDB/Prometheus/Grafana, added rollback procedures, updated file paths, corrected docker-compose references, aligned with real deploy.sh workflow |

---

**END OF REVISED COMPLETION DOCUMENT**
