# dp-016: Architecture Decision Questions

**Feature**: Configuration Architecture Review
**Date**: 2026-02-01
**Status**: Pending Discussion

---

## Overview

These 5 questions must be answered to finalize the configuration architecture. Each question has architectural implications that affect implementation scope and ordering.

---

## Q1: Source of Truth

**Should etcd become the primary source of truth for runtime config?**

| Option | Description |
|--------|-------------|
| **A: YAML Primary** | YAML files remain source of truth, etcd is runtime cache. Config changes require git workflow. Fix current implementation inconsistencies. |
| **B: etcd Primary** | etcd becomes source of truth. MCP can modify directly. Optional export to YAML for git versioning. |

**Implications**:
- Option A: Preserves git workflow, less change, but MCP is read-only
- Option B: Enables full MCP administration, hot-reload natural, but bigger change

**Discussion Notes**:
```
2026-02-01: User clarified:
- Pi has NVMe SSD (not microSD), so storage wear is NOT a concern
- Webhook off commit is acceptable automation path
- Power loss must not lose config state → Git provides this durability
- Future MCP/UI would write to JSON, then trigger deploy flow
- Git commit/push serves as backup after MCP changes

Bronze already works this way. Silver is broken (reads files directly).
Fix Silver to match Bronze pattern.

MCP write flow: MCP → Write JSON → deploy.sh sync → etcd → reload → git push (backup)

2026-02-01: JSON selected as platform standard because:
- Agents write 90%+ of config; JSON has no indentation errors
- MCP speaks JSON natively; no conversion needed
- JSON Schema validation is mature with excellent tooling
- Strict format eliminates ambiguity (true vs "yes" vs yes)
- etcd tooling assumes JSON; native storage format
```

**Decision**: [x] Option A (JSON Primary, etcd as runtime cache)

**Implications**:
1. Fix Silver streaming to read from etcd (air-013)
2. Fix data dictionary sync to use etcd path
3. MCP write tools generate/modify JSON files (native format)
4. deploy.sh sync remains the propagation mechanism
5. Git is the durability/backup strategy
6. JSON Schema validates all configs before deployment

---

## Q2: Config Splitting

**Should we split config into "runtime" vs "schema" sections?**

| Section Type | Contents | Characteristics |
|--------------|----------|-----------------|
| **Runtime** | `sources`, `storage` | Can change without restart, MCP-editable, hot-reloadable |
| **Schema** | `fields`, `entity_schemas`, `silver_etl` | DDL implications, needs review, git-versioned |

**Implications**:
- If YES: Different storage/access patterns per section, more complex but matches reality
- If NO: All config treated uniformly, simpler mental model

**Discussion Notes**:
```
2026-02-01: Discussion evolved beyond runtime/schema split.

Key insight: Full reload on every change is risky - can break working streams
that weren't touched. Need isolation.

Decision: Split by STREAM, not by section type.
- Each stream is an isolated unit
- Changes to one stream don't affect others
- Atomic per-stream updates

This led to "Declarative Deploy" concept - agents declare what changed in a
manifest, deploy executes only the necessary actions.
```

**Decision**: [x] Other: Per-stream isolation (not runtime/schema split)

**Implications**:
1. etcd storage: blob per stream (not flattened, not by section)
2. Sync: incremental per-stream
3. Validation: per-stream before write
4. Reload: only affected streams

---

## Q3: Storage Format

**Should we switch arrays from flattened keys to blob JSON?**

| Format | Example | Pros | Cons |
|--------|---------|------|------|
| **Flattened** | `/streams/x/fields/0/name` | Granular updates | Complex, order-dependent |
| **Blob JSON** | `/streams/x/fields` = `[{...}]` | Atomic updates, simpler | All-or-nothing |
| **Hybrid** | Core fields flat, arrays as blob | Best of both | More complexity |

**Affected Arrays**:
- `fields` (Bronze schema)
- `sources` (data sources)
- `field_mappings` (Silver ETL)
- `dq_rules` (data quality)

**Discussion Notes**:
```
2026-02-01: Given Q2 decision (per-stream isolation), storage format follows naturally.

- Per-stream blob makes atomic updates simple
- Flattening adds complexity without benefit
- Sync writes one key per stream, not 50+ keys

Storage:
  /streams/{stream-id}/config = { full StreamConfig as JSON blob }
```

**Decision**: [x] JSON per-stream (native etcd format)

**Implications**:
1. Simpler sync logic (one put per stream)
2. Atomic updates (all or nothing per stream)
3. Matches incremental sync granularity
4. JSON stored natively in etcd (no format conversion needed)
5. Apps parse JSON from etcd via serde_json (fast, mature)
6. MCP tools can read/write without conversion

---

## Q4: Silver Table DDL

**Should DDL generation be part of config sync or a separate step?**

| Option | Description | When DDL Runs |
|--------|-------------|---------------|
| **A: Integrated** | `deploy.sh sync` detects missing tables, creates them | Every sync |
| **B: Separate MCP** | MCP `create_silver_table` tool generates and applies DDL | On demand |
| **C: Separate CLI** | `deploy.sh silver-migrate` remains manual | Manual step |
| **D: Validation Only** | Sync validates table exists, fails if missing | Sync time (no create) |

**Implications**:
- Option A: Fully automated, but DDL changes without review
- Option B: MCP-driven, explicit action required
- Option C: Current state, manual step remains
- Option D: Fails fast but doesn't solve the problem

**Discussion Notes**:
```
2026-02-01: With declarative deploy, DDL becomes a declaration type.

Agent explicitly declares silver-table action in manifest:
  - type: silver-table
    stream: new-sensor
    action: create

This matches declarative philosophy - agent declares intent, deploy executes.
No magic auto-creation, explicit is better than implicit.
```

**Decision**: [x] B: Explicit declaration in manifest

**Implications**:
1. New manifest change type: `silver-table`
2. Actions: create | validate-only | skip
3. Deploy generates DDL from silver_etl config
4. Deploy applies migration
5. Agent must know to declare it (but simple rule: new stream with silver_etl = add silver-table)

---

## Q5: Hot-Reload Scope

**Which components should support hot-reload?**

| Component | Feasibility | Effort | Notes |
|-----------|-------------|--------|-------|
| **Sources** (MQTT, HTTP) | ✅ Achievable | Low | Methods exist, need wiring |
| **Bronze Subscribers** | ⚠️ Possible | Medium | Needs coordinator refactoring |
| **Silver Subscribers** | ❌ Difficult | High | Ownership model blocks this |
| **DDL Changes** | ❌ Not possible | N/A | Schema migrations require restart |

**Options**:
- [ ] Sources only (Phase 1)
- [ ] Sources + Bronze subscribers (Phase 1+2)
- [ ] Full hot-reload (requires significant refactoring)
- [ ] No hot-reload (restart required for all changes)

**Discussion Notes**:
```
2026-02-01: With declarative deploy, reload scope becomes a declaration option.

Manifest declares reload intent per stream:
  - type: stream
    id: air-quality
    reload: sources    # or: full | none

Start with:
- sources: Hot-reload MQTT/HTTP sources (methods exist, need wiring)
- full: Restart app for stream changes (current behavior, explicit)

Defer:
- Subscriber hot-reload (significant refactoring needed)
```

**Decision**: [x] sources + full (defer subscriber hot-reload)

**Implications**:
1. Manifest reload options: sources | full | none
2. Wire etcd watch → SourceManager.update_sources_for_stream()
3. Full = app restart (or specific stream restart when implemented)
4. Subscriber hot-reload is future work (not in dp-016)

---

## Summary Table

| Question | Decision | Date | ADR |
|----------|----------|------|-----|
| Q1: Source of Truth | **JSON Primary** (etcd as runtime cache) | 2026-02-01 | ADR-016-001 |
| Q2: Config Splitting | **Per-stream isolation** (not runtime/schema) | 2026-02-01 | - |
| Q3: Storage Format | **JSON per stream** (native etcd format) | 2026-02-01 | - |
| Q4: Silver Table DDL | **Explicit declaration** in manifest | 2026-02-01 | - |
| Q5: Hot-Reload Scope | **sources + full** (defer subscriber) | 2026-02-01 | - |
| Q6: Merge fields/entity_schemas | **Yes, merge** (simplify) | 2026-02-01 | - |
| Q7: Silver cross-stream | **Defer to Gold layer** | 2026-02-01 | - |
| Q8: Config schema versioning | **Breaking + migration tool** | 2026-02-01 | - |
| Emergent: Deploy | **Declarative Deploy** with JSON manifest | 2026-02-01 | ADR-016-002 |
| Emergent: Validator | **JSON Schema validator component** | 2026-02-01 | - |
| Emergent: Format | **JSON as platform standard** | 2026-02-01 | ADR-016-001 |
| Future: Data schema evolution | **Deferred** | - | - |

---

---

## Q6: Merge fields and entity_schemas?

*Added during discussion - simplification opportunity identified.*

**Current State**: Two overlapping sections in stream config:
- `fields` - Bronze payload structure (name, type, unit, range, nullable)
- `entity_schemas` - Data dictionary docs (name, type, unit, description, device_class)

**Proposal**: Merge into enriched `fields` section, eliminate `entity_schemas`.

```json
{
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "nullable": false,
      "unit": "µg/m³",
      "range": [0.0, 500.0],
      "description": "Particulate matter 2.5 micrometers",
      "device_class": "sensor"
    }
  ]
}
```

*Note: `description` field replaces comments (JSON has no comments). This is part of the JSON platform standard - descriptions are queryable, validated, and don't rot.*

**Discussion Notes**:
```
2026-02-01: Identified overlap between fields and entity_schemas.
Merging simplifies mental model - one field, one definition.
No functionality lost - attributes just move to fields section.
Data dictionary sync pulls from enriched fields instead of separate section.
```

**Decision**: [x] Merge (eliminate entity_schemas section)

**Implications**:
1. Enriched `fields` with description, device_class attributes
2. Remove `entity_schemas` section from config
3. Update data dictionary sync to read from `fields`
4. Migration needed for existing configs
5. Validator schema updated accordingly

---

## Q7: Silver cross-stream merging?

*Added during discussion - scope question.*

**Question**: Should silver_etl support merging multiple Bronze streams into one Silver table?

**Options**:
- A: Silver ETL merges (config-driven, complex)
- B: Database joins/views (SQL, flexible)
- C: Defer to Gold layer (right abstraction level)

**Discussion Notes**:
```
2026-02-01: Cross-stream combination is aggregation/analytics concern.
Silver should stay simple: 1:1 with streams.
Gold layer is the right place for views, aggregations, feature engineering.
Defer this to Gold layer design (fe-* features).
```

**Decision**: [x] Defer to Gold layer

**Implications**:
1. Silver tables remain 1:1 with streams
2. Cross-stream queries use ad-hoc SQL joins for now
3. Gold layer design will address configured views/aggregations
4. No scope creep into dp-016

---

## Q8: Config Schema Versioning

**Question**: How do we handle changes to the config JSON structure itself?

**Decision**: Breaking changes with migration tool + version in file

**Approach**:
```json
{
  "config_version": 2,
  "stream_id": "air-quality",
  "description": "Air quality measurements from AirGradient sensors",
  "fields": [...]
}
```

**Components**:
1. `config_version` field in every config file
2. App/Validator only supports current version (no backward compat)
3. Migration tool transforms configs between versions
4. Breaking changes are explicit, managed events
5. JSON Schema versioned alongside config schema

**Workflow**:
```
1. Configs at config_version: 1
2. Schema change decision (e.g., merge entity_schemas → fields)
3. Write migration transform in ndp-migrate-config (Rust CLI)
4. Run: ndp-migrate-config --from 1 --to 2
5. All configs transformed to config_version: 2
6. Update stream-config.schema.json to v2
7. Commit migrated configs to git
8. App/Validator enforces v2
```

**Discussion Notes**:
```
2026-02-01: Rejected backward compatibility (accumulates complexity).
Rejected app supporting multiple versions (same problem).
Clean breaks with migration tool keeps app simple.
Version in file makes schema explicit and auditable.
JSON format makes migrations straightforward (standard JSON parsing).
```

**Decision**: [x] Breaking changes + migration tool + version in file

---

## Future Consideration: Data Schema Evolution

*Noted during Q8 discussion - out of scope for dp-016*

**Problem**: Source data structure may change over time. When replaying historical Bronze data through Silver ETL, old data may not match current `field_mappings`.

**Example**:
- Jan: Source sends `{"pm25": 10}` → config: `source_path: raw_payload.pm25`
- Feb: Source changes to `{"particulate_matter_25": 10}` → config updated
- Replay Jan data → NULL (old field name not in new config)

**Bronze is safe** - raw payloads preserved as-is.
**Silver needs solution** - field_mappings assume single source structure.

**Potential approaches** (for future feature):
- Fallback paths: `source_path: [new_field, old_field]`
- Time-bounded mappings: `valid_from` per mapping
- Transform functions: `coalesce(new, old)`
- Schema registry: Track source schema versions

**Deferred to**: Future feature (data lineage / schema evolution)

---

## Emergent Decision: Declarative Deploy

*This emerged from Q2/Q3 discussion and is a key architectural addition.*

**Concept**: Agents declare what changed in a manifest. The manifest is part of the release (versioned in git). Deploy reads manifest and executes only necessary actions in correct order.

**Manifest Structure**:
```json
{
  "config_version": 1,
  "changes": [
    {
      "type": "stream",
      "id": "air-quality",
      "reload": "sources"
    },
    {
      "type": "migration",
      "file": "003_add_aqi_table.sql"
    },
    {
      "type": "dimensions",
      "id": "entity_context"
    }
  ]
}
```

**File**: `.deploy/manifest.json` (versioned in git WITH the release)

**Release Model**:
- **Git tags = platform versions** (v1.0.0, v1.1.0, etc.)
- **Manifest is part of the release** - versioned in git with the config
- **Each release has its manifest** defining how to deploy that version
- **Rebuild from any point to any point** by running releases in sequence

**Device-Local State** (NOT in git):
- `/var/ndp/deployed-version` - What's deployed on this device
- `/var/ndp/deployed-at` - Timestamp of last deploy

**Benefits**:
1. Agents don't need to know deploy internals
2. Auditable (manifest in git with the release)
3. Incremental by design
4. Enables rollback via git checkout + deploy
5. Extensible (new change types easy to add)
6. Reproducible (any version can be deployed to any device)
7. JSON format enables IDE autocomplete and validation

**TODO**:
- [ ] Document valid declaration types
- [ ] Add JSON Schema validation for manifest (`manifest.schema.json`)
- [ ] Implement smart deploy.sh that reads manifest

---

## Next Steps

After all questions are answered:
1. Create ADR-016-001 for source of truth decision
2. Create ADR-016-002 for DDL strategy (if needed)
3. Update SYNTHESIS-AND-RECOMMENDATIONS.md with decisions
4. Create IMPLEMENTATION-ROADMAP.md with prioritized features

---

*Questions documented: 2026-02-01*
