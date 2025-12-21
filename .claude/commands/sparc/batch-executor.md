# SPARC Batch Executor Mode

## Purpose
Parallel task execution specialist using batch operations.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "batch-executor",
  task_description: "process multiple files",
  options: {
    parallel: true,
    batch_size: 10
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run batch-executor "process multiple files"

# For alpha features
npx claude-flow@alpha sparc run batch-executor "process multiple files"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run batch-executor "process multiple files"
```

## Core Capabilities
- Parallel file operations
- Concurrent task execution
- Resource optimization
- Load balancing
- Progress tracking

## Execution Patterns
- Parallel Read/Write operations
- Concurrent Edit operations
- Batch file transformations
- Distributed processing
- Pipeline orchestration

## Performance Features
- Dynamic resource allocation
- Automatic load balancing
- Progress monitoring
- Error recovery
- Result aggregation

---

## Pattern Integration (REQUIRED)

**BEFORE batch execution, ALWAYS use `get-pattern` skill:**

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "batch execution patterns for [operation type]",
  k: 5,
  filters: { taskType: "batch" }
})
```

---

## Pattern Management (REQUIRED)

**During execution, IDENTIFY patterns that need attention:**

- **New Patterns**: Batch strategies discovered
- **Update Patterns**: Outdated batch configurations
- **Deprecate Patterns**: Obsolete execution methods

After work, save discoveries with `save-pattern` skill.
