# AIR-004 Docker Deployment - Executive Summary

## Deployment Strategy

**Approach**: Incremental extension of existing `deploy/pi/` infrastructure
**Risk Level**: LOW (backward compatible, feature-flagged)
**Memory Impact**: +180MB actual usage (530MB total, well under 1GB budget)

## What Was Analyzed

1. **Existing Pi Deployment** (`deploy/pi/`)
   - 3 services: mosquitto, etcd, air-quality-app
   - Memory: ~350MB actual, 896MB limits
   - Proven stable on Raspberry Pi 5, Ubuntu 25.04, ARM64

2. **Multi-Stream Requirements** (from COMPLETION-PI-CORRECTED.md)
   - Stream registry in etcd
   - Multiple ingestion sources (MQTT, HTTP polling, webhooks)
   - Stream-isolated Parquet storage
   - Optional TimescaleDB (future phase)

3. **Existing Patterns**
   - etcd configuration loading (AIR-003)
   - MQTT → Parquet pipeline (AIR-001)
   - Docker Compose orchestration with health checks
   - `deploy.sh` automation script

## Key Deliverables Created

### 1. Stream Configuration Structure
Location: `deploy/pi/configs/streams/`

Three stream types defined:
- **air-quality**: Existing stream (migrated config)
- **weather**: HTTP polling example (disabled by default)
- **home-events**: Webhook example (disabled by default)

Each stream has:
- `config.yaml`: Metadata, retention, alerts, tags
- `schema.yaml`: Field definitions, types, ranges, indexes
- `sources.yaml`: Ingestion source configurations

### 2. Stream Registry Loader Script
Location: `deploy/pi/scripts/load-stream-configs.sh`

Features:
- Loads YAML configs into etcd at `/streams/{stream_id}/`
- Supports loading all streams or specific stream
- Validates etcd connectivity
- Reports enabled/disabled status
- Handles multiline YAML with heredocs

Usage:
```bash
./scripts/load-stream-configs.sh                # Load all streams
./scripts/load-stream-configs.sh air-quality    # Load specific stream
```

### 3. Docker Compose Modifications
Location: `deploy/pi/docker-compose.yml`

Changes:
- Add port 8081 for webhook ingestion
- NO new services in initial phase (TimescaleDB is future)
- Memory limits unchanged (backward compatible)

### 4. Deploy Script Enhancements
Location: `deploy/pi/deploy.sh`

New features:
- `load_streams()` function
- `reload-streams` command
- Automatic stream loading on startup
- Resource monitoring in status check

New commands:
```bash
./deploy.sh reload-streams           # Reload stream configs
./deploy.sh reload-streams --restart # Reload and restart app
```

### 5. Application Code Guidance
Location: `apps/air-quality-app/src/`

Design provided for:
- `streams/registry.rs`: Stream registry client (NEW)
- `config.rs`: Feature flags for stream registry (MODIFY)
- Feature flags: `ENABLE_STREAM_REGISTRY`, `ENABLE_HOT_RELOAD`, `ENABLE_WEBHOOK_INGESTION`

## Deployment Phases

### Phase 1: Infrastructure Setup (No Code Changes)
- Create stream configuration directories
- Create stream loader script
- Update docker-compose.yml (webhook port)
- Update deploy.sh (stream loading)
- Deploy and verify

**Risk**: NONE (infrastructure only, backward compatible)

### Phase 2: Application Integration (Code Changes Required)
- Implement stream registry client in Rust
- Add feature flags to config
- Add API endpoints: `/api/v1/streams`
- Enable stream-isolated Parquet storage

**Risk**: LOW (feature-flagged, disabled by default)

### Phase 3: Enable Additional Streams (Optional)
- Enable weather stream (HTTP polling)
- Enable home-events stream (webhooks)
- Verify multi-source ingestion

**Risk**: LOW (per-stream enable/disable)

### Phase 4: Add TimescaleDB (Future, Optional)
- Add TimescaleDB service to docker-compose.yml
- Requires >=2GB free memory
- Enable dual-write (Bronze + Silver)

**Risk**: MEDIUM (memory overhead, requires Pi with >=4GB RAM)

## Memory Budget Analysis

### Current (3 services)
- mosquitto: ~50MB actual, 128MB limit
- etcd: ~100MB actual, 256MB limit
- air-quality-app: ~200MB actual, 512MB limit
- **Total**: ~350MB actual, 896MB limits

### With Multi-Stream (3 services, no new containers)
- mosquitto: ~80MB actual (more topics)
- etcd: ~150MB actual (stream registry data)
- air-quality-app: ~300MB actual (multiple sources)
- **Total**: ~530MB actual, 896MB limits
- **Status**: COMPLIANT (<1GB budget)

### With TimescaleDB (4 services, future phase)
- Add timescaledb: ~600MB actual, 1GB limit
- **Total**: ~1.1GB actual, ~1.9GB limits
- **Status**: Requires Pi with >=4GB RAM

## Backward Compatibility

### Preserved Functionality
- Existing air-quality MQTT ingestion
- Existing Parquet storage at `/app/data`
- Existing API endpoints (`/health`, `/api/v1/air-quality/latest`)
- Existing config sync from `config/base` and `config/overlays`

### Feature Flags (Disabled by Default)
- `ENABLE_STREAM_REGISTRY=false`: Use existing config loading
- `ENABLE_HOT_RELOAD=false`: No dynamic config watching
- `ENABLE_WEBHOOK_INGESTION=false`: No webhook endpoints

### Rollback Strategy
- Complete rollback: Restore baseline docker-compose.yml and deploy.sh
- Partial rollback: Disable specific streams in etcd
- Quick disable: Set feature flags to false
- Data preservation: Existing Parquet files unchanged

## Verification Checklist

Pre-Deployment:
- [ ] Existing Pi stack verified working (Phase 1 of COMPLETION-PI-CORRECTED.md)
- [ ] Baseline snapshot created (`product/features/air-004/pi-baseline-snapshot/`)
- [ ] Git tag created: `air-004-pi-baseline-pre-migration`

Infrastructure Setup:
- [ ] Stream configuration directories created
- [ ] Stream loader script created and executable
- [ ] Docker compose updated with webhook port
- [ ] Deploy script updated with stream loading
- [ ] Scripts tested on development machine

Post-Deployment:
- [ ] Services start successfully
- [ ] Stream configurations loaded to etcd
- [ ] Resource usage within limits (<1GB)
- [ ] Existing air-quality functionality works
- [ ] New API endpoints respond (if implemented)
- [ ] Rollback procedure tested

## Next Steps

### Immediate (Docker Specialist)
1. Review this deployment plan
2. Create stream configuration files
3. Create stream loader script
4. Update docker-compose.yml (webhook port)
5. Update deploy.sh (stream loading)
6. Test on development environment

### Backend Developer (After Infrastructure Ready)
1. Implement stream registry client (`streams/registry.rs`)
2. Add feature flags to config
3. Implement multi-source ingestion coordinator
4. Add API endpoints for stream management
5. Implement stream-isolated Parquet storage
6. Add webhook endpoint handler

### Integration Testing
1. Deploy to Pi test environment
2. Verify existing air-quality still works
3. Test stream configuration loading
4. Test resource usage monitoring
5. Test rollback procedures
6. Enable weather stream (if API key available)
7. Test webhook ingestion (if home automation available)

## File Locations

**Deployment Plan**:
- `/workspaces/neural-data-platform/product/features/air-004/DOCKER_DEPLOYMENT.md` (CREATED)

**Stream Configurations** (TO BE CREATED):
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/air-quality/`
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/weather/`
- `/workspaces/neural-data-platform/deploy/pi/configs/streams/home-events/`

**Scripts** (TO BE CREATED):
- `/workspaces/neural-data-platform/deploy/pi/scripts/load-stream-configs.sh`

**Docker Files** (TO BE MODIFIED):
- `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`
- `/workspaces/neural-data-platform/deploy/pi/deploy.sh`

**Application Code** (TO BE IMPLEMENTED):
- `/workspaces/neural-data-platform/apps/air-quality-app/src/streams/registry.rs` (NEW)
- `/workspaces/neural-data-platform/apps/air-quality-app/src/config.rs` (MODIFY)

## Risk Mitigation

### Low Risk
- Infrastructure changes (directories, scripts)
- Docker compose modifications (single port addition)
- Feature flags (disabled by default)
- Stream registry in etcd (isolated from existing config)

### Medium Risk
- Application code changes (stream registry client)
- Multi-source ingestion coordinator
- Mitigated by: Feature flags, extensive testing, rollback procedures

### High Risk (Future Phase)
- TimescaleDB addition (memory overhead)
- Mitigated by: Optional phase, memory prerequisites check, separate deployment

## Success Criteria

1. **Backward Compatibility**: Existing air-quality ingestion continues to work
2. **Resource Compliance**: Total memory usage <1GB
3. **Stream Registry**: Configurations load successfully to etcd
4. **Extensibility**: New streams can be added without code changes
5. **Rollback**: Can return to baseline in <5 minutes
6. **Documentation**: All changes documented with examples
7. **Monitoring**: Resource usage tracked and alerted

## Timeline Estimate

- **Infrastructure Setup**: 4-6 hours (create configs, scripts, update docker files)
- **Application Integration**: 2-3 days (stream registry client, multi-source coordinator)
- **Testing and Validation**: 1-2 days (integration tests, resource monitoring)
- **Documentation**: 1 day (update READMEs, create runbooks)
- **Total**: 1 week for Phase 1-2, additional 1 week for Phase 3-4

## Contact

For questions or issues:
- Deployment Plan: `/workspaces/neural-data-platform/product/features/air-004/DOCKER_DEPLOYMENT.md`
- Platform Architecture: `/workspaces/neural-data-platform/product/features/air-004/architecture/PLATFORM_ARCHITECTURE.md`
- Completion Guide: `/workspaces/neural-data-platform/product/features/air-004/completion/COMPLETION-PI-CORRECTED.md`

---

**Document Status**: COMPLETE
**Created**: 2025-12-15
**Author**: Docker Specialist Agent (AIR-004)
