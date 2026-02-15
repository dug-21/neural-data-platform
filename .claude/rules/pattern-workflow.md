# Pattern Workflow (Learning Loop)

This is a long-running development program. You are not only here to build — you are training future agents to be effective. Knowledge capture is as important as code delivery.

## The Loop (mandatory, every task)

```
BEFORE work:  /get-pattern  → Search patterns + RL predictions + certified recall
DURING work:  Apply patterns, note gaps or new discoveries
AFTER work:   /reflexion    → Rate patterns + feed RL engine + optional causal edges (REQUIRED)
              /save-pattern → Store NEW reusable knowledge (if any)
PERIODIC:     /learner      → Train RL policy, discover causal patterns
```

## IMPORTANT: This is AgentDB, NOT claude-flow memory

The `/get-pattern`, `/save-pattern`, and `/reflexion` skills use **AgentDB** — the permanent knowledge store. This is completely separate from `claude-flow memory` which is transient coordination state.

| | AgentDB (this workflow) | Claude-Flow Memory |
|---|---|---|
| **What** | Architecture, conventions, procedures, RL sessions, causal edges | Task status, agent coordination |
| **Persistence** | Permanent — survives forever | Transient — dies after the work |
| **Tools** | `/get-pattern`, `/save-pattern`, `/reflexion`, `/learner` skills | `claude-flow memory store/retrieve` via Bash |
| **When** | Before/after every task | During swarm coordination only |
| **Example** | "Domain adapters use Source/Sink traits", RL predicts best pattern | "Agent-3 finished schema work" |

**Do NOT** use `claude-flow memory` for storing architectural knowledge.
**Do NOT** use AgentDB patterns for storing task coordination status.

## AgentDB Skills

| Skill | When | What |
|-------|------|------|
| `get-pattern` | BEFORE work | Search patterns + RL predictions + certified recall |
| `save-pattern` | AFTER discoveries | Store NEW reusable knowledge |
| `reflexion` | AFTER work | Rate EACH pattern + feed RL engine + optional causal edges |
| `learner` | Post-feature | Train RL policy, discover causal patterns from episodes |

## Persistent Learning Session

The RL engine requires a persistent session for `learning_predict`, `learning_feedback`, and `learning_train`. The session ID is stored in auto-memory (`MEMORY.md`).

- **Current session**: See `MEMORY.md` for the active session ID
- Skills read the session ID automatically — no manual intervention needed
- If no session exists, RL features are skipped gracefully (pattern search still works)
- To create a new session: use `learning_start_session(user_id="ndp-project", session_type="decision-transformer", config={learning_rate: 0.01, discount_factor: 0.99})`

## Full Learning Data Flow

```
save-pattern ──► patterns table ──► get-pattern reads
                      ▲                    │
                      │ update scores       │ usage data
                      │                    ▼
reflexion    ──► episodes table ──► learning_feedback ──► learning_train
                      │                                        │
                      ▼                                        ▼
              learner_discover ──► causal edges ──► recall_with_certificate
                                                         │
                                                         ▼
                                                  learning_predict
                                                  learning_explain
```

The `/get-pattern` skill reads from three sources: pattern search (similarity), recall_with_certificate (multi-signal), and learning_predict (RL recommendations). The `/reflexion` skill writes to two destinations: reflexion_store (episodes) and learning_feedback (RL engine). The `/learner` skill trains the RL policy and discovers causal patterns.

## How reflexion works (READ THIS)

Reflexion is **feedback on specific patterns**, not a project status update. It ranks patterns so future agents find the best knowledge first.

Record ONE reflexion entry per pattern you used. Reference the pattern by ID and name.

**GOOD reflexion** (specific, per-pattern, actionable):
```
reflexion_store(
  task="Used pattern ID 27 (deprecated-approaches) during ops-002",
  reward=1.0, success=true,
  critique="Prevented me from trying DuckDB for Gold aggregations. Saved significant rework."
)
reflexion_store(
  task="Used pattern ID 19 (release-workflow) for ops-002 release",
  reward=0.6, success=true,
  critique="Missing Gold layer manifest steps. Had to improvise. Needs update via save-pattern."
)
```

**BAD reflexion** (status update, no pattern reference):
```
reflexion_store(
  task="Completed ops-002 config-driven refactoring",
  reward=0.9, success=true,
  critique="Used SPARC phases, delivered all generators successfully"
)
```
This tells the system nothing about which patterns helped or hurt.

**Reflexion reward scale**:
- `1.0` — Pattern was exactly right, followed it directly
- `0.7-0.9` — Pattern helped but needed minor adaptation
- `0.4-0.6` — Pattern was partially relevant, significant gaps
- `0.1-0.3` — Pattern was misleading or outdated, caused rework
- `0.0` — Pattern was wrong, actively harmful

**If get-pattern returned nothing useful**: still record a reflexion with reward=0.0 and critique explaining what you searched for and what was missing. This signals gaps that `/save-pattern` should fill.

## When to check patterns (get-pattern)

- Starting a new feature (search for similar implementations)
- Debugging an issue (search for past solutions)
- Refactoring code (search for learned patterns)
- Performance work (search for optimization strategies)
- Any SPARC phase transition

## When to store patterns (save-pattern)

- Solved a tricky bug (store the solution)
- Completed a feature (store the approach)
- Found a performance fix (store the optimization)
- Discovered a security issue (store the vulnerability pattern)
- Created a new convention or procedure
- Reflexion revealed a gap (pattern was missing or outdated)

## Continuous Improvement Triggers

| Trigger | Worker | When |
|---------|--------|------|
| After major refactor | `optimize` | Performance optimization |
| After adding features | `testgaps` | Missing test coverage |
| After security changes | `audit` | Security analysis |
| After API changes | `document` | Documentation |
| Every 5+ file changes | `map` | Codebase mapping |
