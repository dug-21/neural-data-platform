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

```
# Store feedback
mcp__agentdb__reflexion_store(
  session_id="feature-id",
  task="task description",
  reward=0.9,
  success=true,
  critique="what worked or didn't"
)

# Retrieve similar experiences
mcp__agentdb__reflexion_retrieve(
  task="search query",
  k=5,
  only_successes=true
)
```

---

## Primary Method: Store Feedback

```
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Used domain-adapter pattern for new HTTP source",
  reward=1.0,
  success=true,
  critique="Pattern was complete - followed Source trait steps exactly, tests passed first try"
)
```

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| session_id | string | Yes | Feature ID (e.g., `dp-004`, `air-011`) |
| task | string | Yes | Description of what you did |
| reward | number | Yes | Success score 0-1 |
| success | boolean | Yes | `true` or `false` |
| critique | string | No | Specific feedback (highly recommended) |
| input | string | No | Task input |
| output | string | No | Task output |
| latency_ms | number | No | Execution time in milliseconds |
| tokens | number | No | Tokens used |

---

## Examples

### Pattern Worked Well

```
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Used domain-adapter pattern for new HTTP source",
  reward=1.0,
  success=true,
  critique="Pattern was complete - followed Source trait steps exactly, tests passed first try"
)
```

### Pattern Partially Worked

```
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Used add-stream pattern but needed adjustment",
  reward=0.6,
  success=true,
  critique="Pattern missing retention field requirement added in v2.0 - should update pattern via save-pattern"
)
```

### Pattern Failed

```
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Pattern mqtt-routing failed for multi-topic subscription",
  reward=0.2,
  success=false,
  critique="Pattern assumes single topic per source - needs update for multi-topic. Used workaround with topic array."
)
```

### Pattern Deprecated

```
mcp__agentdb__reflexion_store(
  session_id="architecture-deprecation",
  task="Pattern architecture:dp-006-etl-engine (ID 40) - DuckDB as ETL engine",
  reward=0.0,
  success=false,
  critique="DEPRECATED: DuckDB has been eliminated from NDP architecture. Use direct TimescaleDB/tokio-postgres instead."
)
```

### No Pattern Found

```
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Implemented TimescaleDB continuous aggregate - no existing pattern",
  reward=0.85,
  success=true,
  critique="No pattern existed. Created new approach using hypertable + continuous_aggregate. Should save as new pattern."
)
```

---

## Retrieve Similar Experiences

```
# Find successful similar work
mcp__agentdb__reflexion_retrieve(
  task="HTTP source implementation",
  k=5,
  only_successes=true,
  min_reward=0.7
)

# Find failures to learn from
mcp__agentdb__reflexion_retrieve(
  task="MQTT configuration",
  k=5,
  only_successes=false
)

# Get synthesized summary
mcp__agentdb__reflexion_retrieve(
  task="parquet storage",
  k=10,
  synthesize_context=true
)
```

### Retrieve Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| task | string | Search query for similar experiences |
| k | number | Number of results (default: 5) |
| only_successes | boolean | Only return successful episodes |
| min_reward | number | Minimum reward threshold (0-1) |
| synthesize_context | boolean | Generate coherent summary |

---

## Reward Scale

| Score | Meaning | When to Use |
|-------|---------|-------------|
| 1.0 | Perfect | Pattern/approach worked exactly as expected |
| 0.8 | Good | Minor adjustments needed |
| 0.6 | Partial | Significant modifications required |
| 0.4 | Weak | Marginally helpful, major workarounds |
| 0.2 | Failed | Didn't work, caused issues |
| 0.0 | Harmful/Deprecated | Actively wrong, wasted time, or obsolete |

---

## Session ID Convention

Use consistent session IDs for aggregation:

| Session ID | Use For |
|------------|---------|
| `{feature-id}` | Feature work (e.g., `dp-004`, `air-011`) |
| `{feature-id}-{phase}` | Specific phase (e.g., `dp-004-spec`) |
| `maintenance` | Bug fixes, refactoring |
| `exploration` | Research, spikes, experiments |
| `architecture-deprecation` | Marking patterns as deprecated |

---

## Critique Best Practices

**Good critiques** (specific, actionable):
```
"Pattern was complete - followed steps exactly and deployment succeeded"
"Missing retention field that's now required in v2.0 schema"
"TimescaleDB connection pattern assumed localhost but we use Docker networking"
"Architecture pattern outdated - ADR-005 superseded the approach"
"DEPRECATED: DuckDB eliminated from architecture. Use tokio-postgres directly."
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

```
# 1. Record the feedback (this skill)
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Used add-stream pattern",
  reward=0.6,
  success=true,
  critique="Pattern missing required retention field"
)

# 2. Update the pattern (save-pattern skill)
mcp__agentdb__agentdb_pattern_store(
  taskType="procedure:add-stream-v2",
  approach="Add Data Stream (v2.0): Now requires retention field. Steps: 1) Create config.yaml, 2) Add retention field (required), 3) Run sync...",
  successRate=0.9,
  tags=["procedure", "streams", "config", "updated"]
)
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
