# AgentDB Memory System: Deep Analysis & Fix Plan

**Date**: 2026-02-15
**Status**: Research complete, implementation needed
**Problem**: Learning loop is broken — data goes in but nothing learns from it

---

## 1. Executive Summary

AgentDB provides **32 MCP tools across 10 families**, but NDP only uses **6 of them** (pattern_store, pattern_search, pattern_stats, reflexion_store, reflexion_retrieve, learner_discover). The result:

- **29 episodes** stored (reflexion feedback) — never used for learning
- **17 patterns** stored (architecture knowledge) — static, never refined
- **0 skills** — never populated
- **0 causal edges** — never discovered
- **0 learning sessions** — RL engine never started
- **0 experiences** — structured RL data never recorded

The system stores knowledge but has **no mechanism to learn from it**.

---

## 2. Current Database State

```
Episodes (reflexion_store):     29  (feedback on pattern usage)
Reasoning Patterns:             17  (manual architecture knowledge)
Skills:                          0  (never populated)
Causal Edges:                    0  (never discovered)
Learning Sessions:               0  (RL never started)
Experiments/Observations:        0  (A/B testing unused)
Database Size:                0.29 MB
```

---

## 3. AgentDB Tool Families (Complete Inventory)

### Currently Used by NDP (6 tools)

| Tool | Used By | Store |
|------|---------|-------|
| `agentdb_pattern_store` | `/save-pattern` | patterns table |
| `agentdb_pattern_search` | `/get-pattern` | patterns table |
| `agentdb_pattern_stats` | `/get-pattern`, `/save-pattern` | patterns table |
| `reflexion_store` | `/reflexion` | episodes table |
| `reflexion_retrieve` | `/get-pattern` (fallback) | episodes table |
| `learner_discover` | `/learner` (MCP alternative) | causal edges |

### Never Used (26 tools)

| Family | Tools | Purpose |
|--------|-------|---------|
| **Core Vector** | `agentdb_insert`, `agentdb_search`, `agentdb_delete`, `agentdb_insert_batch`, `agentdb_init`, `agentdb_clear_cache` | Generic vector CRUD |
| **Skills** | `skill_create`, `skill_search`, `skill_create_batch` | Reusable procedure library |
| **Causal** | `causal_add_edge`, `causal_query` | Cause-effect knowledge graph |
| **RL Learning** | `learning_start_session`, `learning_end_session`, `learning_predict`, `learning_feedback`, `learning_train`, `learning_metrics`, `learning_transfer`, `learning_explain` | Reinforcement learning engine |
| **Experience** | `experience_record`, `reward_signal` | Structured RL data pipeline |
| **Recall** | `recall_with_certificate` | Multi-signal ranked retrieval |
| **Batch** | `reflexion_store_batch`, `skill_create_batch`, `agentdb_pattern_store_batch` | Bulk operations |
| **Stats** | `db_stats`, `agentdb_stats` | Diagnostics |

---

## 4. The Broken Learning Loop

### What Currently Happens

```
save-pattern  ──► PATTERNS TABLE ──► get-pattern reads
                     (17 records)        (static, never refined)

reflexion     ──► EPISODES TABLE ──► reflexion_retrieve
                     (29 records)        (manual fallback only)

learner_discover ──► CAUSAL EDGES ──► (nothing reads these)
                     (0 records - needs 3+ attempts per pattern)
```

### What Should Happen (Full Loop)

```
BEFORE TASK:
  get-pattern ──► patterns table (semantic search)
  learning_predict ──► trained RL policy (action recommendations)
  recall_with_certificate ──► blended retrieval (similarity + causal + recency)

DURING TASK:
  experience_record ──► structured SARS tuples (state/action/reward/state')

AFTER TASK:
  reflexion_store ──► episodes (self-critique)
  reward_signal ──► composite reward calculation
  causal_add_edge ──► cause-effect relationships from this task

PERIODIC:
  learner_discover ──► mine episodes for causal patterns
  learning_train ──► train RL policy from experiences
  learning_transfer ──► apply learning across domains
  Pattern successRate updates from reflexion feedback
```

---

## 5. Eight Critical Gaps

### GAP 1: Learner Outputs Go to Wrong Table (HIGH)
**Problem**: `/learner` CLI writes to the **skills table** via `agentdb skill consolidate`. But `/get-pattern` searches the **patterns table** via `agentdb_pattern_search`. Auto-discovered knowledge is orphaned.
**Fix**: Use `agentdb_pattern_store` (not skill_create) when consolidating discoveries. Or add skill_search to get-pattern.

### GAP 2: Pattern Success Rates Are Static (HIGH)
**Problem**: When `save-pattern` stores a pattern with `successRate=0.9`, that score never changes. Reflexion feedback goes to a separate episodes table. The pattern's own score is never updated based on real usage.
**Fix**: After reflexion, update the pattern's successRate based on accumulated reward scores. Or use `recall_with_certificate` which blends similarity + causal uplift + recency.

### GAP 3: RL Engine Never Started (HIGH)
**Problem**: 8 `learning_*` MCP tools exist for full reinforcement learning but are never called. No sessions, no training, no predictions.
**Fix**: Start a persistent RL session. Feed reflexion data as learning_feedback. Periodically train. Use learning_predict/learning_explain before tasks.

### GAP 4: Causal Edges Inaccessible (MEDIUM)
**Problem**: Even if `learner_discover` finds causal patterns, no normal workflow tool queries them. The `causal_query` tool exists but is never called.
**Fix**: Integrate causal_query into get-pattern or use recall_with_certificate (which blends causal uplift into scoring).

### GAP 5: No Structured Experience Recording (MEDIUM)
**Problem**: `experience_record` captures state-before/state-after/action/outcome as structured RL tuples. Never called. Reflexion captures similar data but as unstructured text.
**Fix**: Add experience_record calls to post-task hooks for tool executions that should train the RL model.

### GAP 6: recall_with_certificate Never Used (MEDIUM)
**Problem**: This tool blends three signals into a single ranked result: `alpha*similarity + beta*causal_uplift + gamma*recency`. It would replace the pattern-only search with a multi-signal retrieval that factors in causal knowledge and freshness. Never called.
**Fix**: Replace or augment get-pattern's primary search with recall_with_certificate.

### GAP 7: learner_discover MCP Tool vs CLI Mismatch (MEDIUM)
**Problem**: The `/learner` skill documents CLI commands (`agentdb learner run`), but the MCP tool `learner_discover` exists and works (I tested it — returns 0 because not enough episodes have structured data). The CLI may not even be installed.
**Fix**: Update learner skill to use MCP tool directly.

### GAP 8: Batch Tools Unused (LOW)
**Problem**: After a feature with 10+ reflexion entries, they're stored one-at-a-time instead of using `reflexion_store_batch`.
**Fix**: Use batch tools for post-feature reflexion dumps.

---

## 6. ReasoningBank: What It Actually Is

### Academic Origin
ReasoningBank (arXiv:2509.25140, Ouyang et al., Google/UIUC) is a memory framework where agents:
1. **RETRIEVE** relevant reasoning strategies before a task
2. **JUDGE** whether the trajectory was successful
3. **DISTILL** generalizable strategies (not raw logs)
4. **CONSOLIDATE** similar strategies into higher-quality patterns

### In claude-flow/AgentDB
ReasoningBank = the **patterns table** + the **learning subsystem**. The `agentdb_pattern_*` tools implement the RETRIEVE/STORE parts. The `learning_*` tools implement JUDGE/TRAIN/PREDICT. The `learner_discover` tool implements DISTILL.

### Two ReasoningBank Skills (Documentation Only)
- `reasoningbank-agentdb/SKILL.md` — Documents TypeScript API using `agentic-flow/reasoningbank` package (NOT installed)
- `reasoningbank-intelligence/SKILL.md` — Documents adaptive learning patterns (NOT installed)

These skills describe the correct architecture but reference npm packages that aren't available locally. The MCP tools implement the same functionality and ARE available.

---

## 7. How the 10 AgentDB Stores Relate

```
┌──────────────────────────────────────────────────────────────┐
│                    AGENT DECISION MAKING                      │
│                                                              │
│  recall_with_certificate  ◄── Blended retrieval              │
│  learning_predict         ◄── RL action recommendations      │
│  learning_explain         ◄── Explainable recommendations    │
│  agentdb_pattern_search   ◄── Pattern-only search            │
│  reflexion_retrieve       ◄── Episode search                 │
│  skill_search             ◄── Skill library search           │
└──────────────┬───────────────────────────────────────────────┘
               │ BEFORE task
               ▼
┌──────────────────────────────────────────────────────────────┐
│                    AGENT EXECUTES TASK                        │
└──────────────┬───────────────────────────────────────────────┘
               │ AFTER task
               ▼
┌──────────────────────────────────────────────────────────────┐
│                    DATA COLLECTION                            │
│                                                              │
│  reflexion_store      ──► EPISODES (self-critique + reward)  │
│  experience_record    ──► EXPERIENCES (SARS tuples)          │
│  reward_signal        ──► COMPUTED REWARDS (multi-factor)    │
│  agentdb_pattern_store──► PATTERNS (if new discovery)        │
│  causal_add_edge      ──► CAUSAL GRAPH (cause-effect)        │
│  skill_create         ──► SKILLS (reusable procedures)       │
└──────────────┬───────────────────────────────────────────────┘
               │ PERIODIC
               ▼
┌──────────────────────────────────────────────────────────────┐
│                    LEARNING & TRAINING                        │
│                                                              │
│  learner_discover     ──► Mine episodes for causal patterns  │
│  learning_train       ──► Train RL policy from experiences   │
│  learning_transfer    ──► Transfer knowledge across domains  │
│  learning_metrics     ──► Track learning effectiveness       │
└──────────────────────────────────────────────────────────────┘
```

### Store Relationships

| Store | Writes To | Reads From | Cognitive Analog |
|-------|-----------|------------|------------------|
| **Patterns** | Manual save-pattern | get-pattern, recall | Strategic Memory |
| **Episodes** | reflexion after work | reflexion retrieve, learner | Episodic Memory |
| **Skills** | learner consolidate | skill_search | Procedural Memory |
| **Causal Edges** | learner discover | recall, causal_query | Causal Reasoning |
| **Experiences** | experience_record | learning_train | RL Training Data |
| **RL Sessions** | learning_start/train | learning_predict | Trained Policies |
| **Rewards** | reward_signal | reflexion, experience | Reward Shaping |
| **Recall** | (read-only aggregator) | All stores | Working Memory |

---

## 8. Recommended Fix: Pragmatic Learning Loop

Rather than implementing the full RL pipeline (which requires significant experience data), here's a pragmatic 3-phase plan:

### Phase 1: Fix What Exists (Immediate)

1. **Fix learner output target**: Update `/learner` skill to use `agentdb_pattern_store` instead of legacy `skill consolidate`. Auto-discovered patterns go to the same table get-pattern reads.

2. **Add causal_query to get-pattern**: When patterns are retrieved, also query causal edges to enrich results with cause-effect knowledge.

3. **Use learner_discover MCP tool**: Replace CLI-based learner with the MCP tool that's already available.

4. **Use recall_with_certificate**: Optionally replace or augment pattern_search with certified recall for multi-signal retrieval.

### Phase 2: Close the Feedback Loop (Short-term)

5. **Start a persistent learning session**: Use `learning_start_session` with `decision-transformer` algorithm. This session persists across conversations.

6. **Feed reflexion data as learning_feedback**: After each reflexion_store, also call learning_feedback with the same data in SARS format:
   - state = "task context / what I'm working on"
   - action = "pattern I chose to apply"
   - reward = reflexion reward score
   - next_state = "outcome / what happened"

7. **Use learning_predict before tasks**: Before get-pattern, also call learning_predict to get RL-recommended actions. Compare with pattern search results.

8. **Periodic learning_train**: After every 10-20 reflexion entries, call learning_train to update the policy.

### Phase 3: Full ReasoningBank Pipeline (Medium-term)

9. **Add experience_record to hooks**: Post-task hooks record structured experiences automatically.

10. **Use reward_signal for composite rewards**: Replace flat reflexion rewards with multi-factor reward calculation (success + efficiency + quality + causal impact).

11. **Implement learning_transfer**: After completing a feature, transfer learning to related domains.

12. **Add causal_add_edge from reflexion**: When reflexion identifies a clear cause-effect (e.g., "using pattern X caused outcome Y"), automatically create a causal edge.

---

## 9. Trust/Verification Use Case

For the trust infrastructure (ops-006), the relevant AgentDB tools are:

- **recall_with_certificate**: Provides provenance certificates for retrieved knowledge — useful for auditing why an agent made a decision
- **causal_query**: "What actions historically caused good/bad outcomes?" — useful for risk assessment
- **learning_explain**: Explainable recommendations with confidence scores and evidence — useful for trust scoring
- **reward_signal**: Multi-factor reward computation — useful for objective trust metrics

The trust-dashboard already uses Bayesian Beta scores from reflexion data. Adding causal edges and RL predictions would strengthen the trust signal.

---

## 10. learning-service.mjs: Hidden Asset

File: `/workspaces/neural-data-platform/.claude/helpers/learning-service.mjs` (1,145 lines)

This is a real implementation with:
- Custom HNSWIndex (in-memory graph with SQLite persistence)
- EmbeddingService (fallback from ONNX to deterministic hash)
- Short-term/long-term pattern promotion pipeline
- Trajectory tracking, consolidation, deduplication

**Status**: Never initialized (`.claude-flow/learning/` directory doesn't exist). Uses `better-sqlite3` (may not be installed). Separate from AgentDB MCP tools — parallel implementation.

**Recommendation**: Don't activate. The AgentDB MCP tools provide the same functionality and are already wired. Focus on using the MCP tools correctly rather than maintaining a separate system.

---

## Appendix A: Quick Test — Proving the Loop Works

```
# 1. Start a learning session
learning_start_session(user_id="ndp", session_type="q-learning", config={learning_rate: 0.1, discount_factor: 0.95})

# 2. Record some feedback from existing reflexion data
learning_feedback(session_id="<from step 1>", state="implementing bronze WAL", action="used WAL-only pattern", reward=0.95, success=true, next_state="bronze working correctly")

# 3. Train on collected feedback
learning_train(session_id="<from step 1>", epochs=10)

# 4. Now ask for a prediction
learning_predict(session_id="<from step 1>", state="implementing new data pipeline")

# 5. Get explained recommendations
learning_explain(query="how to implement a new data stream")
```

## Appendix B: Tool Parameter Gotcha

`agentdb_pattern_search` requires parameter `task` (not `query`). This has caused crashes when called with `query` parameter — the tokenizer receives null text.
