# FE-001 Phase D: Validation + Dashboard

**Feature**: Gold Layer Foundation - Phase D Validation
**Phase**: Specification (SPARC-S)
**Created**: 2026-02-04
**Status**: Draft

---

## 1. Executive Summary

Phase D is the **critical validation phase** that proves the V1.1 Gold Layer architecture works as designed. The centerpiece is the **Fast-Follower Test**: adding the `outdoor-air-quality` stream to the Gold layer using **only configuration changes** - zero Rust code modifications.

### 1.1 Phase D Success Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| **Fast-Follower Time** | < 1 hour | Timed exercise with checkpoints |
| **Code Changes** | Zero Rust changes | Git diff verification |
| **Architecture Validation** | Config-only stream addition | Fast-follower test pass |
| **Dashboard Load Time** | < 2 seconds | Browser performance measurement |

### 1.2 Why Phase D Matters

Phase D exists to answer one question:

> **Is the Gold Layer architecture truly config-driven and extensible?**

If a developer can add a new stream (outdoor-air-quality) to Gold layer aggregates, aligned view, and dashboard by **only editing JSON/YAML config files**, then V1.1 has achieved its design goal. If code changes are required, the architecture has failed and must be fixed before V1.2.

---

## 2. Phase D Features

### 2.1 Feature Summary

| ID | Feature | Priority | Owner | Dependencies |
|----|---------|----------|-------|--------------|
| v11-V01 | Fast-Follower Stream Test | **Critical** | ndp-tester | v11-A02, v11-A04 |
| v11-V02 | New Feature Type Test | Medium | ndp-tester | v11-A06 |
| v11-008 | Basic Feature Computation | Medium | ndp-rust-dev | v11-A06 |
| v11-009 | Lag Feature Computation | Medium | ndp-rust-dev | v11-A06 |
| v11-010 | Gold Layer Data Dictionary | Medium | ndp-analytics-engineer | v11-003, v11-005 |
| v11-011 | Correlation-Ready Dashboard | High | ndp-grafana-dev | v11-005, v11-007 |

### 2.2 Feature Specification Files

| Feature | Specification File |
|---------|-------------------|
| v11-V01 | [SPEC-D01-fast-follower-test.md](./SPEC-D01-fast-follower-test.md) |
| v11-008/v11-009 | [SPEC-D02-feature-computation.md](./SPEC-D02-feature-computation.md) |
| v11-009 | [SPEC-D03-lag-features.md](./SPEC-D03-lag-features.md) |
| v11-010 | [SPEC-D04-data-dictionary.md](./SPEC-D04-data-dictionary.md) |
| v11-011 | [SPEC-D05-dashboard.md](./SPEC-D05-dashboard.md) |

---

## 3. Dependencies

### 3.1 Phase A-C Prerequisites

Phase D requires all prior phases to be complete:

| Phase | Feature | Required For |
|-------|---------|--------------|
| **Phase A** | v11-A01: Gold ETL JSON Schema | Fast-follower config validation |
| **Phase A** | v11-A02: ndp-gold-ddl tool | DDL generation from config |
| **Phase A** | v11-A03: Alignment JSON Schema | Aligned view config |
| **Phase A** | v11-A04: Alignment Interpreter | Aligned view SQL generation |
| **Phase A** | v11-A05: Objectives JSON Schema | Objectives config |
| **Phase A** | v11-A06: Feature Type Registry | Feature computation |
| **Phase B** | v11-003: air-quality continuous aggregate | Reference implementation |
| **Phase C** | v11-005: Aligned view (3 streams) | Baseline for 4th stream addition |

### 3.2 External Dependencies

| Dependency | Required For | Risk |
|------------|--------------|------|
| outdoor-air-quality Silver table | Fast-follower test | Low (already exists) |
| TimescaleDB continuous aggregates | Gold layer | Low (already using) |
| Grafana 9+ | Dashboard | Low (already deployed) |

---

## 4. Timing Requirements

### 4.1 Fast-Follower Test Timing Budget

The fast-follower test MUST complete in under 1 hour:

| Step | Time Budget | Cumulative |
|------|-------------|------------|
| 1. Read documentation | 10 min | 10 min |
| 2. Create gold_etl config | 15 min | 25 min |
| 3. Update domain config | 10 min | 35 min |
| 4. Create/update manifest | 5 min | 40 min |
| 5. Run deploy.sh apply | 5 min | 45 min |
| 6. Verify in database | 5 min | 50 min |
| 7. Update dashboard (optional) | 10 min | 60 min |

### 4.2 Timing Checkpoints

The fast-follower test procedure includes mandatory timing checkpoints to validate the 1-hour target. See [SPEC-D01-fast-follower-test.md](./SPEC-D01-fast-follower-test.md) for the detailed procedure.

---

## 5. Architecture Validation Checklist

Phase D validates these architectural properties:

### 5.1 Config-Driven Design

- [ ] New stream added via `gold_etl` config section only
- [ ] No changes to `tools/ndp-gold-ddl/` source code
- [ ] No changes to `deploy/pi/deploy.sh`
- [ ] No changes to `core/src/gold/` modules
- [ ] No changes to any `.rs` files

### 5.2 Extensibility

- [ ] Domain config updated with 4th stream
- [ ] Aligned view regenerates with new stream columns
- [ ] Feature computations work for new stream
- [ ] Data dictionary auto-populated for new Gold objects

### 5.3 Consistency

- [ ] outdoor-air-quality follows same patterns as air-quality
- [ ] Naming conventions are consistent
- [ ] NULL handling follows stream_type rules
- [ ] Refresh policies match other streams

---

## 6. Test Strategy

### 6.1 Fast-Follower Test Procedure

The fast-follower test is a **timed exercise** with the following structure:

1. **Pre-Test Setup**: Clean slate verification
2. **Timed Test**: Execute config-only stream addition
3. **Verification**: Confirm Gold layer operational
4. **Post-Test Analysis**: Document findings

### 6.2 Integration Tests

| Test | Purpose | Location |
|------|---------|----------|
| `fast_follower_test.rs` | Automated fast-follower validation | `tools/ndp-gold-ddl/tests/integration/` |
| Dashboard load test | Verify < 2s load time | Manual + CI |
| Data dictionary query | Verify Gold metadata complete | SQL queries |

### 6.3 London TDD Interfaces

Phase D tests verify these interfaces work without modification:

```rust
// These interfaces MUST work for outdoor-air-quality without code changes

// GoldEtlConfig must deserialize new stream config
fn load_gold_etl(stream_id: &str) -> Result<GoldEtlConfig, ConfigError>;

// DDL generator must produce valid SQL for new stream
fn generate_continuous_aggregate(
    config: &GoldEtlConfig,
    stream_id: &str
) -> Result<String, GeneratorError>;

// Alignment generator must include new stream
fn generate_aligned_view(
    domain: &DomainConfig
) -> Result<String, GeneratorError>;
```

---

## 7. Deliverables

### 7.1 Specification Documents

| Document | Description |
|----------|-------------|
| PHASE-D-OVERVIEW.md | This document |
| SPEC-D01-fast-follower-test.md | Critical validation procedure |
| SPEC-D02-feature-computation.md | Basic aggregate features |
| SPEC-D03-lag-features.md | Time-lagged features |
| SPEC-D04-data-dictionary.md | Gold metadata specification |
| SPEC-D05-dashboard.md | Correlation-ready Grafana dashboard |

### 7.2 Test Artifacts

| Artifact | Description |
|----------|-------------|
| `fast-follower-test.manifest.json` | Test manifest for Phase D |
| `FAST-FOLLOWER-REPORT.md` | Timed test results and findings |
| Test fixture configs | outdoor-air-quality gold_etl samples |

### 7.3 Exit Criteria

Phase D is complete when:

- [ ] Fast-follower test passes (< 1 hour, zero code changes)
- [ ] Lag features working for all streams
- [ ] Data dictionary complete for Gold layer
- [ ] Dashboard loads in < 2 seconds
- [ ] All specification documents complete
- [ ] All integration tests pass
- [ ] FAST-FOLLOWER-REPORT.md documented

---

## 8. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Fast-follower test reveals architecture gap | Medium | **Critical** | Stop and fix architecture before Phase E |
| Dashboard performance poor on Pi | Low | Medium | Use continuous aggregate queries only |
| Data dictionary sync incomplete | Low | Low | Manual verification query |
| Feature computation missing edge cases | Low | Medium | Add feature types in Phase E |

### 8.1 Critical Risk: Architecture Gap

If the fast-follower test fails (requires code changes), this is a **critical finding**:

1. **Stop Phase D immediately**
2. Document the gap in FAST-FOLLOWER-REPORT.md
3. Return to Phase A-C to fix architecture
4. Re-run fast-follower test after fix
5. Only proceed to Phase E after successful test

---

## 9. Team Assignments

| Role | Agent | Responsibilities |
|------|-------|------------------|
| **Lead** | ndp-tester | Fast-follower test execution, timing |
| Support | ndp-grafana-dev | Dashboard development |
| Support | ndp-analytics-engineer | Data dictionary, SQL queries |
| Support | ndp-rust-dev | Feature computation (if gaps found) |

---

## 10. References

### 10.1 FE-001 Documents

- [SCOPE.md](../../SCOPE.md) - Full V1.1 scope definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Deployment flow
- [SPARC-COORDINATION.md](../../SPARC-COORDINATION.md) - Phase coordination

### 10.2 Stream Configurations

- `config/base/streams/outdoor-air-quality/config.json` - Target stream for fast-follower
- `config/base/streams/air-quality/config.json` - Reference implementation

### 10.3 External

- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)
- [Grafana Provisioning](https://grafana.com/docs/grafana/latest/administration/provisioning/)

---

*Phase D Specification created: 2026-02-04*
*Next: Individual feature specifications (SPEC-D01 through SPEC-D05)*
