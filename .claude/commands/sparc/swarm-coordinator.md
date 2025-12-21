# SPARC Swarm Coordinator Mode

## Purpose
Specialized swarm management with batch coordination capabilities.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "swarm-coordinator",
  task_description: "manage development swarm",
  options: {
    topology: "hierarchical",
    max_agents: 10
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run swarm-coordinator "manage development swarm"

# For alpha features
npx claude-flow@alpha sparc run swarm-coordinator "manage development swarm"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run swarm-coordinator "manage development swarm"
```

## Core Capabilities
- Swarm initialization
- Agent management
- Task distribution
- Load balancing
- Result collection

## Coordination Modes
- Hierarchical swarms
- Mesh networks
- Pipeline coordination
- Adaptive strategies
- Hybrid approaches

## Management Features
- Dynamic scaling
- Resource optimization
- Failure recovery
- Performance monitoring
- Quality assurance

---

## Pattern Integration (REQUIRED)

**BEFORE coordinating swarm, ALWAYS use `get-pattern` skill:**

```javascript
mcp__agentdb__agentdb_pattern_search({
  task: "swarm coordination patterns for [task type]",
  k: 5,
  filters: { taskType: "coordination" }
})
```

---

## Pattern Management (REQUIRED)

**During coordination, IDENTIFY patterns that need attention:**

- **New Patterns**: Coordination strategies discovered
- **Update Patterns**: Outdated swarm configurations
- **Deprecate Patterns**: Obsolete coordination methods

After work, save discoveries with `save-pattern` skill.
