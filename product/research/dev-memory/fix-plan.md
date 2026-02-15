# Fix Plan: Make AgentDB Learning Actually Work

## The Problem in One Sentence

You're storing patterns and reflexions, but nothing reads reflexions to improve future pattern recommendations — the "learn" step is missing.

## Current Data Flow (Broken)

```
save-pattern ──► patterns table ──► get-pattern reads (static, never refined)
reflexion    ──► episodes table ──► DEAD END (nothing learns from this)
```

## Target Data Flow (Working)

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

## Phase 1: Quick Wins (Can Do Now)

### 1.1 Update `/learner` skill to use MCP tools
Replace CLI-based `agentdb learner run` with `mcp__agentdb__learner_discover`. Replace `agentdb skill consolidate` with `mcp__agentdb__agentdb_pattern_store` (write to patterns table, not legacy skills table).

### 1.2 Add `recall_with_certificate` to `/get-pattern`
After pattern_search, also call recall_with_certificate for multi-signal retrieval. This factors in causal knowledge and recency, not just similarity.

### 1.3 Fix the parameter bug
Document that `agentdb_pattern_search` requires `task` parameter, not `query`. Update any code that passes `query`.

## Phase 2: Close the Learning Loop

### 2.1 Create a persistent learning session
```
learning_start_session(
  user_id="ndp-project",
  session_type="decision-transformer",
  config={learning_rate: 0.01, discount_factor: 0.99}
)
```
Store the session_id in auto-memory for reuse across conversations.

### 2.2 Update `/reflexion` skill to also feed the RL engine
After every `reflexion_store`, also call:
```
learning_feedback(
  session_id=<persistent session>,
  state=<task context>,
  action=<pattern used or approach taken>,
  reward=<reflexion reward>,
  success=<reflexion success>,
  next_state=<outcome>
)
```

### 2.3 Add periodic training trigger
After every ~10 reflexions, call:
```
learning_train(session_id=<persistent session>, epochs=20)
```

### 2.4 Update `/get-pattern` to check RL predictions
Before or after pattern_search, also call:
```
learning_predict(session_id=<persistent session>, state=<current task context>)
```
Present both pattern search results AND RL predictions.

## Phase 3: Full Pipeline (Medium-term)

### 3.1 Add `causal_add_edge` to reflexion
When a reflexion clearly identifies a cause-effect relationship, create a causal edge.

### 3.2 Use `reward_signal` for composite rewards
Replace flat 0-1 reflexion scores with multi-factor reward:
```
reward_signal(
  success=true,
  quality_score=0.9,
  efficiency_score=0.8,
  reward_function="shaped"
)
```

### 3.3 Add `experience_record` to post-task hooks
Automatically record tool executions as structured RL experiences.

### 3.4 Periodic `learning_transfer`
After completing a feature, transfer learning to related domains.

## Files to Modify

| File | Change |
|------|--------|
| `.claude/skills/learner/SKILL.md` | Replace CLI with MCP tools, target patterns table |
| `.claude/skills/get-pattern/SKILL.md` | Add recall_with_certificate, add learning_predict |
| `.claude/skills/reflexion/SKILL.md` | Add learning_feedback after reflexion_store |
| `.claude/rules/pattern-workflow.md` | Document the full learning loop |
| Auto-memory `MEMORY.md` | Store persistent learning session ID |

## Success Criteria

- `learning_metrics()` returns non-zero episodes and training data
- `learning_predict()` returns actual recommendations (not empty)
- `learning_explain()` returns evidence-backed suggestions
- `causal_query()` returns discovered cause-effect edges
- `agentdb_stats()` shows: Learning Sessions > 0, Causal Edges > 0
