# Pattern Workflow (Learning Loop)

This is a long-running development program. You are not only here to build — you are training future agents to be effective. Knowledge capture is as important as code delivery.

## The Loop (mandatory, every task)

```
BEFORE work:  /get-pattern  → Search existing project approaches
DURING work:  Apply patterns, note gaps or new discoveries
AFTER work:   /reflexion    → Rate if get-pattern results helped (REQUIRED)
              /save-pattern → Store NEW reusable knowledge (if any)
```

## Two Memory Systems

| System | Purpose | Persistence | Use For |
|--------|---------|-------------|---------|
| **get-pattern/save-pattern/reflexion** | Application knowledge | Permanent (AgentDB) | Patterns, procedures, architecture |
| **claude-flow memory** | Swarm/session state | Transient | Coordination, task progress, working memory |

## AgentDB Skills

| Skill | When | What |
|-------|------|------|
| `get-pattern` | BEFORE work | Search existing patterns and approaches |
| `save-pattern` | AFTER discoveries | Store NEW reusable knowledge |
| `reflexion` | AFTER work | Record if patterns helped (required) |
| `learner` | Post-feature | Auto-discover patterns from episodes |

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

## Continuous Improvement Triggers

| Trigger | Worker | When |
|---------|--------|------|
| After major refactor | `optimize` | Performance optimization |
| After adding features | `testgaps` | Missing test coverage |
| After security changes | `audit` | Security analysis |
| After API changes | `document` | Documentation |
| Every 5+ file changes | `map` | Codebase mapping |
