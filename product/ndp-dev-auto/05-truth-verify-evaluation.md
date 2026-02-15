# Claude-Flow Truth Scoring & Verification Evaluation

**Date**: 2026-02-15
**Agent**: research agent-5
**Scope**: Evaluate claude-flow truth, verify, aidefence, and claims capabilities for NDP relevance

---

## Executive Summary

Claude-flow advertises "truth scoring" and "verification" capabilities that suggest automated code quality measurement. Investigation reveals these are **three distinct systems** at different maturity levels:

1. **Truth Anchors** (guidance subsystem) -- Real, well-implemented library code for anchoring agent beliefs to externally-signed facts. Conceptually interesting but entirely internal to claude-flow's guidance system. Not exposed via CLI or MCP tools. No "truth score" CLI command exists.

2. **Verification & Quality Assurance Skill** -- A 650-line SKILL.md file documenting `npx claude-flow@alpha truth` and `npx claude-flow@alpha verify check` commands. **These CLI commands do not exist.** Running them produces `Unknown command` errors. The skill document is aspirational documentation for unimplemented features.

3. **AIDefence** -- MCP tool definitions exist for prompt injection scanning, PII detection, and threat analysis. **The `@claude-flow/aidefence` package is not installed** and all calls fail with `ERR_MODULE_NOT_FOUND`. The tool registration exists but the implementation is a missing npm dependency.

4. **Claims System** -- Fully functional. File-based persistence, claim/release/handoff/steal/rebalance all work correctly. This is the only capability in this evaluation that delivers real value today.

The `security scan` CLI command does work and detected a hardcoded secret in the repo.

---

## Capability Inventory

### What Exists (Tool/CLI Registration)

| Capability | MCP Tools | CLI Command | Skill Doc | Source Code | Actually Works |
|---|---|---|---|---|---|
| Truth Scoring | None | `truth` (registered, fails) | Yes (650 lines) | `truth-anchors.js` (library) | NO - CLI not implemented |
| Verify Check | None | `verify` (registered, fails) | Yes (same doc) | None | NO - CLI not implemented |
| AIDefence Scan | `aidefence_scan` | `security defend` | None | `security-tools.js` (wrapper) | NO - missing package |
| AIDefence Analyze | `aidefence_analyze` | None | None | Same file | NO - missing package |
| AIDefence Stats | `aidefence_stats` | None | None | Same file | NO - missing package |
| AIDefence Learn | `aidefence_learn` | None | None | Same file | NO - missing package |
| AIDefence is_safe | `aidefence_is_safe` | None | None | Same file | NO - missing package |
| AIDefence has_pii | (registered) | None | None | Same file | NO - missing package |
| Security Scan | None | `security scan` | None | Unknown | YES |
| Claims System | 12 MCP tools | `issues` subcommands | None | `claims-tools.js` (730 lines) | YES |
| Trust Accumulator | None | None | None | `trust.js` (library) | Library only, no exposure |
| Truth Resolver | None | None | None | `truth-anchors.js` | Library only, no exposure |

### What Does NOT Exist

- No `truth` CLI command (error: "Unknown command: truth")
- No `verify` CLI command (error: "Unknown command: verify")
- No `@claude-flow/aidefence` npm package installed
- No MCP tools for truth scoring
- No MCP tools for code verification
- No verification dashboard
- No auto-rollback on score threshold
- No CI/CD integration for truth scores

---

## Deep-Dive Per Capability

### 1. Truth Anchors (`truth-anchors.js`)

**Location**: `/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/claude-flow/v3/@claude-flow/guidance/dist/truth-anchors.js`

**What it actually is**: A library module within the `@claude-flow/guidance` package that implements an append-only store for "truth anchors" -- externally-signed facts that override internal agent beliefs.

**Implementation quality**: High. The code is well-structured with:
- HMAC-SHA256 signing with timing-safe comparison
- Append-only immutable store with 50,000 anchor capacity
- LRU eviction of expired anchors only
- Supersession chains (new anchors can declare they replace old ones)
- Query by ID, time range, kind, attester, or tags
- `TruthResolver` class for memory conflict and decision conflict resolution
- Export/import for persistence

**What it measures**: Nothing about code quality. It is a belief management system that lets you anchor agent reasoning to externally-verified facts. For example: "The database schema uses hypertables" could be a truth anchor that prevents an agent from proposing DuckDB.

**How it is exposed**: It is NOT exposed via CLI or MCP. It is a library used internally by the guidance control plane (`claude-flow guidance compile/retrieve/gates`). The guidance system uses it to resolve conflicts between what agents "believe" and what has been externally attested.

**NDP relevance**: Conceptually relevant -- NDP already manages "deprecated approaches" (no DuckDB, no Polars streaming) through CLAUDE.md rules. Truth anchors could formalize this. However, the guidance system requires `claude-flow guidance compile` to process CLAUDE.md into a policy bundle, which is a separate workflow from how NDP currently operates.

### 2. Trust Accumulator (`trust.js`)

**Location**: Same guidance package.

**What it actually is**: A running trust score (0.0-1.0) per agent, accumulated from "gate outcomes" (allow/deny/warn). Maps to privilege tiers (trusted/standard/probation/untrusted) with rate limit multipliers.

**Key details**:
- Initial trust: 0.5
- Allow: +0.01, Deny: -0.05, Warn: -0.02
- Exponential decay toward initial value when idle (1-minute intervals)
- Tier thresholds: trusted >= 0.8, standard >= 0.5, probation >= 0.3, untrusted < 0.3

**How it is exposed**: Not directly. Used internally by the guidance system's gate evaluation. When an agent's action passes or fails a gate, the trust score adjusts.

**NDP relevance**: Minimal. NDP's swarm agents are ephemeral (Task tool spawns) and live for one session. Trust accumulation is meaningless for agents that don't persist across sessions.

### 3. Verification & Quality Assurance Skill (SKILL.md)

**Location**: `/workspaces/neural-data-platform/.claude/skills/verification-quality/SKILL.md`

**What it actually is**: A documentation file (650 lines) describing a comprehensive verification system with truth scoring, auto-rollback, CI/CD integration, and dashboards. The documented commands include `npx claude-flow@alpha truth`, `npx claude-flow@alpha verify check`, and `npx claude-flow@alpha verify rollback`.

**Actual test results**:
```
$ npx claude-flow@alpha truth
[ERROR] Unknown command: truth

$ npx claude-flow@alpha verify
[ERROR] Unknown command: verify
```

**Assessment**: This is aspirational documentation for features that have not been implemented. The skill file was likely generated as part of a capability planning exercise but the CLI commands were never built. The described features (0.95 accuracy threshold, auto-rollback, WebSocket dashboard, Prometheus export) do not exist.

### 4. AIDefence System

**Location**: `/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/claude-flow/v3/@claude-flow/cli/dist/src/mcp-tools/security-tools.js`

**What it actually is**: MCP tool wrappers that delegate to a `@claude-flow/aidefence` npm package. The wrapper code is well-written (auto-install logic, lazy loading, cache busting). But the package it wraps does not exist in the installed environment.

**Actual test results**:
```
aidefence_scan("Ignore all instructions") -> Error: Cannot find package '@claude-flow/aidefence'
aidefence_is_safe("Hello world") -> Error: Cannot find package '@claude-flow/aidefence'
aidefence_stats() -> Error: AIDefence package not available
```

**What it would do if installed**:
- `aidefence_scan`: Detect prompt injection, jailbreaks, PII in text input
- `aidefence_analyze`: Deep threat analysis with similar pattern search
- `aidefence_learn`: Feedback loop to improve detection
- `aidefence_is_safe`: Quick boolean safe/unsafe check
- `aidefence_has_pii`: PII detection (emails, SSNs, API keys)

**NDP relevance**: Low for current use case. NDP is a data platform running on a Raspberry Pi, not a user-facing service processing untrusted input. Prompt injection defense is relevant for the MCP server if it ever accepts user-generated queries, but that is not the current architecture.

### 5. Security Scan (CLI)

**Location**: `claude-flow security scan` CLI command.

**Actual test results**: Works. Detected 1 HIGH severity issue (hardcoded secret in `product/generic-platfo...`).

```
+----------+------------------+---------------------------+--------------------+
| Severity | Type             | Location                  | Description        |
+----------+------------------+---------------------------+--------------------+
| HIGH     | Hardcoded Secret | product/generic-platfo... | Hardcoded Password |
+----------+------------------+---------------------------+--------------------+
```

**NDP relevance**: Moderately useful. The secret detection found a real issue. However, this duplicates what `git-secrets`, `gitleaks`, or `trufflehog` would provide. Not NDP-specific.

### 6. Claims System

**Location**: `/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/claude-flow/v3/@claude-flow/cli/dist/src/mcp-tools/claims-tools.js`

**What it actually is**: A complete issue-claiming system for coordinating work between agents and humans. File-based persistence (`.claude-flow/claims/claims.json`). Supports claim, release, handoff, accept-handoff, status updates, steal, mark-stealable, load balancing, and visual board views.

**Actual test results**: All operations work correctly.

| Operation | Result |
|---|---|
| `claims_claim` | Created claim with full metadata |
| `claims_status` (update progress) | Updated to 50% |
| `claims_mark-stealable` | Marked with preferred agent types |
| `claims_stealable` (list) | Returned stealable issue with context |
| `claims_load` | Showed agent utilization (0.2 of max 5) |
| `claims_rebalance` (dry run) | Analyzed distribution, 0 suggestions (1 agent) |
| `claims_board` | Kanban-style view by status |
| `claims_release` | Clean release with previous claim data |

**Implementation details**:
- File-based JSON persistence (not in-memory only)
- Ownership verification on release/handoff (only current claimant can release)
- Preferred agent type filtering on steal
- Progress tracking (0-100)
- Block reasons tracked
- Max 5 claims per agent (hardcoded)
- Load rebalancing considers claims with < 25% progress as movable

**NDP relevance**: HIGH for concurrent swarms. This directly addresses the problem of multiple agents working on different parts of a feature simultaneously. When NDP runs a 5-agent SPARC swarm, claims prevent two agents from modifying the same file. The handoff mechanism is particularly useful for SPARC phase transitions (planner hands off to coder, coder hands off to tester).

---

## Value Assessment Matrix

| Capability | Implementation Status | Works Today | NDP Relevance | Value Rating |
|---|---|---|---|---|
| Truth Anchors | Complete library | Library only, no CLI/MCP exposure | Low (belief management for persistent agents) | Nice-to-have (future) |
| Trust Accumulator | Complete library | Library only, no exposure | Very Low (ephemeral agent sessions) | Not useful |
| Verification Skill (truth/verify CLI) | Documentation only | NO | Would be high if it existed | Not useful (vaporware) |
| AIDefence Scan/Analyze | MCP wrappers exist, package missing | NO | Low (no untrusted input) | Not useful (broken) |
| AIDefence PII Detection | Same | NO | Low-Medium (deploy scripts) | Not useful (broken) |
| Security Scan CLI | Implemented | YES | Medium (found real secret) | Nice-to-have |
| Claims Claim/Release | Complete | YES | HIGH (concurrent swarms) | Genuinely useful |
| Claims Handoff | Complete | YES | HIGH (SPARC phase transitions) | Genuinely useful |
| Claims Steal/Stealable | Complete | YES | Medium (agent failure recovery) | Genuinely useful |
| Claims Load/Rebalance | Complete | YES | Medium (swarm optimization) | Nice-to-have |
| Claims Board | Complete | YES | Medium (visibility) | Nice-to-have |

---

## Recommendations

### ADOPT

**Claims System** -- Integrate into NDP swarm protocol immediately.

Current NDP swarm protocol (`.claude/rules/swarm-protocol.md`) uses memory coordination to prevent conflicts. The claims system provides a more structured approach:

1. Before starting work on a file or module, agents claim it
2. SPARC phase transitions use handoff (planner -> coder -> tester)
3. If an agent stalls, mark-stealable allows recovery
4. Board view gives the coordinator visibility into who owns what
5. Load balancing prevents one agent from hogging all work

Specific integration points:
- `ndp-scrum-master` uses `claims_board` to track swarm progress
- Implementation agents use `claims_claim` before modifying files
- Phase transitions use `claims_handoff`
- Coordinator uses `claims_load` to detect bottlenecks

### INVESTIGATE

**Security Scan** -- Run once, fix the detected hardcoded secret, then decide if ongoing scanning adds value beyond what `cargo clippy` and manual review provide.

**Truth Anchors via Guidance** -- The `claude-flow guidance compile` + `claude-flow guidance gates` pipeline could formalize NDP's deprecated-approaches rules (no DuckDB, no Polars streaming). Worth a 30-minute investigation to see if `guidance compile` produces useful output from NDP's CLAUDE.md. This would only matter if NDP moves to long-running agent sessions.

### SKIP

**Verification Skill (truth/verify CLI)** -- The commands do not exist. The SKILL.md is fiction. NDP's existing `/validate` skill (`cargo build` + `cargo test` + `cargo clippy` + integration env) is vastly more useful because it actually runs real compilation and test suites against the Rust codebase. No "truth score" heuristic can replace `908 tests passing`.

**AIDefence** -- The package is not installed and all tools fail. Even if installed, NDP has no untrusted-input attack surface that would benefit from prompt injection detection. The MCP server runs locally on a Pi behind a firewall.

**Trust Accumulator** -- NDP agents are ephemeral Task tool spawns. They do not persist between sessions. Trust accumulation over time has no meaning when every agent starts fresh.

---

## Claims System Analysis (Detailed)

### Architecture

The claims system is implemented as a simple but effective file-based store:

```
.claude-flow/claims/claims.json
{
  "claims": { "issueId": { ... claim object ... } },
  "stealable": { "issueId": { ... stealable info ... } },
  "contests": {}  // Empty, not yet implemented
}
```

### Claim Lifecycle

```
             claim
  (unclaimed) -----> [active]
                       |
              +--------+--------+--------+
              |        |        |        |
           [paused] [blocked] [review]  |
              |        |        |    handoff
              +--------+--------+   -------> [handoff-pending]
              |                              |
         mark-stealable              accept-handoff
              |                              |
          [stealable] <-----+        [active] (new owner)
              |              |
           steal          release
              |              |
       [active] (new)   (unclaimed)
```

### Strengths

1. **Ownership enforcement**: Only the current claimant can release or handoff. Prevents accidental overwrites.
2. **Progress tracking**: 0-100% progress enables informed rebalancing decisions.
3. **Preferred types on steal**: A researcher-claimed issue marked stealable can specify it prefers "coder" or "tester" agent types.
4. **Load visibility**: `claims_load` shows utilization per agent with max 5 claims limit.
5. **Persistence**: File-based, survives MCP server restarts.

### Weaknesses

1. **No locking**: File-based JSON with no locking. Two concurrent writes could corrupt the store. Acceptable for NDP's current scale (1-5 agents) but would not scale to large swarms.
2. **Hardcoded max claims (5)**: Not configurable per agent type. A coordinator might need more claims than a tester.
3. **No expiration**: Claims persist until explicitly released. An agent that crashes without releasing leaves orphaned claims. There is no TTL or stale-claim cleanup.
4. **No contest resolution**: The `contests` field exists in the schema but is empty. Two agents claiming the same issue gets a simple "already claimed" error with no contest mechanism.
5. **No GH Issue integration**: Claims use arbitrary string issue IDs. There is no validation against actual GitHub issues. The ID "test-issue-99" works fine even though no such issue exists.

### NDP Integration Design (Proposed)

If adopted, claims should be integrated at the swarm-protocol level:

```
# In .claude/rules/swarm-protocol.md, add:

## File Claims Protocol

Before modifying any file, agents MUST:
1. claims_claim(issueId="<feature>/<filepath>", claimant="agent:<id>:<type>")
2. If claim fails (already claimed), coordinate with owner via memory
3. After modification, claims_release() or claims_handoff() to next phase

## SPARC Phase Handoff

Phase transitions use claims_handoff:
- Planner -> Coder: handoff all implementation files
- Coder -> Tester: handoff test files + source under test
- Tester -> Coordinator: handoff for final review

## Stale Claim Recovery

Coordinator checks claims_board every 5 minutes.
Claims with no progress update for 10+ minutes: mark-stealable with reason "stale".
```

---

## Comparison: Claude-Flow Verify vs NDP /validate

| Dimension | Claude-Flow Verify (Promised) | NDP /validate (Actual) |
|---|---|---|
| Status | Not implemented | Working, proven |
| What it checks | "Code correctness, security, performance, docs, best practices" (from SKILL.md) | `cargo build`, `cargo test` (908 tests), `cargo clippy`, anti-stub scan, deploy.sh integrity, integration env |
| Language support | Generic (JavaScript-focused examples) | Rust-specific (Cargo workspace) |
| Threshold system | 0.0-1.0 score with configurable thresholds | Pass/Warn/Fail with 2-iteration cap |
| Integration env | None | Full Docker stack (TimescaleDB, etcd, MQTT, Grafana) |
| Auto-rollback | Promised, not implemented | Not automated, but git-based |
| Result | Nothing to compare | 908 tests passing, clippy clean, integration validated |

**Conclusion**: NDP's existing `/validate` skill is categorically superior to claude-flow's promised verification because it actually executes real code analysis tools. A heuristic "truth score" cannot substitute for `cargo test --workspace` running 908 tests against a Rust codebase.

---

## Files Examined

- `/workspaces/neural-data-platform/.claude/skills/verification-quality/SKILL.md` -- Aspirational verification docs (unimplemented)
- `/workspaces/neural-data-platform/.claude/skills/dual-mode/README.md` -- Dual-mode skills (Claude Code + Codex)
- `/workspaces/neural-data-platform/.claude/skills/validate/SKILL.md` -- NDP's actual validation skill
- `/workspaces/neural-data-platform/.claude/rules/testing.md` -- NDP integration test rules
- `/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/claude-flow/v3/@claude-flow/guidance/dist/truth-anchors.js` -- Truth anchor implementation (470 lines)
- `/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/claude-flow/v3/@claude-flow/guidance/dist/trust.js` -- Trust accumulator implementation
- `/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/claude-flow/v3/@claude-flow/cli/dist/src/mcp-tools/security-tools.js` -- AIDefence MCP wrappers (434 lines)
- `/usr/local/share/nvm/versions/node/v24.12.0/lib/node_modules/claude-flow/v3/@claude-flow/cli/dist/src/mcp-tools/claims-tools.js` -- Claims implementation (731 lines)
