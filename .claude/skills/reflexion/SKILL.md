---
name: "reflexion"
description: "Record feedback on pattern effectiveness. Stores episodes that train the recommendation system and enable pattern discovery via learner."
---

# Reflexion - Evaluate Pattern Effectiveness

## What This Skill Does

Records feedback on patterns and approaches used during work. This feedback:
1. Trains the recommendation system for better pattern suggestions
2. Provides data for `learner` skill to auto-discover new patterns
3. Tracks what works and what doesn't over time

**Use this AFTER completing work** to record what helped and what didn't.

---

## Quick Reference

```bash
# Store feedback
npx agentdb reflexion store "session-id" "task description" reward success "critique"

# Retrieve similar experiences
npx agentdb reflexion retrieve "search query" --k 5 --only-successes

# Get critique summary
npx agentdb reflexion critique-summary "topic" true
```

---

## Primary Method: Store Feedback

```bash
npx agentdb reflexion store \
  "dp-004" \
  "Used domain-adapter pattern for new HTTP source" \
  1.0 \
  true \
  "Pattern was complete - followed Source trait steps exactly, tests passed first try"
```

### Parameters (positional)

| Position | Parameter | Description |
|----------|-----------|-------------|
| 1 | session-id | Feature ID (e.g., `dp-004`, `air-011`) |
| 2 | task | Description of what you did |
| 3 | reward | Success score 0-1 |
| 4 | success | `true` or `false` |
| 5 | critique | Specific feedback (required) |
| 6 | input | Optional: task input |
| 7 | output | Optional: task output |
| 8 | latency-ms | Optional: execution time |
| 9 | tokens | Optional: tokens used |

---

## Examples

### Pattern Worked Well

```bash
npx agentdb reflexion store \
  "dp-004" \
  "Used domain-adapter pattern for new HTTP source" \
  1.0 \
  true \
  "Pattern was complete - followed Source trait steps exactly, tests passed first try"
```

### Pattern Partially Worked

```bash
npx agentdb reflexion store \
  "dp-004" \
  "Used add-stream pattern but needed adjustment" \
  0.6 \
  true \
  "Pattern missing retention field requirement added in v2.0 - should update pattern via save-pattern"
```

### Pattern Failed

```bash
npx agentdb reflexion store \
  "dp-004" \
  "Pattern mqtt-routing failed for multi-topic subscription" \
  0.2 \
  false \
  "Pattern assumes single topic per source - needs update for multi-topic. Used workaround with topic array."
```

### No Pattern Found

```bash
npx agentdb reflexion store \
  "dp-004" \
  "Implemented TimescaleDB continuous aggregate - no existing pattern" \
  0.85 \
  true \
  "No pattern existed. Created new approach using hypertable + continuous_aggregate. Should save as new pattern."
```

---

## Retrieve Similar Experiences

```bash
# Find successful similar work
npx agentdb reflexion retrieve "HTTP source implementation" \
  --k 5 \
  --only-successes \
  --min-reward 0.7

# Find failures to learn from
npx agentdb reflexion retrieve "MQTT configuration" \
  --k 5 \
  --only-failures

# Get synthesized summary
npx agentdb reflexion retrieve "parquet storage" \
  --k 10 \
  --synthesize-context
```

### Retrieve Parameters

| Parameter | Description |
|-----------|-------------|
| `--k` | Number of results |
| `--only-successes` | Only successful episodes |
| `--only-failures` | Only failed episodes |
| `--min-reward` | Minimum reward threshold |
| `--synthesize-context` | Generate summary |

---

## Get Critique Summary

Aggregate lessons from critiques:

```bash
# Get critique summary for failures
npx agentdb reflexion critique-summary "mqtt" true

# Get all critiques for a topic
npx agentdb reflexion critique-summary "architecture" false
```

---

## Reward Scale

| Score | Meaning | When to Use |
|-------|---------|-------------|
| 1.0 | Perfect | Pattern/approach worked exactly as expected |
| 0.8 | Good | Minor adjustments needed |
| 0.6 | Partial | Significant modifications required |
| 0.4 | Weak | Marginally helpful, major workarounds |
| 0.2 | Failed | Didn't work, caused issues |
| 0.0 | Harmful | Actively wrong, wasted time |

---

## Session ID Convention

Use consistent session IDs for aggregation:

| Session ID | Use For |
|------------|---------|
| `{feature-id}` | Feature work (e.g., `dp-004`, `air-011`) |
| `{feature-id}-{phase}` | Specific phase (e.g., `dp-004-spec`) |
| `maintenance` | Bug fixes, refactoring |
| `exploration` | Research, spikes, experiments |

---

## Critique Best Practices

**Good critiques** (specific, actionable):
```
"Pattern was complete - followed steps exactly and deployment succeeded"
"Missing retention field that's now required in v2.0 schema"
"TimescaleDB connection pattern assumed localhost but we use Docker networking"
"Architecture pattern outdated - ADR-005 superseded the approach"
```

**Poor critiques** (vague, unusable):
```
"It worked"              # Too vague
"Failed"                 # No actionable info
"Good pattern"           # Doesn't explain what made it good
```

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Search for relevant patterns
2. DURING work:  Apply patterns, note gaps and discoveries
3. AFTER work:   reflexion    → Record what helped (THIS SKILL)
                 save-pattern → Store NEW discoveries (if any)
                 learner      → Auto-discover patterns from episodes (periodic)
```

---

## After Recording Feedback

If your critique identifies a pattern that needs updating:

```bash
# 1. Record the feedback (this skill)
npx agentdb reflexion store \
  "dp-004" \
  "Used add-stream pattern" \
  0.6 \
  true \
  "Pattern missing required retention field"

# 2. Update the pattern (save-pattern skill)
npx agentdb skill create \
  "add-stream-v2" \
  "Add Data Stream (v2.0): Now requires retention field. Steps: 1) Create config.yaml, 2) Add retention field (required), 3) Run sync..." \
  "tags: streams, config, updated"
```

---

## Related Skills

- **`get-pattern`** - Search patterns BEFORE work
- **`save-pattern`** - Store NEW patterns after discovering reusable approaches
- **`learner`** - Auto-discover patterns from reflexion episodes

---

## What NOT to Use This For

| Don't Record | Use Instead |
|--------------|-------------|
| New patterns you discovered | `save-pattern` |
| Swarm coordination state | claude-flow memory tools |
| Transient task/agent memory | claude-flow memory tools |
| Architecture decisions | `save-pattern` |

**Reflexion is for FEEDBACK on work done, not storing new knowledge.**
