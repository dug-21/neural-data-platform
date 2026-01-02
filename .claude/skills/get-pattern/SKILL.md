---
name: "get-pattern"
description: "Retrieve APPLICATION patterns (architecture, procedures, conventions) via semantic search. Use BEFORE implementing to ensure consistency."
---

# Get Pattern - Retrieve Application Knowledge

## What This Skill Does

Retrieves established **application patterns** (architecture, procedures, conventions) for the Neural Data Platform using AgentDB's semantic vector search.

**Use this BEFORE implementing anything** to ensure you follow project standards.

---

## Quick Reference

```bash
# Search reasoning patterns
npx agentdb query --query "your search" --k 5 --min-confidence 0.6

# Search past successful experiences
npx agentdb reflexion retrieve "your search" --k 5 --only-successes

# Check what patterns exist
npx agentdb db stats
```

---

## Primary Method: Search Reasoning Patterns

```bash
npx agentdb query \
  --query "domain adapter pattern hexagonal architecture" \
  --k 5 \
  --min-confidence 0.6
```

### Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `--query` | Search text (semantic match) | required |
| `--k` | Number of results | 5 |
| `--min-confidence` | Minimum similarity threshold (0-1) | 0.0 |
| `--domain` | Filter by category | none |
| `--synthesize-context` | Generate summary | false |

### Examples

**Find architecture patterns:**
```bash
npx agentdb query --query "domain adapter pattern" --k 5 --min-confidence 0.7
```

**Find deployment procedures:**
```bash
npx agentdb query --query "deploy raspberry pi docker" --domain "deployment" --k 3
```

**Get synthesized summary:**
```bash
npx agentdb query --query "add new data stream" --k 5 --synthesize-context
```

---

## Secondary Method: Search Past Experiences

If no patterns exist, search reflexion episodes for similar past work:

```bash
npx agentdb reflexion retrieve "MQTT source implementation" \
  --k 5 \
  --only-successes \
  --min-reward 0.7
```

### Parameters

| Parameter | Description |
|-----------|-------------|
| `--k` | Number of results |
| `--only-successes` | Only successful episodes |
| `--only-failures` | Only failed episodes |
| `--min-reward` | Minimum reward threshold |
| `--synthesize-context` | Generate summary |

---

## Pattern Categories

| Category | --domain Value | What It Contains |
|----------|----------------|------------------|
| Architecture | `architecture` | System design, ADRs, traits, schemas |
| Data Flow | `data-flow` | Pipeline patterns, ETL approaches |
| Development | `development` | Implementation procedures, guides |
| Deployment | `deployment` | Operational procedures, infrastructure |
| Troubleshooting | `troubleshooting` | Checklists, common issues |
| Conventions | `conventions` | Naming rules, style guides |
| Streams | `streams` | Data stream documentation |
| Silver | `silver` | Silver layer, SQL, data dictionary |

---

## Interpreting Results

Results include:

| Field | Meaning |
|-------|---------|
| `taskType` | Pattern category |
| `approach` | The actual pattern content (markdown) |
| `successRate` | How often this pattern succeeded (0-1) |
| `similarity` | How relevant to your query (0-1) |

**High-value patterns**: successRate > 0.8 AND similarity > 0.6

---

## If No Patterns Found

1. **Check pattern stats:**
   ```bash
   npx agentdb db stats
   ```

2. **Search reflexion episodes:**
   ```bash
   npx agentdb reflexion retrieve "your query" --k 10
   ```

3. **Check file-based documentation:**
   - `docs/architecture/` - Architecture documents
   - `docs/procedures/` - Step-by-step procedures
   - `product/features/*/architecture/` - Feature ADRs

4. **After implementing**, store the new pattern via `save-pattern`

---

## The Pattern Workflow

```
1. BEFORE work:  get-pattern  → Search for relevant patterns (THIS SKILL)
2. DURING work:  Apply the pattern, note what works/gaps
3. AFTER work:   reflexion    → Record if pattern helped (required)
                 save-pattern → Store NEW discoveries (if any)
                 learner      → Auto-discover patterns from episodes (periodic)
```

---

## Related Skills

- **`save-pattern`** - Store NEW patterns after discovering reusable approaches
- **`reflexion`** - Record feedback on pattern effectiveness (REQUIRED after using patterns)
- **`learner`** - Auto-discover patterns from successful episodes

---

## What NOT to Use This For

| Don't Search For | Use Instead |
|------------------|-------------|
| Current swarm status | claude-flow swarm tools |
| Agent task state | claude-flow task tools |
| Working memory | claude-flow memory tools |
| Session context | claude-flow memory with TTL |

**Patterns are PERMANENT application knowledge, not transient swarm state.**
