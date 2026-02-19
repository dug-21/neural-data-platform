---
name: "get-pattern"
description: "Retrieve APPLICATION patterns (architecture, procedures, conventions) from AgentDB using multi-signal retrieval: pattern search, causal recall, and RL predictions. Use BEFORE implementing to ensure consistency."
---

# Get Pattern - Retrieve Application Knowledge

## What This Skill Does

Retrieves established **application patterns** (architecture, procedures, conventions) for the Neural Data Platform using three complementary signals:

1. **Pattern Search** (primary) — Semantic similarity against the patterns table
2. **Recall with Certificate** (enriched) — Blends similarity + causal uplift + recency
3. **Learning Predict** (optional) — RL-based action recommendations from past episodes

**Use this BEFORE implementing anything** to ensure you follow project standards.

---

## Quick Reference

```
# 1. Search patterns by task description (primary)
mcp__agentdb__agentdb_pattern_search(task="domain adapter pattern", k=5)

# 2. Enriched recall with causal scoring (enhanced)
mcp__agentdb__recall_with_certificate(query="domain adapter pattern", k=12)

# 3. RL-recommended actions (optional, requires learning session)
mcp__agentdb__learning_predict(session_id="ndp-learning-v1", state="implementing new domain adapter")

# 4. Explainable recommendations with evidence
mcp__agentdb__learning_explain(query="domain adapter pattern", k=5)

# Get pattern statistics
mcp__agentdb__agentdb_pattern_stats()

# Fallback: search reflexion episodes
mcp__agentdb__reflexion_retrieve(task="how to add a stream", k=5, only_successes=true)
```

---

## Primary Method: Pattern Search

```
mcp__agentdb__agentdb_pattern_search(
  task="<query>",
  k=<number>,
  threshold=<0-1>,
  filters={taskType: "architecture:*", minSuccessRate: 0.8}
)
```

**CRITICAL**: The parameter is `task`, NOT `query`. Using `query` will crash. This is different from `recall_with_certificate` which uses `query`.

### Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `task` | What you're looking for (semantic search) | required |
| `k` | Number of results | 10 |
| `threshold` | Minimum similarity (0-1) | 0 |
| `filters.taskType` | Filter by category | optional |
| `filters.minSuccessRate` | Minimum success rate | optional |
| `filters.tags` | Filter by tags | optional |

### Examples

```
# Find architecture patterns
mcp__agentdb__agentdb_pattern_search(task="domain adapter pattern", k=5)

# Find deployment procedures
mcp__agentdb__agentdb_pattern_search(task="deploy to raspberry pi", k=3)

# Find naming conventions with filter
mcp__agentdb__agentdb_pattern_search(
  task="naming conventions streams fields",
  k=5,
  filters={taskType: "conventions:*"}
)

# Find high-success patterns only
mcp__agentdb__agentdb_pattern_search(
  task="mqtt configuration",
  k=5,
  filters={minSuccessRate: 0.9}
)
```

---

## Fallback Method: Reflexion Retrieve

If no patterns exist, search past experiences:

```
mcp__agentdb__reflexion_retrieve(
  task="HTTP source implementation",
  k=5,
  only_successes=true,
  min_reward=0.7
)

# Get synthesized context
mcp__agentdb__reflexion_retrieve(
  task="timescaledb schema",
  k=10,
  synthesize_context=true
)
```

### Retrieve Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `task` | string | Task description to find similar work |
| `k` | number | Number of results |
| `only_successes` | boolean | Only successful episodes |
| `min_reward` | number | Minimum success score (0-1) |
| `synthesize_context` | boolean | Generate coherent summary |

---

## Enhanced Method: Recall with Certificate

Blends three signals for richer retrieval: **similarity** (how well it matches), **causal uplift** (did using this lead to success?), and **recency** (how recent is the knowledge?).

```
mcp__agentdb__recall_with_certificate(
  query="<what you're looking for>",
  k=12,
  alpha=0.7,
  beta=0.2,
  gamma=0.1
)
```

### Parameters

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `query` | string | What you're looking for (semantic search) | required |
| `k` | number | Number of results | 12 |
| `alpha` | number | Weight for similarity (0-1) | 0.7 |
| `beta` | number | Weight for causal uplift (0-1) | 0.2 |
| `gamma` | number | Weight for recency (0-1) | 0.1 |

### When to Use

- When pattern_search returns results but you want to prioritize **proven** patterns (increase `beta`)
- When working on a recently-changed area (increase `gamma` for freshest knowledge)
- When you want a **provenance certificate** showing why each result was ranked

### Tuning Weights

| Scenario | alpha | beta | gamma |
|----------|-------|------|-------|
| Default (balanced) | 0.7 | 0.2 | 0.1 |
| Proven patterns only | 0.4 | 0.5 | 0.1 |
| Recent changes matter | 0.5 | 0.1 | 0.4 |
| Pure similarity (like pattern_search) | 1.0 | 0.0 | 0.0 |

---

## Optional Method: Learning Predict

Gets RL-based action recommendations based on what worked in past episodes. **Requires a persistent learning session** — only works after `learning_start_session` has been called and seeded with data.

```
mcp__agentdb__learning_predict(
  session_id="ndp-learning-v1",
  state="implementing Silver ETL for new weather stream"
)
```

### Parameters

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `session_id` | string | Learning session ID (see MEMORY.md for current ID) | required |
| `state` | string | Description of your current task/context | required |

### Returns

- **Recommended action** with confidence score
- **Alternative actions** ranked by expected reward
- Use alongside pattern_search results to validate your approach

### Important Notes

- If no learning session exists yet, skip this step — it will error
- The session_id is stored in auto-memory (`MEMORY.md`) once created
- This improves over time as more reflexion data feeds the RL model

---

## Optional Method: Learning Explain

Gets explainable recommendations with supporting evidence from past episodes and causal reasoning chains.

```
mcp__agentdb__learning_explain(
  query="deploying new stream to Pi",
  k=5,
  explain_depth="detailed",
  include_evidence=true,
  include_confidence=true,
  include_causal=true
)
```

### Parameters

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `query` | string | Task description to get recommendations for | required |
| `k` | number | Number of recommendations to return | 5 |
| `explain_depth` | string | Detail level: `"summary"`, `"detailed"`, or `"full"` | `"detailed"` |
| `include_evidence` | boolean | Include supporting evidence from past episodes | true |
| `include_confidence` | boolean | Include confidence scores | true |
| `include_causal` | boolean | Include causal reasoning chains | true |

### When to Use

- When you want to understand **why** an approach is recommended
- When making high-stakes decisions (architecture changes, deployment procedures)
- When pattern_search returned multiple conflicting patterns and you need to decide

---

## Pattern Categories

| Category | Example Queries |
|----------|-----------------|
| Architecture | "domain adapter pattern", "hexagonal architecture" |
| Data Flow | "ingestion pipeline", "bronze silver gold" |
| Development | "add new stream", "implement source trait" |
| Deployment | "docker deployment", "raspberry pi setup" |
| Troubleshooting | "mqtt not working", "parquet write errors" |
| Conventions | "naming conventions", "code organization" |

---

## Interpreting Results

Results from `agentdb_pattern_search` include:

| Field | Meaning |
|-------|---------|
| `ID` | Pattern identifier |
| `taskType` | Category (e.g., `architecture:domain-adapter`) |
| `Similarity` | How well it matches your query (0-1) |
| `Success Rate` | How often this pattern succeeded (0-100%) |
| `Approach` | The pattern content/description |
| `Uses` | Number of times used |

**High-value patterns**: Success Rate > 80% AND Similarity > 0.3

**Deprecated patterns**: Check reflexion episodes - patterns with reward=0.0 and success=false may be obsolete.

---

## Typical Workflow

```
# Step 1: Pattern search (primary — always do this)
mcp__agentdb__agentdb_pattern_search(task="what I'm about to implement", k=5)

# Step 2: Enriched recall (enhanced — do this for important decisions)
mcp__agentdb__recall_with_certificate(query="what I'm about to implement", k=12)
# Combines similarity + causal uplift + recency for richer ranking

# Step 3: RL prediction (optional — only if learning session exists)
mcp__agentdb__learning_predict(
  session_id="ndp-learning-v1",
  state="description of current task context"
)

# Step 4: Combine results
# - Pattern search gives you the content
# - Recall certificates show which patterns are causally proven
# - Learning predict suggests the best action based on past outcomes
# - If results conflict, prefer patterns with high causal uplift

# Step 5: If nothing found — check reflexion for past experiences
mcp__agentdb__reflexion_retrieve(task="similar task", k=5, only_successes=true)

# Step 6: After work — record feedback (reflexion skill)
mcp__agentdb__reflexion_store(
  session_id="feature-id",
  task="task description",
  reward=0.9,
  success=true,
  critique="Pattern worked well"
)

# Step 7: If you discovered something new — save it (save-pattern skill)
mcp__agentdb__agentdb_pattern_store(
  taskType="category:name",
  approach="description",
  successRate=0.9,
  tags=["tag1", "tag2"]
)
```

**Minimum viable workflow**: Steps 1 + 6 (pattern search + reflexion). Steps 2-3 are enhancements for higher-quality retrieval.

---

## CRITICAL: Record Pattern Usage

After using a pattern, **always use the `reflexion` skill** to record whether it helped:

```
# Pattern worked well
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Used domain-adapter pattern for new HTTP source",
  reward=1.0,
  success=true,
  critique="Pattern was complete - followed steps exactly, tests passed"
)

# Pattern needed fixes
mcp__agentdb__reflexion_store(
  session_id="dp-004",
  task="Used add-stream pattern but needed adjustment",
  reward=0.6,
  success=true,
  critique="Pattern missing retention field - should update via save-pattern"
)
```

Without feedback, the system can't learn which patterns work.

---

## If No Patterns Found

1. **Check pattern stats:**
   ```
   mcp__agentdb__agentdb_pattern_stats()
   ```

2. **Search reflexion episodes:**
   ```
   mcp__agentdb__reflexion_retrieve(task="your query", k=10, synthesize_context=true)
   ```

3. **Check file-based documentation:**
   - `docs/architecture/` - Architecture documents
   - `docs/procedures/` - Step-by-step procedures
   - `product/features/*/architecture/` - Feature ADRs

4. **After implementing**, store the new pattern via `save-pattern`

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Search for relevant patterns (THIS SKILL)
2. DURING work:  Apply the pattern, note what works/gaps
3. AFTER work:   reflexion    → Record if pattern helped (required)
                 save-pattern → Store NEW discoveries (if any)
                 learner      → Auto-discover patterns from episodes (periodic)
```

---

## Related Skills

- **`save-pattern`** - Store NEW patterns after discovering reusable approaches
- **`reflexion`** - Record feedback on pattern effectiveness (REQUIRED after using patterns)
- **`pattern-manage`** - Delete, deprecate, update, deduplicate patterns (lifecycle management)
- **`learner`** - Auto-discover patterns from successful episodes (user-invoked)

---

## Parameter Naming Reference

Different AgentDB tools use different parameter names for the search text. Using the wrong name causes crashes.

| Tool | Search Parameter | Other Required |
|------|-----------------|----------------|
| `agentdb_pattern_search` | **`task`** | — |
| `recall_with_certificate` | **`query`** | — |
| `learning_predict` | **`state`** | `session_id` |
| `learning_explain` | **`query`** | — |
| `reflexion_retrieve` | **`task`** | — |

---

## What NOT to Use This For

| Don't Search For | Use Instead |
|------------------|-------------|
| Current swarm status | claude-flow swarm tools |
| Agent task state | claude-flow task tools |
| Working memory | claude-flow memory tools |
| Session context | claude-flow memory with TTL |

**Patterns are PERMANENT application knowledge, not transient swarm state.**
