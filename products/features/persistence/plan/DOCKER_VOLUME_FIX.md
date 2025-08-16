# Docker Volume Configuration Fix - Neural-Trader Only

## Problem Statement

Current neural-trader Docker configuration has critical volume mount misalignment:
- **Neural-trader writes to**: `/opt/neural-trader/*`
- **Volumes mount to**: `/var/lib/neural-trader`
- **Result**: All neural-trader data written to ephemeral container filesystem, lost on restart

**NOTE**: This fix ONLY affects neural-trader service. Data-ingestion service is NOT modified.

## Immediate Fix Required

### 1. Docker Compose Volume Corrections

#### Current (BROKEN)
```yaml
neural-trader:
  volumes:
    - neural_trader_data:/var/lib/neural-trader      # WRONG PATH
    - neural_trader_logs:/var/log/neural-trader      # WRONG PATH
```

#### Fixed Configuration
```yaml
neural-trader:
  volumes:
    # Model persistence (CRITICAL)
    - neural_trader_models:/opt/neural-trader/models
    - neural_trader_checkpoints:/opt/neural-trader/checkpoints
    - neural_trader_backup:/opt/neural-trader/backup
    
    # Operational data
    - neural_trader_exports:/opt/neural-trader/exports
    - neural_trader_config:/opt/neural-trader/config:ro
    
    # Logging (correct path)
    - neural_trader_logs:/opt/neural-trader/logs
    
    # Metadata database
    - neural_trader_metadata:/opt/neural-trader/metadata
```

### 2. Volume Definitions Update

#### Add to volumes section:
```yaml
volumes:
  # Existing volumes (DO NOT MODIFY)
  # timescaledb_data: (keep as-is)
  # redis_data: (keep as-is)
  # data_ingestion_logs: (keep as-is - NOT OUR CONCERN)
    
  # Neural-trader specific volumes (NEW - ADD THESE ONLY)
  neural_trader_models:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ${MODEL_STORAGE_PATH:-./volumes/models}
      
  neural_trader_checkpoints:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ${CHECKPOINT_PATH:-./volumes/checkpoints}
      
  neural_trader_backup:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: ${BACKUP_PATH:-./volumes/backup}
      
  neural_trader_metadata:
    driver: local
    
  # Operational volumes
  neural_trader_exports:
    driver: local
  neural_trader_config:
    driver: local
  neural_trader_logs:
    driver: local
```

### 3. Environment Variable Configuration

#### Add to .env file:
```bash
# Model Storage Paths (absolute paths recommended)
MODEL_STORAGE_PATH=/data/neural-trader/models
CHECKPOINT_PATH=/data/neural-trader/checkpoints
BACKUP_PATH=/data/neural-trader/backup

# Or use relative paths for development
# MODEL_STORAGE_PATH=./volumes/models
# CHECKPOINT_PATH=./volumes/checkpoints
# BACKUP_PATH=./volumes/backup
```

### 4. Directory Structure Creation

#### Pre-deployment script:
```bash
#!/bin/bash
# create_volumes.sh

# Base directory
BASE_DIR="${MODEL_STORAGE_BASE:-/data/neural-trader}"

# Create directory structure
mkdir -p "${BASE_DIR}/models/active"
mkdir -p "${BASE_DIR}/models/archive"
mkdir -p "${BASE_DIR}/checkpoints"
mkdir -p "${BASE_DIR}/backup"
mkdir -p "${BASE_DIR}/exports"
mkdir -p "${BASE_DIR}/metadata"
mkdir -p "${BASE_DIR}/logs"

# Set permissions (Docker user will be neural:neural)
chown -R 1000:1000 "${BASE_DIR}"
chmod -R 755 "${BASE_DIR}"
chmod 700 "${BASE_DIR}/models"  # Restrict model access

echo "Volume directories created at ${BASE_DIR}"
```

### 5. Dockerfile Updates

#### Ensure correct ownership:
```dockerfile
# Runtime stage updates
FROM debian:bullseye-slim

# Create neural-trader user with specific UID/GID
RUN groupadd -r -g 1000 neural && \
    useradd -r -u 1000 -g neural neural

# Create ALL necessary directories
RUN mkdir -p /opt/neural-trader/{models,checkpoints,backup,exports,logs,config,metadata} \
    && mkdir -p /opt/neural-trader/models/{active,archive} \
    && chown -R neural:neural /opt/neural-trader \
    && chmod 755 /opt/neural-trader \
    && chmod 700 /opt/neural-trader/models

# Environment variables
ENV MODEL_STORAGE_PATH=/opt/neural-trader/models
ENV CHECKPOINT_PATH=/opt/neural-trader/checkpoints
ENV BACKUP_PATH=/opt/neural-trader/backup
ENV METADATA_PATH=/opt/neural-trader/metadata
```

### 6. Migration Strategy

#### Phase 1: Non-Breaking Addition (Day 1)
```yaml
# Add new volumes alongside existing (no removal yet)
volumes:
  - neural_trader_data:/var/lib/neural-trader  # Keep for now
  - neural_trader_models:/opt/neural-trader/models  # Add new
```

#### Phase 2: Data Migration (Day 2)
```bash
# If any data exists in old location, migrate it
docker exec neural_trader_app bash -c "
  if [ -d /var/lib/neural-trader ]; then
    cp -r /var/lib/neural-trader/* /opt/neural-trader/ 2>/dev/null || true
  fi
"
```

#### Phase 3: Cleanup (Day 3)
```yaml
# Remove old volume mounts after verification
volumes:
  # - neural_trader_data:/var/lib/neural-trader  # REMOVED
  - neural_trader_models:/opt/neural-trader/models
  # ... other new volumes
```

### 7. Backup Volume Strategy

#### Automated Backup Configuration:
```yaml
# docker-compose.prod.yml addition
backup:
  image: alpine:latest
  container_name: neural_trader_backup
  volumes:
    - neural_trader_models:/source/models:ro
    - neural_trader_checkpoints:/source/checkpoints:ro
    - neural_trader_backup:/backup
  environment:
    - BACKUP_SCHEDULE=${BACKUP_SCHEDULE:-0 2 * * *}  # 2 AM daily
  command: |
    sh -c "
    apk add --no-cache tar gzip
    while true; do
      tar -czf /backup/models_$(date +%Y%m%d_%H%M%S).tar.gz /source/models
      tar -czf /backup/checkpoints_$(date +%Y%m%d_%H%M%S).tar.gz /source/checkpoints
      find /backup -name '*.tar.gz' -mtime +7 -delete  # Keep 7 days
      sleep 86400  # Sleep 24 hours
    done
    "
```

### 8. Volume Permissions Script

#### fix_permissions.sh:
```bash
#!/bin/bash
# Fix permissions if needed after volume creation

VOLUMES_PATH="${1:-/data/neural-trader}"

# Fix ownership
docker run --rm \
  -v neural_trader_models:/models \
  -v neural_trader_checkpoints:/checkpoints \
  -v neural_trader_backup:/backup \
  alpine:latest \
  sh -c "chown -R 1000:1000 /models /checkpoints /backup"

echo "Permissions fixed for neural-trader volumes"
```

### 9. Verification Commands

#### Post-deployment verification:
```bash
# Verify volume mounts
docker exec neural_trader_app df -h | grep neural-trader

# Check write permissions
docker exec neural_trader_app touch /opt/neural-trader/models/test.txt && echo "Write test passed"

# Verify persistence
docker restart neural_trader_app
docker exec neural_trader_app ls -la /opt/neural-trader/models/

# Check volume actual location on host
docker volume inspect neural_trader_models
```

### 10. Monitoring Volume Health

#### Add health checks:
```yaml
healthcheck:
  test: |
    ["CMD", "sh", "-c", 
     "test -w /opt/neural-trader/models && 
      test -w /opt/neural-trader/checkpoints"]
  interval: 60s
  timeout: 10s
  retries: 3
```

## Rollback Plan

If issues occur after deployment:

1. **Stop containers**: `docker-compose down`
2. **Restore old config**: `git checkout docker-compose.prod.yml`
3. **Restart with old config**: `docker-compose up -d`
4. **Investigate issues**: Check logs and permissions

## Implementation Checklist

- [ ] Update docker-compose.prod.yml neural-trader service volumes ONLY
- [ ] Create host directories for neural-trader with create_volumes.sh
- [ ] Update .env with neural-trader storage paths
- [ ] Test neural-trader volume mounts in development
- [ ] Verify data-ingestion is UNCHANGED
- [ ] Deploy to staging environment
- [ ] Verify write permissions for neural-trader
- [ ] Test neural-trader persistence across restarts
- [ ] Deploy to production
- [ ] Monitor neural-trader for 24 hours
- [ ] Remove old neural-trader volume configurations

## Expected Outcomes

1. **Immediate**: Proper neural-trader volume mounting at correct paths
2. **Short-term**: Neural-trader model persistence across restarts
3. **Long-term**: Reliable neural-trader backup and recovery capability
4. **No Impact**: Data-ingestion service remains completely unchanged

## Risk Assessment

- **Low Risk**: Adding new volumes doesn't affect existing data
- **Medium Risk**: Permission issues may require container restart
- **Mitigation**: Staged rollout with verification at each step