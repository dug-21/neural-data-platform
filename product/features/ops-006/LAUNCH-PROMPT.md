# Implementation Launch Prompt: ops-006

## Proposed Prompt

> Implement ops-006: Validation Pipeline & Trust Infrastructure
>
> GitHub Issue: #{N}
> Brief: product/features/ops-006/IMPLEMENTATION-BRIEF.md
> Acceptance Map: product/features/ops-006/ACCEPTANCE-MAP.md
>
> Pattern IDs from planning: 25 (validate-plan), 26 (validate-impl), 27 (hooks), 28 (trust storage), 29 (trust-dashboard), 30 (shadow-judge), 31 (acceptance-map), 32 (launch-prompt), 33 (glass-box-report), 34 (test-baseline)
>
> Constraints:
> - No Rust code -- all changes are .md, .sh, .json, SKILL.md files
> - No external tools -- all checks via Claude Code hooks and skills
> - No new agent definitions -- tighten existing only
> - All validation checks are shell commands in skills, not compiled tests
> - Trust scores use existing AgentDB reflexion_store
>
> Wave structure: 3 waves
> - Wave 1 (WS1+WS2): Cleanup + definition tightening (13 tasks, delete stale files, fix agent defs, update protocols)
> - Wave 2 (WS3+WS4+WS5): Hook enforcement + validation skills (6 tasks, create hook scripts, validate-plan, upgrade validate-impl)
> - Wave 3 (WS6): Trust infrastructure (5 tasks, trust-dashboard, shadow-judge, trust recording)

## Reminders for User

- Review ALIGNMENT-REPORT.md -- all 7 principles PASS, no variances to approve
- Verify the 18 acceptance criteria in SCOPE.md match your expectations
- Edit the prompt above if scope has changed since planning
- Replace #{N} with the actual GitHub Issue number after creation
- This is NOT a typical implementation -- no Rust code, no cargo builds needed during implementation
- Implementation agents should be `researcher` or generic type, not `ndp-rust-dev`

## Gotchas Discovered During Planning

- **Nested Claude Code sessions are not possible.** The scrum-master cannot spawn sub-agents via `claude` CLI when running inside a Claude Code session. Implementation must either use the Task tool directly or have the coordinator produce artifacts itself.
- **Hook continueOnError semantics.** Setting `continueOnError: false` on a PreToolUse hook for Bash commands will block ALL matching Bash commands if the hook fails, not just git commit. The hook script must handle non-commit commands gracefully (pass through with exit 0).
- **Agent definition "should" vs "must" audit.** Some uses of "should" in agent definitions are intentional (suggestions, not requirements). The implementation agent must read each usage in context, not blindly replace all "should" with "must".
- **Prompt-check.sh skip keywords.** The skip keywords list includes "reflexion" and "save-pattern", which means pattern workflow commands get the SIMPLE_TASK response instead of any protocol hint. This may be intentional (pattern commands don't need swarm protocol) but should be verified.
- **verification-quality skill directory.** Contains a SKILL.md that documents non-existent CLI commands. Deleting this directory is safe -- no protocol or agent references it.
- **AgentDB pattern ID collision risk.** The new ADR pattern IDs (25-34) may collide with existing patterns if the database was re-initialized. Use `agentdb_pattern_search` with `task="adr:ops-006"` to verify IDs before referencing them.

## Key Deliverables Reference

| Artifact | Path |
|----------|------|
| Implementation Brief | product/features/ops-006/IMPLEMENTATION-BRIEF.md |
| Acceptance Map | product/features/ops-006/ACCEPTANCE-MAP.md |
| Architecture ADRs | product/features/ops-006/architecture/ARCHITECTURE.md |
| Adherence Audit | product/features/ops-006/specification/ADHERENCE-AUDIT.md |
| Specification | product/features/ops-006/specification/SPECIFICATION.md |
| Task Decomposition | product/features/ops-006/specification/TASK-DECOMPOSITION.md |
| Alignment Report | product/features/ops-006/ALIGNMENT-REPORT.md |
| Scope | product/features/ops-006/SCOPE.md |
| Research Reports | product/ndp-dev-auto/ (7 reports + PROPOSED-ACTIONS.md) |
