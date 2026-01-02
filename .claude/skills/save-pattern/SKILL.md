---
name: "save-pattern"
description: "Store, update, or deprecate APPLICATION patterns (architecture, procedures, conventions) in AgentDB's Reasoning Patterns table. NOT for swarm/transient memory."
---

# Save Pattern - Store Application Knowledge

## What This Skill Does

Stores **application patterns** to AgentDB's **Reasoning Patterns** table with semantic embeddings. Patterns are versioned and searchable via `get-pattern`.

**Use this AFTER completing work** to share reusable knowledge with future agents.

---

## Quick Reference

```bash
# Store a new pattern
npx agentdb store-pattern --type "category" --domain "ndp-patterns" \
  --pattern '{"name": "...", "approach": "..."}' --confidence 0.9

# Check existing patterns
npx agentdb db stats

# Search before creating (avoid duplicates)
npx agentdb query --query "pattern name" --k 3
```

---

## Primary Method: Store Pattern

```bash
npx agentdb store-pattern \
  --type "architecture" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "pattern-name",
    "approach": "# Pattern Name\n\n## Context\n...",
    "version": "1.0",
    "tags": ["tag1", "tag2"],
    "related_files": ["path/to/file.rs"]
  }' \
  --confidence 0.9
```

### Parameters

| Parameter | Description | Required |
|-----------|-------------|----------|
| `--type` | Category (see table below) | Yes |
| `--domain` | Namespace (use `ndp-patterns`) | Yes |
| `--pattern` | JSON with pattern content | Yes |
| `--confidence` | Success rate 0-1 | Yes |

---

## Pattern JSON Structure

```json
{
  "name": "pattern-name",
  "approach": "# Pattern Name\n\n## Context\nWhen to use...\n\n## Steps\n1. First...\n2. Second...\n\n## Example\n...",
  "version": "1.0",
  "tags": ["category", "topic1", "topic2"],
  "related_files": ["path/to/file.rs", "docs/relevant.md"],
  "last_verified": "2025-01-02"
}
```

### Approach Template (Markdown)

```markdown
# Pattern Name

## Context
When and why to use this pattern.

## Prerequisites
- Required setup or conditions

## Steps
1. First step with details
2. Second step with details
3. Verification step

## Example
Concrete usage example with file references.

## Related Files
- `path/to/relevant/file.rs`

## Changelog
- v1.0 (2025-01-02): Initial version
```

---

## Examples

### Store Architecture Pattern

```bash
npx agentdb store-pattern \
  --type "architecture" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "domain-adapter-source",
    "approach": "# Domain Adapter Pattern\n\n## Context\nAll data sources implement the Source trait for uniform handling.\n\n## Steps\n1. Create struct implementing Source trait\n2. Implement fetch() -> Vec<TimeSeriesPoint>\n3. Implement health_check() -> HealthStatus\n\n## Related Files\n- core/src/traits.rs\n- core/src/sources/http_poll.rs",
    "version": "1.0",
    "tags": ["hexagonal", "traits", "source"]
  }' \
  --confidence 0.95
```

### Store Development Procedure

```bash
npx agentdb store-pattern \
  --type "development" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "add-data-stream",
    "approach": "# Add New Data Stream\n\n## Prerequisites\n- Stream config YAML ready\n- etcd running\n\n## Steps\n1. Create config/base/streams/{stream-id}/config.yaml\n2. Define fields array with name, source_path, unit\n3. Run ./deploy.sh sync\n4. Verify: etcdctl get /streams/{id}/config",
    "version": "1.0",
    "tags": ["streams", "config", "etcd"]
  }' \
  --confidence 0.9
```

### Store Troubleshooting Pattern

```bash
npx agentdb store-pattern \
  --type "troubleshooting" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "mqtt-data-not-appearing",
    "approach": "# MQTT Data Not Appearing\n\n## Symptoms\n- Sensor data not in Parquet files\n- No errors in logs\n\n## Root Causes\n1. Topic mismatch\n2. Missing stream_id in routing\n\n## Solution\n1. Check mosquitto_sub -t # for actual topics\n2. Verify config.yaml source.topics matches\n3. Ensure IngestionRouter tags stream_id",
    "version": "1.0",
    "tags": ["mqtt", "debugging", "parquet"]
  }' \
  --confidence 0.85
```

---

## Pattern Categories

| Category | --type Value | Use For |
|----------|--------------|---------|
| Architecture | `architecture` | System design, ADRs, traits, schemas |
| Data Flow | `data-flow` | Pipeline patterns, ETL approaches |
| Development | `development` | Implementation procedures, guides |
| Deployment | `deployment` | Operational procedures, infrastructure |
| Troubleshooting | `troubleshooting` | Debug checklists, common issues |
| Conventions | `conventions` | Naming rules, style guides |
| Streams | `streams` | Data stream documentation |
| Silver | `silver` | Silver layer, SQL, data dictionary |

---

## Update Existing Pattern

Store with same name, increment version:

```bash
npx agentdb store-pattern \
  --type "development" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "add-data-stream",
    "approach": "# Add Data Stream (v2.0)\n\n## Changelog\n- v2.0: Added retention field requirement\n- v1.0: Initial version\n\n## Steps\n1. Create config.yaml\n2. NEW: Add retention field (required)\n3. ...",
    "version": "2.0",
    "tags": ["streams", "config", "updated"]
  }' \
  --confidence 0.85
```

---

## Deprecate Pattern

Set confidence to 0 and mark as deprecated:

```bash
npx agentdb store-pattern \
  --type "deployment" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "manual-config-sync",
    "approach": "# DEPRECATED: Manual Config Sync\n\n## Status\nDEPRECATED as of 2025-01-02\n\n## Replacement\nUse automated-config-sync instead.\n\n## Migration\n1. Stop manual etcdctl commands\n2. Edit YAML in config/streams/\n3. Run ./deploy.sh sync",
    "status": "deprecated",
    "replacement": "automated-config-sync"
  }' \
  --confidence 0.0
```

**Prefer deprecation over deletion** - preserves historical context.

---

## Confidence Guidelines

| Score | When to Use |
|-------|-------------|
| 0.95+ | Proven pattern, used successfully multiple times |
| 0.85 | Works well, minor edge cases |
| 0.70 | Generally works, some adjustments needed |
| 0.50 | Experimental, needs validation |
| 0.0 | Deprecated or superseded |

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

## Best Practices

1. **Check first** - Use `get-pattern` before creating duplicates
2. **Be specific** - Include concrete examples, not just theory
3. **Reference files** - Link to actual code that implements the pattern
4. **Use consistent tags** - Category + descriptive tags (kebab-case)
5. **Include verification** - How to confirm the pattern worked
6. **Store only reusable knowledge** - Not one-off solutions
7. **Update, don't duplicate** - Increment version instead of new pattern

---

## Related Skills

- **`get-pattern`** - Search patterns BEFORE work (always check first)
- **`reflexion`** - Record feedback on pattern effectiveness
- **`learner`** - Auto-discover patterns from successful episodes

---

## What NOT to Use This For

| Don't Store | Use Instead |
|-------------|-------------|
| Swarm coordination state | claude-flow memory tools |
| Agent task status | claude-flow task tools |
| Temporary working memory | claude-flow memory with TTL |
| Session-specific context | claude-flow memory tools |

**Patterns are PERMANENT application knowledge, not transient swarm state.**
