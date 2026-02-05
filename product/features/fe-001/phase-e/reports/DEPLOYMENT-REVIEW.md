# Phase E Deployment Readiness Review

> **Date:** 2026-02-05
> **Reviewer:** ndp-scrum-master
> **Feature:** FE-001 Gold Layer Foundation - Phase E
> **Current Version:** v1.1.5 (deployed to Pi)

---

## Executive Summary

**Overall Deployment Readiness Score: 65/100**

Phase E SPARC artifacts are well-specified but have **critical gaps** in deployment infrastructure. The project has established excellent release methodology (RELEASE-POLICY.md, DEPLOYMENT-DECLARATIVES.md), but Phase E introduces new deployment requirements that are not yet implemented:

1. **Grafana dashboard deployment** - Declaration type not defined in manifest schema
2. **SQL view declaration** - No `sql-view` type for events_unified
3. **Missing deployment scripts** - No `handle_grafana_dashboard()` in deploy.sh

---

## Version Recommendation

**Recommended Version for Phase E: v1.2.0**

| Rationale | Version Impact |
|-----------|----------------|
| New feature: Threshold crossing events (v11-012) | MINOR bump |
| New feature: Unified events view (v11-013) | MINOR bump |
| New feature: Gold Layer Dashboard (v11-014) | MINOR bump |
| No breaking changes to existing APIs | Not MAJOR |
| More than bug fix | Not PATCH |

**Note:** The current version is v1.1.5. Phase E should be released as **v1.2.0** following SemVer guidelines for new features.

---

## Deployment Artifacts Assessment

### Manifest Template Readiness

| Criterion | Status | Notes |
|-----------|--------|-------|
| Template exists | PASS | `.deploy/releases/TEMPLATE.manifest.json` |
| Schema validation | PASS | `$schema` references manifest.schema.json |
| Required fields | PASS | version, release_version, description, changes |
| Phase E types defined | **FAIL** | No `grafana-dashboard` or `sql-view` types |

### Declaration Types Gap Analysis

| Phase E Component | Required Declaration Type | Current Status | Gap |
|-------------------|---------------------------|----------------|-----|
| Threshold crossings view | `sql-view` or `gold-tables` | Not defined | **Critical** |
| Unified events view | `sql-view` or `domain` extension | Partial | **Medium** |
| Events hourly aggregate | `gold-tables` extension | Partial | **Medium** |
| Gold Layer Dashboard | `grafana-dashboard` | **Not defined** | **Critical** |
| Datasource config | Existing (timescaledb.yaml) | PASS | None |

### Currently Supported Declaration Types (from deploy.sh)

```
etcd-config, dimensions, silver-tables, streams, dashboards,
gold-tables, domains, migrations
```

**Missing for Phase E:**
- `grafana-dashboard` - New type needed
- `sql-view` - For non-continuous-aggregate views (events_unified is a regular view)

---

## Grafana Dashboard Provisioning Assessment

### Current State

| Component | Status | Location |
|-----------|--------|----------|
| Provisioning config | EXISTS | `config/grafana/provisioning/dashboards/dashboards.yaml` |
| Dashboard folder path | Configured | `/var/lib/grafana/dashboards` |
| TimescaleDB datasource | EXISTS | `config/grafana/provisioning/datasources/timescaledb.yaml` |
| Datasource UID | `timescaledb-silver` | Matches spec requirement |
| Existing dashboards | 4 | forecast-accuracy, indoor-environment, personal-weather-forecast, pipeline-health |

### Spec vs Reality Comparison

| SPEC-E03 Requirement | Current Implementation | Gap |
|----------------------|------------------------|-----|
| Dashboard JSON at `config/grafana/dashboards/gold-layer-overview.json` | Does not exist | **Create** |
| Datasource name `NDP-TimescaleDB` | Actually `timescaledb-silver` | **Update spec or config** |
| Provisioning via `config/grafana/provisioning/dashboards/ndp.yaml` | Uses `dashboards.yaml` | **Naming inconsistency** |
| `handle_grafana_dashboard()` in deploy.sh | Does not exist | **Create** |
| Manifest entry for dashboard | Type not supported | **Add type** |

### Grafana Provisioning Architecture

```
SPEC-E03 Expected:                    ACTUAL (Current):
====================                  ==================
config/grafana/                       config/grafana/
  provisioning/                         provisioning/
    dashboards/                           dashboards/
      ndp.yaml              -->           dashboards.yaml
    datasources/                          datasources/
      ndp-timescaledb.yaml  -->           timescaledb.yaml
  dashboards/                           dashboards/
    gold-layer-overview.json              (4 existing dashboards)
```

**Decision Required:** Align spec with existing naming conventions or migrate to spec conventions.

---

## Rollback Procedures

### Current Rollback Support

| Aspect | Status | Notes |
|--------|--------|-------|
| Quick rollback documented | PASS | RELEASE-POLICY.md section |
| Previous manifest deploy | PASS | `./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json` |
| Git tag rollback | PASS | `git checkout vX.Y.Z` |
| Database migration rollback | WARN | Manual reverse migration required |
| Grafana dashboard rollback | **FAIL** | Not addressed |

### Phase E Specific Rollback Risks

| Component | Rollback Risk | Mitigation |
|-----------|---------------|------------|
| events_unified view | Low | DROP VIEW is safe |
| threshold_crossings view | Low | DROP VIEW is safe |
| events_hourly CA | Medium | DROP MATERIALIZED VIEW loses data |
| Gold Layer Dashboard | Low | Grafana can re-import |
| Datasource changes | Low | No changes planned |

**Recommendation:** Add note to rollback procedure that events_hourly data will be lost if CA is dropped; data can be regenerated by refresh policy.

---

## Pi-Specific Considerations

### Resource Limits

| Resource | Budget (from AC-E-PERF-03) | Assessed |
|----------|----------------------------|----------|
| events_unified | < 30 MB (30 days) | TBD - requires production data |
| events_hourly | < 10 MB (30 days) | TBD - requires production data |
| Total Phase E storage | < 50 MB | TBD |
| Dashboard load time | < 3 seconds | TBD |
| Query performance | < 100ms (30-day range) | TBD |

### Pi Deployment Checklist (Not in Current Docs)

| Check | Status | Notes |
|-------|--------|-------|
| Docker memory limit | Not specified | Consider adding to deploy instructions |
| CPU throttling handling | Not documented | Pi 5 should handle, but no monitoring |
| Network timeout handling | Not specified | Grafana queries may timeout on slow refresh |
| Storage growth projection | Not documented | Estimate needed for long-term planning |

---

## Health Checks Post-Deployment

### Current Health Checks in deploy.sh

```bash
echo "  Grafana:     $(curl -s -o /dev/null -w '%{http_code}' http://localhost:3000/api/health 2>/dev/null || echo 'Not running')"
```

### Missing Phase E Health Checks

| Check | Currently Implemented | Recommended |
|-------|----------------------|-------------|
| events_unified view exists | No | Add query check |
| events_hourly CA refresh status | No | Add job status check |
| Dashboard loads successfully | No | Add Grafana API check |
| Threshold crossings generating | No | Add row count check |

### Recommended Post-Deploy Verification Script

```bash
# Phase E verification (to be added to deploy.sh)
verify_phase_e() {
    log "Verifying Phase E deployment..."

    # Check events_unified view
    if dcx timescaledb psql -U postgres -d ndp -c "SELECT 1 FROM gold.events_unified LIMIT 1" >/dev/null 2>&1; then
        log "  events_unified: OK"
    else
        error "  events_unified: FAILED"
    fi

    # Check events_hourly CA
    if dcx timescaledb psql -U postgres -d ndp -c "SELECT 1 FROM gold.events_hourly LIMIT 1" >/dev/null 2>&1; then
        log "  events_hourly: OK"
    else
        error "  events_hourly: FAILED"
    fi

    # Check dashboard exists
    if curl -sf "http://localhost:3000/api/dashboards/uid/gold-layer-overview" -H "Authorization: Bearer ${GRAFANA_API_KEY}" >/dev/null 2>&1; then
        log "  Gold Layer Dashboard: OK"
    else
        warn "  Gold Layer Dashboard: Not found (manual verification required)"
    fi
}
```

---

## Missing Artifacts

### Critical (Blocking Deployment)

| Artifact | Location | Owner | Notes |
|----------|----------|-------|-------|
| `grafana-dashboard` declaration type | deploy.sh, manifest.schema.json | ndp-rust-dev | Must be implemented |
| `handle_grafana_dashboard()` function | deploy.sh | ndp-rust-dev | SPEC-E03 provides template |
| `gold-layer-overview.json` dashboard | config/grafana/dashboards/ | ndp-grafana-dev | Must be created |
| v1.2.0.manifest.json | .deploy/releases/ | ndp-scrum-master | Create when ready |

### Medium Priority (Should Have)

| Artifact | Location | Notes |
|----------|----------|-------|
| Post-deployment verification | deploy.sh | Add verify_phase_e() function |
| Grafana API key documentation | deploy/pi/README.md | Required for dashboard API calls |
| Dashboard import error handling | deploy.sh | Graceful failure on import error |

### Low Priority (Nice to Have)

| Artifact | Location | Notes |
|----------|----------|-------|
| Dashboard screenshot in docs | product/features/fe-001/phase-e/ | Visual verification aid |
| Grafana alert integration | Future | For crossing frequency monitoring |

---

## Deployment Flow Validation

### Execution Order Compliance

Per DEPLOYMENT-DECLARATIVES.md, the 12-phase execution order:

| Phase | Declaration | Phase E Impact |
|-------|-------------|----------------|
| 1 | Validation | - |
| 2 | Container builds | - |
| 2.5 | Tool builds | ndp-gold-ddl (already built) |
| 3 | Migrations | - |
| 4 | Silver tables | - |
| 5 | Gold tables | events_hourly CA (new) |
| 6 | Domains | events_unified, threshold_crossings (via domain) |
| 7 | Streams | - |
| 8 | Dimensions | - |
| 9 | Dictionary | - |
| 10 | Container restarts | grafana (to pick up dashboard) |
| 11 | Device state | Update deployed-version |

**Gap:** Grafana dashboard deployment not in execution order. Should be Phase 10.5 or integrated into Phase 10.

### Proposed Manifest for Phase E

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.2.0",
  "description": "Release v1.2.0: Unified Event Abstraction - threshold crossings, unified events view, Gold Layer Dashboard",
  "changes": [
    {
      "type": "tool",
      "id": "ndp-gold-ddl",
      "action": "build",
      "profile": "release",
      "note": "Adds threshold crossing and events generators"
    },
    {
      "type": "domain",
      "domain_id": "indoor-air-quality",
      "action": "sync",
      "note": "Generates threshold_crossings, events_unified, events_hourly"
    },
    {
      "type": "grafana-dashboard",
      "id": "gold-layer-overview",
      "path": "config/grafana/dashboards/gold-layer-overview.json",
      "action": "import",
      "note": "REQUIRES: handle_grafana_dashboard() implementation"
    },
    {
      "type": "container",
      "target": "grafana",
      "action": "restart",
      "note": "Pick up new dashboard via provisioning"
    },
    {
      "type": "dictionary",
      "action": "sync"
    }
  ]
}
```

---

## Recommendations

### Before Phase E Implementation

1. **Implement `grafana-dashboard` declaration type**
   - Add to deploy.sh `handle_grafana_dashboard()` function
   - Update manifest.schema.json to include type
   - Use Grafana HTTP API for import (as spec suggests)

2. **Align datasource naming**
   - Option A: Update SPEC-E03 to use `timescaledb-silver` (existing UID)
   - Option B: Migrate to `NDP-TimescaleDB` (spec name)
   - **Recommendation:** Option A (less risk)

3. **Create dashboard JSON**
   - Follow SPEC-E03 panel specifications
   - Use `timescaledb-silver` as datasource UID
   - Include all 4 rows as specified

### During Phase E Implementation

4. **Extend ndp-gold-ddl for events**
   - Threshold crossing generator (v11-012)
   - Unified events view generator (v11-013)
   - Events hourly CA generator

5. **Add post-deployment verification**
   - Add verify_phase_e() to deploy.sh
   - Check all Phase E objects exist
   - Report row counts

### After Phase E Deployment

6. **Update CHANGELOG.md**
   - Add v1.2.0 section
   - Document all Phase E features

7. **Tag release**
   - `git tag -a v1.2.0 -m "Release v1.2.0: Unified Event Abstraction"`

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Dashboard import fails | Medium | Low | Grafana provisioning fallback |
| Events view performance | Low | Medium | Indexes defined in spec |
| Missing declaration type blocks deploy | **High** | **High** | Implement before Phase E |
| Datasource name mismatch | Medium | Low | Update spec or config |
| Pi resource limits exceeded | Low | Medium | Monitor during deployment |

---

## Deployment Readiness Checklist

### Pre-Implementation (Score: 2/5)

- [x] RELEASE-POLICY.md exists and is comprehensive
- [x] DEPLOYMENT-DECLARATIVES.md documents declaration types
- [ ] `grafana-dashboard` declaration type implemented
- [ ] `gold-layer-overview.json` dashboard file exists
- [ ] v1.2.0.manifest.json created

### Infrastructure (Score: 4/5)

- [x] Grafana provisioning configured
- [x] TimescaleDB datasource configured
- [x] Dashboard folder specified
- [x] Existing dashboards work as reference
- [ ] `handle_grafana_dashboard()` implemented

### Documentation (Score: 4/5)

- [x] Phase E specifications complete (SPEC-E01, E02, E03)
- [x] Acceptance criteria defined
- [x] Deployment section in SPEC-E03
- [x] CONFIG-DEPLOYMENT-FLOW.md documents integration points
- [ ] Grafana API key documentation

### Rollback (Score: 3/5)

- [x] General rollback procedure documented
- [x] Previous manifest deployment supported
- [x] Git tag rollback documented
- [ ] Phase E specific rollback notes
- [ ] Dashboard rollback procedure

---

## Conclusion

Phase E specifications are comprehensive and well-documented. The primary gap is **deployment infrastructure** - specifically the `grafana-dashboard` declaration type and associated handler function. This is a blocking issue that must be resolved before Phase E can be deployed following NDP standards.

**Recommended Action:**
1. Implement `handle_grafana_dashboard()` in deploy.sh (estimated: 1-2 hours)
2. Create dashboard JSON file following SPEC-E03 (estimated: 2-4 hours)
3. Proceed with Phase E implementation
4. Release as v1.2.0

---

*Review completed: 2026-02-05 by ndp-scrum-master*
