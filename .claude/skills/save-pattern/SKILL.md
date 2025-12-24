---
name: "save-pattern"
description: "Store, update, or deprecate APPLICATION patterns (architecture, procedures, conventions) in AgentDB. NOT for swarm/transient memory."
---

# Save Pattern - Store Application Knowledge

## What This Skill Does

Manages the full lifecycle of **application patterns** in AgentDB:
- **Store** - Create new patterns with semantic embeddings
- **Update** - Replace patterns (AgentDB tracks versions)
- **Deprecate** - Mark patterns as outdated with replacement guidance
- **Delete** - Remove patterns entirely

## CRITICAL: What This Skill IS and IS NOT For

### USE FOR (Application Knowledge)

| Type | Examples |
|------|----------|
| **Architecture** | System design, ADRs, schemas, trait patterns |
| **Procedures** | "How to add a stream", "How to deploy", step-by-step guides |
| **Conventions** | Naming rules, code style, organization standards |
| **Data Flow** | Pipeline patterns, ETL approaches, transformation logic |
| **Troubleshooting** | Common issues and solutions, debugging checklists |

### DO NOT USE FOR (Transient/Swarm Memory)

| Type | Use Instead |
|------|-------------|
| Swarm coordination state | claude-flow memory tools |
| Agent task status | claude-flow task tools |
| Temporary working memory | claude-flow memory tools |
| Inter-agent messages | claude-flow DAA tools |
| Session-specific context | claude-flow memory with TTL |

**Patterns are PERMANENT application knowledge, not transient swarm state.**

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Check existing patterns first
2. DURING work:  Discover new approaches
3. AFTER work:   save-pattern → Store reusable discoveries (THIS SKILL)
                 reflexion    → Evaluate if get-pattern results helped
```

---

## Quick Usage

### Store New Pattern

```bash
npx agentdb store-pattern \
  --type "development" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "pattern-name",
    "approach": "# Pattern Name\n\n## Context\nWhen/why to use this.\n\n## Prerequisites\n- Required setup\n\n## Steps\n1. First step\n2. Second step\n\n## Example\nConcrete usage.\n\n## Verification\nHow to confirm it worked.",
    "version": "1.0",
    "related_files": ["path/to/related/file.md"],
    "last_verified": "2025-12-22"
  }' \
  --confidence 0.9
```

### Pattern Content Template

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
| `architecture` | System design, ADRs, schemas | `adr`, `design`, `schema`, `traits` |
| `data-flow` | Pipeline patterns, transformations | `pipeline`, `etl`, `channel`, `ingestion` |
| `development` | Implementation procedures | `procedure`, `howto`, `coding`, `rust` |
| `deployment` | Operational procedures | `docker`, `deploy`, `infrastructure`, `pi` |
| `troubleshooting` | Common issues and solutions | `debug`, `fix`, `checklist`, `errors` |
| `conventions` | Naming rules, style guides | `naming`, `style`, `organization` |
| `procedures` | Multi-component changes | `stream`, `source`, `parser`, `dashboard` |
| `streams` | Active stream documentation | `mqtt`, `http`, `sensor`, `weather` |

---

## Update Existing Pattern

Store with the same pattern name - AgentDB handles versioning:

```bash
npx agentdb store-pattern \
  --type "development" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "add-stream",
    "approach": "# Add New Data Stream (Updated)\n\n## Steps\n1. Create config...\n2. NEW: Add retention field (required since v2.0)\n3. ...",
    "version": "2.0",
    "last_verified": "2025-12-22",
    "changelog": "Added required retention field"
  }' \
  --confidence 0.85
```

---

## Deprecate Pattern

When a pattern is replaced by a better approach:

```bash
npx agentdb store-pattern \
  --type "deployment" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "manual-config-sync",
    "approach": "# DEPRECATED: Manual Config Sync\n\n## Status\nDEPRECATED as of 2025-12-22\n\n## Replacement\nUse deployment:automated-config-sync instead.\n\n## Migration\n1. Stop using manual etcdctl put commands\n2. Edit YAML files in config/streams/\n3. Run: ./deploy.sh sync\n\n## Reason\nManual sync was error-prone.",
    "status": "deprecated",
    "replacement": "automated-config-sync",
    "deprecation_date": "2025-12-22"
  }' \
  --confidence 0.0
```

**Prefer Deprecation over Deletion** - deprecated patterns serve as historical reference.

---

## Real Examples

### Architecture Pattern

```bash
npx agentdb store-pattern \
  --type "architecture" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "domain-adapter-source",
    "approach": "# Domain Adapter Pattern\n\n## Context\nAll data sources implement the Source trait for uniform handling.\n\n## Implementation\n1. Create struct implementing Source trait\n2. Implement fetch() -> Vec<TimeSeriesPoint>\n3. Implement health_check() -> HealthStatus\n\n## Example\nSee: core/src/sources/http_poll.rs",
    "tags": ["architecture", "traits", "hexagonal", "source"],
    "related_files": ["core/src/sources/http_poll.rs", "core/src/traits.rs"]
  }' \
  --confidence 0.95
```

### Procedure Pattern

```bash
npx agentdb store-pattern \
  --type "procedures" \
  --domain "ndp-patterns" \
  --pattern '{
    "name": "add-grafana-dashboard",
    "approach": "# Add Grafana Dashboard\n\n## Prerequisites\n- DuckDB datasource configured (uid: duckdb-ndp)\n- Stream data in Parquet bronze layer\n\n## Steps\n1. Create JSON in config/grafana/dashboards/\n2. Use time_bucket() for aggregation\n3. Filter by stream_id in WHERE clause\n4. Handle unit conversions in SQL\n\n## Verification\nRestart Grafana, check dashboard loads without errors",
    "tags": ["procedures", "grafana", "dashboard", "visualization"],
    "related_files": ["config/grafana/dashboards/indoor-vs-outdoor.json"]
  }' \
  --confidence 0.9
```

---

## Check Pattern Health

```bash
npx agentdb db stats
```

Returns: total patterns, average success rates, top task types, patterns needing review.

---

## Best Practices

1. **Be Specific** - Include concrete examples, not just theory
2. **Include Context** - Explain when/why to use the pattern
3. **Reference Files** - Link to actual code that implements the pattern
4. **Use Consistent Tags** - Category + descriptive tags (kebab-case)
5. **Deprecate Don't Delete** - Preserve knowledge unless harmful
6. **Set Realistic Success Rates** - Start at 0.8-0.9, let feedback adjust
7. **Include Verification Steps** - How to confirm the pattern worked
8. **Store Only Reusable Knowledge** - Not one-off solutions

---

## Related Skills

- **`get-pattern`** - Retrieve patterns BEFORE work (always check first)
- **`reflexion`** - Record feedback on whether `get-pattern` results helped

**NOT related to:**
- Swarm coordination (use claude-flow tools)
- Transient task memory (use claude-flow memory with TTL)
- Agent state management (use claude-flow tools)
