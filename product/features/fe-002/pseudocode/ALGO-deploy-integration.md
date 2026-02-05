# ALGO-004: Deploy Script Domain Validation Integration

## Overview

Algorithm for integrating domain configuration validation into the `deploy.sh` workflow, ensuring that invalid domain configs cannot be deployed to production.

**Feature:** FE-002 Domain Configuration Standardization
**Phase:** Pseudocode (SPARC P)
**AC Reference:** AC-B7 (deploy.sh validates domain configs before deployment)

---

## Deployment Pipeline Context

```
                    ┌────────────────────────────────────────────────┐
                    │                  deploy.sh                      │
                    │                                                 │
                    │  ┌──────────────────────────────────────────┐  │
                    │  │  PRE-DEPLOYMENT VALIDATION (NEW)         │  │
                    │  │  ├── Stream config validation            │  │
                    │  │  └── Domain config validation  ◄─────────┼──┼── FE-002
                    │  └──────────────────────────────────────────┘  │
                    │                      │                          │
                    │                      ▼ (fail-fast if invalid)   │
                    │  ┌──────────────────────────────────────────┐  │
                    │  │  CONFIG SYNC                              │  │
                    │  │  ├── Sync streams to etcd                │  │
                    │  │  └── Sync domains to etcd (if added)     │  │
                    │  └──────────────────────────────────────────┘  │
                    │                      │                          │
                    │                      ▼                          │
                    │  ┌──────────────────────────────────────────┐  │
                    │  │  SERVICE DEPLOYMENT                       │  │
                    │  │  ├── Docker compose up                   │  │
                    │  │  └── Health checks                       │  │
                    │  └──────────────────────────────────────────┘  │
                    └────────────────────────────────────────────────┘
```

---

## Algorithm Specification

### ALGORITHM: DeployWithDomainValidation

```
ALGORITHM: DeployWithDomainValidation
INPUT:
    command: Deploy command (deploy, sync, update, etc.)
    options: Deploy options (--no-validate, --domain-only, etc.)
OUTPUT:
    exit_code: 0 (success) | 1 (validation failed) | 2 (deployment failed)

PRECONDITIONS:
    - ndp-validate binary is built or available
    - Config directories exist
    - Domain.json files exist (post FE-002 Phase A)

POSTCONDITIONS:
    - If validation fails: no changes deployed
    - If validation passes: deployment proceeds as normal
    - Validation results logged for audit

COMPLEXITY:
    Time: O(v + d) where v = validation time, d = deployment time
    Space: O(1) (streaming output)
```

---

## Main Deployment Flow

```
BEGIN DeployWithDomainValidation(command, options):

    // ========================================
    // PHASE 0: Environment Setup
    // ========================================

    Log("NDP Deployment Starting")
    Log("Environment: " + DEPLOY_ENV)
    Log("Command: " + command)

    // Resolve config directories based on environment
    IF DEPLOY_ENV == "integration" THEN:
        CONFIG_STREAMS_DIR <- REPO_ROOT + "/config/integration/base/streams"
        CONFIG_DOMAINS_DIR <- REPO_ROOT + "/config/integration/domains"
    ELSE:
        CONFIG_STREAMS_DIR <- REPO_ROOT + "/config/base/streams"
        CONFIG_DOMAINS_DIR <- REPO_ROOT + "/config/domains"
    END IF

    // ========================================
    // PHASE 1: Pre-Deployment Validation
    // ========================================

    IF NOT options.skip_validation THEN:
        Log("Phase 1: Pre-deployment validation")

        validation_result <- ValidateAllConfigs(
            streams_dir=CONFIG_STREAMS_DIR,
            domains_dir=CONFIG_DOMAINS_DIR,
            options=options
        )

        IF NOT validation_result.success THEN:
            Log(ColorRed("VALIDATION FAILED"))
            Log("Deployment aborted. Fix validation errors and retry.")
            PrintValidationSummary(validation_result)
            RETURN 1
        END IF

        Log(ColorGreen("Validation passed"))
    ELSE:
        Log(ColorYellow("WARNING: Skipping validation (--no-validate)"))
    END IF

    // ========================================
    // PHASE 2: Execute Command
    // ========================================

    Log("Phase 2: Executing " + command)

    SWITCH command:
        CASE "deploy":
            result <- ExecuteFullDeploy()
        CASE "sync":
            result <- ExecuteConfigSync()
        CASE "update":
            result <- ExecuteUpdate(options.target)
        CASE "start":
            result <- ExecuteStart()
        CASE "stop":
            result <- ExecuteStop()
        DEFAULT:
            result <- ExecuteCommand(command)
    END SWITCH

    IF result.exit_code != 0 THEN:
        Log(ColorRed("Deployment failed"))
        RETURN 2
    END IF

    Log(ColorGreen("Deployment successful"))
    RETURN 0

END DeployWithDomainValidation
```

---

## Subroutine: Validate All Configs

```
SUBROUTINE: ValidateAllConfigs(streams_dir, domains_dir, options)
INPUT:
    streams_dir: Path to stream configs
    domains_dir: Path to domain configs
    options: Validation options
OUTPUT:
    validation_result: {
        success: Boolean
        stream_results: Array<ValidationResult>
        domain_results: Array<ValidationResult>
        summary: ValidationSummary
    }

BEGIN:
    result <- {
        success: TRUE,
        stream_results: [],
        domain_results: [],
        summary: { total: 0, passed: 0, failed: 0, errors: 0, warnings: 0 }
    }

    // ----------------------------------------
    // Step 1: Validate Stream Configs
    // ----------------------------------------

    Log("Validating stream configurations...")

    stream_configs <- DiscoverStreamConfigs(streams_dir)

    FOR EACH config_path IN stream_configs:
        Log("  Validating: " + RelativePath(config_path))

        stream_result <- ExecuteValidation(
            "ndp-validate " + QuotePath(config_path)
        )

        result.stream_results.APPEND(stream_result)
        result.summary.total <- result.summary.total + 1

        IF stream_result.valid THEN:
            result.summary.passed <- result.summary.passed + 1
            Log("    " + ColorGreen("[PASS]"))
        ELSE:
            result.summary.failed <- result.summary.failed + 1
            result.summary.errors <- result.summary.errors + stream_result.error_count
            result.success <- FALSE
            Log("    " + ColorRed("[FAIL]") + " (" + stream_result.error_count + " errors)")
        END IF

        result.summary.warnings <- result.summary.warnings + stream_result.warning_count
    END FOR

    // ----------------------------------------
    // Step 2: Validate Domain Configs (NEW)
    // ----------------------------------------

    Log("Validating domain configurations...")

    domain_configs <- DiscoverDomainConfigs(domains_dir)

    IF domain_configs IS EMPTY THEN:
        Log("  No domain configs found (skipping)")
    ELSE:
        FOR EACH config_path IN domain_configs:
            Log("  Validating: " + RelativePath(config_path))

            domain_result <- ExecuteValidation(
                "ndp-validate --domain " + QuotePath(config_path)
            )

            result.domain_results.APPEND(domain_result)
            result.summary.total <- result.summary.total + 1

            IF domain_result.valid THEN:
                result.summary.passed <- result.summary.passed + 1
                Log("    " + ColorGreen("[PASS]"))
            ELSE:
                result.summary.failed <- result.summary.failed + 1
                result.summary.errors <- result.summary.errors + domain_result.error_count
                result.success <- FALSE
                Log("    " + ColorRed("[FAIL]") + " (" + domain_result.error_count + " errors)")
            END IF

            result.summary.warnings <- result.summary.warnings + domain_result.warning_count
        END FOR
    END IF

    // ----------------------------------------
    // Step 3: Cross-Validation (Streams + Domains)
    // ----------------------------------------

    IF NOT options.skip_cross_validation THEN:
        Log("Cross-validating stream references...")

        cross_errors <- CrossValidateDomainsAndStreams(
            stream_configs,
            domain_configs
        )

        IF cross_errors IS NOT EMPTY THEN:
            result.success <- FALSE
            result.summary.errors <- result.summary.errors + Length(cross_errors)

            FOR EACH error IN cross_errors:
                Log("  " + ColorRed("[CROSS-VALIDATION]") + " " + error.message)
            END FOR
        END IF
    END IF

    RETURN result

END ValidateAllConfigs
```

---

## Subroutine: Discover Domain Configs

```
SUBROUTINE: DiscoverDomainConfigs(domains_dir)
INPUT: Path to domains directory
OUTPUT: Array of domain.json paths

BEGIN:
    configs <- []

    IF NOT DirectoryExists(domains_dir) THEN:
        Log("WARNING: Domains directory not found: " + domains_dir)
        RETURN configs
    END IF

    // Iterate over domain directories
    FOR EACH entry IN ListDirectory(domains_dir):
        IF IsDirectory(entry) THEN:
            // Check for domain.json (FE-002 format)
            json_path <- entry + "/domain.json"

            IF FileExists(json_path) THEN:
                configs.APPEND(json_path)
            ELSE:
                // Fallback: check for domain.yaml (pre-FE-002)
                yaml_path <- entry + "/domain.yaml"

                IF FileExists(yaml_path) THEN:
                    Log("WARNING: Found YAML domain config (should be JSON): " + yaml_path)
                    configs.APPEND(yaml_path)
                END IF
            END IF
        END IF
    END FOR

    RETURN configs

END DiscoverDomainConfigs
```

---

## Subroutine: Cross-Validation

```
SUBROUTINE: CrossValidateDomainsAndStreams(stream_configs, domain_configs)
INPUT:
    stream_configs: Array of stream config paths
    domain_configs: Array of domain config paths
OUTPUT:
    Array of cross-validation errors

BEGIN:
    errors <- []

    // Build set of valid stream IDs from stream configs
    valid_streams <- SET()

    FOR EACH config_path IN stream_configs:
        TRY:
            content <- ReadFile(config_path)
            config <- ParseJson(content)
            stream_id <- config["stream_id"]

            IF stream_id IS NOT NULL THEN:
                valid_streams.ADD(stream_id)
            END IF
        CATCH:
            // Skip invalid stream configs (already caught by stream validation)
            CONTINUE
        END TRY
    END FOR

    // Validate domain stream references against actual streams
    FOR EACH domain_path IN domain_configs:
        TRY:
            content <- ReadFile(domain_path)

            // Handle both JSON and YAML
            IF EndsWith(domain_path, ".json") THEN:
                config <- ParseJson(content)
            ELSE:
                config <- ParseYaml(content)
            END IF

            // Extract domain content
            domain <- config["domain"] OR config  // Handle wrapped vs flat

            // Check each stream reference
            FOR EACH stream IN domain["streams"]:
                stream_id <- stream["stream_id"]

                IF stream_id NOT IN valid_streams THEN:
                    errors.APPEND({
                        domain_path: domain_path,
                        stream_id: stream_id,
                        message: "Domain '" + domain["id"] + "' references " +
                                 "non-existent stream '" + stream_id + "'"
                    })
                END IF
            END FOR

        CATCH error:
            // Skip invalid domain configs (already caught by domain validation)
            CONTINUE
        END TRY
    END FOR

    RETURN errors

END CrossValidateDomainsAndStreams
```

---

## Subroutine: Execute Validation Command

```
SUBROUTINE: ExecuteValidation(command)
INPUT: Validation command string
OUTPUT: ValidationResult

BEGIN:
    // Execute ndp-validate and capture output
    process <- SpawnProcess(command)
    stdout <- process.stdout
    stderr <- process.stderr
    exit_code <- process.wait()

    // Parse JSON output (ndp-validate outputs JSON by default)
    TRY:
        result <- ParseJson(stdout)

        RETURN {
            valid: result["valid"],
            error_count: result["summary"]["total_errors"],
            warning_count: result["summary"]["total_warnings"],
            errors: result["errors"],
            warnings: result["warnings"],
            raw_output: stdout
        }
    CATCH:
        // Fallback for non-JSON output or error
        RETURN {
            valid: exit_code == 0,
            error_count: IF exit_code != 0 THEN 1 ELSE 0,
            warning_count: 0,
            errors: [stderr],
            warnings: [],
            raw_output: stdout + stderr
        }
    END TRY

END ExecuteValidation
```

---

## Shell Script Implementation

### New Function: validate_domain_configs()

```bash
# ============================================================================
# Domain Configuration Validation (FE-002)
# ============================================================================

validate_domain_configs() {
    log "Validating domain configurations..."

    local domains_dir="$CONFIG_DOMAINS_DIR"
    local errors=0
    local warnings=0
    local total=0

    # Check if domains directory exists
    if [ ! -d "$domains_dir" ]; then
        warn "Domains directory not found: $domains_dir"
        return 0
    fi

    # Find all domain configs
    for domain_dir in "$domains_dir"/*/; do
        if [ -d "$domain_dir" ]; then
            local domain_json="${domain_dir}domain.json"
            local domain_yaml="${domain_dir}domain.yaml"
            local config_path=""

            # Prefer JSON over YAML (FE-002)
            if [ -f "$domain_json" ]; then
                config_path="$domain_json"
            elif [ -f "$domain_yaml" ]; then
                warn "Found YAML domain config (should be JSON): $domain_yaml"
                config_path="$domain_yaml"
            else
                continue
            fi

            ((total++))
            log "  Validating: $(basename "$domain_dir")"

            # Run validation
            local result
            if result=$(ndp-validate --domain "$config_path" --format json 2>&1); then
                local valid
                valid=$(echo "$result" | jq -r '.valid // true')

                if [ "$valid" = "true" ]; then
                    echo -e "    ${GREEN}[PASS]${NC}"
                else
                    local err_count
                    err_count=$(echo "$result" | jq -r '.summary.total_errors // 0')
                    echo -e "    ${RED}[FAIL]${NC} ($err_count errors)"
                    ((errors += err_count))

                    # Show errors in verbose mode
                    if [ "$VERBOSE" = "1" ]; then
                        echo "$result" | jq -r '.errors[] | "      - \(.path): \(.message)"'
                    fi
                fi

                local warn_count
                warn_count=$(echo "$result" | jq -r '.summary.total_warnings // 0')
                ((warnings += warn_count))
            else
                echo -e "    ${RED}[ERROR]${NC} Validation command failed"
                ((errors++))
            fi
        fi
    done

    # Summary
    log "Domain validation: $total configs, $errors errors, $warnings warnings"

    if [ "$errors" -gt 0 ]; then
        return 1
    fi

    return 0
}
```

### New Function: validate_all_configs()

```bash
# ============================================================================
# Combined Configuration Validation
# ============================================================================

validate_all_configs() {
    log "Running pre-deployment validation..."

    local stream_errors=0
    local domain_errors=0

    # Validate stream configs (existing)
    if ! validate_stream_configs; then
        stream_errors=1
    fi

    # Validate domain configs (NEW - FE-002)
    if ! validate_domain_configs; then
        domain_errors=1
    fi

    # Cross-validation
    if ! cross_validate_configs; then
        return 1
    fi

    if [ "$stream_errors" -ne 0 ] || [ "$domain_errors" -ne 0 ]; then
        error "Validation failed. Fix errors before deployment."
        return 1
    fi

    log "All configurations valid"
    return 0
}
```

### New Function: cross_validate_configs()

```bash
# ============================================================================
# Cross-Validation: Ensure domains reference valid streams
# ============================================================================

cross_validate_configs() {
    log "Cross-validating stream references..."

    local errors=0

    # Build list of valid stream IDs
    local valid_streams=()
    for stream_dir in "$CONFIG_STREAMS_DIR"/*/; do
        if [ -f "${stream_dir}config.json" ]; then
            local stream_id
            stream_id=$(jq -r '.stream_id // empty' "${stream_dir}config.json" 2>/dev/null)
            if [ -n "$stream_id" ]; then
                valid_streams+=("$stream_id")
            fi
        fi
    done

    # Check domain stream references
    for domain_dir in "$CONFIG_DOMAINS_DIR"/*/; do
        local domain_json="${domain_dir}domain.json"

        if [ -f "$domain_json" ]; then
            # Extract stream_ids from domain config
            local referenced_streams
            referenced_streams=$(jq -r '.domain.streams[].stream_id // empty' "$domain_json" 2>/dev/null)

            for ref_stream in $referenced_streams; do
                if ! printf '%s\n' "${valid_streams[@]}" | grep -qx "$ref_stream"; then
                    warn "  Domain $(basename "$domain_dir") references unknown stream: $ref_stream"
                    ((errors++))
                fi
            done
        fi
    done

    if [ "$errors" -gt 0 ]; then
        error "Cross-validation failed: $errors stream reference errors"
        return 1
    fi

    return 0
}
```

### Updated Deploy Command

```bash
# ============================================================================
# Main Deploy Function (Updated)
# ============================================================================

cmd_deploy() {
    log "Starting full deployment..."

    # Phase 1: Validation (NEW - includes domains)
    if [ "$SKIP_VALIDATION" != "1" ]; then
        if ! validate_all_configs; then
            error "Deployment aborted due to validation errors"
            exit 1
        fi
    else
        warn "Skipping validation (SKIP_VALIDATION=1)"
    fi

    # Phase 2: Build (existing)
    cmd_build

    # Phase 3: Sync configs (existing)
    cmd_sync

    # Phase 4: Start services (existing)
    cmd_start

    # Phase 5: Health check (existing)
    cmd_status

    log "Deployment complete"
}
```

---

## CLI Options

### New Options for deploy.sh

```
DEPLOY OPTIONS:
  --no-validate       Skip pre-deployment validation
  --domain-only       Only validate domain configs (skip streams)
  --stream-only       Only validate stream configs (skip domains)
  --strict            Treat warnings as errors
  --verbose           Show detailed validation output

ENVIRONMENT VARIABLES:
  SKIP_VALIDATION=1   Same as --no-validate
  VERBOSE=1           Same as --verbose
```

---

## Error Handling

### Failure Modes and Responses

| Failure Mode | Detection | Response | Exit Code |
|--------------|-----------|----------|-----------|
| ndp-validate not found | Command check | Error message, abort | 2 |
| Stream config invalid | Validation exit code | Log errors, abort | 1 |
| Domain config invalid | Validation exit code | Log errors, abort | 1 |
| Cross-validation fail | Reference check | Log warnings, abort | 1 |
| Deployment failure | Docker exit code | Log errors | 2 |

### Error Message Format

```
[DEPLOY] Validating domain configurations...
[DEPLOY]   Validating: indoor-air-quality
    [FAIL] (2 errors)
      - $.domain.streams[0].stream_id: Stream 'air-qualitee' not found
      - $.domain.objectives[0].target.stream: Objective references stream 'nonexistent'

[ERROR] Deployment aborted due to validation errors

To skip validation (NOT RECOMMENDED):
  SKIP_VALIDATION=1 ./deploy.sh

To view detailed errors:
  ndp-validate --domain config/domains/indoor-air-quality/domain.json --format human
```

---

## Integration Test Scenarios

### Test 1: Valid Configs Deploy Successfully

```bash
# Setup: All configs valid
# Expected: Deployment proceeds

$ ./deploy.sh
[DEPLOY] Running pre-deployment validation...
[DEPLOY] Validating stream configurations...
[DEPLOY]   Validating: air-quality
    [PASS]
[DEPLOY]   Validating: outdoor-weather
    [PASS]
[DEPLOY] Validating domain configurations...
[DEPLOY]   Validating: indoor-air-quality
    [PASS]
[DEPLOY] Cross-validating stream references...
[DEPLOY] All configurations valid
[DEPLOY] Starting full deployment...
# ... deployment continues ...
```

### Test 2: Invalid Domain Config Blocks Deploy

```bash
# Setup: domain.json has invalid stream reference
# Expected: Deployment aborted

$ ./deploy.sh
[DEPLOY] Running pre-deployment validation...
[DEPLOY] Validating stream configurations...
[DEPLOY]   Validating: air-quality
    [PASS]
[DEPLOY] Validating domain configurations...
[DEPLOY]   Validating: indoor-air-quality
    [FAIL] (1 errors)
[ERROR] Validation failed. Fix errors before deployment.
# Exit code: 1
```

### Test 3: Skip Validation Flag Works

```bash
# Setup: Invalid configs but skip validation
# Expected: Deployment proceeds (with warning)

$ SKIP_VALIDATION=1 ./deploy.sh
[DEPLOY] Starting full deployment...
[WARN] Skipping validation (SKIP_VALIDATION=1)
# ... deployment continues ...
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Discover configs | O(d) | O(d) |
| Validate single | O(v) | O(e) |
| Cross-validate | O(d * s) | O(s) |
| Total validation | O(d * (v + s)) | O(d + s + e) |

Where:
- d = number of domains
- s = number of streams
- v = single validation time
- e = error count

---

## References

- **FE-002 SCOPE.md**: AC-B7 (deploy.sh validates domain configs)
- **deploy.sh**: Current deployment script
- **ndp-validate**: Validation tool
- **ALGO-003**: Schema validation pipeline
