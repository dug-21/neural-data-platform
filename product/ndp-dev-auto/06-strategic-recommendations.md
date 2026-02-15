# 06: Strategic Gaps and Forward-Looking Recommendations

> Research Agent: agent-6 (strategic analysis)
> Date: 2026-02-15
> Scope: Observability, cost, knowledge evolution, self-healing, rollback, security, docs, dependencies, DX, scaling
> Files reviewed: 35+ protocol/agent/skill/config/feature/research files
> Context: Reports 01 (protocol evaluation), 02 (concurrent swarms), 03 (local CI/CD)

---

## Executive Summary: Top 5 Strategic Priorities

1. **Agent Decision Observability** -- There is no audit trail for WHY an agent made a choice. When code breaks, the human cannot trace the decision chain backward. This is the single largest governance gap for a system training future agents.

2. **Knowledge Graph Decay Prevention** -- AgentDB has ~58 patterns and growing, but no mechanism to detect conflicts between patterns, prune stale entries at scale, or version superseded patterns atomically. Pattern ID 29 was manually deprecated in favor of ID 57; this process does not scale.

3. **Cross-Feature Dependency Tracking** -- fe-004 depends on fe-003. This is recorded as a prose line in SCOPE.md ("Predecessor: fe-003"). There is no machine-readable dependency graph, no validation that prerequisites are met before implementation starts, and no impact analysis when a predecessor changes.

4. **Rollback Speed and Safety** -- 13 reverted commits in recent history. BUG-004 required 5 attempt-and-revert cycles (jemalloc, mimalloc, Polars removal, glibc tuning) before finding the root cause. There is no automated rollback procedure, no "known good" marker, and no mechanism to isolate agent changes for safe revert.

5. **Human Friction Reduction** -- The human must manually invoke `/get-pattern`, `/reflexion`, `/save-pattern`, review alignment reports, approve variances, manage MEMORY.md, update AgentDB deprecations, and orchestrate release artifacts. The ratio of ceremony to productive work is high and increasing as the system matures.

---

## Detailed Analysis

### 1. Agent Decision Observability

**Current state**: Agents write code and produce summaries. The summaries say WHAT was done but not WHY. When an agent chooses approach A over approach B, that reasoning lives only in the agent's context window and is lost when the session ends.

**Evidence of the gap**:
- BUG-004 debugging required 5 attempts across v1.1.19 through v1.1.25. Each attempt was a different agent session. There is no record of why jemalloc was chosen first, why mimalloc replaced it, or why both were reverted. The human had to reconstruct the decision chain from commit messages and CHANGELOG entries.
- The reflexion system captures whether patterns helped (reward score 0.0-1.0) but not the agent's reasoning about alternatives considered and rejected.
- Settings.json has 11 hooks that fire automatically (pre-edit, post-edit, pre-command, post-command, pre-task, post-task, etc.) but none of them log the agent's decision rationale.

**What is missing**:

| Gap | Impact |
|-----|--------|
| Decision log per agent session | Cannot replay WHY code was written this way |
| Alternative-rejected log | Cannot learn from paths not taken |
| Pattern selection rationale | Know which patterns were consulted, but not why one was chosen over another |
| Inter-agent coordination trace | When scrum-master corrects drift, no record of what the correction was and why |
| Cost attribution per decision | Cannot measure which decisions were expensive (token-wise) vs cheap |

**Recommendation**: Introduce a lightweight decision journal. Each agent session appends entries to a structured log (stored in claude-flow memory with a `decisions` namespace):

```
decision_log_entry:
  agent: ndp-rust-dev
  swarm: swarm-fe004-impl
  timestamp: 2026-02-15T10:30:00Z
  decision: "Used pgvector SQL fallback instead of HNSW wrapper"
  alternatives_considered:
    - "HNSW via ruvector-core -- rejected because ruvector-graph did not compile in Phase 0"
  patterns_consulted: [ID 30, ID 42]
  pattern_chosen: ID 42
  confidence: 0.8
  file_paths: ["crates/ndp-intelligence/src/similarity/pgvector.rs"]
```

This log feeds into reflexion (provides the reasoning behind the reward score) and into future pattern searches (next agent facing a similar decision finds the rationale, not just the outcome).

**Effort**: Medium. Protocol change + agent prompt addition. No MCP server changes.

---

### 2. Cost Optimization and Token Usage

**Current state**: No visibility into token consumption. The settings.json configures model routing (Opus default, Haiku for routing) but there is no measurement, budget, or optimization feedback loop.

**Evidence of the gap**:
- The 3-tier model routing table exists (swarm-protocol.md: Agent Booster < Haiku < Sonnet/Opus) but the `model-route` hook fires automatically with no reporting on what it selected or why.
- A typical planning swarm reads the vision corpus (~730 lines) + SPARC artifacts (~5000 lines) + AgentDB patterns + coordination memory. A typical implementation swarm reads the brief (~300 lines) + source code + runs cargo. There is no measurement of which phase consumes more tokens.
- Swarm-protocol.md specifies 15 max agents. At Opus pricing, a 15-agent swarm with 2 corrective iterations could consume substantial resources with no cap or warning.
- The continuous improvement plan (v3) identifies context window exhaustion as a pressure point but addresses it by truncating cargo output and capping iterations -- not by measuring and budgeting.

**What is missing**:

| Gap | Impact |
|-----|--------|
| Token usage per swarm | Cannot compare cost of planning vs implementation |
| Token usage per agent | Cannot identify which agent type is most expensive |
| Token budget per feature | No spending cap -- a complex feature burns unlimited tokens |
| Model routing effectiveness | No data on whether Haiku routing saves money vs Opus for simple tasks |
| Context window utilization | No measurement of how full the coordinator's context gets |
| Wasted context from stale reads | Agents read files that turn out to be irrelevant -- no tracking |

**Recommendation**: Add a cost estimation layer as a post-task hook that logs estimated tokens (input + output) per agent. Store in claude-flow memory under `cost/{swarm-id}/{agent-id}`. The coordinator summarizes total estimated cost in its return to the primary agent. Over time, this data reveals:
- Which agent types need Opus vs which work fine with Haiku
- Whether the implementation brief actually reduces cost vs reading full specs
- Where context is wasted (files read but not used)

**Quick win**: Add a line to the ndp-scrum-master return template: "Estimated agent count: N, waves: M, model tier: Opus/Haiku". Even without actual token counts, this creates cost awareness.

---

### 3. Knowledge Graph Evolution

**Current state**: AgentDB holds ~58 patterns. Patterns are versioned by convention (`-v2` suffix) and deprecated by manual MEMORY.md annotation. The reflexion system provides per-pattern feedback (reward 0.0-1.0). The `learner` skill auto-discovers patterns from episodes. Pattern maintenance cadence is defined (weekly prune, biweekly review) but not enforced.

**Evidence of the gap**:
- Pattern ID 29 (feature-dir-structure) was manually deprecated in favor of ID 57 (feature-dir-structure-v2). This required a human to notice the conflict, update MEMORY.md, and remember to note it. Future agents searching for "feature directory structure" will find both patterns and must read MEMORY.md to know which is current.
- Pattern ID 32 (sparc-swarm-workflow) was deprecated in favor of ID 58 (sparc-swarm-workflow-v2). Same manual process.
- No mechanism detects when two patterns give contradictory advice. Report 01 identified 6 contradictions in protocol files -- the same class of problem likely exists in AgentDB patterns.
- The `save-pattern` skill says to append `-v2` for updated patterns, but there is no link between the old and new pattern (no "supersedes" field).
- Pruning relies on reflexion reward scores, but reflexion is recorded only by the primary agent after the swarm completes (Report 01, finding 5.4). Sub-agent pattern usage is lost, meaning many patterns never receive feedback.

**What is missing**:

| Gap | Impact |
|-----|--------|
| Supersession links | Old pattern has no pointer to replacement; agents may follow outdated advice |
| Conflict detection | Two patterns could give contradictory guidance with no warning |
| Automated deprecation | Human must manually mark patterns as deprecated in MEMORY.md |
| Pattern coverage map | No visibility into which areas of the codebase have patterns vs which are undocumented |
| Reflexion completeness | Sub-agents never record reflexion; most pattern usage goes unscored |
| Pattern deduplication | `save-pattern` checks for duplicates before storing but relies on semantic similarity; near-duplicates with different wording accumulate |

**Recommendation -- 3-part approach**:

**A. Supersession metadata**: When storing a new pattern via `save-pattern`, require an optional `supersedes` field. If provided, the old pattern gets flagged with `superseded_by: <new_id>`. `get-pattern` search results show superseded patterns with a warning.

**B. Conflict scanner**: Periodic job (weekly, via `/learner` or a new skill) that searches for patterns in the same `taskType` category with divergent `approach` text. Flag potential conflicts for human review. Implementation: for each taskType, compute pairwise similarity of approach text. Pairs with high taskType overlap but low approach similarity are conflict candidates.

**C. Per-agent reflexion**: Modify agent prompts to include a reflexion step before returning results. Each sub-agent records its own pattern usage and effectiveness. The primary agent's reflexion becomes a summary, not the sole source of feedback. This directly addresses the signal gap identified in Report 01.

---

### 4. Self-Healing Workflows

**Current state**: When a swarm fails, the protocol specifies a 2-iteration drift correction budget. If that is exhausted, the protocol says "return to primary agent" with a failure summary. There is no automatic recovery, no checkpoint, and no mechanism to resume a partially-completed swarm.

**Evidence of the gap**:
- Report 01 scores error recovery 2/5 -- the weakest area.
- The swarm-protocol.md `Spawn and Wait` pattern has no timeout: "WAIT -- let agents complete." A hung agent blocks forever.
- If the coordinator's context window is exhausted mid-swarm, all state is lost. There is no checkpoint mechanism.
- The `session_save` and `session_restore` MCP tools exist but are not wired into swarm protocols. They could provide checkpointing.

**What is missing**:

| Gap | Current Workaround | Automated Solution |
|-----|--------------------|--------------------|
| Agent timeout | Human notices session seems stuck | Coordinator sets deadline per wave; after timeout, proceeds with available results |
| Context exhaustion | Session crashes; human re-starts | Coordinator checkpoints state to memory after each wave; new coordinator can resume from checkpoint |
| Partial success | All-or-nothing per wave | Coordinator commits successful agent results immediately; failed agents get targeted re-spawn |
| MCP unavailability | Swarm silently degrades | Fallback to file-based coordination (write JSON to `product/features/{id}/swarm-state.json`) |
| Build environment corruption | Human runs `cargo clean` | Pre-spawn baseline check: `cargo build --workspace` must pass before agents are spawned |

**Recommendation**: Implement swarm checkpointing using `session_save`:

After each wave completes, the coordinator writes a checkpoint:
```
memory_store(
  key: "checkpoint-wave-N",
  namespace: "{swarm-id}",
  value: {
    wave: N,
    completed_tasks: [...],
    failed_tasks: [...],
    files_modified: [...],
    test_status: "pass/fail",
    next_wave: N+1
  }
)
```

If the coordinator crashes or context is exhausted, a new coordinator reads the checkpoint and resumes from wave N+1. This transforms the current all-or-nothing model into an incremental progress model.

**Effort**: Medium-high. Protocol changes + coordinator prompt rewrite + checkpoint schema design.

---

### 5. Rollback Strategies

**Current state**: Git is the only rollback mechanism. The release policy requires annotated tags and manifests, but there is no rapid rollback procedure. Revert is manual (`git revert <sha>`).

**Evidence of the gap**:
- 13 reverted commits in recent history (since Dec 2024).
- BUG-004 fix sequence: v1.1.19 (jemalloc), revert, v1.1.19 (mimalloc), revert + revert, v1.1.21 (Polars removal), v1.1.22 (instrumentation), v1.1.23 (glibc tuning), v1.1.24 (watchdog), v1.1.25 (WAL-only). Five attempts over four days with three revert commits. Each revert was manual.
- No "last known good" marker in the codebase. The human tracks this in MEMORY.md ("v1.1.24 deployed").
- The deploy.sh manifest system (`vX.Y.Z.manifest.json`) defines what should be deployed but has no rollback-to-previous capability.
- Agent-produced code is committed to the main branch directly. There is no branch-per-feature isolation for agent work (Report 02 recommends this for concurrent swarms but it is not implemented).

**What is missing**:

| Gap | Impact |
|-----|--------|
| Automated `git revert` procedure | Human must manually identify the commit(s) to revert |
| Branch-per-feature for agent work | Agent code lands on main immediately; revert scope is unclear |
| "Last known good" tag | Deploy.sh does not know which version to rollback to |
| Deploy.sh rollback command | `deploy.sh rollback` does not exist; must manually `deploy.sh build` with old tag |
| Test-before-merge gate | No PR workflow; agent code merges without automated validation |
| Staged rollback (per-layer) | Cannot rollback Bronze changes without affecting Silver changes in same commit |

**Recommendation -- 3-tier rollback**:

**Tier 1 -- Quick revert (seconds)**: `deploy.sh rollback` reads the previous manifest from `.deploy/releases/` and redeploys the previous Docker image. Requires: (a) keeping the previous Docker image tagged, (b) a `previous_version` field in the current manifest.

**Tier 2 -- Git branch isolation (minutes)**: Agent swarms work on feature branches (`feature/{phase}-{NNN}`). Code merges to main only after `/validate` passes. This is the standard approach recommended by Report 02 for concurrent swarms. It also provides a clean revert boundary: delete the branch, main is untouched.

**Tier 3 -- Selective revert (minutes)**: For commits already on main, a `/rollback` skill that: (a) identifies commits by feature ID (`git log --grep='{feature-id}'`), (b) generates a revert commit for each, (c) runs `/validate` on the reverted state, (d) creates a GH Issue documenting the rollback.

---

### 6. Security Review Automation

**Current state**: Agent routing table references `security-architect` and `auditor` agents (line 24 of agent-routing.md) but neither has an NDP-specific definition. They fall back to generic agents. The settings.json configures `"security": {"autoScan": true, "scanOnEdit": true, "cveCheck": true, "threatModel": true}` but these are claude-flow features, not NDP-specific security gates.

**Evidence of the gap**:
- No `ndp-security-auditor` agent definition exists despite being referenced in routing.
- The `scripts/validation/pre-commit-hook.sh` checks for hardcoded secrets (`password`, `secret`, `api_key`) but is not installed.
- No `cargo audit` in the validation pipeline (Report 03 does not include it in any proposed layer).
- The alignment criteria (ALIGNMENT-CRITERIA.md) include "Privacy by Architecture" as principle 6, but there is no automated check that new code does not introduce telemetry, phone-home, or cloud dependencies.
- Docker images are built from source on Pi. No image scanning (Trivy, Grype) is performed.

**Should a security agent be in every swarm?**

No -- the overhead would be disproportionate for most tasks. Instead:

| Trigger | Security Action |
|---------|----------------|
| New dependency added to Cargo.toml | `cargo audit` check |
| Network-facing code modified (MQTT, HTTP, MCP) | Security review by dedicated agent |
| Config parsing code modified | Input validation review |
| Docker/deploy changes | Image scanning, secret leak check |
| New external data source added | Privacy review (does data leave device?) |

**Recommendation**: Create `ndp-security-auditor` agent definition with narrow scope (dependency audit, secret scanning, privacy verification). Add `cargo audit` to `/validate` Tier 2. Add a trigger in implementation-protocol.md: if `Cargo.toml` or network-facing files are modified, spawn security auditor as an additional agent in the wave.

**Effort**: Medium. Agent definition + `/validate` enhancement + protocol trigger.

---

### 7. Documentation Generation

**Current state**: Documentation is manually maintained. CHANGELOG.md is manually written per release (detailed, high quality). Architecture docs in `docs/` are updated ad-hoc. API documentation relies on Rust doc comments and `cargo doc`. No auto-generated architecture diagrams exist.

**Evidence of the gap**:
- CHANGELOG.md entries are thorough (v1.1.25 entry is 52 lines) but must be manually written during release. This is both a strength (human-quality prose) and a burden.
- 15 MB of feature documentation in `product/features/` across 51 features. No index or search beyond AgentDB patterns.
- Agent definitions (16 agents) have stale technology status references (Report 01: ndp-architect.md says "Silver Layer (Planned)" when Silver is implemented).
- No architecture diagrams are generated from code. The data flow (Bronze -> Silver -> Gold) is described in prose in multiple locations.
- The `document` daemon worker is configured in settings.json (runs hourly) but its output and effectiveness are unknown.

**What could be automated**:

| Document Type | Current | Automated Approach |
|---------------|---------|-------------------|
| CHANGELOG | Manual, per release | Generate from conventional commits (`git cliff` or `standard-version`). Human reviews and edits. |
| API docs | `cargo doc` (manual run) | Add `just docs` recipe. Generate on release. |
| Architecture diagrams | None | Generate module dependency graph from `Cargo.toml` workspace members. Mermaid diagram. |
| Feature index | None | Generate from `product/features/*/SCOPE.md` frontmatter -- table of features with status, version, dependencies. |
| Agent capability matrix | README.md (manual) | Generate from agent definition frontmatter -- table of agents with scope, capabilities, pattern domains. |
| Stale reference detection | Human catches | Script that checks for references to deprecated patterns, old version numbers, removed files. |

**Recommendation**: Prioritize feature index and stale reference detection. Both are high-value, low-effort, and address immediate pain (51 features with no index, 6+ stale references identified in Report 01).

**Quick win**: A `just docs` recipe that runs `cargo doc --workspace --no-deps` and generates a feature index from SCOPE.md files.

---

### 8. Cross-Feature Dependency Tracking

**Current state**: Feature dependencies are recorded in prose within SCOPE.md files. fe-004's SCOPE.md says "Predecessor: fe-003 (Phase 0+1, GH Issue #17)". fe-004 also says "Parent feature: gold-002" and "Parent roadmap: product/features/gold-002/IMPLEMENTATION-ROADMAP.md". These are human-readable but not machine-processable.

**Evidence of the gap**:
- Only 2 of 51 features have IMPLEMENTATION-BRIEF.md files (fe-003 and fe-004). The brief is a new artifact (introduced in continuous improvement plan v3). Earlier features have no brief.
- The fe-004 SCOPE.md lists 8 specific code artifacts from fe-003 as prerequisites. If fe-003 had changed any of these (e.g., trait signature), fe-004's implementation would break with no warning.
- There is no validation that fe-003 is actually complete before fe-004 implementation begins. The "predecessor" line is informational only.
- The vision roadmap (ALIGNMENT-CRITERIA.md) defines version targets (v1.0 complete, v1.1 in progress, v1.2 planned) but there is no machine-readable mapping from features to versions.
- GitHub Issues track individual features but have no dependency links (no "blocked by" or "depends on" fields).

**What is missing**:

| Gap | Impact |
|-----|--------|
| Machine-readable dependency graph | Cannot auto-validate prerequisites |
| Prerequisite completion check | Implementation swarm starts even if predecessor is incomplete |
| Impact analysis on change | Changing fe-003 code does not flag fe-004 as potentially affected |
| Feature-to-version mapping | Cannot auto-generate "what ships in v1.2" list |
| Dependency visualization | Human must read 51 SCOPE.md files to understand the dependency tree |

**Recommendation**: Introduce a `features.json` manifest at `product/features/features.json`:

```json
{
  "features": {
    "fe-003": {
      "version": "v1.2.0",
      "status": "complete",
      "depends_on": [],
      "github_issue": 17,
      "artifacts": ["crates/ndp-intelligence/", "crates/ndp-lib/src/gold/embeddings/"]
    },
    "fe-004": {
      "version": "v1.2.0",
      "status": "in-progress",
      "depends_on": ["fe-003"],
      "github_issue": 18,
      "artifacts": ["crates/ndp-intelligence/src/similarity/", "apps/ndp-intelligence-app/"]
    }
  }
}
```

The ndp-scrum-master reads this manifest before spawning an implementation swarm. If `depends_on` features are not `status: "complete"`, it warns the primary agent. A `just feature-graph` recipe generates a Mermaid diagram from this manifest.

**Effort**: Low-medium. JSON file + validation in scrum-master prompt + optional visualization.

---

### 9. Developer Experience (Human Friction)

**Current state**: The human is the bottleneck in multiple loops: pattern workflow, vision alignment approval, release artifact creation, swarm orchestration, and knowledge maintenance.

**Evidence from protocol analysis**:

| Manual Step | Frequency | Time Estimate |
|-------------|-----------|---------------|
| `/get-pattern` invocation | Every task start | 1-2 minutes |
| `/reflexion` per pattern used | Every task end | 3-5 minutes (multiple entries) |
| `/save-pattern` for new discoveries | After discoveries | 2-3 minutes |
| Review ALIGNMENT-REPORT.md | Every planning swarm | 5-10 minutes |
| Approve variances | When alignment flags issues | 2-5 minutes |
| Update MEMORY.md | After every session | 3-5 minutes |
| Create release artifacts (manifest, tag, changelog) | Per release | 15-30 minutes |
| Deprecate stale patterns in MEMORY.md | Biweekly | 10-15 minutes |
| Manage GH Issues (create, update, close) | Per feature/bug | 5-10 minutes |

**Total ceremony per feature session**: 30-60 minutes of overhead that does not write code.

**The pattern workflow is the heaviest overhead**. Before work: search patterns. After work: record reflexion per pattern used, save new patterns. The reflexion format is specific (must reference pattern ID and name, provide per-pattern reward score). A 3-pattern session requires 3 separate reflexion entries. This is valuable for knowledge quality but creates significant friction.

**What could be reduced**:

| Current Friction | Reduction |
|------------------|-----------|
| Manual `/get-pattern` invocation | Auto-invoke on swarm start (already partially done via protocol, but human must type the command) |
| Per-pattern reflexion entries | Batch reflexion: agent generates all entries in one skill invocation |
| MEMORY.md manual updates | Auto-generate from AgentDB stats + GH Issue state + git tags |
| Release artifact creation | `/release` skill that generates manifest, changelog entry, and tag from git history |
| Pattern deprecation | Auto-flag patterns with <0.3 average reflexion reward in last 10 uses |

**Recommendation -- prioritize three reductions**:

1. **Batch reflexion**: Modify the reflexion skill to accept an array of pattern feedback entries in one invocation. Current: 3 patterns = 3 skill calls. Proposed: 1 skill call with 3 entries.

2. **Auto-MEMORY.md**: A `/memory-sync` skill that reads AgentDB stats, GH Issue state, git tags, and test counts, then updates MEMORY.md sections automatically. Human reviews and approves.

3. **Release automation**: A `/release` skill that: (a) generates CHANGELOG entry from conventional commits since last tag, (b) creates manifest from workspace Cargo.toml versions, (c) creates annotated tag, (d) validates all 3 artifacts exist. Human triggers and reviews but does not write.

---

### 10. Scaling to Larger Teams

**Current state**: Single human, single Claude Code instance, single swarm at a time. Report 02 addresses technical concurrent swarm architecture but does not address the organizational scaling question: what happens with 2+ humans and 2+ independent Claude Code sessions?

**Scaling dimensions**:

| Dimension | Current | Next Scale | Challenges |
|-----------|---------|------------|------------|
| Humans | 1 | 2-3 | Who approves which swarm? Conflicting MEMORY.md updates. |
| Concurrent sessions | 1 | 2-3 | Git branch conflicts. Pattern store conflicts. GH Issue assignment. |
| Features in flight | 1-2 | 3-5 | Dependency management. Shared crate contention. |
| Agent count per swarm | 3-8 | 8-15 | Context window pressure. Coordination overhead. |
| Total agent count | 3-8 | 15-30 | API rate limits. Token budget. |

**What is missing for multi-human**:

| Gap | Impact |
|-----|--------|
| Session ownership | Two humans cannot distinguish their swarm results in shared memory |
| MEMORY.md merge conflicts | MEMORY.md is a single file updated by every session; concurrent edits conflict |
| Pattern store write conflicts | Two agents storing patterns simultaneously could create near-duplicates |
| GH Issue assignment | No convention for which human owns which issue |
| Branch protection | No rules preventing one session from pushing to another session's branch |

**This is not an immediate concern** -- NDP is currently a single-human project. But the architecture decisions made now (especially around state management and coordination) will either enable or prevent multi-human scaling later.

**Recommendation**: Design for single-human-multiple-sessions first (the concurrent swarms work from Report 02). Multi-human can wait until the project reaches that scale. The key enabler is the swarm-ID namespace isolation proposed in Report 02 -- it naturally extends to session-ID isolation for multiple humans.

---

## Opportunity Matrix

| Area | Impact | Effort | Priority |
|------|--------|--------|----------|
| Cross-feature dependency tracking | High -- prevents broken prerequisites | Low -- JSON manifest + validation | P0 |
| Rollback: branch-per-feature | High -- clean revert boundary | Low -- protocol change | P0 |
| Agent decision observability | High -- governance, learning | Medium -- protocol + prompts | P1 |
| Knowledge graph conflict detection | High -- prevents contradictory advice | Medium -- periodic scan + schema | P1 |
| Batch reflexion | Medium -- reduces 3x overhead to 1x | Low -- skill modification | P0 |
| `cargo audit` in /validate | Medium -- catches vulnerable deps | Low -- one-line addition | P0 |
| Security agent triggers | Medium -- targeted security review | Medium -- agent + protocol | P1 |
| Feature index generation | Medium -- 51 features navigable | Low -- script + Justfile recipe | P0 |
| CHANGELOG from commits | Medium -- eliminates manual writing | Medium -- tool setup | P1 |
| Self-healing checkpoints | High -- swarm resilience | High -- protocol rewrite | P2 |
| Release automation skill | Medium -- reduces release friction | Medium -- skill creation | P1 |
| Auto-MEMORY.md sync | Low-Medium -- reduces manual upkeep | Medium -- skill creation | P2 |
| Per-agent reflexion | Medium -- improves pattern feedback | Medium -- prompt changes | P2 |
| Stale reference detection | Low -- catches outdated docs | Low -- grep script | P0 |
| Cost estimation layer | Low -- visibility only | Medium -- hook + storage | P2 |
| Multi-human scaling | Low (not needed yet) | High -- architecture rethink | P3 |

---

## Recommended Roadmap

### This Week (Quick Wins)

| Item | Effort | Description |
|------|--------|-------------|
| QW-1: `features.json` manifest | 1h | Create machine-readable feature dependency graph. Start with fe-003, fe-004, ops-005. |
| QW-2: `cargo audit` in /validate | 15m | Add `cargo audit` to Tier 2 of validate skill. Install `cargo-audit` in devcontainer. |
| QW-3: Feature index generator | 1h | Script that reads `product/features/*/SCOPE.md` and generates a table: feature ID, version target, status, dependencies, GH Issue. Add as `just feature-index` recipe. |
| QW-4: Stale reference scanner | 30m | Script that checks agent definitions and protocol files for references to deprecated patterns (IDs 29, 32), removed files, old version numbers. Add as `just stale-check` recipe. |
| QW-5: Branch-per-feature convention | 30m | Document in implementation-protocol.md: implementation swarms create `feature/{phase}-{NNN}` branches. Merge to main only after `/validate` passes. |
| QW-6: Batch reflexion format | 30m | Update reflexion skill SKILL.md to show batch format: one invocation with array of `{pattern_id, reward, critique}` entries. |

**Total quick win effort**: ~4 hours.

### 3-Month Horizon

| Item | Effort | Description |
|------|--------|-------------|
| M1: Agent decision journal | 3d | Design decision log schema. Add to agent prompts. Store in claude-flow memory. Make searchable via `memory_search`. |
| M2: Pattern conflict scanner | 2d | Weekly job via `/learner` extension. Detect patterns in same taskType with divergent approaches. Flag for human review. |
| M3: Pattern supersession metadata | 1d | Add `supersedes` field to `save-pattern`. Update `get-pattern` to warn on superseded results. |
| M4: ndp-security-auditor agent | 2d | Agent definition + routing + conditional spawn triggers in implementation protocol. |
| M5: Release automation skill | 3d | `/release` skill: CHANGELOG generation from conventional commits, manifest creation, tag creation. |
| M6: CHANGELOG from commits | 2d | Set up `git-cliff` or equivalent. Configure to generate Keep-a-Changelog format from conventional commits. Human edits before release. |
| M7: deploy.sh rollback command | 2d | Implement `deploy.sh rollback` that redeploys the previous manifest's Docker image. |

### 6-Month Horizon

| Item | Effort | Description |
|------|--------|-------------|
| H1: Swarm checkpointing | 1w | Checkpoint after each wave. Resume from checkpoint on failure. Enables incremental progress. |
| H2: Per-agent reflexion | 3d | Sub-agents record their own pattern usage. Primary agent summarizes. Better feedback signal. |
| H3: Cost estimation layer | 3d | Post-task hook estimates tokens. Stores per-swarm cost. Weekly summary for human. |
| H4: Auto-MEMORY.md sync | 3d | Skill reads AgentDB, GH Issues, git tags, test counts. Generates MEMORY.md sections. Human approves. |
| H5: Feature dependency validation | 2d | ndp-scrum-master reads `features.json` before spawning implementation swarm. Warns if prerequisites incomplete. |
| H6: Architecture diagram generation | 2d | Generate Mermaid diagram from Cargo.toml workspace members showing crate dependency graph. Update on release. |

### 12-Month Horizon

| Item | Effort | Description |
|------|--------|-------------|
| Y1: Multi-session coordination | 2w | Extend swarm-ID isolation (Report 02) to support multiple concurrent Claude Code sessions. Session registry. Conflict detection. |
| Y2: Autonomous pattern lifecycle | 1w | Patterns auto-deprecate when average reflexion reward drops below threshold. Auto-prune old superseded patterns. Human reviews quarterly. |
| Y3: Predictive cost budgeting | 1w | Based on historical token data, estimate cost before swarm launch. Budget alerts. Model tier auto-downgrade when budget is tight. |
| Y4: Self-healing swarm orchestrator | 2w | Coordinator automatically recovers from agent failures, context exhaustion, MCP outages. Full checkpoint/resume. |

---

## Synthesis with Other Research Reports

### Connections to Report 01 (Protocol/Agent Evaluation)

Report 01 scored error recovery 2/5 and identified 6 contradictions. This report's self-healing (area 4) and rollback (area 5) recommendations directly address the error recovery gap. The decision journal (area 1) provides the audit trail that Report 01 noted was missing for understanding why drift corrections were made.

Report 01 identified 37 unreferenced skills as noise. This report's stale reference scanner (QW-4) would detect these automatically.

### Connections to Report 02 (Concurrent Swarms)

Report 02's swarm-ID namespace isolation is a prerequisite for the decision journal (area 1) -- decisions must be scoped to a swarm. The `features.json` manifest (area 8) enables Report 02's inter-swarm dependency signaling with concrete data rather than ad-hoc memory messages.

Report 02's branch-per-swarm recommendation aligns with this report's branch-per-feature recommendation (area 5, QW-5). They are the same mechanism serving two purposes: concurrent isolation and clean rollback.

### Connections to Report 03 (Local CI/CD)

Report 03 proposes a testing pyramid with `just` as the task runner. This report's quick wins (QW-3: feature-index, QW-4: stale-check) are natural additions to that Justfile. The `cargo audit` recommendation (QW-2) fills a gap Report 03 did not address.

Report 03's proposed GitHub Actions CI (MT-4) would enforce this report's branch-per-feature convention (QW-5) via branch protection rules.

### Net New Areas Not Covered by Reports 01-03

| Area | Why It Matters |
|------|---------------|
| Agent decision observability | No other report addresses the WHY behind agent choices |
| Knowledge graph evolution | Reports focus on knowledge USE, not knowledge MAINTENANCE |
| Cross-feature dependencies | Reports address intra-swarm coordination, not inter-feature coordination |
| Security automation | Reports address quality (testing) but not security |
| Documentation generation | Reports address validation but not documentation |
| Human friction | Reports optimize agent workflows but not the human's workflow |

---

## Files Referenced in This Research

| File | Path | Relevance |
|------|------|-----------|
| CLAUDE.md | `/workspaces/neural-data-platform/CLAUDE.md` | Project rules, feature conventions |
| Alignment Criteria | `/workspaces/neural-data-platform/product/vision/ALIGNMENT-CRITERIA.md` | Vision principles, security/privacy requirements |
| Pattern Workflow | `/workspaces/neural-data-platform/.claude/rules/pattern-workflow.md` | Learning loop, reflexion format |
| Agent Routing | `/workspaces/neural-data-platform/.claude/rules/agent-routing.md` | Missing security agent, routing gaps |
| Swarm Protocol | `/workspaces/neural-data-platform/.claude/rules/swarm-protocol.md` | Error recovery gaps, spawn-and-wait |
| Improvement Plan | `/workspaces/neural-data-platform/product/continuous-improvement-plan.md` | Existing workstreams, context protection |
| Get Pattern Skill | `/workspaces/neural-data-platform/.claude/skills/get-pattern/SKILL.md` | Pattern retrieval workflow |
| Save Pattern Skill | `/workspaces/neural-data-platform/.claude/skills/save-pattern/SKILL.md` | Pattern storage, versioning convention |
| Agent Roster | `/workspaces/neural-data-platform/.claude/agents/ndp/README.md` | 16 agents, missing specializations |
| Settings | `/workspaces/neural-data-platform/.claude/settings.json` | 11 hooks, model routing, security config |
| fe-004 Scope | `/workspaces/neural-data-platform/product/features/fe-004/SCOPE.md` | Cross-feature dependency example |
| ops-005 Scope | `/workspaces/neural-data-platform/product/features/ops-005/SCOPE.md` | Edge cases, performance testing gap |
| CHANGELOG | `/workspaces/neural-data-platform/CHANGELOG.md` | Release documentation pattern |
| Report 01 | `/workspaces/neural-data-platform/product/ndp-dev-auto/01-protocol-agent-evaluation.md` | Protocol gaps, contradictions |
| Report 02 | `/workspaces/neural-data-platform/product/ndp-dev-auto/02-concurrent-swarms.md` | Namespace isolation, claims system |
| Report 03 | `/workspaces/neural-data-platform/product/ndp-dev-auto/03-local-cicd-e2e.md` | Testing pyramid, Justfile, CI/CD gaps |
