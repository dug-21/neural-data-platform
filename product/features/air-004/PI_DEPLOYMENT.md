# AIR-004: Pi Production Deployment Reference

## Overview

This document is the **source of truth** for the Raspberry Pi 5 production deployment. The actual deployment configuration is in `deploy/pi/`, NOT `docker/production/`.

---

## Deployment Location

```
deploy/pi/
├── deploy.sh                      # Deployment orchestration script
├── docker-compose.yml             # Service definitions
├── README.md                      # Usage documentation
└── mosquitto/
    └── mosquitto.conf             # MQTT broker configuration
```

---

## Services (3 Total)

| Service | Container Name | Image | Ports | Memory |
|---------|---------------|-------|-------|--------|
| MQTT Broker | mqtt-broker | eclipse-mosquitto:2.0 | 1883, 9001 | 128MB |
| Config Store | etcd | quay.io/coreos/etcd:v3.5.11 | 2379 | 256MB |
| Air Quality App | air-quality-app | neural-data-platform/air-quality-app:latest | 8080, 9090 | 512MB |

**Total Memory**: ~896MB allocated (of 16GB available on Pi 5)

---

## Docker Volumes

| Volume | Mount Path | Purpose |
|--------|-----------|---------|
| pi_air-quality-data | /app/data | **PRIMARY** - Parquet files |
| pi_etcd-data | /etcd-data | Configuration persistence |
| pi_mosquitto-data | /mosquitto/data | MQTT message persistence |
| pi_mosquitto-logs | /mosquitto/log | MQTT logs |

---

## Deployment Commands

```bash
cd /workspaces/neural-data-platform/deploy/pi

./deploy.sh              # Full deploy: build + start + sync
./deploy.sh start        # Start services (no rebuild)
./deploy.sh stop         # Stop all services
./deploy.sh logs         # View live logs
./deploy.sh status       # Health check and diagnostics
./deploy.sh update       # Pull latest, rebuild, redeploy
./deploy.sh build        # Build Docker images only
./deploy.sh sync         # Re-sync config to etcd
```

---

## Network Configuration

- **Network Name**: `neural-network` (bridge driver)
- **Service Discovery**: Docker DNS (mosquitto, etcd resolve to containers)

### Port Mappings

| Port | Service | Purpose |
|------|---------|---------|
| 1883 | mosquitto | MQTT protocol |
| 9001 | mosquitto | WebSocket |
| 2379 | etcd | Client API |
| 8080 | air-quality-app | REST API |
| 9090 | air-quality-app | Prometheus metrics |

---

## Configuration Sync

Configuration is loaded from Git → etcd:

```bash
# Executed automatically by deploy.sh start
ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production
```

**Sources**:
- `config/base/air-quality/config.yaml` - Base settings
- `config/overlays/production/air-quality/config.yaml` - Production overrides

**etcd Keys**:
```
/air-quality/server/host
/air-quality/server/port
/air-quality/mqtt/broker_url
/air-quality/mqtt/topic_pattern
/air-quality/storage/base_path
...
```

---

## Data Persistence

### Primary Data Path
```
/app/data/
├── air-quality/
│   └── YYYY/MM/DD/readings.parquet
└── wal/
    └── *.wal (Write-Ahead Log)
```

### Host Path (for manual access)
```
/var/lib/docker/volumes/pi_air-quality-data/_data/
```

### Backup Command
```bash
docker run --rm -v pi_air-quality-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/air-quality-backup-$(date +%Y%m%d).tar.gz /data
```

---

## Health Checks

### Verify All Services
```bash
./deploy.sh status
```

### Manual Checks
```bash
# MQTT
docker exec mqtt-broker mosquitto_sub -t 'airgradient/#' -C 1 -W 3

# etcd
docker exec etcd etcdctl endpoint health

# Air Quality App
curl http://localhost:8080/health
```

---

## Build Times

| Scenario | Time |
|----------|------|
| First build (cold) | 15-30 minutes |
| Subsequent builds (cached) | 5-10 minutes |
| Start services | <30 seconds |

---

## Environment Variables

Set in docker-compose.yml for air-quality-app:

```yaml
RUST_LOG: "info"
STORAGE_PATH: "/app/data"
ETCD_ENDPOINT: "http://etcd:2379"
MQTT_BROKER_URL: "mosquitto"
MQTT_PORT: "1883"
```

---

## CRITICAL: What MUST NOT Change

1. **Volume mounts** - Data loss if wrong
2. **Service names** - DNS resolution fails
3. **Port assignments** - App config depends on these
4. **Config sync** - App won't load settings without etcd
5. **deploy.sh workflow** - Operational procedures depend on it

---

## AIR-004 Impact

When implementing AIR-004 multi-stream support:

1. **Preserve**: Existing `deploy/pi/` structure
2. **Extend**: Add new services alongside existing
3. **Test**: Validate on Pi 5 before merge
4. **Rollback**: Must be possible via `./deploy.sh`

---

*Document Generated: 2025-12-15*
*Source of Truth: `/workspaces/neural-data-platform/deploy/pi/`*
