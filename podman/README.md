# Podman Development Architecture for Neural Trader

This directory contains the Podman-based development setup for Neural Trader, designed to leverage Podman's rootless containers, systemd integration, and pod-based service grouping.

## Architecture Overview

### Pod Structure

1. **Database Pod** (`neural-trader-db`)
   - TimescaleDB (PostgreSQL with time-series extensions)
   - pgAdmin (development only)
   
2. **Cache Pod** (`neural-trader-cache`)
   - Redis
   - Redis Commander (development only)

3. **Application Pod** (`neural-trader-app`)
   - Data Ingestion Service
   - Neural Trader Main Application

4. **Monitoring Pod** (`neural-trader-monitoring`)
   - Prometheus
   - Grafana

### Key Features

- **Rootless Containers**: All containers run without root privileges
- **Systemd Integration**: Services can be managed via systemd
- **SELinux Support**: Proper volume mount labels (`:Z`)
- **Pod Isolation**: Related services grouped in pods with shared network namespaces
- **Native Podman Commands**: No dependency on docker-compose
- **Podman-Compose Support**: Optional compatibility layer

## Quick Start

```bash
# Create all pods and start services
./scripts/podman-up.sh

# Stop all services
./scripts/podman-down.sh

# View status
./scripts/podman-status.sh

# Generate systemd units
./scripts/generate-systemd-units.sh
```

## Directory Structure

```
podman/
├── README.md
├── containers/           # Container definitions
│   ├── timescaledb/
│   ├── redis/
│   ├── data-ingestion/
│   └── neural-trader/
├── pods/                # Pod definitions
│   ├── database.yml
│   ├── cache.yml
│   ├── application.yml
│   └── monitoring.yml
├── scripts/            # Management scripts
│   ├── podman-up.sh
│   ├── podman-down.sh
│   ├── podman-status.sh
│   └── generate-systemd-units.sh
├── systemd/           # Systemd unit files
│   └── *.service
└── volumes/          # Named volume definitions
```

## SELinux Considerations

All volume mounts use the `:Z` flag for proper SELinux labeling when needed. This ensures containers can access host files in SELinux-enforcing environments.

## Network Architecture

Each pod has its own network namespace, with containers in the same pod sharing localhost. Inter-pod communication uses Podman's internal DNS.

## Security

- All containers run rootless by default
- Secrets managed via Podman secrets
- Network policies enforce isolation
- Resource limits enforced via cgroups v2