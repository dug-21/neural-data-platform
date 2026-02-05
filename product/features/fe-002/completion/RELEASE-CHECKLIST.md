# FE-002: Domain Configuration Standardization - Release Checklist

> **Feature:** FE-002 Domain Configuration Standardization
> **Version:** 1.0
> **Created:** 2026-02-05
> **Purpose:** Release preparation checklist for FE-002 deployment

---

## Overview

This checklist ensures all release artifacts are properly created and the feature is ready for deployment. Follow this checklist after all acceptance criteria in [FE-002-DONE-DEFINITION.md](./FE-002-DONE-DEFINITION.md) are met.

---

## Version Determination

### Version Bump Analysis

| Change Type | Impact | Version Bump |
|-------------|--------|--------------|
| New CLI flag (`--domain`) | New feature | MINOR |
| File format change (YAML to JSON) | Internal, no API change | - |
| New validation capability | New feature | MINOR |
| Deploy workflow enhancement | New feature | - |

**Recommended Bump:** MINOR (e.g., 1.1.0 -> 1.2.0)

**Rationale:** FE-002 adds new functionality (domain validation in CLI and deploy workflow) without breaking existing behavior. The YAML to JSON migration is internal and produces identical outputs.

### Current Version Check

```bash
# Check current version
cat /var/ndp/deployed-version 2>/dev/null || echo "Not deployed"

# Check latest tag
git describe --tags --abbrev=0

# Check latest manifest
ls -la .deploy/releases/*.manifest.json | tail -1
```

**Current Version:** `______`
**New Version:** `______`

---

## Pre-Release Checklist

### Verification Complete

- [ ] All Phase A acceptance criteria passed
- [ ] All Phase B acceptance criteria passed
- [ ] Golden master comparison passed (CRITICAL)
- [ ] All tests passing
- [ ] Code review approved
- [ ] Documentation complete

**Verification Date:** `______`
**Verified By:** `______`

### Git State Clean

```bash
# Verify no uncommitted changes
git status --porcelain

# Verify on correct branch
git branch --show-current
```

- [ ] No uncommitted changes
- [ ] On main branch (or release branch)

---

## Release Artifact Checklist

### 1. Create Release Manifest

**Location:** `.deploy/releases/v{X.Y.Z}.manifest.json`

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.X.Y",
  "description": "Release v1.X.Y: Domain Configuration Standardization (FE-002)",
  "changes": [
    {
      "type": "config-format",
      "description": "Migrate domain config from YAML to JSON",
      "files": ["config/domains/indoor-air-quality/domain.json"]
    },
    {
      "type": "tool-update",
      "tool": "ndp-gold-ddl",
      "description": "Update loader to use JSON parser"
    },
    {
      "type": "tool-update",
      "tool": "ndp-validate",
      "description": "Add --domain flag for domain validation"
    },
    {
      "type": "workflow-update",
      "file": "deploy/pi/deploy.sh",
      "description": "Add domain validation to deployment workflow"
    },
    {
      "type": "dictionary",
      "action": "sync"
    }
  ],
  "github_issues": [
    {"number": 11, "title": "GAP-001: Domain config uses YAML instead of JSON"},
    {"number": 13, "title": "GAP-003: No JSON Schema validation for domain configs"}
  ]
}
```

- [ ] Manifest created at `.deploy/releases/v{X.Y.Z}.manifest.json`
- [ ] Manifest validates: `jq . .deploy/releases/v{X.Y.Z}.manifest.json`
- [ ] Version matches planned version

### 2. Update CHANGELOG.md

**Location:** `CHANGELOG.md`

Add entry at the top of the file:

```markdown
## [1.X.Y] - YYYY-MM-DD

### Added
- Domain validation in `ndp-validate` with `--domain` flag (FE-002)
- Two-layer validation for domain configs (Layer 1: Schema, Layer 2: Semantic)
- Domain validation step in deploy.sh workflow
- 30+ new tests for domain validation

### Changed
- Domain configuration format from YAML to JSON (ADR-016-001 compliance)
- ndp-gold-ddl loader updated to use serde_json

### Removed
- serde_yaml dependency from ndp-gold-ddl

### Fixed
- GAP-001: Domain config now uses JSON format (#11)
- GAP-003: Domain configs now have JSON Schema validation (#13)
```

- [ ] CHANGELOG entry added
- [ ] Date is correct
- [ ] Version matches manifest
- [ ] All changes documented

### 3. Create Git Tag

```bash
# Create annotated tag
git tag -a v{X.Y.Z} -m "Release v{X.Y.Z}: Domain Configuration Standardization (FE-002)

Changes:
- Migrate domain config from YAML to JSON (GAP-001)
- Add --domain validation flag (GAP-003)
- Add domain validation to deploy.sh

Closes: #11, #13"

# Verify tag
git show v{X.Y.Z}
```

- [ ] Tag created with `-a` flag (annotated)
- [ ] Tag message includes description
- [ ] Tag references GitHub issues

### 4. Commit Release Changes

```bash
# Stage release files
git add .deploy/releases/v{X.Y.Z}.manifest.json
git add CHANGELOG.md

# Commit
git commit -m "release: v{X.Y.Z} - Domain Configuration Standardization (FE-002)

- Migrate domain config from YAML to JSON
- Add --domain validation flag to ndp-validate
- Add domain validation to deploy.sh workflow
- Close GitHub issues #11, #13"
```

- [ ] Manifest committed
- [ ] CHANGELOG committed
- [ ] Commit message follows convention

---

## Documentation Updates

### Feature Documentation

- [ ] STATUS.md updated to "done"
- [ ] All completion documents created:
  - [ ] ACCEPTANCE-CRITERIA.md
  - [ ] VERIFICATION-PROCEDURE.md
  - [ ] FE-002-DONE-DEFINITION.md
  - [ ] RELEASE-CHECKLIST.md (this file)

### Architecture Documentation

- [ ] ADR-016-001 compliance verified (JSON source of truth)
- [ ] dp-019 compliance verified (two-layer validation)
- [ ] No new ADRs required (reuses existing patterns)

### Tool Documentation

- [ ] ndp-validate --help updated with `--domain` flag
- [ ] deploy.sh inline comments updated (if changed)

---

## GitHub Issue Updates

### Issue #11: GAP-001

**Update Comment:**
```
Resolved in v{X.Y.Z}

Changes:
- config/domains/indoor-air-quality/domain.yaml → domain.json
- tools/ndp-gold-ddl/src/config/loader.rs updated to use serde_json
- serde_yaml dependency removed

Verification:
- Golden master comparison: PASS (DDL unchanged)
- All tests passing: PASS

Closed by PR #{PR_NUMBER}
```

- [ ] Comment added to #11
- [ ] Issue closed
- [ ] Linked to PR

### Issue #13: GAP-003

**Update Comment:**
```
Resolved in v{X.Y.Z}

Changes:
- ndp-validate --domain flag added
- Layer 1 (JSON Schema) validation integrated
- Layer 2 (Semantic) validation wired to CLI
- deploy.sh validates domains in Phase 1

Verification:
- Layer 1 errors show JSONPath: PASS
- Layer 2 semantic validation runs: PASS
- 30+ new tests: PASS

Closed by PR #{PR_NUMBER}
```

- [ ] Comment added to #13
- [ ] Issue closed
- [ ] Linked to PR

---

## Push Checklist

### Push Code and Tag

```bash
# Push code
git push origin main

# Push tag
git push origin v{X.Y.Z}

# Verify remote
git ls-remote --tags origin | grep v{X.Y.Z}
```

- [ ] Code pushed
- [ ] Tag pushed
- [ ] Tag visible on remote

---

## Post-Release Verification

### Deployment Verification

```bash
# On target device
git pull
git describe --tags --exact-match  # Should show v{X.Y.Z}

# Deploy
./deploy.sh apply .deploy/releases/v{X.Y.Z}.manifest.json

# Verify
cat /var/ndp/deployed-version  # Should show v{X.Y.Z}
```

- [ ] Pull successful
- [ ] Tag matches
- [ ] Deployment successful
- [ ] Version file updated

### Smoke Tests

```bash
# Test domain validation
ndp-validate --domain config/domains/indoor-air-quality/domain.json

# Test DDL generation
ndp-gold-ddl generate --domain indoor-air-quality

# Test deploy dry-run
./deploy.sh apply --dry-run .deploy/releases/v{X.Y.Z}.manifest.json
```

- [ ] Validation works
- [ ] DDL generation works
- [ ] Deploy dry-run works

---

## Final Sign-Off

### Release Approved

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | | | |
| Reviewer | | | |
| Release Manager | | | |

### Release Summary

| Item | Value |
|------|-------|
| Version | v{X.Y.Z} |
| Release Date | YYYY-MM-DD |
| Manifest | `.deploy/releases/v{X.Y.Z}.manifest.json` |
| Git Tag | v{X.Y.Z} |
| GitHub Issues | #11 (closed), #13 (closed) |
| Feature | FE-002 Domain Configuration Standardization |

---

## Rollback Procedure

If issues are discovered after release:

### Quick Rollback

```bash
# On target device
# 1. Deploy previous version
./deploy.sh apply .deploy/releases/v{PREV}.manifest.json

# 2. Verify
cat /var/ndp/deployed-version
```

### Full Rollback (if needed)

```bash
# 1. Checkout previous tag
git checkout v{PREV}

# 2. Restore YAML config (from git history)
git show v{PREV}:config/domains/indoor-air-quality/domain.yaml > config/domains/indoor-air-quality/domain.yaml

# 3. Deploy
./deploy.sh apply .deploy/releases/v{PREV}.manifest.json
```

**Note:** Domain config format rollback requires restoring YAML file from git history.

---

## References

- [Release Policy](../../../../docs/procedures/RELEASE-POLICY.md) - Full versioning standard
- [Deployment Declaratives](../../../../docs/procedures/DEPLOYMENT-DECLARATIVES.md) - Manifest format
- [FE-002-DONE-DEFINITION.md](./FE-002-DONE-DEFINITION.md) - Definition of Done
- [VERIFICATION-PROCEDURE.md](./VERIFICATION-PROCEDURE.md) - Verification steps

---

*Release Checklist created: 2026-02-05 by ndp-scrum-master (SPARC Completion)*
