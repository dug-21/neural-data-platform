# dp-016: Configuration Architecture Review

## Problem Statement

Adding a new stream to NDP requires navigating multiple configuration systems, manual steps, and undocumented tribal knowledge. The air-012 implementation exposed systemic issues:

1. **Multiple sources of truth** - YAML files, etcd, CSV dimension files
2. **Silent failures** - Config sync errors don't surface, Silver ETL silently doesn't start
3. **No validation** - Invalid configs accepted until runtime failure
4. **Manual DDL** - Silver tables require hand-written SQL
5. **Unclear boundaries** - Overlapping config sections (`fields`, `entity_schemas`, `silver_etl`)
6. **No lifecycle management** - No defined process for updates or deletions

This feature is a **design review**, not implementation. The output is a proposed architecture and prioritized implementation roadmap.

---

## Scope

### In Scope

1. **As-Is Documentation** - Document current "add stream" process with all pain points
2. **Configuration Storage** - How/where config is stored (YAML, etcd, CSV, SQL)
3. **Configuration Deployment** - How config moves from source to runtime
4. **Configuration Consumption** - How each component reads config:
   - Bronze Subscriber
   - Silver ETL
   - Data Dictionary
   - Silver Table Schema
5. **Lifecycle Operations**:
   - Initial load (new streams)
   - Updates (schema changes, new fields)
   - Deletions (deprecating streams)
6. **Silver Layer Design** - Stream consolidation vs joins at query time

### Out of Scope

- Implementation of changes (separate features from roadmap)
- Gold layer configuration (future feature)
- Alert/trigger configuration (future feature)
- Deployment automation (ops concern, separate feature)

---

## Deliverables

| Deliverable | Location | Description |
|-------------|----------|-------------|
| AS-IS-PROCESS.md | `specification/` | Current stream addition process, step-by-step |
| PAIN-POINTS.md | `specification/` | Catalogued issues with current approach |
| CONFIG-INVENTORY.md | `architecture/` | Complete inventory of all config artifacts |
| PROPOSED-ARCHITECTURE.md | `architecture/` | Target state design |
| ADR-016-001-*.md | `architecture/` | Key architectural decisions |
| IMPLEMENTATION-ROADMAP.md | `architecture/` | Prioritized feature list |

---

## Part 1: As-Is Documentation

### Current "Add Stream" Process

Document every step required today to add a new stream, including:

1. **Bronze Configuration**
   - Where to create YAML file
   - Required sections and fields
   - Source configuration (MQTT, HTTP, etc.)
   - Field definitions

2. **Config Deployment**
   - How YAML syncs to etcd
   - What `deploy.sh sync` does
   - Failure modes and their symptoms

3. **Dimension Data**
   - Which CSV file to edit
   - Column definitions
   - How dimensions load to TimescaleDB

4. **Silver Schema**
   - Where DDL files go
   - Required table structure
   - Hypertable conversion
   - Indexes, compression, retention

5. **Silver ETL Configuration**
   - `silver_etl` section in YAML
   - Field mappings
   - DQ rules
   - How ETL discovers streams

6. **Verification Steps**
   - How to confirm Bronze is working
   - How to confirm Silver is working
   - Common failure symptoms

### Known Pain Points to Document

| Area | Pain Point |
|------|------------|
| Storage | YAML vs etcd dual source of truth |
| Storage | Dimension data in separate CSV |
| Validation | No schema validation at sync time |
| Validation | Invalid field references accepted |
| Deployment | Manual SQL execution required |
| Deployment | Manual Pi SSH for restarts |
| Consumption | `list_streams()` only sees etcd, not YAML |
| Consumption | Silver ETL loads from YAML, not etcd |
| Lifecycle | No update path (delete and recreate) |
| Lifecycle | No deletion process defined |
| Structure | Overlapping `fields` / `entity_schemas` |
| Structure | `silver_etl` in `extra` section hack |

---

## Part 2: Configuration Inventory

### Questions to Answer

**Storage:**
- What config artifacts exist? (YAML, etcd keys, CSV, SQL)
- What is the intended source of truth for each?
- What is the actual source of truth for each?
- Where do they diverge?

**Deployment:**
- What moves config from source to runtime?
- What validation occurs at each stage?
- What are the failure modes?
- How are failures surfaced (or not)?

**Consumption:**
- Which components read which configs?
- Do components agree on source of truth?
- What happens when sources disagree?

### Components to Analyze

| Component | Config Source | What It Reads |
|-----------|---------------|---------------|
| ConfigSyncService | YAML files | Stream definitions |
| StreamRegistry | etcd | Stream list, StreamConfig |
| BronzeSubscriber | etcd (via registry) | Source config, field definitions |
| SilverSubscriber | YAML (direct) | silver_etl section |
| DimensionLoader | CSV files | Entity context, etc. |
| Silver DDL | SQL files | Table schemas |
| Data Dictionary | ? | Column metadata |

---

## Part 3: Proposed Architecture

### Design Questions

1. **Single Source of Truth**
   - Should all config live in YAML → sync to etcd?
   - Or should etcd be the source, with YAML as bootstrap?
   - Or eliminate etcd for config entirely?

2. **Config Structure**
   - Should stream config be monolithic or componentized?
   - Bronze config separate from Silver config?
   - Dimension entries inline or separate?

3. **Validation Pipeline**
   - When should validation occur? (edit time, sync time, startup time)
   - What should be validated? (syntax, semantics, references)
   - How should errors surface?

4. **Schema Generation**
   - Should Silver DDL be generated from config?
   - What about indexes, compression, retention policies?
   - How to handle schema evolution?

5. **Silver Layer Design**
   - One Silver table per stream? Or consolidated tables?
   - If consolidated: by source type? by domain?
   - Trade-offs: simplicity vs query flexibility

6. **Lifecycle Management**
   - How to update a stream config?
   - How to deprecate/delete a stream?
   - Schema migration strategy?

### Candidate Architectures

**Option A: YAML as Source, etcd as Cache**
```
YAML → validate → sync to etcd → components read etcd
```
- Pro: YAML is human-readable, git-versioned
- Con: Sync step can fail silently (current problem)

**Option B: etcd as Source, YAML as Bootstrap**
```
YAML (initial) → etcd ← API for updates
                   ↓
            components read etcd
```
- Pro: Single runtime source of truth
- Con: Editing requires API, not text editor

**Option C: YAML Only, No etcd for Config**
```
YAML → validate → components read YAML directly
```
- Pro: Simplest, no sync
- Con: No runtime config updates, file mounts required

**Option D: Database as Source**
```
YAML (import) → PostgreSQL config tables → components read DB
```
- Pro: Transactional, queryable, validated
- Con: More infrastructure, migration complexity

---

## Part 4: Silver Layer Considerations

### Current Approach

One Silver table per stream:
- `silver.air_quality_readings` (AirGradient)
- `silver.weather_readings` (Outdoor weather)
- `silver.forecast_readings` (NWS forecasts)
- `silver.state_events` (Home Assistant)

### Alternative: Consolidated Tables

**By measurement type:**
- `silver.sensor_readings` (all numeric time-series)
- `silver.state_events` (all discrete state changes)
- `silver.forecast_data` (all predictions)

**Trade-offs:**

| Aspect | Per-Stream Tables | Consolidated Tables |
|--------|-------------------|---------------------|
| Schema clarity | Clear, typed columns | Generic, flexible |
| Query simplicity | JOIN across tables | Single table queries |
| Schema evolution | Change one table | Affects all streams |
| Storage efficiency | Optimized per stream | Some column waste |
| New stream effort | New table DDL | Just config |

### Questions to Resolve

1. Do we need cross-stream queries often enough to justify consolidation?
2. Would a hybrid work? (consolidated for similar streams, separate for unique)
3. How does this affect Gold layer design?

---

## Part 5: Implementation Roadmap

The architecture review will produce a prioritized list of implementation features. Candidate features (order TBD by review):

| Feature | Description | Dependency |
|---------|-------------|------------|
| Unified Config Source | Eliminate YAML/etcd split (air-013) | None |
| Config Validation | Schema and semantic validation | Unified source |
| Silver DDL Generation | Create tables from config (dp-015) | Validation |
| Dimension Integration | Inline dimensions in stream config | Unified source |
| Lifecycle Management | Update and delete operations | All above |
| Observability | Startup validation, health checks | Validation |

The review will determine:
- Which features are needed
- What order to implement
- What can be combined
- What should be deferred

---

## Success Criteria

1. **As-Is documented** - Anyone can follow the current process (ugly as it is)
2. **Problems catalogued** - Every pain point from air-012 captured
3. **Architecture proposed** - Clear target state with rationale
4. **Decisions recorded** - ADRs for key choices
5. **Roadmap produced** - Prioritized implementation plan with dependencies

---

## Approach

### Phase 1: Discovery (Specification)

1. Walk through current codebase
2. Document each config artifact and its consumers
3. Trace data flow from YAML to running components
4. Catalogue all pain points and failure modes

### Phase 2: Analysis (Architecture)

1. Evaluate candidate architectures against requirements
2. Analyze Silver layer consolidation trade-offs
3. Draft proposed architecture
4. Identify key decisions requiring ADRs

### Phase 3: Planning (Roadmap)

1. Break proposed architecture into implementation features
2. Identify dependencies between features
3. Prioritize based on impact and effort
4. Produce implementation roadmap

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| air-012 retrospective | Complete | Provides pain point input |
| air-013 scope | Complete | Will be absorbed/refined |
| dp-015 scope | Complete | Will be absorbed/refined |

---

## References

- `product/features/air-012/reports/RETROSPECTIVE.md` - Source of pain points
- `product/features/air-013/SCOPE.md` - Unified config source (to absorb)
- `product/features/dp-015/SCOPE.md` - Silver DDL generation (to absorb)

---

## Notes

This is a **design feature**, not an implementation feature. The output is documentation and a roadmap. Implementation will be tracked as separate features derived from the roadmap.

Expected artifacts:
- 2-3 specification documents
- 3-4 architecture documents including ADRs
- 1 implementation roadmap

---

*Scope created: 2026-01-31*
*Triggered by: air-012 retrospective*
