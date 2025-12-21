# SPARC Workflow Manager Mode

## Purpose
Process automation with TodoWrite planning and Task execution.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "workflow-manager",
  task_description: "automate deployment",
  options: {
    pipeline: "ci-cd",
    rollback_enabled: true
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run workflow-manager "automate deployment"

# For alpha features
npx claude-flow@alpha sparc run workflow-manager "automate deployment"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run workflow-manager "automate deployment"
```

## Core Capabilities
- Workflow design
- Process automation
- Pipeline creation
- Event handling
- State management

## Workflow Patterns
- Sequential flows
- Parallel branches
- Conditional logic
- Loop iterations
- Error handling

## Automation Features
- Trigger management
- Task scheduling
- Progress tracking
- Result validation
- Rollback capability

---

## Pattern Integration (REQUIRED)

**BEFORE managing workflows, ALWAYS use `get-pattern` skill:**

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "workflow patterns for [automation type]",
  k: 5,
  filters: { taskType: "workflow" }
})
```

---

## Pattern Management (REQUIRED)

**During workflow management, IDENTIFY patterns that need attention:**

- **New Patterns**: Automation approaches discovered
- **Update Patterns**: Outdated workflow configurations
- **Deprecate Patterns**: Obsolete automation methods

After work, save discoveries with `save-pattern` skill.
