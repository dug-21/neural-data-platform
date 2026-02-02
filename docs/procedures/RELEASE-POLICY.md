# NDP Release Policy

This document defines the versioning standard and release methodology for the Neural Data Platform. All releases MUST follow these conventions to ensure consistency, traceability, and alignment with the declarative deployment system.

## Table of Contents

- [Semantic Versioning](#semantic-versioning)
  - [Version Format](#version-format)
  - [When to Bump MAJOR](#when-to-bump-major)
  - [When to Bump MINOR](#when-to-bump-minor)
  - [When to Bump PATCH](#when-to-bump-patch)
- [Release Artifacts](#release-artifacts)
  - [Manifest File](#manifest-file)
  - [Git Tag](#git-tag)
  - [Changelog Entry](#changelog-entry)
- [Release Checklist](#release-checklist)
- [Version Bump Examples](#version-bump-examples)
- [Release Workflow](#release-workflow)
- [Device State Tracking](#device-state-tracking)
- [Rollback Procedure](#rollback-procedure)
- [Troubleshooting](#troubleshooting)

---

## Semantic Versioning

NDP follows **Semantic Versioning 2.0.0** (https://semver.org/).

### Version Format

```
MAJOR.MINOR.PATCH

Examples:
  1.0.0   - Initial stable release
  1.2.0   - Added new feature
  1.2.3   - Bug fix release
  2.0.0   - Breaking change
```

### When to Bump MAJOR

Increment MAJOR when you make **incompatible changes** that require migration or break existing functionality.

| Change Type | Example | From | To |
|-------------|---------|------|-----|
| Schema breaking change | Remove `entity_schemas` (v1.1 to v2.0) | 1.x.x | 2.0.0 |
| API breaking change | Change `/api/v1/` endpoint contract | 1.x.x | 2.0.0 |
| Config format change | Require new mandatory field | 1.x.x | 2.0.0 |
| Database schema break | Drop column, change type incompatibly | 1.x.x | 2.0.0 |
| Protocol change | MQTT topic format change | 1.x.x | 2.0.0 |

**MAJOR releases require:**
- Migration script or procedure
- Updated documentation
- Changelog section explaining the breaking change
- Communication to users before deployment

### When to Bump MINOR

Increment MINOR when you add **backwards-compatible functionality**.

| Change Type | Example | From | To |
|-------------|---------|------|-----|
| New stream | Add `weather-station` stream | 1.0.0 | 1.1.0 |
| New Silver table | Add `silver.forecast_readings` | 1.1.0 | 1.2.0 |
| New API endpoint | Add `/api/v1/forecast` | 1.2.0 | 1.3.0 |
| New feature | Add hot-reload capability | 1.3.0 | 1.4.0 |
| New MCP tool | Add `create_stream` tool | 1.4.0 | 1.5.0 |
| New dimension table | Add `dim_sensor_calibration` | 1.5.0 | 1.6.0 |
| New config field (optional) | Add optional `metadata` field | 1.6.0 | 1.7.0 |

**MINOR releases:**
- Are backwards compatible
- Do not require migration
- Should include updated documentation

### When to Bump PATCH

Increment PATCH when you make **backwards-compatible bug fixes**.

| Change Type | Example | From | To |
|-------------|---------|------|-----|
| Bug fix | Fix null handling in ETL | 1.2.0 | 1.2.1 |
| Config correction | Fix typo in field mapping | 1.2.1 | 1.2.2 |
| Security patch | Update dependency version | 1.2.2 | 1.2.3 |
| Performance fix | Optimize query performance | 1.2.3 | 1.2.4 |
| Documentation fix | Correct procedure steps | 1.2.4 | 1.2.5 |
| DQ rule adjustment | Tune threshold value | 1.2.5 | 1.2.6 |

**PATCH releases:**
- Fix issues without adding features
- Are safe to deploy immediately
- Should reference the issue/bug being fixed

---

## Release Artifacts

Each release consists of three required artifacts:

### Manifest File

Location: `.deploy/releases/v{MAJOR}.{MINOR}.{PATCH}.manifest.json`

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.2.0",
  "description": "Release v1.2.0: Add weather-station stream with Silver table",
  "changes": [
    {"type": "stream", "id": "weather-station", "action": "create"},
    {"type": "silver-table", "stream_id": "weather-station", "action": "sync"},
    {"type": "dictionary", "action": "sync"}
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `$schema` | string | No | Schema reference for validation |
| `version` | string | **Yes** | Manifest schema version (currently `"1.0"`) |
| `release_version` | string | **Yes** | Semantic version of this release |
| `description` | string | **Yes** | Human-readable description |
| `changes` | array | **Yes** | Array of deployment declarations |

### Git Tag

Format: `v{MAJOR}.{MINOR}.{PATCH}`

```bash
# Create annotated tag (REQUIRED - annotated tags include metadata)
git tag -a v1.2.0 -m "Release v1.2.0: Add weather-station stream"

# Push tag
git push origin v1.2.0
```

**Tag requirements:**
- MUST be annotated (use `-a` flag)
- MUST match manifest `release_version`
- Tag message SHOULD match manifest `description`

### Changelog Entry

Location: `CHANGELOG.md`

```markdown
## [1.2.0] - 2026-02-02

### Added
- Weather station stream (`weather-station`)
- Silver table `silver.weather_readings` with hypertable

### Changed
- Updated data dictionary with weather fields

### Fixed
- N/A
```

**Changelog format:**
- Follow [Keep a Changelog](https://keepachangelog.com/) format
- Group changes by type: Added, Changed, Deprecated, Removed, Fixed, Security
- Include date in ISO format (YYYY-MM-DD)

---

## Release Checklist

Use this checklist for every release:

### Pre-Release

- [ ] All changes tested in integration environment
- [ ] Stream configs validated: `./tools/ndp-validate/ndp-validate.sh --all`
- [ ] DDL generation tested: `./deploy.sh apply --dry-run <manifest>`
- [ ] No uncommitted changes: `git status` is clean
- [ ] On correct branch (usually `main` or release branch)

### Create Release

- [ ] Determine version bump (MAJOR/MINOR/PATCH)
- [ ] Create manifest: `.deploy/releases/v{X}.{Y}.{Z}.manifest.json`
- [ ] Verify manifest: `cat .deploy/releases/vX.Y.Z.manifest.json | jq .`
- [ ] Update CHANGELOG.md with release notes
- [ ] Commit: `git commit -m "release: v{X}.{Y}.{Z}"`
- [ ] Create tag: `git tag -a v{X}.{Y}.{Z} -m "Release v{X}.{Y}.{Z}: {description}"`
- [ ] Push code: `git push`
- [ ] Push tag: `git push origin v{X}.{Y}.{Z}`

### Deploy Release

- [ ] On target device: `git pull`
- [ ] Verify tag: `git describe --tags --exact-match` shows `v{X}.{Y}.{Z}`
- [ ] Deploy: `./deploy.sh apply .deploy/releases/v{X}.{Y}.{Z}.manifest.json`
- [ ] Verify device state: `cat /var/ndp/deployed-version`
- [ ] Verify services: `./deploy.sh status`
- [ ] Smoke test: Verify data flow

### Post-Release

- [ ] Monitor logs for errors: `./deploy.sh logs`
- [ ] Verify Grafana dashboards (if applicable)
- [ ] Document any issues encountered

---

## Version Bump Examples

### Example 1: Add New Stream (MINOR)

**Scenario**: Adding a new `indoor-air-quality` stream.

**Current version**: `1.2.3`
**New version**: `1.3.0`

```bash
# 1. Create stream config
mkdir -p config/base/streams/indoor-air-quality
cat > config/base/streams/indoor-air-quality/config.json << 'EOF'
{
  "config_version": 2,
  "stream_id": "indoor-air-quality",
  ...
}
EOF

# 2. Create manifest
cat > .deploy/releases/v1.3.0.manifest.json << 'EOF'
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.3.0",
  "description": "Release v1.3.0: Add indoor-air-quality stream",
  "changes": [
    {"type": "stream", "id": "indoor-air-quality", "action": "create"},
    {"type": "silver-table", "stream_id": "indoor-air-quality", "action": "sync"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

# 3. Update CHANGELOG.md
# Add section for v1.3.0

# 4. Commit and tag
git add .
git commit -m "release: v1.3.0 - Add indoor-air-quality stream"
git tag -a v1.3.0 -m "Release v1.3.0: Add indoor-air-quality stream"
git push && git push origin v1.3.0
```

### Example 2: Bug Fix (PATCH)

**Scenario**: Fix incorrect field mapping in `air-quality` stream.

**Current version**: `1.3.0`
**New version**: `1.3.1`

```bash
# 1. Fix the config
vim config/base/streams/air-quality/config.json
# Correct the field mapping

# 2. Create manifest
cat > .deploy/releases/v1.3.1.manifest.json << 'EOF'
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.3.1",
  "description": "Release v1.3.1: Fix pm2_5 field mapping in air-quality stream",
  "changes": [
    {"type": "stream", "id": "air-quality", "action": "update"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

# 3. Update CHANGELOG.md
# Add section for v1.3.1 under "Fixed"

# 4. Commit and tag
git add .
git commit -m "fix: Correct pm2_5 field mapping in air-quality stream"
git tag -a v1.3.1 -m "Release v1.3.1: Fix pm2_5 field mapping"
git push && git push origin v1.3.1
```

### Example 3: Schema Migration (MAJOR)

**Scenario**: Remove deprecated `entity_schemas` field (v1.1 to v2.0 migration).

**Current version**: `1.3.1`
**New version**: `2.0.0`

```bash
# 1. Run migration script (if available)
./scripts/ndp-migrate-config.sh --from 1.1 --to 2 --dry-run
./scripts/ndp-migrate-config.sh --from 1.1 --to 2

# 2. Update all stream configs to v2.0 format
# (done by migration script)

# 3. Create manifest
cat > .deploy/releases/v2.0.0.manifest.json << 'EOF'
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "2.0.0",
  "description": "Release v2.0.0: Schema migration - remove entity_schemas",
  "changes": [
    {"type": "stream", "id": "air-quality", "action": "update"},
    {"type": "stream", "id": "outdoor-weather", "action": "update"},
    {"type": "stream", "id": "indoor-air-quality", "action": "update"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

# 4. Update CHANGELOG.md with BREAKING CHANGES section
# Add migration instructions

# 5. Commit and tag
git add .
git commit -m "release: v2.0.0 - Remove deprecated entity_schemas (BREAKING)"
git tag -a v2.0.0 -m "Release v2.0.0: Schema migration - remove entity_schemas"
git push && git push origin v2.0.0
```

### Example 4: Database Migration with App Rebuild (MINOR)

**Scenario**: Add new forecast accuracy table with app changes.

**Current version**: `2.0.0`
**New version**: `2.1.0`

```bash
# 1. Create migration file
cat > migrations/003-forecast-accuracy.sql << 'EOF'
CREATE TABLE IF NOT EXISTS silver.forecast_accuracy (
    time TIMESTAMPTZ NOT NULL,
    ...
);
SELECT create_hypertable('silver.forecast_accuracy', 'time', if_not_exists => TRUE);
EOF

# 2. Create manifest
cat > .deploy/releases/v2.1.0.manifest.json << 'EOF'
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "2.1.0",
  "description": "Release v2.1.0: Add forecast accuracy tracking",
  "changes": [
    {"type": "container", "target": "air-quality-app", "action": "build"},
    {"type": "migration", "file": "migrations/003-forecast-accuracy.sql"},
    {"type": "container", "target": "air-quality-app", "action": "restart"}
  ]
}
EOF

# 3. Update CHANGELOG.md

# 4. Commit and tag
git add .
git commit -m "release: v2.1.0 - Add forecast accuracy tracking"
git tag -a v2.1.0 -m "Release v2.1.0: Add forecast accuracy tracking"
git push && git push origin v2.1.0
```

---

## Release Workflow

### Standard Release Flow

```
Developer workstation                      Target device (Pi)
========================                   ====================

1. Make changes
   └── config, code, migrations

2. Create release manifest
   └── .deploy/releases/vX.Y.Z.manifest.json

3. Update CHANGELOG.md

4. Commit changes
   └── git commit -m "release: vX.Y.Z"

5. Create annotated tag
   └── git tag -a vX.Y.Z -m "..."

6. Push code and tag
   └── git push && git push origin vX.Y.Z

                                           7. Pull latest
                                              └── git pull

                                           8. Verify tag
                                              └── git describe --tags

                                           9. Deploy
                                              └── ./deploy.sh apply .deploy/releases/vX.Y.Z.manifest.json

                                           10. Verify
                                               └── cat /var/ndp/deployed-version
```

### Release Branch Strategy (Optional)

For larger releases requiring staged rollout:

```
main ─────────────────────────────────────────────────────────►
       \                                     /
        └─► release/2.0 ───► (testing) ────►
```

```bash
# Create release branch
git checkout -b release/2.0
# ... make changes, test ...

# Merge back to main
git checkout main
git merge release/2.0

# Tag on main
git tag -a v2.0.0 -m "Release v2.0.0"
git push && git push origin v2.0.0
```

---

## Device State Tracking

After deployment, the device records its state in `/var/ndp/`:

| File | Contents | Example |
|------|----------|---------|
| `/var/ndp/deployed-version` | Current version | `v1.2.0` |
| `/var/ndp/deployed-manifest` | Manifest path | `.deploy/releases/v1.2.0.manifest.json` |
| `/var/ndp/deployed-timestamp` | Deploy time | `2026-02-02T10:30:00Z` |

**Query device state:**

```bash
# Current version
cat /var/ndp/deployed-version

# Last manifest
cat /var/ndp/deployed-manifest

# Deploy timestamp
cat /var/ndp/deployed-timestamp

# All state
ls -la /var/ndp/
```

---

## Rollback Procedure

If a release causes issues, roll back to the previous version:

### Quick Rollback

```bash
# On target device

# 1. Identify previous version
ls .deploy/releases/  # Find previous manifest

# 2. Deploy previous version
./deploy.sh apply .deploy/releases/v1.2.0.manifest.json

# 3. Verify
cat /var/ndp/deployed-version  # Should show v1.2.0
```

### Full Rollback (with Git)

```bash
# On target device

# 1. Checkout previous tag
git checkout v1.2.0

# 2. Deploy
./deploy.sh apply .deploy/releases/v1.2.0.manifest.json

# 3. Verify
./deploy.sh status
cat /var/ndp/deployed-version
```

**Note**: Database migrations may not be automatically rolled back. For schema changes, you may need to apply a reverse migration manually.

---

## Troubleshooting

### Manifest Validation Fails

```
ERROR: Invalid release_version format
```
**Fix**: Ensure `release_version` follows `X.Y.Z` format (no `v` prefix in JSON).

```
ERROR: release_version (1.2.0) does not match manifest filename (v1.3.0)
```
**Fix**: Ensure manifest filename matches the `release_version` field.

### Tag Mismatch

```
WARNING: Git tag (v1.2.1) does not match manifest version (v1.2.0)
```
**Fix**: Create a new tag or update the manifest to match.

### Missing Artifacts

```
ERROR: Manifest not found: .deploy/releases/v1.2.0.manifest.json
```
**Fix**: Create the manifest file before tagging.

### Device State Not Updated

```bash
cat /var/ndp/deployed-version
# Shows old version
```
**Fix**: Check deploy.sh logs; ensure Phase 9 completed. Verify `/var/ndp/` directory exists and is writable.

---

## See Also

- [Declarative Deploy](DEPLOYMENT-DECLARATIVES.md) - Manifest format and declaration types
- [Pi Deployment](../../deploy/pi/README.md) - deploy.sh commands
- [Webhook Deployment Spec](WEBHOOK-DEPLOYMENT-SPEC.md) - Automated deployment (future)
- [dp-021 SCOPE](../../product/features/dp-021/SCOPE.md) - Feature documentation

---

*Document created: 2026-02-02*
*Feature: dp-021 Config Lifecycle & Release Management*
