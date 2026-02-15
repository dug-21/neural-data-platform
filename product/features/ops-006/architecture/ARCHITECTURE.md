# Architecture: ops-006 Validation Pipeline & Trust Infrastructure

> Date: 2026-02-15
> Feature: ops-006
> Scope: 10 ADRs covering validation skills, hook enforcement, trust storage, and planning deliverable formats

---

## ADR-001: validate-plan Skill Structure

### Context
Planning swarms produce 5-7 artifacts but nothing verifies they are internally consistent, complete, or reference valid resources. The vision guardian checks alignment principles but not artifact quality (valid pattern IDs, existing file paths, complete AC coverage).

### Decision
Create `.claude/skills/validate-plan/SKILL.md` that runs after planning swarm completion. The skill performs 5 checks:

**Check 1 -- Required artifacts exist:**
```bash
for f in IMPLEMENTATION-BRIEF.md ACCEPTANCE-MAP.md LAUNCH-PROMPT.md ALIGNMENT-REPORT.md; do
  [ -f "product/features/${FEATURE_ID}/$f" ] || echo "MISSING: $f"
done
[ -f "product/features/${FEATURE_ID}/specification/SPECIFICATION.md" ] || echo "MISSING: SPECIFICATION.md"
[ -f "product/features/${FEATURE_ID}/architecture/ARCHITECTURE.md" ] || echo "MISSING: ARCHITECTURE.md"
```

**Check 2 -- AC coverage:** Parse SCOPE.md acceptance criteria list, verify each AC-ID appears in the IMPLEMENTATION-BRIEF.md or ACCEPTANCE-MAP.md.

**Check 3 -- ADR pattern IDs resolve:** Extract pattern IDs from the brief's Resolved Decisions table, verify each resolves via `agentdb_pattern_search`.

**Check 4 -- No stale references:** Grep deliverables for deprecated pattern IDs (29, 32) and known removed file paths.

**Check 5 -- Internal consistency:** File paths in the brief's "Files to Create/Modify" section are valid workspace paths. AC-IDs in ACCEPTANCE-MAP.md match those in SCOPE.md.

Output: `product/features/{id}/reports/validate-plan-report.md` with per-check PASS/FAIL/WARN.

Invocation: After planning swarm returns, scrum-master runs `/validate-plan {feature-id}`.

### Consequences
- Catches incomplete or inconsistent planning before implementation starts
- Adds 1-2 minutes to planning phase
- Requires ACCEPTANCE-MAP.md to exist (ADR-007 dependency)
- Does not check spec quality (content), only completeness and consistency

---

## ADR-002: validate-impl Skill Structure (4 Tiers)

### Context
Current `/validate` has 3 tiers (build+test, clippy, integration) but lacks process adherence checks, spec compliance mapping, and risk classification. ops-006 SCOPE.md specifies a 4-tier structure with glass box reporting.

### Decision
Upgrade `.claude/skills/validate/SKILL.md` to support 4 tiers. The existing Tier 1-2 logic is preserved; Tiers 2b-4 are new.

**Tier 1 -- Compilation (unchanged):**
- `cargo build --workspace` (truncated output)
- `cargo test --workspace` (summary only)
- Anti-stub scan (grep for todo!/unimplemented! in non-test .rs files)
- deploy.sh syntax check (if modified)

**Tier 2 -- Process Adherence (NEW):**
All checks are shell commands, not compiled tests.

```bash
# Banned dependency scan
grep -rn 'duckdb\|polars\|jemalloc' --include='Cargo.toml' . | grep -v '#'

# Anti-stub scan (expanded)
grep -rn 'todo!()\|unimplemented!()\|TODO\|FIXME\|HACK' --include='*.rs' \
  core/ apps/ crates/ tools/ | grep -v '_test\|test_\|#\[test\]'

# File scope check (files modified vs brief)
git diff --name-only HEAD~1 | while read f; do
  grep -q "$f" "product/features/${FEATURE_ID}/IMPLEMENTATION-BRIEF.md" || \
    echo "WARN: $f not in brief"
done

# Stale reference scan
grep -rn 'pattern.*29\|pattern.*32' --include='*.md' .claude/ product/features/

# Config schema validation
for f in config/base/streams/*.json; do
  python3 -c "import json; json.load(open('$f'))" 2>&1 || echo "FAIL: $f invalid"
done
```

**Tier 3 -- Spec Compliance (NEW):**
- Parse ACCEPTANCE-MAP.md for test function names
- Cross-reference against `cargo test --list 2>/dev/null | grep "test_"`
- Report: per-AC COVERED/NOT_COVERED with test function name
- Test count delta: compare against stored baseline (see ADR-010)
- New dependency check: diff Cargo.toml against brief's Dependencies section

**Tier 4 -- Risk Classification (NEW):**
- Scope: count modified files. narrow (<5), moderate (5-15), broad (>15)
- Depth: surface (comments/names), logic (behavior), structural (new modules/traits)
- Domain: tooling (.claude/, product/), platform (crates/, tools/), core (core/, apps/)
- Composite risk: LOW (narrow+surface+tooling), MEDIUM (mixed), HIGH (broad+structural+core)
- Anomaly flags: test count decrease, >500 line diff, new external dependency

Output format: see ADR-009 (Glass Box Report).

### Consequences
- Replaces current 3-tier with 4-tier; backward compatible (Tier 1-2a unchanged)
- Tier 2 checks are ~5 seconds total (shell commands)
- Tier 3 requires ACCEPTANCE-MAP.md to exist
- Tier 4 is informational -- no blocking action, helps user decide review depth
- Integration tier (old Tier 3) becomes part of Tier 1 (deploy.sh/docker-compose remains path-gated)

---

## ADR-003: Hook Enforcement Model

### Context
All current hooks have `continueOnError: true` -- nothing blocks. The adherence audit found zero gating hooks. ops-006 needs some hooks to be actual gates (block on failure) while others remain advisory.

### Decision
Three new hooks, each with distinct enforcement levels:

**Hook 1 -- Pre-commit quality gate (BLOCKS):**
Matcher: `^Bash$` with command pattern matching `git commit`.

Implementation: A new script `.claude/hooks/pre-commit-gate.sh` that:
1. Runs `cargo fmt --check` on staged .rs files
2. Greps staged .rs files for `todo!()`, `unimplemented!()` (excluding test files)
3. Returns exit code 1 if either check fails

Hook config in settings.json:
```json
{
  "matcher": "^Bash$",
  "hooks": [{
    "type": "command",
    "command": "echo \"$TOOL_INPUT_command\" | grep -q 'git commit' && bash .claude/hooks/pre-commit-gate.sh || true",
    "timeout": 15000,
    "continueOnError": false
  }]
}
```

Key: `continueOnError: false` makes this a gate. The `|| true` on the grep ensures non-commit bash commands pass through.

**Hook 2 -- Post-task artifact check (WARNS):**
Fires after Task tool completion. Checks if expected artifacts were produced based on task description keywords.

Implementation: `.claude/hooks/post-task-check.sh`:
- If task description contains "planning": check for IMPLEMENTATION-BRIEF.md, ACCEPTANCE-MAP.md, LAUNCH-PROMPT.md
- If task description contains "implementation": check for modified .rs files, test results
- Reports violations but does not block (`continueOnError: true`)

**Hook 3 -- Pre-spawn workspace check (BLOCKS):**
Before implementation swarm spawns agents, verify workspace compiles.

Implementation: The scrum-master runs `cargo build --workspace 2>&1 | tail -5` before spawning agents. If build fails, abort spawning. This is a protocol step, not a settings.json hook, because it needs to run at a specific point in the swarm flow (after init, before spawn).

Add to `implementation-protocol.md` Step 3c pre-spawn checklist:
```
- [ ] cargo build --workspace passes (abort if fails)
```

**Hook 4 -- Pre-commit test regression (WARNS):**
Alongside the pre-commit gate, compare test count against baseline.

```bash
CURRENT=$(cargo test --workspace 2>&1 | grep "test result" | grep -oP '\d+ passed' | grep -oP '\d+')
BASELINE=$(cat .ndp/test-baseline.txt 2>/dev/null || echo "0")
[ "$CURRENT" -lt "$BASELINE" ] && echo "WARN: Test count decreased ($CURRENT < $BASELINE)"
```

This warns but does not block (`continueOnError: true`).

### Consequences
- Pre-commit gate is the ONLY blocking hook -- prevents format violations and stubs from being committed
- Post-task check surfaces artifact gaps immediately after agent completion
- Pre-spawn check prevents wasted agent time on broken workspace
- Test regression warning catches count decreases before commit
- `continueOnError: false` is used sparingly -- only where failure should absolutely prevent the action

---

## ADR-004: Trust Storage Model

### Context
ops-006 needs trust scores that accumulate over time, per validation check. DEV-ARCH-FLOW-PROPOSED.md specifies using AgentDB's existing reflexion_store with a `trust:validation:*` prefix -- no new infrastructure.

### Decision
Trust scores are reflexion entries in AgentDB with a structured task prefix:

**Write (after each validation run):**
```
reflexion_store(
  task = "trust:validation:tier2:banned_deps",
  reward = 1.0,
  success = true,
  critique = "Correctly found 0 banned deps. Feature: ops-006"
)
```

**Task prefix convention:**
```
trust:validation:{tier}:{check_name}
```

Where:
- `tier`: tier1, tier2, tier3, tier4
- `check_name`: build, test, clippy, banned_deps, stub_scan, file_scope, stale_refs, config_valid, ac_coverage, test_delta, new_deps, risk_score

**Read (dashboard query):**
```
reflexion_retrieve(task = "trust:validation", limit = 200)
```

Returns all trust entries. Dashboard skill (ADR-005) computes scores from these.

**Scoring formula:**
```
Trust(check) = Beta(correct + 1, incorrect + 1)
             = (correct + 1) / (correct + incorrect + 2)
```

Where:
- `correct` = count of entries where `reward = 1.0` for this check
- `incorrect` = count of entries where `reward = 0.0` for this check

Initial state: no entries, so Trust = 1/2 = 0.5 (uninformative prior).

**Self-reported vs shadow-compared:**
Initially, all entries are self-reported (validation reports its own correctness = 1.0 always). Once shadow-judge is used (ADR-006), entries include human-compared results with `reward = 0.0` for false negatives.

### Consequences
- Zero new infrastructure -- uses existing AgentDB reflexion table
- Semantic search works: "which tier 2 checks are regressing?"
- /learner can analyze trust data alongside pattern reflexions
- Self-reported trust starts at 1.0 (uninformative) -- only shadow-judge adds real signal
- Prefix convention enables filtering without new query features

---

## ADR-005: trust-dashboard Skill

### Context
Users need visibility into how well the validation pipeline performs over time. Trust scores accumulate in AgentDB (ADR-004) but are not human-readable without a rendering step.

### Decision
Create `.claude/skills/trust-dashboard/SKILL.md` that:

1. **Queries** AgentDB: `reflexion_retrieve(task="trust:validation", limit=500)`
2. **Groups** entries by check_name (extracted from task prefix)
3. **Computes** per-check Beta scores: `(correct + 1) / (correct + incorrect + 2)`
4. **Computes** composite score:
   ```
   Composite = 0.30 * avg(Tier1_checks)
             + 0.30 * avg(Tier2_checks)
             + 0.15 * avg(Tier3_checks)
             + 0.15 * (1 - rework_rate)
             + 0.10 * scope_conformance
   ```
5. **Renders** human-readable output:

```
TRUST DASHBOARD (2026-02-15)
============================

COMPOSITE SCORE: 0.72

PER-CHECK SCORES:
  Tier 1 (Compilation):
    build ............... 0.95 (19/20)
    test ................ 0.90 (18/20)
    clippy .............. 0.85 (17/20)

  Tier 2 (Process Adherence):
    banned_deps ......... 1.00 (5/5)
    stub_scan ........... 0.83 (5/6)
    file_scope .......... 0.75 (3/4)

  Tier 3 (Spec Compliance):
    ac_coverage ......... 0.67 (2/3)
    test_delta .......... 1.00 (3/3)

LAST 5 FEATURES:
  | Feature | Tier1 | Tier2 | Tier3 | Tier4 | Overall |
  |---------|-------|-------|-------|-------|---------|
  | ops-006 |  PASS |  PASS |  WARN | LOW   |  WARN   |
  | fe-004  |  PASS |  PASS |  PASS | LOW   |  PASS   |

TREND: Stable (no regression in last 5 features)
```

Invocation: User runs `/trust-dashboard` at any time.

### Consequences
- Read-only skill -- no side effects
- Depends on ADR-004 entries existing (empty dashboard shows "No data yet")
- Composite formula weights are hardcoded initially; could become configurable
- Shows trend direction but not statistical significance (need 20+ features for that)

---

## ADR-006: shadow-judge Skill

### Context
Self-reported trust scores (ADR-004) are always 1.0 because the validation reports its own correctness. Real trust calibration requires comparing automated validation against human judgment. This is the shadow mode mechanism from DEV-ARCH-FLOW-PROPOSED.md Phase B.

### Decision
Create `.claude/skills/shadow-judge/SKILL.md` with two commands:

**Approve (human agrees with validation):**
```
/shadow-judge approve
```
Records: for each check in the most recent validation report, store a reflexion entry with reward=1.0 (validation was correct).

**Reject (human found something validation missed):**
```
/shadow-judge reject "missed null check in foo.rs, stale import in bar.rs"
```
Records:
- For the specific missed checks, store reflexion entries with reward=0.0
- Store the human's critique as the reflexion critique field
- Identify which tier/check_name should have caught it

**Implementation logic:**
1. Read the most recent validation report from `product/features/{id}/reports/`
2. Parse the per-check results
3. On approve: for each PASS check, store `reflexion_store(task="trust:validation:{tier}:{check}", reward=1.0, critique="Human confirmed. Feature: {id}")`
4. On reject: parse human notes, map to check_names, store reward=0.0 for missed checks and reward=1.0 for checks that correctly passed

**Storage format:**
Same as ADR-004 -- reflexion entries with `trust:validation:*` prefix. The only difference is `reward=0.0` for false negatives, which calibrates the Beta distribution.

### Consequences
- Shadow mode requires human discipline (must run /shadow-judge after every review)
- False negatives (reward=0.0) are the high-signal events -- they lower trust scores
- Need 20+ shadow comparisons for statistically meaningful trust scores
- No automated mechanism forces shadow-judge usage; it's a process commitment

---

## ADR-007: ACCEPTANCE-MAP.md Format

### Context
Acceptance criteria in SCOPE.md are prose. ops-006 needs a machine-parseable mapping from AC-IDs to verification methods so validate-impl (ADR-002, Tier 3) can check spec compliance automatically.

### Decision
Planning swarm produces `product/features/{id}/ACCEPTANCE-MAP.md` with this format:

```markdown
# {feature-id} Acceptance Criteria Map

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | Description from SCOPE.md | test | `test_function_name` | PENDING |
| AC-02 | Description | manual | "Run /trust-dashboard, verify output" | PENDING |
| AC-03 | Description | file-check | "File .claude/hooks/pre-commit-gate.sh exists" | PENDING |
| AC-04 | Description | grep | "grep 'continueOnError.*false' .claude/settings.json" | PENDING |
```

**Verification method types:**
- `test` -- a cargo test function name (can be verified by `cargo test --list`)
- `manual` -- requires human verification (document what to check)
- `file-check` -- file exists at expected path
- `grep` -- content matches expected pattern
- `shell` -- run a shell command, check exit code

**AC-ID convention:** `AC-{NN}` matching SCOPE.md numbering. Same IDs used in SCOPE.md, ACCEPTANCE-MAP.md, IMPLEMENTATION-BRIEF.md, and GH Issue checklist.

**Lifecycle:**
1. Planning: specification agent creates with status PENDING
2. Implementation: scrum-master updates status to IN_PROGRESS, PASS, FAIL, or DEFERRED
3. Completion: all entries must be PASS, MANUAL_VERIFIED, or DEFERRED(with reason)

### Consequences
- Enables automated Tier 3 spec compliance checking in validate-impl
- Adds ~10 minutes to planning phase (specification agent)
- Verification detail field enables diverse check types (not just cargo tests)
- Must be maintained during implementation (status updates)

---

## ADR-008: LAUNCH-PROMPT.md Format

### Context
The handoff from planning to implementation currently requires the user to craft an implementation kickoff prompt manually. This is error-prone -- users may forget constraints, pattern IDs, or gotchas discovered during planning. A pre-crafted launch prompt reduces friction and improves implementation quality.

### Decision
Planning swarm produces `product/features/{id}/LAUNCH-PROMPT.md` with this structure:

```markdown
# Implementation Launch Prompt: {feature-id}

## Proposed Prompt

> Implement {feature-id}: {title}
>
> GitHub Issue: #{N}
> Brief: product/features/{id}/IMPLEMENTATION-BRIEF.md
> Acceptance Map: product/features/{id}/ACCEPTANCE-MAP.md
>
> Pattern IDs from planning: {list ADR pattern IDs}
>
> Constraints:
> - {key constraint 1}
> - {key constraint 2}
>
> Wave structure: {N} waves
> - Wave 1: {summary}
> - Wave 2: {summary}

## Reminders for User

- Review ALIGNMENT-REPORT.md for any variances before proceeding
- Verify acceptance criteria in SCOPE.md match your expectations
- Edit the prompt above if scope has changed since planning

## Gotchas Discovered During Planning

- {gotcha 1 from planning agents}
- {gotcha 2}

## Key Deliverables Reference

| Artifact | Path |
|----------|------|
| Brief | product/features/{id}/IMPLEMENTATION-BRIEF.md |
| Acceptance Map | product/features/{id}/ACCEPTANCE-MAP.md |
| Architecture ADRs | product/features/{id}/architecture/ARCHITECTURE.md |
| Alignment Report | product/features/{id}/ALIGNMENT-REPORT.md |
```

**User workflow:** Read LAUNCH-PROMPT.md, optionally edit the proposed prompt, paste it to start implementation.

### Consequences
- Reduces implementation kickoff friction
- Captures planning gotchas that would otherwise be lost
- User retains full control (edit before pasting)
- Adds ~5 minutes to planning phase

---

## ADR-009: Glass Box Report Format

### Context
Current validation reports are pass/fail with minimal detail. ops-006 requires "glass box" reports that show not just what passed, but what was NOT checked and what needs human review.

### Decision
Both validate-plan and validate-impl produce reports in this format:

```markdown
# Validation Report: {feature-id} {type}

> Date: {date}
> Type: plan | impl (wave N)
> Feature: {feature-id}

## Summary

RESULT: PASS | WARN | FAIL
Checks: {N passed} / {M total} ({K not checked})
Confidence: {score}/100

## Tier Results

### Tier 1: Compilation
| Check | Result | Evidence |
|-------|--------|----------|
| Build | PASS | 0 errors |
| Test | PASS | 924 passed (908 existing + 16 new) |
| Clippy | PASS | 0 warnings |
| Anti-stub | PASS | 0 matches |

### Tier 2: Process Adherence
| Check | Result | Evidence |
|-------|--------|----------|
| Banned deps | PASS | 0 found in Cargo.toml |
| File scope | WARN | 1 file not in brief: README.md |
| Stale refs | PASS | No deprecated pattern IDs found |
| Config valid | PASS | All stream configs parse |

### Tier 3: Spec Compliance
| AC-ID | Test | Result |
|-------|------|--------|
| AC-01 | test_adherence_audit | PASS |
| AC-02 | (file-check) | PASS |
| AC-11 | (manual) | NOT_CHECKED |

### Tier 4: Risk Classification
| Dimension | Value |
|-----------|-------|
| Scope | narrow (4 files) |
| Depth | surface |
| Domain | tooling |
| Risk | LOW |
| Anomalies | none |

## NOT CHECKED

| Item | Reason |
|------|--------|
| Integration deploy | No Pi available in dev environment |
| AC-11 LAUNCH-PROMPT.md | Manual verification required |

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

Report path: `product/features/{id}/reports/validate-{plan|impl}-{wave}.md`

### Consequences
- Human can see exactly what was and was not verified
- NOT CHECKED section prevents false confidence
- RECOMMENDED HUMAN REVIEW focuses attention
- Confidence score is a heuristic, not a formal measure
- Reports are git-tracked for audit trail

---

## ADR-010: Test Baseline and Flaky Test Management

### Context
ops-006 requires test count regression detection (AC-18) and flaky test separation (AC-16). Current test count is 908 (platform-core). No baseline is stored and no flaky manifest exists.

### Decision

**Test baseline storage:**
Store baseline in `.ndp/test-baseline.txt` (simple text file, git-tracked):
```
908
```

Updated after each successful release. validate-impl Tier 3 compares current count against this baseline.

**Flaky test manifest:**
Store in `.ndp/flaky-tests.txt` (one test name per line, git-tracked):
```
# Known flaky tests (wiremock timing)
weather_polling_integration::test_nws_polling_success
weather_polling_integration::test_nws_polling_retry
weather_polling_integration::test_nws_polling_timeout
weather_polling_integration::test_nws_concurrent_polling
weather_polling_integration::test_nws_polling_circuit_breaker
# Pre-existing failure (hourly vs daily partitioning)
acceptance_partition_structure
```

**How validate uses it:**
```bash
# Run tests, capture output
RESULTS=$(cargo test --workspace 2>&1)

# Extract pass count
CURRENT=$(echo "$RESULTS" | grep "test result" | grep -oP '\d+ passed' | \
  awk '{sum += $1} END {print sum}')
BASELINE=$(cat .ndp/test-baseline.txt 2>/dev/null || echo "0")

# Check regression
if [ "$CURRENT" -lt "$BASELINE" ]; then
  echo "WARN: Test count decreased ($CURRENT < $BASELINE)"
fi

# Check if failures are all flaky
echo "$RESULTS" | grep "FAILED" | while read line; do
  test_name=$(echo "$line" | awk '{print $2}')
  grep -q "$test_name" .ndp/flaky-tests.txt 2>/dev/null && \
    echo "KNOWN FLAKY: $test_name" || \
    echo "REAL FAILURE: $test_name"
done
```

**Update process:**
- Baseline updated manually after confirmed successful release
- Flaky manifest updated when new flaky tests are identified or old ones are fixed
- Both files are simple, git-tracked, human-editable

### Consequences
- Simple file-based approach, no database or AgentDB storage needed
- Flaky tests are visible and tracked, not silently ignored
- Test regression is caught before commit (via pre-commit hook warning)
- Manual baseline update ensures intentional threshold changes
- `.ndp/` directory used for project metadata (not `.claude/` which is for agent tooling)
