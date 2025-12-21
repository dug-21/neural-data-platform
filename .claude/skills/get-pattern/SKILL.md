---
name: "get-pattern"
description: "Retrieve project patterns using semantic search via AgentDB. Use BEFORE implementing anything to ensure consistency with established approaches."
---

# Get Pattern

## What This Skill Does

Retrieves established patterns, conventions, and architecture documentation for the Neural Data Platform using AgentDB's semantic vector search. Patterns are found by **meaning**, not just exact key match.

**Use this BEFORE implementing anything** to ensure you follow project standards.

## When to Use

- Before implementing anything new
- When unsure "how we do things here"
- To find similar past approaches
- "How do I add a new stream/source/parser?"
- "What's the architecture of this system?"
- "What are the naming conventions?"

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

After using a pattern, **always use the `reflexion` skill** to record whether it helped. This trains the system to recommend better patterns over time.

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

### Deployment Commands
```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "deployment commands and procedures",
  k: 3,
  filters: { taskType: "deployment" }
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

- `reflexion` - Record whether patterns worked (REQUIRED after using patterns)
- `save-pattern` - Store new patterns or update outdated ones
- `agentdb-memory-patterns` - Advanced memory management
- `agentdb-vector-search` - Direct vector search capabilities
