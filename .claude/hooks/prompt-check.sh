#!/bin/bash
# UserPromptSubmit hook: detect planning vs implementation, enforce protocol
# Output is injected into model context as <user-prompt-submit-hook>

PROMPT="$1"
PROMPT_LOWER=$(echo "$PROMPT" | tr '[:upper:]' '[:lower:]')

# Always inject current version (keeps versioning OUT of CLAUDE.md)
CURRENT_VERSION=$(git describe --tags --abbrev=0 2>/dev/null || echo "unknown")

# Simple tasks — no protocol needed
SKIP_KEYWORDS="typo|update comment|quick question|what is|how do|explain |read the|show me|^list |^commit|^push|^status|^review |^check |rename |single file|one line|^debug |^where is|^find |^search |^look at|^can you|^does |^tell me|^why does|^why is|insights|reflexion|save-pattern|get-pattern|learner|align"

if echo "$PROMPT_LOWER" | grep -qiE "$SKIP_KEYWORDS"; then
  echo "[SIMPLE_TASK] No swarm needed. Version: $CURRENT_VERSION"
  exit 0
fi

# Planning keywords
PLAN_KEYWORDS="sparc planning|specification phase|pseudocode phase|architecture phase|design |research |scope |roadmap|sparc s |sparc p |sparc a |phase-a|phase-b|plan for|planning swarm"

# Implementation keywords
IMPL_KEYWORDS="implement|tdd|build the|code the|fix |refactor|migrate|sparc r |sparc c |refinement phase|completion phase|phase-r|phase-c|implementation swarm"

# General swarm keywords (ask user to clarify)
SWARM_KEYWORDS="feature|schema|etl|pipeline|generator|migration|new stream|new table|gold layer|silver layer|bronze layer|ops-|dp-|fe-|ml-|al-|db-|across|multiple files"

if echo "$PROMPT_LOWER" | grep -qiE "$PLAN_KEYWORDS"; then
  echo "================================================================"
  echo "PLANNING SWARM — Version: $CURRENT_VERSION"
  echo "Protocol: .claude/rules/planning-protocol.md"
  echo ""
  echo "MANDATORY steps:"
  echo "  1. /get-pattern — search AgentDB for relevant patterns"
  echo "  2. claude-flow swarm init --topology hierarchical --max-agents 8"
  echo "  3. claude-flow memory store --namespace {feature-id}"
  echo "  4. Spawn planning agents (ndp-architect, specification, pseudocode)"
  echo "  5. Spawn ndp-vision-guardian — produces ALIGNMENT-REPORT.md"
  echo "     Vision criteria: product/vision/ALIGNMENT-CRITERIA.md"
  echo "  6. Generate IMPLEMENTATION-BRIEF.md"
  echo ""
  echo "PLANNING RULES:"
  echo "  - Output to product/features/{id}/{phase}/ ONLY"
  echo "  - NO code changes. NO implementation agents."
  echo "  - Present alignment variances to user before proceeding"
  echo ""
  echo "AFTER: /reflexion + /save-pattern"
  echo "================================================================"
  exit 0
fi

if echo "$PROMPT_LOWER" | grep -qiE "$IMPL_KEYWORDS"; then
  echo "================================================================"
  echo "IMPLEMENTATION SWARM — Version: $CURRENT_VERSION"
  echo "Protocol: .claude/rules/implementation-protocol.md"
  echo ""
  echo "MANDATORY steps:"
  echo "  1. /get-pattern — search AgentDB for relevant patterns"
  echo "  2. claude-flow swarm init --topology hierarchical --max-agents 8"
  echo "  3. claude-flow memory store --namespace {feature-id}"
  echo "  4. Read IMPLEMENTATION BRIEF (not full spec tree)"
  echo "  5. Spawn implementation agents (ndp-rust-dev, ndp-tester)"
  echo "  6. /validate before presenting results"
  echo ""
  echo "IMPLEMENTATION RULES:"
  echo "  - Agents read brief + specific source files only"
  echo "  - cargo test --workspace before reporting"
  echo "  - /validate: Tier 1 (unit) always, Tier 3 (integration) for qualifying changes"
  echo "  - Max 2 validation fix iterations"
  echo "  - Track progress via GitHub Issue"
  echo "  - Truncate cargo output: first error + summary"
  echo ""
  echo "AFTER: /reflexion + /save-pattern"
  echo "================================================================"
  exit 0
fi

if echo "$PROMPT_LOWER" | grep -qiE "$SWARM_KEYWORDS"; then
  echo "================================================================"
  echo "SWARM DETECTED — Version: $CURRENT_VERSION"
  echo "Determine if this is PLANNING or IMPLEMENTATION, then follow"
  echo "the appropriate protocol in .claude/rules/"
  echo "  Planning:       .claude/rules/planning-protocol.md"
  echo "  Implementation: .claude/rules/implementation-protocol.md"
  echo ""
  echo "MANDATORY: /get-pattern first. /reflexion + /save-pattern after."
  echo "================================================================"
  exit 0
fi

# Medium-complexity: pattern workflow still required
echo "[TASK] Version: $CURRENT_VERSION. Run /get-pattern before work. /reflexion when done."
