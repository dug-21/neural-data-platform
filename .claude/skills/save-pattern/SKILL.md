---
name: "save-pattern"
description: "Store, update, or deprecate project patterns in AgentDB. Use after discovering reusable approaches or when patterns become outdated."
---

# Save Pattern

## What This Skill Does

Manages the full lifecycle of project patterns in AgentDB:
- **Store** - Create new patterns with semantic embeddings
- **Update** - Replace patterns (AgentDB tracks versions)
- **Deprecate** - Mark patterns as outdated with replacement guidance
- **Delete** - Remove patterns entirely

Patterns are stored with success tracking, enabling the system to learn which patterns are actually helpful over time.

## When to Use

| Situation | Operation |
|-----------|-----------|
| Discovered a reusable approach | Store |
| Defined a new process or procedure | Store |
| Significantly changed the architecture | Update |
| Pattern procedure has changed | Update |
| Pattern replaced by better approach | Deprecate |
| Pattern is wrong/dangerous/obsolete | Delete |

---

## Quick Usage

### Store New Pattern

```javascript
mcp__agentdb__agentdb_pattern_store({
  taskType: "development",
  approach: "# Add New Data Stream\n\n## Prerequisites\n- Access to config/streams directory\n- etcd running\n\n## Steps\n1. Create config/streams/{stream-id}/config.yaml\n2. Define schema with fields (snake_case names)\n3. Add sources configuration\n4. Run ./deploy.sh sync\n5. Verify: docker exec etcd etcdctl get /streams/{id}/config\n\n## Example\nSee: config/streams/outdoor-weather/config.yaml",
  successRate: 0.9,
  tags: ["stream", "procedure", "config", "etcd"],
  metadata: {
    pattern_name: "add-stream",
    created_by: "agent",
    version: "1.0",
    related_files: ["docs/procedures/HOW_TO_ADD_NEW_STREAM.md"],
    last_verified: "2025-12-21"
  }
})
```

### Pattern Content Structure

Use this template for pattern content:

```markdown
# Pattern Name

## Context
When/why you would use this pattern.

## Prerequisites
What must be in place before starting.

## Steps
1. First step
2. Second step
3. ...

## Example
Concrete usage example or file reference.

## Verification
How to confirm it worked.

## Related
- Related patterns or files
```

---

## Pattern Categories

| Category | Use For | Example Tags |
|----------|---------|--------------|
| `architecture` | System design, ADRs, schemas | `adr`, `design`, `schema` |
| `data-flow` | Pipeline patterns, transformations | `pipeline`, `etl`, `channel` |
| `development` | Implementation procedures | `procedure`, `howto`, `coding` |
| `deployment` | Operational procedures | `docker`, `deploy`, `infrastructure` |
| `troubleshooting` | Common issues and solutions | `debug`, `fix`, `checklist` |
| `conventions` | Naming rules, style guides | `naming`, `style`, `organization` |
| `procedures` | Multi-component changes | `stream`, `source`, `parser` |
| `streams` | Active stream documentation | `mqtt`, `http`, `sensor` |

---

## Update Existing Pattern

Store with the same pattern_name - AgentDB handles versioning:

```javascript
mcp__agentdb__agentdb_pattern_store({
  taskType: "development",
  approach: "# Add New Data Stream (Updated)\n\n## Prerequisites\n...\n\n## Steps\n1. Create config...\n2. NEW: Add retention field (required since v2.0)\n3. ...",
  successRate: 0.85,
  tags: ["stream", "procedure", "config", "etcd"],
  metadata: {
    pattern_name: "add-stream",
    created_by: "agent",
    version: "2.0",
    related_files: ["docs/procedures/HOW_TO_ADD_NEW_STREAM.md"],
    last_verified: "2025-12-21",
    changelog: "Added required retention field"
  }
})
```

---

## Deprecate Pattern

When a pattern is replaced by a better approach:

```javascript
mcp__agentdb__agentdb_pattern_store({
  taskType: "deployment",
  approach: "# DEPRECATED: Manual Config Sync\n\n## Status\nDEPRECATED as of 2025-12-21\n\n## Replacement\nUse `deployment:automated-config-sync` instead.\n\n## Migration\n1. Stop using manual etcdctl put commands\n2. Edit YAML files in config/streams/\n3. Run: ./deploy.sh sync\n\n## Reason\nManual sync was error-prone and didn't validate YAML structure. Automated sync includes validation and atomic updates.",
  successRate: 0.0,
  tags: ["deprecated", "deployment", "config"],
  metadata: {
    pattern_name: "manual-config-sync",
    status: "deprecated",
    replacement: "automated-config-sync",
    deprecation_date: "2025-12-21",
    created_by: "agent"
  }
})
```

---

## Delete Pattern

For patterns that are actively harmful or completely wrong:

```javascript
mcp__agentdb__agentdb_delete({
  filters: {
    session_id: "ndp-patterns"
  }
  // Note: Prefer deprecation over deletion to preserve history
})
```

**Prefer Deprecation over Deletion** - deprecated patterns serve as historical reference and prevent others from re-discovering bad approaches.

---

## Check Pattern Health

See overall pattern statistics:

```javascript
mcp__agentdb__agentdb_pattern_stats({})
```

Returns:
- Total patterns
- Average success rates by category
- Top task types
- Patterns with low success rates (need review)

---

## Record Pattern Outcomes

When you or another agent uses a pattern, record the outcome to improve future recommendations:

### Success
```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used add-stream pattern to create weather stream",
  input: "Add weather monitoring stream",
  output: "Successfully created and synced weather stream config",
  reward: 1.0,
  success: true,
  critique: "Pattern was complete and accurate"
})
```

### Partial Success (Pattern Needed Adjustment)
```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used add-stream pattern but needed adjustment",
  input: "Add new data stream",
  output: "Completed after adding missing retention field",
  reward: 0.6,
  success: true,
  critique: "Pattern missing retention field - should be updated"
})
```

### Failure
```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "add-stream pattern failed",
  input: "Add new data stream",
  output: "Could not sync - schema completely changed",
  reward: 0.1,
  success: false,
  critique: "Pattern is obsolete - etcd schema changed in refactor"
})
```

---

## Auto-Discover Patterns

Let AgentDB find patterns from successful experiences:

```javascript
mcp__agentdb__learner_discover({
  min_attempts: 3,
  min_success_rate: 0.7,
  min_confidence: 0.8,
  dry_run: false
})
```

This analyzes past experiences and automatically creates patterns from recurring successful approaches.

---

## Alternative: Store with Full Control

For more control over the stored data:

```javascript
mcp__agentdb__agentdb_insert({
  text: "# Add New Stream\n\nComplete procedure for adding a new data stream to the Neural Data Platform...",
  session_id: "ndp-patterns",
  tags: ["development", "procedure", "streams"],
  metadata: {
    category: "development",
    pattern_name: "add-stream",
    status: "active",
    version: "1.0",
    created_by: "agent",
    related_files: ["docs/procedures/HOW_TO_ADD_NEW_STREAM.md"],
    last_verified: "2025-12-21"
  }
})
```

---

## Best Practices

1. **Be Specific** - Include concrete examples, not just theory
2. **Include Context** - Explain when/why to use the pattern
3. **Reference Files** - Link to actual code that implements the pattern
4. **Use Consistent Tags** - Category + descriptive tags (kebab-case)
5. **Deprecate Don't Delete** - Preserve knowledge unless harmful
6. **Record Outcomes** - Always log success/failure after using patterns
7. **Set Realistic Success Rates** - Start at 0.8, let usage adjust it
8. **Include Verification Steps** - How to confirm the pattern worked

---

## Related Skills

- `get-pattern` - Retrieve stored patterns
- `reflexion` - Record whether patterns worked (use before saving new patterns)
- `agentdb-memory-patterns` - Advanced memory management
- `agentdb-learning` - Reinforcement learning capabilities
- `reasoningbank-agentdb` - Adaptive learning integration
