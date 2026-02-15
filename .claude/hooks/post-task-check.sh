#!/bin/bash
# Post-task artifact existence check
# Called by settings.json PostToolUse Task hook after agent completion
# Reports missing artifacts but does NOT block (advisory)
#
# Usage: bash .claude/hooks/post-task-check.sh "$TASK_DESCRIPTION"

TASK_DESC="${1:-}"
TASK_DESC_LOWER=$(echo "$TASK_DESC" | tr '[:upper:]' '[:lower:]')

# Exit silently if no description
if [ -z "$TASK_DESC" ]; then
  exit 0
fi

MISSING=""

# ---- Planning task checks ----
if echo "$TASK_DESC_LOWER" | grep -qiE "planning|plan for|sparc planning|specification phase"; then
  # Extract feature ID from task description (e.g., ops-006, fe-004)
  FEATURE_ID=$(echo "$TASK_DESC" | grep -oP '[a-z]+-\d{3}' | head -1)

  if [ -n "$FEATURE_ID" ]; then
    FEATURE_DIR="product/features/$FEATURE_ID"

    [ ! -f "$FEATURE_DIR/IMPLEMENTATION-BRIEF.md" ] && MISSING="$MISSING\n  - $FEATURE_DIR/IMPLEMENTATION-BRIEF.md"
    [ ! -f "$FEATURE_DIR/ACCEPTANCE-MAP.md" ] && MISSING="$MISSING\n  - $FEATURE_DIR/ACCEPTANCE-MAP.md"
    [ ! -f "$FEATURE_DIR/LAUNCH-PROMPT.md" ] && MISSING="$MISSING\n  - $FEATURE_DIR/LAUNCH-PROMPT.md"
    [ ! -f "$FEATURE_DIR/ALIGNMENT-REPORT.md" ] && MISSING="$MISSING\n  - $FEATURE_DIR/ALIGNMENT-REPORT.md"
  fi
fi

# ---- Implementation task checks ----
if echo "$TASK_DESC_LOWER" | grep -qiE "implement|implementation|build the|code the|refactor"; then
  # Check if any files were modified recently (within last 5 minutes)
  RECENT_CHANGES=$(find . -name "*.rs" -newer /tmp/.ndp-task-start 2>/dev/null | head -1)

  # If no .rs files changed but task was about implementation, note it
  if [ -z "$RECENT_CHANGES" ] && echo "$TASK_DESC_LOWER" | grep -qiE "rust|crate|module|trait|struct"; then
    MISSING="$MISSING\n  - No .rs files modified (expected for implementation task)"
  fi
fi

# ---- Report ----
if [ -n "$MISSING" ]; then
  echo "[POST-TASK CHECK] Missing expected artifacts:"
  echo -e "$MISSING"
  echo ""
  echo "This is advisory. The task may have intentionally skipped these."
fi

exit 0
