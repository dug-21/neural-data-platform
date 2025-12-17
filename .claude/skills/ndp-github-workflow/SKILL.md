---
name: "ndp-github-workflow"
description: "NDP-specific GitHub workflow conventions for branches, commits, PRs, and releases. Use this skill for ALL git operations in the Neural Data Platform project."
---

# NDP GitHub Workflow

## What This Skill Does

Defines the **project-specific** GitHub conventions for the Neural Data Platform. This skill supersedes generic GitHub skills for NDP work.

**Always use this skill for:**
- Creating branches
- Writing commit messages
- Creating pull requests
- Tagging releases

## Branch Strategy

### Branch Naming Convention

```
main                                    # Always deployable, protected
└── feature/{feature-id}               # Feature development
    └── feature/{feature-id}/bug-{nnn} # Bug fix during feature (optional)
```

### Rules

| Branch Type | Pattern | Example |
|-------------|---------|---------|
| Main | `main` | Protected, requires PR |
| Feature | `feature/{feature-id}` | `feature/air-006` |
| Feature Bug | `feature/{feature-id}/bug-{nnn}` | `feature/air-006/bug-001` |
| Hotfix | `hotfix/{issue-id}` | `hotfix/config-sync-fix` |
| Release | `release/v{major}.{minor}` | `release/v1.3` |

### Creating a Feature Branch

```bash
# From main, create feature branch
git checkout main
git pull origin main
git checkout -b feature/{feature-id}

# Example
git checkout -b feature/air-006
```

### Feature ID Format

Feature IDs follow `{phase}-{NNN}` pattern:

| Phase | Prefix | Example | Focus |
|-------|--------|---------|-------|
| Air Quality | `air` | `air-005` | Sensors, external data |
| Data Platform | `dp` | `dp-001` | Silver layer, ETL |
| Feature Engineering | `fe` | `fe-001` | ML features |
| Dashboards | `db` | `db-001` | Grafana |
| Predictions | `ml` | `ml-001` | ruv-FANN |
| Alerts | `al` | `al-001` | Triggers |

**Note**: The phase prefix changes as the project evolves. Check `product/features/` for current active phase.

### Feature ID Sources

- Match `product/features/{feature-id}/` directory
- Examples: `air-005`, `dp-001`, `fe-001`

---

## Commit Convention

### Format

```
{type}({scope}): {description}

{optional body}

{optional footer}
```

### Types

| Type | Use For |
|------|---------|
| `feat` | New feature or functionality |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code change that doesn't fix bug or add feature |
| `test` | Adding or updating tests |
| `chore` | Build, CI, dependencies, tooling |
| `perf` | Performance improvement |

### Scope

The scope identifies **what** is affected:

| Scope Type | Examples |
|------------|----------|
| Feature ID | `air-005`, `air-006` |
| Component | `storage`, `router`, `config` |
| Layer | `deploy`, `docker`, `ci` |

### Rules

1. **Type and scope are lowercase**
2. **Description is lowercase, no period at end**
3. **Description is imperative mood** ("add" not "added")
4. **Scope matches feature ID when working on a feature**
5. **Max 72 characters for first line**

### Examples

```bash
# Feature work (phase prefix varies by current phase)
feat(dp-001): add timescaledb continuous aggregates
fix(dp-001): correct timestamp parsing in etl pipeline
test(dp-001): add integration tests for silver layer

feat(air-005): implement http polling for weather api
fix(fe-001): correct window aggregation logic

# Component work (not feature-specific)
fix(storage): partition by stream_id instead of location_id
refactor(router): simplify validation logic
perf(parquet): reduce memory allocation in batch writes

# Infrastructure
chore(deploy): update docker-compose resource limits
docs(architecture): add silver layer design doc
chore(ci): add rust clippy check to workflow
```

### Multi-line Commits

For complex changes, add a body:

```bash
git commit -m "feat(air-006): implement config sync service

- Add ConfigSyncService to sync YAML to etcd on startup
- Support environment variable expansion in configs
- Add validation before sync

Closes: BUG-001"
```

---

## Pull Request Convention

### PR Title

Same format as commits:
```
{type}({scope}): {description}
```

### PR Template

Use this structure for PR descriptions:

```markdown
## Feature
{feature-id}: {feature title}

## Summary
{1-3 sentence description of changes}

## Changes
- {Change 1}
- {Change 2}
- {Change 3}

## Checklist
- [ ] SPARC documentation updated
- [ ] Tests passing (`cargo test`)
- [ ] Linting clean (`cargo clippy`)
- [ ] Documentation updated
- [ ] STATUS.md updated

## Testing
{How to test these changes}

## Related
- Scope: `product/features/{id}/SCOPE.md`
- Completion: `product/features/{id}/completion/COMPLETION.md`
- Bug: `product/features/{id}/bugs/BUG-{nnn}-*.md` (if applicable)
```

### PR Size Guidelines

| Size | Lines Changed | Guidance |
|------|---------------|----------|
| Small | < 200 | Ideal, easy to review |
| Medium | 200-500 | Acceptable for features |
| Large | 500+ | Consider splitting |

### Merge Strategy

- **Squash merge** for feature branches → main
- **Regular merge** for release branches
- **Delete branch** after merge

---

## Release Tagging

### Version Format

```
v{major}.{minor}.{patch}
```

- **Major**: Breaking changes, architectural shifts
- **Minor**: New features, AIR-NNN completions
- **Patch**: Bug fixes, small improvements

### Tagging Process

```bash
# Ensure on main with latest
git checkout main
git pull origin main

# Create annotated tag
git tag -a v1.3.0 -m "Release v1.3.0: AIR-005 External Data Integration

Features:
- HTTP polling source for external APIs
- OpenWeatherMap weather and air quality integration
- ConfigSyncService for GitOps configuration

Bug Fixes:
- BUG-001: Stream registry config sync gap resolved"

# Push tag
git push origin v1.3.0
```

### Tag Message Template

```
Release v{version}: {title}

Features:
- {feature 1}
- {feature 2}

Bug Fixes:
- {fix 1}
- {fix 2}

Breaking Changes:
- {breaking change, if any}
```

---

## Common Workflows

### Starting a New Feature

```bash
# 1. Determine phase prefix and next sequence number
#    Check product/features/ for current phase (air, dp, fe, db, ml, al)
#    Example: Starting first Data Platform feature → dp-001

# 2. Create feature directory
mkdir -p product/features/dp-001/{specification,pseudocode,architecture,refinement,completion,bugs,reports}

# 3. Create branch
git checkout main
git pull origin main
git checkout -b feature/dp-001

# 4. Initial commit
git add product/features/dp-001/
git commit -m "chore(dp-001): initialize feature directory structure"
git push -u origin feature/dp-001
```

### Fixing a Bug During Feature Development

```bash
# Option 1: Fix in feature branch (simple fix)
git add .
git commit -m "fix(air-006): resolve config sync race condition

Closes: BUG-002"

# Option 2: Separate bug branch (complex fix)
git checkout -b feature/air-006/bug-002
# ... make fixes ...
git commit -m "fix(air-006): resolve config sync race condition"
git checkout feature/air-006
git merge feature/air-006/bug-002
git branch -d feature/air-006/bug-002
```

### Completing a Feature

```bash
# 1. Ensure all tests pass
cargo test

# 2. Update STATUS.md to "done"
# 3. Create PR (example for dp-001: Silver Layer)
gh pr create --title "feat(dp-001): implement silver layer with TimescaleDB" \
  --body "## Feature
dp-001: Silver Layer Implementation

## Summary
Adds TimescaleDB integration for queryable time-series data with continuous aggregates.

## Changes
- TimescaleDB schema with hypertables
- ETL from Parquet to TimescaleDB
- Continuous aggregates for dashboards
- Grafana data source configuration

## Checklist
- [x] SPARC documentation updated
- [x] Tests passing
- [x] Documentation updated
- [x] STATUS.md updated

## Related
- Scope: product/features/dp-001/SCOPE.md
- Completion: product/features/dp-001/completion/COMPLETION.md"
```

---

## Quick Reference Card

### Phase Prefixes
```
air = Air Quality (foundation)    dp = Data Platform (Silver layer)
fe  = Feature Engineering         db = Dashboards
ml  = Predictions (ruv-FANN)      al = Alerts
```

### Branch Names
```
feature/dp-001           # Data Platform feature 1
feature/dp-001/bug-001   # Bug fix during dp-001
feature/fe-001           # Feature Engineering feature 1
hotfix/config-fix        # Hotfix (no phase)
release/v1.3             # Release branch
```

### Commit Prefixes
```
feat(scope):     # New feature
fix(scope):      # Bug fix
docs(scope):     # Documentation
test(scope):     # Tests
chore(scope):    # Maintenance
refactor(scope): # Code improvement
perf(scope):     # Performance
```

### Example Commits
```
feat(dp-001): add timescaledb schema migrations
fix(dp-001): correct continuous aggregate refresh policy
docs(dp-001): update architecture with silver layer design
test(fe-001): add feature extraction unit tests
chore(deploy): add timescaledb to docker-compose
```

---

## Enforcement

All NDP agents MUST use these conventions. When performing git operations:

1. **Check branch name** matches pattern before creating
2. **Validate commit message** format before committing
3. **Use PR template** when creating pull requests
4. **Reference feature docs** in PRs

If you see violations of these conventions, correct them or flag to the user.
