# Docker Deployment Checklist

## Pre-Deployment Checklist

### Development Environment

- [ ] Docker and Docker Compose installed
- [ ] Docker daemon running
- [ ] Sufficient disk space (5GB+ recommended)
- [ ] Ports 8080, 9090, 1883 available
- [ ] Configuration files reviewed

### Production Environment (Raspberry Pi 5)

- [ ] Raspberry Pi 5 with 8GB RAM
- [ ] Pi OS 64-bit installed
- [ ] SSH access configured
- [ ] Static IP address assigned (recommended)
- [ ] Internet connection available
- [ ] 32GB+ SD card or SSD
- [ ] Adequate cooling solution
- [ ] Power supply (27W USB-C recommended)

## Build Checklist

### Multi-Architecture Build

- [ ] Docker buildx installed and configured
- [ ] GitHub Container Registry access configured
- [ ] Authentication to registry successful
- [ ] Multi-platform builder created
- [ ] Build script permissions set (`chmod +x`)

### Build Steps

```bash
# 1. Verify buildx
docker buildx version

# 2. Login to registry
docker login ghcr.io -u YOUR_USERNAME

# 3. Run build script
./scripts/build-multiarch.sh v0.1.0

# 4. Verify images pushed
docker manifest inspect ghcr.io/neural-data-platform/air-quality:latest
```

## Development Deployment Checklist

### Initial Setup

- [ ] Clone repository
- [ ] Review configuration in `config/base/air-quality.yaml`
- [ ] Review development overrides in `config/overlays/development/overrides.yaml`
- [ ] Create necessary directories

### Deployment Steps

```bash
# 1. Make scripts executable
chmod +x scripts/*.sh

# 2. Start development stack
./scripts/dev-up.sh

# 3. Verify services running
docker compose -p neural-air-quality ps

# 4. Check health endpoints
curl http://localhost:8080/health
curl http://localhost:9090/metrics

# 5. Test MQTT
mosquitto_sub -h localhost -t 'airgradient/+/measures' -v
```

### Verification

- [ ] All containers running (green status)
- [ ] Health checks passing
- [ ] API responds on port 8080
- [ ] Metrics available on port 9090
- [ ] MQTT broker accepts connections
- [ ] No errors in logs: `docker compose logs`

## Production Deployment Checklist (Pi 5)

### Step 1: Prepare Pi 5

```bash
# On Pi 5
cd /home/pi
git clone https://github.com/neural-data-platform/neural-data-platform.git
cd neural-data-platform

# Run setup script
sudo ./scripts/setup-pi5.sh

# Reboot for cgroup changes
sudo reboot
```

#### Verification
- [ ] Docker installed: `docker --version`
- [ ] Docker Compose installed: `docker compose version`
- [ ] Directories created: `ls -la /opt/neural`
- [ ] Helper commands available: `which neural-status`
- [ ] User in docker group: `groups pi | grep docker`

### Step 2: Deploy Configuration

```bash
# Copy configuration files to /opt/neural
sudo cp docker-compose.prod.yml /opt/neural/
sudo cp -r config /opt/neural/
sudo cp -r mosquitto /opt/neural/

# Create data directories
sudo mkdir -p /opt/neural/data/{air-quality,mosquitto}
sudo mkdir -p /opt/neural/logs/mosquitto
sudo mkdir -p /opt/neural/models

# Set permissions
sudo chown -R 1000:1000 /opt/neural
```

#### Verification
- [ ] Files in place: `ls -la /opt/neural`
- [ ] Permissions correct: `ls -l /opt/neural`
- [ ] Configuration valid: `cd /opt/neural && docker compose -f docker-compose.prod.yml config`

### Step 3: Pull Images

```bash
# Login to GitHub Container Registry
docker login ghcr.io -u YOUR_USERNAME

# Pull image
docker pull ghcr.io/neural-data-platform/air-quality:latest

# Verify image
docker images | grep air-quality
```

#### Verification
- [ ] Registry login successful
- [ ] Image pulled successfully
- [ ] Image architecture is arm64: `docker inspect ghcr.io/neural-data-platform/air-quality:latest | grep Architecture`

### Step 4: Start Services

```bash
# Manual start
cd /opt/neural
docker compose -f docker-compose.prod.yml up -d

# Or use systemd
sudo systemctl start neural-air-quality
```

#### Verification
- [ ] Containers running: `docker compose ps` or `neural-status`
- [ ] Health checks passing (wait 30s): `docker inspect pi5-air-quality | grep Health`
- [ ] No errors in logs: `neural-logs` or `docker compose logs`
- [ ] Resource usage acceptable: `docker stats --no-stream`

### Step 5: Verify Application

```bash
# Test API
curl http://localhost:8080/health

# Test metrics
curl http://localhost:9090/metrics

# Test MQTT
mosquitto_sub -h localhost -t 'airgradient/+/measures' -v

# Check system resources
neural-status
```

#### Verification
- [ ] Health endpoint returns 200 OK
- [ ] Metrics endpoint accessible
- [ ] MQTT broker accepting connections
- [ ] CPU usage < 80%
- [ ] Memory usage < 1.5GB
- [ ] Temperature < 70°C

### Step 6: Enable Auto-Start

```bash
# Enable systemd service
sudo systemctl enable neural-air-quality

# Verify enabled
sudo systemctl is-enabled neural-air-quality
```

#### Verification
- [ ] Service enabled
- [ ] Service status active: `systemctl status neural-air-quality`

## Post-Deployment Checklist

### Monitoring Setup

- [ ] Metrics collection verified
- [ ] Log rotation configured
- [ ] Disk space monitoring setup
- [ ] Temperature monitoring active
- [ ] Alert thresholds configured

### Security Hardening

- [ ] MQTT authentication enabled (production)
- [ ] TLS/SSL configured (production)
- [ ] Firewall rules configured
- [ ] SSH key authentication enabled
- [ ] Default passwords changed
- [ ] Unnecessary ports closed

### Backup Configuration

- [ ] Backup script created
- [ ] Backup schedule configured
- [ ] Backup location specified
- [ ] Restore procedure tested
- [ ] Retention policy defined

### Documentation

- [ ] Deployment documented
- [ ] Configuration documented
- [ ] Recovery procedures documented
- [ ] Contact information updated
- [ ] Runbook created

## Testing Checklist

### Functional Testing

```bash
# 1. Publish test message
mosquitto_pub -h localhost -t 'airgradient/test/measures' \
  -m '{"pm02":12.5,"rco2":800,"atmp":22.5,"rhum":45.0}'

# 2. Check logs for processing
neural-logs | grep "test"

# 3. Verify prediction output
mosquitto_sub -h localhost -t 'neural/predictions' -v -C 1

# 4. Test API endpoints
curl http://localhost:8080/health
curl http://localhost:8080/api/v1/status  # if available
```

#### Verification
- [ ] Message received and processed
- [ ] Predictions generated
- [ ] No errors in logs
- [ ] API responses correct

### Performance Testing

```bash
# Monitor resources during operation
watch -n 5 'docker stats --no-stream'

# Check temperature
watch -n 10 'vcgencmd measure_temp'

# Test sustained load (if applicable)
# ... load testing script ...
```

#### Verification
- [ ] CPU usage stable
- [ ] Memory usage stable
- [ ] No memory leaks observed
- [ ] Temperature within limits
- [ ] Response times acceptable

### Recovery Testing

```bash
# 1. Stop services
docker compose -f /opt/neural/docker-compose.prod.yml down

# 2. Restart services
docker compose -f /opt/neural/docker-compose.prod.yml up -d

# 3. Verify recovery
neural-status
```

#### Verification
- [ ] Services restart successfully
- [ ] Data persisted correctly
- [ ] Connections re-established
- [ ] No data loss

## Rollback Checklist

### If Deployment Fails

```bash
# 1. Stop new deployment
docker compose down

# 2. Restore previous configuration
# ... restore from backup ...

# 3. Start previous version
docker compose up -d

# 4. Verify old version working
curl http://localhost:8080/health
```

#### Steps
- [ ] Current deployment stopped
- [ ] Previous configuration restored
- [ ] Previous version running
- [ ] Services healthy
- [ ] Root cause identified

## Maintenance Checklist (Weekly)

- [ ] Check disk space: `df -h`
- [ ] Review logs for errors: `neural-logs`
- [ ] Check resource usage: `neural-status`
- [ ] Verify backups completed
- [ ] Update images if needed: `docker pull ...`
- [ ] Clean up old images: `docker image prune`
- [ ] Check temperature trends
- [ ] Review metrics in Prometheus (if enabled)

## Troubleshooting Reference

### Common Issues

| Issue | Check | Solution |
|-------|-------|----------|
| Container won't start | Logs: `docker logs pi5-air-quality` | Check config, resources |
| High memory usage | `docker stats` | Adjust limits in compose file |
| MQTT connection fails | `docker logs pi5-mosquitto` | Check broker config, network |
| High temperature | `vcgencmd measure_temp` | Improve cooling, reduce load |
| Disk full | `df -h` | Clean old data, adjust retention |
| Service not auto-starting | `systemctl status neural-air-quality` | Check systemd service config |

### Quick Commands

```bash
# View status
neural-status

# View logs
neural-logs

# Restart app
neural-restart

# Full restart
cd /opt/neural && docker compose -f docker-compose.prod.yml restart

# Emergency stop
docker compose -f /opt/neural/docker-compose.prod.yml down
```

## Sign-Off Checklist

Before marking deployment complete:

- [ ] All services running and healthy
- [ ] Monitoring configured and working
- [ ] Backups configured and tested
- [ ] Documentation complete and accessible
- [ ] Team trained on operations
- [ ] Emergency contacts updated
- [ ] Rollback procedure tested
- [ ] Handoff to operations complete

## Notes Section

**Deployment Date**: _________________

**Deployed By**: _________________

**Version**: _________________

**Issues Encountered**:

**Resolutions**:

**Special Configuration**:

**Next Steps**:
