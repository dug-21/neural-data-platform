# Docker Multi-Architecture Deployment - Complete Guide

## 📋 Table of Contents

1. [Quick Start](#quick-start)
2. [What's Included](#whats-included)
3. [Architecture](#architecture)
4. [Getting Started](#getting-started)
5. [Documentation](#documentation)
6. [File Structure](#file-structure)
7. [Support](#support)

## 🚀 Quick Start

### Development (30 seconds)

```bash
cd /workspaces/neural-data-platform
./scripts/dev-up.sh
```

Access: http://localhost:8080/health

### Production - Raspberry Pi 5 (5 minutes)

```bash
# One-time setup
sudo ./scripts/setup-pi5.sh
sudo reboot

# Deploy
sudo cp docker-compose.prod.yml /opt/neural/
sudo cp -r config /opt/neural/
sudo cp -r mosquitto /opt/neural/
docker login ghcr.io
cd /opt/neural
docker compose -f docker-compose.prod.yml up -d
```

## 📦 What's Included

### ✅ Complete Docker Configuration

- **Multi-architecture support**: amd64 (Mac Intel, cloud) + arm64 (Mac M-series, Pi 5)
- **Multi-stage Dockerfile**: Optimized build with cargo-chef caching
- **Development stack**: Full environment with MQTT, monitoring (optional)
- **Production stack**: Pi 5 optimized with resource limits
- **Monitoring**: Prometheus + Grafana ready
- **Security**: Non-root user, minimal base image

### ✅ Configuration Management

- **Hierarchical config**: Base + environment overlays (FR-8.4 compliant)
- **MQTT broker**: Eclipse Mosquitto pre-configured
- **Application config**: Complete YAML configuration
- **Environment-specific**: Development vs Production settings

### ✅ Deployment Scripts

- **build-multiarch.sh**: Build for amd64 + arm64 simultaneously
- **dev-up.sh**: Start development environment
- **dev-down.sh**: Stop development environment
- **setup-pi5.sh**: Complete Raspberry Pi 5 setup

### ✅ Comprehensive Documentation

- **Deployment Guide**: 500+ lines, complete reference
- **Quick Start**: Fast reference for common tasks
- **Checklist**: Step-by-step deployment validation
- **Implementation Summary**: Technical specifications

## 🏗️ Architecture

### Multi-Stage Build

```
┌─────────────────────────────────────────────────┐
│ Stage 1: Chef (cargo-chef environment)          │
└─────────────────┬───────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────┐
│ Stage 2: Planner (analyze dependencies)         │
└─────────────────┬───────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────┐
│ Stage 3: Builder (compile with cache)           │
│ - Install build deps                            │
│ - Cook dependencies (cached)                    │
│ - Build application                             │
└─────────────────┬───────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────┐
│ Stage 4: Runtime (Debian bookworm-slim)         │
│ - Minimal dependencies                          │
│ - Non-root user                                 │
│ - Health checks                                 │
│ - Final size: <100MB                            │
└─────────────────────────────────────────────────┘
```

### Service Architecture

```
Development Stack:
┌─────────────────────────────────────────────────┐
│                                                 │
│  ┌──────────────┐      ┌──────────────┐       │
│  │   Mosquitto  │◄────►│ Air Quality  │       │
│  │   (MQTT)     │      │     App      │       │
│  └──────────────┘      └──────┬───────┘       │
│                               │                │
│  ┌──────────────┐      ┌──────▼───────┐       │
│  │  Prometheus  │◄─────│   Metrics    │       │
│  │  (optional)  │      │   Endpoint   │       │
│  └──────┬───────┘      └──────────────┘       │
│         │                                      │
│  ┌──────▼───────┐                             │
│  │   Grafana    │                             │
│  │  (optional)  │                             │
│  └──────────────┘                             │
│                                                 │
└─────────────────────────────────────────────────┘

Production Stack (Pi 5):
┌─────────────────────────────────────────────────┐
│                                                 │
│  ┌──────────────┐      ┌──────────────┐       │
│  │   Mosquitto  │◄────►│ Air Quality  │       │
│  │  (256MB RAM) │      │ (1.75GB RAM) │       │
│  │  (0.5 CPU)   │      │  (2.0 CPU)   │       │
│  └──────────────┘      └──────────────┘       │
│                                                 │
│  Persistent Volumes:                           │
│  - /opt/neural/data                            │
│  - /opt/neural/models                          │
│  - /opt/neural/logs                            │
│                                                 │
└─────────────────────────────────────────────────┘
```

### Configuration Hierarchy

```
┌─────────────────────────────────────────┐
│ config/base/air-quality.yaml            │
│ (Base configuration for all envs)       │
└──────────────┬──────────────────────────┘
               │
      ┌────────▼────────┐
      │   Environment   │
      │    Specific     │
      └────┬───────┬────┘
           │       │
    ┌──────▼─┐  ┌─▼────────┐
    │  Dev   │  │   Prod   │
    │Override│  │ Override │
    └────────┘  └──────────┘
           │       │
      ┌────▼───────▼────┐
      │ Environment Vars │
      └────────┬─────────┘
               │
      ┌────────▼─────────┐
      │ Final App Config │
      └──────────────────┘
```

## 🎯 Getting Started

### Prerequisites

**Development**:
- Docker Desktop or Docker Engine
- 4GB RAM, 2 CPU cores
- 10GB disk space

**Production (Pi 5)**:
- Raspberry Pi 5 with 8GB RAM
- 32GB+ SD card or SSD
- Pi OS 64-bit
- Internet connection

### Installation

#### 1. Clone Repository

```bash
git clone https://github.com/neural-data-platform/neural-data-platform.git
cd neural-data-platform
```

#### 2. Make Scripts Executable

```bash
chmod +x scripts/*.sh
```

#### 3. Choose Your Path

**For Development**:
```bash
./scripts/dev-up.sh
```

**For Production (Pi 5)**:
```bash
sudo ./scripts/setup-pi5.sh
# Follow prompts and reboot
# Then continue with deployment...
```

## 📚 Documentation

### Primary Guides

| Document | Purpose | When to Use |
|----------|---------|-------------|
| [DOCKER_QUICKSTART.md](DOCKER_QUICKSTART.md) | Fast reference | Daily operations |
| [DOCKER_DEPLOYMENT.md](DOCKER_DEPLOYMENT.md) | Complete guide | Initial setup, troubleshooting |
| [DEPLOYMENT_CHECKLIST.md](DEPLOYMENT_CHECKLIST.md) | Step-by-step | Production deployment |
| [DOCKER_IMPLEMENTATION_SUMMARY.md](DOCKER_IMPLEMENTATION_SUMMARY.md) | Technical specs | Understanding architecture |

### Quick Reference Links

**Common Tasks**:
- [Build multi-arch images](DOCKER_DEPLOYMENT.md#building-multi-architecture-images)
- [Start development](DOCKER_QUICKSTART.md#30-second-start-development)
- [Deploy to Pi 5](DOCKER_QUICKSTART.md#5-minute-start-raspberry-pi-5)
- [Configure settings](DOCKER_DEPLOYMENT.md#configuration)
- [Monitor services](DOCKER_DEPLOYMENT.md#monitoring)

**Troubleshooting**:
- [Container won't start](DOCKER_DEPLOYMENT.md#container-fails-to-start)
- [MQTT issues](DOCKER_DEPLOYMENT.md#mqtt-connection-failed)
- [Memory problems](DOCKER_DEPLOYMENT.md#out-of-memory-pi-5)
- [Performance tuning](DOCKER_DEPLOYMENT.md#performance-tuning)

## 📁 File Structure

### Core Files

```
neural-data-platform/
├── Dockerfile                          # Multi-stage build
├── docker-compose.yml                  # Development stack
├── docker-compose.prod.yml             # Production stack (Pi 5)
├── .dockerignore                       # Build optimization
│
├── config/
│   ├── base/
│   │   └── air-quality.yaml           # Base configuration
│   ├── overlays/
│   │   ├── development/
│   │   │   └── overrides.yaml         # Dev settings
│   │   └── production/
│   │       └── overrides.yaml         # Production settings
│   ├── prometheus.yml                  # Metrics scraping
│   └── grafana/
│       ├── datasources/
│       │   └── prometheus.yml         # Grafana datasource
│       └── dashboards/
│           └── dashboard.yml          # Dashboard config
│
├── mosquitto/
│   └── config/
│       └── mosquitto.conf             # MQTT broker config
│
├── scripts/
│   ├── build-multiarch.sh             # Multi-arch build
│   ├── dev-up.sh                      # Start development
│   ├── dev-down.sh                    # Stop development
│   └── setup-pi5.sh                   # Pi 5 setup
│
└── docs/
    ├── DOCKER_README.md               # This file
    ├── DOCKER_QUICKSTART.md           # Quick reference
    ├── DOCKER_DEPLOYMENT.md           # Complete guide
    ├── DEPLOYMENT_CHECKLIST.md        # Step-by-step
    └── DOCKER_IMPLEMENTATION_SUMMARY.md  # Tech specs
```

### Configuration Files Detail

**Base Configuration** (`config/base/air-quality.yaml`):
- HTTP server settings
- MQTT connection parameters
- Storage configuration
- Model inference settings
- Logging configuration
- Health checks

**Development Overrides** (`config/overlays/development/overrides.yaml`):
- Debug logging
- Relaxed timeouts
- Mock data enabled
- CORS enabled

**Production Overrides** (`config/overlays/production/overrides.yaml`):
- Resource limits
- Performance tuning
- Watchdog enabled
- System metrics

## 🔧 Common Operations

### Development

```bash
# Start
./scripts/dev-up.sh

# With monitoring
./scripts/dev-up.sh --monitoring

# View logs
docker compose -p neural-air-quality logs -f

# View specific service
docker compose -p neural-air-quality logs -f air-quality-app

# Stop
./scripts/dev-down.sh

# Stop and clean
./scripts/dev-down.sh --clean

# Restart single service
docker compose -p neural-air-quality restart air-quality-app
```

### Production (Pi 5)

```bash
# Status
neural-status

# Logs
neural-logs

# Restart
neural-restart

# Manual operations
cd /opt/neural
docker compose -f docker-compose.prod.yml [start|stop|restart|logs|ps]

# Check resources
docker stats --no-stream

# Check temperature
vcgencmd measure_temp
```

### Building Images

```bash
# Local build
docker build -t air-quality:local .

# Multi-architecture build
./scripts/build-multiarch.sh

# Build specific version
./scripts/build-multiarch.sh v0.1.0

# Build to custom registry
./scripts/build-multiarch.sh v0.1.0 ghcr.io/custom-org
```

### Testing

```bash
# Health check
curl http://localhost:8080/health

# Metrics
curl http://localhost:9090/metrics

# MQTT publish
mosquitto_pub -h localhost -t 'airgradient/test/measures' \
  -m '{"pm02":12.5,"rco2":800,"atmp":22.5,"rhum":45.0}'

# MQTT subscribe
mosquitto_sub -h localhost -t 'neural/predictions' -v

# All services
docker compose ps
```

## 🎨 Features

### Multi-Architecture

- ✅ **linux/amd64**: Mac Intel, AWS, GCP, Azure
- ✅ **linux/arm64**: Mac M-series, Raspberry Pi 5
- ✅ **Single image**: Works on all platforms
- ✅ **Build once**: Deploy anywhere

### Performance

- ✅ **Fast builds**: cargo-chef dependency caching
- ✅ **Small images**: <100MB compressed
- ✅ **Quick rebuilds**: 2-3 minutes for code changes
- ✅ **Optimized**: Pi 5 resource limits and tuning

### Security

- ✅ **Non-root**: Container runs as user (UID 1000)
- ✅ **Minimal base**: Debian bookworm-slim
- ✅ **No secrets**: Environment variables only
- ✅ **Health checks**: Automatic monitoring
- ✅ **Resource limits**: DoS prevention

### Observability

- ✅ **Health endpoints**: /health
- ✅ **Metrics**: Prometheus format
- ✅ **Logging**: JSON structured (prod)
- ✅ **Monitoring**: Grafana dashboards
- ✅ **Tracing**: Ready for distributed tracing

## 🛠️ Configuration

### Environment Variables

Key variables (set in docker-compose files):

```bash
RUST_LOG=info              # Logging level (debug, info, warn, error)
CONFIG_PATH=/config/...    # Base config file path
CONFIG_OVERLAY=/config/... # Environment override path
MQTT_BROKER_URL=mqtt://... # MQTT broker URL
HTTP_PORT=8080             # API port
METRICS_PORT=9090          # Prometheus metrics port
DATA_DIR=/data             # Data storage directory
MODELS_DIR=/models         # ML model directory
```

### Resource Limits

**Development**: No limits (for easier debugging)

**Production (Pi 5)**:
```yaml
Air Quality App:
  CPU: 2.0 cores (max)
  Memory: 1.75GB (max)
  Threads: 2

Mosquitto:
  CPU: 0.5 cores (max)
  Memory: 256MB (max)
```

### Ports

| Port | Service | Purpose |
|------|---------|---------|
| 8080 | HTTP | API endpoints, health check |
| 9090 | HTTP | Prometheus metrics |
| 1883 | MQTT | Message broker |
| 3000 | HTTP | Grafana dashboards (dev) |
| 9091 | HTTP | Prometheus UI (dev) |

## 🆘 Support

### Quick Help

**Container won't start**:
```bash
docker logs <container-name>
docker compose config
```

**High resource usage**:
```bash
docker stats
neural-status  # Pi 5 only
```

**MQTT problems**:
```bash
docker logs mosquitto
mosquitto_sub -h localhost -t '$SYS/#' -v
```

### Documentation

- [Quick Start Guide](DOCKER_QUICKSTART.md)
- [Full Deployment Guide](DOCKER_DEPLOYMENT.md)
- [Troubleshooting](DOCKER_DEPLOYMENT.md#troubleshooting)
- [Configuration Guide](DOCKER_DEPLOYMENT.md#configuration)

### External Resources

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose](https://docs.docker.com/compose/)
- [Mosquitto MQTT](https://mosquitto.org/)
- [Prometheus](https://prometheus.io/)
- [Grafana](https://grafana.com/)

## 📝 License

This configuration is part of the Neural Data Platform project.

## 🙏 Acknowledgments

- Built with [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) for fast Rust builds
- Uses [Eclipse Mosquitto](https://mosquitto.org/) MQTT broker
- Monitoring with [Prometheus](https://prometheus.io/) and [Grafana](https://grafana.com/)

---

**Last Updated**: 2025-12-13
**Version**: 1.0.0
**Status**: Production Ready
