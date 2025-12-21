# SPARC Analyzer Mode

## Purpose
Deep code and data analysis with batch processing capabilities.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "analyzer",
  task_description: "analyze codebase performance",
  options: {
    parallel: true,
    detailed: true
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run analyzer "analyze codebase performance"

# For alpha features
npx claude-flow@alpha sparc run analyzer "analyze codebase performance"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run analyzer "analyze codebase performance"
```

## Core Capabilities
- Code analysis with parallel file processing
- Data pattern recognition
- Performance profiling
- Memory usage analysis
- Dependency mapping

## Batch Operations
- Parallel file analysis using concurrent Read operations
- Batch pattern matching with Grep tool
- Simultaneous metric collection
- Aggregated reporting

## Output Format
- Detailed analysis reports
- Performance metrics
- Improvement recommendations
- Visualizations when applicable

---

## Pattern Integration (REQUIRED)

**BEFORE starting analysis, ALWAYS use `get-pattern` skill:**

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "analysis patterns for [your task]",
  k: 5,
  filters: { taskType: "analysis" }
})
```

---

## Pattern Management (REQUIRED)

**During analysis, IDENTIFY patterns that need attention:**

- **New Patterns**: Analysis approaches worth documenting
- **Update Patterns**: Outdated metrics or thresholds
- **Deprecate Patterns**: Obsolete analysis methods

After work, save discoveries with `save-pattern` skill.