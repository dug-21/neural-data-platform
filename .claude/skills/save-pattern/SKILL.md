---
name: "save-pattern"
description: "Store APPLICATION patterns (architecture, procedures, conventions) in AgentDB's patterns table. NOT for swarm/transient memory."
---

# Save Pattern - Store Application Knowledge

## What This Skill Does

Stores **application patterns** to AgentDB's **patterns table** with semantic embeddings. Patterns are searchable via `get-pattern` using `agentdb_pattern_search`.

**Use this AFTER completing work** to share reusable knowledge with future agents.

---

## Quick Reference

```
# Store a new pattern
mcp__agentdb__agentdb_pattern_store(
  taskType="architecture:domain-adapter",
  approach="Description of the pattern and how to apply it...",
  successRate=0.9,
  tags=["architecture", "rust", "ndp"]
)

# Check existing patterns first (avoid duplicates)
mcp__agentdb__agentdb_pattern_search(task="pattern topic", k=3)

# Get pattern statistics
mcp__agentdb__agentdb_pattern_stats()
```

---

## Primary Method: Pattern Store

```
mcp__agentdb__agentdb_pattern_store(
  taskType="<category:name>",
  approach="<full pattern description>",
  successRate=<0-1>,
  tags=["tag1", "tag2"]
)
```

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `taskType` | string | Yes | Category and name (e.g., `architecture:domain-adapter`) |
| `approach` | string | Yes | Full pattern content - what it does, how to use it |
| `successRate` | number | No | Confidence level 0-1 (default: 0.9 for proven patterns) |
| `tags` | array | No | Array of tags for filtering |

---

## Examples

### Store Architecture Pattern

```
mcp__agentdb__agentdb_pattern_store(
  taskType="architecture:domain-adapter-source",
  approach="Domain Adapter Pattern for Data Sources: All data sources implement the Source trait for uniform handling. Steps: 1) Create struct implementing Source trait, 2) Implement fetch() -> Vec<TimeSeriesPoint>, 3) Implement health_check() -> HealthStatus. Related files: core/src/traits.rs, core/src/sources/http_poll.rs",
  successRate=0.95,
  tags=["architecture", "hexagonal", "traits", "source"]
)
```

### Store Development Procedure

```
mcp__agentdb__agentdb_pattern_store(
  taskType="procedure:add-data-stream",
  approach="Add New Data Stream: Prerequisites - Stream config YAML ready, etcd running. Steps: 1) Create config/base/streams/{stream-id}/config.yaml, 2) Define fields array with name, source_path, unit, 3) Run ./deploy.sh sync, 4) Verify: etcdctl get /streams/{id}/config",
  successRate=0.9,
  tags=["procedure", "streams", "config", "etcd"]
)
```

### Store Troubleshooting Pattern

```
mcp__agentdb__agentdb_pattern_store(
  taskType="troubleshoot:mqtt-data-not-appearing",
  approach="MQTT Data Not Appearing - Symptoms: Sensor data not in Parquet files, no errors in logs. Root Causes: 1) Topic mismatch, 2) Missing stream_id in routing. Solution: 1) Check mosquitto_sub -t # for actual topics, 2) Verify config.yaml source.topics matches, 3) Ensure IngestionRouter tags stream_id",
  successRate=0.85,
  tags=["troubleshoot", "mqtt", "debugging", "parquet"]
)
```

### Store Naming Convention

```
mcp__agentdb__agentdb_pattern_store(
  taskType="conventions:naming",
  approach="Stream IDs use kebab-case (outdoor-weather, air-quality-pm25). Database fields use snake_case (temperature_celsius). Rust structs use PascalCase (WeatherReading). Config keys use dot notation (streams.outdoor-weather.enabled).",
  successRate=0.95,
  tags=["conventions", "naming", "style"]
)
```

---

## Pattern Categories (taskType)

Use consistent `taskType` prefixes for categorization:

| Category | taskType Prefix | Examples |
|----------|-----------------|----------|
| Architecture | `architecture:` | `architecture:domain-adapter`, `architecture:data-layers` |
| Procedures | `procedure:` | `procedure:add-stream`, `procedure:deploy-to-pi` |
| Implementation | `implementation:` | `implementation:etl-persistence`, `implementation:mcp-tool` |
| Configuration | `configuration:` | `configuration:gitops`, `configuration:etcd` |
| Testing | `testing:` | `testing:mcp-tool`, `testing:csv-dimension` |
| Deployment | `deployment:` | `deployment:docker`, `deployment:resource-constraints` |
| Troubleshooting | `troubleshoot:` | `troubleshoot:mqtt-issues`, `troubleshoot:parquet-errors` |
| Conventions | `conventions:` | `conventions:naming`, `conventions:code-style` |
| MCP Tools | `mcp:` | `mcp:tool-implementation`, `mcp:silver-storage` |
| ETL | `etl:` | `etl:run-lifecycle`, `etl:persistence` |
| Data Quality | `data-quality:` | `data-quality:framework`, `data-quality:csv-patterns` |

---

## Best Practices

### 1. Check First

Always search before creating to avoid duplicates:

```
mcp__agentdb__agentdb_pattern_search(task="pattern topic", k=5)
```

### 2. Be Specific

Include concrete details:
- **Good**: "Create config/base/streams/{id}/config.yaml with fields array containing name, source_path, unit"
- **Bad**: "Create a config file"

### 3. Include Tags

Add relevant tags for better searchability.

### 4. Reference Files

Mention actual code paths:
```
"Related files: core/src/traits.rs, docs/procedures/HOW_TO_ADD_STREAM.md"
```

### 5. Include Verification

How to confirm the pattern worked:
```
"Verify: Run cargo test, check logs for 'Source initialized'"
```

---

## Update vs. Create New

Use `/pattern-manage` for updates and cleanup:

1. **Update in place** (preferred — no duplicate):
   ```bash
   node .claude/skills/pattern-manage/scripts/pattern-manage.js update 5 --approach "New content..." --success_rate 0.95
   ```

2. **Deprecate old + create new** (when approach fundamentally changed):
   ```bash
   node .claude/skills/pattern-manage/scripts/pattern-manage.js deprecate 5
   ```
   Then use `agentdb_pattern_store` to create the replacement.

3. **Delete obsolete** (pattern references deleted code or is a duplicate):
   ```bash
   node .claude/skills/pattern-manage/scripts/pattern-manage.js delete 5
   ```

4. **Find duplicates**:
   ```bash
   node .claude/skills/pattern-manage/scripts/pattern-manage.js duplicates
   ```

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Search for existing patterns
2. DURING work:  Note gaps, discover new approaches
3. AFTER work:   save-pattern → Store NEW discoveries (THIS SKILL)
                 reflexion    → Record if existing patterns helped
                 learner      → Auto-discover patterns from episodes
```

---

## Related Skills

- **`get-pattern`** - Search patterns BEFORE work (always check first)
- **`reflexion`** - Record feedback on pattern effectiveness
- **`learner`** - Auto-discover patterns from successful episodes
- **`pattern-manage`** - Delete, deprecate, update, deduplicate patterns

---

## What NOT to Use This For

| Don't Store | Use Instead |
|-------------|-------------|
| Swarm coordination state | claude-flow memory tools |
| Agent task status | claude-flow task tools |
| Temporary working memory | claude-flow memory with TTL |
| Session-specific context | claude-flow memory tools |
| Feedback on patterns | `reflexion` skill |

**Patterns are PERMANENT application knowledge, not transient swarm state.**
