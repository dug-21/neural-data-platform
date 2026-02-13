---
name: "validate"
description: "NDP 3-tier validation for implementation work. Tier 1: build+test. Tier 2: clippy. Tier 3: integration env via deploy.sh or docker-compose."
---

# /validate — NDP Implementation Validation

## What This Skill Does

Runs a structured 3-tier validation against the NDP workspace. Use this at the end of every implementation session, before reporting results to the user.

---

## Quick Reference

```bash
# Tier 1 — Always run
cargo build --workspace 2>&1 | grep -A5 "^error" | head -20
cargo build --workspace 2>&1 | tail -3
cargo test --workspace 2>&1 | tail -30

# Tier 2 — Always for new/modified code
cargo clippy --workspace -- -D warnings 2>&1 | head -30

# Tier 3 — Only when qualifying paths are touched (see table below)
```

---

## Tier 1: Unit (ALWAYS)

Run every time, no exceptions.

### 1a. Build

```bash
cargo build --workspace 2>&1 | grep -A5 "^error" | head -20
cargo build --workspace 2>&1 | tail -3
```

Report: PASS if exit code 0, FAIL with first error otherwise.

### 1b. Test

```bash
cargo test --workspace 2>&1 | tail -30
```

Report: pass/fail count from summary line.

### 1c. Anti-Stub Scan

```bash
grep -rn 'todo!()\\|unimplemented!()\\|TODO\\|FIXME\\|HACK' --include='*.rs' core/ apps/ crates/ tools/ | grep -v '_test\\|test_\\|#\\[test\\]' | head -10
```

Report: WARN if any matches in non-test code. Zero tolerance per CLAUDE.md rule 6.

### 1d. deploy.sh Integrity

```bash
# Only if deploy.sh was modified
git diff --name-only HEAD 2>/dev/null | grep -q 'deploy.sh' && bash -n deploy/pi/deploy.sh
```

Report: PASS/SKIP. Catches syntax errors in the deployment script.

---

## Tier 2: Lint (ALWAYS for new code)

```bash
cargo clippy --workspace -- -D warnings 2>&1 | head -30
```

Report: PASS if zero warnings, WARN with first warning otherwise.

---

## Tier 3: Integration (qualifying changes only)

Check which paths were modified, then use the trigger table to decide Path A or Path B.

### Trigger Table

| Changed Paths | Integration Path |
|---------------|------------------|
| `core/`, `apps/`, `crates/` (Rust binary changes) | A — deploy.sh |
| `config/base/streams/`, `config/integration/` | A — deploy.sh |
| `apps/silver-etl/`, `crates/ndp-lib/src/silver/` | A — deploy.sh |
| `tools/ndp-gold-ddl/`, `deploy/pi/init-scripts/` | B — docker-compose |
| `config/grafana/` | B — docker-compose |
| `core/ndp-mcp-server/` | B — docker-compose |
| None of the above | SKIP Tier 3 |

### Path A — Full Release Test (deploy.sh)

```bash
DEPLOY_ENV=integration ./deploy/pi/deploy.sh build
DEPLOY_ENV=integration ./deploy/pi/deploy.sh deploy
DEPLOY_ENV=integration ./deploy/pi/deploy.sh status
# Verify services are healthy, then:
DEPLOY_ENV=integration ./deploy/pi/deploy.sh stop
```

### Path B — Docker Compose Only

```bash
docker compose -f docker-compose.integration.yml up -d
# Run targeted checks (e.g., psql for DDL, curl for MCP)
docker compose -f docker-compose.integration.yml down -v
```

### Path SKIP

If no qualifying paths were touched, report: `Tier 3: SKIP (no qualifying changes)`.

---

## Cargo Output Truncation (CRITICAL)

Cargo output fills context windows fast. ALWAYS truncate:

```bash
# Build errors: first error + summary only
cargo build --workspace 2>&1 | grep -A5 "^error" | head -20
cargo build --workspace 2>&1 | tail -3

# Test output: summary only
cargo test --workspace 2>&1 | tail -30

# Clippy: first warnings only
cargo clippy --workspace -- -D warnings 2>&1 | head -30
```

NEVER pipe full cargo output into context. If you need more detail, read the specific error, fix it, re-run.

---

## Validation Iteration Cap

- **Iteration 1**: Fix the FIRST blocking error. Re-run /validate.
- **Iteration 2**: If still failing, STOP. Report to user:
  `"Validation failed after 2 attempts. Remaining errors: [summary]"`
- **NEVER iterate beyond 2.** This protects context window for user intervention.

### Context-Saving Delegation

If the coordinator has consumed significant context, delegate validation to a sub-agent:

```
Spawn a Task agent (ndp-tester) with prompt:
  "Run /validate on the workspace. Fix up to 2 blocking errors.
   Report: PASS/WARN/FAIL with summary."
```

---

## Report Format

After validation, report to the user in this format:

```
## Validation Result: PASS | WARN | FAIL

**Tier 1 — Unit**
- Build: PASS | FAIL (first error: ...)
- Tests: X passed, Y failed
- Anti-stub: PASS | WARN (N matches)
- deploy.sh: PASS | SKIP

**Tier 2 — Lint**
- Clippy: PASS | WARN (N warnings)

**Tier 3 — Integration**
- Path: A (deploy.sh) | B (docker-compose) | SKIP
- Result: PASS | FAIL (details)

**Overall**: PASS | WARN | FAIL
```

Overall result:
- **PASS**: All tiers green
- **WARN**: Tier 1/2 pass but with warnings (anti-stub, clippy)
- **FAIL**: Any tier has blocking failures after 2 fix iterations

---

## When NOT to Use /validate

- Planning-only sessions (no code changes)
- Documentation-only changes
- SPARC Specification/Pseudocode/Architecture phases
- Reading/research tasks

## Related

- `.claude/rules/implementation-protocol.md` — full implementation swarm protocol
- `.claude/rules/testing.md` — integration environment details
- `docker-compose.integration.yml` — integration stack definition
- `deploy/pi/deploy.sh` — deployment orchestrator (supports `DEPLOY_ENV=integration`)
