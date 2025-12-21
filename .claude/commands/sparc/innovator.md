# SPARC Innovator Mode

## Purpose
Creative problem solving with WebSearch and Memory integration.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "innovator",
  task_description: "innovative solutions for scaling",
  options: {
    research_depth: "comprehensive",
    creativity_level: "high"
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run innovator "innovative solutions for scaling"

# For alpha features
npx claude-flow@alpha sparc run innovator "innovative solutions for scaling"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run innovator "innovative solutions for scaling"
```

## Core Capabilities
- Creative ideation
- Solution brainstorming
- Technology exploration
- Pattern innovation
- Proof of concept

## Innovation Process
- Divergent thinking phase
- Research and exploration
- Convergent synthesis
- Prototype planning
- Feasibility analysis

## Knowledge Sources
- WebSearch for trends
- Memory for context
- Cross-domain insights
- Pattern recognition
- Analogical reasoning

---

## Pattern Integration (REQUIRED)

**BEFORE innovating, ALWAYS use `get-pattern` skill:**

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "innovation patterns for [your domain]",
  k: 5,
  filters: { taskType: "innovation" }
})
```

---

## Pattern Management (REQUIRED)

**During innovation, IDENTIFY patterns that need attention:**

- **New Patterns**: Novel approaches worth documenting
- **Update Patterns**: Existing patterns to evolve
- **Deprecate Patterns**: Old approaches being replaced

After work, save discoveries with `save-pattern` skill.
