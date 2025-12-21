# Pattern Skills → AgentDB Conversion Recommendations

## Executive Summary

Convert `get-pattern` and `save-pattern` skills from claude-flow's simple key-value memory to AgentDB's semantic vector database. This enables:

1. **Semantic search** - Find patterns by meaning, not just exact key match
2. **Learning from usage** - Track which patterns are actually helpful
3. **Auto-deprecation** - Low success patterns surface for review
4. **Consistency reinforcement** - Agents learn "how we do things here"

---

## Current State vs Proposed State

| Aspect | Current (claude-flow memory) | Proposed (AgentDB) |
|--------|------------------------------|---------------------|
| Storage | Key-value (`category:name → content`) | Vector with metadata (`embedding + structured data`) |
| Search | Exact key match or substring | Semantic similarity (find by meaning) |
| Learning | None | Tracks usage_count, success_count, confidence |
| Retrieval | Manual key lookup | AI-powered semantic search |
| Deprecation | Manual (overwrite with notice) | Auto-flagged when success_rate < threshold |
| Multi-agent | Simple namespace isolation | QUIC sync + federated learning |

---

## Recommended MCP Tools for Pattern Skills

### For `get-pattern` (Retrieval)

**Primary Tool: `mcp__agentdb__agentdb_pattern_search`**
```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "how to add a new data stream",  // Natural language query
  k: 5,                                   // Top 5 results
  threshold: 0.7,                         // Minimum similarity
  filters: {
    taskType: "development",              // Optional: filter by category
    minSuccessRate: 0.6                   // Only proven patterns
  }
})
```

**Alternative for Specific Lookup: `mcp__agentdb__agentdb_search`**
```javascript
mcp__agentdb__agentdb_search({
  query: "add stream procedure",
  k: 3,
  filters: {
    tags: ["development", "procedure"],
    session_id: "ndp-patterns"           // Use session_id as namespace
  }
})
```

### For `save-pattern` (Storage)

**Primary Tool: `mcp__agentdb__agentdb_pattern_store`**
```javascript
mcp__agentdb__agentdb_pattern_store({
  taskType: "development",                // Category
  approach: "# Add New Stream\n\n## Steps\n1. Create config...",  // Full content
  successRate: 0.9,                       // Initial confidence
  tags: ["stream", "procedure", "config"],
  metadata: {
    created_by: "agent-id",
    version: "1.0",
    related_files: ["docs/procedures/HOW_TO_ADD_NEW_STREAM.md"]
  }
})
```

**Alternative for Rich Data: `mcp__agentdb__agentdb_insert`**
```javascript
mcp__agentdb__agentdb_insert({
  text: "# Add New Stream\n\nProcedure for adding a new data stream...",
  session_id: "ndp-patterns",
  tags: ["architecture", "procedure", "streams"],
  metadata: {
    category: "development",
    pattern_name: "add-stream",
    status: "active",                     // active | deprecated | experimental
    deprecation_date: null,
    replacement: null,
    last_verified: "2025-12-21"
  }
})
```

### For Pattern Feedback (Learning)

**After Pattern Applied Successfully:**
```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Added new air-quality stream",
  input: "User asked to add air quality monitoring",
  output: "Created config/streams/air-quality/config.yaml, synced to etcd",
  reward: 1.0,                            // Success
  success: true,
  critique: "Pattern was complete, no adjustments needed"
})
```

**After Pattern Failed or Needed Adjustment:**
```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Add new stream failed",
  input: "User asked to add new stream",
  output: "Config sync failed due to missing field",
  reward: 0.3,                            // Partial failure
  success: false,
  critique: "Pattern missing required 'retention' field introduced in v2.0"
})
```

---

## Proposed Skill Structure

### `get-pattern` Skill (Updated)

```markdown
---
name: "get-pattern"
description: "Retrieve project patterns using semantic search. Use BEFORE implementing to ensure consistency."
---

# Get Pattern (AgentDB)

## When to Use
- Before implementing anything new
- When unsure "how we do things here"
- To find similar past approaches

## Quick Usage

### Method 1: Semantic Search (Recommended)
```javascript
// Find patterns by meaning
mcp__agentdb__agentdb_pattern_search({
  task: "<describe what you're trying to do>",
  k: 5,
  threshold: 0.7,
  filters: { minSuccessRate: 0.6 }
})
```

### Method 2: Direct Lookup by Category
```javascript
mcp__agentdb__agentdb_search({
  query: "<category>:<pattern-name>",
  k: 1,
  filters: { session_id: "ndp-patterns" }
})
```

## After Using a Pattern

**CRITICAL**: Record whether the pattern helped:

```javascript
// Pattern worked well
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "<what you did>",
  reward: 1.0,
  success: true,
  critique: "Pattern was accurate and complete"
})

// Pattern needed updates
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "<what you did>",
  reward: 0.5,
  success: true,
  critique: "<what was missing or wrong>"
})
```

## Pattern Categories
- `architecture` - System design, ADRs
- `development` - Implementation procedures
- `deployment` - Operational procedures
- `conventions` - Naming, style rules
- `troubleshooting` - Common issues/fixes
```

### `save-pattern` Skill (Updated)

```markdown
---
name: "save-pattern"
description: "Store or update project patterns with learning capabilities."
---

# Save Pattern (AgentDB)

## When to Use
- After discovering a reusable approach
- When a procedure has changed
- To update outdated patterns (based on low success rates)

## Quick Usage

### Store New Pattern
```javascript
mcp__agentdb__agentdb_pattern_store({
  taskType: "<category>",
  approach: "<full pattern content with steps>",
  successRate: 0.8,
  tags: ["<tag1>", "<tag2>"],
  metadata: {
    pattern_name: "<kebab-case-name>",
    created_by: "agent",
    related_files: ["<path/to/relevant/file>"]
  }
})
```

### Update Existing Pattern
Same as store - AgentDB handles versioning and tracks the new version's success separately.

### Deprecate Pattern
```javascript
mcp__agentdb__agentdb_pattern_store({
  taskType: "<category>",
  approach: "# DEPRECATED: <old-name>\n\n## Replacement\nUse <new-pattern> instead.\n\n## Reason\n<why deprecated>",
  successRate: 0.0,                       // Mark as low success
  tags: ["deprecated", "<category>"],
  metadata: {
    pattern_name: "<old-pattern-name>",
    status: "deprecated",
    replacement: "<new-pattern-name>",
    deprecation_date: "2025-12-21"
  }
})
```

## Check Pattern Health
```javascript
// See which patterns need attention
mcp__agentdb__agentdb_pattern_stats({})
// Returns: total patterns, avg success rates, patterns needing review
```
```

---

## Agent Reinforcement: Start Every Task with Pattern Check

Add this instruction to agent prompts or CLAUDE.md:

```markdown
## Pattern-First Development

**BEFORE implementing anything:**
1. Search for relevant patterns: `mcp__agentdb__agentdb_pattern_search({ task: "<what you're about to do>" })`
2. If pattern exists with successRate > 0.7: Follow it
3. If pattern exists with successRate < 0.5: Review and consider updating
4. If no pattern: Implement, then save a pattern if reusable

**AFTER completing work:**
1. If you used a pattern: Record success/failure via `reflexion_store`
2. If pattern was wrong/outdated: Update it via `save-pattern`
3. If you discovered a new approach: Save it for future agents
```

---

## Migration Path

### Phase 1: Initialize AgentDB
```bash
npx agentdb@latest init ./.agentdb/ndp-patterns.db --dimension 1536
npx agentdb@latest mcp  # Start MCP server
claude mcp add agentdb npx agentdb@latest mcp
```

### Phase 2: Migrate Existing Patterns
Export from claude-flow memory and import into AgentDB:
```bash
# Export current patterns
npx claude-flow@alpha memory export ndp-patterns-backup.json --namespace ndp-patterns

# Transform and load into AgentDB (script needed)
# For each pattern: convert to agentdb_pattern_store format
```

### Phase 3: Update Skills
Replace the skill files with the new AgentDB-based versions.

### Phase 4: Update CLAUDE.md
Add the "Pattern-First Development" section to agent instructions.

---

## Benefits Summary

| Benefit | How AgentDB Enables It |
|---------|------------------------|
| **Project consistency** | Semantic search finds relevant patterns even with different wording |
| **Swarm evolution** | Success/failure tracking improves pattern recommendations over time |
| **Auto-cleanup** | Low success_rate patterns surface for review/deprecation |
| **Cross-agent learning** | QUIC sync shares patterns instantly across agents |
| **Provenance** | `recall_with_certificate` provides audit trail |
| **Pattern discovery** | `learner_discover` auto-extracts patterns from successful episodes |

---

## Key MCP Tools Reference

| Tool | Purpose |
|------|---------|
| `agentdb_pattern_search` | Semantic search for patterns |
| `agentdb_pattern_store` | Store patterns with metadata |
| `agentdb_pattern_stats` | Health check on patterns |
| `agentdb_search` | General vector search |
| `agentdb_insert` | Store with custom structure |
| `reflexion_store` | Record experience with outcome |
| `reflexion_retrieve` | Find similar past experiences |
| `skill_create` | Store reusable procedures |
| `skill_search` | Find applicable procedures |
| `learner_discover` | Auto-discover patterns |
| `recall_with_certificate` | Retrieve with provenance |
