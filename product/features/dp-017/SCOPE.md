# dp-017: Integration Environment Alignment

## Problem Statement

The integration test environment (`docker-compose.integration.yml`) has drifted from production (`deploy/pi/docker-compose.yml`). This makes it impossible to safely test deployment changes locally before deploying to the Pi.

**Discovered during dp-016 planning**: We need a declarative deployment system, but can't safely build one without a matching test environment.

---

## Current State

### deploy/pi/deploy.sh Already Supports Integration Mode

The deployment script already has `DEPLOY_ENV` switching:
```bash
DEPLOY_ENV=integration ./deploy.sh deploy   # Uses docker-compose.integration.yml
DEPLOY_ENV=pi ./deploy.sh deploy            # Uses deploy/pi/docker-compose.yml (default)
```

This is the foundation we're building on. The integration compose file needs to be a valid target.

### Production Stack (deploy/pi/docker-compose.yml)
- etcd
- mosquitto
- timescaledb
- air-quality-app (includes Silver ETL as event subscriber)
- grafana
- ndp-mcp-server

### Integration Stack (docker-compose.integration.yml) - STALE
- etcd ✅
- mosquitto ✅ (config outdated)
- timescaledb ✅ (init scripts path broken)
- air-quality-app ✅
- **silver-etl-daemon** ⚠️ OBSOLETE - ETL migrated to air-quality-app
- grafana ❌ missing
- ndp-mcp-server ❌ missing

### Other Compose Files (root level)
- `docker-compose.yml` - Minimal dev environment
- `docker-compose.prod.yml` - Purpose unclear, possibly stale

---

## Goals

1. **Align integration with production** - Same services, same architecture
2. **Remove obsolete artifacts** - silver-etl-daemon references
3. **Working test environment** - `docker compose up` creates functional stack
4. **Easy spin-up/down** - Test harness script for CI/local testing

---

## Tasks

### Compose File Alignment
| ID | Task | Description | Status |
|----|------|-------------|--------|
| 1 | Update docker-compose.integration.yml | Match production services | ✅ Done |
| 2 | Remove silver-etl-daemon | Delete obsolete service definition | ✅ Done |
| 3 | Add ndp-mcp-server | Mirror production | ✅ Done |
| 4 | Add grafana to default services | Remove `dashboards` profile to match production | Pending |
| 5 | Fix mosquitto config | Remove deprecated `max_retained_messages` | ✅ Done |
| 6 | Fix init script paths | Correct volume mount for TimescaleDB init | ✅ Done |
| 7 | Fix volume name | Rename `bronze-data` → `air-quality-data` | Pending |

### deploy.sh Fixes
| ID | Task | Description | Status |
|----|------|-------------|--------|
| 8 | Fix status() hardcoded container | `docker exec air-quality-app` → `dcx air-quality-app` | Pending |
| 9 | Fix refresh() hardcoded container | `docker restart grafana` → `dc restart grafana` | Pending |

### Verification
| ID | Task | Description | Status |
|----|------|-------------|--------|
| 10 | Verify infrastructure | etcd, timescaledb, mosquitto healthy | ✅ Done |
| 11 | Test `./deploy.sh sync` | Config sync to etcd works | Pending |
| 12 | Test `./deploy.sh init-streams` | Stream initialization works | Pending |
| 13 | Test `./deploy.sh sync-dictionary` | Data dictionary sync works | Pending |
| 14 | Test data flow | MQTT → Bronze → Silver | Pending |

### Deferred
| ID | Task | Description | Status |
|----|------|-------------|--------|
| 15 | Audit root compose files | Decide: keep, merge, or delete | Deferred |

---

## Out of Scope

- Building/pushing Docker images to registry
- CI/CD pipeline integration (future work)
- Performance testing
- Multi-Pi deployment
- Evaluating apps/silver-etl/ - Outside of removing from containerization/config, analysis/removal of the app is deferred

---

## Success Criteria

**Primary: deploy.sh works in integration mode**

1. `DEPLOY_ENV=integration ./deploy.sh deploy` starts full stack
2. `DEPLOY_ENV=integration ./deploy.sh status` shows all services healthy
3. `DEPLOY_ENV=integration ./deploy.sh sync` syncs config to etcd
4. `DEPLOY_ENV=integration ./deploy.sh init-streams` initializes streams
5. `DEPLOY_ENV=integration ./deploy.sh sync-dictionary` populates data dictionary
6. `DEPLOY_ENV=integration ./deploy.sh stop` cleanly stops all services

**Secondary: Data flow validation**

7. Can inject MQTT message and see it flow to Silver layer
8. No references to obsolete silver-etl-daemon

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Docker available | ✅ Confirmed | Works in dev container |
| Production compose | ✅ Exists | deploy/pi/docker-compose.yml |

---

## Blocking

- **dp-016**: Config Architecture Review (cannot safely test deployment without this)

---

## Estimated Effort

1-2 days

---

## References

- `deploy/pi/docker-compose.yml` - Production reference
- `docker-compose.integration.yml` - File to update
- dp-016 IMPLEMENTATION-ROADMAP.md - Documents this as prerequisite
