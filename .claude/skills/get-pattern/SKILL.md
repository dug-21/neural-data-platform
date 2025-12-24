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
| Current swarm status | claude-flow swarm tools |
| Agent task state | claude-flow task tools |
| Working memory | claude-flow memory tools |
| Session context | claude-flow memory with TTL |

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

```bash
npx agentdb query --query "how to add a new data stream" --k 5 --min-confidence 0.7
```

**Examples:**
```bash
# Find architecture patterns
npx agentdb query --query "data pipeline architecture" --k 3

# Find deployment procedures
npx agentdb query --query "deploy to raspberry pi" --domain "deployment" --k 3

# Find naming conventions
npx agentdb query --query "naming conventions for streams and fields" --k 2

# Get synthesized context with patterns and insights
npx agentdb query --query "domain adapter pattern" --k 5 --synthesize-context
```

### Method 2: Category-Filtered Search

When you know the domain or need filtered results:

```bash
# Filter by domain
npx agentdb query --query "add new data source" --domain "development" --k 3

# Use metadata filters (MongoDB-style)
npx agentdb query --query "add new data source" --filters '{"metadata.category":"procedure"}' --k 3
```

### Method 3: Find Similar Past Experiences

See how similar tasks were handled before:

```bash
# Retrieve successful episodes
npx agentdb reflexion retrieve "adding a new MQTT data source" --k 5 --only-successes

# Get synthesized context from past experiences
npx agentdb reflexion retrieve "MQTT data source" --k 5 --synthesize-context
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

```bash
# Pattern worked well
npx agentdb reflexion store "ndp-patterns" \
  "Used [pattern-name] for [what you did]" \
  1.0 true \
  "Pattern was accurate and complete"

# Pattern needed fixes
npx agentdb reflexion store "ndp-patterns" \
  "Used [pattern-name] but needed adjustment" \
  0.6 true \
  "Pattern missing [gap] - should update via save-pattern"
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
```bash
npx agentdb query --query "add new data stream to the platform" --k 3
```

### Understanding the Architecture
```bash
npx agentdb query --query "system architecture and component relationships" --k 5 --synthesize-context
```

### Naming Conventions
```bash
npx agentdb query --query "naming conventions for streams fields and modules" --k 3
```

### Creating Grafana Dashboards
```bash
npx agentdb query --query "grafana dashboard creation with DuckDB" --domain "procedures" --k 3
```

---

## Check Pattern Health

See which patterns exist and their success rates:

```bash
npx agentdb db stats
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

**NOT related to:**
- Swarm coordination (use claude-flow tools)
- Transient task memory (use claude-flow memory with TTL)
- Agent state management (use claude-flow tools)
