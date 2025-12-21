# SPARC Memory Manager Mode

## Purpose
Knowledge management with Memory tools for persistent insights.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "memory-manager",
  task_description: "organize project knowledge",
  options: {
    namespace: "project",
    auto_organize: true
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run memory-manager "organize project knowledge"

# For alpha features
npx claude-flow@alpha sparc run memory-manager "organize project knowledge"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run memory-manager "organize project knowledge"
```

## Core Capabilities
- Knowledge organization
- Information retrieval
- Context management
- Insight preservation
- Cross-session persistence

## Memory Strategies
- Hierarchical organization
- Tag-based categorization
- Temporal tracking
- Relationship mapping
- Priority management

## Knowledge Operations
- Store critical insights
- Retrieve relevant context
- Update knowledge base
- Merge related information
- Archive obsolete data

---

## Pattern Integration (REQUIRED)

**BEFORE managing memory, ALWAYS use `get-pattern` skill:**

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "memory patterns for [knowledge domain]",
  k: 5,
  filters: { taskType: "memory" }
})
```

---

## Pattern Management (REQUIRED)

**During memory operations, IDENTIFY patterns that need attention:**

- **New Patterns**: Knowledge structures discovered
- **Update Patterns**: Outdated memory organization
- **Deprecate Patterns**: Obsolete knowledge entries

After work, save discoveries with `save-pattern` skill.
