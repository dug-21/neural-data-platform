# Docker Deployment Guide

## Overview

This guide covers Docker deployment for the Neural Data Platform air-quality application across multiple architectures:
- **linux/amd64**: Mac Intel, cloud servers (AWS, GCP, Azure)
- **linux/arm64**: Mac M-series, Raspberry Pi 5

## Architecture

### Multi-Stage Docker Build

The Dockerfile uses a 4-stage build process:

1. **Chef Stage**: Prepares build environment with cargo-chef
2. **Planner Stage**: Analyzes dependencies and generates recipe
3. **Builder Stage**: Compiles application with cached dependencies
4. **Runtime Stage**: Minimal final image (<100MB compressed)

### Key Features

- Dependency caching with cargo-chef (faster rebuilds)
- Multi-architecture support (buildx)
- Non-root user for security
- Health checks
- Resource limits for Pi 5
- Persistent volumes for data and models

## Quick Start

### Development (Local Machine)

```bash
# Start development stack
./scripts/dev-up.sh

# Start with monitoring (Prometheus + Grafana)
./scripts/dev-up.sh --monitoring

# View logs
docker compose -p neural-air-quality logs -f

# Stop environment
./scripts/dev-down.sh

# Stop and clean volumes
./scripts/dev-down.sh --clean
```

Access services:
- Air Quality API: http://localhost:8080
- Health Check: http://localhost:8080/health
- Metrics: http://localhost:9090/metrics
- MQTT Broker: mqtt://localhost:1883
- Prometheus: http://localhost:9091 (with --monitoring)
- Grafana: http://localhost:3000 (with --monitoring, admin/admin)

### Production (Raspberry Pi 5)

```bash
# 1. Setup Pi 5 (one-time)
sudo ./scripts/setup-pi5.sh
sudo reboot

# 2. Deploy configuration files
sudo cp docker-compose.prod.yml /opt/neural/
sudo cp -r config /opt/neural/
sudo cp -r mosquitto /opt/neural/

# 3. Login to GitHub Container Registry
docker login ghcr.io -u YOUR_USERNAME

# 4. Start services
cd /opt/neural
docker compose -f docker-compose.prod.yml up -d

# 5. Enable auto-start
sudo systemctl enable neural-air-quality
sudo systemctl start neural-air-quality
```

Helper commands (after setup):
```bash
neural-status    # Show status and resource usage
neural-logs      # View application logs
neural-restart   # Restart application
```

## Building Multi-Architecture Images

### Prerequisites

1. Install Docker Desktop (includes buildx) or enable buildx
2. Ensure you have access to push to the container registry

### Build and Push

```bash
# Build and push (defaults to latest)
./scripts/build-multiarch.sh

# Build specific version
./scripts/build-multiarch.sh v0.1.0

# Build to custom registry
./scripts/build-multiarch.sh v0.1.0 ghcr.io/your-org
```

The script will:
1. Create or use existing buildx builder
2. Build for both amd64 and arm64
3. Push to container registry
4. Tag as both VERSION and latest

### Manual Build

```bash
# Create builder (one-time)
docker buildx create --name neural-builder --use --platform linux/amd64,linux/arm64

# Build and push
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag ghcr.io/neural-data-platform/air-quality:latest \
  --push \
  .
```

## Configuration

### Environment Variables

The application supports configuration via environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Logging level | `info` |
| `CONFIG_PATH` | Base config file | `/config/air-quality.yaml` |
| `CONFIG_OVERLAY` | Environment overrides | - |
| `DATA_DIR` | Data storage directory | `/data` |
| `MODELS_DIR` | Model storage directory | `/models` |
| `MQTT_BROKER_URL` | MQTT broker URL | `mqtt://mosquitto:1883` |
| `HTTP_PORT` | HTTP API port | `8080` |
| `METRICS_PORT` | Metrics port | `9090` |

### Configuration Files

Configuration uses a hierarchical overlay system:

1. **Base**: `/config/base/air-quality.yaml` - Default settings
2. **Development**: `/config/overlays/development/overrides.yaml` - Dev overrides
3. **Production**: `/config/overlays/production/overrides.yaml` - Prod overrides

Example: Modify prediction interval in production:

```yaml
# config/overlays/production/overrides.yaml
prediction:
  interval_minutes: 10  # Override default 5 minutes
```

### Resource Limits

#### Development (No Limits)
Development environment has no resource constraints for easier debugging.

#### Production (Pi 5 - 8GB RAM, 4 cores)

**Air Quality App:**
- CPU: 2 cores (limit), 1 core (reserved)
- Memory: 1.75GB (limit), 1GB (reserved)
- Threads: 2 (configured via env vars)

**Mosquitto:**
- CPU: 0.5 cores (limit), 0.25 cores (reserved)
- Memory: 256MB (limit), 128MB (reserved)

## Volumes and Persistence

### Development Volumes

Docker named volumes (managed by Docker):
- `mosquitto-data`: MQTT broker persistence
- `mosquitto-logs`: MQTT broker logs
- `air-quality-data`: Application data
- `air-quality-models`: ML models
- `prometheus-data`: Metrics storage
- `grafana-data`: Dashboard configuration

### Production Volumes (Pi 5)

Host bind mounts for easier backup:
- `/opt/neural/data/mosquitto`: MQTT persistence
- `/opt/neural/logs/mosquitto`: MQTT logs
- `/opt/neural/data/air-quality`: Application data
- `/opt/neural/models`: ML models

## Networking

### Development
- Network: `neural-network` (bridge)
- All services communicate via service names (DNS)

### Production
- Network: `pi5-neural-network` (bridge)
- Same internal DNS for service communication

### Port Mapping

| Service | Internal Port | External Port | Purpose |
|---------|---------------|---------------|---------|
| Air Quality App | 8080 | 8080 | HTTP API |
| Air Quality App | 9090 | 9090 | Metrics |
| Mosquitto | 1883 | 1883 | MQTT |
| Prometheus | 9090 | 9091 | Metrics UI (dev only) |
| Grafana | 3000 | 3000 | Dashboards (dev only) |

## Health Checks

All services include health checks:

### Air Quality App
```bash
curl -f http://localhost:8080/health
```

Checks:
- HTTP server responsive
- MQTT connection active
- Models loaded
- Storage available

### Mosquitto
```bash
mosquitto_sub -h localhost -t '$SYS/#' -C 1 -W 3
```

## Monitoring

### Prometheus Metrics

Metrics available at `http://localhost:9090/metrics`:
- Request latency
- Request count
- Error rates
- MQTT message rates
- Prediction accuracy
- System resources (CPU, memory, temperature on Pi 5)

### Grafana Dashboards

Access Grafana at `http://localhost:3000` (development with --monitoring):
- Default credentials: admin/admin
- Pre-configured Prometheus datasource
- Dashboard provisioning enabled

### Pi 5 Monitoring

```bash
# System status
neural-status

# Application logs
neural-logs

# Specific service logs
neural-logs mosquitto

# Temperature
vcgencmd measure_temp

# Resource usage
docker stats
```

## Troubleshooting

### Build Issues

**Error: cargo-chef not found**
```bash
# Pull latest cargo-chef image
docker pull lukemathwalker/cargo-chef:latest-rust-1.75
```

**Error: buildx not available**
```bash
# Install Docker Desktop or enable buildx
docker buildx install
```

### Runtime Issues

**Container fails to start**
```bash
# Check logs
docker compose logs air-quality-app

# Check health
docker inspect neural-air-quality | jq '.[0].State.Health'
```

**MQTT connection failed**
```bash
# Check Mosquitto logs
docker compose logs mosquitto

# Test MQTT connection
mosquitto_sub -h localhost -t 'test' -v
```

**Out of memory (Pi 5)**
```bash
# Check memory usage
docker stats

# Reduce limits in docker-compose.prod.yml
# Or disable predictions temporarily
```

### Pi 5 Specific

**Service won't start**
```bash
# Check systemd status
sudo systemctl status neural-air-quality

# Check Docker service
sudo systemctl status docker

# View journal
sudo journalctl -u neural-air-quality -f
```

**Temperature throttling**
```bash
# Check temperature
vcgencmd measure_temp

# Check throttling
vcgencmd get_throttled

# Improve cooling or reduce CPU limits
```

## Security Considerations

### Development
- No authentication on MQTT (allow_anonymous: true)
- No TLS/SSL
- Running as non-root user inside containers

### Production Recommendations

1. **Enable MQTT Authentication**
```bash
# Create password file
mosquitto_passwd -c /opt/neural/mosquitto/config/passwd username

# Update mosquitto.conf
allow_anonymous false
password_file /mosquitto/config/passwd
```

2. **Enable TLS/SSL**
```conf
# mosquitto.conf
listener 8883
cafile /mosquitto/certs/ca.crt
certfile /mosquitto/certs/server.crt
keyfile /mosquitto/certs/server.key
```

3. **Use Secrets Management**
```bash
# Docker secrets (swarm mode) or external vault
docker secret create mqtt_password mqtt_pass.txt
```

4. **Firewall Configuration**
```bash
# UFW example (Pi 5)
sudo ufw allow 22/tcp    # SSH
sudo ufw allow 8080/tcp  # API
sudo ufw enable
```

## Maintenance

### Backup Data (Pi 5)

```bash
# Backup script
#!/bin/bash
BACKUP_DIR=/backup/$(date +%Y%m%d)
mkdir -p $BACKUP_DIR

# Stop services
docker compose -f /opt/neural/docker-compose.prod.yml down

# Backup data
cp -r /opt/neural/data $BACKUP_DIR/
cp -r /opt/neural/models $BACKUP_DIR/
cp -r /opt/neural/config $BACKUP_DIR/

# Restart services
docker compose -f /opt/neural/docker-compose.prod.yml up -d
```

### Update Application

```bash
# Pull latest image
docker pull ghcr.io/neural-data-platform/air-quality:latest

# Restart with new image
docker compose -f /opt/neural/docker-compose.prod.yml up -d
```

### Clean Up

```bash
# Remove unused images
docker image prune -a

# Remove unused volumes
docker volume prune

# Remove all stopped containers
docker container prune
```

## Performance Tuning

### Pi 5 Optimizations

Already configured in production overlay:
- `RAYON_NUM_THREADS=2`: Limit parallel workers
- `TOKIO_WORKER_THREADS=2`: Limit async workers
- Batch size: 4 (memory efficient)
- Thread pool: 4 threads
- Model caching: 512MB

### Additional Tuning

Edit `/workspaces/neural-data-platform/config/overlays/production/overrides.yaml`:

```yaml
# Reduce memory usage
models:
  cache_size_mb: 256  # Reduce from 512

# Less frequent predictions
prediction:
  interval_minutes: 10  # Increase from 5

# Smaller data retention
storage:
  retention_days: 14  # Reduce from 30
  max_size_mb: 1024   # Limit storage
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Build and Push Docker Image

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to GHCR
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Build and push
        run: ./scripts/build-multiarch.sh ${GITHUB_REF#refs/tags/}
```

## Additional Resources

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose](https://docs.docker.com/compose/)
- [Buildx Multi-platform](https://docs.docker.com/build/building/multi-platform/)
- [Raspberry Pi Docker](https://docs.docker.com/engine/install/raspberry-pi-os/)
- [Mosquitto MQTT](https://mosquitto.org/documentation/)

## Support

For issues and questions:
1. Check logs: `docker compose logs`
2. Check health: `curl http://localhost:8080/health`
3. Review configuration files
4. Consult troubleshooting section above
