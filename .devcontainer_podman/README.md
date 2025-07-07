# Neural Trader - Podman Development Environment

This directory contains the configuration for running Neural Trader in a Podman-based development environment on your local workstation.

## Why Podman?

- **Rootless by default**: Enhanced security without requiring root privileges
- **No daemon**: Direct process execution, lower overhead
- **Docker-compatible**: Can use existing Docker images and compose files
- **Better systemd integration**: Native support for generating systemd services
- **SELinux-friendly**: Designed to work with SELinux enabled

## Prerequisites

1. **Install Podman** (4.0+):
   ```bash
   # Fedora/RHEL/CentOS
   sudo dnf install podman podman-compose

   # Ubuntu/Debian
   sudo apt-get update
   sudo apt-get install podman podman-compose

   # macOS
   brew install podman podman-compose
   ```

2. **Configure Podman for rootless operation**:
   ```bash
   # Set up rootless Podman
   podman system migrate
   
   # Enable lingering for systemd services
   loginctl enable-linger $USER
   ```

3. **VS Code with Dev Containers extension**

## Quick Start

1. **Clone the repository**:
   ```bash
   git clone https://github.com/your-org/neural-trader.git
   cd neural-trader
   ```

2. **Open in VS Code**:
   ```bash
   code .
   ```

3. **Reopen in Container**:
   - Press `F1` → "Dev Containers: Reopen in Container"
   - Select "Neural Trader - Podman Development"

4. **Services will start automatically**, or manually run:
   ```bash
   bash .devcontainer_podman/scripts/start-services.sh
   ```

## Architecture

### Pod Structure

The environment uses Podman pods to group related services:

1. **Database Pod** (`neural-trader-db-pod`):
   - TimescaleDB (PostgreSQL with time-series extensions)
   - pgAdmin (database management UI)

2. **Cache Pod** (`neural-trader-cache-pod`):
   - Redis (in-memory data store)
   - Redis Commander (Redis management UI)

3. **Application Pod** (`neural-trader-app-pod`):
   - Neural Trader application
   - Data Ingestion service

4. **Monitoring Pod** (`neural-trader-monitoring-pod`):
   - Prometheus (metrics collection)
   - Grafana (visualization)

### Network Configuration

- All pods share the `neural-trader-net` network
- Services communicate using container names
- Rootless networking via slirp4netns

### Volume Mounts

- Uses `:Z` flag for proper SELinux context
- Persistent volumes for databases
- Bind mounts for development code

## Usage

### Starting Services

```bash
# Using scripts (recommended)
bash .devcontainer_podman/scripts/start-services.sh

# Using podman-compose
cd .devcontainer_podman
podman-compose up -d

# Using native Podman commands
podman pod start neural-trader-db-pod
podman pod start neural-trader-cache-pod
```

### Stopping Services

```bash
# Stop all services
bash .devcontainer_podman/scripts/stop-services.sh

# Stop and remove pods
bash .devcontainer_podman/scripts/stop-services.sh --clean
```

### Accessing Services

| Service | URL | Credentials |
|---------|-----|-------------|
| PostgreSQL | `localhost:5432` | `postgres:dev_password` |
| Redis | `localhost:6379` | No auth |
| pgAdmin | `http://localhost:8082` | `admin@neural-trader.local:admin` |
| Redis Commander | `http://localhost:8081` | No auth |
| Neural Trader API | `http://localhost:3030` | See .env |
| Prometheus | `http://localhost:9090` | No auth |
| Grafana | `http://localhost:3000` | `admin:admin` |

### Database Access

```bash
# PostgreSQL CLI
podman exec -it timescaledb psql -U postgres -d neural_trader

# Redis CLI
podman exec -it redis redis-cli
```

### Viewing Logs

```bash
# View specific container logs
podman logs -f timescaledb
podman logs -f redis

# View pod logs
podman pod logs neural-trader-db-pod
```

## Systemd Integration

Generate systemd services for automatic startup:

```bash
# Generate systemd units
podman generate systemd --new --name neural-trader-db-pod > ~/.config/systemd/user/neural-trader-db.service
podman generate systemd --new --name neural-trader-cache-pod > ~/.config/systemd/user/neural-trader-cache.service

# Enable services
systemctl --user enable neural-trader-db.service
systemctl --user enable neural-trader-cache.service

# Start services
systemctl --user start neural-trader-db.service
systemctl --user start neural-trader-cache.service
```

## Troubleshooting

### Permission Issues

```bash
# Fix SELinux contexts
chcon -Rt svirt_sandbox_file_t /path/to/mounted/directory

# Or use :Z flag in volume mounts (recommended)
-v /host/path:/container/path:Z
```

### Networking Issues

```bash
# Check Podman network
podman network ls
podman network inspect neural-trader-net

# Recreate network if needed
podman network rm neural-trader-net
podman network create neural-trader-net
```

### Storage Issues

```bash
# Check storage
podman system df

# Clean up unused resources
podman system prune -a

# Reset storage (WARNING: removes all data)
podman system reset
```

## Differences from Docker

1. **No Docker daemon**: Podman runs directly, no background service
2. **Rootless by default**: Better security, no sudo required
3. **Pods**: Group containers like Kubernetes
4. **Systemd integration**: Native service management
5. **Different socket location**: `/run/user/1000/podman/podman.sock`

## Migration from Docker

1. **Images**: Same images work, just change registry if needed
2. **Compose files**: Use `podman-compose` or convert to pods
3. **Volumes**: Add `:Z` for SELinux contexts
4. **Networking**: Similar but uses CNI instead of docker0
5. **Commands**: `docker` → `podman` (mostly compatible)

## Security Benefits

- **Rootless containers**: No privilege escalation risks
- **User namespaces**: Better isolation
- **SELinux enforcement**: Additional security layer
- **No shared daemon**: Isolated container processes
- **Audit logging**: Better security tracking

## Performance Tips

1. **Use fuse-overlayfs**: Better performance for rootless
2. **Enable caching**: `--storage-opt overlay.mount_program=/usr/bin/fuse-overlayfs`
3. **Adjust ulimits**: Increase file descriptors if needed
4. **Use pods**: Shared namespaces reduce overhead
5. **Native systemd**: More efficient than Docker's approach

## Additional Resources

- [Podman Documentation](https://docs.podman.io)
- [Migrating from Docker](https://podman.io/whatis.html)
- [Podman Compose](https://github.com/containers/podman-compose)
- [Rootless Podman](https://github.com/containers/podman/blob/main/docs/tutorials/rootless_tutorial.md)