---
name: "get-pattern"
description: "Retrieve APPLICATION patterns (architecture, procedures, conventions) via semantic search. Use BEFORE implementing to ensure consistency."
---

# Get Pattern - Retrieve Application Knowledge

## What This Skill Does

Retrieves established **application patterns** (architecture, procedures, conventions) for the Neural Data Platform using AgentDB's semantic vector search. Patterns are found by **meaning**, not just exact key match.

**Use this BEFORE implementing anything** to ensure you follow project standards.

## CRITICAL: What This Skill IS and IS NOT For

### USE FOR (Application Knowledge)

| Type | Example Queries |
|------|-----------------|
| **Architecture** | "domain adapter pattern", "data pipeline architecture" |
| **Procedures** | "how to add a new stream", "deployment commands" |
| **Conventions** | "naming conventions", "code organization" |
| **Troubleshooting** | "common build errors", "debugging checklist" |

### DO NOT USE FOR (Transient/Swarm Memory)

| Type | Use Instead |
|------|-------------|
| Current swarm status | `mcp__claude-flow__swarm_status` |
| Agent task state | `mcp__claude-flow__task_status` |
| Working memory | `mcp__claude-flow__memory_usage` |
| Session context | MCP memory tools |

**Patterns are PERMANENT application knowledge, not transient swarm state.**

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Retrieve relevant patterns (THIS SKILL)
2. DURING work:  Apply the pattern, note what works
3. AFTER work:   reflexion    → Did get-pattern results help?
                 save-pattern → Store NEW discoveries (if any)
```

---

## Quick Usage

### Method 1: Semantic Search (Recommended)

Find patterns by describing what you're trying to do:

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "how to add a new data stream",
  k: 5,
  threshold: 0.7,
  filters: { minSuccessRate: 0.6 }
})
```

**Examples:**
```javascript
// Find architecture patterns
mcp__agentdb__agentdb_pattern_search({
  task: "data pipeline architecture",
  k: 3
})

// Find deployment procedures
mcp__agentdb__agentdb_pattern_search({
  task: "deploy to raspberry pi",
  k: 3,
  filters: { taskType: "deployment" }
})

// Find naming conventions
mcp__agentdb__agentdb_pattern_search({
  task: "naming conventions for streams and fields",
  k: 2
})
```

### Method 2: Category-Filtered Search

When you know the category:

```javascript
mcp__agentdb__agentdb_search({
  query: "add new data source",
  k: 3,
  filters: {
    tags: ["development", "procedure"],
    session_id: "ndp-patterns"
  }
})
```

### Method 3: Find Similar Past Experiences

See how similar tasks were handled before:

```javascript
mcp__agentdb__reflexion_retrieve({
  task: "adding a new MQTT data source",
  k: 5,
  only_successes: true
})
```

---

## Pattern Categories

| Category | What It Contains |
|----------|------------------|
| `architecture` | System design decisions, ADRs, traits, schemas |
| `data-flow` | Pipeline patterns, data transformation approaches |
| `development` | Implementation procedures, step-by-step guides |
| `deployment` | Operational procedures, infrastructure patterns |
| `troubleshooting` | Checklists, common issues and solutions |
| `conventions` | Naming rules, style guides, organization |
| `procedures` | Multi-component changes (e.g., "How to add a stream") |
| `streams` | Documentation of active data streams |

---

## CRITICAL: Record Pattern Usage

After using a pattern, **always use the `reflexion` skill** to record whether it helped:

```javascript
// Pattern worked well
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used [pattern-name] for [what you did]",
  input: "Pattern retrieved: [description]",
  output: "Result: [what you accomplished]",
  reward: 1.0,
  success: true,
  critique: "Pattern was accurate and complete"
})

// Pattern needed fixes
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used [pattern-name] but needed adjustment",
  input: "Pattern retrieved: [description]",
  output: "Completed after [modifications]",
  reward: 0.6,
  success: true,
  critique: "Pattern missing [gap] - should update via save-pattern"
})
```

Without feedback, the system can't learn which patterns actually work.

---

## Key Architecture Documents (File References)

When patterns reference files, check these locations:

| Document | Path |
|----------|------|
| Platform Architecture | `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` |
| C4 System Diagram | `docs/architecture/diagrams/neural-data-platform-c4.drawio` |
| Component Dependencies | `docs/architecture/COMPONENT_DEPENDENCY_MAP.md` |
| Add New Stream | `docs/procedures/HOW_TO_ADD_NEW_STREAM.md` |
| Add New Source | `docs/procedures/HOW_TO_ADD_NEW_SOURCE.md` |

---

## Common Pattern Lookups

### Adding a New Data Stream
```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "add new data stream to the platform",
  k: 3
})
```

### Understanding the Architecture
```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "system architecture and component relationships",
  k: 5
})
```

### Naming Conventions
```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "naming conventions for streams fields and modules",
  k: 3
})
```

### Creating Grafana Dashboards
```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "grafana dashboard creation with DuckDB",
  k: 3,
  filters: { taskType: "procedures" }
})
```

---

## Check Pattern Health

See which patterns exist and their success rates:

```javascript
mcp__agentdb__agentdb_pattern_stats({})
```

Returns: total patterns, average success rates, top task types, patterns needing review.

---

## If Pattern Not Found

If no relevant pattern exists:

1. Implement the solution
2. If the approach is reusable, use `save-pattern` skill to store it
3. Future agents will benefit from your discovery

---

## Related Skills

- **`save-pattern`** - Store NEW patterns for architecture, procedures, conventions
- **`reflexion`** - Record feedback on whether `get-pattern` results helped (REQUIRED after using patterns)
- `agentdb-memory-patterns` - Advanced memory management
- `agentdb-vector-search` - Direct vector search capabilities

**NOT related to:**
- Swarm coordination (use claude-flow MCP tools)
- Transient task memory (use MCP memory with TTL)
- Agent state management (use claude-flow tools)
