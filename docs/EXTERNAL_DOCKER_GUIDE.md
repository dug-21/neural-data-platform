# External Docker Solution for Neural Trader

## Overview

This guide explains how to use the external Docker solution to run Neural Trader when facing disk space constraints in containerized development environments (like GitHub Codespaces or Dev Containers).

## Problem

When running inside a Dev Container or Codespace, building Docker images consumes the container's limited disk space, which can quickly fill up, especially with large Rust builds and multiple service images.

## Solution

The external Docker solution uses your host machine's Docker daemon instead of the container's Docker-in-Docker setup. This approach:

- **Saves disk space** by building images on the host
- **Improves performance** by avoiding nested virtualization
- **Maintains isolation** between development environment and services

## Prerequisites

1. Docker Desktop installed on your host machine
2. Docker configured to accept TCP connections (for Docker Desktop: Settings → General → "Expose daemon on tcp://localhost:2375 without TLS")
3. Access to host networking from your container

## Quick Start

### 1. Start Services with External Docker

```bash
# Make scripts executable (first time only)
chmod +x scripts/start_full_stock_simulation_external.sh
chmod +x scripts/stop_external_docker.sh

# Start the full stack using external Docker
./scripts/start_full_stock_simulation_external.sh
```

### 2. Stop Services

```bash
# Stop all services
./scripts/stop_external_docker.sh
```

## Configuration

### Custom Docker Host

If your Docker daemon is not at the default location, set the `DOCKER_HOST_OVERRIDE` environment variable:

```bash
# Example for remote Docker host
export DOCKER_HOST_OVERRIDE="tcp://192.168.1.100:2375"
./scripts/start_full_stock_simulation_external.sh

# Example for Docker using a different port
export DOCKER_HOST_OVERRIDE="tcp://localhost:2376"
./scripts/start_full_stock_simulation_external.sh
```

### Development Tools

The external configuration includes optional development tools that can be enabled:

- **Redis Commander**: Web UI for Redis (port 8081)
- **PgAdmin**: PostgreSQL management tool (port 8082)

These are disabled by default to save resources but can be started when needed.

## Architecture

```
┌─────────────────────────────────┐
│   Dev Container/Codespace       │
│                                 │
│  ┌──────────────────────────┐  │
│  │  Your Development Code   │  │
│  └──────────────────────────┘  │
│                                 │
│  Uses DOCKER_HOST to connect   │
│           ↓                     │
└─────────────────────────────────┘
            │
            │ TCP Connection
            │ (port 2375)
            ↓
┌─────────────────────────────────┐
│   Host Docker Daemon            │
│                                 │
│  ┌──────────────────────────┐  │
│  │   Neural Trader Stack    │  │
│  │  - TimescaleDB           │  │
│  │  - Redis                 │  │
│  │  - Neural Trader App     │  │
│  │  - Data Ingestion        │  │
│  └──────────────────────────┘  │
│                                 │
│  Images & containers run here   │
└─────────────────────────────────┘
```

## Troubleshooting

### Cannot connect to Docker daemon

1. Ensure Docker Desktop is running
2. Check TCP exposure is enabled in Docker settings
3. Verify firewall allows connection on port 2375
4. Try connecting manually: `DOCKER_HOST=tcp://host.docker.internal:2375 docker info`

### Services not accessible

- Services are exposed on standard ports (3030, 5432, 6379, etc.)
- Access them via `localhost` from your host machine
- From within the dev container, use `host.docker.internal`

### Build context issues

The script creates a temporary context at `/tmp/neural-trader-context`. If you encounter permission issues:

```bash
# Clean up and retry
rm -rf /tmp/neural-trader-context
./scripts/start_full_stock_simulation_external.sh
```

## Differences from Standard Setup

1. **Named volumes** instead of bind mounts for some directories
2. **Optimized build context** to reduce transfer size
3. **Profile-based optional services** to save resources
4. **Automatic cleanup** of temporary build directories

## Best Practices

1. **Stop services when not in use** to free host resources
2. **Use development tools sparingly** - enable only when needed
3. **Monitor host disk space** - images are now stored on host
4. **Regular cleanup** - remove unused images/volumes periodically

## Advanced Usage

### Custom Docker Compose

You can modify `docker-compose.external.yml` for your specific needs:

```bash
# Use custom compose file
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml up -d

# Scale specific services
DOCKER_HOST=$EXTERNAL_DOCKER_HOST docker-compose -f docker-compose.external.yml up -d --scale data-ingestion=2
```

### Direct Docker Commands

All Docker commands work with the DOCKER_HOST variable:

```bash
# List containers
DOCKER_HOST=tcp://host.docker.internal:2375 docker ps

# View logs
DOCKER_HOST=tcp://host.docker.internal:2375 docker logs neural-trader-neural-trader-1

# Execute commands
DOCKER_HOST=tcp://host.docker.internal:2375 docker exec -it neural-trader-timescaledb-1 psql -U postgres
```

## Security Considerations

⚠️ **Warning**: Exposing Docker daemon on TCP without TLS is insecure. Only use this in development environments behind a firewall. For production or public networks, use TLS authentication.

## Support

If you encounter issues:

1. Check the [main documentation](../README.md)
2. Review [Docker networking docs](https://docs.docker.com/network/)
3. Open an issue with detailed error messages and environment info