# 03: Local CI/CD and E2E Testing Workflows

## Executive Summary

NDP has a functional but fragmented validation landscape. Three independent systems exist -- the `/validate` skill (3-tier, agent-facing), the `scripts/validation/` suite (legacy "Phase 3" hooks), and `deploy.sh` (integration environment orchestrator) -- but they are not wired together into a coherent pipeline. There are no GitHub Actions workflows, no pre-commit hooks actually installed, no test runner for benchmarks, and 128 `#[ignore]`-annotated tests that never run automatically. The Makefiles reference a "Neural Trader" era and are non-functional for the current NDP workspace.

The key opportunity is building a graduated pipeline that runs the right checks at the right time: sub-second formatting checks on save, seconds-level unit tests on commit, minutes-level integration tests before merge, and a full E2E deploy cycle on demand.

---

## Current State Audit

### What Exists

| Component | Location | Status | Notes |
|-----------|----------|--------|-------|
| `/validate` skill | `.claude/skills/validate/SKILL.md` | **Active, agent-facing** | 3-tier: build+test, clippy, integration. Used by ndp-scrum-master. |
| `deploy.sh` | `deploy/pi/deploy.sh` (2775 lines) | **Active, production** | Supports `DEPLOY_ENV=integration` for local testing. `build`, `status`, `sync`, Silver ETL commands. |
| `docker-compose.integration.yml` | Root | **Active** | Full stack: mosquitto, etcd, TimescaleDB+pgvector, air-quality-app, MCP server, Grafana. |
| `scripts/validation/pre-commit-hook.sh` | `scripts/validation/` | **Exists, NOT installed** | Checks forbidden patterns (TODO, stubs, hardcoded secrets), formatting, file sizes. Legacy "Phase 3" / Neural Trader code. |
| `scripts/validation/install-hooks.sh` | `scripts/validation/` | **Exists, NOT used** | Would install pre-commit, commit-msg (conventional commits), pre-push hooks. |
| `scripts/validation/run-production-validation.sh` | `scripts/validation/` | **Broken** | References `production-validator` binary that does not exist in the current workspace. |
| `Makefile` | Root | **Legacy, non-functional** | References `config-store`, `ruv-fann`, `daa-coordinator`, `data-ingestion` -- none of these exist in the NDP workspace. |
| `Makefile.v2` | Root | **Legacy, non-functional** | References `docker-compose.v2.yml` that does not exist. |
| `scripts/coverage_llvm.sh` | `scripts/` | **Exists, untested** | Uses `cargo-llvm-cov` for LLVM-based coverage. References legacy package names. |
| `.cargo/config.toml` | Root | **Active but stale** | Sets `-C instrument-coverage` for x86_64. Has aliases for `cargo llvm-cov`. References `autonomous-platform` (legacy). |
| `.github/coverage-badge.json` | `.github/` | **Placeholder** | Badge shows "pending" -- no CI generates coverage data. |
| GitHub Actions workflows | `.github/workflows/` | **None exist** | Zero workflow files for the NDP project. |
| Git hooks | `.git/hooks/` | **Only .sample files** | No active hooks installed. |
| Benchmarks | N/A | **None** | No `benches/` directory, no `[[bench]]` sections in any active `Cargo.toml`. `criterion` only in archived legacy crates. |
| `cargo-nextest` | `.devcontainer/setup.sh` | **Commented out** | Listed but not installed in devcontainer setup. |

### What Works Well

1. **`/validate` skill** -- Well-designed 3-tier system that agents use reliably during implementation swarms. Path-based trigger table correctly routes to deploy.sh vs docker-compose. Iteration cap (2 attempts) prevents context window exhaustion.

2. **`deploy.sh` integration mode** -- `DEPLOY_ENV=integration` is a genuine E2E environment with all services (MQTT, etcd, TimescaleDB, app, MCP, Grafana). Health checks on all services. Config sync works.

3. **Cargo workspace test suite** -- 908+ tests across the workspace. `cargo test --workspace` is the established baseline. London TDD style with mockall is consistently applied.

4. **Anti-stub scanning** -- `/validate` includes `grep` scan for `todo!()`, `unimplemented!()`, TODO, FIXME. Enforces CLAUDE.md Rule 6.

### What Does Not Work / Gaps

1. **No pre-commit enforcement**: The pre-commit hook script exists but is never installed. `cargo fmt --check` and `cargo clippy` never run automatically before commits. Agents rely on `/validate` post-implementation but nothing prevents malformed commits.

2. **128 ignored tests never run**: 128 `#[ignore]` annotations across 21 files. Most are integration tests requiring TimescaleDB or external services. No system automatically runs them against the integration environment. Known flaky tests (5 wiremock timing tests) are not tagged or quarantined separately.

3. **No GitHub Actions CI**: Zero workflow files. Every validation happens locally or via agent swarms. There is no automated gate on PR merge.

4. **Legacy validation scripts are broken**: `run-production-validation.sh` calls a `production-validator` binary that does not exist. The Makefiles reference Neural Trader Phase 3 binaries that were removed long ago. These scripts are misleading -- they appear functional but will fail immediately.

5. **No performance benchmarks**: No `criterion` benches, no `cargo bench` targets. ops-005 EC-24 identifies this as a gap. No baseline metrics for ingestion throughput, ETL latency, or memory profile.

6. **No test coverage tracking**: `cargo-llvm-cov` is referenced in `.cargo/config.toml` and `scripts/coverage_llvm.sh` but neither is configured for the current workspace. The coverage badge shows "pending" permanently.

7. **No regression detection**: Test count is tracked informally in MEMORY.md (908 tests) but nothing automatically detects count regressions. A change that deletes tests would not be flagged.

8. **Docker build time**: `deploy.sh` warns "15-30 minutes on first run" for Docker builds. BuildKit cache mounts help subsequent builds, but ARM64 cross-compilation for Pi is not tested locally.

9. **No test sharding or parallelization strategy**: `cargo test` runs with default parallelism. For 908+ tests this is adequate, but as the test suite grows (fe-004 will add more), there is no plan for sharding by crate or test type.

10. **Flaky test quarantine missing**: 5 known flaky `weather_polling_integration` tests (wiremock timing) and `acceptance_partition_structure` are documented in MEMORY.md but not programmatically quarantined. They can cause false negatives in `/validate` runs.

---

## Proposed Testing Pyramid

### Layer 0: Editor/Save (0-2 seconds)

**Tools**: `rust-analyzer` (already in devcontainer), `cargo fmt` on save

**What runs**:
- Format check on modified files
- Syntax errors flagged by rust-analyzer
- No manual step needed

**Implementation**: VS Code `settings.json` with `"editor.formatOnSave": true` for Rust files. Already partially configured in devcontainer.

**Cost**: Zero runtime cost. Already available.

### Layer 1: Pre-commit (5-15 seconds)

**Tools**: Git pre-commit hook (lightweight, not the existing heavy Phase 3 script)

**What runs**:
1. `cargo fmt --check` -- formatting only (fast, no compilation)
2. Anti-stub scan on staged `.rs` files (grep for `todo!()`, `unimplemented!()`)
3. Conventional commit message validation (optional, commit-msg hook)

**What does NOT run**: `cargo build`, `cargo test`, `cargo clippy` -- these are too slow for pre-commit and would create developer friction.

**Implementation**: New lightweight hook at `.git/hooks/pre-commit`, installed via a simple script. The existing `scripts/validation/pre-commit-hook.sh` is too aggressive (checks for `mock_`, `stub_`, `localhost`, `password` patterns -- many of which are legitimate in test code and configuration). A new, focused hook is needed.

**Cost**: Low. Minimal false positives. Fast enough to not interrupt flow.

### Layer 2: Pre-push / On-demand (1-3 minutes)

**Tools**: `cargo build`, `cargo test`, `cargo clippy`

**What runs** (equivalent to `/validate` Tier 1 + Tier 2):
1. `cargo build --workspace` -- catch compilation errors
2. `cargo test --workspace` -- all unit tests (908+ currently, ~40-90 seconds)
3. `cargo clippy --workspace -- -D warnings` -- lint
4. Test count regression check (compare against known baseline)

**What does NOT run**: Integration tests, Docker builds, deploy.sh

**Trigger**: Pre-push hook (blocking for main/release branches), or manual via `just check` / `make check`.

**Cost**: Medium. 1-3 minutes per run. Could be parallelized but cargo handles parallelism internally.

### Layer 3: Integration (5-15 minutes)

**Tools**: `docker-compose.integration.yml`, `deploy.sh` with `DEPLOY_ENV=integration`

**What runs**:
1. Docker image build (`dc build`)
2. Full stack startup with health checks
3. Config sync to etcd
4. Silver schema migration
5. `#[ignore]` integration tests against live TimescaleDB
6. MQTT data injection test (publish and verify Bronze write + Silver ETL)
7. MCP server endpoint smoke tests
8. Stack teardown

**Trigger**: On-demand before merge, or as part of agent swarm `/validate` Tier 3.

**Cost**: High. Requires Docker, takes 5-15 minutes. Only run when qualifying paths are touched (use the existing trigger table from `/validate`).

### Layer 4: E2E / Release (15-30 minutes)

**Tools**: `deploy.sh` full cycle, synthetic load testing (future)

**What runs**:
1. Full `deploy.sh build` + `deploy.sh deploy` + `deploy.sh status`
2. Data flow verification: MQTT publish -> Bronze WAL -> Silver hypertable -> Gold CA
3. Performance baselines (future, per ops-005 EC-24)
4. Memory profile check (RSS after N minutes under load)
5. Rollback test: apply manifest, verify, rollback

**Trigger**: Before tagging a release. Manual or automated via release workflow.

**Cost**: Very high. Only for releases and major changes.

---

## Tool Evaluation for NDP

### Task Runner: `just` vs `cargo-make` vs `make`

| Criterion | `just` | `cargo-make` | `make` |
|-----------|--------|-------------|--------|
| Installation | `cargo install just` | `cargo install cargo-make` | Pre-installed |
| Syntax | Simple, modern | TOML-based, verbose | Makefile syntax |
| Dependency management | Yes (recipe deps) | Yes (task deps) | Yes (target deps) |
| Cross-platform | Yes | Yes | Mostly (GNU make) |
| Rust ecosystem fit | Good | Native | Adequate |
| Conditional logic | Limited | Rich (conditions, env) | Moderate |
| Learning curve | Low | Medium | Low |

**Recommendation**: `just`. Reasons:
- NDP already has two broken Makefiles that confuse the codebase. A fresh `Justfile` makes a clean break.
- `just` syntax is simpler and more readable than `cargo-make` TOML.
- No TOML nesting -- each recipe is a clear shell script.
- Lists recipes with `just --list`, making it self-documenting.
- Already common in Rust projects.

### Local CI Runner: `act` vs Custom Scripts

| Criterion | `act` (GitHub Actions locally) | Custom scripts |
|-----------|-------------------------------|----------------|
| Fidelity to CI | High (runs actual workflows) | Low |
| Docker required | Yes (nested containers) | Depends |
| ARM64 Pi support | Limited | Full control |
| Setup complexity | Medium | Low |
| Maintenance | Tied to GH Actions syntax | Independent |

**Recommendation**: Custom scripts via `just` for now. Reasons:
- NDP has no GitHub Actions workflows yet. Running `act` on zero workflows does nothing.
- NDP's deployment target is Pi (ARM64). `act` runs x86_64 containers. The build environment mismatch makes `act` less valuable than for typical web apps.
- If/when GitHub Actions are added, `act` can be layered on top.

### Test Runner: `cargo test` vs `cargo-nextest`

| Criterion | `cargo test` | `cargo-nextest` |
|-----------|-------------|-----------------|
| Parallel execution | Per-test within crate | Per-test across crates |
| Output format | Default (noisy) | JUnit XML, human-readable |
| Test retries | No | Yes (flaky test handling) |
| Test filtering | Basic | Advanced (partitions, filters) |
| CI integration | Basic | JUnit reports, timing |
| Installation | Built-in | `cargo install cargo-nextest` |

**Recommendation**: Adopt `cargo-nextest`. Reasons:
- Built-in retry support directly addresses the 5 known flaky wiremock tests.
- JUnit XML output enables future CI integration and test result tracking.
- Per-test parallelism across crates is faster for 13-member workspaces.
- Test partitioning enables sharding if the test suite grows beyond 2000 tests.
- Already listed (commented out) in `.devcontainer/setup.sh`, so the team was already considering it.

### Coverage: `cargo-llvm-cov` vs `cargo-tarpaulin`

| Criterion | `cargo-llvm-cov` | `cargo-tarpaulin` |
|-----------|-------------------|-------------------|
| Accuracy | High (source-based) | Medium (ptrace-based) |
| Speed | Fast | Slower |
| Platform | Linux, macOS | Linux only |
| ARM64 support | Yes | Limited |
| Report formats | HTML, LCOV, JSON | HTML, LCOV, JSON, Coveralls |

**Recommendation**: `cargo-llvm-cov`. It is already partially configured in `.cargo/config.toml`. Source-based coverage is more accurate and faster. ARM64 support matters for Pi deployment validation.

---

## Agent Integration Points

### Where CI/CD Hooks into the Swarm Workflow

```
                    Agent Swarm Workflow
                    ====================

/get-pattern -----> Patterns from AgentDB
      |
      v
ndp-scrum-master --> spawn agents
      |
      v
Agents implement --> file changes
      |
      v
/validate ---------> Tier 1: cargo build + test     [LAYER 2]
      |               Tier 2: cargo clippy           [LAYER 2]
      |               Tier 3: deploy.sh integration  [LAYER 3]
      |
      v
Drift check -------> anti-stub scan, file scope check
      |
      v
gh issue comment --> results posted
      |
      v
/reflexion --------> Pattern feedback
```

### Proposed Enhancements

1. **Pre-spawn validation**: Before the scrum-master spawns agents, run `cargo build --workspace` to ensure the workspace compiles. This prevents agents from starting work on a broken baseline.

2. **Test count gate**: After `/validate` Tier 1, compare test count against the baseline (currently 908). If the count decreased, flag as WARN. Formula: `new_count >= baseline - known_flaky_count`.

3. **Flaky test quarantine in `/validate`**: Add `--exclude` patterns for known flaky tests so they do not cause false FAILs during agent validation. Maintain a `tests/flaky.txt` manifest.

4. **Coverage delta check**: After agent changes, run `cargo-llvm-cov` on modified crates only. Report coverage delta. Do not enforce a threshold initially -- just report.

5. **Integration test runner agent**: For `/validate` Tier 3, spawn a dedicated `ndp-tester` agent that:
   - Starts the integration stack
   - Runs `#[ignore]` tests against it
   - Captures results
   - Tears down the stack
   - Reports pass/fail

   This keeps integration testing out of the main agent's context window.

6. **Post-commit hook for agents**: After an agent creates a commit, automatically run `cargo fmt --check` and `cargo clippy --workspace -- -D warnings`. This catches formatting/lint issues before the human sees them.

---

## Implementation Roadmap

### Quick Wins (1-2 hours each)

| Item | Effort | Impact | Description |
|------|--------|--------|-------------|
| QW-1: Install `just` and create `Justfile` | 1h | High | Replace broken Makefiles with working recipes: `just check`, `just test`, `just lint`, `just integration`, `just clean`. |
| QW-2: Lightweight pre-commit hook | 1h | Medium | `cargo fmt --check` + anti-stub grep on staged files. No compilation. Install script: `just install-hooks`. |
| QW-3: Delete or archive legacy Makefiles | 30m | Low | Remove `Makefile` and `Makefile.v2` (or move to `archive/`). They reference non-existent binaries and confuse tooling. |
| QW-4: Fix `.cargo/config.toml` | 30m | Low | Remove `autonomous-platform` references. Keep `instrument-coverage` flag but gate it behind an env var to avoid affecting normal builds. |
| QW-5: Test count baseline in `/validate` | 1h | Medium | After `cargo test --workspace`, parse the summary line and compare against a stored baseline (e.g., `tests/baseline-count.txt`). Warn on regression. |
| QW-6: Flaky test manifest | 30m | Medium | Create `tests/flaky.txt` listing the 5 wiremock tests + `acceptance_partition_structure`. `/validate` can exclude or separately report these. |

### Medium-term (1-2 days each)

| Item | Effort | Impact | Description |
|------|--------|--------|-------------|
| MT-1: Install and configure `cargo-nextest` | 2h | High | Add to devcontainer setup. Create `.config/nextest.toml` with retry policy for flaky tests. Update `/validate` to use nextest for Tier 1 tests. |
| MT-2: Integration test runner script | 1d | High | `just integration` recipe that starts docker-compose, runs `cargo test --workspace -- --ignored` (or nextest equivalent), captures results, tears down. Usable by agents and humans. |
| MT-3: Coverage reporting | 1d | Medium | Configure `cargo-llvm-cov` for the current workspace. Generate HTML + LCOV reports. Update coverage badge. Create `just coverage` recipe. |
| MT-4: GitHub Actions CI workflow | 1d | High | Basic workflow: on PR to main, run `cargo build --workspace`, `cargo test --workspace`, `cargo clippy`. No Docker/integration (save that for local). |
| MT-5: Pre-push hook | 2h | Medium | Run `just check` (build + test + clippy) before push. Blocking for main/release branches. Warning-only for feature branches. |
| MT-6: Conventional commit enforcement | 1h | Low | Install commit-msg hook that validates `type(scope): description` format. Already designed in `scripts/validation/pre-commit-hook.sh`. |

### Long-term (1 week+)

| Item | Effort | Impact | Description |
|------|--------|--------|-------------|
| LT-1: Performance benchmark framework | 1w | High | Create `benches/` directory with criterion benchmarks for: WAL write throughput, Parquet read speed, Silver ETL batch processing, embedding generation. Track baselines. Addresses ops-005 EC-24. |
| LT-2: ARM64 cross-compilation CI | 3d | Medium | GitHub Actions workflow that cross-compiles for `aarch64-unknown-linux-gnu`. Validates that the binary can be built for Pi without deploying. Uses `cross` or Docker buildx. |
| LT-3: Synthetic load generator | 1w | High | Tool that generates N streams at M events/sec via MQTT. Runs against integration environment. Measures end-to-end latency (MQTT publish to Silver row), memory growth, disk usage. |
| LT-4: Automated regression detection | 3d | Medium | Store test results (nextest JUnit XML) per commit. Compare against previous run. Flag new failures, missing tests, and performance regressions. Could be a simple script or a dedicated service. |
| LT-5: Agent-driven test generation | 1w | Medium | When an `ndp-rust-dev` agent creates a new module, automatically spawn an `ndp-tester` agent to generate test cases. Use the module's trait signatures and doc comments as input. |
| LT-6: Integration test isolation | 3d | Medium | Each integration test gets its own TimescaleDB schema (or database) to enable parallel execution. Currently all ignored tests would compete for the same `silver` schema. |

---

## Cost/Benefit Analysis

### Quick Wins (Total: ~5 hours)

| Item | Cost | Benefit | ROI |
|------|------|---------|-----|
| QW-1: Justfile | 1h setup, zero ongoing | Every developer and agent gets `just check` instead of remembering cargo incantations. Self-documenting. | Very High |
| QW-2: Pre-commit hook | 1h setup, seconds per commit | Catches formatting issues before they enter git history. Prevents trivial `/validate` failures. | High |
| QW-3: Remove legacy Makefiles | 30m | Eliminates confusion from broken tooling. New contributors do not waste time trying `make test`. | Medium |
| QW-4: Fix cargo config | 30m | Prevents coverage instrumentation from accidentally affecting release builds. | Low |
| QW-5: Test count baseline | 1h | Detects accidental test deletion. The test count has been tracked manually in MEMORY.md -- automate it. | Medium |
| QW-6: Flaky manifest | 30m | Reduces false negatives in `/validate`. Currently agents may waste 2 fix iterations on flaky tests. | Medium |

### Medium-term (Total: ~5 days)

| Item | Cost | Benefit | ROI |
|------|------|---------|-----|
| MT-1: cargo-nextest | 2h setup, minimal ongoing | Faster test runs, JUnit output, built-in retries for flaky tests. Direct improvement to agent validation speed. | High |
| MT-2: Integration test runner | 1d setup, ongoing maintenance | 128 ignored tests finally run somewhere. Catches real bugs that unit tests miss (like the Silver NULL constraint issue). | Very High |
| MT-3: Coverage reporting | 1d setup | Visibility into untested code. Can inform agent test generation. Makes the coverage badge meaningful. | Medium |
| MT-4: GitHub Actions CI | 1d setup, ongoing | Automated gate on PRs. Catches issues before human review. Standard industry practice. | High |
| MT-5: Pre-push hook | 2h | Prevents pushing code that fails build/test. Saves time for both agents and the human reviewer. | Medium |
| MT-6: Conventional commits | 1h | Consistent commit history. Enables automated changelog generation (already used in release policy). | Low |

### Long-term (Total: ~4 weeks)

| Item | Cost | Benefit | ROI |
|------|------|---------|-----|
| LT-1: Benchmarks | 1w | Performance baselines for the platform. Catches regressions before Pi deployment. Addresses ops-005. | High |
| LT-2: ARM64 CI | 3d | Catches ARM64-specific issues before deploying to Pi. Currently untested until physical deployment. | Medium |
| LT-3: Load generator | 1w | Validates platform behavior under sustained load. Critical for long-running Pi deployment. | High |
| LT-4: Regression detection | 3d | Automated comparison of test results across commits. Reduces manual tracking. | Medium |
| LT-5: Agent test generation | 1w | Reduces time agents spend writing tests. Improves test coverage organically. | Medium |
| LT-6: Integration test isolation | 3d | Enables parallel integration tests. Currently they would deadlock on shared schemas. | Medium |

---

## Proposed `Justfile` (Quick Win QW-1)

```just
# NDP Development Commands
# Usage: just <recipe>
# List all recipes: just --list

set dotenv-load := false

# Default recipe: show help
default:
    @just --list

# === Layer 1: Fast checks (pre-commit) ===

# Check formatting only (no compilation)
fmt-check:
    cargo fmt --check

# Format all code
fmt:
    cargo fmt

# === Layer 2: Build + Test + Lint ===

# Full pre-push validation (build + test + clippy)
check: build test lint

# Build the workspace
build:
    cargo build --workspace 2>&1 | tail -5

# Run all unit tests
test:
    cargo test --workspace 2>&1 | tail -30

# Run clippy lints
lint:
    cargo clippy --workspace -- -D warnings 2>&1 | head -30

# Anti-stub scan
stubs:
    @grep -rn 'todo!()\|unimplemented!()\|TODO\|FIXME\|HACK' \
        --include='*.rs' core/ apps/ crates/ tools/ \
        | grep -v '_test\|test_\|#\[test\]' | head -20 || echo "No stubs found"

# === Layer 3: Integration ===

# Start integration environment
integration-up:
    docker compose -f docker-compose.integration.yml up -d
    @echo "Waiting for services..."
    @sleep 15
    DEPLOY_ENV=integration ./deploy/pi/deploy.sh status

# Stop integration environment
integration-down:
    docker compose -f docker-compose.integration.yml down -v

# Run ignored integration tests (requires integration-up)
integration-test:
    cargo test --workspace -- --ignored 2>&1 | tail -50

# Full integration cycle
integration: integration-up integration-test integration-down

# === Layer 4: Release ===

# Full release validation
release-check:
    DEPLOY_ENV=integration ./deploy/pi/deploy.sh build
    DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy
    DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
    DEPLOY_ENV=integration ./deploy/pi/deploy.sh stop

# === Utilities ===

# Run coverage analysis
coverage:
    cargo llvm-cov --workspace --html --output-dir target/coverage

# Install git hooks
install-hooks:
    @echo "Installing pre-commit hook..."
    @cp scripts/hooks/pre-commit .git/hooks/pre-commit
    @chmod +x .git/hooks/pre-commit
    @echo "Hooks installed."

# Clean build artifacts
clean:
    cargo clean

# Show test count vs baseline
test-count:
    @echo -n "Current test count: "
    @cargo test --workspace 2>&1 | grep "test result" | \
        awk '{sum += $2} END {print sum}'
```

---

## Proposed Lightweight Pre-commit Hook (Quick Win QW-2)

```bash
#!/bin/bash
# NDP pre-commit hook: fast checks only
# Install: just install-hooks

set -e

# Only check staged Rust files
STAGED_RS=$(git diff --cached --name-only --diff-filter=ACM | grep '\.rs$' || true)

if [ -z "$STAGED_RS" ]; then
    exit 0
fi

# 1. Format check (no compilation, fast)
if ! cargo fmt --check --quiet 2>/dev/null; then
    echo "ERROR: cargo fmt check failed. Run 'cargo fmt' to fix."
    exit 1
fi

# 2. Anti-stub scan on staged files only
STUBS=$(echo "$STAGED_RS" | xargs grep -n 'todo!()\|unimplemented!()' 2>/dev/null | grep -v '#\[test\]\|_test\|test_' || true)
if [ -n "$STUBS" ]; then
    echo "ERROR: Stubs found in staged files:"
    echo "$STUBS"
    exit 1
fi

exit 0
```

---

## Proposed GitHub Actions Workflow (Medium-term MT-4)

```yaml
name: CI
on:
  pull_request:
    branches: [main]
  push:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - name: Format check
        run: cargo fmt --check
      - name: Build
        run: cargo build --workspace
      - name: Test
        run: cargo test --workspace
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings
      - name: Anti-stub scan
        run: |
          if grep -rn 'todo!()\|unimplemented!()' --include='*.rs' \
               core/ apps/ crates/ tools/ \
               | grep -v '_test\|test_\|#\[test\]'; then
            echo "::error::Stubs found in non-test code"
            exit 1
          fi
```

---

## Files Referenced in This Research

| File | Path | Purpose |
|------|------|---------|
| Validate skill | `/workspaces/neural-data-platform/.claude/skills/validate/SKILL.md` | 3-tier validation used by agents |
| Deploy script | `/workspaces/neural-data-platform/deploy/pi/deploy.sh` | Production + integration deployment |
| Integration compose | `/workspaces/neural-data-platform/docker-compose.integration.yml` | Local integration stack |
| Testing rules | `/workspaces/neural-data-platform/.claude/rules/testing.md` | Testing conventions |
| Implementation protocol | `/workspaces/neural-data-platform/.claude/rules/implementation-protocol.md` | Validation tiers in Step 3e |
| Pre-commit hook | `/workspaces/neural-data-platform/scripts/validation/pre-commit-hook.sh` | Legacy hook (not installed) |
| Install hooks | `/workspaces/neural-data-platform/scripts/validation/install-hooks.sh` | Hook installer (not used) |
| Production validation | `/workspaces/neural-data-platform/scripts/validation/run-production-validation.sh` | Broken (missing binary) |
| Makefile | `/workspaces/neural-data-platform/Makefile` | Legacy, non-functional |
| Makefile.v2 | `/workspaces/neural-data-platform/Makefile.v2` | Legacy, non-functional |
| Cargo config | `/workspaces/neural-data-platform/.cargo/config.toml` | Coverage instrumentation, stale aliases |
| Coverage script | `/workspaces/neural-data-platform/scripts/coverage_llvm.sh` | LLVM coverage (references legacy packages) |
| Root Dockerfile | `/workspaces/neural-data-platform/Dockerfile` | Multi-stage build for air-quality-app |
| Devcontainer setup | `/workspaces/neural-data-platform/.devcontainer/setup.sh` | Dev environment (nextest commented out) |
| Workspace Cargo.toml | `/workspaces/neural-data-platform/Cargo.toml` | 13 workspace members |
| ops-005 scope | `/workspaces/neural-data-platform/product/features/ops-005/SCOPE.md` | EC-24: performance testing gap |
| Improvement plan | `/workspaces/neural-data-platform/product/continuous-improvement-plan.md` | Constraint: no CI/CD changes |
| Swarm protocol | `/workspaces/neural-data-platform/.claude/rules/swarm-protocol.md` | Agent coordination model |
| Coverage badge | `/workspaces/neural-data-platform/.github/coverage-badge.json` | Placeholder, always "pending" |
