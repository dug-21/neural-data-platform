# Research Protocol

Phase 1 protocol. Used by the primary agent or Design Leader to coordinate research and scope definition.

Triggers on: new feature, research, explore, scope, investigate.

---

## Execution Model

Phase 1 is **interactive**, not fire-and-forget. The research agent and human collaborate in conversation — exploring the problem space, discussing findings, and converging on scope. This is fundamentally different from the design and delivery protocols where agents run autonomously.

```
Human                           Primary Agent / Design Leader     Research Agent
─────                           ─────────────────────────────     ──────────────
"I want to build {intent}"
                                get-pattern
                                spawn researcher ──────────────►  explore problem space
                                                                  search codebase + patterns
                                                                  analyze constraints
                                ◄──────────────────────────────── return findings + scope proposal
                                present findings to human
"adjust X, remove Y"
                                relay feedback to researcher ───► refine scope
                                ◄──────────────────────────────── return revised proposal
                                present revised scope
"approved"
                                researcher writes SCOPE.md ─────► SCOPE.md written
                                ◄──────────────────────────────── confirm
                                human reviews SCOPE.md
                                proceed to Phase 2 (design)
```

---

## When to Use This Protocol

Use when:
- Starting a new feature from a high-level idea
- The problem space is unclear and needs exploration
- Technology choices need investigation
- Existing codebase patterns need analysis before scoping

Skip when:
- SCOPE.md already exists (proceed to design protocol)
- The scope is well-understood and the human can write SCOPE.md directly
- Bug fixes or small changes that don't need research

---

## Flow

### Step 1: Human Provides Intent

The human describes what they want at a high level. This can be as vague as "I want predictive alerts" or as specific as "add a new MQTT source for weather data."

### Step 2: Spawn Research Agent

```
Task(
  subagent_type: "ndp-researcher",
  prompt: "Research the problem space for a potential new feature.

    Human's intent: {intent description}
    Feature area: {phase prefix if known — dp, fe, ml, etc.}

    Explore:
    1. Existing codebase — what's already built that relates to this?
    2. AgentDB patterns — what architectural decisions constrain this?
    3. Technical landscape — what approaches exist for this problem?
    4. Project constraints — ARM64/Pi, memory budget, config-driven, no banned deps
    5. Dependencies — what would this feature depend on? What depends on it?

    Return:
    - Research findings (organized by area)
    - Proposed scope boundaries (what's in, what's out)
    - Key risks or unknowns
    - Recommended phase prefix and feature number
    - Open questions for the human"
)
```

### Step 3: Present and Iterate

Present the research findings to the human. The human may:
- Accept the proposed scope
- Adjust boundaries (add/remove items)
- Ask for deeper investigation in specific areas
- Challenge assumptions

If the human requests changes, relay them to the research agent (spawn again with updated context) or discuss directly.

### Step 4: Write SCOPE.md

When human and agent converge on scope, the research agent writes SCOPE.md:

```
product/features/{phase}-{NNN}/SCOPE.md
```

SCOPE.md follows the project's existing format:
- Feature ID and title
- Objective (2-3 sentences)
- Background / motivation
- Acceptance criteria (numbered, testable)
- Constraints
- NOT in scope
- Dependencies
- Version target

### Step 5: Human Approves

The human reviews SCOPE.md. If approved, Phase 1 is complete and the workflow proceeds to Phase 2 (design protocol).

If the human requests changes, the agent revises SCOPE.md until approved.

---

## Research Agent Behavior

The ndp-researcher agent:

**Does**:
- Search AgentDB for relevant patterns (`/get-pattern`)
- Read existing codebase files to understand current state
- Analyze project constraints and dependencies
- Identify risks and unknowns
- Propose scope boundaries with rationale
- Write SCOPE.md when scope is agreed
- Record learning (`/reflexion`, `/save-pattern`)

**Does NOT**:
- Make architecture decisions (that's Phase 2, ndp-architect)
- Write specifications (that's Phase 2, ndp-specification)
- Write code or pseudocode
- Modify any files outside `product/features/{feature-id}/`
- Proceed to Phase 2 without human approval of SCOPE.md

---

## Pattern Workflow

Before research: `/get-pattern` with the feature's problem domain.
After research: `/reflexion` for each pattern retrieved.

If the research reveals that an existing pattern is outdated or wrong, record it immediately with reward 0.0.

---

## Output

Phase 1 produces one artifact:

| Artifact | Path | Author |
|----------|------|--------|
| SCOPE.md | `product/features/{phase}-{NNN}/SCOPE.md` | ndp-researcher (human-approved) |

When SCOPE.md is approved, proceed to the design protocol (Session 1 continues with Phase 2).
