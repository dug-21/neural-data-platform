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

## Key Principle
CLI coordinates strategy via Bash. Claude Code's Task tool executes with real agents.
