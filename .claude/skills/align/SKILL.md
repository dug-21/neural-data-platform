---
name: "align"
description: "On-demand vision alignment check. Spawns ndp-vision-guardian to review SPARC artifacts against product/vision/ALIGNMENT-CRITERIA.md."
---

# /align — Vision Alignment Check

## What This Skill Does

Runs an on-demand alignment check of SPARC planning artifacts against the product vision. Spawns the `ndp-vision-guardian` agent to review specifications and produce an ALIGNMENT-REPORT.md.

Use this:
- During or after SPARC planning phases (S, P, A)
- When you want to gut-check a feature against the vision
- Before generating an Implementation Brief
- Anytime you want to verify alignment without running a full planning swarm

---

## Usage

```
/align {feature-id}
```

Example:
```
/align fe-003
```

---

## What Happens

1. **Read** `product/vision/ALIGNMENT-CRITERIA.md` (user-owned vision criteria)
2. **Read** `product/features/{feature-id}/SCOPE.md` (what the user asked for)
3. **Read** all SPARC artifacts in `product/features/{feature-id}/` (specification/, pseudocode/, architecture/)
4. **Spawn** `ndp-vision-guardian` agent via Task tool:

```
Spawn a Task agent (ndp-vision-guardian) with prompt:
  "Review the SPARC artifacts at product/features/{feature-id}/ against the vision
   criteria in product/vision/ALIGNMENT-CRITERIA.md. Read the SCOPE.md to understand
   user intent. Produce an ALIGNMENT-REPORT.md at
   product/features/{feature-id}/ALIGNMENT-REPORT.md.
   Flag all variances requiring user approval."
```

5. **Present** the "Variances Requiring Approval" section to the user
6. **Wait** for user decision on each variance before proceeding

---

## Output

The agent produces: `product/features/{feature-id}/ALIGNMENT-REPORT.md`

Key sections presented to the user:
- Summary table (7 principles, each PASS/WARN/VARIANCE/FAIL)
- Scope alignment (gaps, additions, simplifications)
- Variances requiring approval (with recommendations)
- Technical constraints check

---

## Vision Criteria Location

The alignment criteria live in a user-owned document:

**`product/vision/ALIGNMENT-CRITERIA.md`**

This document consolidates checkable criteria from:
- `product/vision/EDGE-INTELLIGENCE-PLATFORM.md` (master vision)
- `product/vision/ROADMAP-TO-V2.md` (version roadmap)
- `product/INTEGRATION_FIRST_MANDATE.md` (extend don't replace)

The user can edit ALIGNMENT-CRITERIA.md at any time. All agents and skills reference it immediately.

---

## Variance Classifications

| Classification | Meaning | Action |
|----------------|---------|--------|
| **PASS** | Aligned | None |
| **WARN** | Minor deviation, documented | Note in report |
| **VARIANCE** | Significant, needs user approval | Present to user |
| **FAIL** | Violates hard constraint | Must fix before implementation |

---

## Integration with Planning Protocol

The planning swarm protocol (`.claude/rules/planning-protocol.md`) calls `/align` automatically as Step 5 after all planning agents complete. You can also invoke it manually at any point.

---

## When NOT to Use /align

- Implementation-only sessions (no SPARC artifacts to review)
- Bug fixes with no spec (use GitHub Issue instead)
- Pure ops/tooling work with no feature spec

## Related

- `product/vision/ALIGNMENT-CRITERIA.md` — the criteria document (user-editable)
- `.claude/agents/ndp/ndp-vision-guardian.md` — the agent definition
- `.claude/rules/planning-protocol.md` — planning swarm protocol (calls /align at Step 5)
