---
name: "get-pattern"
description: "Retrieve established project patterns, conventions, architecture decisions, and reference documents. Use when you need to understand how something is done in this project before implementing."
---

# Get Pattern

## What This Skill Does

Retrieves established patterns, conventions, and architecture documentation for the Neural Data Platform. Use this **before implementing anything** to ensure you follow project standards.

## When to Use

- "How do I add a new stream/source/parser?"
- "What's the architecture of this system?"
- "What are the naming conventions?"
- "Where should I put this file?"
- "What's the deployment process?"
- "How does the data pipeline work?"
- "How do I add a new stream or data source?"

## Quick Reference

### Pattern Hierarchy (ndp-patterns namespace)

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

### Category Reference

| Category | What It Contains |
|----------|------------------|
| `architecture` | System design decisions, ADRs, traits, schemas |
| `data-flow` | Pipeline patterns, data transformation approaches |
| `development` | Implementation procedures, step-by-step guides |
| `deployment` | Operational procedures, infrastructure patterns |
| `troubleshooting` | Checklists, common issues and solutions |
| `conventions` | Naming rules, style guides, organization |
| `procedures` | Common requests requiring changes across comopnents (ex: How to add a stream) |
| `streams` | Documentation of active data streams |

---

## Usage

### Method 1: CLI Commands

```bash
# Search for patterns by keyword
claude-flow memory query "<search-term>" --namespace ndp-patterns

# List all patterns in namespace
claude-flow memory list --namespace ndp-patterns

# Get specific pattern (query with exact key)
claude-flow memory query "<category>:<pattern-name>" --namespace ndp-patterns
```

### Method 2: MCP Tools (Preferred for Agents)

```javascript
// Search patterns
mcp__claude-flow__memory_search({
  pattern: "<search-term>",
  namespace: "ndp-patterns",
  limit: 5
})

// Retrieve specific pattern
mcp__claude-flow__memory_usage({
  action: "retrieve",
  key: "<category>:<pattern-name>",
  namespace: "ndp-patterns"
})

// List all patterns
mcp__claude-flow__memory_usage({
  action: "list",
  namespace: "ndp-patterns"
})
```

---

## Key Architecture Documents (File References)

When patterns reference files, check these locations:

| Document | Path | Contains |
|----------|------|----------|
| Platform Architecture | `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` | Full system architecture, evolution, components |
| C4 System Diagram | `docs/architecture/diagrams/neural-data-platform-c4.drawio` | Visual architecture (open in draw.io) |
| Component Dependencies | `docs/architecture/COMPONENT_DEPENDENCY_MAP.md` | How components relate |
| MLOps Building Blocks | `product/features/v2Planning/architecture/MLOPS-BUILDING-BLOCKS.md` | Future ML platform design |

## Key Procedure Documents (File References)

| Procedure | Path |
|-----------|------|
| Add New Stream | `docs/procedures/HOW_TO_ADD_NEW_STREAM.md` |
| Add New Source | `docs/procedures/HOW_TO_ADD_NEW_SOURCE.md` |
| Deployment | `deploy/pi/README.md` |
| Docker Setup | `docs/DOCKER_DEPLOYMENT.md` |

---

## Common Pattern Lookups

### Adding a New Data Stream

```bash
claude-flow memory query "add-stream" --namespace ndp-patterns
```

Or read: `docs/procedures/HOW_TO_ADD_NEW_STREAM.md`

**Quick Summary:**
1. Create `config/streams/{stream-id}/config.yaml`
2. Define schema with `fields:` (snake_case names)
3. Add `sources:` configuration
4. Run `./deploy.sh sync` to load into etcd
5. Verify: `docker exec etcd etcdctl get /streams/{id}/config`

### Adding a New Source Type

```bash
claude-flow memory query "add-source" --namespace ndp-patterns
```

Or read: `docs/procedures/HOW_TO_ADD_NEW_SOURCE.md`

**Quick Summary:**
1. Add `SourceType` variant to `core/src/types/stream_config.rs`
2. Create config struct in `core/src/sources/{name}.rs`
3. Implement `Source` trait with `fetch()` and `health_check()`
4. Create handler for channel pattern
5. Register in `SourceManager`

### Understanding the Architecture

```bash
claude-flow memory query "architecture" --namespace ndp-patterns
```

Or read: `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`

**Quick Summary:**
- Domain Adapter Pattern (hexagonal architecture)
- Core traits: `Source`, `Store`, `Forecast`
- Pipeline: Source → mpsc channel → StorageWriter → ParquetStore
- Config: Stream Registry (etcd) → Legacy etcd → YAML → Defaults

### Naming Conventions

```bash
claude-flow memory query "naming" --namespace ndp-patterns
```

**Quick Summary:**
| Element | Convention | Example |
|---------|------------|---------|
| Stream ID | kebab-case, 3-64 chars | `air-quality`, `outdoor-weather` |
| Field Name | snake_case | `pm25`, `wind_speed` |
| Source Type | snake_case enum | `mqtt`, `http_poll` |
| Rust modules | snake_case | `http_polling_source.rs` |
| Config files | kebab-case.yaml | `outdoor-weather.yaml` |

### Deployment Commands

```bash
claude-flow memory query "deployment" --namespace ndp-patterns
```

**Quick Summary:**
```bash
./deploy.sh          # Full deploy (build + start)
./deploy.sh start    # Start services (no rebuild)
./deploy.sh stop     # Stop all services
./deploy.sh logs     # View live logs
./deploy.sh status   # Check service health
./deploy.sh sync     # Re-sync config to etcd
./deploy.sh update   # Pull latest and redeploy
```

### Data Pipeline Pattern

```bash
claude-flow memory query "pipeline" --namespace ndp-patterns
```

**Quick Summary:**
```
Source (MqttSource, HttpPollingSource)
    │
    │ fetch() → Vec<TimeSeriesPoint>
    ▼
tokio::mpsc channel (buffer: 1000)
    │
    │ recv()
    ▼
StorageWriter
    │ batch: 100 points
    │ timeout: 5 seconds
    ▼
ParquetStore
    │ WAL for crash recovery
    ▼
/data/{stream-id}/YYYY-MM-DD_HH.parquet
```

---

## If Pattern Not Found

If a pattern doesn't exist in memory:

1. Check the pattern index: `.claude/patterns/INDEX.yaml`
2. Search documentation: `docs/` and `product/features/`
3. If you discover a new pattern, use `save-pattern` skill to store it

---

## Related Skills

- `save-pattern` - Store new patterns you discover
- `swarm-orchestration` - Multi-agent coordination
- `sparc-methodology` - Development workflow
