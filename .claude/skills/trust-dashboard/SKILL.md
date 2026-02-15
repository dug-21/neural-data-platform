---
name: "trust-dashboard"
description: "Render per-check Bayesian trust scores from AgentDB reflexion entries. Read-only -- no side effects."
---

# /trust-dashboard -- Validation Trust Scores

## What This Skill Does

Queries AgentDB for trust reflexion entries (written by `/validate` after each run), computes per-check Bayesian trust scores using Beta distributions, and renders a human-readable dashboard. This is a read-only skill with no side effects.

Trust scores start uninformative (0.5) and calibrate over time as more validation runs and `/shadow-judge` comparisons accumulate.

---

## Usage

```
/trust-dashboard
```

No arguments required. Queries all `trust:validation:*` entries from AgentDB.

---

## How It Works

### Step 1: Query AgentDB

```
reflexion_retrieve(task="trust:validation", limit=500)
```

This returns all reflexion entries with the `trust:validation:*` prefix, written by `/validate` (ADR-004) and `/shadow-judge` (ADR-006).

### Step 2: Group by Check Name

Parse the task prefix to extract tier and check name:
```
trust:validation:{tier}:{check_name}
```

Group entries by `{tier}:{check_name}`.

### Step 3: Compute Beta Scores

For each check, compute the Bayesian trust score:

```
correct = count of entries where reward = 1.0
incorrect = count of entries where reward = 0.0

Trust(check) = (correct + 1) / (correct + incorrect + 2)
```

This is a Beta(correct+1, incorrect+1) distribution mean. With no data, Trust = 0.5 (uninformative prior). As evidence accumulates, the score converges to the true reliability.

### Step 4: Compute Composite Score

```
Composite = 0.30 * avg(Tier1_checks)
          + 0.30 * avg(Tier2_checks)
          + 0.15 * avg(Tier3_checks)
          + 0.15 * (1 - rework_rate)
          + 0.10 * scope_conformance
```

Where:
- `rework_rate` = proportion of validation runs that required fix iterations
- `scope_conformance` = proportion of file_scope checks that passed

If a tier has no data, use 0.5 (uninformative) for its average.

### Step 5: Render Dashboard

```
TRUST DASHBOARD ({date})
============================

COMPOSITE SCORE: {score}

PER-CHECK SCORES:
  Tier 1 (Compilation):
    build ............... {score} ({correct}/{total})
    test ................ {score} ({correct}/{total})
    clippy .............. {score} ({correct}/{total})
    anti_stub ........... {score} ({correct}/{total})

  Tier 2 (Process Adherence):
    banned_deps ......... {score} ({correct}/{total})
    stub_scan ........... {score} ({correct}/{total})
    file_scope .......... {score} ({correct}/{total})
    stale_refs .......... {score} ({correct}/{total})
    config_valid ........ {score} ({correct}/{total})

  Tier 3 (Spec Compliance):
    ac_coverage ......... {score} ({correct}/{total})
    test_delta .......... {score} ({correct}/{total})
    new_deps ............ {score} ({correct}/{total})

  Tier 4 (Risk Classification):
    risk_score .......... {score} ({correct}/{total})

SHADOW JUDGE CALIBRATION:
  Total comparisons: {N}
  Human agreed: {M}
  Human disagreed: {K}
  False negative rate: {K/N}

LAST 5 FEATURES:
  | Feature | Tier1 | Tier2 | Tier3 | Tier4 | Overall |
  |---------|-------|-------|-------|-------|---------|
  | {id}    |  {r}  |  {r}  |  {r}  | {risk}|  {r}    |

TREND: {Improving|Stable|Regressing} (based on last 5 composite scores)
```

### Empty State

If no trust entries exist yet:

```
TRUST DASHBOARD ({date})
============================

No trust data available yet.

Trust scores accumulate after each /validate run.
Run /validate on a feature to start building trust data.
Run /shadow-judge after reviewing to add calibration signal.
```

---

## Interpreting Scores

| Score Range | Meaning |
|-------------|---------|
| 0.90 - 1.00 | High trust -- check is reliable |
| 0.70 - 0.89 | Good trust -- occasional false negatives |
| 0.50 - 0.69 | Uninformative -- not enough data or mixed results |
| 0.30 - 0.49 | Low trust -- check frequently misses issues |
| 0.00 - 0.29 | Very low trust -- check is unreliable |

**Important**: Scores below 0.5 are unusual and indicate the check is wrong more often than right. This typically means the check itself needs revision, not that the code is bad.

---

## Data Sources

| Source | What It Writes | When |
|--------|---------------|------|
| `/validate` | `reward=1.0` for each passing check | After every validation run |
| `/shadow-judge approve` | `reward=1.0` for all checks | When human agrees |
| `/shadow-judge reject` | `reward=0.0` for missed checks | When human finds issues |

Self-reported trust (from `/validate` alone) is always 1.0. Only `/shadow-judge reject` adds real calibration signal (reward=0.0).

---

## Statistical Note

The Beta distribution needs approximately 20+ observations per check for statistically meaningful scores. With fewer observations, the score is heavily influenced by the prior (0.5). The dashboard does not attempt significance testing -- it shows raw Beta means and observation counts so the user can judge data sufficiency.

---

## Related

- `.claude/skills/validate/SKILL.md` -- writes trust entries after validation
- `.claude/skills/shadow-judge/SKILL.md` -- human calibration input
- `product/features/ops-006/architecture/ARCHITECTURE.md` -- ADR-004 (trust storage), ADR-005 (dashboard)
