# Pre-Commit Hooks Documentation - air-001

This document outlines the pre-commit checks that should be run before committing code for the air-001 feature.

## Overview

Pre-commit hooks ensure code quality and consistency before changes are committed to version control. These checks help catch issues early and maintain high standards across the codebase.

## Required Checks

### 1. Code Formatting (cargo fmt)

**Command:**
```bash
cargo fmt --check
```

**Purpose:** Ensures all Rust code follows the standard formatting conventions.

**Fix Command:**
```bash
cargo fmt
```

**Configuration:** Uses default `rustfmt.toml` or workspace settings.

---

### 2. Linting (cargo clippy)

**Command:**
```bash
cargo clippy --workspace --all-features --all-targets -- -D warnings
```

**Purpose:** Catches common mistakes, anti-patterns, and style issues.

**Failure Conditions:**
- Any clippy warnings or errors
- Code that doesn't follow Rust best practices

**Fix:** Address all clippy suggestions before committing.

---

### 3. Unit Tests (cargo test)

**Command:**
```bash
cargo test --workspace --all-features
```

**Purpose:** Ensures all existing tests pass with the new changes.

**Failure Conditions:**
- Any failing tests
- Compilation errors in test code

**Fix:** Update implementation or tests to ensure all tests pass.

---

### 4. TODO/FIXME Check

**Command:**
```bash
git diff --cached --name-only | xargs grep -n "TODO\|FIXME"
```

**Purpose:** Prevents committing code with unresolved TODO or FIXME comments.

**Allowed Exceptions:**
- TODOs in documentation files (`.md`)
- TODOs in issue tracking comments with issue numbers

**Fix:** Complete or remove TODOs, or convert them to tracked issues.

---

## Optional Checks (Recommended)

### 5. Security Audit

**Command:**
```bash
cargo audit
```

**Purpose:** Checks for known security vulnerabilities in dependencies.

**Action:** Review and update vulnerable dependencies before committing.

---

### 6. Documentation Check

**Command:**
```bash
cargo doc --workspace --all-features --no-deps
```

**Purpose:** Ensures documentation compiles without errors.

**Action:** Fix any broken documentation links or malformed doc comments.

---

### 7. Code Coverage Threshold

**Command:**
```bash
cargo llvm-cov --workspace --all-features --summary-only
```

**Purpose:** Ensures test coverage meets minimum threshold (85%+ for air-001).

**Action:** Add tests for uncovered code paths.

---

## Setting Up Pre-Commit Hooks

### Option 1: Manual Git Hook

Create `.git/hooks/pre-commit` (make executable with `chmod +x`):

```bash
#!/bin/bash

echo "Running pre-commit checks for air-001..."

# Format check
echo "1/4 Checking code formatting..."
if ! cargo fmt --check; then
    echo "❌ Code formatting check failed. Run 'cargo fmt' to fix."
    exit 1
fi

# Clippy
echo "2/4 Running clippy..."
if ! cargo clippy --workspace --all-features --all-targets -- -D warnings; then
    echo "❌ Clippy check failed. Fix all warnings before committing."
    exit 1
fi

# Tests
echo "3/4 Running tests..."
if ! cargo test --workspace --all-features --quiet; then
    echo "❌ Tests failed. Fix failing tests before committing."
    exit 1
fi

# TODO/FIXME check
echo "4/4 Checking for TODOs/FIXMEs..."
if git diff --cached --name-only | grep -E '\.rs$' | xargs grep -n "TODO\|FIXME" 2>/dev/null; then
    echo "❌ Found TODO or FIXME in staged Rust files. Please resolve before committing."
    exit 1
fi

echo "✅ All pre-commit checks passed!"
exit 0
```

### Option 2: Using pre-commit Framework

Install [pre-commit](https://pre-commit.com/):

```bash
pip install pre-commit
```

Create `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: local
    hooks:
      - id: cargo-fmt
        name: cargo fmt
        entry: cargo fmt
        args: ['--check']
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-clippy
        name: cargo clippy
        entry: cargo clippy
        args: ['--workspace', '--all-features', '--all-targets', '--', '-D', 'warnings']
        language: system
        types: [rust]
        pass_filenames: false

      - id: cargo-test
        name: cargo test
        entry: cargo test
        args: ['--workspace', '--all-features']
        language: system
        types: [rust]
        pass_filenames: false

      - id: check-todos
        name: check for TODOs
        entry: bash -c 'if grep -r "TODO\|FIXME" --include="*.rs" .; then exit 1; fi'
        language: system
        types: [rust]
        pass_filenames: false
```

Install hooks:
```bash
pre-commit install
```

---

## Bypassing Hooks (Use Sparingly)

In rare cases where you need to bypass hooks:

```bash
git commit --no-verify -m "your message"
```

**⚠️ Warning:** Only use `--no-verify` when absolutely necessary and with team approval.

---

## CI/CD Integration

The same checks run automatically in the GitHub Actions CI/CD pipeline defined in `.github/workflows/air-001-ci.yml`.

**Important:** Even if you bypass local hooks, the CI pipeline will still enforce these checks.

---

## Performance Optimization

For faster local development, you can run checks only on changed files:

```bash
# Format only staged files
git diff --cached --name-only | grep '\.rs$' | xargs rustfmt --check

# Run clippy only on changed workspace members
cargo clippy --package <changed-package>
```

---

## Troubleshooting

### "cargo fmt --check" fails but code looks correct
- Ensure you're using the same rustfmt version as CI
- Run `cargo fmt` to auto-fix formatting

### Clippy warnings in generated code
- Add `#[allow(clippy::...)]` attributes with justification comments
- Update `.clippy.toml` if needed for project-wide exceptions

### Tests timeout in pre-commit hook
- Consider running only unit tests locally: `cargo test --lib`
- Let CI handle longer integration tests

### TODO check blocks legitimate comments
- Convert TODOs to tracked GitHub issues
- Reference issue numbers in comments instead: `// See issue #123`

---

## Contact

For questions or issues with pre-commit hooks, contact the air-001 feature team or create an issue in the repository.

---

Last Updated: 2025-12-13
