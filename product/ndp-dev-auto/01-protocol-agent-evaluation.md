# Protocol and Agent Evaluation Report

> Evaluator: research agent-1
> Date: 2026-02-15
> Scope: Swarm protocols, agent definitions, skill integration, knowledge flow
> Files reviewed: 15 protocol/agent/skill/config files

---

## Executive Summary

- **The protocol stack is remarkably well-structured** with clear separation between planning and implementation swarms, a dual memory system (AgentDB permanent / claude-flow transient), and a mandatory learning loop. The architecture is one of the most sophisticated agent coordination systems in a real production codebase.
- **The coordinator bottleneck is real but manageable.** ndp-scrum-master is the single point of coordination for every swarm, which creates a single-threaded gate. For the current project scale (3-8 agent swarms) this works. At 10-15 agents it will strain context windows.
- **Error recovery is the weakest area.** There is a 2-iteration drift budget and an instruction to "return to primary agent" on persistent failure, but no protocol for partial rollback, agent timeout handling, or context window exhaustion mid-swarm.
- **Contradictions exist between the prompt-check hook and the protocols.** The hook tells agents to use `claude-flow swarm init` CLI, while all three protocol files explicitly say this command is cosmetic and should NOT be used. The ndp-scrum-master.md also references the CLI form at line 34.
- **Skills sprawl is significant.** There are 44 SKILL.md files in `.claude/skills/`, but only 7 are referenced by the NDP agent system. The remaining 37 are unreferenced noise that increases cognitive load for agents that encounter them.

---

## Detailed Findings

### 1. Protocol Completeness

**Score: 4/5**

The three-file protocol stack (`swarm-protocol.md`, `planning-protocol.md`, `implementation-protocol.md`) covers the full lifecycle well:

**Strengths:**
- Clear 4-phase flow for both planning and implementation
- Explicit message sequencing (what goes in which message)
- Pre-spawn checklists prevent premature agent launches
- Concurrency rules (batch ALL of same type in one message) are consistent across all three files
- Validation has three explicit tiers with path-based routing
- Multi-wave features are addressed with explicit sequencing rules

**Gaps identified:**

| Gap | Severity | Details |
|-----|----------|---------|
| No agent timeout protocol | P0 | If a Task agent hangs or exhausts its context window, there is no guidance on detection or recovery. The "spawn and wait" pattern has no timeout bound. |
| No partial failure recovery | P1 | If 3 of 5 agents succeed and 2 fail, there is no protocol for using the 3 successful results while remediating the 2 failures. The 2-iteration drift budget applies per-wave but does not address selective re-spawn. |
| No context window budget | P1 | The "Agent Context Budget" section says what to include in prompts but never quantifies the actual token limit. Agents producing large outputs (e.g., ARCHITECTURE.md with 10 ADRs) may cause the coordinator to exhaust its own context window during synthesis. |
| No rollback procedure | P2 | If an implementation swarm produces code that breaks the build and the 2-iteration fix cap is reached, there is no guidance on reverting changes. The protocol says "return to primary agent" but not what the primary agent should do. |
| No cross-swarm dependencies | P2 | The protocols assume each swarm is independent. There is no mechanism for an implementation swarm to discover that another concurrent swarm has modified shared files. |

### 2. Agent Roster

**Score: 4/5**

The roster of 16 NDP agents is comprehensive and well-organized into 5 tiers (Coordination, Core, Domain Scientists, Data Engineering, ML/Viz/Alerts).

**Strengths:**
- Clear scope classifications (narrow/specialized/broad) with documented boundaries
- Agent creation guide is excellent -- one of the best agent onboarding documents reviewed
- "Teach how to think, not what to implement" principle is well-enforced
- Each agent has a dedicated pattern domain in AgentDB

**Missing specializations:**

| Missing Agent | Justification |
|---------------|---------------|
| `ndp-deploy-engineer` | Deployment is complex (Pi-specific Docker, git-as-transport, deploy.sh). Currently delegated to generic `ndp-rust-dev` which lacks deploy domain knowledge. ops-005 SCOPE.md shows the complexity. |
| `ndp-security-auditor` | Agent routing table references `security-architect` and `auditor` (line 24 of agent-routing.md) but neither has an NDP-specific definition. They fall back to generic agents. |
| `ndp-config-engineer` | Config is the project's central abstraction (etcd, YAML hierarchy, stream registry). No agent specializes in configuration lifecycle. ops-005 shows 24 edge cases around config-driven behavior. |

**Redundancy risk:**
- `specification` and `pseudocode` agents referenced in planning spawns (line 148, planning-protocol.md) are NOT in the NDP agent roster. They appear to be generic Claude Code agent types, not NDP-customized. This means they lack NDP domain knowledge when producing SPARC artifacts.

### 3. Coordinator Pattern (ndp-scrum-master)

**Score: 3/5**

**Strengths:**
- Single coordinator prevents coordination split-brain
- Well-defined authority boundaries (spawn agents, steer via memory, detect drift)
- Clear return contract (files, tests, validation, issues, drift)
- GitHub Issue lifecycle management is thorough

**Weaknesses:**

1. **Context window pressure.** The scrum-master must: read the protocol file, read the brief/SCOPE, initialize MCP, create all tasks, seed memory, spawn all agents, receive all results, run drift checks, run validation, post GH Issue updates, and return a summary. For a 5-wave feature with 3 agents per wave, this coordinator processes 15 agent results plus all coordination overhead. The context window will be under severe pressure.

2. **Contradictory init instructions.** ndp-scrum-master.md line 34 says "Run `claude-flow swarm init`" but swarm-protocol.md line 47 says "Do NOT use `claude-flow swarm init` CLI -- they are cosmetic and create no real state." This is a direct contradiction within the same system.

3. **No delegation hierarchy.** For large swarms (10-15 agents), there is no concept of sub-coordinators. The scrum-master is the only coordinator, even though the Anti-Drift Config section suggests mesh topology for 10-15 agents. Mesh topology implies peer communication, but all coordination still flows through the single scrum-master.

4. **Planning and implementation overload.** The same ndp-scrum-master handles both planning swarms (specification, pseudocode, architecture, vision alignment, ADR storage, brief generation, GH Issue creation -- 7 sequential steps) and implementation swarms (agent spawning, drift check, validation, GH Issue update -- 4 steps per wave). These are fundamentally different coordination patterns crammed into one agent definition.

### 4. Error Recovery

**Score: 2/5**

This is the most significant gap in the protocol stack.

**What exists:**
- Drift detection after each wave (5 checks: out-of-scope files, stubs, missed criteria, test count decrease, no output)
- 2-iteration corrective budget per wave
- "Return to primary agent" as the fallback
- Max 2 validation fix iterations

**What is missing:**

| Gap | Impact |
|-----|--------|
| No agent timeout handling | A single hung agent blocks the entire wave. No guidance on how long to wait or how to detect a hung agent. |
| No partial success extraction | If 2 of 5 agents succeed, their work is lost when the coordinator returns "failed" to the primary. No guidance on committing partial results. |
| No context window exhaustion recovery | If the coordinator runs out of context, the entire swarm is lost. No checkpointing mechanism. |
| No file conflict resolution | Two agents editing the same file will produce conflicting changes. No merge protocol. |
| No cost/time budget | No mechanism to limit how many tokens a swarm can consume. A 15-agent swarm with 2 corrective iterations could consume enormous resources. |
| No graceful degradation | If the MCP layer (hive-mind, memory) is unavailable, agents have no fallback coordination mechanism. |

### 5. Knowledge Flow

**Score: 4/5**

The dual memory system is a standout design decision.

**Strengths:**
- AgentDB (permanent) vs claude-flow memory (transient) distinction is hammered home in 5 separate files. Hard to miss.
- The `/spec-compile` skill is an excellent bridge: it decomposes a full brief into searchable memory chunks and stores ADRs permanently.
- The Level-1 summary pattern is smart: every agent gets context on WHY, WHAT CONSTRAINS THEM, and HOW TO GET DETAILS. This is the primary anti-drift mechanism.
- The reflexion system with reward scoring (0.0-1.0 per pattern) creates a feedback loop that improves pattern quality over time.

**Weaknesses:**

1. **Planning-to-implementation handoff relies on a single artifact.** The IMPLEMENTATION-BRIEF.md bridges planning and implementation swarms. If the brief is poorly written (too vague, missing ADR pattern IDs, wrong file paths), the entire implementation swarm drifts. There is no quality gate on the brief itself.

2. **ADR storage timing issue.** Planning-protocol.md Step 3e stores ADRs in AgentDB AFTER architecture agents complete but BEFORE the implementation brief is generated. If ADR storage fails, the brief will reference pattern IDs that do not exist. There is no error handling for this sequence.

3. **Memory TTL not configured.** Claude-flow memory is described as "transient" but there is no explicit TTL set on memory_store calls. If a session crashes and resumes, stale coordination state may persist and mislead agents.

4. **Reflexion is primary-agent-only.** The protocol explicitly states that sub-agents do NOT run reflexion -- only the primary agent does after the swarm returns. This means the primary agent must reflexion for ALL patterns used by ALL agents, based on a summary. Nuanced per-agent pattern feedback is lost.

### 6. Redundancy and Contradictions

**Score: 3/5**

**Positive redundancy (reinforcement):**
- The "Two Memory Systems" table appears in 4 files. This is intentional and effective -- agents reading any protocol file will encounter it.
- The "batch ALL in ONE message" concurrency rules appear in 3 files. Also effective.
- Pattern Integration (REQUIRED) section appears in every agent definition. Good enforcement.

**Problematic redundancy/contradictions:**

| Location A | Location B | Contradiction |
|------------|------------|---------------|
| `swarm-protocol.md` line 47: "Do NOT use `claude-flow swarm init` CLI" | `ndp-scrum-master.md` line 34: "Run `claude-flow swarm init`" | Direct contradiction on the primary coordination command |
| `swarm-protocol.md` line 47: "Do NOT use CLI commands" | `prompt-check.sh` lines 36, 60: outputs "claude-flow swarm init --topology hierarchical" | Hook injects contradictory guidance |
| `planning-protocol.md` line 148: agents are `specification`, `pseudocode` | `agents/ndp/README.md`: no `specification` or `pseudocode` agent definitions | Undefined agent types referenced in spawning |
| `swarm-run/SKILL.md` line 16: "TeamCreate or parallel Tasks" | `swarm-protocol.md` line 15: "Do NOT use TeamCreate" | swarm-run still references TeamCreate as an option |
| `ndp-architect.md` ADR format uses `# ADR-NNN` with `## Status` | `planning-protocol.md` ADR format uses `## ADR-NNN` with `### Context` | Inconsistent ADR heading levels |
| `settings.json` line 199: `claude-opus-4-5-20251101` | `MEMORY.md`: "Default agent model: opus" | Model identifier may be stale |

**Stale references:**
- `ndp-architect.md` line 49: "Silver Layer (Planned)" -- Silver is implemented (908 tests, TimescaleDB active)
- `ndp-architect.md` line 50: "Gold Layer (Future)" -- Gold DDL generator exists (`ndp-gold-ddl`)
- `ndp-tester.md` lines 39-46: test directory structure references `tests/components/redis_streams/` and `tests/orchestrator/` which may be outdated

### 7. Scalability

**Score: 3/5**

**Works well at small scale (3-8 agents):**
- Hierarchical topology with single coordinator is clean
- Spawn-and-wait is simple and deterministic
- 2-iteration drift budget is reasonable

**Strain points at large scale (10-15 agents):**

| Issue | Threshold | Impact |
|-------|-----------|--------|
| Coordinator context window | ~8 agents per wave | Scrum-master accumulates all results. At 10+ agents returning 200+ lines each, synthesis exceeds capacity. |
| Sequential waves | >3 waves | Each wave adds a full read-spawn-wait-check-validate cycle. 5 waves with 3 agents each = 15 agents but 5 serial blocking rounds. |
| Memory namespace collision | >1 concurrent swarm | No namespace isolation scheme for concurrent swarms. If two features run simultaneously, `memory_store` keys may collide. |
| GH Issue comment volume | >5 waves | Each wave posts a comment. A complex feature produces 5-10 comments that clutter the issue. |
| Skill discovery | 44 skills | Agents using ToolSearch to find skills must sift through 44 SKILL.md files, many irrelevant. |

**No horizontal scaling path.** The coordinator-delegation model is inherently vertical -- one scrum-master per swarm. There is no design for: multiple concurrent coordinators, sub-coordinator delegation, or coordinator handoff when context is exhausted.

### 8. Onboarding (New Agent Type)

**Score: 5/5**

This is the strongest area of the system.

**What works:**
- `AGENT-CREATION-GUIDE.md` is comprehensive (377 lines) with:
  - Clear principle: "Agents know WHEN, Skills know HOW"
  - Stability boundary definition (what goes in agent vs patterns)
  - Required frontmatter schema
  - Required sections template
  - Pattern domain assignment table
  - Full checklist for new agent creation
  - Example of well-structured agent
- Consistent structure across all 16 existing agent definitions makes it easy to pattern-match
- Pattern domains are clearly assigned with no overlaps
- Related Agents / Related Skills sections create discoverable collaboration paths

**Minor friction:**
- The guide does not address how to register a new agent with the routing table (`agent-routing.md`). A new agent could be created perfectly but never be routed to.
- No testing guidance for agent definitions. How do you validate that a new agent actually works before deploying it?

---

## Scored Assessment

| Area | Score (1-5) | Summary |
|------|-------------|---------|
| Protocol completeness | 4 | Covers planning and implementation well. Missing timeout, partial failure, rollback. |
| Agent roster | 4 | 16 well-defined agents. Missing deploy, security, config specialists. Generic `specification`/`pseudocode` agents not NDP-customized. |
| Coordinator pattern | 3 | Works at current scale. Single-threaded bottleneck. Contradictory init instructions. No sub-coordinator concept. |
| Error recovery | 2 | Drift detection exists but no timeout handling, partial success extraction, context exhaustion recovery, or file conflict resolution. |
| Knowledge flow | 4 | Dual memory system is excellent. Spec-compile is innovative. Brief quality gate missing. Reflexion limited to primary agent. |
| Redundancy/contradictions | 3 | Positive reinforcement is intentional. 6 contradictions identified. Stale architecture references in agent definitions. |
| Scalability | 3 | Works for 3-8 agents. No horizontal scaling path. Context window pressure at 10+. No concurrent swarm isolation. |
| Onboarding | 5 | Best-in-class agent creation guide. Only missing routing registration and validation guidance. |

**Overall: 3.5/5** -- A strong system with clear architectural vision, excellent knowledge management, but needing hardening for error recovery and scale.

---

## Prioritized Recommendations

### P0 -- Fix Before Next Feature

| # | Recommendation | Effort | Files Affected |
|---|---------------|--------|----------------|
| P0-1 | **Resolve the `claude-flow swarm init` contradiction.** Remove CLI references from `ndp-scrum-master.md` (line 34) and `prompt-check.sh` (lines 36, 60). All three should say: use MCP `hive-mind_init` only. | Small | `ndp-scrum-master.md`, `prompt-check.sh` |
| P0-2 | **Add agent timeout protocol.** Define a maximum wait time per agent (e.g., 10 minutes). If exceeded, the coordinator marks the task as failed and proceeds with available results. Document in `swarm-protocol.md`. | Small | `swarm-protocol.md` |
| P0-3 | **Fix TeamCreate reference in swarm-run.** Line 16 of `swarm-run/SKILL.md` still shows "TeamCreate or parallel Tasks." Remove TeamCreate to match swarm-protocol.md. | Trivial | `swarm-run/SKILL.md` |

### P1 -- Address Within Next 2 Features

| # | Recommendation | Effort | Files Affected |
|---|---------------|--------|----------------|
| P1-1 | **Add partial failure protocol.** When N of M agents fail, commit successful results, create targeted re-spawn prompts for failures, and decrement the drift budget. Document in `swarm-protocol.md` under a new "Partial Failure Recovery" section. | Medium | `swarm-protocol.md`, `implementation-protocol.md` |
| P1-2 | **Create NDP-specific `specification` and `pseudocode` agent definitions.** These are referenced in planning spawns but fall back to generic agents. Define them with NDP domain knowledge (SPARC conventions, project structure, naming conventions). | Medium | New files in `.claude/agents/ndp/` |
| P1-3 | **Add implementation brief quality gate.** After the brief is generated (planning-protocol.md Step 3f), add a brief validation step: verify all ADR pattern IDs resolve, all file paths exist in the codebase, and all acceptance criteria from SCOPE.md are addressed. | Medium | `planning-protocol.md` |
| P1-4 | **Standardize ADR heading levels.** Choose one format (recommend `## ADR-NNN:` with `### Context/Decision/Consequences` as in planning-protocol.md) and update ndp-architect.md to match. | Small | `ndp-architect.md` |
| P1-5 | **Update stale technology status in ndp-architect.md.** Silver is no longer "Planned" -- it is implemented. Gold DDL generation exists. Update the technology table and data layer diagram. | Small | `ndp-architect.md` |
| P1-6 | **Add namespace isolation for concurrent swarms.** Prefix all claude-flow memory keys with a swarm instance ID (e.g., `swarm-{uuid}-{feature-id}-context`) to prevent collision when multiple features run concurrently. | Medium | `swarm-protocol.md`, `swarm-run/SKILL.md` |

### P2 -- Strategic Improvements

| # | Recommendation | Effort | Files Affected |
|---|---------------|--------|----------------|
| P2-1 | **Introduce sub-coordinator concept.** For 10+ agent swarms, allow the scrum-master to delegate wave coordination to a `wave-coordinator` sub-agent. This agent handles spawning, drift-checking, and validating a single wave, then returns results to the scrum-master. Reduces context window pressure. | Large | `swarm-protocol.md`, `ndp-scrum-master.md`, new agent definition |
| P2-2 | **Add context window budget tracking.** Have the coordinator estimate token usage: ~500 tokens per agent prompt, ~1000 tokens per agent result, ~2000 tokens for coordination overhead. If estimated total exceeds 80% of context window, split into sub-waves automatically. | Large | `swarm-protocol.md`, `implementation-protocol.md` |
| P2-3 | **Create `ndp-deploy-engineer` agent.** Specializes in deploy.sh, Docker on Pi, git-as-transport, declarative manifests, integration environment. Currently this domain knowledge is spread across `ndp-rust-dev` and ad-hoc patterns. | Medium | New file in `.claude/agents/ndp/`, update `agent-routing.md` |
| P2-4 | **Prune unreferenced skills.** Of the 44 skills in `.claude/skills/`, only 7 are referenced by NDP protocols (`get-pattern`, `save-pattern`, `reflexion`, `learner`, `validate`, `align`, `ndp-github-workflow`, `swarm-run`, `spec-compile`). Archive or remove the remaining 35 to reduce noise. Alternatively, add a `deprecated: true` flag to their frontmatter. | Medium | `.claude/skills/` directory |
| P2-5 | **Enable per-agent reflexion.** Modify agent prompts to include a reflexion step before returning results. Each agent records its own pattern usage. The primary agent reviews these reflexions rather than guessing pattern effectiveness from a summary. | Medium | `swarm-protocol.md`, agent prompt templates |
| P2-6 | **Add file conflict detection.** Before spawning agents, partition the file modification list so no two agents edit the same file. If overlap is unavoidable, designate one agent as the "owner" and have the other agent write to a staging path for manual merge. | Medium | `implementation-protocol.md` |
| P2-7 | **Document rollback procedure.** When implementation swarm validation fails after 2 iterations, define: (a) git stash/revert the changes, (b) post failure details to GH Issue, (c) create a new GH Issue for the fix, (d) return to user with specific failure context. | Small | `implementation-protocol.md` |

---

## Appendix: File Inventory Reviewed

| File | Lines | Role |
|------|-------|------|
| `.claude/rules/swarm-protocol.md` | 196 | Base swarm protocol |
| `.claude/rules/planning-protocol.md` | 302 | Planning swarm flow |
| `.claude/rules/implementation-protocol.md` | 327 | Implementation swarm flow |
| `.claude/rules/agent-routing.md` | 44 | Agent selection/routing |
| `.claude/rules/pattern-workflow.md` | 103 | Learning loop |
| `.claude/rules/testing.md` | 48 | Testing conventions |
| `.claude/agents/ndp/README.md` | 149 | Agent roster |
| `.claude/agents/ndp/AGENT-CREATION-GUIDE.md` | 377 | Agent onboarding |
| `.claude/agents/ndp/ndp-scrum-master.md` | 284 | Coordinator definition |
| `.claude/agents/ndp/ndp-architect.md` | 177 | Architect definition |
| `.claude/agents/ndp/ndp-rust-dev.md` | 152 | Developer definition |
| `.claude/agents/ndp/ndp-tester.md` | 305 | Tester definition |
| `.claude/agents/ndp/ndp-vision-guardian.md` | 183 | Vision guardian definition |
| `.claude/skills/swarm-run/SKILL.md` | 204 | Working swarm launcher |
| `.claude/skills/spec-compile/SKILL.md` | 219 | Spec-to-memory compiler |
| `.claude/settings.json` | 327 | Hook configuration |
| `.claude/hooks/prompt-check.sh` | 93 | Prompt routing hook |
| `CLAUDE.md` | 128 | Project instructions |
