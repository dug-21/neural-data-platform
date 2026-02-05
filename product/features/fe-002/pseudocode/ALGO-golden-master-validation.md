# ALGO-002: Golden Master Validation for Safe Migration

## Overview

Algorithm for the golden master testing approach that **guarantees NO behavioral change** during the YAML-to-JSON migration. This is the critical safety mechanism for FE-002.

**Feature:** FE-002 Domain Configuration Standardization
**Phase:** Pseudocode (SPARC P)
**Risk Level:** Critical (this algorithm IS the risk mitigation)

---

## Core Principle

```
INVARIANT: Before migration DDL == After migration DDL

If ndp-gold-ddl generates IDENTICAL SQL before and after the
YAML-to-JSON migration, then the migration has ZERO behavioral impact.
```

---

## Algorithm Specification

### ALGORITHM: GoldenMasterValidation

```
ALGORITHM: GoldenMasterValidation
INPUT:
    domain_id: Domain to validate (e.g., "indoor-air-quality")
    config_dir: Path to config directory
OUTPUT:
    validation_result: PASS | FAIL
    diff_report: Detailed differences if FAIL

PRECONDITIONS:
    - ndp-gold-ddl binary is built and available
    - Git working tree is clean (no uncommitted changes)
    - domain.yaml exists at config_dir/domains/{domain_id}/domain.yaml

POSTCONDITIONS:
    - If PASS: domain.json is semantically equivalent to domain.yaml
    - If FAIL: No changes committed, rollback to original state

COMPLEXITY:
    Time: O(n + g) where n = config size, g = DDL generation time
    Space: O(d) where d = DDL output size
```

---

## Main Validation Flow

```
BEGIN GoldenMasterValidation(domain_id, config_dir):

    // ========================================
    // PHASE 1: Capture Baseline (Before)
    // ========================================

    Log("Phase 1: Capturing baseline DDL from YAML configuration")

    yaml_path <- config_dir + "/domains/" + domain_id + "/domain.yaml"
    baseline_dir <- CreateTempDir("golden-master-baseline")

    IF NOT FileExists(yaml_path) THEN:
        RETURN Error("Source YAML not found: " + yaml_path)
    END IF

    // Generate DDL from current YAML configuration
    baseline_result <- ExecuteCommand(
        "ndp-gold-ddl generate --domain " + domain_id + " --config-dir " + config_dir
    )

    IF baseline_result.exit_code != 0 THEN:
        RETURN Error("Baseline DDL generation failed: " + baseline_result.stderr)
    END IF

    baseline_ddl <- baseline_result.stdout
    baseline_path <- baseline_dir + "/baseline.sql"
    WriteFile(baseline_path, baseline_ddl)

    // Compute hash for quick comparison
    baseline_hash <- SHA256(baseline_ddl)

    Log("Baseline captured: " + baseline_hash[0:16] + "...")
    Log("Baseline DDL lines: " + CountLines(baseline_ddl))

    // ========================================
    // PHASE 2: Perform Conversion
    // ========================================

    Log("Phase 2: Converting YAML to JSON")

    json_path <- config_dir + "/domains/" + domain_id + "/domain.json"

    conversion_result <- ConvertDomainYamlToJson(
        yaml_path=yaml_path,
        schema_path=config_dir + "/schemas/domain.schema.json"
    )

    IF conversion_result.is_error THEN:
        RETURN Error("Conversion failed: " + conversion_result.message)
    END IF

    // Backup YAML for potential rollback
    yaml_backup <- baseline_dir + "/domain.yaml.backup"
    CopyFile(yaml_path, yaml_backup)

    // ========================================
    // PHASE 3: Update Loader (Simulated)
    // ========================================

    Log("Phase 3: Switching loader to JSON")

    // The loader.rs changes are:
    // 1. domain.yaml -> domain.json (path)
    // 2. serde_yaml -> serde_json (parser)
    //
    // For validation, we don't actually modify loader.rs yet.
    // Instead, we test with a modified binary or environment flag.

    // Option A: Use environment variable to toggle format
    SetEnv("NDP_DOMAIN_FORMAT", "json")

    // Option B: Rebuild with JSON loader (slower, more thorough)
    // rebuild_result <- ExecuteCommand("cargo build -p ndp-gold-ddl")

    // ========================================
    // PHASE 4: Capture New Output (After)
    // ========================================

    Log("Phase 4: Capturing DDL from JSON configuration")

    // Now generate DDL using the JSON file
    // This requires loader.rs to already be updated to read JSON
    // OR using the environment flag approach

    new_result <- ExecuteCommand(
        "ndp-gold-ddl generate --domain " + domain_id + " --config-dir " + config_dir
    )

    IF new_result.exit_code != 0 THEN:
        // Rollback: restore YAML
        CopyFile(yaml_backup, yaml_path)
        DeleteFile(json_path)
        RETURN Error("New DDL generation failed: " + new_result.stderr)
    END IF

    new_ddl <- new_result.stdout
    new_path <- baseline_dir + "/new.sql"
    WriteFile(new_path, new_ddl)

    new_hash <- SHA256(new_ddl)

    Log("New DDL captured: " + new_hash[0:16] + "...")
    Log("New DDL lines: " + CountLines(new_ddl))

    // ========================================
    // PHASE 5: Compare Outputs
    // ========================================

    Log("Phase 5: Comparing baseline vs new DDL")

    comparison <- CompareDDL(baseline_ddl, new_ddl)

    IF comparison.is_identical THEN:
        Log("PASS: DDL output is IDENTICAL")
        Log("Hash match: " + baseline_hash[0:16] + " == " + new_hash[0:16])

        // Safe to delete YAML
        DeleteFile(yaml_path)
        CleanupTempDir(baseline_dir)

        RETURN ValidationResult(
            status="PASS",
            baseline_hash=baseline_hash,
            new_hash=new_hash,
            diff_report=None
        )
    ELSE:
        Log("FAIL: DDL output DIFFERS")

        diff_report <- GenerateDiffReport(baseline_ddl, new_ddl)

        // ROLLBACK: Restore original state
        Log("Rolling back changes...")
        CopyFile(yaml_backup, yaml_path)
        DeleteFile(json_path)

        RETURN ValidationResult(
            status="FAIL",
            baseline_hash=baseline_hash,
            new_hash=new_hash,
            diff_report=diff_report
        )
    END IF

END GoldenMasterValidation
```

---

## Subroutine: DDL Comparison

```
SUBROUTINE: CompareDDL(baseline_ddl, new_ddl)
INPUT:
    baseline_ddl: Original DDL string
    new_ddl: New DDL string after migration
OUTPUT:
    comparison_result: Object with is_identical, differences

BEGIN:
    // ----------------------------------------
    // Strategy 1: Hash Comparison (Fast Path)
    // ----------------------------------------

    baseline_hash <- SHA256(baseline_ddl)
    new_hash <- SHA256(new_ddl)

    IF baseline_hash == new_hash THEN:
        RETURN {
            is_identical: TRUE,
            method: "hash_match",
            differences: []
        }
    END IF

    // ----------------------------------------
    // Strategy 2: Normalized Comparison
    // ----------------------------------------

    // Normalize whitespace and comments for semantic comparison
    baseline_normalized <- NormalizeDDL(baseline_ddl)
    new_normalized <- NormalizeDDL(new_ddl)

    IF baseline_normalized == new_normalized THEN:
        RETURN {
            is_identical: TRUE,
            method: "normalized_match",
            differences: [],
            note: "Whitespace-only differences detected"
        }
    END IF

    // ----------------------------------------
    // Strategy 3: Line-by-Line Diff
    // ----------------------------------------

    differences <- ComputeLineDiff(baseline_ddl, new_ddl)

    RETURN {
        is_identical: FALSE,
        method: "diff",
        differences: differences
    }

END CompareDDL


SUBROUTINE: NormalizeDDL(ddl_string)
INPUT: Raw DDL string
OUTPUT: Normalized DDL string for comparison

BEGIN:
    lines <- SplitLines(ddl_string)
    normalized_lines <- []

    FOR EACH line IN lines:
        // Remove leading/trailing whitespace
        trimmed <- Trim(line)

        // Skip empty lines
        IF trimmed == "" THEN:
            CONTINUE
        END IF

        // Skip pure comment lines (-- ...)
        IF StartsWith(trimmed, "--") THEN:
            CONTINUE
        END IF

        // Normalize internal whitespace (multiple spaces -> single)
        normalized <- CollapseWhitespace(trimmed)

        // Lowercase keywords for case-insensitive comparison
        // (PostgreSQL keywords are case-insensitive)
        normalized <- NormalizeSqlKeywords(normalized)

        normalized_lines.APPEND(normalized)
    END FOR

    RETURN JoinLines(normalized_lines, "\n")

END NormalizeDDL


SUBROUTINE: NormalizeSqlKeywords(line)
INPUT: SQL line
OUTPUT: Line with keywords in consistent case

BEGIN:
    // List of SQL keywords to normalize
    keywords <- [
        "CREATE", "VIEW", "AS", "SELECT", "FROM", "WHERE",
        "JOIN", "LEFT", "RIGHT", "FULL", "OUTER", "INNER",
        "ON", "AND", "OR", "NOT", "NULL", "IS", "COALESCE",
        "CASE", "WHEN", "THEN", "ELSE", "END", "ORDER", "BY",
        "GROUP", "HAVING", "UNION", "ALL", "DISTINCT"
    ]

    result <- line

    FOR EACH keyword IN keywords:
        // Replace case-insensitively with uppercase version
        result <- RegexReplace(
            result,
            "\\b" + keyword + "\\b",
            ToUpperCase(keyword),
            flags="IGNORE_CASE"
        )
    END FOR

    RETURN result

END NormalizeSqlKeywords
```

---

## Subroutine: Diff Report Generation

```
SUBROUTINE: GenerateDiffReport(baseline_ddl, new_ddl)
INPUT:
    baseline_ddl: Original DDL
    new_ddl: New DDL
OUTPUT:
    diff_report: Human-readable diff report

BEGIN:
    report <- StringBuilder()

    report.AppendLine("=" * 70)
    report.AppendLine("GOLDEN MASTER VALIDATION FAILED")
    report.AppendLine("=" * 70)
    report.AppendLine("")

    // ----------------------------------------
    // Summary Statistics
    // ----------------------------------------

    baseline_lines <- CountLines(baseline_ddl)
    new_lines <- CountLines(new_ddl)

    report.AppendLine("SUMMARY:")
    report.AppendLine("  Baseline lines: " + baseline_lines)
    report.AppendLine("  New lines:      " + new_lines)
    report.AppendLine("  Difference:     " + (new_lines - baseline_lines) + " lines")
    report.AppendLine("")

    // ----------------------------------------
    // Unified Diff
    // ----------------------------------------

    report.AppendLine("UNIFIED DIFF:")
    report.AppendLine("-" * 70)

    diff_lines <- ComputeUnifiedDiff(
        baseline_ddl,
        new_ddl,
        fromfile="baseline.sql",
        tofile="new.sql",
        context_lines=3
    )

    FOR EACH diff_line IN diff_lines:
        IF StartsWith(diff_line, "-") AND NOT StartsWith(diff_line, "---") THEN:
            report.AppendLine(ColorRed(diff_line))
        ELSE IF StartsWith(diff_line, "+") AND NOT StartsWith(diff_line, "+++") THEN:
            report.AppendLine(ColorGreen(diff_line))
        ELSE:
            report.AppendLine(diff_line)
        END IF
    END FOR

    report.AppendLine("-" * 70)
    report.AppendLine("")

    // ----------------------------------------
    // Semantic Analysis
    // ----------------------------------------

    report.AppendLine("SEMANTIC ANALYSIS:")

    // Check for specific issues
    issues <- AnalyzeDifferences(baseline_ddl, new_ddl)

    FOR EACH issue IN issues:
        report.AppendLine("  - " + issue.category + ": " + issue.description)
    END FOR

    IF issues IS EMPTY THEN:
        report.AppendLine("  No semantic differences detected (likely formatting only)")
    END IF

    report.AppendLine("")

    // ----------------------------------------
    // Recommended Actions
    // ----------------------------------------

    report.AppendLine("RECOMMENDED ACTIONS:")
    report.AppendLine("  1. Review the diff above carefully")
    report.AppendLine("  2. Check domain.json for conversion errors")
    report.AppendLine("  3. Verify field mappings in MapFields algorithm")
    report.AppendLine("  4. If differences are expected, update baseline")
    report.AppendLine("")

    RETURN report.ToString()

END GenerateDiffReport


SUBROUTINE: AnalyzeDifferences(baseline_ddl, new_ddl)
INPUT: Two DDL strings
OUTPUT: Array of semantic issues

BEGIN:
    issues <- []

    // Check for missing views
    baseline_views <- ExtractViewNames(baseline_ddl)
    new_views <- ExtractViewNames(new_ddl)

    FOR EACH view IN baseline_views:
        IF view NOT IN new_views THEN:
            issues.APPEND({
                category: "MISSING_VIEW",
                description: "View '" + view + "' present in baseline but missing in new"
            })
        END IF
    END FOR

    FOR EACH view IN new_views:
        IF view NOT IN baseline_views THEN:
            issues.APPEND({
                category: "EXTRA_VIEW",
                description: "View '" + view + "' present in new but missing in baseline"
            })
        END IF
    END FOR

    // Check for column differences
    baseline_columns <- ExtractColumnReferences(baseline_ddl)
    new_columns <- ExtractColumnReferences(new_ddl)

    column_diff <- SetDifference(baseline_columns, new_columns)
    IF column_diff IS NOT EMPTY THEN:
        issues.APPEND({
            category: "COLUMN_MISMATCH",
            description: "Column references differ: " + Join(column_diff, ", ")
        })
    END IF

    // Check for JOIN differences
    baseline_joins <- ExtractJoinClauses(baseline_ddl)
    new_joins <- ExtractJoinClauses(new_ddl)

    IF baseline_joins != new_joins THEN:
        issues.APPEND({
            category: "JOIN_MISMATCH",
            description: "JOIN clauses differ between baseline and new"
        })
    END IF

    RETURN issues

END AnalyzeDifferences
```

---

## Subroutine: Rollback Procedure

```
SUBROUTINE: RollbackMigration(domain_id, config_dir, backup_dir)
INPUT:
    domain_id: Domain being rolled back
    config_dir: Config directory path
    backup_dir: Directory containing backup files
OUTPUT:
    success: Boolean

BEGIN:
    Log("Starting rollback for domain: " + domain_id)

    yaml_path <- config_dir + "/domains/" + domain_id + "/domain.yaml"
    json_path <- config_dir + "/domains/" + domain_id + "/domain.json"
    yaml_backup <- backup_dir + "/domain.yaml.backup"

    // ----------------------------------------
    // Step 1: Restore YAML from backup
    // ----------------------------------------

    IF FileExists(yaml_backup) THEN:
        Log("Restoring domain.yaml from backup...")
        CopyFile(yaml_backup, yaml_path)
    ELSE:
        Log("WARNING: No YAML backup found at " + yaml_backup)
    END IF

    // ----------------------------------------
    // Step 2: Remove JSON if created
    // ----------------------------------------

    IF FileExists(json_path) THEN:
        Log("Removing domain.json...")
        DeleteFile(json_path)
    END IF

    // ----------------------------------------
    // Step 3: Verify restoration
    // ----------------------------------------

    IF FileExists(yaml_path) THEN:
        // Verify YAML is valid
        TRY:
            yaml_content <- ReadFile(yaml_path)
            ParseYaml(yaml_content)
            Log("Rollback complete: domain.yaml restored and valid")
            RETURN TRUE
        CATCH:
            Log("ERROR: Restored YAML is invalid")
            RETURN FALSE
        END TRY
    ELSE:
        Log("ERROR: Rollback failed - YAML not restored")
        RETURN FALSE
    END IF

END RollbackMigration
```

---

## Complete Validation Workflow Script

```
SCRIPT: validate-migration.sh

#!/bin/bash
# Golden Master Validation Script for FE-002
# Usage: ./validate-migration.sh <domain_id>

set -e

DOMAIN_ID="${1:-indoor-air-quality}"
CONFIG_DIR="${CONFIG_DIR:-./config}"
TEMP_DIR=$(mktemp -d)

echo "================================================"
echo "FE-002 Golden Master Validation"
echo "Domain: $DOMAIN_ID"
echo "Config Dir: $CONFIG_DIR"
echo "Temp Dir: $TEMP_DIR"
echo "================================================"

# Phase 1: Capture baseline
echo ""
echo "[Phase 1] Capturing baseline DDL..."
ndp-gold-ddl generate --domain "$DOMAIN_ID" --config-dir "$CONFIG_DIR" > "$TEMP_DIR/baseline.sql"
BASELINE_HASH=$(sha256sum "$TEMP_DIR/baseline.sql" | cut -d' ' -f1)
echo "Baseline hash: ${BASELINE_HASH:0:16}..."
echo "Baseline lines: $(wc -l < "$TEMP_DIR/baseline.sql")"

# Phase 2: Convert YAML to JSON
echo ""
echo "[Phase 2] Converting YAML to JSON..."
YAML_PATH="$CONFIG_DIR/domains/$DOMAIN_ID/domain.yaml"
JSON_PATH="$CONFIG_DIR/domains/$DOMAIN_ID/domain.json"

# Backup YAML
cp "$YAML_PATH" "$TEMP_DIR/domain.yaml.backup"

# Convert using Python (or jq/yq)
python3 -c "
import yaml
import json

with open('$YAML_PATH', 'r') as f:
    data = yaml.safe_load(f)

# Wrap in domain key for schema compliance
wrapped = {'domain': data}

with open('$JSON_PATH', 'w') as f:
    json.dump(wrapped, f, indent=2)
    f.write('\n')

print('Conversion complete')
"

# Validate JSON against schema
echo "Validating JSON against schema..."
python3 -c "
import json
from jsonschema import validate, ValidationError

with open('$JSON_PATH') as f:
    data = json.load(f)

with open('$CONFIG_DIR/schemas/domain.schema.json') as f:
    schema = json.load(f)

try:
    validate(data, schema)
    print('Schema validation: PASS')
except ValidationError as e:
    print(f'Schema validation: FAIL - {e.message}')
    exit(1)
"

# Phase 3: Generate new DDL (requires loader.rs update)
echo ""
echo "[Phase 3] Generating DDL from JSON..."
# NOTE: This step requires loader.rs to be updated first
# For initial testing, manually update loader.rs then run this

# Simulated: If loader.rs not yet updated, skip this phase
if [ "$SKIP_DDL_CHECK" = "1" ]; then
    echo "SKIPPING DDL comparison (SKIP_DDL_CHECK=1)"
    exit 0
fi

ndp-gold-ddl generate --domain "$DOMAIN_ID" --config-dir "$CONFIG_DIR" > "$TEMP_DIR/new.sql"
NEW_HASH=$(sha256sum "$TEMP_DIR/new.sql" | cut -d' ' -f1)
echo "New hash: ${NEW_HASH:0:16}..."
echo "New lines: $(wc -l < "$TEMP_DIR/new.sql")"

# Phase 4: Compare
echo ""
echo "[Phase 4] Comparing DDL outputs..."

if [ "$BASELINE_HASH" = "$NEW_HASH" ]; then
    echo ""
    echo "================================================"
    echo "VALIDATION: PASS"
    echo "DDL output is IDENTICAL"
    echo "Safe to commit JSON and remove YAML"
    echo "================================================"

    # Remove YAML
    rm "$YAML_PATH"
    echo "Removed: $YAML_PATH"

    # Cleanup
    rm -rf "$TEMP_DIR"
    exit 0
else
    echo ""
    echo "================================================"
    echo "VALIDATION: FAIL"
    echo "DDL output DIFFERS"
    echo "================================================"

    # Show diff
    echo ""
    echo "--- Diff ---"
    diff -u "$TEMP_DIR/baseline.sql" "$TEMP_DIR/new.sql" || true

    # Rollback
    echo ""
    echo "Rolling back..."
    cp "$TEMP_DIR/domain.yaml.backup" "$YAML_PATH"
    rm -f "$JSON_PATH"
    echo "Restored: $YAML_PATH"
    echo "Removed: $JSON_PATH"

    # Cleanup
    rm -rf "$TEMP_DIR"
    exit 1
fi
```

---

## Integration with CI/CD

### GitHub Actions Workflow Step

```yaml
- name: Validate Domain Migration (Golden Master)
  run: |
    # Build ndp-gold-ddl
    cargo build --release -p ndp-gold-ddl

    # Run golden master validation
    ./scripts/validate-migration.sh indoor-air-quality

  env:
    CONFIG_DIR: ./config
```

### Pre-commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit

# Check if domain.json was modified
if git diff --cached --name-only | grep -q "domain.json"; then
    echo "Domain config modified - running golden master validation..."

    # Extract domain_id from path
    DOMAIN_JSON=$(git diff --cached --name-only | grep "domain.json")
    DOMAIN_ID=$(dirname "$DOMAIN_JSON" | xargs basename)

    ./scripts/validate-migration.sh "$DOMAIN_ID"
fi
```

---

## Complexity Analysis

| Phase | Time Complexity | Space Complexity | Notes |
|-------|-----------------|------------------|-------|
| Capture Baseline | O(g) | O(d) | g = DDL gen time, d = DDL size |
| Conversion | O(n) | O(n) | n = config size |
| Capture New | O(g) | O(d) | Same as baseline |
| Hash Compare | O(d) | O(1) | Hash computation |
| Diff (if needed) | O(d^2) | O(d) | Worst case LCS |
| Rollback | O(n) | O(1) | File copy |

**Total: O(g + n + d^2)** worst case, **O(g + n)** typical (hash match)
**Space: O(d)** for DDL storage

---

## References

- **FE-002 SCOPE.md**: Feature specification
- **ndp-gold-ddl**: DDL generator tool
- **domain.rs**: Domain configuration structs
- **loader.rs**: Configuration loading logic
- **ALGO-001**: YAML to JSON conversion algorithm
