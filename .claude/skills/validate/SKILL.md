---
name: "validate"
description: "NDP 4-tier implementation validation. Tier 1: compilation (build+test+anti-stub). Tier 2: process adherence. Tier 3: spec compliance. Tier 4: risk classification. Produces glass box reports."
---

# /validate -- NDP Implementation Validation

## What This Skill Does

Runs a structured 4-tier validation against the NDP workspace. Use this at the end of every implementation session, before reporting results to the user. Produces a glass box report showing exactly what was checked.

---

## Quick Reference

```bash
# Tier 1 -- Always run (compilation)
cargo build --workspace 2>&1 | grep -A5 "^error" | head -20
cargo test --workspace 2>&1 | tail -30

# Tier 2 -- Always run (process adherence)
# Banned deps, stub scan, file scope, stale refs, config valid

# Tier 3 -- When ACCEPTANCE-MAP.md exists (spec compliance)
# AC coverage, test delta, new deps

# Tier 4 -- Always compute (risk classification)
# Scope, depth, domain -> LOW/MEDIUM/HIGH
```

---

## Tier 1: Compilation (ALWAYS)

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

**Flaky test separation**: Compare any failures against `.ndp/flaky-tests.txt`:
```bash
RESULTS=$(cargo test --workspace 2>&1)
echo "$RESULTS" | grep "FAILED" | while read line; do
  test_name=$(echo "$line" | awk '{print $2}')
  grep -q "$test_name" .ndp/flaky-tests.txt 2>/dev/null && \
    echo "KNOWN FLAKY: $test_name" || \
    echo "REAL FAILURE: $test_name"
done
```

### 1c. Anti-Stub Scan

```bash
grep -rn 'todo!()\|unimplemented!()\|TODO\|FIXME\|HACK' --include='*.rs' core/ apps/ crates/ tools/ | grep -v '_test\|test_\|#\[test\]' | head -10
```

Report: WARN if any matches in non-test code. Zero tolerance per CLAUDE.md rule 6.

### 1d. deploy.sh Integrity

```bash
# Only if deploy.sh was modified
git diff --name-only HEAD 2>/dev/null | grep -q 'deploy.sh' && bash -n deploy/pi/deploy.sh
```

Report: PASS/SKIP. Catches syntax errors in the deployment script.

### 1e. Integration (qualifying changes only)

Check which paths were modified, then use the trigger table:

| Changed Paths | Integration Path |
|---------------|------------------|
| `core/`, `apps/`, `crates/` (Rust binary changes) | A -- deploy.sh |
| `config/base/streams/`, `config/integration/` | A -- deploy.sh |
| `apps/silver-etl/`, `crates/ndp-lib/src/silver/` | A -- deploy.sh |
| `tools/ndp-gold-ddl/`, `deploy/pi/init-scripts/` | B -- docker-compose |
| `config/grafana/` | B -- docker-compose |
| `core/ndp-mcp-server/` | B -- docker-compose |
| None of the above | SKIP |

**Path A**: `DEPLOY_ENV=integration ./deploy/pi/deploy.sh build && deploy && status && stop`
**Path B**: `docker compose -f docker-compose.integration.yml up -d` then targeted checks, then `down -v`

---

## Tier 2: Process Adherence (ALWAYS)

All checks are shell commands, not compiled tests.

### 2a. Banned Dependency Scan

```bash
grep -rn 'duckdb\|polars\|jemalloc' --include='Cargo.toml' . | grep -v '#'
```

Report: PASS if zero matches, FAIL if any found.

### 2b. Anti-Stub Scan (Expanded)

```bash
grep -rn 'todo!()\|unimplemented!()\|TODO\|FIXME\|HACK' --include='*.rs' \
  core/ apps/ crates/ tools/ | grep -v '_test\|test_\|#\[test\]' | head -20
```

Report: PASS if zero matches, WARN with match count.

### 2c. File Scope Check

Compare modified files against the implementation brief:

```bash
FEATURE_ID="${1:-unknown}"
git diff --name-only HEAD~1 2>/dev/null | while read f; do
  grep -q "$f" "product/features/${FEATURE_ID}/IMPLEMENTATION-BRIEF.md" 2>/dev/null || \
    echo "WARN: $f modified but not listed in brief"
done
```

Report: PASS if all files are in scope, WARN with list of out-of-scope files.

### 2d. Stale Reference Scan

```bash
grep -rn 'pattern.*\b29\b\|pattern.*\b32\b' --include='*.md' .claude/ product/features/ 2>/dev/null | \
  grep -v 'deprecated\|SELF-CHECK\|ADHERENCE-AUDIT' | head -10
```

Report: PASS if zero matches, WARN with list.

### 2e. Config Schema Validation

```bash
for f in config/base/streams/*.json; do
  python3 -c "import json; json.load(open('$f'))" 2>&1 || echo "FAIL: $f invalid JSON"
done
```

Report: PASS if all parse, FAIL with list of invalid files.

---

## Tier 3: Spec Compliance (when ACCEPTANCE-MAP.md exists)

### 3a. AC Coverage

Parse ACCEPTANCE-MAP.md for test function names and verification methods:

```bash
FEATURE_ID="${1:-unknown}"
MAP="product/features/${FEATURE_ID}/ACCEPTANCE-MAP.md"

if [ -f "$MAP" ]; then
  # For 'test' verification methods, check cargo test --list
  grep '| test |' "$MAP" | while IFS='|' read _ acid _ _ detail _; do
    test_name=$(echo "$detail" | tr -d '` ')
    cargo test --workspace --list 2>/dev/null | grep -q "$test_name" && \
      echo "COVERED: $acid ($test_name)" || \
      echo "NOT_COVERED: $acid ($test_name)"
  done

  # For 'file-check' methods, verify file exists
  grep '| file-check |' "$MAP" | while IFS='|' read _ acid _ _ detail _; do
    filepath=$(echo "$detail" | grep -oP '`[^`]+`' | tr -d '`' | head -1)
    [ -f "$filepath" ] && echo "COVERED: $acid" || echo "NOT_COVERED: $acid"
  done
fi
```

### 3b. Test Count Delta

```bash
CURRENT=$(cargo test --workspace 2>&1 | grep "test result" | \
  grep -oP '\d+ passed' | awk '{sum += $1} END {print sum+0}')
BASELINE=$(cat .ndp/test-baseline.txt 2>/dev/null || echo "0")

if [ "$CURRENT" -lt "$BASELINE" ]; then
  echo "WARN: Test count decreased ($CURRENT < $BASELINE)"
elif [ "$CURRENT" -gt "$BASELINE" ]; then
  echo "PASS: Test count increased ($CURRENT > $BASELINE, +$((CURRENT - BASELINE)) new)"
else
  echo "PASS: Test count stable ($CURRENT)"
fi
```

### 3c. New Dependency Check

```bash
git diff HEAD~1 -- '**/Cargo.toml' 2>/dev/null | grep '^+.*=' | grep -v '^\+\+\+' | \
  grep -v 'version\|edition\|name\|path\|workspace' | head -10
```

Report: list new dependencies for human review.

---

## Tier 4: Risk Classification (ALWAYS)

Compute risk level based on diff characteristics. Informational only -- does not block.

### 4a. Scope

```bash
FILE_COUNT=$(git diff --name-only HEAD~1 2>/dev/null | wc -l)
if [ "$FILE_COUNT" -lt 5 ]; then
  SCOPE="narrow"
elif [ "$FILE_COUNT" -le 15 ]; then
  SCOPE="moderate"
else
  SCOPE="broad"
fi
```

### 4b. Depth

- **surface**: Only comments, variable names, formatting changes
- **logic**: Behavior changes (new conditions, changed control flow)
- **structural**: New modules, new traits, new crates

```bash
# Heuristic: check for new files, new modules, new trait definitions
NEW_FILES=$(git diff --name-only --diff-filter=A HEAD~1 2>/dev/null | wc -l)
NEW_TRAITS=$(git diff HEAD~1 2>/dev/null | grep '^+.*trait ' | wc -l)
NEW_MODS=$(git diff HEAD~1 2>/dev/null | grep '^+.*mod ' | wc -l)

if [ "$((NEW_FILES + NEW_TRAITS + NEW_MODS))" -gt 3 ]; then
  DEPTH="structural"
elif git diff HEAD~1 2>/dev/null | grep -q '^+.*if \|^+.*match \|^+.*loop \|^+.*while '; then
  DEPTH="logic"
else
  DEPTH="surface"
fi
```

### 4c. Domain

```bash
CHANGED=$(git diff --name-only HEAD~1 2>/dev/null)
if echo "$CHANGED" | grep -q '^core/\|^apps/'; then
  DOMAIN="core"
elif echo "$CHANGED" | grep -q '^crates/\|^tools/'; then
  DOMAIN="platform"
else
  DOMAIN="tooling"
fi
```

### 4d. Composite Risk

| Scope | Depth | Domain | Risk |
|-------|-------|--------|------|
| narrow | surface | tooling | LOW |
| narrow | logic | platform | MEDIUM |
| moderate | logic | any | MEDIUM |
| broad | structural | core | HIGH |
| any | any (with anomaly) | any | +1 level |

**Anomaly flags** (each bumps risk up one level):
- Test count decreased
- Diff exceeds 500 lines
- New external dependency added

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

NEVER pipe full cargo output into context.

---

## Validation Iteration Cap

- **Iteration 1**: Fix the FIRST blocking error. Re-run /validate.
- **Iteration 2**: If still failing, STOP. Report to user:
  `"Validation failed after 2 attempts. Remaining errors: [summary]"`
- **NEVER iterate beyond 2.** This protects context window for user intervention.

---

## Glass Box Report Format

Output to `product/features/{feature-id}/reports/validate-impl-{wave}.md`:

```markdown
# Validation Report: {feature-id} impl (wave N)

> Date: {date}
> Type: impl (wave N)
> Feature: {feature-id}

## Summary

RESULT: PASS | WARN | FAIL
Checks: {N passed} / {M total} ({K not checked})
Confidence: {score}/100

## Tier 1: Compilation

| Check | Result | Evidence |
|-------|--------|----------|
| Build | PASS/FAIL | {error count or "0 errors"} |
| Test | PASS/FAIL | {X passed, Y failed (Z known flaky)} |
| Anti-stub | PASS/WARN | {match count} |
| deploy.sh | PASS/SKIP | {syntax check result} |
| Integration | PASS/FAIL/SKIP | {path A/B result or "no qualifying changes"} |

## Tier 2: Process Adherence

| Check | Result | Evidence |
|-------|--------|----------|
| Banned deps | PASS/FAIL | {count found in Cargo.toml} |
| Stub scan | PASS/WARN | {match count in non-test code} |
| File scope | PASS/WARN | {count of out-of-scope files} |
| Stale refs | PASS/WARN | {deprecated pattern IDs found} |
| Config valid | PASS/FAIL | {invalid config files} |

## Tier 3: Spec Compliance

| AC-ID | Verification | Result |
|-------|-------------|--------|
| AC-01 | {method} | COVERED/NOT_COVERED |
| AC-02 | {method} | COVERED/NOT_COVERED |

Test delta: {current} vs {baseline} ({+N new} or {-N regression})

## Tier 4: Risk Classification

| Dimension | Value |
|-----------|-------|
| Scope | narrow/moderate/broad ({N} files) |
| Depth | surface/logic/structural |
| Domain | tooling/platform/core |
| Risk | LOW/MEDIUM/HIGH |
| Anomalies | {list or "none"} |

## NOT CHECKED

| Item | Reason |
|------|--------|
| {item} | {reason} |

## RECOMMENDED HUMAN REVIEW

- {item 1 with rationale}
- {item 2 with rationale}
```

**Confidence score calculation:**
```
confidence = (checks_passed / checks_total) * 100
           - (5 * count(NOT_CHECKED))
           - (10 * count(WARN))
           - (25 * count(FAIL))
```
Minimum 0, maximum 100.

---

## Overall Result

- **PASS**: All tiers green
- **WARN**: Tier 1/2 pass but with warnings (anti-stub, clippy, out-of-scope files)
- **FAIL**: Any tier has blocking failures after 2 fix iterations

---

## Trust Recording (after each validation run)

After producing the validation report, record trust entries in AgentDB for each check that was evaluated. This feeds the `/trust-dashboard` skill.

For each check in Tiers 1-4 that was evaluated (not skipped):

```
reflexion_store(
  task = "trust:validation:{tier}:{check_name}",
  reward = 1.0,
  success = true,
  critique = "Self-reported: {PASS|WARN|FAIL}. Feature: {feature-id}. Evidence: {brief evidence}"
)
```

**Task prefix convention**: `trust:validation:{tier}:{check_name}`

Where:
- `tier`: tier1, tier2, tier3, tier4
- `check_name`: build, test, clippy, anti_stub, integration, banned_deps, stub_scan, file_scope, stale_refs, config_valid, ac_coverage, test_delta, new_deps, risk_score

**Important**: Self-reported entries always have `reward=1.0` because the validation reports its own correctness. Real calibration comes from `/shadow-judge reject` which writes `reward=0.0` for false negatives.

Do NOT skip the trust recording step. Even self-reported data establishes the baseline for shadow-judge calibration.

---

## When NOT to Use /validate

- Planning-only sessions (no code changes) -- use `/validate-plan` instead
- Documentation-only changes
- SPARC Specification/Pseudocode/Architecture phases
- Reading/research tasks

---

## Related

- `.claude/rules/implementation-protocol.md` -- full implementation swarm protocol
- `.claude/rules/testing.md` -- integration environment details, baseline/flaky management
- `.claude/skills/validate-plan/SKILL.md` -- planning artifact validation
- `docker-compose.integration.yml` -- integration stack definition
- `deploy/pi/deploy.sh` -- deployment orchestrator (supports `DEPLOY_ENV=integration`)
