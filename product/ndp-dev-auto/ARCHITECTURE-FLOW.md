# NDP Development Flow: End-to-End Architecture

How a feature goes from idea to deployed knowledge. Every system, every handoff, one page.

---

## 1. One-Page Overview

```
USER                    AGENTS                         SYSTEMS
----                    ------                         -------

 Idea
  |
  v
 Write SCOPE.md -----> product/features/{id}/SCOPE.md
  |
  v
 "Plan {id}" --------> Primary Agent
                          |
                          | /get-pattern
                          v
                        AgentDB ---------> Existing patterns, ADRs
                          |
                          | Task(ndp-scrum-master)
                          v
                        PLANNING SWARM
                          |  hive-mind_init ---------> Claude-Flow MCP (coordination)
                          |  TaskCreate (batch) -----> Task tracking
                          |  memory_store -----------> Transient memory (namespace)
                          |
                          |  Task() spawn parallel:
                          |    ndp-architect ---------> architecture/ARCHITECTURE.md
                          |    specification ---------> specification/SPECIFICATION.md
                          |    pseudocode ------------> pseudocode/PSEUDOCODE.md
                          |
                          |  Task(ndp-vision-guardian) > ALIGNMENT-REPORT.md
                          |  /save-pattern (ADRs) ---> AgentDB (permanent)
                          |  Generate ----------------> IMPLEMENTATION-BRIEF.md
                          |  gh issue create ---------> GitHub Issue (#N)
                          v
                        Primary Agent
                          |  present variances to user
                          |  /reflexion
                          v
 Approve variances      USER
  |
  v
 "Implement {id}" ----> Primary Agent
                          |
                          | /get-pattern
                          | gh issue view #N (read brief)
                          | Task(ndp-scrum-master)
                          v
                        IMPLEMENTATION SWARM
                          |  hive-mind_init
                          |  /spec-compile -----------> ADRs to AgentDB
                          |                             Brief sections to Claude-Flow memory
                          |                             Level-1 summary for agent prompts
                          |
                          |  Task() spawn wave:
                          |    ndp-rust-dev -----------> Rust code
                          |    ndp-tester -------------> Tests
                          |    ndp-timescale-dev ------> Schema/ETL
                          |
                          |  Drift check
                          |  Validate (Tier 1/2/3) ---> cargo build/test/clippy
                          |                             deploy.sh (integration)
                          |  gh issue comment --------> GitHub Issue (#N)
                          v
                        Primary Agent
                          |  /reflexion
                          |  /save-pattern
                          v
 Review results         USER
  |
  v
 "Release" -----------> Release workflow
                          |  .deploy/releases/vX.Y.Z.manifest.json
                          |  git tag vX.Y.Z
                          |  CHANGELOG.md
                          v
 Deploy to Pi --------> deploy.sh build/deploy/status
```

---

## 2. Phase Breakdown

### Phase 1: Scope (Human)

| Aspect | Detail |
|--------|--------|
| Who | Human only |
| Action | Write `product/features/{id}/SCOPE.md` |
| Artifacts | SCOPE.md (agents never modify this) |
| Memory | None |

Feature IDs follow `{phase}-{NNN}`: dp-001, fe-003, ops-005, etc.

### Phase 2: Plan (Planning Swarm)

```
Primary Agent -----> ndp-scrum-master -----> { ndp-architect, spec, pseudocode }
                                       \---> ndp-vision-guardian
```

| Step | Actor | Skills/Tools | Memory | Artifacts |
|------|-------|-------------|--------|-----------|
| Pattern search | Primary | `/get-pattern` | Read AgentDB | -- |
| Delegate | Primary | Task(ndp-scrum-master) | -- | -- |
| Init coordination | Scrum-master | `hive-mind_init` MCP | Write Claude-Flow | state.json |
| Define tasks | Scrum-master | `TaskCreate` (batch) | Write Claude-Flow | Task list |
| Seed context | Scrum-master | `memory_store` MCP | Write Claude-Flow | {id}-context |
| Spec + Arch | Parallel agents | Read, Write, Glob | Read Claude-Flow | SPECIFICATION.md, ARCHITECTURE.md, PSEUDOCODE.md |
| Vision check | ndp-vision-guardian | Read alignment criteria | -- | ALIGNMENT-REPORT.md |
| Store ADRs | Scrum-master | `/save-pattern` | Write AgentDB | Pattern IDs |
| Generate brief | Scrum-master | Write | -- | IMPLEMENTATION-BRIEF.md |
| Track | Scrum-master | `gh issue create` | -- | GitHub Issue #N |
| Learn | Primary | `/reflexion` | Write AgentDB | Reflexion entries |

### Phase 3: Implement (Implementation Swarm)

```
Primary Agent -----> ndp-scrum-master -----> { ndp-rust-dev, ndp-tester, ... }
                                       \---> drift check + validate
```

| Step | Actor | Skills/Tools | Memory | Artifacts |
|------|-------|-------------|--------|-----------|
| Pattern search | Primary | `/get-pattern` | Read AgentDB | -- |
| Read brief | Primary | `gh issue view #N` | -- | -- |
| Delegate | Primary | Task(ndp-scrum-master) | -- | -- |
| Compile spec | Scrum-master | `/spec-compile` | Write both (ADRs permanent, sections transient) | Level-1 summary |
| Init coordination | Scrum-master | `hive-mind_init` MCP | Write Claude-Flow | state.json |
| Define tasks | Scrum-master | `TaskCreate` (batch) | Write Claude-Flow | Task list |
| Spawn wave | Scrum-master | Task() parallel | -- | -- |
| Code | ndp-rust-dev | Read, Write, Edit, Bash | Read both | Rust source |
| Test | ndp-tester | Read, Write, Bash | Read both | Test files |
| Drift check | Scrum-master | File comparison | Read Claude-Flow | -- |
| Validate T1 | Scrum-master | `cargo build/test` | -- | Pass/fail |
| Validate T2 | Scrum-master | `cargo clippy` | -- | Pass/fail |
| Validate T3 | Scrum-master | `deploy.sh` or `docker compose` | -- | Pass/fail |
| Update tracking | Scrum-master | `gh issue comment` | -- | Issue comment |
| Learn | Primary | `/reflexion`, `/save-pattern` | Write AgentDB | Patterns + reflexions |

Multi-wave: repeat spawn-drift-validate per wave. Max 2 fix iterations per wave.

### Phase 4: Release (Human + Agent)

| Step | Actor | Tool | Artifact |
|------|-------|------|----------|
| Manifest | Agent | Write | `.deploy/releases/vX.Y.Z.manifest.json` |
| Tag | Agent | `git tag -a vX.Y.Z` | Annotated tag |
| Changelog | Agent | Edit | CHANGELOG.md entry |
| Deploy | Human/script | `deploy.sh build && deploy.sh deploy` | Running containers |
| Verify | Human/script | `deploy.sh status` | Health check |

### Phase 5: Learn (Post-feature)

| Step | Actor | Tool | Memory |
|------|-------|------|--------|
| Rate each pattern | Primary | `/reflexion` (per pattern ID) | Write AgentDB |
| Store discoveries | Primary | `/save-pattern` | Write AgentDB |
| Auto-discover | Primary | `/learner` (periodic) | Write AgentDB |

---

## 3. System Connections

### Two Memory Systems

```
                    +---------------------------+
                    |        AgentDB            |
                    |   (permanent knowledge)   |
                    |                           |
  /get-pattern ---->|  Patterns table           |
  /save-pattern --->|  (semantic embeddings)    |
  /reflexion ------>|  Reflexion table           |
                    |  (reward-ranked feedback) |
                    +---------------------------+
                              ^
                              | ADRs stored permanently
                              | Reflexion ranks patterns
                              |
  - - - - - - - - - - - - - - | - - - - - - - - - - - -
                              |
                    +---------------------------+
                    |    Claude-Flow Memory      |
                    |   (transient coordination) |
                    |                           |
  memory_store ---->|  Namespaced key-value     |
  memory_search --->|  (spec sections, results) |
  memory_retrieve ->|  (task status, context)   |
                    +---------------------------+
                              ^
                              | Spec sections live here during swarm
                              | Agent results stored here
                              | Dies when swarm ends
```

**Rule**: Useful 6 months from now --> AgentDB. Useful only during this swarm --> Claude-Flow memory.

### Skill-to-System Mapping

| Skill | Underlying Tool | System |
|-------|----------------|--------|
| `/get-pattern` | `agentdb_pattern_search` | AgentDB (permanent) |
| `/save-pattern` | `agentdb_pattern_store` | AgentDB (permanent) |
| `/reflexion` | `agentdb_reflexion_store` | AgentDB (permanent) |
| `/spec-compile` | `memory_store` + `agentdb_pattern_store` | Both |
| `/swarm-run` | `hive-mind_init` + Task tool | Claude-Flow + Claude Code |
| `/align` | Read alignment criteria | File system |
| `/validate` | `cargo` + `deploy.sh` | Build system |

### Agent Routing

```
Task Type          Primary Agents                     Topology
---------          --------------                     --------
Feature            ndp-scrum-master + architect +      hierarchical
                   rust-dev + tester + reviewer
Bug Fix            ndp-scrum-master + researcher +     hierarchical
                   rust-dev + tester
Refactor           ndp-scrum-master + architect +      hierarchical
                   rust-dev + reviewer
Schema/ETL         architect + timescale-dev +         hierarchical
                   dq-engineer + domain specialist
```

Always use NDP-specific agents over generic ones (ndp-rust-dev not coder, ndp-architect not system-architect).

### Coordination Architecture

```
Claude-Flow MCP Layer              Claude Code Runtime Layer
(state, memory, tracking)          (actual agent processes)

  hive-mind/init  ──────────────>  Task tool spawns real agents
  memory/store    <──────────────  Agents write results back
  task/create     <──────────────  Agents update task status
  hive-mind/status ─────────────>  Orchestrator checks progress
```

Key insight: `claude-flow swarm init` and `agent spawn` CLI commands are cosmetic -- they create metadata only. The Task tool creates real running agents. MCP tools provide the coordination backbone.

### Tracking Flow

```
SCOPE.md                          GitHub Issue #N
  |                                  ^        |
  | planning swarm reads             |        | impl swarm reads brief
  v                                  |        v
IMPLEMENTATION-BRIEF.md ----------->+  gh issue comment (wave results)
                                     |
                                     v
                                  Commits reference (#N)
                                     |
                                     v
                                  .deploy/releases/vX.Y.Z.manifest.json
```

### Validation Tiers

```
Tier 1 (always)     cargo build --workspace && cargo test --workspace
Tier 2 (new code)   cargo clippy --workspace -- -D warnings
Tier 3a (binary)    DEPLOY_ENV=integration ./deploy/pi/deploy.sh build/deploy/status/stop
Tier 3b (schema)    docker compose -f docker-compose.integration.yml up -d / down -v
```

Qualifying paths for Tier 3a: core/, apps/, crates/, config/base/streams/.
Qualifying paths for Tier 3b: tools/ndp-gold-ddl/, deploy/pi/init-scripts/, config/grafana/.

---

## 4. Current Gaps

| Gap | Impact | Evidence |
|-----|--------|----------|
| No automated validation gate | Validation is manual -- scrum-master runs cargo/deploy.sh but nothing prevents merging a failing build | implementation-protocol.md: "max 2 validation fix iterations" then gives up |
| Reflexion compliance is honor-system | No enforcement that agents actually call `/reflexion` after work; settings.json has a reminder but it is not blocking | pattern-workflow.md: "A session without reflexion is incomplete" but no gate |
| spec-compile is optional | Implementation swarm can skip `/spec-compile`, leaving agents without Level-1 summary; drift risk increases | implementation-protocol.md: "if /spec-compile was run" (conditional) |
| No cross-swarm coordination | Two concurrent swarms on overlapping files have no locking or conflict detection | swarm-protocol.md has no mention of file locks or concurrent feature work |
| Planning-to-implementation handoff is manual | User must explicitly trigger "implement {id}" after planning completes; no automated pipeline | planning-protocol.md ends with "present variances to user" then stops |
| Integration test stubs | 10 empty integration test stubs in fe-003; anti-stub rule exists but was not enforced | MEMORY.md: "10 empty integration test stubs" |
| Vision alignment is advisory | ALIGNMENT-REPORT.md flags variances but nothing blocks implementation if user ignores them | ALIGNMENT-CRITERIA.md: VARIANCE classification says "present to user" only |
| No rollback procedure | Release policy defines forward deployment but no documented rollback if deploy.sh fails on Pi | Release artifacts are forward-only; deploy.sh has no `rollback` subcommand |
