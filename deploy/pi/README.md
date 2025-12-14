# Raspberry Pi 5 Deployment

Deploy the Neural Data Platform air quality monitoring stack on your Pi 5.

## Prerequisites

- Raspberry Pi 5 (16GB RAM recommended)
- Ubuntu 25.04 (or similar ARM64 Linux)
- Docker and Docker Compose installed
- Git installed

## Quick Start

```bash
# 1. Clone the repository
git clone https://github.com/dug-21/neural-data-platform.git
cd neural-data-platform/deploy/pi

# 2. Deploy (builds and starts everything)
chmod +x deploy.sh
./deploy.sh
```

First build takes **15-30 minutes** (Rust compilation). Subsequent builds use Docker cache.

## Commands

```bash
./deploy.sh          # Full deploy (build + start)
./deploy.sh start    # Start services (no rebuild)
./deploy.sh stop     # Stop all services
./deploy.sh logs     # View live logs
./deploy.sh status   # Check service health
./deploy.sh update   # Pull latest code and redeploy
./deploy.sh sync     # Re-sync config to etcd
```

## Services

| Service | Port | Description |
|---------|------|-------------|
| MQTT Broker | 1883 | Receives AirGradient sensor data |
| Air Quality App | 8080 | HTTP API, data ingestion |
| Metrics | 9090 | Prometheus metrics |
| etcd | 2379 | Configuration store |

## AirGradient Sensor Setup

Configure your AirGradient sensor to send data to your Pi:

1. Connect to sensor's WiFi AP
2. Configure MQTT:
   - **Server**: `<pi-ip-address>`
   - **Port**: `1883`
   - **Topic**: `airgradient/readings/{device_id}`

The sensor will publish readings every ~10 seconds.

## Verify Data Flow

```bash
# Watch incoming MQTT messages
docker exec mqtt-broker mosquitto_sub -t 'airgradient/#' -v

# Check stored Parquet files
docker exec air-quality-app ls -la /data/

# Query the API
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/readings/latest
```

## Data Storage

Parquet files are stored in the `air-quality-data` Docker volume:
- Location: `/var/lib/docker/volumes/pi_air-quality-data/_data/`
- Format: `air-quality/YYYY/MM/DD/readings.parquet`

To backup:
```bash
docker run --rm -v pi_air-quality-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/air-quality-backup-$(date +%Y%m%d).tar.gz /data
```

## Configuration

Configuration is stored in etcd and loaded from `config/` on startup.

To modify config:
1. Edit files in `config/overlays/production/air-quality/`
2. Re-sync: `./deploy.sh sync`

Environment-specific overrides via env vars:
```bash
export AIR_QUALITY_MQTT_BROKER_URL=custom-broker
./deploy.sh start
```

## Resource Usage

Expected resource consumption on Pi 5:

| Service | Memory | CPU |
|---------|--------|-----|
| Mosquitto | ~50MB | <1% |
| etcd | ~100MB | <1% |
| Air Quality App | ~200MB | <5% |
| **Total** | **~350MB** | **<7%** |

Your 16GB Pi has plenty of headroom.

## Troubleshooting

**Build fails with memory error:**
```bash
# Limit parallel jobs
CARGO_BUILD_JOBS=2 docker compose build
```

**MQTT not receiving data:**
```bash
# Test MQTT connectivity
mosquitto_pub -h localhost -t test -m "hello"
docker exec mqtt-broker mosquitto_sub -t '#' -v
```

**etcd not starting:**
```bash
# Check logs
docker compose logs etcd
# Reset etcd data if corrupted
docker volume rm pi_etcd-data
./deploy.sh start
```

**App can't connect to MQTT:**
```bash
# Verify network
docker exec air-quality-app ping mosquitto
# Check environment
docker exec air-quality-app env | grep MQTT
```

## Logs

```bash
# All services
./deploy.sh logs

# Specific service
docker compose logs -f air-quality-app
docker compose logs -f mosquitto
docker compose logs -f etcd
```

## Updating

```bash
# Pull latest and redeploy
./deploy.sh update

# Or manually
git pull
./deploy.sh build
./deploy.sh start
```
