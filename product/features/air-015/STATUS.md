# AIR-015: Configuration Lifecycle & Development Workflow Maturity - Status

> **Last Updated:** 2026-02-04
> **Current Phase:** Scope Defined
> **Overall Progress:** 5%

---

## Philosophy

**Small iterative changes, deployed frequently, issues isolated quickly.**

This feature adds guardrails without sacrificing velocity.

---

## Deliverable Status

| ID | Deliverable | Priority | Status | Progress |
|----|-------------|----------|--------|----------|
| D1 | Directory Cleanup | High | Not Started | 0% |
| D2 | Overlay Integration (deploy.sh) | High | Not Started | 0% |
| D3 | Local Validation Script | High | Not Started | 0% |
| D4 | GitHub PR Workflow | Medium | Not Started | 0% |
| D5 | CI Config Validation | Medium | Not Started | 0% |
| D6 | Developer Documentation | Medium | Not Started | 0% |
| D7 | Fixture Isolation | Low | Not Started | 0% |
| D8 | Legacy Config Cleanup | Low | Opportunistic | - |

---

## Checklist

### D1: Directory Cleanup
- [ ] Remove `config/duckdb/`
- [ ] Create `config/overlays/integration/overrides.yaml`
- [ ] Create `config/README.md`

### D2: Overlay Integration
- [ ] Add `--environment` flag to deploy.sh
- [ ] Implement overlay merge (yq or jq-based)
- [ ] Add validation before etcd sync
- [ ] Update etcd sync for `config/domains/`
- [ ] Document overlay precedence

### D3: Local Validation Script
- [ ] Create `scripts/validate.sh`
- [ ] Include: cargo check, cargo test, config validation
- [ ] Document git hook installation
- [ ] Add `make validate` target

### D4: GitHub PR Workflow
- [ ] Create `.github/pull_request_template.md`
- [ ] Document branch naming in CONTRIBUTING.md or docs
- [ ] Create `.github/workflows/pr-validate.yml`

### D5: CI Config Validation
- [ ] Add config validation to PR workflow
- [ ] Trigger on `config/**` changes
- [ ] Fail PR on validation errors

### D6: Developer Documentation
- [ ] Create `docs/procedures/CONFIG-LIFECYCLE.md`
- [ ] Create `docs/procedures/GIT-WORKFLOW.md`
- [ ] Update CLAUDE.md development workflow section

### D7: Fixture Isolation
- [ ] Audit test imports
- [ ] Create fixture loading helper
- [ ] Update `tests/fixtures/README.md`

### D8: Legacy Cleanup (Opportunistic)
- [ ] Document `enabled: false` pattern
- [ ] Fix violations during related work

---

## Recent Activity

| Date | Activity | Outcome |
|------|----------|---------|
| 2026-02-04 | Expanded scope | Added git workflow, local CI, deployment impacts |
| 2026-02-04 | Created SCOPE.md | Initial problem and solution documented |

---

## Key Decisions

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-04 | Feature branches + PR for CI | Validation gates without blocking velocity |
| 2026-02-04 | Local validation is opt-in | Developer choice, CI enforces |
| 2026-02-04 | Direct commits still allowed | Trivial fixes shouldn't require branches |
| 2026-02-04 | FE-001 remediation is opportunistic | Fix forward, don't stop progress |
| 2026-02-04 | Default environment is `production` | Backward compatible |

---

## Blockers

| Blocker | Impact | Owner | Resolution |
|---------|--------|-------|------------|
| None | - | - | - |

---

## Next Actions

1. [ ] Review expanded SCOPE.md
2. [ ] Decide on implementation order
3. [ ] Start with D1 + D3 (quick wins)
