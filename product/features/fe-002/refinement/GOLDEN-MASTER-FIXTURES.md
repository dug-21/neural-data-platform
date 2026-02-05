# FE-002: Golden Master Fixtures Reference

> **Feature ID:** FE-002
> **Version:** 1.0
> **Created:** 2026-02-05
> **Purpose:** Document what fixtures to capture, where to store them, and how to manage baselines

---

## 1. Overview

Golden Master fixtures are the source of truth for FE-002 testing. They capture the exact DDL output of `ndp-gold-ddl` BEFORE any changes, serving as the acceptance criteria for the migration.

**Critical Rule:** Fixtures MUST be captured and committed BEFORE any Phase A code changes begin.

---

## 2. Fixtures to Capture

### 2.1 Domain-Level Outputs

| Fixture Name | Command | Description |
|--------------|---------|-------------|
| `domain_indoor-air-quality_sync.sql` | `ndp-gold-ddl generate --domain indoor-air-quality --action sync` | Aligned view with existence check |
| `domain_indoor-air-quality_recreate.sql` | `ndp-gold-ddl generate --domain indoor-air-quality --action recreate` | Aligned view with DROP CASCADE |

### 2.2 Stream-Level Outputs

| Fixture Name | Command | Description |
|--------------|---------|-------------|
| `stream_air-quality_sync.sql` | `ndp-gold-ddl generate --stream air-quality --action sync` | Continuous aggregate + refresh policy |
| `stream_air-quality_recreate.sql` | `ndp-gold-ddl generate --stream air-quality --action recreate` | With DROP before CREATE |
| `stream_outdoor-weather_sync.sql` | `ndp-gold-ddl generate --stream outdoor-weather --action sync` | Weather stream CA |
| `stream_outdoor-weather_recreate.sql` | `ndp-gold-ddl generate --stream outdoor-weather --action recreate` | Weather stream CA |
| `stream_home-assistant-state_sync.sql` | `ndp-gold-ddl generate --stream home-assistant-state --action sync` | State event CA |
| `stream_home-assistant-state_recreate.sql` | `ndp-gold-ddl generate --stream home-assistant-state --action recreate` | State event CA |
| `stream_outdoor-air-quality_sync.sql` | `ndp-gold-ddl generate --stream outdoor-air-quality --action sync` | Outdoor AQI CA |
| `stream_outdoor-air-quality_recreate.sql` | `ndp-gold-ddl generate --stream outdoor-air-quality --action recreate` | Outdoor AQI CA |

### 2.3 State Transitions Outputs

| Fixture Name | Command | Description |
|--------------|---------|-------------|
| `stream_home-assistant-state_transitions_sync.sql` | `ndp-gold-ddl generate --stream home-assistant-state --transitions --action sync` | Transitions view |
| `stream_home-assistant-state_transitions_recreate.sql` | `ndp-gold-ddl generate --stream home-assistant-state --transitions --action recreate` | Transitions view |

### 2.4 Integrity Manifest

| Fixture Name | Content | Description |
|--------------|---------|-------------|
| `CHECKSUMS.sha256` | SHA-256 hashes of all .sql files | Integrity verification |
| `CAPTURE_INFO.txt` | Timestamp, git SHA, command versions | Provenance tracking |

---

## 3. Fixture Storage Location

### 3.1 Directory Structure

```
tools/ndp-gold-ddl/
├── tests/
│   └── fixtures/
│       └── golden-master/
│           ├── CHECKSUMS.sha256           # Integrity manifest
│           ├── CAPTURE_INFO.txt           # Capture metadata
│           ├── domain_indoor-air-quality_sync.sql
│           ├── domain_indoor-air-quality_recreate.sql
│           ├── stream_air-quality_sync.sql
│           ├── stream_air-quality_recreate.sql
│           ├── stream_outdoor-weather_sync.sql
│           ├── stream_outdoor-weather_recreate.sql
│           ├── stream_home-assistant-state_sync.sql
│           ├── stream_home-assistant-state_recreate.sql
│           ├── stream_outdoor-air-quality_sync.sql
│           ├── stream_outdoor-air-quality_recreate.sql
│           ├── stream_home-assistant-state_transitions_sync.sql
│           └── stream_home-assistant-state_transitions_recreate.sql
```

### 3.2 Why This Location

- **Close to tests**: Tests in `tests/golden_master_test.rs` can easily reference fixtures
- **In-crate**: Part of `ndp-gold-ddl` crate, versioned together
- **CI accessible**: No special setup needed in CI/CD
- **Gitignore-safe**: Not in a commonly ignored directory

---

## 4. Capture Procedure

### 4.1 Pre-Capture Checklist

Before running the capture script:

- [ ] Working directory is repository root
- [ ] `main` branch is checked out (or target branch)
- [ ] No uncommitted changes in config/ or tools/ndp-gold-ddl/
- [ ] `cargo build -p ndp-gold-ddl` succeeds
- [ ] Domain config exists: `config/domains/indoor-air-quality/domain.yaml`
- [ ] Stream configs exist for all referenced streams

### 4.2 Capture Script

```bash
#!/bin/bash
# Location: scripts/capture-golden-master.sh
# Run from repository root

set -euo pipefail

# Configuration
FIXTURES_DIR="tools/ndp-gold-ddl/tests/fixtures/golden-master"
CONFIG_DIR="./config"
TOOL="cargo run -p ndp-gold-ddl --release --quiet --"

# Ensure clean fixtures directory
rm -rf "$FIXTURES_DIR"
mkdir -p "$FIXTURES_DIR"

echo "========================================"
echo "GOLDEN MASTER BASELINE CAPTURE"
echo "========================================"
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Git Branch: $(git rev-parse --abbrev-ref HEAD)"
echo "Git SHA: $(git rev-parse HEAD)"
echo "Working Directory: $(pwd)"
echo ""

# Create capture info file
cat > "$FIXTURES_DIR/CAPTURE_INFO.txt" << EOF
Golden Master Baseline Capture
==============================
Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)
Git Branch: $(git rev-parse --abbrev-ref HEAD)
Git SHA: $(git rev-parse HEAD)
Git Status: $(git status --porcelain | wc -l) uncommitted changes
Rust Version: $(rustc --version)
Cargo Version: $(cargo --version)
Host: $(uname -a)

Purpose: Capture DDL output from ndp-gold-ddl BEFORE FE-002 Phase A migration.
         These fixtures serve as the acceptance criteria for YAML->JSON migration.

Captured By: $(whoami)
EOF

# =====================================
# DOMAIN OUTPUTS
# =====================================
echo "Capturing domain outputs..."

echo "  - indoor-air-quality (sync)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --domain indoor-air-quality --action sync \
    > "$FIXTURES_DIR/domain_indoor-air-quality_sync.sql"

echo "  - indoor-air-quality (recreate)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --domain indoor-air-quality --action recreate \
    > "$FIXTURES_DIR/domain_indoor-air-quality_recreate.sql"

# =====================================
# STREAM OUTPUTS
# =====================================
echo "Capturing stream outputs..."

# Air Quality
echo "  - air-quality (sync)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream air-quality --action sync \
    > "$FIXTURES_DIR/stream_air-quality_sync.sql" 2>/dev/null || \
    echo "-- Stream air-quality has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_air-quality_sync.sql"

echo "  - air-quality (recreate)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream air-quality --action recreate \
    > "$FIXTURES_DIR/stream_air-quality_recreate.sql" 2>/dev/null || \
    echo "-- Stream air-quality has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_air-quality_recreate.sql"

# Outdoor Weather
echo "  - outdoor-weather (sync)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream outdoor-weather --action sync \
    > "$FIXTURES_DIR/stream_outdoor-weather_sync.sql" 2>/dev/null || \
    echo "-- Stream outdoor-weather has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_outdoor-weather_sync.sql"

echo "  - outdoor-weather (recreate)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream outdoor-weather --action recreate \
    > "$FIXTURES_DIR/stream_outdoor-weather_recreate.sql" 2>/dev/null || \
    echo "-- Stream outdoor-weather has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_outdoor-weather_recreate.sql"

# Home Assistant State
echo "  - home-assistant-state (sync)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream home-assistant-state --action sync \
    > "$FIXTURES_DIR/stream_home-assistant-state_sync.sql" 2>/dev/null || \
    echo "-- Stream home-assistant-state has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_home-assistant-state_sync.sql"

echo "  - home-assistant-state (recreate)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream home-assistant-state --action recreate \
    > "$FIXTURES_DIR/stream_home-assistant-state_recreate.sql" 2>/dev/null || \
    echo "-- Stream home-assistant-state has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_home-assistant-state_recreate.sql"

# Outdoor Air Quality
echo "  - outdoor-air-quality (sync)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream outdoor-air-quality --action sync \
    > "$FIXTURES_DIR/stream_outdoor-air-quality_sync.sql" 2>/dev/null || \
    echo "-- Stream outdoor-air-quality has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_outdoor-air-quality_sync.sql"

echo "  - outdoor-air-quality (recreate)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream outdoor-air-quality --action recreate \
    > "$FIXTURES_DIR/stream_outdoor-air-quality_recreate.sql" 2>/dev/null || \
    echo "-- Stream outdoor-air-quality has no gold_etl config or is disabled" \
    > "$FIXTURES_DIR/stream_outdoor-air-quality_recreate.sql"

# =====================================
# TRANSITIONS OUTPUTS
# =====================================
echo "Capturing transitions outputs..."

echo "  - home-assistant-state transitions (sync)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream home-assistant-state --transitions --action sync \
    > "$FIXTURES_DIR/stream_home-assistant-state_transitions_sync.sql" 2>/dev/null || \
    echo "-- Stream home-assistant-state has no transitions config" \
    > "$FIXTURES_DIR/stream_home-assistant-state_transitions_sync.sql"

echo "  - home-assistant-state transitions (recreate)"
$TOOL --config-dir "$CONFIG_DIR" \
    generate --stream home-assistant-state --transitions --action recreate \
    > "$FIXTURES_DIR/stream_home-assistant-state_transitions_recreate.sql" 2>/dev/null || \
    echo "-- Stream home-assistant-state has no transitions config" \
    > "$FIXTURES_DIR/stream_home-assistant-state_transitions_recreate.sql"

# =====================================
# GENERATE CHECKSUMS
# =====================================
echo ""
echo "Generating checksums..."
cd "$FIXTURES_DIR"
sha256sum *.sql CAPTURE_INFO.txt > CHECKSUMS.sha256
cd - > /dev/null

# =====================================
# SUMMARY
# =====================================
echo ""
echo "========================================"
echo "CAPTURE COMPLETE"
echo "========================================"
echo ""
echo "Files captured:"
ls -la "$FIXTURES_DIR"/*.sql | wc -l
echo ""
echo "Checksums:"
cat "$FIXTURES_DIR/CHECKSUMS.sha256"
echo ""
echo "Next steps:"
echo "  1. Review captured files"
echo "  2. Commit to repository: git add $FIXTURES_DIR && git commit -m 'chore(fe-002): capture golden master baselines'"
echo "  3. Push to remote before starting Phase A"
echo ""
```

### 4.3 Post-Capture Verification

```bash
#!/bin/bash
# Location: scripts/verify-golden-master.sh
# Verify captured fixtures are valid

set -euo pipefail

FIXTURES_DIR="tools/ndp-gold-ddl/tests/fixtures/golden-master"

echo "========================================"
echo "GOLDEN MASTER VERIFICATION"
echo "========================================"

# Check all expected files exist
EXPECTED_FILES=(
    "domain_indoor-air-quality_sync.sql"
    "domain_indoor-air-quality_recreate.sql"
    "stream_air-quality_sync.sql"
    "stream_air-quality_recreate.sql"
    "stream_outdoor-weather_sync.sql"
    "stream_outdoor-weather_recreate.sql"
    "stream_home-assistant-state_sync.sql"
    "stream_home-assistant-state_recreate.sql"
    "stream_outdoor-air-quality_sync.sql"
    "stream_outdoor-air-quality_recreate.sql"
    "stream_home-assistant-state_transitions_sync.sql"
    "stream_home-assistant-state_transitions_recreate.sql"
    "CHECKSUMS.sha256"
    "CAPTURE_INFO.txt"
)

echo "Checking file existence..."
MISSING=0
for file in "${EXPECTED_FILES[@]}"; do
    if [[ ! -f "$FIXTURES_DIR/$file" ]]; then
        echo "  MISSING: $file"
        MISSING=$((MISSING + 1))
    else
        echo "  OK: $file"
    fi
done

if [[ $MISSING -gt 0 ]]; then
    echo ""
    echo "ERROR: $MISSING expected files are missing"
    exit 1
fi

# Verify checksums
echo ""
echo "Verifying checksums..."
cd "$FIXTURES_DIR"
if sha256sum -c CHECKSUMS.sha256; then
    echo "All checksums valid"
else
    echo "ERROR: Checksum verification failed"
    exit 1
fi
cd - > /dev/null

# Check files are not empty (unless they're placeholders)
echo ""
echo "Checking file contents..."
for file in "$FIXTURES_DIR"/*.sql; do
    lines=$(wc -l < "$file")
    if [[ $lines -lt 3 ]]; then
        # Check if it's a valid placeholder
        if grep -q "has no gold_etl config" "$file" || grep -q "has no transitions config" "$file"; then
            echo "  OK (placeholder): $(basename "$file")"
        else
            echo "  WARNING: $(basename "$file") has only $lines lines"
        fi
    else
        echo "  OK: $(basename "$file") ($lines lines)"
    fi
done

echo ""
echo "========================================"
echo "VERIFICATION COMPLETE"
echo "========================================"
```

---

## 5. Regenerating Baselines

### 5.1 When to Regenerate

Baselines should ONLY be regenerated when:

1. **Intentional DDL change** - A new feature changes DDL output
2. **Bug fix** - A bug fix corrects incorrect DDL
3. **Schema evolution** - Domain schema changes require DDL updates

**Never regenerate just to make tests pass without understanding why they failed.**

### 5.2 Regeneration Procedure

```bash
# 1. Document the change
echo "Reason for regeneration: [describe why]" >> CHANGELOG.md

# 2. Verify the change is intentional
git diff  # Review code changes

# 3. Regenerate baselines
./scripts/capture-golden-master.sh

# 4. Review the new baselines
git diff tools/ndp-gold-ddl/tests/fixtures/golden-master/

# 5. Commit with detailed message
git add tools/ndp-gold-ddl/tests/fixtures/golden-master/
git commit -m "chore(fe-002): update golden master baselines

Reason: [describe why baselines changed]

Changes:
- [list specific DDL changes]

Verified:
- [ ] Change is intentional
- [ ] New DDL is correct
- [ ] Tests pass with new baselines"
```

### 5.3 Review Checklist for Baseline Updates

Before approving a baseline update PR:

- [ ] Is there a clear reason documented for the update?
- [ ] Does the code change justify the DDL change?
- [ ] Are the new baselines functionally correct?
- [ ] Will the new DDL work with existing databases?
- [ ] Is there a migration path if needed?

---

## 6. Version Control Strategy

### 6.1 Git Handling

```gitignore
# These files should NOT be in .gitignore
# They must be committed:
# tools/ndp-gold-ddl/tests/fixtures/golden-master/*.sql
# tools/ndp-gold-ddl/tests/fixtures/golden-master/CHECKSUMS.sha256
# tools/ndp-gold-ddl/tests/fixtures/golden-master/CAPTURE_INFO.txt
```

### 6.2 Commit Message Template

```
chore(fe-002): [action] golden master baselines

[One line summary]

Fixtures affected:
- [list of files]

Reason: [why this change was made]

Verified by:
- cargo test -p ndp-gold-ddl --test golden_master_test
```

### 6.3 Branch Strategy

```
main
  │
  ├── fe-002/phase-0-baselines    # Capture baselines (merge first)
  │     └── Commit: chore(fe-002): capture golden master baselines
  │
  ├── fe-002/phase-a-migration    # YAML to JSON (depends on phase-0)
  │     └── Baselines unchanged
  │
  └── fe-002/phase-b-validation   # Add validation (depends on phase-a)
        └── Baselines unchanged
```

---

## 7. Fixture File Format

### 7.1 SQL File Structure

Each `.sql` fixture should be:
- Complete, executable SQL
- Generated directly by `ndp-gold-ddl`
- Include comments showing generator info
- UTF-8 encoded
- Unix line endings (LF, not CRLF)

### 7.2 Expected Content Patterns

**Domain sync mode** (`domain_*_sync.sql`):
```sql
-- Aligned view for domain: indoor-air-quality
-- Streams: indoor, outdoor, state, outdoor_aqi
-- Mode: SYNC (create if not exists)

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_matviews
        ...
```

**Domain recreate mode** (`domain_*_recreate.sql`):
```sql
-- Aligned view for domain: indoor-air-quality
-- Streams: indoor, outdoor, state, outdoor_aqi
-- Mode: RECREATE (drop and create)

-- Drop existing view
DROP MATERIALIZED VIEW IF EXISTS gold.indoor_air_quality_aligned CASCADE;
...
```

**Stream sync mode** (`stream_*_sync.sql`):
```sql
-- Gold layer DDL for stream: air-quality
-- Generated by ndp-gold-ddl

CREATE SCHEMA IF NOT EXISTS gold;

-- Sync mode: Create if not exists
-- CA-SYNC-CHECK: schema=gold name=air_quality_hourly
CREATE MATERIALIZED VIEW gold.air_quality_hourly
...
```

### 7.3 CHECKSUMS.sha256 Format

```
e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  domain_indoor-air-quality_sync.sql
d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592  domain_indoor-air-quality_recreate.sql
...
```

### 7.4 CAPTURE_INFO.txt Format

```
Golden Master Baseline Capture
==============================
Timestamp: 2026-02-05T10:30:00Z
Git Branch: main
Git SHA: abc123def456...
Git Status: 0 uncommitted changes
Rust Version: rustc 1.75.0
Cargo Version: cargo 1.75.0
Host: Linux 6.12.54-linuxkit

Purpose: Capture DDL output from ndp-gold-ddl BEFORE FE-002 Phase A migration.
         These fixtures serve as the acceptance criteria for YAML->JSON migration.

Captured By: developer
```

---

## 8. Troubleshooting

### 8.1 Capture Fails

| Error | Cause | Solution |
|-------|-------|----------|
| "Config not found" | Domain/stream config missing | Verify config files exist |
| "Build error" | Rust compilation failed | Run `cargo build -p ndp-gold-ddl` first |
| "No gold_etl config" | Stream doesn't have Gold enabled | Expected - creates placeholder |
| "Permission denied" | Can't write to fixtures dir | Check directory permissions |

### 8.2 Verification Fails

| Error | Cause | Solution |
|-------|-------|----------|
| "Missing file" | Capture incomplete | Re-run capture script |
| "Checksum mismatch" | File modified after capture | Re-run capture or investigate |
| "Empty file" | Generation failed silently | Check stderr during capture |

### 8.3 Test Comparison Fails

| Diff Location | Likely Cause | Investigation |
|---------------|--------------|---------------|
| Comments only | Version/timestamp change | Usually safe - update baseline |
| SQL structure | Parser difference | Compare YAML vs JSON config |
| Field order | HashMap iteration order | Check serde configuration |
| Missing columns | Config field not parsed | Debug config loading |
| Extra whitespace | String formatting | Check template generation |

---

## 9. References

- [TEST-STRATEGY.md](./TEST-STRATEGY.md) - Overall testing strategy
- [TDD-GUIDE.md](./TDD-GUIDE.md) - TDD implementation guide
- [TEST-PLAN.md](./TEST-PLAN.md) - Detailed test cases
- [FE-002 SCOPE.md](../SCOPE.md) - Feature scope and requirements

---

*Golden Master Fixtures Reference created: 2026-02-05*
*Feature: FE-002 Domain Configuration Standardization*
*Total fixtures: 14 files (12 SQL + 2 metadata)*
