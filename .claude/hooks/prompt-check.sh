#!/bin/bash
# UserPromptSubmit hook: detect swarm-qualifying tasks and enforce protocol
# Output is injected into model context as <user-prompt-submit-hook>

PROMPT="$1"
PROMPT_LOWER=$(echo "$PROMPT" | tr '[:upper:]' '[:lower:]')

# Swarm-qualifying keywords
SWARM_KEYWORDS="feature|implement|refactor|sparc|swarm|schema|etl|pipeline|generator|migration|new stream|new table|gold layer|silver layer|bronze layer|ops-|dp-|fe-|ml-|al-|db-|across|multiple files"

# Skip-swarm keywords (simple tasks, questions, git ops)
SKIP_KEYWORDS="typo|update comment|quick question|what is|how do|explain |read the|show me|^list |^commit|^push|^status|^review |^check |rename |single file|one line|^debug |^where is|^find |^search |^look at|^can you|^does |^tell me|^why does|^why is"

if echo "$PROMPT_LOWER" | grep -qiE "$SKIP_KEYWORDS"; then
  echo "[SIMPLE_TASK] No swarm needed."
  exit 0
fi

if echo "$PROMPT_LOWER" | grep -qiE "$SWARM_KEYWORDS"; then
  echo "================================================================"
  echo "SWARM REQUIRED — This task qualifies for swarm orchestration."
  echo ""
  echo "You MUST execute these steps in your FIRST message:"
  echo "  1. claude-flow swarm init --topology hierarchical --max-agents 8 --strategy specialized"
  echo "  2. claude-flow memory store --namespace {feature-id} --key context --value '{task summary}'"
  echo "  3. Spawn agents via Task tool with namespace in their prompts"
  echo ""
  echo "Do NOT skip swarm init. Do NOT go straight to implementation."
  echo "See .claude/rules/swarm-protocol.md for full protocol."
  echo "================================================================"
  exit 0
fi

# Default: let the route hook handle it
npx @claude-flow/cli@latest hooks route --task "$PROMPT" 2>/dev/null || true
