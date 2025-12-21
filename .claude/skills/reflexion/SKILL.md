---
name: "reflexion"
description: "Record whether patterns and approaches worked. Essential feedback that trains the system to recommend better patterns over time."
---

# Reflexion - Record What Worked

## What This Skill Does

Records the outcome of your work - whether patterns you used were helpful, approaches that succeeded or failed, and lessons learned. This feedback is **essential** for the learning system to improve recommendations over time.

**Use this AFTER completing any significant work** to train the pattern memory.

## Why This Matters

AgentDB uses reinforcement learning to recommend patterns. Without feedback:
- Good patterns never get reinforced
- Bad patterns keep getting recommended
- The system can't learn from experience

Your feedback directly improves future agent performance.

---

## Quick Usage

### Pattern Worked Well

```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Brief description of what you did",
  input: "What was requested",
  output: "What you produced/accomplished",
  reward: 1.0,
  success: true,
  critique: "Pattern was complete and accurate"
})
```

### Pattern Partially Worked

```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Brief description of what you did",
  input: "What was requested",
  output: "What you produced (with adjustments)",
  reward: 0.6,
  success: true,
  critique: "Pattern needed adjustment - [describe what was missing or outdated]"
})
```

### Pattern Failed / Approach Didn't Work

```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Brief description of what you attempted",
  input: "What was requested",
  output: "What happened (failure description)",
  reward: 0.2,
  success: false,
  critique: "Pattern failed because [reason] - needs update or replacement"
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

The `critique` field is the most valuable part - it tells future agents what to watch for.

### Good Critique Examples

```
"Pattern was complete - followed steps exactly and deployment succeeded"

"Missing retention field that's now required in v2.0 schema - pattern needs update"

"TimescaleDB connection pattern assumed localhost but we use Docker networking"

"Error handling pattern didn't cover the async timeout case we encountered"

"Architecture pattern was outdated - ADR-005 superseded the approach described"
```

### Poor Critique Examples (Avoid)

```
"It worked"           // Too vague
"Failed"              // No actionable info
"Good pattern"        // Doesn't help future agents
```

---

## When to Record Feedback

Record feedback when you've:

1. **Used a pattern from `get-pattern`** - Did it help?
2. **Completed a significant implementation** - What worked?
3. **Encountered an issue** - What went wrong?
4. **Discovered something new** - Worth remembering?
5. **Deviated from a pattern** - Why and what worked better?

---

## Recording Multiple Outcomes

If you used multiple patterns in one task, record feedback for each:

```javascript
// Feedback on architecture pattern
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used domain-adapter pattern for new source",
  input: "Add HTTP polling source",
  output: "Implemented HttpPollingSource with Source trait",
  reward: 1.0,
  success: true,
  critique: "Domain adapter pattern worked perfectly for new source type"
})

// Feedback on deployment pattern
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used Docker deployment pattern",
  input: "Deploy new source to Pi",
  output: "Deployment failed initially, fixed port mapping",
  reward: 0.6,
  success: true,
  critique: "Pattern missing port mapping for new source - added 8080:8080"
})
```

---

## Suggesting Pattern Updates

If your critique identifies a pattern that needs updating, also use `save-pattern` to fix it:

```javascript
// First, record the feedback
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used add-stream pattern",
  input: "Add new sensor stream",
  output: "Completed after adding retention field",
  reward: 0.6,
  success: true,
  critique: "Pattern missing required 'retention' field - needs update"
})

// Then, update the pattern (use save-pattern skill)
// This ensures future agents get the correct version
```

---

## Causal Learning

For outcomes that establish cause-effect relationships, also record causal edges:

```javascript
mcp__agentdb__causal_add_edge({
  cause: "Using batch size > 1000 for Parquet writes",
  effect: "Memory exhaustion on Raspberry Pi",
  uplift: -0.8,
  confidence: 0.95,
  sample_size: 3
})
```

This helps the system learn "if X, then Y" relationships.

---

## Check Your Learning Impact

See how your feedback has influenced the system:

```javascript
// View learning metrics
mcp__agentdb__learning_metrics({
  time_window_days: 7,
  group_by: "task"
})

// See pattern health
mcp__agentdb__agentdb_pattern_stats({})
```

---

## Integration with Pattern Workflow

The complete pattern workflow:

1. **BEFORE work**: Use `get-pattern` to find relevant patterns
2. **DURING work**: Note what works, what doesn't
3. **AFTER work**:
   - Use `reflexion` (this skill) to record outcomes
   - Use `save-pattern` if you discovered a new reusable approach

---

## Related Skills

- `get-pattern` - Retrieve patterns before work
- `save-pattern` - Store new patterns after discoveries
- `agentdb-learning` - Advanced reinforcement learning features
- `reasoningbank-agentdb` - Adaptive learning with trajectory tracking
