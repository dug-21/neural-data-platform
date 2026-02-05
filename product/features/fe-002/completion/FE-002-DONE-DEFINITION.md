# FE-002: Domain Configuration Standardization - Definition of Done

> **Feature:** FE-002 Domain Configuration Standardization
> **Version:** 1.0
> **Created:** 2026-02-05
> **Last Updated:** 2026-02-05

---

## Executive Summary

This document defines the complete acceptance criteria for FE-002 Domain Configuration Standardization. The feature is considered DONE when both phases pass their acceptance criteria, the architecture gaps are resolved, and GitHub issues are closed.

**Primary Success Metrics:**
1. Domain configs use JSON format (ADR-016-001 compliance)
2. Domain validation shows clear errors with JSONPath locations (dp-019 compliance)
3. Zero functional regression (DDL output unchanged)

---

## Definition of Done Checklist

### 1. All Phases Complete

| Phase | Description | Criteria Document | Status |
|-------|-------------|-------------------|--------|
| Phase A | YAML to JSON Migration | [ACCEPTANCE-CRITERIA.md](./ACCEPTANCE-CRITERIA.md#phase-a-yaml-to-json-migration-gap-001) | [ ] |
| Phase B | Schema Validation Integration | [ACCEPTANCE-CRITERIA.md](./ACCEPTANCE-CRITERIA.md#phase-b-schema-validation-integration-gap-003) | [ ] |

---

### 2. Code Complete Criteria

| Criterion | Target | Verification | Status |
|-----------|--------|--------------|--------|
| **domain.json created** | File exists, valid JSON | `jq . config/domains/indoor-air-quality/domain.json` | [ ] |
| **domain.yaml deleted** | File removed | `ls domain.yaml` returns error | [ ] |
| **loader.rs updated** | Uses serde_json | `grep serde_json loader.rs` | [ ] |
| **serde_yaml removed** | Not in Cargo.toml | `grep serde_yaml Cargo.toml` empty | [ ] |
| **--domain CLI flag** | Flag implemented | `ndp-validate --help` shows flag | [ ] |
| **Layer 1 validation** | Schema validation works | Invalid schema produces error | [ ] |
| **Layer 2 validation** | Semantic validation works | Invalid references produce error | [ ] |
| **deploy.sh integration** | Validates before deploy | Validation in Phase 1 | [ ] |

---

### 3. Test Complete Criteria

| Component | Target Coverage | Test Count | Status |
|-----------|-----------------|------------|--------|
| ndp-gold-ddl (existing) | Maintained | No regression | [ ] |
| ndp-validate Layer 1 | > 90% | 10+ new tests | [ ] |
| ndp-validate Layer 2 | > 90% | 10+ new tests | [ ] |
| ndp-validate CLI | > 80% | 5+ new tests | [ ] |
| Error formatting | > 80% | 5+ new tests | [ ] |
| **Total new tests** | - | >= 30 | [ ] |

**Verification Commands:**
```bash
# Run all tests
cargo test -p ndp-gold-ddl
cargo test -p ndp-validate

# Check test counts
cargo test -p ndp-validate 2>&1 | grep "test result"
```

---

### 4. Architecture Success Criteria (CRITICAL)

| Criterion | Target | Verification | Status |
|-----------|--------|--------------|--------|
| **Zero functional regression** | DDL identical to baseline | Golden master comparison | [ ] |
| **ADR-016-001 compliance** | JSON as source of truth | No YAML domain configs | [ ] |
| **dp-019 compliance** | Two-layer validation | Layer 1 + Layer 2 for domains | [ ] |

**CRITICAL GATE:** If golden master comparison fails, FE-002 is NOT DONE.

**Verification:**
```bash
# Pre-migration: capture baseline
ndp-gold-ddl generate --domain indoor-air-quality > baseline.sql

# Post-migration: compare
ndp-gold-ddl generate --domain indoor-air-quality > new.sql
diff baseline.sql new.sql  # MUST be empty
```

---

### 5. Documentation Complete Criteria

| Document | Location | Status |
|----------|----------|--------|
| SCOPE.md | product/features/fe-002/SCOPE.md | [x] |
| STATUS.md (marked "done") | product/features/fe-002/STATUS.md | [ ] |
| ACCEPTANCE-CRITERIA.md | product/features/fe-002/completion/ | [ ] |
| VERIFICATION-PROCEDURE.md | product/features/fe-002/completion/ | [ ] |
| FE-002-DONE-DEFINITION.md | product/features/fe-002/completion/ | [ ] |
| RELEASE-CHECKLIST.md | product/features/fe-002/completion/ | [ ] |

---

### 6. Review Complete Criteria

| Review | Reviewer | Date | Status |
|--------|----------|------|--------|
| Code review (Phase A) | | | [ ] |
| Code review (Phase B) | | | [ ] |
| Test review | | | [ ] |
| Documentation review | | | [ ] |

---

### 7. GitHub Issues Resolved

| Issue | Title | Status |
|-------|-------|--------|
| [#11](https://github.com/dug-21/neural-data-platform/issues/11) | GAP-001: Domain config uses YAML instead of JSON | [ ] Closed |
| [#13](https://github.com/dug-21/neural-data-platform/issues/13) | GAP-003: No JSON Schema validation for domain configs | [ ] Closed |

**Closure Requirements:**
- Link to implementing PR
- Reference verification procedure results
- Confirm acceptance criteria met

---

### 8. Deployment Criteria

| Criterion | Verification | Status |
|-----------|--------------|--------|
| **Dry-run passes** | `deploy.sh apply --dry-run` succeeds | [ ] |
| **Local deployment** | Config deploys to local environment | [ ] |
| **Validation integrated** | deploy.sh validates domains | [ ] |

---

### 9. Release Criteria

| Artifact | Location | Status |
|----------|----------|--------|
| Manifest file | `.deploy/releases/v1.X.Y.manifest.json` | [ ] |
| CHANGELOG entry | CHANGELOG.md | [ ] |
| Git tag | `vX.Y.Z` | [ ] |

See [RELEASE-CHECKLIST.md](./RELEASE-CHECKLIST.md) for details.

---

### 10. Learning/Feedback Criteria

| Requirement | Verification | Status |
|-------------|--------------|--------|
| Participating agents recorded reflexion | AgentDB skill | [ ] |
| Patterns stored | `save-pattern` skill used | [ ] |
| Lessons documented | In completion docs | [ ] |

---

## Final Acceptance Checklist

### Pre-Completion Gate (Phase A)

- [ ] domain.json exists and is valid JSON
- [ ] domain.yaml deleted
- [ ] loader.rs uses serde_json
- [ ] No serde_yaml references in ndp-gold-ddl
- [ ] All existing tests pass
- [ ] **CRITICAL:** Golden master DDL comparison passes

### Pre-Completion Gate (Phase B)

- [ ] `ndp-validate --domain` flag works
- [ ] Layer 1 errors show JSONPath locations
- [ ] Layer 2 semantic validation runs
- [ ] Clear, actionable error messages
- [ ] deploy.sh validates domains
- [ ] 30+ new tests pass

### Functional Completeness

- [ ] Domain configs in JSON format
- [ ] Two-layer validation for domains
- [ ] IDE autocomplete works (JSON Schema)
- [ ] Error messages are developer-friendly

### Quality Gates

- [ ] Zero regressions in existing tests
- [ ] DDL output unchanged from baseline
- [ ] Code review approved
- [ ] Documentation complete

### Release Readiness

- [ ] Release manifest created
- [ ] CHANGELOG updated
- [ ] Version determined (MINOR bump expected)
- [ ] GitHub issues closed

---

## Sign-Off

### Phase Sign-Offs

| Phase | Lead Agent | Date | Signature |
|-------|------------|------|-----------|
| Phase A | ndp-rust-dev | | |
| Phase B | ndp-rust-dev | | |
| Testing | ndp-tester | | |
| Documentation | ndp-scrum-master | | |

### Final Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Technical Lead | | | |
| Product Owner | | | |

---

## Post-Completion Actions

After FE-002 is marked DONE:

### 1. Update STATUS.md

```markdown
# Update these fields:
> **Current Phase:** Done
> **Overall Progress:** 100%

## Phase Status
| Phase | Status | Progress |
|-------|--------|----------|
| Phase A | Complete | 100% |
| Phase B | Complete | 100% |
```

### 2. Store Patterns (via save-pattern skill)

Store these patterns for future use:
- YAML to JSON migration procedure
- Two-layer validation wiring pattern
- CLI flag addition pattern for validators
- Golden master testing pattern

### 3. Record Reflexion (via reflexion skill)

All agents should record:
- What worked well in the feature
- What was challenging
- What would improve future similar features

### 4. Close GitHub Issues

Close with references:
```
Resolved by PR #XX

Verification:
- AC-A-001 through AC-A-006: PASS
- AC-B-001 through AC-B-006: PASS
- Golden master comparison: PASS
- All tests passing: PASS

See verification results: product/features/fe-002/completion/VERIFICATION-PROCEDURE.md
```

### 5. Create Release

Follow [RELEASE-CHECKLIST.md](./RELEASE-CHECKLIST.md) for:
- Version determination (expected: MINOR bump)
- Manifest creation
- CHANGELOG entry
- Git tag

---

## Related Documents

### FE-002 Documents
- [SCOPE.md](../SCOPE.md) - Feature scope and requirements
- [STATUS.md](../STATUS.md) - Current progress tracking
- [ACCEPTANCE-CRITERIA.md](./ACCEPTANCE-CRITERIA.md) - Detailed acceptance criteria
- [VERIFICATION-PROCEDURE.md](./VERIFICATION-PROCEDURE.md) - Step-by-step verification
- [RELEASE-CHECKLIST.md](./RELEASE-CHECKLIST.md) - Release preparation

### NDP Documents
- [Release Policy](../../../../docs/procedures/RELEASE-POLICY.md)
- [Deployment Declaratives](../../../../docs/procedures/DEPLOYMENT-DECLARATIVES.md)
- [ADR-016-001](../../../../product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md) - JSON source of truth

### Reference Documents
- [FE-001-DONE-DEFINITION.md](../../fe-001/completion/FE-001-DONE-DEFINITION.md) - Pattern reference
- [FE-001 VALIDATION-PROCEDURE.md](../../fe-001/phase-d/refinement/VALIDATION-PROCEDURE.md) - Verification patterns

---

*Definition of Done created: 2026-02-05 by ndp-scrum-master (SPARC Completion)*
