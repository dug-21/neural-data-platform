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
| Recording swarm coordination state | MCP memory tools |
| Transient task/agent memory | `mcp__claude-flow__memory_usage` |
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

```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used [pattern-name] pattern for [what you did]",
  input: "Pattern retrieved: [pattern description]",
  output: "Result: [what you accomplished using the pattern]",
  reward: 1.0,
  success: true,
  critique: "Pattern was complete and accurate - no adjustments needed"
})
```

### Pattern Partially Worked

When a pattern needed modifications:

```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used [pattern-name] pattern but needed adjustment",
  input: "Pattern retrieved: [pattern description]",
  output: "Completed after [what you had to change]",
  reward: 0.6,
  success: true,
  critique: "Pattern missing [specific gap] - consider updating via save-pattern"
})
```

### Pattern Failed

When a pattern was wrong or outdated:

```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Pattern [pattern-name] failed",
  input: "Pattern retrieved: [pattern description]",
  output: "Failed because [what went wrong]",
  reward: 0.2,
  success: false,
  critique: "Pattern is outdated/wrong - [specific issue]. Needs update via save-pattern"
})
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
- Swarm/coordination state (use MCP memory tools)

---

## After Recording Feedback

If your critique identifies a pattern that needs updating:

```javascript
// 1. Record the feedback (this skill)
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used add-stream pattern",
  input: "Pattern for adding new data streams",
  output: "Completed after adding retention field",
  reward: 0.6,
  success: true,
  critique: "Pattern missing required 'retention' field - needs update"
})

// 2. Update the pattern (save-pattern skill)
mcp__agentdb__agentdb_pattern_store({
  taskType: "development",
  approach: "# Add New Data Stream (Updated)\n\n...updated content...",
  successRate: 0.85,
  tags: ["stream", "procedure"],
  metadata: { pattern_name: "add-stream", version: "2.0" }
})
```

---

## Causal Learning

For outcomes that establish cause-effect relationships:

```javascript
mcp__agentdb__causal_add_edge({
  cause: "Using batch size > 1000 for Parquet writes",
  effect: "Memory exhaustion on Raspberry Pi",
  uplift: -0.8,
  confidence: 0.95,
  sample_size: 3
})
```

---

## Check Learning Impact

See how feedback has influenced recommendations:

```javascript
mcp__agentdb__learning_metrics({
  time_window_days: 7,
  group_by: "task"
})

mcp__agentdb__agentdb_pattern_stats({})
```

---

## Related Skills

- **`get-pattern`** - Retrieve patterns BEFORE work (use this first)
- **`save-pattern`** - Store NEW patterns for architecture, procedures, conventions
- `agentdb-learning` - Advanced reinforcement learning features
- `reasoningbank-agentdb` - Adaptive learning with trajectory tracking

**NOT related to:**
- Swarm coordination memory
- Transient task state
- Agent communication
