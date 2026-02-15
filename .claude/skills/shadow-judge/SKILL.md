---
name: "shadow-judge"
description: "Record human judgment on validation results. Approve confirms accuracy; reject identifies false negatives. Stores calibration data in AgentDB."
---

# /shadow-judge -- Human Judgment Recording

## What This Skill Does

Compares automated validation results against human judgment. When a human reviews code after `/validate` runs, they can record whether the validation was accurate (approve) or missed something (reject). This calibrates trust scores over time.

This is the shadow mode mechanism -- validation observes, humans verify, trust accumulates.

---

## Usage

### Approve (human agrees with validation)

```
/shadow-judge approve
```

Records that the most recent validation report was correct. For each check in the report, stores a reflexion entry with `reward=1.0`.

### Reject (human found something validation missed)

```
/shadow-judge reject "missed null check in foo.rs, stale import in bar.rs"
```

Records that the validation missed specific issues. Identifies which tier/check should have caught the problem and stores `reward=0.0` for those checks.

---

## How It Works

### Step 1: Find Most Recent Validation Report

Look for the most recent `validate-impl-*.md` or `validate-plan-report.md` in `product/features/*/reports/`.

```bash
REPORT=$(ls -t product/features/*/reports/validate-*.md 2>/dev/null | head -1)
```

If no report is found, display an error: "No recent validation report found. Run /validate first."

### Step 2: Parse Report

Extract the per-check results from the report:
- Feature ID (from report header)
- Each check name, tier, and result (PASS/FAIL/WARN/NOT_CHECKED)

### Step 3: Record Judgment

#### On Approve

For each check that was PASS in the report:

```
reflexion_store(
  task = "trust:validation:{tier}:{check_name}",
  reward = 1.0,
  success = true,
  critique = "Human confirmed correct. Feature: {feature-id}"
)
```

This reinforces trust in checks that were accurate.

#### On Reject

1. Parse the human's rejection note to identify what was missed
2. Map missed items to the tier/check that should have caught them:

| Missed Issue | Likely Check |
|-------------|-------------|
| Formatting issue | tier1:build or clippy |
| Stub/TODO left in code | tier2:stub_scan |
| Banned dependency | tier2:banned_deps |
| File outside scope | tier2:file_scope |
| Stale reference | tier2:stale_refs |
| Missing acceptance criteria | tier3:ac_coverage |
| Test regression | tier3:test_delta |
| Security concern | (manual -- note in critique) |

3. For each identified check that should have caught the issue:

```
reflexion_store(
  task = "trust:validation:{tier}:{check_name}",
  reward = 0.0,
  success = false,
  critique = "Human found issue validation missed: {human_note}. Feature: {feature-id}"
)
```

4. For checks that correctly passed (not implicated by the rejection):

```
reflexion_store(
  task = "trust:validation:{tier}:{check_name}",
  reward = 1.0,
  success = true,
  critique = "Human confirmed correct (other checks rejected). Feature: {feature-id}"
)
```

### Step 4: Confirm

Display a summary:

```
Shadow Judge Result:
  Report: {report path}
  Feature: {feature-id}
  Action: approve | reject
  Checks confirmed correct: {N}
  Checks marked as false negative: {M}
  Total reflexion entries stored: {N+M}
```

---

## When to Use

Run `/shadow-judge` after you have:
1. Run `/validate` on a feature
2. Personally reviewed the code changes
3. Formed your own judgment about correctness

The discipline of running shadow-judge after every review is what builds meaningful trust scores. Without it, all scores stay at 1.0 (self-reported).

---

## False Negative Tracking

False negatives (reward=0.0) are the high-signal events. They indicate:
- A check that should exist but doesn't
- A check that runs but has gaps in its coverage
- A check that is too permissive

The `/trust-dashboard` highlights checks with low trust scores, which reveals systematic gaps in the validation pipeline.

---

## Examples

### Example: Approve after clean review

```
User reviews ops-006 implementation, finds no issues.

/shadow-judge approve

Result: 15 checks confirmed correct, 0 false negatives.
```

### Example: Reject with specific finding

```
User reviews fe-004, finds a TODO comment in production code.

/shadow-judge reject "TODO comment at line 47 of crates/ndp-lib/src/feature.rs"

Result: tier2:stub_scan marked as false negative (reward=0.0).
Other 14 checks confirmed correct (reward=1.0).
```

---

## Related

- `.claude/skills/validate/SKILL.md` -- produces the reports this skill judges
- `.claude/skills/trust-dashboard/SKILL.md` -- visualizes the trust scores
- `product/features/ops-006/architecture/ARCHITECTURE.md` -- ADR-006 (shadow-judge design)
