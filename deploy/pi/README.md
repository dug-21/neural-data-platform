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

### Core Commands

```bash
./deploy.sh              # Full deploy (build + start)
./deploy.sh start        # Start services (no rebuild)
./deploy.sh stop         # Stop all services
./deploy.sh logs         # View live logs
./deploy.sh status       # Check service health
./deploy.sh build        # Build Docker images only
```

### Update Commands

```bash
./deploy.sh update              # Pull latest code and redeploy
./deploy.sh update --no-cache   # Force clean rebuild
./deploy.sh refresh             # Pull configs only (no rebuild)
```

### Configuration Commands

```bash
./deploy.sh sync            # Sync all configs to etcd
./deploy.sh list-streams    # List configured streams
./deploy.sh sync-dictionary # Sync data dictionary to TimescaleDB
./deploy.sh sync-dimensions # Sync dimension tables
```

### Declarative Deploy (dp-020)

```bash
./deploy.sh apply                    # Apply .deploy/manifest.json
./deploy.sh apply path/to/manifest   # Apply specific manifest
```

See [Declarative Deploy](#declarative-deploy-dp-020) section below.

### Silver ETL Commands

```bash
./deploy.sh silver-migrate        # Run TimescaleDB migrations
./deploy.sh silver-etl            # Run Bronze → Silver ETL once
./deploy.sh silver-daemon         # Start continuous ETL daemon
./deploy.sh silver-daemon-stop    # Stop ETL daemon
./deploy.sh silver-daemon-logs    # View daemon logs
./deploy.sh silver-daemon-status  # Check daemon status
```

---

## Declarative Deploy (dp-020)

The declarative deploy system allows you to define **what changed** in a manifest file, and `deploy.sh apply` orchestrates the deployment automatically.

### Release Workflow

```bash
# 1. Create/edit your stream configuration
vim config/base/streams/my-sensor/config.json

# 2. Create release manifest
cat > .deploy/releases/v1.2.0.manifest.json << 'EOF'
{
  "version": "1.0",
  "description": "Release v1.2.0: Add my-sensor stream",
  "changes": [
    {"type": "stream", "id": "my-sensor", "action": "create"},
    {"type": "silver-table", "stream_id": "my-sensor", "action": "sync"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

# 3. Commit and push
git add config/base/streams/my-sensor/ .deploy/releases/v1.2.0.manifest.json
git commit -m "feat: Add my-sensor stream (v1.2.0)"
git tag -a v1.2.0 -m "Release v1.2.0"
git push && git push --tags

# 4. On Pi: Deploy the release
git pull
./deploy.sh apply .deploy/releases/v1.2.0.manifest.json
```

### Manifest Naming Convention

Release manifests follow the pattern: `v{MAJOR}.{MINOR}.{PATCH}.manifest.json`

```
.deploy/
├── manifest.json                    # Working manifest (optional)
└── releases/
    ├── v1.0.0.manifest.json         # Initial release
    ├── v1.1.0.manifest.json         # Added outdoor-weather
    ├── v1.2.0.manifest.json         # Added my-sensor
    └── v1.2.1.manifest.json         # Bug fix release
```

This provides:
- **Audit trail**: Every release manifest is preserved
- **Rollback reference**: Know exactly what was deployed in each version
- **Git tag alignment**: Manifest version matches git tag

### 9-Phase Orchestration

The `apply` command executes declarations in dependency order:

| Phase | Description |
|-------|-------------|
| 1. Validation | Validate manifest schema and infrastructure readiness |
| 2. Container Builds | Build container images (if declared) |
| 3. Migrations | Run SQL migration files |
| 4. Silver Tables | Generate and apply DDL from config |
| 5. Streams | Sync stream configs to etcd |
| 6. Dimensions | Sync dimension CSVs to TimescaleDB |
| 7. Dictionary | Sync data dictionary metadata |
| 8. Container Restarts | Restart containers (if declared) |
| 9. Device State | Update `/var/ndp/deployed-*` files |

### Declaration Types

| Type | Purpose | Required Fields |
|------|---------|-----------------|
| `stream` | Sync stream config to etcd | `id` |
| `silver-table` | Generate/apply Silver DDL | `stream_id` |
| `migration` | Run SQL migration | `file` |
| `dimensions` | Sync dimension tables | - |
| `dictionary` | Sync data dictionary | - |
| `container` | Build or restart container | `target`, `action` |

Full documentation: [docs/procedures/DEPLOYMENT-DECLARATIVES.md](../../docs/procedures/DEPLOYMENT-DECLARATIVES.md)

---

## Services

| Service | Port | Description |
|---------|------|-------------|
| MQTT Broker | 1883 | Receives AirGradient sensor data |
| Air Quality App | 8080 | HTTP API, data ingestion |
| MCP Server | 9100 | AI agent integration |
| etcd | 2379 | Configuration store |
| TimescaleDB | 5432 | Silver layer time-series database |
| Grafana | 3000 | Dashboards and visualization |

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
docker exec mosquitto mosquitto_sub -t 'airgradient/#' -v

# Check stored Parquet files (Bronze layer)
docker exec air-quality-app ls -la /data/

# Check Silver layer tables
docker exec timescaledb psql -U postgres -d ndp -c "\dt silver.*"

# Query the API
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/readings/latest
```

## Data Storage

### Bronze Layer (Parquet)
- Location: Docker volume `pi_air-quality-data`
- Format: `{stream}/YYYY/MM/DD/*.parquet`

### Silver Layer (TimescaleDB)
- Schema: `silver.*`
- Tables: Hypertables with compression and retention policies

To backup:
```bash
# Bronze (Parquet files)
docker run --rm -v pi_air-quality-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/bronze-backup-$(date +%Y%m%d).tar.gz /data

# Silver (TimescaleDB)
docker exec timescaledb pg_dump -U postgres -d ndp -n silver > silver-backup.sql
```

## Configuration

Configuration is stored in:
- **Stream configs**: `config/base/streams/*/config.json`
- **etcd**: Runtime configuration registry
- **TimescaleDB**: Data dictionary and dimension tables

To modify config:
1. Edit files in `config/base/streams/{stream-id}/`
2. Create release manifest with changes
3. Deploy: `./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json`

## Resource Usage

Expected resource consumption on Pi 5:

| Service | Memory | CPU |
|---------|--------|-----|
| Mosquitto | ~50MB | <1% |
| etcd | ~100MB | <1% |
| Air Quality App | ~200MB | <5% |
| TimescaleDB | ~500MB | <5% |
| Grafana | ~200MB | <3% |
| **Total** | **~1GB** | **<15%** |

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
docker exec mosquitto mosquitto_sub -t '#' -v
```

**etcd not starting:**
```bash
# Check logs
docker compose logs etcd
# Reset etcd data if corrupted
docker volume rm pi_etcd-data
./deploy.sh start
```

**TimescaleDB issues:**
```bash
# Check logs
docker compose logs timescaledb
# Connect directly
docker exec -it timescaledb psql -U postgres -d ndp
```

**Apply command fails:**
```bash
# Check manifest syntax
cat .deploy/releases/vX.Y.Z.manifest.json | jq .

# Run with verbose logging
DEPLOY_ENV=integration ./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json
```

## Logs

```bash
# All services
./deploy.sh logs

# Specific service
docker compose logs -f air-quality-app
docker compose logs -f timescaledb
docker compose logs -f mosquitto
docker compose logs -f etcd
```

## Updating

```bash
# Standard release deployment
git pull
./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json

# Or full rebuild
./deploy.sh update
```
