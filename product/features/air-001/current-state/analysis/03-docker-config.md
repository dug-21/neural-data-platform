# Docker and Deployment Configuration Analysis

**Analysis Date:** December 14, 2025
**Scope:** All Dockerfiles, docker-compose files, CI/CD workflows

---

## 1. Current Docker Configuration Status

### Containerized Services

| Environment | Services | Status |
|-------------|----------|--------|
| Development | MQTT, Air Quality App, Prometheus, Grafana | WORKING |
| Production (Pi5) | MQTT, Air Quality App (pre-built) | CONFIG EXISTS |
| Comprehensive (v2) | Redis, TimescaleDB, all services | LEGACY |
| Testing | PostgreSQL, Redis, mocks | LEGACY |

---

## 2. Main Air Quality Dockerfile

**File:** `/workspaces/neural-data-platform/Dockerfile`

### Multi-Stage Build Architecture

```dockerfile
# Stage 1: Chef (dependency caching)
FROM rust:1.75-slim-bookworm as chef

# Stage 2: Planner (extract recipe)
FROM chef as planner

# Stage 3: Builder (compile)
FROM chef as builder

# Stage 4: Runtime (minimal image)
FROM debian:bookworm-slim
```

### Key Features
- **Target size:** <100MB compressed
- **Architectures:** linux/amd64, linux/arm64 (Pi5)
- **User:** Non-root (appuser:1000)
- **Health check:** `curl -f http://localhost:8080/health`
- **Runtime:** Debian bookworm-slim

### Security Features
- Binary stripping post-compilation
- Minimal runtime dependencies
- Read-only filesystem (except /tmp)
- No-new-privileges security option

---

## 3. Docker Compose Files

### Development (`docker-compose.yml`)

```yaml
services:
  mosquitto:        # Eclipse Mosquitto 2.0
  air-quality-app:  # Rust API (builds from Dockerfile)
  prometheus:       # Optional (profiles: monitoring)
  grafana:          # Optional (profiles: monitoring)

networks:
  neural-network: bridge

volumes:
  - mosquitto-data
  - mosquitto-logs
  - air-quality-data
  - air-quality-models
```

### Production Pi5 (`docker-compose.prod.yml`)

```yaml
services:
  mosquitto:
    deploy:
      resources:
        limits: { cpus: '0.5', memory: 256M }

  air-quality-app:
    image: ghcr.io/neural-data-platform/air-quality:latest  # PRE-BUILT
    deploy:
      resources:
        limits: { cpus: '2.0', memory: 1792M }
        reservations: { cpus: '1.0', memory: 1024M }
    environment:
      - RAYON_NUM_THREADS=2
      - TOKIO_WORKER_THREADS=2
    logging:
      driver: json-file
      options: { max-size: "10m", max-file: "3" }

networks:
  pi5-neural-network: bridge

volumes:
  # Host paths for Pi5
  - /opt/neural/data/mosquitto
  - /opt/neural/data/air-quality
  - /opt/neural/models
```

---

## 4. Multi-Architecture Support

### Current Status

| Feature | Status | Notes |
|---------|--------|-------|
| Dockerfile multi-arch | PARTIAL | Targets defined, no buildx |
| CI binary builds | YES | x86_64 + aarch64 in workflow |
| Docker image builds | NO | Only Rust binaries, not OCI images |
| Manifest lists | NO | No multi-arch image manifest |
| Platform specs in compose | NO | Missing `platform:` field |

### CI/CD Pipeline (`air-001-ci.yml`)

```yaml
# Lines 180-226: Multi-Architecture Build
- Targets: x86_64, aarch64
- Cross-compilation tools installed
- Artifacts uploaded to GitHub Actions
- NO Docker image building
- NO registry push
```

### What's Missing

1. **No buildx configuration** in docker-compose files
2. **No platform specification** in services
3. **No manifest list generation** for multi-arch images
4. **No automated Docker image building** for arm64
5. **Production image references GHCR** but no buildx workflow exists

---

## 5. Volume Mounts for Data Persistence

### Development

| Volume | Purpose | Mount |
|--------|---------|-------|
| mosquitto-data | MQTT persistence | /mosquitto/data |
| mosquitto-logs | MQTT logs | /mosquitto/log |
| air-quality-data | Parquet storage | /data |
| air-quality-models | ML models | /models |
| prometheus-data | Metrics | /prometheus |
| grafana-data | Dashboards | /var/lib/grafana |

### Production (Pi5 Host Paths)

```
/opt/neural/data/mosquitto
/opt/neural/logs/mosquitto
/opt/neural/data/air-quality
/opt/neural/models
```

---

## 6. Health Check Implementations

### Air Quality App

**Dockerfile:**
```yaml
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3
  CMD curl -f http://localhost:8080/health || exit 1
```

**Response Format:**
```json
{
  "status": "healthy|degraded|unhealthy",
  "mqtt": "connected|disconnected",
  "storage": "ok|error",
  "last_reading_age_seconds": 120
}
```

### Service Health Checks

| Service | Check | Interval | Timeout |
|---------|-------|----------|---------|
| Mosquitto (dev) | `mosquitto_sub -t $$SYS/#` | 30s | 10s |
| Mosquitto (prod) | Same | 60s | 10s |
| Air Quality | `curl /health` | 30s-60s | 10s |
| Prometheus | `wget --spider /-/healthy` | 10s | 5s |
| Grafana | `curl /api/health` | 10s | 5s |

---

## 7. MQTT Network Configuration

### Mosquitto Configuration (`mosquitto/config/mosquitto.conf`)

```
listener 1883
max_connections -1
max_queued_messages 1000
max_inflight_messages 20
persistence true
persistence_location /mosquitto/data/
max_qos 2
retain_available true
max_retained_messages 1000

# Security (currently disabled for development)
allow_anonymous true
# TLS on port 8883 (commented out)
```

### App MQTT Configuration (`config/base/air-quality.yaml`)

```yaml
mqtt:
  broker_url: mqtt://localhost:1883
  client_id: air-quality-app
  keep_alive_seconds: 60
  clean_session: false
  qos: 1
  topics:
    airgradient: airgradient/+/measures
    predictions: neural/predictions
  reconnect:
    min_delay_ms: 1000
    max_delay_ms: 60000
    max_retries: 10
```

---

## 8. What's Missing for E2E Testing

### Critical Gaps

1. **No E2E Test Docker Compose**
   - `docker-compose.test.yml` is for legacy neural-trader
   - No air quality test harness

2. **No Multi-Arch Docker Build Pipeline**
   - CI builds binaries, not images
   - No GHCR push workflow
   - Production image doesn't exist

3. **No Test Data Publisher**
   - No mock AirGradient sensor container
   - No MQTT test message generator

4. **No Test Observer**
   - No container to validate data flow
   - No integration test runner in Docker

5. **Missing Observability**
   - No AlertManager configured
   - No log aggregation (Loki/ELK)
   - No distributed tracing
   - No air quality Grafana dashboards

6. **Security Not Production-Ready**
   - MQTT anonymous access enabled
   - No TLS/SSL configuration
   - No secrets management

---

## 9. Required E2E Docker Architecture

### Proposed `docker-compose.e2e.yml`

```yaml
version: "3.8"

services:
  # MQTT Broker
  mosquitto:
    image: eclipse-mosquitto:2.0
    healthcheck:
      test: ["CMD", "mosquitto_sub", "-t", "$$SYS/#", "-C", "1"]
    volumes:
      - ./mosquitto/config:/mosquitto/config:ro

  # Air Quality Application
  air-quality-app:
    build:
      context: .
      dockerfile: Dockerfile
    depends_on:
      mosquitto:
        condition: service_healthy
    environment:
      - MQTT_BROKER_URL=mqtt://mosquitto:1883
      - DATA_DIR=/data
    volumes:
      - e2e-data:/data
      - e2e-models:/models
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]

  # Mock AirGradient Sensor
  sensor-simulator:
    build:
      context: ./tests/e2e/sensor-simulator
    depends_on:
      mosquitto:
        condition: service_healthy
    environment:
      - MQTT_BROKER=mosquitto:1883
      - SENSOR_SERIAL=ecda3b1eaaaf
      - PUBLISH_INTERVAL=5s

  # Test Observer
  test-observer:
    build:
      context: ./tests/e2e/test-observer
    depends_on:
      air-quality-app:
        condition: service_healthy

  # E2E Test Runner
  test-runner:
    build:
      context: ./tests/e2e/test-runner
    depends_on:
      - test-observer
    command: ["cargo", "test", "--test", "e2e_tests"]

  # Monitoring (optional)
  prometheus:
    image: prom/prometheus:latest
    profiles: ["monitoring"]
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml:ro

volumes:
  e2e-data:
  e2e-models:

networks:
  default:
    name: air-quality-e2e
```

---

## 10. Priority Fixes for E2E

### Week 1: Docker Infrastructure

1. **Create `docker-compose.e2e.yml`** with test harness
2. **Add buildx workflow** to CI for multi-arch images
3. **Push to GHCR** on tag/release

### Week 2: Test Containers

4. **Create sensor-simulator** container (mock AirGradient)
5. **Create test-observer** container (validate data flow)
6. **Create test-runner** container (execute E2E tests)

### Week 3: Observability

7. **Add AlertManager** with air quality rules
8. **Provision Grafana dashboards** for PM2.5, CO2, AQI
9. **Configure log aggregation** with Loki

---

## 11. Key Files Reference

| File | Purpose | Status |
|------|---------|--------|
| `/Dockerfile` | Main app image | Complete |
| `/docker-compose.yml` | Development | Complete |
| `/docker-compose.prod.yml` | Production (Pi5) | Image refs missing |
| `/docker-compose.test.yml` | Testing | Legacy, needs replacement |
| `/config/base/air-quality.yaml` | Base config | Complete |
| `/config/overlays/production/overrides.yaml` | Prod config | Complete |
| `/mosquitto/config/mosquitto.conf` | MQTT broker | Security disabled |
| `/.github/workflows/air-001-ci.yml` | CI/CD | Needs Docker push |
