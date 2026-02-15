---
name: "learner"
description: "Auto-discover patterns from reflexion episodes. Run post-feature to consolidate successful approaches into reusable patterns."
---

# Learner - Auto-Discover Patterns

## What This Skill Does

Analyzes reflexion episodes to automatically discover:
1. **Causal patterns** - What actions lead to successful outcomes
2. **Reusable patterns** - Stored to the **patterns table** via `agentdb_pattern_store`
3. **Patterns needing review** - Low-performing or conflicting patterns

**Run this AFTER completing a feature** to consolidate learnings.

> **Note**: For manual pattern storage, use `save-pattern` skill.
> This skill uses MCP tools exclusively. All discovered knowledge goes to the **patterns** table (searchable via `get-pattern`).

---

## Quick Reference

```
# Discover causal patterns from episodes
mcp__agentdb__learner_discover(min_attempts=3, min_success_rate=0.6, min_confidence=0.7)

# Query causal edges
mcp__agentdb__causal_query(min_confidence=0.5, limit=10)

# View database statistics
mcp__agentdb__agentdb_stats(detailed=true)

# Search discovered patterns
mcp__agentdb__agentdb_pattern_search(task="feature-topic", k=5)
```

---

## Step 1: Discover Causal Patterns

Auto-discover causal patterns from reflexion episodes:

```
mcp__agentdb__learner_discover(
  min_attempts=3,
  min_success_rate=0.6,
  min_confidence=0.7,
  dry_run=false
)
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `min_attempts` | number | 3 | Minimum times pattern was tried |
| `min_success_rate` | number | 0.6 | Minimum success rate |
| `min_confidence` | number | 0.7 | Statistical confidence threshold |
| `dry_run` | boolean | false | Preview without storing |

### Examples

**Standard discovery:**
```
mcp__agentdb__learner_discover(min_attempts=3, min_success_rate=0.6, min_confidence=0.7)
```

**Aggressive (more patterns, lower thresholds):**
```
mcp__agentdb__learner_discover(min_attempts=2, min_success_rate=0.5, min_confidence=0.6)
```

**Conservative (fewer, higher-confidence patterns):**
```
mcp__agentdb__learner_discover(min_attempts=5, min_success_rate=0.8, min_confidence=0.9)
```

**Dry run (preview without storing):**
```
mcp__agentdb__learner_discover(min_attempts=3, min_success_rate=0.6, min_confidence=0.7, dry_run=true)
```

---

## Step 2: Store Discovered Patterns

After `learner_discover` returns results, store high-value discoveries to the **patterns table** so `get-pattern` can find them:

```
mcp__agentdb__agentdb_pattern_store(
  taskType="learner:discovered-pattern-name",
  approach="Description of the discovered pattern, including cause-effect relationship and how to apply it",
  successRate=0.85,
  tags=["learner", "auto-discovered", "topic"]
)
```

This replaces the legacy `agentdb skill consolidate` command, which wrote to the skills table (orphaned from `get-pattern` searches).

---

## Step 3: Query What Was Learned

### View Causal Edges

```
mcp__agentdb__causal_query(min_confidence=0.5, limit=10)
```

With filters:
```
# Filter by cause
mcp__agentdb__causal_query(cause="Source trait", min_confidence=0.7, min_uplift=0.1, limit=20)

# Filter by effect
mcp__agentdb__causal_query(effect="data ingestion", min_confidence=0.8, limit=10)
```

### Search Patterns (preferred)

```
mcp__agentdb__agentdb_pattern_search(task="data ingestion", k=5)
```

### Search Legacy Skills (read-only)

```
mcp__agentdb__skill_search(task="data ingestion", k=5)
```

### View Database Stats

```
mcp__agentdb__agentdb_stats(detailed=true)
```

---

## Prune Low-Quality Data (CLI Only)

These operations have no MCP equivalent. Use CLI when needed:

```bash
# Remove episodes older than 90 days with reward < 0.5
agentdb reflexion prune 90 0.5

# Remove low-confidence causal edges
agentdb learner prune 0.5 0.05 90
```

---

## Post-Feature Workflow

Run after completing a feature:

```
# 1. Discover causal patterns
mcp__agentdb__learner_discover(min_attempts=3, min_success_rate=0.7, min_confidence=0.8)

# 2. Review results, then store valuable patterns to patterns table
mcp__agentdb__agentdb_pattern_store(
  taskType="learner:pattern-name",
  approach="What was discovered and how to apply it",
  successRate=0.85,
  tags=["learner", "auto-discovered"]
)

# 3. View what was learned
mcp__agentdb__agentdb_stats(detailed=true)

# 4. Search patterns to verify they're findable
mcp__agentdb__agentdb_pattern_search(task="feature-topic", k=5)
```

---

## Understanding Results

### Causal Edges (from learner_discover)

```
Cause: "Using Source trait with health_check"
Effect: "Reliable data ingestion with automatic recovery"
Uplift: 0.35 (35% improvement)
Confidence: 0.92
```

### Stored Patterns (from agentdb_pattern_store)

```
taskType: "learner:http-source-reliability"
approach: "Implementing health_check on Source trait leads to 35% more reliable ingestion..."
successRate: 0.89
tags: ["learner", "auto-discovered", "source-trait"]
```

---

## Thresholds Guide

| Parameter | Low (exploratory) | Standard | High (conservative) |
|-----------|-------------------|----------|---------------------|
| `min_attempts` | 2 | 3 | 5 |
| `min_success_rate` | 0.5 | 0.7 | 0.9 |
| `min_confidence` | 0.6 | 0.8 | 0.95 |

---

## Maintenance Schedule

| Frequency | Action | Tool |
|-----------|--------|------|
| **Post-feature** | Discover patterns | `mcp__agentdb__learner_discover` |
| **Post-feature** | Store to patterns table | `mcp__agentdb__agentdb_pattern_store` |
| **Monthly** | Review stats | `mcp__agentdb__agentdb_stats` |
| **Quarterly** | Prune stale data | `agentdb reflexion prune` (CLI) |

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Search for relevant patterns
2. DURING work:  Apply patterns, note gaps
3. AFTER work:   reflexion    → Record what helped
                 save-pattern → Store NEW discoveries manually
                 learner      → Auto-discover patterns (THIS SKILL)
```

---

## Related Skills

- **`get-pattern`** - Search patterns BEFORE work
- **`save-pattern`** - Store NEW patterns manually
- **`reflexion`** - Record feedback that feeds learner

---

## What NOT to Use This For

| Don't Use For | Use Instead |
|---------------|-------------|
| Storing specific patterns | `save-pattern` |
| Recording work feedback | `reflexion` |
| Searching patterns | `get-pattern` |

**Learner is for AUTOMATIC discovery, not manual pattern management.**
