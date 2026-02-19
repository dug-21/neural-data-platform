---
name: "reflexion"
description: "Record feedback on pattern effectiveness. Stores episodes that train the recommendation system, feed the RL engine for smarter pattern ranking, build causal knowledge, and enable pattern discovery via learner."
---

# Reflexion - Evaluate Pattern Effectiveness

## What This Skill Does

Records feedback on patterns and approaches used during work. This feedback:
1. Trains the recommendation system for better pattern suggestions
2. Feeds the RL engine for smarter pattern ranking (Enhanced)
3. Builds causal knowledge graphs linking actions to outcomes (Enhanced)
4. Provides data for `learner` skill to auto-discover new patterns
5. Tracks what works and what doesn't over time

**Use this AFTER completing work** to record what helped and what didn't.

---

## Quick Reference

```
# PRIMARY: Store feedback
mcp__agentdb__reflexion_store(
  session_id="feature-id",
  task="task description",
  reward=0.9,
  success=true,
  critique="what worked or didn't"
)

# ENHANCED: Feed RL engine (if persistent session exists in auto-memory)
mcp__agentdb__learning_feedback(
  session_id="<ndp-learning-session-id>",
  state="task context",
  action="pattern or approach used",
  reward=0.9, success=true,
  next_state="outcome description"
)

# ENHANCED: Record cause-effect (only for clear causal relationships)
mcp__agentdb__causal_add_edge(
  cause="action taken",
  effect="observed outcome",
  uplift=0.8, confidence=0.9
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
1. BEFORE work:  get-pattern      → Search for relevant patterns
2. DURING work:  Apply patterns, note gaps and discoveries
3. AFTER work:   reflexion         → Record what helped (THIS SKILL)
                 + learning_feedback → Feed RL engine (Enhanced, if session exists)
                 + causal_add_edge  → Record cause-effect links (Enhanced, if clear)
                 save-pattern      → Store NEW discoveries (if any)
                 learner           → Auto-discover patterns from episodes (periodic)
                 learning_train    → Retrain RL policy (periodic, every ~10 entries)
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

## Enhanced: Feed RL Engine

After every `reflexion_store`, also feed the RL engine so it can learn which patterns lead to better outcomes. This requires a persistent learning session stored in auto-memory.

**Prerequisites**: A persistent learning session must exist. Check auto-memory at `/home/vscode/.claude/projects/-workspaces-neural-data-platform/memory/MEMORY.md` for the `ndp-learning-session-id`. If no session exists, skip this step (reflexion_store alone is sufficient).

```
# After reflexion_store, feed the same data to the RL engine
mcp__agentdb__learning_feedback(
  session_id="<persistent-session-id-from-auto-memory>",
  state="Working on dp-004: adding HTTP source adapter",
  action="Applied pattern ID 31 (domain-adapter-source-trait)",
  reward=1.0,
  success=true,
  next_state="HTTP source adapter implemented, tests passing"
)
```

### RL Feedback Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| session_id | string | Yes | Persistent session ID from auto-memory |
| state | string | Yes | Task context — what you were working on |
| action | string | Yes | Pattern used or approach taken (reference pattern ID when possible) |
| reward | number | Yes | Same reward as reflexion_store (0-1) |
| success | boolean | Yes | Same success as reflexion_store |
| next_state | string | Yes | Outcome — what resulted from the action |

### Example: Full Reflexion + RL Feedback

```
# 1. PRIMARY: Store reflexion episode
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Used domain-adapter pattern for new HTTP source",
  reward=1.0,
  success=true,
  critique="Pattern was complete - followed Source trait steps exactly, tests passed first try"
)

# 2. ENHANCED: Feed RL engine (only if persistent session exists)
mcp__agentdb__learning_feedback(
  session_id="<ndp-learning-session-id>",
  state="dp-004: implementing HTTP source adapter",
  action="Applied pattern ID 31 (domain-adapter-source-trait)",
  reward=1.0,
  success=true,
  next_state="HTTP source adapter complete, 12 tests passing"
)
```

**When to skip**: If no persistent learning session exists in auto-memory, or if the reflexion is for a deprecation/housekeeping entry that doesn't represent a real state-action-outcome sequence.

---

## Enhanced: Causal Knowledge

When a reflexion clearly identifies a cause-effect relationship (e.g., "using pattern X caused outcome Y"), also record it as a causal edge. This builds a knowledge graph that future agents can query to understand why certain patterns lead to certain outcomes.

**Only use this for clear, observable cause-effect relationships** — not every reflexion warrants a causal edge.

```
mcp__agentdb__causal_add_edge(
  cause="Applied WAL-only Bronze pattern (no accumulator)",
  effect="Memory usage dropped from 180MB to 12MB steady-state",
  uplift=0.9,
  confidence=0.95
)
```

### Causal Edge Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| cause | string | Yes | The action, pattern, or decision applied |
| effect | string | Yes | The observed outcome |
| uplift | number | Yes | Estimated improvement magnitude (0-1 scale) |
| confidence | number | No | How certain the cause-effect link is (default: 0.8) |

### When to Add Causal Edges

| Scenario | Add Edge? | Example |
|----------|-----------|---------|
| Pattern directly solved the problem | Yes | "Domain adapter pattern -> clean HTTP integration" |
| Architecture decision had measurable impact | Yes | "WAL-only hot path -> 15x memory reduction" |
| Pattern partially helped, unclear attribution | No | "Used several patterns, hard to say which helped" |
| Pattern was deprecated/harmful | Yes | "DuckDB ETL engine -> schema incompatibility failures" |
| Routine work, no notable cause-effect | No | "Standard config update, worked as expected" |

### Example: Reflexion + Causal Edge

```
# 1. PRIMARY: Store reflexion
mcp__agentdb__reflexion_store(
  session_id="ops-004",
  task="Used WAL-only Bronze pattern to fix memory leak",
  reward=1.0,
  success=true,
  critique="Removing accumulator eliminated root cause. Memory stable at 12MB vs 180MB+ before."
)

# 2. ENHANCED: Record the causal relationship
mcp__agentdb__causal_add_edge(
  cause="Removed in-memory accumulator, switched to WAL-only Bronze hot path",
  effect="RSS memory stabilized at 12MB, eliminated unbounded growth (was 180MB+)",
  uplift=0.9,
  confidence=0.95
)
```

---

## Periodic: Train RL Policy

After accumulating ~10+ reflexion entries with RL feedback, trigger a training run to update the RL policy. This improves future pattern recommendations by `get-pattern`.

```
mcp__agentdb__learning_train(
  session_id="<ndp-learning-session-id>",
  epochs=20
)
```

**When to train**:
- After completing a feature (typically 5-15 reflexion entries)
- After a batch of bug fixes with pattern feedback
- When `get-pattern` recommendations feel stale or inaccurate
- Roughly every 10+ `learning_feedback` calls

**Do not over-train**: Training after every single reflexion is wasteful. Batch the feedback and train periodically.

---

## Related Skills

- **`get-pattern`** - Search patterns BEFORE work (uses RL predictions for ranking)
- **`save-pattern`** - Store NEW patterns after discovering reusable approaches
- **`pattern-manage`** - Delete, deprecate, update, deduplicate patterns (lifecycle management)
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
