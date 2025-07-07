# Podman Quick Start Guide for Neural Trader

## Prerequisites

1. **Install Podman** (v4.0+):
   ```bash
   # Ubuntu/Debian
   sudo apt-get update
   sudo apt-get install -y podman
   
   # RHEL/CentOS/Fedora
   sudo dnf install -y podman
   
   # macOS
   brew install podman
   podman machine init
   podman machine start
   ```

2. **Enable rootless mode** (recommended):
   ```bash
   # Check if running rootless
   podman info | grep rootless
   
   # Enable lingering for systemd integration
   sudo loginctl enable-linger $USER
   ```

## Quick Start

### 1. Clone and Navigate
```bash
cd /workspaces/neural-trader
```

### 2. Set Environment Variables
```bash
# Copy example environment file
cp .env.example .env

# Edit with your API keys
nano .env
```

### 3. Start All Services
```bash
# Using helper scripts (recommended)
./podman/scripts/podman-up.sh

# Or using podman-compose
cd podman
podman-compose up -d

# Or using native Podman commands
./podman/scripts/podman-native-up.sh
```

### 4. Check Status
```bash
./podman/scripts/podman-status.sh
```

### 5. Access Services
- **Neural Trader App**: http://localhost:3030
- **Grafana Dashboard**: http://localhost:3000
- **Prometheus**: http://localhost:9090
- **pgAdmin**: http://localhost:8082 (development only)
- **Redis Commander**: http://localhost:8081 (development only)

## Managing Services

### Stop All Services
```bash
./podman/scripts/podman-down.sh
```

### Restart Services
```bash
./podman/scripts/podman-down.sh
./podman/scripts/podman-up.sh
```

### View Logs
```bash
# All services
podman logs -f neural-trader-app-neural-trader

# Specific pod
podman pod logs neural-trader-db
```

## Systemd Integration

### Generate Systemd Units
```bash
./podman/scripts/generate-systemd-units.sh
```

### Enable Auto-start on Boot
```bash
./podman/scripts/enable-rootless-autostart.sh
```

### Manage via Systemd
```bash
# Start
systemctl --user start neural-trader.target

# Stop
systemctl --user stop neural-trader.target

# Status
systemctl --user status neural-trader.target

# Logs
journalctl --user -u neural-trader.target -f
```

## Development Tips

### Access Pod Shell
```bash
# Database pod
podman exec -it neural-trader-db-timescaledb psql -U neural_trader -d neural_trader_db

# Redis pod
podman exec -it neural-trader-cache-redis redis-cli -a $REDIS_PASSWORD

# Application pod
podman exec -it neural-trader-app-neural-trader /bin/bash
```

### SELinux Contexts (if enabled)
```bash
# Check SELinux status
getenforce

# If enforcing, volumes use :Z flag automatically
# Manual relabel if needed
chcon -Rt svirt_sandbox_file_t ./config
```

### Network Troubleshooting
```bash
# List networks
podman network ls

# Inspect network
podman network inspect neural-trader-net

# Check DNS resolution
podman exec neural-trader-app-neural-trader nslookup neural-trader-db
```

## Common Issues

### 1. Permission Denied
```bash
# Run rootless (recommended)
podman unshare chown -R $UID:$GID /path/to/volume

# Or adjust permissions
chmod -R 755 ./config
```

### 2. Port Already in Use
```bash
# Find process using port
sudo lsof -i :5432

# Change port in pod YAML or use different port
```

### 3. Pod Won't Start
```bash
# Check pod status
podman pod inspect neural-trader-db

# Remove and recreate
podman pod rm -f neural-trader-db
./podman/scripts/podman-up.sh
```

## Production Deployment

### 1. Use Secrets Properly
```bash
# Create production secrets
podman secret create prod-db-password /path/to/password/file
```

### 2. Resource Limits
```bash
# Set in pod YAML or via CLI
--memory=4g --cpus=2
```

### 3. Monitoring
```bash
# Podman built-in metrics
podman stats

# Prometheus endpoint
curl http://localhost:3030/metrics
```

## Cleanup

### Remove Everything (including data)
```bash
./podman/scripts/podman-down.sh --remove-volumes --remove-images
```

### Remove Specific Components
```bash
# Remove pod
podman pod rm -f neural-trader-db

# Remove volume
podman volume rm neural-trader-timescale-data

# Remove network
podman network rm neural-trader-net
```