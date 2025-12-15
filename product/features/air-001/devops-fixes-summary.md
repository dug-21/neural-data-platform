# DevOps Configuration Fixes - Air Quality App

**Date**: 2025-12-14
**Branch**: feature/air-001-implementation
**Agent**: DevOps

---

## Problem Statement

The air-quality-app deployment had configuration persistence issues:
1. Volume was mounted at `/app/data` but configs specified different paths
2. Environment variable mismatch between docker-compose and application code
3. Application not using etcd configuration (developer task)

---

## Fixes Applied

### 1. Fixed docker-compose.yml Environment Variable
**File**: `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`

**Change**:
```diff
  environment:
    - RUST_LOG=info
-   - DATA_DIR=/app/data
+   - STORAGE_PATH=/app/data
    - ETCD_ENDPOINT=http://etcd:2379
```

**Reason**: Application code reads `STORAGE_PATH` env var, not `DATA_DIR`.

---

### 2. Fixed Base Configuration
**File**: `/workspaces/neural-data-platform/config/base/air-quality/config.yaml`

**Change**:
```diff
  storage:
-   base_path: "/data/parquet"
+   base_path: "/app/data"
    wal_enabled: true
```

**Reason**: Must match docker-compose volume mount at `/app/data`.

---

### 3. Fixed Production Configuration
**File**: `/workspaces/neural-data-platform/config/overlays/production/air-quality/config.yaml`

**Change**:
```diff
  storage:
-   base_path: "/var/data/air-quality/parquet"
+   base_path: "/app/data"
    batch_size: 500
```

**Reason**: Must match docker-compose volume mount at `/app/data`.

---

## Configuration Flow Documentation

### Complete Config Priority Chain

```
1. etcd config (when implemented)
   ├─ Synced from YAML via sync-config-to-etcd.sh
   └─ Can be overridden by environment variables
        └─ Format: AIR_QUALITY_STORAGE_BASE_PATH

2. Environment variables (current)
   └─ STORAGE_PATH=/app/data (set in docker-compose.yml)

3. YAML config files (current default)
   ├─ base/air-quality/config.yaml
   └─ overlays/production/air-quality/config.yaml (overrides base)

4. Application defaults (fallback)
   └─ Hardcoded in src/config.rs
```

### Current State (After Fixes)

**Application reads config in this order**:
1. Load from YAML file (config.yaml)
2. Apply environment variable overrides (STORAGE_PATH, MQTT_BROKER_URL, etc.)
3. Fall back to defaults if YAML not found

**Expected behavior after developer integrates etcd**:
1. Try to load from etcd at ETCD_ENDPOINT
2. Apply environment variable overrides
3. Fall back to YAML if etcd unavailable
4. Fall back to defaults if YAML unavailable

---

## File Changes Summary

| File | Change Type | Status |
|------|-------------|--------|
| `/deploy/pi/docker-compose.yml` | Environment variable fix | ✅ FIXED |
| `/config/base/air-quality/config.yaml` | Storage path fix | ✅ FIXED |
| `/config/overlays/production/air-quality/config.yaml` | Storage path fix | ✅ FIXED |
| `/apps/air-quality-app/src/main.rs` | Etcd integration | ⏳ DEVELOPER TASK |

---

## Verification Steps

### 1. Verify Config Sync to etcd

After fixes, run the sync script:
```bash
cd /workspaces/neural-data-platform
./scripts/sync-config-to-etcd.sh production
```

Expected etcd keys:
```
/air-quality/storage/base_path = "/app/data"
/air-quality/storage/wal_enabled = true
/air-quality/storage/batch_size = 500
/air-quality/storage/batch_timeout_secs = 10
```

Verify:
```bash
docker exec etcd etcdctl get --prefix /air-quality/storage
```

### 2. Verify Application Startup

Start the application:
```bash
cd /workspaces/neural-data-platform/deploy/pi
docker compose up -d
```

Check logs:
```bash
docker logs air-quality-app | head -50
```

Expected output:
```
INFO Loaded configuration from config.yaml
INFO Starting air quality server on 0.0.0.0:8080
INFO Initializing ParquetStore at: /app/data
```

### 3. Verify Data Persistence

Generate test data:
```bash
# Send MQTT message to trigger ingestion
docker exec mosquitto mosquitto_pub -t "airgradient/readings/test" -m '{"pm25": 12.5, "co2": 450}'
```

Check if parquet files are created:
```bash
docker exec air-quality-app ls -la /app/data/
```

Restart container and verify data persists:
```bash
docker compose restart air-quality-app
docker exec air-quality-app ls -la /app/data/
```

### 4. Verify Environment Variable Override

Test that STORAGE_PATH env var works:
```bash
# Temporarily override in docker-compose.yml
# - STORAGE_PATH=/tmp/test-data

docker compose up -d
docker exec air-quality-app ls -la /tmp/test-data/
```

---

## Configuration Patterns

### Environment Variable Naming

**Current pattern** (mixed):
- `STORAGE_PATH` - Legacy env var read by config.rs
- `MQTT_BROKER_URL` - Service-specific env var
- `ETCD_ENDPOINT` - Global env var

**etcd client pattern** (when implemented):
- `AIR_QUALITY_STORAGE_BASE_PATH` - Namespaced env var
- `AIR_QUALITY_MQTT_BROKER_URL` - Namespaced env var

**Recommendation**: Support both patterns for backward compatibility.

### Config File Hierarchy

```
config/
├── base/
│   └── air-quality/
│       └── config.yaml          # Base config for all environments
└── overlays/
    ├── development/
    │   └── air-quality/
    │       └── config.yaml      # Dev overrides (if needed)
    └── production/
        └── air-quality/
            └── config.yaml      # Production overrides
```

**How overlays work**:
1. Base config is loaded first
2. Overlay config is merged on top (overwrites matching keys)
3. Result is synced to etcd with `sync-config-to-etcd.sh`

---

## Known Issues & Developer Tasks

### Issues Resolved
- ✅ Storage path mismatch between docker volume and configs
- ✅ Environment variable name mismatch (DATA_DIR vs STORAGE_PATH)
- ✅ Production overlay had incorrect storage path

### Developer Must Complete
- ⏳ Update main.rs to call `config_etcd::load_from_etcd()` before falling back to YAML
- ⏳ Test etcd config loading with different environment combinations
- ⏳ Add config validation on startup (verify paths are writable)

### Future Enhancements
- Add health check that reports config source (etcd vs yaml vs defaults)
- Add API endpoint to view current configuration
- Add config hot-reload from etcd without restart
- Implement config versioning and rollback

---

## Testing Checklist for Developer

After integrating etcd config loading:

- [ ] App loads config from etcd when available
- [ ] App falls back to YAML when etcd unavailable
- [ ] Environment variables override both etcd and YAML
- [ ] Storage path is correct in all scenarios
- [ ] Data persists across container restarts
- [ ] Config changes in etcd are applied (with/without restart)
- [ ] Logs clearly show which config source was used
- [ ] All integration tests pass

---

## Related Files

### Deployment
- `/deploy/pi/docker-compose.yml` - Production deployment config
- `/scripts/sync-config-to-etcd.sh` - Config synchronization script

### Configuration
- `/config/base/air-quality/config.yaml` - Base configuration
- `/config/overlays/production/air-quality/config.yaml` - Production overrides

### Application
- `/apps/air-quality-app/src/config.rs` - File-based config loader
- `/apps/air-quality-app/src/config_etcd.rs` - etcd-based config loader (not yet used)
- `/apps/air-quality-app/src/main.rs` - Application entry point (needs update)

### Documentation
- `/product/features/air-001/config-flow-analysis.md` - Detailed analysis
- `/product/features/air-001/devops-fixes-summary.md` - This document

---

## Contact

**DevOps Agent** - Configuration and deployment fixes
**Developer Agent** - Application code integration (etcd config loading)
**Tester Agent** - Validation of fixes and integration

---

## Appendix: Config Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Docker Compose Start                      │
│  - Mounts volume: /app/data                                  │
│  - Sets env: STORAGE_PATH=/app/data                          │
│  - Sets env: ETCD_ENDPOINT=http://etcd:2379                  │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              Application Startup (main.rs)                   │
└──────────────────────┬──────────────────────────────────────┘
                       │
       ┌───────────────┴───────────────┐
       │                               │
       ▼ (current)                     ▼ (future)
┌──────────────────┐          ┌──────────────────┐
│  Load from YAML  │          │  Load from etcd  │
│  + env overrides │          │  + env overrides │
└─────────┬────────┘          └─────────┬────────┘
          │                             │
          │                    ┌────────┘
          │                    │ (on etcd failure)
          ▼                    ▼
┌─────────────────────────────────────────┐
│  Final Config Applied                   │
│  storage.base_path = "/app/data"        │
│  (from STORAGE_PATH env var)            │
└──────────────────┬──────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────┐
│  ParquetStore initialized at /app/data  │
│  Data written to docker volume          │
│  Data persists across restarts          │
└─────────────────────────────────────────┘
```

---

## End of Summary

**Status**: DevOps fixes complete. Waiting for developer to integrate etcd config loading.

**Next Steps**:
1. Developer updates main.rs to use config_etcd
2. Tester validates all config scenarios
3. Deploy to production after validation
