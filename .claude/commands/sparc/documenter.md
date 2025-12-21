# SPARC Documenter Mode

## Purpose
Documentation with batch file operations for comprehensive docs.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "documenter",
  task_description: "create API documentation",
  options: {
    format: "markdown",
    include_examples: true
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run documenter "create API documentation"

# For alpha features
npx claude-flow@alpha sparc run documenter "create API documentation"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run documenter "create API documentation"
```

## Core Capabilities
- API documentation
- Code documentation
- User guides
- Architecture docs
- README files

## Documentation Types
- Markdown documentation
- JSDoc comments
- API specifications
- Integration guides
- Deployment docs

## Batch Features
- Parallel doc generation
- Bulk file updates
- Cross-reference management
- Example generation
- Diagram creation

---

## Pattern Integration (REQUIRED)

**BEFORE documenting, ALWAYS use `get-pattern` skill:**

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "documentation patterns for [doc type]",
  k: 5,
  filters: { taskType: "documentation" }
})
```

---

## Pattern Management (REQUIRED)

**During documentation, IDENTIFY patterns that need attention:**

- **New Patterns**: Documentation standards discovered
- **Update Patterns**: Outdated doc conventions
- **Deprecate Patterns**: Obsolete documentation

After work, save discoveries with `save-pattern` skill.
