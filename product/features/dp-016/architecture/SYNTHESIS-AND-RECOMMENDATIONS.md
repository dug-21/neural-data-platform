# dp-016: Architecture Synthesis and Recommendations

**Feature**: Configuration Architecture Review
**Date**: 2026-02-01
**Status**: Ready for Discussion

---

## Executive Summary

The research swarm analyzed 7 areas of the NDP configuration architecture. The core finding is that **the intended architecture exists but was inconsistently implemented**:

| Component | Intended Flow | Actual Flow | Status |
|-----------|--------------|-------------|--------|
| Bronze runtime | YAML → etcd → Runtime | YAML → etcd → Runtime | ✅ Works |
| Silver streaming | YAML → etcd → Runtime | YAML → Runtime (direct) | ❌ Broken |
| Silver batch | YAML → etcd → Runtime | etcd → YAML fallback | ⚠️ Different |
| Data dictionary | YAML → etcd → Runtime | YAML → DB (direct) | ❌ Bypasses etcd |
| Hot-reload | etcd watch → Managers | Not wired | ❌ Missing |

**Root Cause**: Each component chose its own config loading path. There was no unified config loading contract.

---

## Critical Decision Factors

Based on analysis, these factors should drive architecture decisions:

### Factor 1: Edge Device Constraints
- **RAM**: 512MB limit for air-quality-app
- **Storage**: SD card wear from frequent writes
- **CPU**: Validation overhead is negligible (<50ms for 50 configs)
- **Network**: etcd is local (<1ms latency)

**Implication**: Minimize etcd writes. Sync on deploy, not on every restart.

### Factor 2: MCP-First Administration Goal
- CRUD APIs already exist in `config-client`
- 15 MCP read tools exist, 0 write tools
- Watch infrastructure exists for hot-reload

**Implication**: Architecture should enable MCP to be the primary admin interface.

### Factor 3: Hot-Reload Feasibility
- Source hot-reload: **Achievable** (methods exist, need wiring)
- Subscriber hot-reload: **Not achievable** without significant refactoring

**Implication**: Plan for Phase 1 (source hot-reload) now, defer subscriber hot-reload.

### Factor 4: Config Section Purposes

| Section | Purpose | Consumer |
|---------|---------|----------|
| `fields` | Metadata/documentation | Validation, MCP tools |
| `sources` | Runtime configuration | SourceManager |
| `storage` | Runtime configuration | BronzeSubscriber |
| `entity_schemas` | Data dictionary | MCP tools, Grafana |
| `silver_etl` | ETL configuration | SilverSubscriber, batch ETL |

**Implication**: Different sections have different access patterns. May need different storage strategies.

### Factor 5: Flattened vs Blob Storage

Current: **Flattened keys** (e.g., `/streams/air-quality/fields/0/name`)

| Approach | Pros | Cons |
|----------|------|------|
| Flattened | Granular updates, individual field access | Complex array handling, more keys |
| Blob JSON | Simpler retrieval, atomic updates | All-or-nothing updates, larger reads |
| Hybrid | Best of both | More complexity |

**Implication**: Arrays (fields, sources, field_mappings) may benefit from blob storage.

---

## Architecture Options

### Option A: YAML Primary + etcd Runtime Cache (Current Intent, Fix Implementation)

```
YAML (git-versioned)
    │
    │ deploy.sh sync (on deploy only)
    ▼
etcd (runtime cache)
    │
    │ All consumers read from etcd
    ▼
┌─────────────────────────────────────────────┐
│ Bronze │ Silver │ Dictionary │ MCP Server  │
└─────────────────────────────────────────────┘
```

**Changes Required**:
1. Fix Silver streaming to read from etcd (not YAML)
2. Fix data dictionary sync to use etcd (or sync on deploy)
3. Unified `ConfigLoader` with consistent behavior

**Pros**:
- Preserves git-versioned YAML as source of truth
- Minimal architectural change
- Human-readable config files

**Cons**:
- Two sources to keep in sync
- Config changes require git workflow

### Option B: etcd Primary (MCP-First)

```
MCP Tools (create/update/delete)
    │
    │ Direct writes to etcd
    ▼
etcd (PRIMARY source of truth)
    │
    │ All consumers read from etcd
    ▼
┌─────────────────────────────────────────────┐
│ Bronze │ Silver │ Dictionary │ MCP Server  │
└─────────────────────────────────────────────┘
    │
    │ Optional: etcd → YAML export for git versioning
    ▼
YAML (backup/version control)
```

**Changes Required**:
1. Implement MCP write tools (create_stream, update_stream, delete_stream)
2. Add enhanced validation for silver_etl section
3. Wire hot-reload via etcd watches
4. Optional: Export to YAML for git versioning

**Pros**:
- Single source of truth
- Enables full MCP administration
- Hot-reload path is natural

**Cons**:
- Config not in git by default (need export)
- More trust in etcd durability
- Bigger change from current workflow

### Option C: Hybrid by Section

```
Sources/Storage (runtime) ──────► etcd (primary)
    ▲                                  │
    │                                  │ MCP can modify
    │                                  ▼
    │                         ┌──────────────┐
    │                         │ Runtime Apps │
    │                         └──────────────┘

Fields/entity_schemas (documentation) ──► YAML (primary)
    │                                          │
    │ deploy.sh sync-dictionary                │
    ▼                                          │
TimescaleDB data_dictionary                    │
                                               │
silver_etl (ETL config) ───────────────────────┘
    │
    │ deploy.sh silver-migrate (generates DDL)
    ▼
TimescaleDB Silver tables
```

**Changes Required**:
1. Split config into "runtime" (sources, storage) vs "schema" (fields, entity_schemas, silver_etl)
2. Runtime sections: MCP-editable, hot-reloadable, etcd primary
3. Schema sections: YAML-primary, git-versioned, deploy-time sync

**Pros**:
- Matches actual access patterns
- Runtime config can change without git workflow
- Schema/DDL changes go through proper review

**Cons**:
- More complex mental model
- Config split across files/systems

---

## Recommendations

### Recommendation 1: Fix the Unified Loading Path First

Before choosing an architecture, fix the immediate problem: **all consumers should load config the same way**.

**Immediate Actions** (dp-016 Phase 1):
1. Create `ConfigLoader` trait with unified `load_stream()` behavior
2. Fix Silver streaming to use this loader (addresses air-013)
3. Add explicit logging when config sources differ

### Recommendation 2: Adopt Option A with Path to Option B

**Short-term** (dp-016): Option A - YAML primary, fix implementation
- Preserves current git workflow
- Minimal disruption
- Clear path to Option B

**Medium-term** (future feature): Option B - MCP primary
- Once MCP write tools exist, make etcd primary
- Export to YAML for git versioning/backup
- Enable hot-reload for sources

### Recommendation 3: Blob Storage for Arrays

Switch `fields`, `sources`, `field_mappings` to blob JSON storage:
- Current flattened arrays are error-prone (order matters)
- Blob storage enables atomic updates
- Simpler retrieval via `get_prefix_nested()`

### Recommendation 4: Validation Before Storage

Add comprehensive validation before etcd save:
1. StreamConfig validation (exists)
2. SilverEtlConfig validation (add)
3. Cross-reference validation (source_path vs fields)
4. Target table existence check

### Recommendation 5: Phase Hot-Reload

| Phase | Scope | Effort |
|-------|-------|--------|
| Phase 1 | Wire etcd watch to SourceManager | 3 days |
| Phase 2 | Add source hot-reload API | 2 days |
| Phase 3 | Defer subscriber hot-reload | Future |

---

## Key Questions for Discussion

### Q1: Source of Truth

**Should etcd become the primary source of truth for runtime config?**

- If YES (Option B): MCP can modify config directly, enables hot-reload
- If NO (Option A): YAML remains primary, git workflow preserved

*This is the fundamental architectural decision.*

### Q2: Config Splitting

**Should we split config into "runtime" vs "schema" sections?**

- Runtime (sources, storage): Could be MCP-editable, hot-reloadable
- Schema (fields, entity_schemas, silver_etl): DDL implications, needs git review

*This affects how we approach MCP administration and hot-reload.*

### Q3: Storage Format

**Should we switch arrays from flattened keys to blob JSON?**

- Flattened: `/streams/x/fields/0/name`, `/streams/x/fields/0/type`
- Blob: `/streams/x/fields` = `[{"name":"...", "type":"..."}]`

*Current flattened approach is complex and error-prone for arrays.*

### Q4: Silver Table DDL

**Should DDL generation be part of config sync or a separate step?**

- Option: Generate DDL from config, apply via MCP `create_silver_table` tool
- Option: Keep as manual step but add validation

*This affects dp-015 (Config-Driven Silver Tables) scope.*

### Q5: Hot-Reload Scope

**Which components should support hot-reload?**

- Sources: Achievable now (methods exist)
- Bronze subscribers: Needs refactoring
- Silver subscribers: Significant refactoring
- DDL changes: Cannot hot-reload (schema migration)

*Guides prioritization of hot-reload work.*

---

## Implementation Roadmap (Draft)

Based on recommendations, here's a proposed order:

### Phase 1: Foundation (dp-016 core)
1. Create unified `ConfigLoader` trait
2. Fix Silver streaming to read from etcd
3. Fix data dictionary sync path
4. Add explicit config source logging

### Phase 2: Validation (dp-016 + dp-017)
1. Add SilverEtlConfig validation
2. Add cross-reference validation
3. Add target table existence check
4. Promote sync errors to ERROR level

### Phase 3: MCP Write Tools (future feature)
1. Implement `create_stream`, `update_stream`, `delete_stream`
2. Add `validate_stream_config` (dry-run)
3. Wire validation into MCP tools

### Phase 4: Hot-Reload (future feature)
1. Wire etcd watch to SourceManager
2. Add `reload_stream` MCP tool
3. Test source hot-reload end-to-end

### Phase 5: DDL Automation (dp-015)
1. Generate DDL from SilverEtlConfig
2. Add `create_silver_table` MCP tool
3. Add migration tracking

---

## Supporting Analysis Documents

| Document | Key Finding |
|----------|-------------|
| ETCD-STORAGE-ANALYSIS.md | Flattened keys, watch capability exists |
| BRONZE-UTILIZATION-ANALYSIS.md | Works correctly (YAML → etcd → runtime) |
| SILVER-UTILIZATION-ANALYSIS.md | Streaming reads YAML directly (root cause of air-013) |
| DICTIONARY-FLOW-ANALYSIS.md | Sync bypasses etcd, reads YAML directly |
| HOT-RELOAD-FEASIBILITY.md | Sources can hot-reload, subscribers cannot |
| EDGE-CONSTRAINTS-ANALYSIS.md | Favors YAML primary, etcd cache |
| MCP-ADMIN-ANALYSIS.md | CRUD APIs exist, MCP tools are read-only |

---

*Ready for architecture discussion.*
