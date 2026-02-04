# AIR-015: Configuration Lifecycle & Development Workflow Maturity

> **Created:** 2026-02-04
> **Priority:** High (Foundational)
> **Type:** Platform Infrastructure / Developer Experience
> **Dependencies:** None (foundational)

---

## Philosophy

**Small iterative changes, deployed frequently, issues isolated quickly.**

This feature elevates platform maturity without sacrificing development velocity. The goal is NOT to slow down with heavyweight processes, but to add guardrails that:
- Catch issues before they reach production
- Make it easier to identify what changed when issues arise
- Preserve the ability to deploy almost immediately
- Scale as the platform and team grow

---

## Problem Statement

The Neural Data Platform is a **config-driven platform** where configurations define streams, schemas, domains, dashboards, and deployment behavior. However, the current development workflow has several maturity gaps:

### Configuration Management Issues

1. **Blurred boundaries between environments** - Development modifies production configs (`config/base/`) for testing
2. **Inconsistent directory semantics** - Different directories have different implied environments:
   - `config/base/` → Production (deployed to Pi)
   - `config/domains/` → Development/Integration (not deployed)
   - `config/grafana/` → Production (used by Grafana)
   - `config/overlays/` → Exists but underutilized
   - `config/schemas/` → Platform schemas (environment-agnostic)
   - `config/duckdb/` → Deprecated (should be removed)
3. **No clear promotion path** - No defined lifecycle for configs from development → integration → production
4. **Test pollution** - Development adds experimental sections to production configs
5. **Overlay system exists but isn't used** - `config/overlays/{development,production}/` has content but isn't integrated into deployment

### Git Workflow Issues

6. **Direct commits to main** - All changes go directly to main branch without review gates
7. **No PR-based workflow** - No opportunity for validation before merge
8. **Harder to isolate issues** - When production breaks, harder to identify which commit caused it
9. **No local CI** - Developers don't have standard pre-commit checks to catch issues early

### Deployment Script Dependencies

10. **etcd sync paths hardcoded** - `deploy.sh` syncs specific config paths to etcd; changing directory structure requires script updates
11. **No validation before sync** - Configs are synced to etcd without validation

---

## Current State Analysis

### Directory Audit

| Directory | Current State | Implied Environment | Used By |
|-----------|---------------|---------------------|---------|
| `config/base/` | Active | Production | Pi deployment, deploy.sh |
| `config/domains/` | New (FE-001) | Development | Not deployed yet |
| `config/grafana/` | Active | Production | Grafana provisioning |
| `config/overlays/development/` | Exists | Development | Unknown/unused |
| `config/overlays/production/` | Exists | Production | Unknown/unused |
| `config/schemas/` | Active | All | Validation tools |
| `config/samples/` | Active | None (examples) | Documentation |
| `config/duckdb/` | Deprecated | None | Should be removed |

### Pain Points

1. **FE-001 added `gold_etl` to `config/base/streams/air-quality/config.json`** - This is production config that now has an undeployed feature section
2. **Test fixtures duplicate config patterns** - `tests/fixtures/configs/` has configs that mirror `config/base/` structure
3. **No validation gate** - Configs can be deployed without passing schema + semantic validation
4. **No clear "this is ready for production" signal** - Developers don't know when a config is experimental vs stable

---

## Proposed Solution

### 1. Clarify Directory Semantics

```
config/
├── base/                    # PRODUCTION: Deployed to Pi, stable only
│   ├── streams/             # Stream definitions (bronze, silver, gold)
│   └── dimensions/          # Dimension tables
│
├── domains/                 # PRODUCTION: Domain configs (Gold layer)
│   └── indoor-air-quality/  # Domain-level configurations
│
├── schemas/                 # PLATFORM: JSON schemas (environment-agnostic)
│   └── *.schema.json        # Validation schemas
│
├── grafana/                 # PRODUCTION: Grafana dashboards/datasources
│   ├── dashboards/
│   ├── datasources/
│   └── provisioning/
│
├── overlays/                # ENVIRONMENT OVERRIDES
│   ├── development/         # Dev environment (Docker Compose local)
│   │   └── overrides.yaml   # Environment-specific settings
│   ├── integration/         # CI/CD testing environment (NEW)
│   │   └── overrides.yaml
│   └── production/          # Pi deployment environment
│       └── overrides.yaml
│
├── samples/                 # DOCUMENTATION: Example configs
│   └── *.example.yaml       # Reference examples only
│
└── [REMOVE: duckdb/]        # DEPRECATED: DuckDB eliminated from architecture
```

### 2. Introduce Config Lifecycle States

Configs progress through defined states:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   DRAFT     │ ──► │  VALIDATED  │ ──► │  DEPLOYED   │
│ (fixtures)  │     │ (base/)     │     │   (Pi)      │
└─────────────┘     └─────────────┘     └─────────────┘
      │                   │                    │
      ▼                   ▼                    ▼
  Development         CI passes           Manifest applied
  testing only        schema +            on target env
                      semantic
```

**State Definitions:**

| State | Location | Can Deploy? | Validation Required |
|-------|----------|-------------|---------------------|
| Draft | `tests/fixtures/` | No | None (test data) |
| Candidate | PR (modifies `config/base/`) | No | Schema + semantic |
| Validated | `config/base/` (merged to main) | Yes | CI enforced |
| Deployed | Target environment | N/A | Manifest declares |

### 3. Establish Development Procedures

#### Rule 1: Tests Never Modify Production Configs

Tests use `tests/fixtures/` exclusively. Production configs (`config/base/`, `config/domains/`) are read-only during testing.

```rust
// GOOD: Test uses fixture
let config = load_config("tests/fixtures/configs/valid/air_quality.json");

// BAD: Test modifies production config
let config = load_config("config/base/streams/air-quality/config.json");
config.gold_etl = Some(experimental_gold_etl); // NO!
```

#### Rule 2: New Features Use `enabled: false` in Production

When adding a new config section to production, it MUST be disabled by default:

```yaml
# config/base/streams/air-quality/config.yaml
gold_etl:
  enabled: false  # REQUIRED until feature is deployment-ready
  aggregates: ...
```

#### Rule 3: Config Changes Require Validation

All PRs modifying `config/base/` or `config/domains/` MUST:
1. Pass schema validation (`ndp-validate --schema`)
2. Pass semantic validation (`ndp-validate --semantic`)
3. Include a manifest entry or explanation of deployment intent

#### Rule 4: Overlay Usage

Environment-specific overrides go in `config/overlays/{environment}/`:

```yaml
# config/overlays/development/overrides.yaml
logging:
  level: debug
mqtt:
  broker_url: mqtt://localhost:1883

# config/overlays/production/overrides.yaml
logging:
  level: info
mqtt:
  broker_url: mqtt://mosquitto:1883
```

Deploy script merges: `base + overlay` for target environment.

#### Rule 5: Manifest Declares Deployment Intent

The deployment manifest explicitly lists what gets deployed:

```json
{
  "declarations": {
    "streams": [
      { "stream_id": "air-quality", "action": "sync" }
    ],
    "gold-tables": [
      { "stream_id": "air-quality", "action": "sync", "enabled": true }
    ]
  }
}
```

Features with `enabled: false` in config are skipped unless manifest overrides.

---

## 4. Git Branching Strategy

### Current State
- All commits go directly to `main`
- Releases are cut from `main`
- Works for solo development but limits traceability

### Proposed: Lightweight Feature Branch Workflow

```
main (production-ready)
  │
  ├── feature/fe-001-phase-b     # Feature work
  ├── feature/air-015-overlays   # This feature
  ├── fix/mqtt-reconnect         # Bug fixes
  │
  └── ← PR merge (squash or rebase)
```

**Principles:**
- `main` is always deployable
- Feature branches for non-trivial work (>1 commit)
- PRs enable validation before merge
- Squash merge keeps history clean
- Direct commits to main still allowed for trivial fixes (typos, single-line changes)

**Branch Naming:**
```
feature/{feature-id}-{short-description}   # New features
fix/{issue-or-slug}                        # Bug fixes
chore/{description}                        # Maintenance, deps, docs
```

**PR Requirements:**
- [ ] Builds pass (`cargo build --release`)
- [ ] Tests pass (`cargo test`)
- [ ] Config validation passes (if configs changed)
- [ ] Manifest included (if deployment-affecting change)

**NOT Required:**
- Code review approval (single developer workflow)
- Complex branching (no develop, staging branches)
- Long-lived branches (merge within days, not weeks)

---

## 5. Local CI Procedures

### Pre-Commit Validation

Developers should run validation locally before committing. This catches issues before they reach the repository.

**Standard Pre-Commit Checks:**

```bash
# Quick validation script (< 30 seconds)
#!/bin/bash
set -e

echo "=== NDP Pre-Commit Validation ==="

# 1. Rust compilation
echo "[1/4] Checking Rust compilation..."
cargo check --all-targets

# 2. Rust tests
echo "[2/4] Running tests..."
cargo test --quiet

# 3. Config validation (if configs changed)
if git diff --cached --name-only | grep -q "^config/"; then
    echo "[3/4] Validating configs..."
    # Validate any changed config files
    for file in $(git diff --cached --name-only | grep "^config/.*\.\(json\|yaml\)$"); do
        if [ -f "$file" ]; then
            ndp-validate --config "$file" || exit 1
        fi
    done
else
    echo "[3/4] No config changes, skipping validation"
fi

# 4. Clippy (warnings as errors for CI, optional locally)
echo "[4/4] Running clippy..."
cargo clippy --quiet -- -D warnings 2>/dev/null || echo "Clippy warnings (non-blocking)"

echo "=== Validation Complete ==="
```

**Installation Options:**

1. **Manual** - Run `./scripts/validate.sh` before committing
2. **Git Hook** - Install as `.git/hooks/pre-commit` (optional, developer choice)
3. **Alias** - `alias nv='./scripts/validate.sh'` for quick validation

**NOT Enforced Locally:**
- Pre-commit hooks are opt-in (some developers find them annoying)
- CI is the enforcement point, local validation is convenience

---

## 6. Deployment Script Impacts

### Current etcd Sync Paths

`deploy.sh` syncs configs to etcd at specific paths:

```bash
# Current sync operations in deploy.sh
etcdctl put /streams/{stream_id}/config < config/base/streams/{stream_id}/config.json
etcdctl put /dimensions/{dim_id}/config < config/base/dimensions/{dim_id}/config.json
```

### Required Updates

If we reorganize config directories, these paths must be updated:

| Change | Script Impact |
|--------|---------------|
| Add `config/domains/` sync | New etcd path: `/domains/{domain_id}/config` |
| Overlay merge | Load base + overlay before sync |
| Validation before sync | Call `ndp-validate` before `etcdctl put` |

**Migration Strategy:**
1. Add new functionality (overlay merge, validation) without changing paths
2. Existing `config/base/` paths remain stable
3. New `config/domains/` gets new etcd namespace
4. Document any path changes in release notes

---

## Deliverables

### D1: Directory Cleanup (Priority: High)

- [ ] Remove `config/duckdb/` (deprecated)
- [ ] Create `config/overlays/integration/overrides.yaml` for CI testing
- [ ] Create `config/README.md` documenting directory semantics

### D2: Overlay Integration in deploy.sh (Priority: High)

- [ ] Add `--environment` flag to `deploy/pi/deploy.sh`
- [ ] Implement overlay merge logic (base + overlay → effective config)
- [ ] Add validation before etcd sync
- [ ] Document overlay precedence rules
- [ ] Update etcd sync to handle `config/domains/`

### D3: Local Validation Script (Priority: High)

- [ ] Create `scripts/validate.sh` for pre-commit validation
- [ ] Include: cargo check, cargo test, config validation
- [ ] Document installation as optional git hook
- [ ] Add `make validate` target

### D4: GitHub PR Workflow (Priority: Medium)

- [ ] Document branch naming conventions
- [ ] Create PR template (`.github/pull_request_template.md`)
- [ ] Add GitHub Action for PR validation (`.github/workflows/pr-validate.yml`)
- [ ] Configure branch protection for `main` (optional, when ready)

### D5: CI Config Validation (Priority: Medium)

- [ ] Add config validation step to PR workflow
- [ ] Validate schema for changed config files
- [ ] Validate semantic for changed config files
- [ ] Generate validation report as PR comment (nice-to-have)

### D6: Developer Documentation (Priority: Medium)

- [ ] Create `docs/procedures/CONFIG-LIFECYCLE.md`
- [ ] Create `docs/procedures/GIT-WORKFLOW.md`
- [ ] Update `CLAUDE.md` with development workflow rules
- [ ] Add overlay examples to `config/samples/`

### D7: Fixture Isolation (Priority: Low)

- [ ] Audit existing tests for production config access
- [ ] Create test helper for fixture loading
- [ ] Update `tests/fixtures/README.md` with patterns

### D8: Legacy Config Cleanup (Priority: Low - Opportunistic)

- [ ] Document pattern: new features should use `enabled: false`
- [ ] Fix existing violations opportunistically during related work
- [ ] NOT a blocking priority - document and move on

**Note on D8:** The FE-001 config issue (gold_etl in production config) is documented but not prioritized for immediate fix. The goal is to establish correct patterns going forward, not to stop and remediate all past issues.

---

## Acceptance Criteria

### AC-015-01: Directory Structure Clarified
- `config/duckdb/` removed
- `config/overlays/integration/overrides.yaml` exists
- `config/README.md` documents all directories with their environments

### AC-015-02: Overlay Merge Works
- `deploy.sh apply manifest.json --environment production` merges production overlay
- `deploy.sh apply manifest.json --environment development` merges development overlay
- Default environment is `production`
- Validation runs before etcd sync

### AC-015-03: Local Validation Available
- `scripts/validate.sh` exists and is executable
- Script validates: compilation, tests, config changes
- Script completes in < 60 seconds for typical changes
- Documentation explains git hook installation (optional)

### AC-015-04: PR Workflow Documented
- `.github/pull_request_template.md` exists
- Branch naming conventions documented
- PR validation workflow runs on PRs to `main`

### AC-015-05: CI Validates Configs
- PR modifying `config/**` triggers config validation
- Validation includes schema and semantic checks
- PR blocked if validation fails

### AC-015-06: Procedures Documented
- `docs/procedures/CONFIG-LIFECYCLE.md` exists (config states, promotion)
- `docs/procedures/GIT-WORKFLOW.md` exists (branching, PRs)
- `CLAUDE.md` updated with development workflow section

### AC-015-07: Iterative Deployment Preserved
- Small changes can still be deployed quickly
- No heavyweight approval processes added
- Direct-to-main commits still allowed for trivial fixes

---

## Non-Goals

- **Not changing config format** - YAML/JSON formats stay as-is
- **Not adding config versioning** - Git provides history
- **Not building a config UI** - CLI tools are sufficient
- **Not multi-tenancy** - Single deployment target (Pi)
- **Not requiring code review** - Single developer workflow, PR is for CI gates not approval
- **Not blocking velocity** - If process slows deployment, process is wrong
- **Not retroactive remediation** - Fix forward, don't stop to fix all past issues

---

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing deploy workflow | High | Backward-compatible changes, `--environment` defaults to current behavior |
| Developer friction | Medium | All new process is opt-in locally, CI is the enforcement |
| CI slowdown | Low | Validation is fast (<30s), parallelize where possible |
| Forgetting to use branches | Low | Document, but allow direct commits for trivial changes |

---

## Timeline Estimate

| Phase | Deliverables | Effort |
|-------|--------------|--------|
| 1 | D1 (directory cleanup) | 0.5 day |
| 2 | D3 (local validation script) | 0.5 day |
| 3 | D2 (overlay integration in deploy.sh) | 1-2 days |
| 4 | D4 + D5 (PR workflow, CI validation) | 1 day |
| 5 | D6 (documentation) | 0.5 day |
| 6 | D7 + D8 (fixture isolation, legacy cleanup) | Opportunistic |

**Core Work: ~3-4 days**
**Polish/Docs: ~1 day**
**Total: ~4-5 days**

### Suggested Implementation Order

1. **D1 + D3** - Quick wins: cleanup deprecated dir, add validation script
2. **D2** - Overlay integration (most complex, highest value)
3. **D4 + D5** - PR workflow and CI (enables future quality gates)
4. **D6** - Documentation (captures decisions for future reference)
5. **D7 + D8** - Low priority, do opportunistically

---

## Open Questions

1. **Should `config/grafana/` support overlays?** - Grafana dashboards may need env-specific datasource URLs
2. **How to handle secrets?** - Currently in overlays, should they be in a separate mechanism?
3. **Should we version config schemas?** - `gold-etl.schema.json` vs `gold-etl.v1.schema.json`
4. **Branch protection on main?** - Can enable "require PR" but may slow trivial fixes
5. **PR merge strategy?** - Squash (cleaner history) vs rebase (preserves commits) vs merge (preserves structure)

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Branch strategy | Feature branches + PR | Enables CI validation without blocking velocity |
| Local CI | Opt-in (script, not enforced hook) | Developer choice, CI is enforcement point |
| Direct commits to main | Still allowed for trivial fixes | Pragmatic, don't over-engineer |
| FE-001 remediation | Low priority, opportunistic | Fix forward, document pattern |
| Default environment | `production` | Backward compatible with current behavior |

---

## References

- [RELEASE-POLICY.md](../../../docs/procedures/RELEASE-POLICY.md) - Manifest-based deployment
- [DEPLOYMENT-DECLARATIVES.md](../../../docs/procedures/DEPLOYMENT-DECLARATIVES.md) - Declaration types
- [FE-001 DECISIONS.md](../fe-001/architecture/DECISIONS.md) - Config-driven architecture decisions
- Existing overlay files: `config/overlays/{development,production}/overrides.yaml`
