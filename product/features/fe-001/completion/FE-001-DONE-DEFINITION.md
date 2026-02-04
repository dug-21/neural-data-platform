# FE-001: Gold Layer Foundation - Definition of Done

> **Feature:** FE-001 Gold Layer Foundation
> **Version:** 1.0
> **Created:** 2026-02-04
> **Last Updated:** 2026-02-04

---

## Executive Summary

This document defines the complete acceptance criteria for FE-001 Gold Layer Foundation. The feature is considered DONE when all five phases (A-E) pass their acceptance criteria, the architecture is validated as config-driven, and the V1.2 handoff requirements are met.

**Primary Success Metric**: A developer can add a new stream to the Gold layer via CONFIG-ONLY changes in under 1 hour.

---

## Definition of Done Checklist

### 1. All Phases Complete

| Phase | Description | Criteria Document | Status |
|-------|-------------|-------------------|--------|
| Phase A | Architecture Foundation | [phase-a/completion/ACCEPTANCE-CRITERIA.md](../phase-a/completion/ACCEPTANCE-CRITERIA.md) | [ ] |
| Phase B | First Stream (air-quality) | [phase-b/completion/ACCEPTANCE-CRITERIA.md](../phase-b/completion/ACCEPTANCE-CRITERIA.md) | [ ] |
| Phase C | Cross-Stream + Alignment | [phase-c/completion/ACCEPTANCE-CRITERIA.md](../phase-c/completion/ACCEPTANCE-CRITERIA.md) | [ ] |
| Phase D | Validation + Dashboard | [phase-d/completion/ACCEPTANCE-CRITERIA.md](../phase-d/completion/ACCEPTANCE-CRITERIA.md) | [ ] |
| Phase E | Unified Event Abstraction | [phase-e/completion/ACCEPTANCE-CRITERIA.md](../phase-e/completion/ACCEPTANCE-CRITERIA.md) | [ ] |

---

### 2. Architecture Success Criteria (Primary)

| Criterion | Target | Verification | Status |
|-----------|--------|--------------|--------|
| **Extensibility** | Add new stream via config only | Phase D Fast-Follower Test | [ ] |
| **Fast-follower time** | < 1 hour to add stream to Gold | Timed exercise | [ ] |
| **Config-driven** | Zero Rust changes for new stream | Git diff verification | [ ] |

**CRITICAL**: If any architecture criterion fails, FE-001 is NOT DONE regardless of phase completion.

---

### 3. Performance Success Criteria

| Criterion | Target | Verification | Status |
|-----------|--------|--------------|--------|
| Aligned view query (30 days) | < 100ms | `EXPLAIN ANALYZE` on Pi 5 | [ ] |
| Continuous aggregate refresh | < 5% sustained CPU | Pi monitoring | [ ] |
| Peak memory usage | < 200 MB | Pi memory monitoring | [ ] |
| Dashboard load time | < 2 seconds | Browser measurement | [ ] |

**Verification Commands:**
```bash
# Aligned view query performance
docker exec timescaledb psql -U postgres -d ndp -c "
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT * FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '30 days';
"
# Expected: Execution Time < 100ms

# Check storage usage
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    schemaname || '.' || matviewname as view_name,
    pg_size_pretty(pg_total_relation_size(schemaname || '.' || matviewname)) as size
FROM pg_matviews
WHERE schemaname = 'gold'
ORDER BY pg_total_relation_size(schemaname || '.' || matviewname) DESC;
"
# Expected: Total < 100 MB for 30 days
```

---

### 4. Completeness Success Criteria (V1.2 Handoff)

| Criterion | Target | Verification | Status |
|-----------|--------|--------------|--------|
| Stream classification | 100% of streams classified | Config audit | [ ] |
| Gold aggregates | All configured streams | Query timescaledb_information | [ ] |
| Aligned view | All domain streams included | Query column list | [ ] |
| Unified events | State + threshold in single view | Query event_type counts | [ ] |
| Objectives | Queryable via data dictionary | Query data_dictionary.objectives | [ ] |
| Data dictionary | All Gold objects documented | Query data_dictionary.gold_* | [ ] |

**Verification Commands:**
```bash
# Check stream classification completeness
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    s.stream_id,
    sc.stream_type
FROM (SELECT DISTINCT stream_id FROM silver.stream_registry) s
LEFT JOIN data_dictionary.stream_classification sc ON s.stream_id = sc.stream_id
WHERE sc.stream_type IS NULL;
"
# Expected: 0 rows (all streams classified)

# Check Gold aggregates exist
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold'
ORDER BY view_name;
"
# Expected: air_quality_hourly, air_quality_daily, outdoor_weather_hourly,
#           state_events_hourly, outdoor_air_quality_hourly

# Check unified events
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT event_type, COUNT(*) FROM gold.events_unified
GROUP BY event_type;
"
# Expected: Both state_transition and threshold_crossing present
```

---

### 5. Test Coverage Success Criteria

| Component | Target Coverage | Achieved | Status |
|-----------|-----------------|----------|--------|
| generators/continuous_aggregate.rs | > 90% | | [ ] |
| generators/aligned_view.rs | > 85% | | [ ] |
| generators/features.rs | > 80% | | [ ] |
| generators/events.rs | > 80% | | [ ] |
| validation/expressions.rs | > 90% | | [ ] |
| validation/config.rs | > 85% | | [ ] |
| registry/feature_types.rs | > 80% | | [ ] |

**Verification Command:**
```bash
# Generate coverage report
cargo tarpaulin -p ndp-gold-ddl --out Html --output-dir coverage/

# Check coverage summary
cargo tarpaulin -p ndp-gold-ddl --out Stdout | grep "Coverage:"
# Expected: Coverage > 80% overall
```

---

### 6. Documentation Success Criteria

| Document | Location | Status |
|----------|----------|--------|
| SCOPE.md | product/features/fe-001/SCOPE.md | [ ] |
| STATUS.md (marked "done") | product/features/fe-001/STATUS.md | [ ] |
| DECISIONS.md | product/features/fe-001/architecture/DECISIONS.md | [ ] |
| SPARC-COORDINATION.md | product/features/fe-001/SPARC-COORDINATION.md | [ ] |
| TESTING-STRATEGY.md | product/features/fe-001/TESTING-STRATEGY.md | [ ] |
| Phase A-E Specifications | product/features/fe-001/phase-*/specification/ | [ ] |
| Phase A-E Acceptance Criteria | product/features/fe-001/phase-*/completion/ | [ ] |
| Fast-Follower Report | product/features/fe-001/phase-d/refinement/FAST-FOLLOWER-REPORT.md | [ ] |

---

### 7. Integration Success Criteria

| Integration Point | Verification | Status |
|-------------------|--------------|--------|
| deploy.sh handles gold-table declarations | Test manifest deployment | [ ] |
| deploy.sh handles domain declarations | Test domain deployment | [ ] |
| ndp-validate includes Gold validation | Test validation errors | [ ] |
| Grafana dashboards provisioned | Dashboard loads | [ ] |
| Data dictionary auto-populated | Query data dictionary | [ ] |
| etcd config sync works | Config deployment test | [ ] |

---

### 8. Learning/Feedback Success Criteria

| Requirement | Verification | Status |
|-------------|--------------|--------|
| All participating agents recorded reflexion | Check AgentDB | [ ] |
| Patterns stored in AgentDB | Query pattern count | [ ] |
| Architecture patterns documented | Check docs/architecture | [ ] |
| Testing patterns documented | Check docs/testing | [ ] |

**Verification Commands:**
```bash
# Check reflexion records (via AgentDB skill)
# /get-pattern domain="fe-001"

# Check patterns stored
# /get-pattern domain="gold-layer"
```

---

## Final Acceptance Checklist

### Architecture Validation (CRITICAL)

- [ ] **AC-D-01 PASSED**: Fast-follower test completed in < 1 hour
- [ ] **AC-D-02 PASSED**: Git diff shows config-only changes
- [ ] No modifications to `tools/ndp-gold-ddl/src/` during fast-follower
- [ ] No modifications to `deploy/pi/deploy.sh` during fast-follower
- [ ] No modifications to `core/` modules during fast-follower

### Functional Completeness

- [ ] 4+ streams in Gold layer (air-quality, outdoor-weather, home-assistant-state, outdoor-air-quality)
- [ ] Aligned view operational with all domain streams
- [ ] Continuous aggregates auto-refreshing
- [ ] State transitions extractable
- [ ] Threshold crossings generating
- [ ] Unified events view operational
- [ ] Data dictionary complete

### Performance Validation

- [ ] 30-day aligned view query < 100ms on Pi 5
- [ ] Refresh policy CPU < 5% sustained
- [ ] Dashboard loads < 2 seconds
- [ ] Total Gold storage < 100 MB for 30 days

### V1.2 Handoff Readiness

- [ ] Event schema contract documented and frozen
- [ ] V1.2 query patterns validated
- [ ] V1.2 team confirms Gold layer meets requirements
- [ ] Monitoring requirements documented (for deferred hysteresis decision)

---

## Sign-Off

### Phase Sign-Offs

| Phase | Lead Agent | Date | Signature |
|-------|------------|------|-----------|
| Phase A | ndp-architect | | |
| Phase B | ndp-rust-dev | | |
| Phase C | ndp-analytics-engineer | | |
| Phase D | ndp-tester | | |
| Phase E | ndp-rust-dev | | |

### Final Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Technical Lead | | | |
| Product Owner | | | |
| V1.2 Representative | | | |

---

## Post-Completion Actions

After FE-001 is marked DONE:

1. **Update STATUS.md**
   ```bash
   # Set status to "done"
   # Record completion date
   # List final deliverables
   ```

2. **Store Patterns** (via save-pattern skill)
   - Gold ETL configuration pattern
   - Continuous aggregate generation pattern
   - Aligned view generation pattern
   - Fast-follower workflow pattern

3. **Record Reflexion** (via reflexion skill)
   - All agents record what worked/didn't work
   - Document lessons learned
   - Note improvements for future features

4. **Archive Test Manifests**
   ```bash
   mv .deploy/test/phase-*.manifest.json .deploy/archive/fe-001/
   ```

5. **Notify V1.2 Team**
   - Gold layer ready for pattern detection
   - Event schema frozen
   - Handoff documentation complete

---

## Related Documents

### FE-001 Documents
- [SCOPE.md](../SCOPE.md) - Full V1.1 scope definition
- [STATUS.md](../STATUS.md) - Current progress tracking
- [DECISIONS.md](../architecture/DECISIONS.md) - Architecture decisions
- [SPARC-COORDINATION.md](../SPARC-COORDINATION.md) - Phase coordination
- [TESTING-STRATEGY.md](../TESTING-STRATEGY.md) - London TDD approach

### Phase Completion Documents
- [Phase A Acceptance Criteria](../phase-a/completion/ACCEPTANCE-CRITERIA.md)
- [Phase B Acceptance Criteria](../phase-b/completion/ACCEPTANCE-CRITERIA.md)
- [Phase C Acceptance Criteria](../phase-c/completion/ACCEPTANCE-CRITERIA.md)
- [Phase D Acceptance Criteria](../phase-d/completion/ACCEPTANCE-CRITERIA.md)
- [Phase E Acceptance Criteria](../phase-e/completion/ACCEPTANCE-CRITERIA.md)

### NDP Documents
- [Release Policy](../../../docs/procedures/RELEASE-POLICY.md)
- [Deployment Declaratives](../../../docs/procedures/DEPLOYMENT-DECLARATIVES.md)

---

*Definition of Done created: 2026-02-04 by ndp-tester*
