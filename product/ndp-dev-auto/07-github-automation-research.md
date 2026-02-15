# 07: GitHub Automation for Action Tracking Workflow

> Research Agent: researcher (github automation analysis)
> Date: 2026-02-15
> Scope: Batch issue creation, automated research flows, progress tracking, swarm-issue integration
> Files reviewed: 25+ skill/agent/command/template/issue files
> Context: Reports 01-06 in product/ndp-dev-auto/

---

## Executive Summary: Top 3 Options with Recommendation

**Option A: gh CLI Loop (Recommended for NDP)**
Create issues via a bash script that reads a structured markdown file and calls `gh issue create` in a loop. Each issue gets standard NDP labels and a templated body. Research is triggered manually per-issue or per-batch by the user invoking a swarm. Results are posted as issue comments via `gh issue comment`.

**Option B: Local-First with Deferred Sync**
Track all actions in a local markdown file with structured YAML frontmatter. A sync script periodically creates/updates GitHub Issues from the local file. Research happens locally; results are written to the file, then synced.

**Option C: Full-Auto Swarm Pipeline**
A master script creates all issues, then for each issue, spawns a research agent that reads the issue body, performs research, and posts findings as a comment. This is technically possible but impractical for NDP today due to context window limits, rate limiting, and the need for human review of research quality.

**Recommendation: Option A with elements of B.** Use a local structured markdown file as the source of truth for action definitions. Run a script to batch-create GitHub Issues from it. Trigger research swarms manually per-issue or in small batches (3-5 at a time). Post results as issue comments. This balances automation with human oversight and stays within the tools that actually work today.

---

## Tool Inventory

### What Exists and Works

| Tool | Status | How It Works |
|------|--------|-------------|
| `gh issue create` | VERIFIED WORKING | Creates issues with title, body, labels. Rate: 5000 API calls/hr (4977 remaining). |
| `gh issue list` | VERIFIED WORKING | Lists/filters issues by label, state, JSON output with jq. |
| `gh issue edit` | VERIFIED WORKING | Adds/removes labels, updates body, assigns milestones. |
| `gh issue comment` | VERIFIED WORKING | Posts comments to issues. Primary mechanism for posting research results. |
| `gh issue close` | VERIFIED WORKING | Closes issues with optional comment. |
| `gh label list` | VERIFIED WORKING | 31 labels exist. Relevant: `implementation`, `ops`, `fe`, `dp`, `P0-critical`, `P1-high`, `P2-normal`, `in-progress`, `blocked`, `needs-review`. |
| `gh project list` | AVAILABLE | GitHub Projects V2 is enabled on repo but no projects exist yet. |
| `gh project create` | AVAILABLE | Can create a project board via CLI. |
| `gh project item-add` | AVAILABLE | Can add issues to project boards. |
| `gh milestone create` | AVAILABLE | No milestones exist yet. Could create one for the research batch. |
| `mcp__claude-flow__github_issue_track` | LOADED | MCP tool: list/create/update/close/assign issues. Wrapper around GitHub API. |
| `mcp__claude-flow__github_repo_analyze` | LOADED | MCP tool: analyze repo structure. |
| `.github/ISSUE_TEMPLATE/ndp-implementation.yml` | EXISTS | Fields: feature-id, sparc-path, version, goal, acceptance, tasks. |
| `.github/ISSUE_TEMPLATE/ndp-bug.yml` | EXISTS | Fields: feature-id, version, severity, description, reproduction, root-cause. |
| `ndp-scrum-master` agent | EXISTS | Coordinates swarms, tracks via GH Issues, manages SPARC phases. |

### What Exists but is Aspirational

These tools are defined in skill/agent/command files but depend on `npx ruv-swarm` commands that do not appear to be installed or functional in the current NDP environment. They describe patterns, not working code.

| Tool | File | Status |
|------|------|--------|
| `swarm-issue` agent | `.claude/agents/github/swarm-issue.md` | PATTERN ONLY. References `npx ruv-swarm github issue-to-swarm`, `issue-decompose`, `issue-progress` -- none of which are installed. |
| `issue-tracker` agent | `.claude/agents/github/issue-tracker.md` | PATTERN ONLY. References `mcp__github__create_issue`, `mcp__github__search_issues` -- these MCP tools (from mcp-github server) are NOT available in current environment. |
| `project-board-sync` agent | `.claude/agents/github/project-board-sync.md` | PATTERN ONLY. References `npx ruv-swarm github board-init`, board-sync, etc. Not installed. |
| `github-project-management` skill | `.claude/skills/github-project-management/SKILL.md` | PATTERN ONLY. Comprehensive but depends on ruv-swarm. |
| `github-workflow-automation` skill | `.claude/skills/github-workflow-automation/SKILL.md` | PATTERN ONLY. GitHub Actions templates -- useful as reference but not directly executable. |

### What is Missing

| Gap | Impact |
|-----|--------|
| No `research-action` issue template | Would need to create one or use freeform body |
| No GitHub Project board | Need to create one for tracking overview |
| No milestones | Could create one for the research batch |
| No `research`, `action-item`, or `decision` labels | Need to create labels for this workflow |
| No batch issue creation script | Need to write one |
| No automated research-to-comment pipeline | Need to build manually using Task tool + gh CLI |

---

## Automation Options Matrix

### Option A: Semi-Automated gh CLI Loop

| Aspect | Detail |
|--------|--------|
| **Issue Creation** | Bash script reads structured markdown, calls `gh issue create` per item. ~30 sec per issue, ~15-20 min for 40 issues. |
| **Research Trigger** | User selects issues (by label filter), invokes a Claude Code session that reads each issue body and performs research. |
| **Result Recording** | Agent posts findings via `gh issue comment <N> --body "..."`. Updates label to `researched`. |
| **Progress Tracking** | `gh issue list --label research-action --json number,title,labels` gives instant status. Add `--jq` for filtering by status labels. |
| **Human Oversight** | Full. Human reviews each research comment before marking decision. |
| **Parallelism** | Can run 3-5 research agents concurrently (separate Task tool invocations). Rate limit is 5000 API calls/hr -- ~125 calls per issue is safe. |
| **Complexity** | Low. Uses only `gh` CLI which is verified working. |
| **Tradeoffs** | (+) Simple, reliable, auditable. (-) Manual trigger per batch. (-) No board view without setting up GitHub Projects. |

### Option B: Local-First Tracking File

| Aspect | Detail |
|--------|--------|
| **Issue Creation** | Deferred. Actions tracked in `product/ndp-dev-auto/PROPOSED-ACTIONS.md` with structured YAML per item. |
| **Research Trigger** | Agent reads the local file, performs research inline, writes results back to the file. |
| **Result Recording** | Written directly to the markdown file under each action. |
| **Progress Tracking** | Checkboxes in the file. `grep -c '\- \[x\]' PROPOSED-ACTIONS.md` for quick count. |
| **Human Oversight** | Full. File is in git, reviewed via diff. |
| **Parallelism** | Problematic -- multiple agents writing to the same file creates merge conflicts. |
| **Complexity** | Low initially, grows with sync complexity. |
| **Tradeoffs** | (+) No GH API dependency, works offline, git-native. (-) No web UI, no assignees, no comments, no external visibility. (-) Sync to GH Issues is a second step that needs its own script. |

### Option C: Full-Auto Research Pipeline

| Aspect | Detail |
|--------|--------|
| **Issue Creation** | Script creates all 40 issues in batch. |
| **Research Trigger** | Master orchestrator iterates through issues, spawns a research agent per issue. |
| **Result Recording** | Each agent reads issue body, performs research, posts results as comment, updates labels. |
| **Progress Tracking** | Fully automated via labels + optional GitHub Project board. |
| **Human Oversight** | Post-hoc review only. Risk of low-quality research going unreviewed. |
| **Parallelism** | High but constrained by context windows and API rate limits. Max ~5 concurrent agents realistically. |
| **Complexity** | High. Needs error handling for failed agents, duplicate comment prevention, progress state management. |
| **Tradeoffs** | (+) Maximum speed. (-) Quality control risk. (-) API rate limits may force throttling. (-) Context window limits mean each agent gets minimal context. (-) Hard to debug when research quality is poor. |

---

## Recommended Flow (Step by Step)

### Phase 1: Setup (One-Time, ~15 min)

1. **Create labels** for the workflow:
   ```
   research-action    (color: #D4C5F9) -- Marks a research action item
   researched         (color: #0E8A16) -- Research complete, awaiting decision
   decided            (color: #1D76DB) -- Decision made, ready to implement
   implementing       (color: #FBCA04) -- Implementation in progress
   action-done        (color: #0075CA) -- Action fully complete
   ```

2. **Create a milestone** (optional but recommended):
   ```
   Name: "NDP Dev Automation Research"
   Description: "30-40 action items from strategic analysis"
   ```

3. **Create a GitHub Project board** (optional, for visual tracking):
   ```
   gh project create --title "NDP Dev Automation Actions" --owner @me
   ```

### Phase 2: Batch Issue Creation (~20 min)

1. **Prepare a structured input file** -- `product/ndp-dev-auto/PROPOSED-ACTIONS.tsv` or structured markdown. Each row: `title | priority | category | context-summary`.

2. **Run the creation script** (example pattern):
   ```bash
   while IFS='|' read -r title priority category context; do
     gh issue create \
       --title "[Action] $title" \
       --label "research-action,$category,$priority" \
       --body "## Research Action Item

   **Category**: $category
   **Priority**: $priority

   ### Context
   $context

   ### Research Needed
   - [ ] Evaluate current state
   - [ ] Identify options
   - [ ] Recommend approach
   - [ ] Estimate effort

   ### Decision
   _Pending research_

   ### Implementation Notes
   _Pending decision_"
   done < PROPOSED-ACTIONS.tsv
   ```

3. **Add all issues to the milestone** (if created):
   ```bash
   gh issue list --label research-action --json number --jq '.[].number' | \
     while read num; do gh issue edit $num --milestone "NDP Dev Automation Research"; done
   ```

### Phase 3: Research Execution (Ongoing)

1. **Select a batch** of 3-5 issues to research:
   ```bash
   gh issue list --label research-action --label P1-high --json number,title --limit 5
   ```

2. **For each issue**, invoke a Claude Code session or Task tool agent:
   - Agent reads the issue body: `gh issue view <N> --json body`
   - Agent performs research (reads code, searches patterns, analyzes)
   - Agent posts findings: `gh issue comment <N> --body "## Research Findings\n..."`
   - Agent updates label: `gh issue edit <N> --add-label researched --remove-label research-action`

3. **Human reviews** the research comment, makes a decision, and posts:
   ```bash
   gh issue comment <N> --body "## Decision\n\nApproach: ...\nRationale: ...\nEffort: ..."
   gh issue edit <N> --add-label decided --remove-label researched
   ```

### Phase 4: Implementation Tracking

1. Issues with `decided` label are ready for implementation.
2. When implementation starts: `gh issue edit <N> --add-label implementing --remove-label decided`
3. When done: `gh issue edit <N> --add-label action-done --remove-label implementing` and close.

### Phase 5: Progress Review

At any time:
```bash
# Summary counts
echo "Pending research: $(gh issue list --label research-action --json number --jq length)"
echo "Researched:       $(gh issue list --label researched --json number --jq length)"
echo "Decided:          $(gh issue list --label decided --json number --jq length)"
echo "Implementing:     $(gh issue list --label implementing --json number --jq length)"
echo "Done:             $(gh issue list --label action-done --state closed --json number --jq length)"
```

---

## Swarm-Issue Agent Assessment

### What the `swarm-issue` Agent Describes

The agent definition at `.claude/agents/github/swarm-issue.md` describes:
- Converting GitHub Issues into swarm tasks via `npx ruv-swarm github issue-to-swarm`
- Auto-decomposing issues into subtasks
- Progress tracking via issue comments
- Label-based automation
- Issue comment commands (`/swarm analyze`, `/swarm decompose`, etc.)

### Reality Check

| Claimed Capability | Actually Available? |
|-------------------|-------------------|
| `npx ruv-swarm github issue-to-swarm` | NO. ruv-swarm is not installed in NDP environment. |
| `npx ruv-swarm github issue-decompose` | NO. |
| `npx ruv-swarm github issue-progress` | NO. |
| `mcp__github__create_issue` (from mcp-github server) | NO. The MCP GitHub server is not configured. |
| `mcp__claude-flow__github_issue_track` | YES. This is available from the claude-flow MCP server. Supports list/create/update/close/assign. |
| `gh issue create/view/edit/comment` (CLI) | YES. Fully functional. |
| GitHub Actions webhook for `/swarm` commands | NO. No such workflow exists in `.github/workflows/`. |

### Practical Assessment

The swarm-issue agent is a **design document**, not a working system. Its conceptual model -- issues as task containers, labels as state machines, comments as coordination channels -- is sound and matches the recommended flow above. But the implementation uses `gh` CLI directly, not the ruv-swarm wrappers.

The `mcp__claude-flow__github_issue_track` tool from the claude-flow MCP server is a working alternative for programmatic issue management. However, for batch operations, `gh` CLI in bash loops is more reliable and transparent.

**Verdict**: The swarm-issue conceptual model is useful. The recommended flow above implements the same pattern using tools that actually work (`gh` CLI). Do not depend on `npx ruv-swarm` commands.

---

## Local-First vs GitHub-First Analysis

### Local-First (Tracking file in repo)

**Pros**:
- Zero API calls needed for tracking
- Works offline
- Native git history for all changes
- Simple grep/awk for progress reports
- No label setup, no project board setup
- Can batch-convert to GH Issues later

**Cons**:
- No web UI for visual overview
- No comment threads for discussion
- Cannot assign issues to agents/people
- Merge conflicts if multiple agents edit simultaneously
- No notification integration
- Progress invisible to anyone not reading the file
- Extra work to sync to GH Issues later

### GitHub-First (Issues as source of truth)

**Pros**:
- Web UI for visual tracking and filtering
- Comment threads for research discussion
- Label-based state machine (research-action -> researched -> decided -> implementing -> done)
- Milestone for grouping
- GitHub Projects for board view
- Notifications when issues are updated
- Existing NDP convention (`ndp-scrum-master` already tracks via GH Issues)
- External visibility for collaborators

**Cons**:
- API calls for every operation (but 5000/hr is plenty for 40 issues)
- Need to set up labels, milestone, optional project board
- Slightly more ceremony than a text file
- Comment bodies can get long and noisy

### Recommendation

**GitHub-First is the clear winner for NDP.** Reasons:
1. NDP already uses GH Issues for implementation tracking (see `ndp-scrum-master.md`, issue templates)
2. The existing label set covers most needs (just add 4-5 new labels)
3. The `gh` CLI is fully authenticated and working with 5000 API calls/hr
4. Comment threads are the natural place for research findings
5. A local file can still be used as the INPUT for batch issue creation -- best of both worlds

---

## Implementation Sketch

### New Assets Needed

**1. Labels to create** (via `gh label create`):
```
research-action     #D4C5F9  "Research action item from strategic analysis"
researched          #0E8A16  "Research complete, awaiting decision"
decided             #1D76DB  "Decision made, awaiting implementation"
implementing        #FBCA04  "Implementation in progress"
action-done         #0075CA  "Action item complete"
```

**2. Issue template** (optional, for manual creation):
```yaml
# .github/ISSUE_TEMPLATE/ndp-research-action.yml
name: Research Action Item
description: Track a research/investigation action from strategic analysis
labels: ["research-action"]
body:
  - type: input
    id: category
    attributes:
      label: Category
      description: "e.g., observability, knowledge-mgmt, testing, dx, security"
    validations:
      required: true
  - type: dropdown
    id: priority
    attributes:
      label: Priority
      options:
        - P0-critical
        - P1-high
        - P2-normal
    validations:
      required: true
  - type: textarea
    id: context
    attributes:
      label: Context
      description: Background from strategic analysis
    validations:
      required: true
  - type: textarea
    id: research-questions
    attributes:
      label: Research Questions
      description: What needs to be investigated
    validations:
      required: true
  - type: textarea
    id: decision-criteria
    attributes:
      label: Decision Criteria
      description: How to evaluate options
```

**3. Batch creation script** (input: structured data, output: GH Issues):
```bash
#!/usr/bin/env bash
# tools/create-research-actions.sh
# Usage: ./create-research-actions.sh < product/ndp-dev-auto/PROPOSED-ACTIONS.md
# Or:    ./create-research-actions.sh product/ndp-dev-auto/PROPOSED-ACTIONS.tsv

set -euo pipefail

MILESTONE=""  # Set after milestone is created

while IFS=$'\t' read -r title priority category context questions; do
  [[ "$title" =~ ^#.*$ ]] && continue  # Skip comments
  [[ -z "$title" ]] && continue         # Skip blanks

  BODY="## Research Action Item

**Category**: ${category}
**Priority**: ${priority}
**Source**: product/ndp-dev-auto/06-strategic-recommendations.md

### Context
${context}

### Research Questions
${questions}

### Checklist
- [ ] Current state assessed
- [ ] Options identified
- [ ] Tradeoffs documented
- [ ] Recommendation made
- [ ] Effort estimated

### Decision
_Pending research_

### Implementation Notes
_Pending decision_"

  echo "Creating: $title"
  gh issue create \
    --title "[Action] ${title}" \
    --label "research-action,${category},${priority}" \
    --body "$BODY" \
    ${MILESTONE:+--milestone "$MILESTONE"}

  sleep 1  # Be nice to the API
done
```

**4. Research execution pattern** (for the human to invoke per-issue):

The user asks Claude Code: "Research issue #N" or processes a batch with:
```
For each issue labeled research-action with priority P1-high:
1. Read the issue body with gh issue view
2. Analyze the codebase for the relevant area
3. Search existing AgentDB patterns
4. Formulate findings and recommendation
5. Post as gh issue comment
6. Update label to researched
```

**5. Progress dashboard script**:
```bash
#!/usr/bin/env bash
# tools/research-progress.sh
echo "=== Research Action Progress ==="
echo "Pending:      $(gh issue list --label research-action --json number --jq length)"
echo "Researched:   $(gh issue list --label researched --json number --jq length)"
echo "Decided:      $(gh issue list --label decided --json number --jq length)"
echo "Implementing: $(gh issue list --label implementing --json number --jq length)"
echo "Done:         $(gh issue list --label action-done --state all --json number --jq length)"
echo ""
echo "=== By Priority ==="
echo "P0-critical:  $(gh issue list --label research-action --label P0-critical --json number --jq length)"
echo "P1-high:      $(gh issue list --label research-action --label P1-high --json number --jq length)"
echo "P2-normal:    $(gh issue list --label research-action --label P2-normal --json number --jq length)"
```

### Optional: GitHub Project Board

If visual tracking is desired:
```bash
# Create project
gh project create --title "NDP Dev Automation Actions" --owner @me

# Get project number
PROJECT_NUM=$(gh project list --owner @me --format json | jq -r '.projects[0].number')

# Add all research-action issues
gh issue list --label research-action --json number --jq '.[].number' | while read num; do
  gh project item-add $PROJECT_NUM --owner @me \
    --url "https://github.com/dug-21/neural-data-platform/issues/$num"
done
```

---

## Speed and Parallelism Considerations

### Rate Limits (Verified)

| Resource | Limit | Current Remaining |
|----------|-------|-------------------|
| Core API | 5000/hr | 4977 |
| Search API | 30/min | 30 |
| GraphQL API | 5000/hr | 4991 |

For 40 issues: creation uses ~40 API calls (well within limits). Each research cycle uses ~5-10 calls per issue (view, comment, edit). Total for full pipeline: ~400-600 calls. Safely within the 5000/hr limit.

### Parallelism Strategy

- **Issue creation**: Sequential with 1-second sleep between calls. Total: ~1 minute for 40 issues.
- **Research execution**: 3-5 issues in parallel via separate Task tool invocations. Each takes 2-5 minutes depending on complexity. A batch of 5 takes ~5 minutes.
- **Full pipeline**: 40 issues / 5 parallel = 8 batches * 5 min = ~40 minutes for all research.
- **Bottleneck**: Not API rate limits but context window depth. Each research agent needs enough context to produce useful findings.

### Context Window Management

Each research agent should receive:
1. The issue body (200-500 tokens)
2. Relevant code snippets from the codebase (500-2000 tokens)
3. Relevant AgentDB patterns (200-500 tokens)
4. Instructions for output format (200 tokens)

Total per-agent context: ~1000-3000 tokens input. This is well within limits.

---

## Summary of Findings

| Question | Answer |
|----------|--------|
| Fastest way to create 30-40 issues? | Bash script with `gh issue create` in a loop. ~1 minute total. |
| New issue template needed? | Recommended but not required. A `ndp-research-action.yml` template would help. |
| Automated research flow? | Feasible via Task tool agents reading issue bodies and posting comments. Manual trigger per batch recommended over full-auto. |
| Best progress tracking? | Labels as state machine + `gh issue list` for counts. Optional GitHub Project board for visual view. |
| Swarm-issue agent useful? | Conceptually yes, practically no -- depends on uninstalled ruv-swarm. Use `gh` CLI instead. |
| Local-first vs GitHub-first? | GitHub-first. Aligns with existing NDP conventions. Local file useful only as batch input. |
| Parallelism approach? | 3-5 concurrent research agents per batch. Rate limits are not the bottleneck. |

### What to Build (Ordered)

1. Create 5 new labels (5 `gh label create` commands)
2. Create a milestone (1 `gh milestone create` command)
3. Prepare structured input data from the action list
4. Write and run the batch creation script
5. (Optional) Create a GitHub Project board
6. (Optional) Create the `ndp-research-action.yml` issue template
7. Begin research in priority order, posting results as issue comments
