---
name: "validate-plan"
description: "Validate planning swarm output: artifact existence, AC coverage, pattern IDs, stale references, internal consistency. Produces a glass box report."
---

# /validate-plan -- Planning Artifact Validation

## What This Skill Does

Validates the output of a planning swarm before handing off to implementation. Runs 5 checks against the planning artifacts for a given feature ID. Produces a glass box report showing exactly what was checked, what passed, and what needs human review.

Run this after the planning swarm completes and before creating the GH Issue.

---

## Usage

```
/validate-plan {feature-id}
```

Where `{feature-id}` is e.g. `ops-006`, `fe-004`, `dp-002`.

---

## 5 Checks

### Check 1: Required Artifacts Exist

```bash
FEATURE_DIR="product/features/${FEATURE_ID}"

for f in IMPLEMENTATION-BRIEF.md ACCEPTANCE-MAP.md LAUNCH-PROMPT.md ALIGNMENT-REPORT.md; do
  [ -f "$FEATURE_DIR/$f" ] && echo "PASS: $f" || echo "FAIL: MISSING $f"
done

[ -f "$FEATURE_DIR/specification/SPECIFICATION.md" ] && echo "PASS: SPECIFICATION.md" || echo "FAIL: MISSING SPECIFICATION.md"
[ -f "$FEATURE_DIR/architecture/ARCHITECTURE.md" ] && echo "PASS: ARCHITECTURE.md" || echo "FAIL: MISSING ARCHITECTURE.md"
```

### Check 2: AC Coverage

Parse SCOPE.md for acceptance criteria IDs, then verify each appears in ACCEPTANCE-MAP.md or IMPLEMENTATION-BRIEF.md.

```bash
# Extract AC-IDs from SCOPE.md
SCOPE_ACS=$(grep -oP 'AC-\d+' "$FEATURE_DIR/SCOPE.md" | sort -u)

# Check each AC appears in the map
for ac in $SCOPE_ACS; do
  grep -q "$ac" "$FEATURE_DIR/ACCEPTANCE-MAP.md" 2>/dev/null && \
    echo "PASS: $ac found in ACCEPTANCE-MAP.md" || \
    echo "FAIL: $ac missing from ACCEPTANCE-MAP.md"
done
```

### Check 3: ADR Pattern IDs Resolve

Extract pattern IDs from the IMPLEMENTATION-BRIEF.md Resolved Decisions table and verify each resolves in AgentDB.

```
For each pattern ID in the brief's "Resolved Decisions" table:
  Call agentdb_pattern_search(task="pattern {id}", k=1)
  If result found: PASS
  If no result: WARN (pattern may not be stored yet)
```

### Check 4: No Stale References

Grep planning artifacts for deprecated pattern IDs and removed file paths.

```bash
# Deprecated pattern IDs
grep -rn 'pattern.*\b29\b\|pattern.*\b32\b' "$FEATURE_DIR/" --include='*.md' && \
  echo "FAIL: Deprecated pattern IDs found" || echo "PASS: No deprecated pattern IDs"

# Known removed paths
for stale in "STATUS.md" "bugs/" "verification-quality"; do
  grep -rn "$stale" "$FEATURE_DIR/" --include='*.md' 2>/dev/null && \
    echo "WARN: Stale reference to $stale" || true
done
```

### Check 5: Internal Consistency

- File paths in the brief's "Files to Create/Modify" section reference valid workspace locations (parent directories exist)
- AC-IDs in ACCEPTANCE-MAP.md match those in SCOPE.md (no extra, no missing)
- Feature ID in brief matches the directory name

```bash
# Verify file paths in brief are plausible
grep -oP '`[^`]+\.(rs|md|sh|json|toml|yaml|yml|txt)`' "$FEATURE_DIR/IMPLEMENTATION-BRIEF.md" | \
  tr -d '`' | while read filepath; do
    parent=$(dirname "$filepath")
    [ -d "$parent" ] || [ "$parent" = "." ] && echo "PASS: $filepath parent exists" || \
      echo "WARN: $filepath parent dir does not exist: $parent"
  done
```

---

## Glass Box Report Format

Output to `product/features/{feature-id}/reports/validate-plan-report.md`:

```markdown
# Validation Report: {feature-id} plan

> Date: {date}
> Type: plan
> Feature: {feature-id}

## Summary

RESULT: PASS | WARN | FAIL
Checks: {N passed} / {M total} ({K not checked})
Confidence: {score}/100

## Check Results

| # | Check | Result | Evidence |
|---|-------|--------|----------|
| 1 | Required artifacts exist | PASS/FAIL | {list missing files} |
| 2 | AC coverage | PASS/FAIL | {X/Y ACs found} |
| 3 | ADR pattern IDs resolve | PASS/WARN | {list unresolved IDs} |
| 4 | No stale references | PASS/WARN | {list stale refs found} |
| 5 | Internal consistency | PASS/WARN | {list inconsistencies} |

## NOT CHECKED

| Item | Reason |
|------|--------|
| Spec quality (content correctness) | Requires human review |
| ADR technical soundness | Requires domain expertise |
| Alignment report thoroughness | Requires human judgment |

## RECOMMENDED HUMAN REVIEW

- Review ALIGNMENT-REPORT.md for any VARIANCE or FAIL items
- Verify SCOPE.md acceptance criteria match your intent
- Review LAUNCH-PROMPT.md proposed prompt before pasting
- Check that architectural decisions (ADRs) align with your expectations
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

- **PASS**: All 5 checks green (no FAIL, at most WARN)
- **WARN**: No FAIL but one or more WARN (stale refs, unresolved patterns)
- **FAIL**: Any check has a FAIL result (missing artifact, missing AC)

---

## Related

- `.claude/rules/planning-protocol.md` -- planning swarm protocol (Step 3h calls this skill)
- `.claude/skills/validate/SKILL.md` -- implementation validation (4-tier)
- `product/vision/ALIGNMENT-CRITERIA.md` -- vision alignment criteria
