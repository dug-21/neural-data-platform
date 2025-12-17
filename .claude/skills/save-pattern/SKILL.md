---
name: "save-pattern"
description: "Manage project patterns: create, update, deprecate, or delete patterns in the knowledge base. Use after discovering reusable approaches or when patterns become stale."
---

# Save Pattern

## What This Skill Does

Manages the full lifecycle of project patterns in memory:
- **Store** - Create new patterns
- **Update** - Replace existing patterns with new content
- **Deprecate** - Mark patterns as outdated with migration guidance
- **Delete** - Remove patterns entirely

## When to Use

| Situation | Operation |
|-----------|-----------|
| Discovered a reusable approach | Store |
| Defined a new process or procedure | Store |
| Significantly changed the architecture | Update |
| Pattern procedure has changed | Update |
| Pattern replaced by better approach | Deprecate |
| Pattern is wrong/dangerous/obsolete | Delete |

## Pattern Hierarchy (ndp-patterns namespace)

```
ndp-patterns
├── architecture/      # ADRs, design patterns, schemas, component relationships
├── data-flow/         # Pipeline patterns, channel patterns, storage flow
├── development/       # How to add streams, sources, parsers
├── deployment/        # Docker, Pi deployment, config sync
├── troubleshooting/   # Checklists, common issues and fixes
├── conventions/       # Naming rules, error handling, organization
├── procedures/        # Common requests requiring changes across comopnents (ex: How to add a stream)
└── streams/           # Documentation of active data streams
```

## Pattern Categories

| Category | Use For |
|----------|---------|
| `architecture` | System design decisions, ADRs, traits, schemas |
| `data-flow` | Pipeline patterns, data transformation approaches |
| `development` | Implementation procedures, coding patterns |
| `deployment` | Operational procedures, infrastructure patterns |
| `troubleshooting` | Checklists, common issues and solutions |
| `conventions` | Naming rules, style guides, organization |
| `procedures` | Common requests requiring changes across comopnents (ex: How to add a stream) |
| `streams` | Documentation of active data streams |

---

## Operations

### 1. Store (Create New Pattern)

Use when you've discovered or implemented something reusable.

#### CLI Method:

```bash
claude-flow memory store "<category>:<pattern-name>" "<pattern-content>" --namespace ndp-patterns
```

#### MCP Method (Preferred for Agents):

```javascript
mcp__claude-flow__memory_usage({
  action: "store",
  key: "<category>:<pattern-name>",
  value: "<pattern-content>",
  namespace: "ndp-patterns",
  ttl: 0  // permanent (0 = no expiration)
})
```

#### Pattern Content Structure:

```
# Pattern Name

## Context
When/why you would use this pattern.

## Problem
What problem does this solve?

## Solution
The actual pattern/procedure.

## Example
Concrete usage example.

## Related
- Related patterns or files
```

#### Example - Store a new pattern:

```bash
claude-flow memory store "development:api-retry-pattern" "# API Retry Pattern

## Context
When calling external APIs that may be unreliable.

## Problem
External APIs can fail transiently (rate limits, timeouts, network issues).

## Solution
Exponential backoff with jitter:
- Initial delay: 1 second
- Max delay: 60 seconds
- Max retries: 5
- Jitter: 0-500ms random

## Example
See: core/src/sources/http_poll.rs

## Related
- development:add-source
- data-flow:pipeline" --namespace ndp-patterns
```

---

### 2. Update (Replace Existing Pattern)

Use when a pattern's content has changed but it's still the right pattern.

Same command as Store - memory overwrites existing keys:

```bash
claude-flow memory store "development:add-stream" "<updated-content>" --namespace ndp-patterns
```

Or via MCP:

```javascript
mcp__claude-flow__memory_usage({
  action: "store",
  key: "development:add-stream",
  value: "<updated-content>",
  namespace: "ndp-patterns"
})
```

**When to Update vs Deprecate:**
- **Update**: Same pattern, new/corrected content (e.g., fixed a step, added detail)
- **Deprecate**: Pattern itself is being replaced by a different approach

---

### 3. Deprecate (Soft Delete with Migration)

Use when a pattern is being replaced by a better approach. Preserves history and guides users to the new pattern.

Store a deprecation notice over the old pattern:

```bash
claude-flow memory store "<category>:<old-pattern-name>" "# DEPRECATED: <Old Pattern Name>

## Status
DEPRECATED as of <date>

## Replacement
Use <category>:<new-pattern-name> instead.

## Migration
<Steps to migrate from old to new approach>

## Reason
<Why this pattern was deprecated>

## Original Pattern (for reference)
<Original content, summarized>" --namespace ndp-patterns
```

#### Example - Deprecate a pattern:

```bash
claude-flow memory store "deployment:manual-config-sync" "# DEPRECATED: Manual Config Sync

## Status
DEPRECATED as of 2025-12-17

## Replacement
Use deployment:config-sync instead (automated via deploy.sh sync).

## Migration
1. Stop using manual etcdctl put commands
2. Edit YAML files in config/streams/
3. Run: ./deploy.sh sync

## Reason
Manual sync was error-prone and didn't validate YAML structure.
Automated sync includes validation and atomic updates." --namespace ndp-patterns
```

---

### 4. Delete (Hard Remove)

Use when a pattern is:
- Completely wrong or dangerous
- Superseded AND no longer useful for reference
- Created by mistake

#### CLI Method:

```bash
# Clear entire namespace (use with caution!)
claude-flow memory clear --namespace ndp-patterns

# For single key deletion, store empty or use MCP
```

#### MCP Method:

```javascript
mcp__claude-flow__memory_usage({
  action: "delete",
  key: "<category>:<pattern-name>",
  namespace: "ndp-patterns"
})
```

**Prefer Deprecation over Deletion** unless the pattern is actively harmful. Deprecated patterns serve as historical reference and prevent others from re-discovering "bad" approaches.

---

## Updating the Pattern Index

After modifying patterns, update `.claude/patterns/INDEX.yaml` to keep it in sync:

**For new patterns:** Add entry under appropriate category

**For deprecated patterns:** Add `deprecated: true` flag:
```yaml
development:
  old-pattern:
    key: "development:old-pattern"
    deprecated: true
    replacement: "development:new-pattern"
    summary: "DEPRECATED - use new-pattern instead"
```

**For deleted patterns:** Remove from INDEX.yaml entirely

---

## Batch Operations

### Export All Patterns (Backup)

```bash
claude-flow memory export ndp-patterns-backup.json --namespace ndp-patterns
```

### Import Patterns (Restore)

```bash
claude-flow memory import ndp-patterns-backup.json --namespace ndp-patterns
```

### List All Patterns

```bash
claude-flow memory list --namespace ndp-patterns
```

Or via MCP:

```javascript
mcp__claude-flow__memory_usage({
  action: "list",
  namespace: "ndp-patterns"
})
```

---

## Verification

After any operation, verify the result:

```bash
# Check pattern exists/content
claude-flow memory query "<category>:<pattern-name>" --namespace ndp-patterns
```

Or via MCP:

```javascript
mcp__claude-flow__memory_usage({
  action: "retrieve",
  key: "<category>:<pattern-name>",
  namespace: "ndp-patterns"
})
```

---

## Best Practices

1. **Be Specific** - Include concrete examples, not just theory
2. **Include Context** - Explain when/why to use the pattern
3. **Reference Files** - Link to actual code that implements the pattern
4. **Use Consistent Keys** - `category:pattern-name` format (kebab-case)
5. **Deprecate Don't Delete** - Preserve knowledge unless harmful
6. **Update INDEX.yaml** - Keep the index in sync with memory
7. **Date Deprecations** - Include when pattern was deprecated

---

## Related Skills

- `get-pattern` - Retrieve stored patterns
- `sparc-methodology` - Development workflow that generates patterns
