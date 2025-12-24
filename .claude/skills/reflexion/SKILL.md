---
name: "reflexion"
description: "Record feedback on whether patterns retrieved via get-pattern were helpful. Essential for training the recommendation system."
---

# Reflexion - Evaluate Pattern Effectiveness

## What This Skill Does

Records feedback on patterns you retrieved using `get-pattern`. This feedback trains the recommendation system to suggest better patterns over time.

**Use this ONLY to evaluate patterns from `get-pattern`** - NOT for storing new patterns or swarm/transient memory.

## CRITICAL: What This Skill Is NOT For

| DO NOT USE FOR | USE INSTEAD |
|----------------|-------------|
| Storing new patterns you discovered | `save-pattern` |
| Recording swarm coordination state | claude-flow memory tools |
| Transient task/agent memory | claude-flow memory tools |
| Storing architecture decisions | `save-pattern` |
| Recording procedures | `save-pattern` |

**This skill is ONLY for evaluating `get-pattern` results.**

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Retrieve relevant patterns
2. DURING work:  Apply the pattern
3. AFTER work:   reflexion    → Did the pattern help? (THIS SKILL)
                 save-pattern → Store NEW discoveries (if any)
```

---

## Quick Usage

### Pattern Worked Well

When a pattern from `get-pattern` was accurate and helpful:

```bash
npx agentdb reflexion store "ndp-patterns" \
  "Used [pattern-name] pattern for [what you did]" \
  1.0 true \
  "Pattern was complete and accurate - no adjustments needed"
```

### Pattern Partially Worked

When a pattern needed modifications:

```bash
npx agentdb reflexion store "ndp-patterns" \
  "Used [pattern-name] pattern but needed adjustment" \
  0.6 true \
  "Pattern missing [specific gap] - consider updating via save-pattern"
```

### Pattern Failed

When a pattern was wrong or outdated:

```bash
npx agentdb reflexion store "ndp-patterns" \
  "Pattern [pattern-name] failed" \
  0.2 false \
  "Pattern is outdated/wrong - [specific issue]. Needs update via save-pattern"
```

---

## Reward Scale

| Score | Meaning | When to Use |
|-------|---------|-------------|
| 1.0 | Perfect | Pattern worked exactly as documented |
| 0.8 | Good | Minor adjustments needed |
| 0.6 | Partial | Significant modifications required |
| 0.4 | Weak | Pattern was marginally helpful |
| 0.2 | Failed | Pattern didn't work, caused issues |
| 0.0 | Harmful | Pattern was actively wrong/dangerous |

---

## What to Include in Critique

The `critique` field trains future recommendations. Be specific:

### Good Critiques

```
"Pattern was complete - followed steps exactly and deployment succeeded"

"Missing retention field that's now required in v2.0 schema - pattern needs update"

"TimescaleDB connection pattern assumed localhost but we use Docker networking"

"Architecture pattern was outdated - ADR-005 superseded the approach described"
```

### Poor Critiques (Avoid)

```
"It worked"           // Too vague - doesn't help future agents
"Failed"              // No actionable info
"Good pattern"        // Doesn't explain what made it good
```

---

## When to Record Feedback

Record feedback when you've:

1. **Retrieved a pattern via `get-pattern`** and applied it
2. **Found a pattern was outdated** or incorrect
3. **Had to modify a pattern** to make it work

**Do NOT record feedback for:**
- Work where you didn't use `get-pattern`
- New discoveries (use `save-pattern` instead)
- Swarm/coordination state (use claude-flow memory tools)

---

## After Recording Feedback

If your critique identifies a pattern that needs updating:

```bash
# 1. Record the feedback (this skill)
npx agentdb reflexion store "ndp-patterns" \
  "Used add-stream pattern" \
  0.6 true \
  "Pattern missing required 'retention' field - needs update"

# 2. Update the pattern (save-pattern skill)
npx agentdb store-pattern \
  --type "development" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "add-stream",
    "approach": "# Add New Data Stream (Updated)\n\n...updated content...",
    "version": "2.0"
  }' \
  --confidence 0.85
```

---

## Causal Learning

For outcomes that establish cause-effect relationships:

```bash
npx agentdb causal add-edge \
  "Using batch size > 1000 for Parquet writes" \
  "Memory exhaustion on Raspberry Pi" \
  -0.8 0.95 3
```

---

## Check Learning Impact

See how feedback has influenced recommendations:

```bash
# Get database statistics
npx agentdb db stats

# Retrieve critique summaries
npx agentdb reflexion critique-summary "pattern-usage" true
```

---

## Related Skills

- **`get-pattern`** - Retrieve patterns BEFORE work (use this first)
- **`save-pattern`** - Store NEW patterns for architecture, procedures, conventions

**NOT related to:**
- Swarm coordination memory (use claude-flow tools)
- Transient task state (use claude-flow tools)
- Agent communication (use claude-flow tools)
