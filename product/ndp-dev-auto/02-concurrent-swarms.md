# Concurrent Multi-Swarm Architecture Research

## Executive Summary

The NDP project currently operates a single-swarm-at-a-time model: one ndp-scrum-master coordinator runs one swarm (planning or implementation) per user session. This research examines the feasibility, architecture, and risks of running multiple swarms simultaneously -- for example, a planning swarm for fe-005 while an implementation swarm runs for fe-004.

**Key finding**: Concurrent swarms are feasible today with namespace isolation, but require changes to three areas: (1) the hive-mind initialization model (currently singleton), (2) memory namespace conventions (currently feature-scoped but not enforced), and (3) file-system coordination for shared crate files. The claims system already exists in claude-flow to handle file-level contention but is unused by current protocols.

**Complexity**: Medium. The MCP tooling already supports namespaced memory and task tracking. The main gaps are the singleton hive-mind state file and the lack of inter-swarm awareness.

---

## Current Architecture: Evidence of Single-Swarm Design

### 1. Singleton Hive-Mind State

The hive-mind persists to a single file at `.claude-flow/hive-mind/state.json`:

```json
{
  "initialized": true,
  "topology": "mesh",
  "workers": [],
  "consensus": { "pending": [], "history": [] },
  "sharedMemory": {},
  "queen": { "agentId": "research-lead", "electedAt": "2026-02-15T04:48:01.237Z", "term": 1 }
}
```

**Problem**: `hive-mind_init` overwrites this file. If Swarm A initializes and Swarm B calls `hive-mind_init` later, Swarm A's state (workers, consensus, sharedMemory) is destroyed.

**Source**: `.claude/rules/implementation-protocol.md` line 120: "This creates `.claude-flow/hive-mind/state.json` for shared coordination state."

### 2. Single Coordinator Pattern

Both `planning-protocol.md` and `implementation-protocol.md` specify spawning a single ndp-scrum-master as the coordinator. The primary agent delegates everything to this one coordinator, then waits.

From `swarm-protocol.md` line 15: "the primary agent spawns `ndp-scrum-master` as the single coordinator, who then spawns worker agents, monitors results, detects drift, and controls flow."

There is no concept of multiple coordinators running concurrently within these protocols.

### 3. Task Store is Flat (No Swarm Scoping)

The task store at `.claude-flow/tasks/store.json` is a flat map of task IDs with no swarm-level grouping. Tasks from different swarms would commingle. While tasks have `tags` arrays that could scope them, no convention enforces this.

### 4. Memory Namespaces Exist But Are Not Isolation-Enforced

Claude-flow memory supports namespaces (the `namespace` parameter on `memory_store`). Current convention uses feature IDs as namespaces (e.g., `namespace: "fe-003"`). This is the strongest existing isolation mechanism, but nothing prevents one swarm from reading/writing another swarm's namespace.

### 5. Daemon Workers Are Already Concurrent

The daemon state (`.claude-flow/daemon-state.json`) shows background workers like `map`, `audit`, `optimize`, `consolidate`, and `testgaps` already run concurrently with `maxConcurrent: 2`. These are lightweight (1-2ms average), not full swarms, but they demonstrate the system can handle parallel workloads.

---

## Proposed Architecture for Concurrent Swarms

### Design Principle: Swarm ID as Universal Namespace Key

Every swarm gets a unique ID (e.g., `swarm-fe004-impl`, `swarm-fe005-plan`). This ID prefixes all coordination state:

```
Memory namespace:  swarm-fe004-impl/{feature-id}
Task tags:         ["swarm-fe004-impl", ...]
Hive-mind ID:      swarm-fe004-impl
Claims context:    swarm-fe004-impl
```

### Layer 1: Hive-Mind Isolation

**Option A -- Multiple State Files (recommended)**:

Instead of a single `state.json`, the hive-mind stores state per swarm ID:

```
.claude-flow/hive-mind/
  swarm-fe004-impl.json
  swarm-fe005-plan.json
  state.json            (legacy, points to active default)
```

This requires a change to `hive-mind_init` to accept an optional `swarmId` parameter, or the MCP server must be patched to namespace state files.

**Option B -- Ignore hive-mind, use memory-only coordination**:

Since the hive-mind state is currently just metadata (workers, consensus are empty in practice), swarms could skip `hive-mind_init` entirely and use only `memory_store`/`memory_search` with namespaced keys. This is simpler but loses the consensus and broadcast capabilities.

**Recommendation**: Option B for Phase 1 (immediate), Option A for Phase 2 (when consensus becomes valuable).

### Layer 2: Memory Namespace Convention

Enforce a two-tier namespace: `{swarm-id}/{purpose}`.

```
swarm-fe004-impl/context       -- Swarm context and brief
swarm-fe004-impl/wave-1-result -- Wave results
swarm-fe004-impl/directive-1   -- Corrective directives
swarm-fe005-plan/context       -- Independent planning swarm
swarm-fe005-plan/spec-output   -- Planning artifacts
```

Each swarm's agents only read/write within their swarm's namespace prefix. Cross-swarm reads are explicitly prohibited unless using the inter-swarm communication channel (see Layer 5).

### Layer 3: Task Scoping

Tag every task with the swarm ID:

```
task_create(
  type: "feature",
  description: "Implement Bronze adapter",
  tags: ["swarm-fe004-impl", "fe-004", "wave-1"],
  assignTo: ["agent-1"]
)
```

Filter task lists by swarm tag: `task_list(tags: "swarm-fe004-impl")`.

### Layer 4: File System Coordination via Claims

The claims system (`claims_claim`, `claims_release`, `claims_handoff`, `claims_steal`) already exists in claude-flow but is completely unused by current swarm protocols.

For concurrent swarms, file-level claims prevent destructive contention:

```
# Swarm A claims Cargo.toml for workspace member addition
claims_claim(issueId: "Cargo.toml", claimant: "agent:swarm-fe004-impl:rust-dev")

# Swarm B tries to claim same file -- gets DENIED or QUEUED
claims_claim(issueId: "Cargo.toml", claimant: "agent:swarm-fe005-plan:architect")
  --> Returns: "Already claimed by swarm-fe004-impl"
```

Critical shared files requiring claims:
- `Cargo.toml` (workspace members)
- `config/base/streams/*.toml` (stream configs)
- `deploy/pi/deploy.sh` (deployment script)
- `docker-compose.*.yml` (Docker configs)
- Any crate's `mod.rs` if both swarms touch the same crate

Files that do NOT need claims (inherently isolated by feature):
- `product/features/{feature-id}/**` (each feature owns its directory)
- New crate directories (each swarm creates new crates in separate paths)
- Test files within feature-scoped crates

### Layer 5: Inter-Swarm Communication

A dedicated namespace `inter-swarm` serves as the message bus:

```
# Swarm A publishes a dependency signal
memory_store(
  key: "swarm-fe004-impl/needs-from/swarm-fe005-plan",
  value: "Need ADR for embedding storage format before implementing Gold materialized view",
  namespace: "inter-swarm"
)

# Swarm B checks for inbound dependencies
memory_search(
  query: "needs-from/swarm-fe005-plan",
  namespace: "inter-swarm"
)
```

**When to communicate**:
- Swarm discovers it depends on another swarm's output
- Swarm modifies a shared interface (trait, struct, config schema)
- Swarm encounters a blocker that another swarm created
- Priority change requires one swarm to yield resources

**When to stay isolated**:
- Normal execution within feature scope
- Reading/writing feature-scoped files
- Running validation on feature-scoped code

### Layer 6: Priority and Scheduling

A swarm priority system using the existing `priority` field on tasks:

| Priority | Swarm Type | Example |
|----------|-----------|---------|
| critical | Active bug fix | BUG-006 hotfix swarm |
| high | Implementation swarm for current sprint | fe-004 impl |
| normal | Planning swarm for next sprint | fe-005 plan |
| low | Research/exploration swarm | architecture spike |

Priority affects:
- **Context window budget**: Higher-priority swarms get larger agent pools
- **File claim resolution**: Higher-priority swarm wins claim conflicts
- **API rate allocation**: If rate-limited, lower-priority swarms pause agents

### Layer 7: Failure Isolation

Each swarm is independently recoverable:

1. **Agent failure**: Scrum-master within the swarm handles re-spawn (existing drift correction, max 2 iterations)
2. **Coordinator failure**: The primary agent spawns a replacement scrum-master with the swarm's namespace; the new coordinator reads memory to resume
3. **Memory corruption**: Each swarm's state is namespaced; corruption in one namespace does not affect others
4. **Full swarm failure**: `hive-mind_shutdown(swarmId: "swarm-fe004-impl")` cleans up one swarm without affecting others
5. **Cascade prevention**: Swarms never share agent processes. A stuck agent in Swarm A cannot block Swarm B's agents.

---

## Implementation Complexity Assessment

### Minimal Changes Required (Phase 1 -- "Namespace-Only")

| Change | Effort | Risk |
|--------|--------|------|
| Add swarm-ID prefix to memory namespace convention | Low (documentation + prompt updates) | Low |
| Add swarm-ID tags to all task_create calls | Low (prompt updates) | Low |
| Skip `hive-mind_init` for concurrent swarms (use memory-only coordination) | Low (remove one MCP call) | Low |
| Add claims_claim for shared files in agent prompts | Medium (new protocol step) | Low |
| Update ndp-scrum-master agent definition to accept swarm-ID | Medium (agent prompt rewrite) | Medium |

**Estimated effort**: 1-2 days of protocol documentation updates + agent definition changes. No code changes to claude-flow MCP server.

### Full Changes Required (Phase 2 -- "Multi-Hive")

| Change | Effort | Risk |
|--------|--------|------|
| Patch `hive-mind_init` to accept swarmId and store per-swarm state files | Medium (MCP server change) | Medium |
| Build inter-swarm communication protocol | Medium (new convention + prompts) | Low |
| Build priority-based claim resolution | High (new logic in claims system) | Medium |
| Build swarm-aware `task_list` filtering | Low (already supports tag filtering) | Low |
| Build swarm lifecycle management (spawn, monitor, shutdown independently) | High (new orchestration layer) | High |

**Estimated effort**: 5-8 days including MCP server patches, protocol updates, and testing.

---

## Risk Analysis

### High Risk

**R1: Cargo.lock / Cargo.toml Contention**
Two implementation swarms both modifying `Cargo.toml` (adding workspace members) or triggering `cargo build` simultaneously will corrupt `Cargo.lock`.
**Mitigation**: Only one implementation swarm runs at a time. Planning swarms do not touch Rust files, so plan+impl is safe.

**R2: Context Window Exhaustion**
Each spawned agent consumes a Claude context window. Running two 8-agent swarms simultaneously means 16+ concurrent context windows plus the primary agent's window.
**Mitigation**: Limit concurrent agent count across all swarms. The daemon config already has `maxConcurrent: 2` for background workers; extend this to swarm agents.

### Medium Risk

**R3: API Rate Limits**
Anthropic API rate limits may throttle concurrent swarms, causing timeouts and retries.
**Mitigation**: Stagger agent spawns across swarms. Use Haiku for simple tasks, reserving Opus/Sonnet capacity for complex work.

**R4: Memory Namespace Collisions**
If an agent in Swarm A accidentally writes to Swarm B's namespace, it could corrupt coordination state.
**Mitigation**: Strict naming conventions enforced by scrum-master prompt. Memory keys include swarm-ID prefix.

**R5: Git Merge Conflicts**
Two swarms committing to the same branch will create merge conflicts.
**Mitigation**: Each swarm works on a separate git branch. Merge only after both complete.

### Low Risk

**R6: Hive-Mind State Overwrite (Phase 1)**
If we skip `hive-mind_init` for Phase 1, swarms lose consensus and broadcast features.
**Mitigation**: These features are unused in practice today (consensus.pending and consensus.history are empty arrays). No functional loss.

**R7: Swarm Ordering Dependencies**
Planning swarm for fe-005 might produce artifacts that fe-004's implementation swarm needs.
**Mitigation**: Inter-swarm communication protocol (Layer 5) handles explicit dependency signaling.

---

## Phased Adoption Plan

### Phase 0: Proof of Concept (0.5 day)

Run two swarms manually in a single session:
1. Start a planning swarm for ops-005 using `namespace: "swarm-ops005-plan"`
2. While it runs, start a research swarm using `namespace: "swarm-research-001"`
3. Verify: memory isolation works, no state corruption, both produce results

No protocol changes. Just test the theory with existing tools.

### Phase 1: Safe Concurrency -- Plan + Impl (2 days)

**Scope**: Allow one planning swarm and one implementation swarm to run concurrently. Planning swarms never touch code files, so file contention is impossible.

Changes:
1. Update `swarm-protocol.md` with concurrent swarm namespace convention
2. Update `ndp-scrum-master.md` to accept `swarmId` parameter and prefix all memory keys
3. Add claims_claim step to implementation protocol for shared files
4. Document which swarm type combinations are safe:

| Swarm A | Swarm B | Safe? | Why |
|---------|---------|-------|-----|
| Planning | Planning | Yes | Both write to separate `product/features/` dirs |
| Planning | Implementation | Yes | No file overlap (planning writes docs, impl writes code) |
| Implementation | Implementation | NO | Shared Cargo.toml, Cargo.lock, potentially shared crates |
| Planning | Research | Yes | Research is read-only |
| Implementation | Research | Yes | Research is read-only |

5. Skip `hive-mind_init` in favor of memory-only coordination
6. Add `inter-swarm` namespace for dependency signaling

### Phase 2: Multi-Impl with Claims (5 days)

**Scope**: Allow two implementation swarms if they operate on non-overlapping crate sets.

Changes:
1. Integrate claims system into implementation protocol:
   - Before editing a file, agent calls `claims_claim`
   - After editing, agent calls `claims_release`
   - On conflict, agent waits or escalates to scrum-master
2. Build crate-level ownership map:
   - Swarm A owns: `crates/ndp-intelligence/`, `apps/ndp-intelligence-app/`
   - Swarm B owns: `tools/ndp-cli/`, `tools/ndp-validate/`
   - Shared: `Cargo.toml` (claimed sequentially)
3. Implement staggered `cargo build` -- only one swarm builds at a time
4. Branch-per-swarm git workflow

### Phase 3: Full Multi-Hive (8 days)

**Scope**: Multiple hive-minds with consensus, priority scheduling, and automatic resource balancing.

Changes:
1. Patch claude-flow MCP to support `swarmId` on hive-mind operations
2. Build swarm registry (tracks all active swarms, their priority, agent count)
3. Implement priority-based preemption (low-priority swarm pauses if high-priority needs resources)
4. Build swarm dashboard via `claims_board` extension
5. Implement swarm session save/restore for long-running swarms that need to pause

---

## Practical Example: Planning fe-005 While Implementing fe-004

### Setup

```
Primary Agent Session:
  1. get-pattern for fe-004 implementation
  2. Spawn scrum-master-A with swarmId="swarm-fe004-impl"
     - Scrum-master-A reads implementation protocol
     - Uses namespace "swarm-fe004-impl"
     - Spawns ndp-rust-dev, ndp-tester agents
     - All agents read/write only in swarm-fe004-impl namespace

  3. get-pattern for fe-005 planning (while swarm A runs)
  4. Spawn scrum-master-B with swarmId="swarm-fe005-plan"
     - Scrum-master-B reads planning protocol
     - Uses namespace "swarm-fe005-plan"
     - Spawns ndp-architect, specification, pseudocode agents
     - All agents write only to product/features/fe-005/

  5. Wait for both to complete
  6. Synthesize results from both namespaces
  7. Reflexion for both
```

### Memory Layout

```
Namespace: swarm-fe004-impl
  key: fe-004-context          value: {brief, constraints, pattern IDs}
  key: wave-1-result           value: {agent-1 output, agent-2 output}
  key: directive-1             value: {drift correction}

Namespace: swarm-fe005-plan
  key: fe-005-context          value: {scope, goals, pattern IDs}
  key: spec-output             value: {specification artifact path}
  key: arch-output             value: {architecture artifact path}

Namespace: inter-swarm
  key: swarm-fe005-plan/publishes/adr-embedding-format
    value: "ADR-001 decided on pgvector storage. Pattern ID: 67"
```

### File Ownership

```
Swarm A (fe-004 impl):        Swarm B (fe-005 plan):
  core/src/*.rs                  product/features/fe-005/specification/
  crates/ndp-intelligence/*.rs   product/features/fe-005/architecture/
  apps/ndp-intelligence-app/     product/features/fe-005/pseudocode/
  Cargo.toml (claimed)           product/features/fe-005/IMPLEMENTATION-BRIEF.md
                                 product/features/fe-005/ALIGNMENT-REPORT.md
                                 (NO code files touched)
```

Zero file overlap. Safe to run concurrently.

---

## Files Referenced in This Research

| File | Path | Relevance |
|------|------|-----------|
| Swarm Protocol | `/workspaces/neural-data-platform/.claude/rules/swarm-protocol.md` | Base protocol, single-swarm design |
| Implementation Protocol | `/workspaces/neural-data-platform/.claude/rules/implementation-protocol.md` | Implementation swarm flow |
| Planning Protocol | `/workspaces/neural-data-platform/.claude/rules/planning-protocol.md` | Planning swarm flow |
| Swarm-Run Skill | `/workspaces/neural-data-platform/.claude/skills/swarm-run/SKILL.md` | Working swarm launcher |
| Scrum Master Agent | `/workspaces/neural-data-platform/.claude/agents/ndp/ndp-scrum-master.md` | Coordinator agent definition |
| Agent Routing | `/workspaces/neural-data-platform/.claude/rules/agent-routing.md` | Agent selection and team formation |
| Hive-Mind State | `/workspaces/neural-data-platform/.claude-flow/hive-mind/state.json` | Singleton state file (the bottleneck) |
| Daemon State | `/workspaces/neural-data-platform/.claude-flow/daemon-state.json` | Background worker concurrency config |
| Task Store | `/workspaces/neural-data-platform/.claude-flow/tasks/store.json` | Flat task store (no swarm scoping) |
| Swarm Config | `/workspaces/neural-data-platform/.claude-flow/swarm/swarm-config.json` | Legacy swarm config (singleton) |
| Config | `/workspaces/neural-data-platform/.claude-flow/config.yaml` | maxAgents: 15, topology: hierarchical-mesh |
| Capabilities | `/workspaces/neural-data-platform/.claude-flow/CAPABILITIES.md` | Full MCP tool reference |
| Dual-Mode Skills | `/workspaces/neural-data-platform/.claude/skills/dual-mode/` | Existing parallel execution patterns |
| Cargo.toml | `/workspaces/neural-data-platform/Cargo.toml` | Workspace members (contention point) |
| Testing Rules | `/workspaces/neural-data-platform/.claude/rules/testing.md` | Integration environment (shared resource) |

---

## MCP Tools Relevant to Concurrent Swarms

### Already Available and Useful

| Tool | Purpose in Concurrent Architecture |
|------|-----------------------------------|
| `memory_store` (with namespace) | Isolated per-swarm coordination state |
| `memory_search` (with namespace) | Scoped retrieval within swarm |
| `task_create` (with tags) | Swarm-scoped task tracking |
| `task_list` (with tag filter) | Filter tasks by swarm |
| `claims_claim` | File-level lock before editing |
| `claims_release` | Release file lock after editing |
| `claims_handoff` | Transfer file ownership between swarms |
| `claims_steal` | Priority-based file claim override |
| `claims_board` | Visual overview of all active claims |
| `session_save` / `session_restore` | Pause/resume individual swarms |
| `hive-mind_broadcast` | Broadcast within a swarm (if hive-mind is per-swarm) |

### Need Changes for Multi-Swarm

| Tool | Current Limitation | Needed Change |
|------|-------------------|---------------|
| `hive-mind_init` | Overwrites singleton state.json | Accept swarmId, write per-swarm state file |
| `hive-mind_status` | Returns single hive state | Accept swarmId parameter |
| `hive-mind_shutdown` | Shuts down the one hive | Accept swarmId, shut down only that swarm |
| `swarm_init` | Cosmetic, creates metadata only | Could be enhanced to register swarm in a registry |
| `coordination_orchestrate` | No swarm awareness | Add swarmId scoping |
