# Claude-Flow Memory Commands Reference

Use claude-flow memory for **transient** swarm coordination and session state. For permanent knowledge, use AgentDB patterns (get-pattern/save-pattern skills).

## Store
```bash
# REQUIRED: --key and --value
# OPTIONAL: --namespace (default: "default"), --ttl, --tags
claude-flow memory store --key "pattern-auth" --value "JWT with refresh tokens" --namespace patterns
claude-flow memory store --key "bug-fix-123" --value "Fixed null check" --namespace solutions --tags "bugfix,auth"
```

## Search (semantic vector search)
```bash
# REQUIRED: --query (full flag, not -q)
# OPTIONAL: --namespace, --limit, --threshold
claude-flow memory search --query "authentication patterns"
claude-flow memory search --query "error handling" --namespace patterns --limit 5
```

## List
```bash
claude-flow memory list
claude-flow memory list --namespace patterns --limit 10
```

## Retrieve
```bash
# REQUIRED: --key
# OPTIONAL: --namespace
claude-flow memory retrieve --key "pattern-auth"
claude-flow memory retrieve --key "pattern-auth" --namespace patterns
```

## Initialize
```bash
claude-flow memory init --force --verbose
```

## Swarm Coordination Key Convention

When agents participate in a swarm, they use `namespace: "coordination"` with this key structure:

```
swarm/{agent-id}/status      ← agent writes on start
swarm/{agent-id}/progress    ← agent writes after each major step
swarm/{agent-id}/complete    ← agent writes before returning
swarm/shared/{feature}-context ← coordinator seeds, agents read
```

Agents automatically follow this convention when spawned with `Your agent ID: <id>` in their prompt (see agent definitions' `## Swarm Coordination` section).

## MCP Tools (preferred over CLI for agents)

Spawned agents should use MCP tools directly (via ToolSearch), not CLI:

```
mcp__claude-flow__memory_store(key, value, namespace)
mcp__claude-flow__memory_retrieve(key, namespace)
mcp__claude-flow__memory_search(query, namespace, limit)
```

## Key Principle
CLI coordinates strategy via Bash. Claude Code's Task tool executes with real agents.
