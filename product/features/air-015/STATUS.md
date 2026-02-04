# AIR-015: Configuration Lifecycle & Development Workflow Maturity - Status

> **Last Updated:** 2026-02-04
> **Current Phase:** Implementation (High Priority Complete)
> **Overall Progress:** 45%

---

## Philosophy

**Small iterative changes, deployed frequently, issues isolated quickly.**

This feature adds guardrails without sacrificing velocity.

---

## Deliverable Status

| ID | Deliverable | Priority | Status | Progress |
|----|-------------|----------|--------|----------|
| D1 | Directory Cleanup | High | **Complete** | 100% |
| D2 | Environment-Based Paths (deploy.sh) | High | **Complete** | 100% |
| D3 | Local Validation Script | High | **Complete** | 100% |
| D4 | GitHub PR Workflow | Medium | Not Started | 0% |
| D5 | CI Config Validation | Medium | Not Started | 0% |
| D6 | Developer Documentation | Medium | Partial | 30% |
| D7 | Fixture Isolation | Low | Not Started | 0% |
| D8 | Legacy Config Cleanup | Low | Opportunistic | - |

---

## Checklist

### D1: Directory Cleanup ✅
- [x] Remove `config/duckdb/` (7 deprecated files removed)
- [x] Create `config/integration/base/streams/` (environment-specific, not overlays)
- [x] Create `config/README.md`

### D2: Environment-Based Paths ✅
- [x] Add `CONFIG_STREAMS_DIR`/`CONFIG_DOMAINS_DIR` based on `DEPLOY_ENV`
- [x] Update `deploy/pi/deploy.sh` to use environment paths
- [x] Update `scripts/sync-streams-to-etcd.sh` to respect `DEPLOY_ENV`
- [x] Auto-select etcd container (`etcd` vs `integration-etcd`)
- [x] Fallback to production if env directory missing

### D3: Local Validation Script ✅
- [x] Create `scripts/validate.sh`
- [x] Include modes: --quick, --config-only, --full
- [x] Document git hook installation in script header
- [ ] Add `make validate` target (deferred)

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
| 2026-02-04 | **D1, D2, D3 Implemented** | Commit 767585d - env-based paths working |
| 2026-02-04 | Tested integration env | `DEPLOY_ENV=integration` syncs to isolated configs |
| 2026-02-04 | Pattern saved | `configuration:environment-based-paths` (ID: 50) |
| 2026-02-04 | Expanded scope | Added git workflow, local CI, deployment impacts |
| 2026-02-04 | Created SCOPE.md | Initial problem and solution documented |

---

## Key Decisions

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-04 | **Complete files per env, not overlays** | Simpler debugging, no merge logic |
| 2026-02-04 | `config/{env}/base/streams/` structure | Mirrors production, clear isolation |
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

1. [x] ~~Review expanded SCOPE.md~~
2. [x] ~~Decide on implementation order~~
3. [x] ~~Start with D1 + D3 (quick wins)~~
4. [ ] D4: Create PR workflow template
5. [ ] D5: Add config validation to CI
6. [ ] D6: Complete developer documentation
