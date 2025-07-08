# Docker Disk Space Management Guide for Neural Trader

## 🚨 Problem Summary

The neural-trader project running in GitHub Codespaces can consume significant disk space due to:
- Multiple Docker images for various services
- Build cache accumulation
- Stopped containers
- Unused volumes
- Log file growth

## ✅ Solution Overview

We've created a comprehensive Docker cleanup script that safely reclaims disk space while preserving important data.

### Quick Start

```bash
# Safe cleanup with confirmation prompts
./scripts/docker-cleanup.sh

# See what would be cleaned without doing it
./scripts/docker-cleanup.sh --dry-run

# Automatic cleanup without prompts
./scripts/docker-cleanup.sh --force
```

## 📊 Current Disk Usage Analysis

Based on the Codespaces environment:
- **Total Space**: 32GB
- **Used**: ~29GB (91%)
- **Available**: ~2.2GB

### Major Space Consumers:
1. **Docker Images**: Multiple service images (TimescaleDB, Redis, Python apps)
2. **Build Cache**: Accumulated from repeated builds
3. **Volumes**: Database data, logs, temporary files
4. **Containers**: Stopped but not removed containers

## 🛠️ Cleanup Script Features

### Safe Mode (Default)
- Removes stopped containers
- Cleans dangling images
- Removes anonymous volumes only
- Preserves named volumes
- Keeps images used in last 7 days

### Aggressive Mode
```bash
./scripts/docker-cleanup.sh --aggressive --force
```
- Stops all running containers
- Removes ALL unused images
- Removes ALL unused volumes
- Clears entire build cache
- Maximum space recovery

### Customizable Options
```bash
# Keep only last 3 days of images
./scripts/docker-cleanup.sh --days 3

# Don't preserve any volumes
./scripts/docker-cleanup.sh --no-preserve-volumes

# Dry run to preview changes
./scripts/docker-cleanup.sh --dry-run
```

## 📈 Expected Space Recovery

### Typical Cleanup Results:
- **Stopped Containers**: 100-500MB
- **Unused Images**: 2-5GB
- **Build Cache**: 1-3GB
- **Anonymous Volumes**: 200MB-1GB
- **Total Recovery**: 3-9GB typically

### After Aggressive Cleanup:
- Can recover 10-15GB or more
- Requires rebuilding images on next run

## 🔧 Preventive Measures

### 1. Docker Daemon Configuration
Add to `/etc/docker/daemon.json`:
```json
{
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  }
}
```

### 2. Container Log Limits
When running containers:
```bash
docker run --log-opt max-size=10m --log-opt max-file=3 <image>
```

### 3. Regular Cleanup Schedule
Add to crontab:
```bash
# Weekly cleanup on Sundays at 2 AM
0 2 * * 0 /workspaces/neural-trader/scripts/docker-cleanup.sh --force
```

### 4. Development Best Practices
- Use `.dockerignore` files
- Multi-stage builds for smaller images
- Regular manual cleanups during development
- Remove unused services from docker-compose

## 🚀 Immediate Actions

1. **Run Initial Cleanup**:
   ```bash
   cd /workspaces/neural-trader
   ./scripts/docker-cleanup.sh --dry-run  # Preview first
   ./scripts/docker-cleanup.sh             # Execute cleanup
   ```

2. **For Critical Space Issues**:
   ```bash
   # Stop all services first
   docker-compose down
   
   # Run aggressive cleanup
   ./scripts/docker-cleanup.sh --aggressive --force
   
   # Restart only needed services
   docker-compose up -d timescaledb redis
   ```

3. **Monitor Space**:
   ```bash
   # Check overall disk usage
   df -h /
   
   # Check Docker specific usage
   docker system df
   ```

## 📝 Script Usage Examples

### Before Major Development Session
```bash
# Clean up to ensure enough space
./scripts/docker-cleanup.sh --force
```

### After Build Errors
```bash
# Clear build cache if builds are failing
docker builder prune -a -f
```

### Weekly Maintenance
```bash
# Balanced cleanup preserving recent work
./scripts/docker-cleanup.sh --days 7 --force
```

### Emergency Space Recovery
```bash
# When space is critically low
./scripts/docker-cleanup.sh --aggressive --force
```

## 🔍 Monitoring and Logs

The cleanup script creates detailed logs at:
```
/tmp/docker-cleanup-YYYYMMDD_HHMMSS.log
```

Review logs to understand:
- What was removed
- How much space was recovered
- Any errors encountered

## ⚠️ Important Notes

1. **Data Loss Risk**: Aggressive cleanup removes volumes. Ensure important data is backed up.

2. **Rebuild Time**: After aggressive cleanup, images need rebuilding which takes time.

3. **Running Services**: The script won't remove resources from running containers unless using aggressive mode.

4. **Codespaces Limits**: GitHub Codespaces has storage limits. Regular cleanup is essential.

## 🤝 Integration with Neural Trader

The cleanup script is designed specifically for neural-trader:
- Preserves critical service data by default
- Integrates with existing scripts
- Follows project conventions
- Logs actions for debugging

## 📞 Support

If you encounter issues:
1. Check the cleanup log file
2. Run with `--dry-run` to preview
3. Use safe mode first before aggressive
4. Report persistent issues to the team

---

**Remember**: Regular cleanup prevents crisis situations. Schedule weekly cleanups to maintain healthy disk usage.