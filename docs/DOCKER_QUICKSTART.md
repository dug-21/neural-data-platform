# Docker Quick Start Guide

## 30-Second Start (Development)

```bash
# 1. Clone and enter directory
git clone https://github.com/neural-data-platform/neural-data-platform.git
cd neural-data-platform

# 2. Start everything
./scripts/dev-up.sh

# 3. Test it
curl http://localhost:8080/health
```

**Done!** Services running at:
- API: http://localhost:8080
- Metrics: http://localhost:9090/metrics
- MQTT: mqtt://localhost:1883

## 5-Minute Start (Raspberry Pi 5)

```bash
# 1. Setup Pi (one-time, includes reboot)
sudo ./scripts/setup-pi5.sh
sudo reboot

# 2. After reboot, deploy
sudo cp docker-compose.prod.yml /opt/neural/
sudo cp -r config /opt/neural/
sudo cp -r mosquitto /opt/neural/
docker login ghcr.io -u YOUR_USERNAME

# 3. Start
cd /opt/neural
docker compose -f docker-compose.prod.yml up -d

# 4. Check status
neural-status
```

## Common Commands

### Development

```bash
# Start
./scripts/dev-up.sh

# Start with monitoring
./scripts/dev-up.sh --monitoring

# View logs
docker compose -p neural-air-quality logs -f

# Stop
./scripts/dev-down.sh

# Stop and clean
./scripts/dev-down.sh --clean
```

### Production (Pi 5)

```bash
# Status
neural-status

# Logs
neural-logs

# Restart
neural-restart

# Manual control
cd /opt/neural
docker compose -f docker-compose.prod.yml [up|down|restart|logs]
```

## Building Images

```bash
# Build multi-architecture (amd64 + arm64)
./scripts/build-multiarch.sh

# Build specific version
./scripts/build-multiarch.sh v0.1.0

# Local build only
docker build -t air-quality:local .
```

## Testing

```bash
# Health check
curl http://localhost:8080/health

# Metrics
curl http://localhost:9090/metrics

# Publish test data
mosquitto_pub -h localhost -t 'airgradient/test/measures' \
  -m '{"pm02":12.5,"rco2":800,"atmp":22.5,"rhum":45.0}'

# Subscribe to predictions
mosquitto_sub -h localhost -t 'neural/predictions' -v
```

## Architecture Support

| Platform | Status | Use Case |
|----------|--------|----------|
| linux/amd64 | ✅ Full | Mac Intel, Cloud (AWS, GCP, Azure) |
| linux/arm64 | ✅ Full | Mac M-series, Raspberry Pi 5 |

## Key Features

- **Multi-stage builds**: Fast, cached, <100MB final image
- **Multi-arch**: Single image works on amd64 and arm64
- **Health checks**: Automatic monitoring and restart
- **Resource limits**: Optimized for Pi 5 (2GB RAM, 2 cores)
- **Persistent storage**: Data and models survive restarts
- **Environment configs**: Separate dev/prod settings
- **Monitoring ready**: Prometheus + Grafana support

## File Locations

### Development
```
docker-compose.yml              # Development stack
config/overlays/development/    # Dev settings
```

### Production (Pi 5)
```
/opt/neural/
  ├── docker-compose.prod.yml   # Production stack
  ├── config/                   # Configuration
  ├── data/                     # Persistent data
  ├── models/                   # ML models
  └── mosquitto/                # MQTT broker
```

## Configuration

Edit configuration files:

```bash
# Base settings (applies to all environments)
config/base/air-quality.yaml

# Development overrides
config/overlays/development/overrides.yaml

# Production overrides (Pi 5 optimized)
config/overlays/production/overrides.yaml
```

Common settings to adjust:

```yaml
# Prediction frequency
prediction:
  interval_minutes: 5

# Storage limits
storage:
  max_size_mb: 1024
  retention_days: 30

# Resource limits (production only)
resources:
  max_memory_mb: 1536
  max_cpu_percent: 80
```

## Troubleshooting

### Container won't start
```bash
# Check logs
docker compose logs air-quality-app

# Check resources
docker stats

# Verify config
docker compose config
```

### MQTT not connecting
```bash
# Check Mosquitto
docker compose logs mosquitto

# Test connection
mosquitto_sub -h localhost -t '$SYS/#' -v
```

### Out of memory (Pi 5)
```bash
# Check usage
neural-status

# Reduce limits in config/overlays/production/overrides.yaml:
models:
  cache_size_mb: 256  # Lower from 512
```

### High temperature (Pi 5)
```bash
# Check temp
vcgencmd measure_temp

# Reduce CPU usage in docker-compose.prod.yml:
deploy:
  resources:
    limits:
      cpus: '1.5'  # Lower from 2.0
```

## Full Documentation

- [Complete Deployment Guide](DOCKER_DEPLOYMENT.md)
- [Deployment Checklist](DEPLOYMENT_CHECKLIST.md)
- Main README (coming soon)

## Quick Reference

| Task | Command |
|------|---------|
| Start dev | `./scripts/dev-up.sh` |
| Stop dev | `./scripts/dev-down.sh` |
| View logs | `docker compose logs -f` |
| Check health | `curl localhost:8080/health` |
| Build image | `./scripts/build-multiarch.sh` |
| Pi status | `neural-status` |
| Pi logs | `neural-logs` |
| Pi restart | `neural-restart` |

## Environment Variables

Key environment variables (set in docker-compose files):

```bash
RUST_LOG=info              # Logging level
CONFIG_PATH=/config/...    # Config file location
MQTT_BROKER_URL=mqtt://... # MQTT broker
HTTP_PORT=8080             # API port
METRICS_PORT=9090          # Metrics port
DATA_DIR=/data             # Data storage
MODELS_DIR=/models         # Model storage
```

## Ports

| Port | Service | Purpose |
|------|---------|---------|
| 8080 | HTTP | API endpoints |
| 9090 | HTTP | Prometheus metrics |
| 1883 | MQTT | Message broker |
| 3000 | HTTP | Grafana (dev only) |
| 9091 | HTTP | Prometheus UI (dev only) |

## Next Steps

1. **Development**: Start with `./scripts/dev-up.sh`
2. **Production**: Follow [Deployment Checklist](DEPLOYMENT_CHECKLIST.md)
3. **Customization**: Edit config files as needed
4. **Monitoring**: Enable with `./scripts/dev-up.sh --monitoring`
5. **CI/CD**: See [Docker Deployment Guide](DOCKER_DEPLOYMENT.md#cicd-integration)
