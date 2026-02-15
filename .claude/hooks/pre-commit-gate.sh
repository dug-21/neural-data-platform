#!/bin/bash
# Pre-commit quality gate hook
# Called by settings.json PreToolUse Bash hook when git commit is detected
# Exit 0 = allow, Exit 1 = block commit
#
# IMPORTANT: This script is called for ALL Bash commands. The settings.json
# hook filters for "git commit" before invoking this script. If somehow
# called for a non-commit command, exit 0 (pass-through).

set -euo pipefail

# ---- Check 1: cargo fmt --check ----
# Only check if there are staged .rs files
STAGED_RS=$(git diff --cached --name-only --diff-filter=ACM 2>/dev/null | grep '\.rs$' || true)

if [ -n "$STAGED_RS" ]; then
  FMT_OUTPUT=$(cargo fmt --check 2>&1 || true)
  if echo "$FMT_OUTPUT" | grep -q "Diff in"; then
    echo "BLOCKED: cargo fmt --check found formatting issues."
    echo "Run 'cargo fmt' to fix, then re-stage and commit."
    echo "$FMT_OUTPUT" | head -10
    exit 1
  fi
fi

# ---- Check 2: Anti-stub scan on staged .rs files ----
if [ -n "$STAGED_RS" ]; then
  # Exclude test files from the scan
  STUB_MATCHES=$(echo "$STAGED_RS" | xargs grep -n 'todo!()\|unimplemented!()' 2>/dev/null \
    | grep -v '_test\.rs\|test_\|#\[test\]\|#\[cfg(test)\]' || true)

  if [ -n "$STUB_MATCHES" ]; then
    echo "BLOCKED: Found todo!() or unimplemented!() in non-test code."
    echo "NDP rule: Never leave stubs. Ask the user if blocked."
    echo "$STUB_MATCHES" | head -10
    exit 1
  fi
fi

# ---- Check 3: Test regression warning (advisory, does not block) ----
BASELINE_FILE=".ndp/test-baseline.txt"
if [ -f "$BASELINE_FILE" ] && [ -n "$STAGED_RS" ]; then
  BASELINE=$(cat "$BASELINE_FILE" 2>/dev/null || echo "0")
  # Run a quick test count check (timeout after 120s to avoid blocking indefinitely)
  CURRENT=$(timeout 120 cargo test --workspace 2>&1 | grep "test result" | grep -oP '\d+ passed' | awk '{sum += $1} END {print sum+0}' 2>/dev/null || echo "0")

  if [ "$CURRENT" -lt "$BASELINE" ] 2>/dev/null; then
    echo "WARNING: Test count decreased ($CURRENT < $BASELINE baseline)."
    echo "This does not block the commit, but investigate before pushing."
  fi
fi

# All checks passed
exit 0
