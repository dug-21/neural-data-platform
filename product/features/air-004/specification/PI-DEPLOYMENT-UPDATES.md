# AIR-004 Specification Updates: Pi Deployment Constraints

## Document Information
- **Update Date**: 2025-12-15
- **Specification Version**: 1.2.0 (Pi Deployment Constraints)
- **Updated By**: SPARC Specification Agent

## Summary of Changes

This update corrects the AIR-004 specification to accurately reflect the production deployment environment: **Raspberry Pi 5 running Ubuntu 25.04 ARM64**.

## Key Updates

### 1. Technical Constraint TC-001: Home-Scale Deployment (UPDATED)

**What Changed:**
- Added explicit Raspberry Pi 5 hardware specifications
- Specified ARM64 architecture (`aarch64-unknown-linux-gnu`)
- Documented actual memory allocation (~896MB for platform services)
- Corrected deployment location to `deploy/pi/` (not `docker/production`)
- Listed actual services: mosquitto, etcd, air-quality-app

**Why It Matters:**
- Developers know exact hardware constraints for optimization
- Build processes target correct architecture
- Memory limits prevent OOM kills on Pi

### 2. NEW Constraint TC-005: Pi Deployment Preservation (ADDED)

**What Changed:**
- Protected deployment assets explicitly listed:
  - `deploy/pi/docker-compose.yml` - MUST remain functional
  - `deploy/pi/deploy.sh` - MUST NOT break
  - Volume names: `pi_air-quality-data`, `pi_etcd-data` - MUST persist
  - Data path: `/app/data` - MUST remain accessible
- Deployment process documented (build, deploy, rollback)
- Testing requirement: validate on Pi 5 before merge

**Why It Matters:**
- Prevents breaking changes to production deployment
- Ensures data persistence across updates
- Documents rollback procedure for failed deployments

### 3. Dependencies Section 6.1: Existing Components (UPDATED)

**What Changed:**
- Corrected Docker Compose location to `deploy/pi/docker-compose.yml`
- Listed actual deployed services with status indicators:
  - etcd: ✅ DEPLOYED (volume: `pi_etcd-data:/etcd-data`)
  - mosquitto: ✅ DEPLOYED (port: 1883)
  - air-quality-app: ✅ DEPLOYED (volume: `pi_air-quality-data:/app/data`)
  - TimescaleDB: ⚠️ NOT deployed (future consideration)
  - Grafana: ⚠️ Optional (not required for core)

**Why It Matters:**
- Developers know which services are actually available
- Prevents assumptions about TimescaleDB availability
- Clarifies Silver layer storage strategy (Parquet-only for v1.0)

### 4. NEW Non-Functional Requirement NFR-006: Pi Deployment Compatibility (ADDED)

**What Changed:**
Added comprehensive Pi deployment requirements:

#### NFR-006.1: Deployment Process
- Requirement: Deploy via `./deploy.sh` in `deploy/pi/`
- Acceptance: Services start within 5 minutes, health checks pass
- Test method: Execute on clean Pi 5 installation

#### NFR-006.2: Build Time
- Requirement: ARM64 build completes within 30 minutes on Pi 5
- Measurement: Time from `docker compose build` to ready
- Optimization: Build cache, multi-stage builds

#### NFR-006.3: Resource Constraints
- Requirement: Operate within memory budget
- Allocation breakdown:
  - mosquitto: ~50MB
  - etcd: ~300MB
  - air-quality-app: ~500MB
  - Total: ~850MB (margin for system)
- Measurement: `docker stats` after 24 hours
- Acceptance: No OOM kills, stable RSS

#### NFR-006.4: Backward Compatibility
- Requirement: Updates preserve existing data/config
- Acceptance: etcd keys accessible, Parquet files queryable, volumes work

**Why It Matters:**
- Establishes measurable deployment success criteria
- Prevents memory overruns on resource-constrained Pi
- Ensures smooth updates without data loss

### 5. Acceptance Test Scenario 8.4: Pi Production Deployment (ADDED)

**What Changed:**
Added three comprehensive deployment scenarios:

1. **Fresh deployment on clean Pi**
   - Build time validation (<30 minutes)
   - Service startup order and health checks
   - Volume creation verification

2. **Update existing deployment preserving data**
   - Incremental build time (<5 minutes with cache)
   - Graceful restart without data loss
   - Configuration and data persistence validation
   - Memory usage compliance

3. **Rollback on deployment failure**
   - Rollback procedure documented
   - Data preservation verification
   - Recovery to previous stable state

**Why It Matters:**
- Provides testable deployment workflows
- Documents rollback procedure for production incidents
- Ensures data safety during updates

### 6. Risk Assessment 9.3: Pi Deployment Risks (ADDED)

**What Changed:**
Added five Pi-specific risks:

| Risk ID | Description | Mitigation |
|---------|-------------|------------|
| R-008 | ARM64 build fails on Pi | Cross-compile from x86_64, use swap |
| R-009 | Volume corruption after unclean shutdown | Backups, WAL for Parquet |
| R-010 | Memory limit exceeded (OOM kills) | Conservative limits, monitoring, alerts |
| R-011 | SD card wear from frequent writes | External SSD, write batching |
| R-012 | Deployment script breaks config | Pre-deployment backup, rollback docs |

**Why It Matters:**
- Proactive risk identification for production environment
- Mitigation strategies documented for operations team
- Prevents common Pi deployment pitfalls

### 7. Validation Checklist Section 14 (UPDATED)

**What Changed:**
Added seven Pi-specific validation checkpoints:
- ✅ Pi 5 production constraints documented
- ✅ Correct deployment location specified
- ✅ Memory budget constraints defined
- ✅ ARM64 build requirements specified
- ✅ Volume preservation requirements documented
- ✅ Deployment process acceptance criteria defined
- ✅ Pi-specific risks identified and mitigated

**Why It Matters:**
- Ensures specification completeness for production deployment
- Provides checklist for future specification reviews
- Documents all Pi-related requirements

### 8. Document Metadata (UPDATED)

**What Changed:**
- Version: 1.1.0 → **1.2.0** (Pi Deployment Constraints)
- Status: "Aligned with Current Implementation" → **"Aligned with Pi Production Deployment"**
- Added: **Production Target**: Raspberry Pi 5 (Ubuntu 25.04 ARM64)
- Added: **Deployment Path**: `/workspaces/neural-data-platform/deploy/pi/`

**Why It Matters:**
- Clear version tracking for specification changes
- Immediately visible production target
- Correct deployment path for developers

## Impact Analysis

### What This Changes

1. **Development Workflow**
   - Must test on `deploy/pi/` configuration
   - Must validate on Pi 5 hardware before merge
   - Must build for ARM64 architecture

2. **Performance Targets**
   - Memory budget: <1GB total (not unlimited)
   - Build time: <30 minutes on Pi (not x86_64 cloud)
   - Resource monitoring required

3. **Storage Strategy**
   - TimescaleDB optional (not required for v1.0)
   - Parquet-only storage acceptable for initial release
   - External SSD recommended for data volumes

### What This Doesn't Change

- ✅ Core functional requirements (FR-001 through FR-006) remain valid
- ✅ Non-Pi NFRs (performance, reliability, security) remain valid
- ✅ Implementation phases remain the same
- ✅ Success criteria remain the same
- ✅ Backward compatibility with existing air-quality stream

## Critical Deployment Notes

**MUST DO** for all AIR-004 development:
1. Test changes on `deploy/pi/` configuration
2. Validate memory usage stays <896MB total
3. Build for `aarch64-unknown-linux-gnu`
4. Preserve volumes: `pi_air-quality-data`, `pi_etcd-data`
5. Ensure `./deploy.sh` remains functional
6. Validate on Pi 5 hardware before production deployment

**MUST NOT** do:
1. Break `deploy/pi/docker-compose.yml`
2. Change volume names or mount paths
3. Exceed 1GB memory budget
4. Remove or corrupt existing Parquet data
5. Invalidate existing etcd configuration keys

## Next Steps

1. **Phase 0: Baseline Verification**
   - Execute on Pi 5 hardware
   - Measure current memory usage
   - Document current performance metrics
   - Create regression test suite

2. **All Future Phases**
   - Test on Pi 5 before merge
   - Monitor memory usage
   - Validate deployment script
   - Preserve data volumes

## References

- **Deployment Configuration**: `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`
- **Deployment Script**: `/workspaces/neural-data-platform/deploy/pi/deploy.sh`
- **Data Volume**: `pi_air-quality-data` → `/app/data`
- **Config Volume**: `pi_etcd-data` → `/etcd-data`
- **Production Services**: mosquitto (1883), etcd (2379), air-quality-app (3000)

---

**Document Version**: 1.0
**Specification Version**: 1.2.0
**Last Updated**: 2025-12-15
**Status**: COMPLETE
