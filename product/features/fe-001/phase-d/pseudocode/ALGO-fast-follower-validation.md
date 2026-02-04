# ALGO-fast-follower-validation: Step-by-Step Validation Procedure

> **Algorithm ID:** D01
> **Feature:** v11-V01 (Fast-Follower Stream Test)
> **Phase:** D (Validation + Dashboard)
> **Created:** 2026-02-04

---

## Purpose

Define the algorithmic validation procedure for the fast-follower test. This algorithm verifies that a new stream (`outdoor-air-quality`) can be added to the Gold layer using **only configuration changes** with zero Rust code modifications, completing in under 1 hour.

---

## Algorithm: ExecuteFastFollowerTest

```
ALGORITHM: ExecuteFastFollowerTest
INPUT:
    stream_id: String                      // "outdoor-air-quality"
    domain_id: String                      // "indoor-air-quality"
    timer: Stopwatch
    config_dir: Path
    deploy_tool: DeployTool
OUTPUT: Result<TestReport, TestError>
REQUIRES:
    - Silver table exists for stream_id
    - Phase A-C architecture complete
    - ndp-gold-ddl tool operational
    - 3 streams already in Gold layer

BEGIN
    timer.start()
    report <- TestReport::new(stream_id, domain_id)

    // ============================================
    // CHECKPOINT 1: Pre-Test Verification (Target: 0 min)
    // ============================================
    precheck_result <- VerifyPreConditions(stream_id)?
    report.add_checkpoint("pre-conditions", timer.elapsed())

    // ============================================
    // CHECKPOINT 2: Documentation Review (Target: 10 min)
    // ============================================
    doc_result <- ReviewDocumentation(stream_id, config_dir)?
    report.add_checkpoint("documentation", timer.elapsed())
    report.available_fields <- doc_result.silver_columns

    // ============================================
    // CHECKPOINT 3: Create gold_etl Config (Target: 25 min)
    // ============================================
    config_result <- CreateGoldEtlConfig(stream_id, config_dir)?
    report.add_checkpoint("gold_etl_config", timer.elapsed())
    report.config_files_modified.push(config_result.config_path)

    // ============================================
    // CHECKPOINT 4: Update Domain Config (Target: 35 min)
    // ============================================
    domain_result <- UpdateDomainConfig(domain_id, stream_id, config_dir)?
    report.add_checkpoint("domain_config", timer.elapsed())
    report.config_files_modified.push(domain_result.config_path)

    // ============================================
    // CHECKPOINT 5: Create Manifest (Target: 40 min)
    // ============================================
    manifest_result <- CreateDeployManifest(stream_id, domain_id)?
    report.add_checkpoint("manifest", timer.elapsed())
    report.config_files_modified.push(manifest_result.manifest_path)

    // ============================================
    // CHECKPOINT 6: Run Deployment (Target: 45 min)
    // ============================================
    deploy_result <- RunDeployment(manifest_result.manifest_path, deploy_tool)?
    report.add_checkpoint("deployment", timer.elapsed())
    report.deployment_output <- deploy_result.output

    // ============================================
    // CHECKPOINT 7: Verification (Target: 55 min)
    // ============================================
    verification_result <- VerifyDeployment(stream_id, domain_id)?
    report.add_checkpoint("verification", timer.elapsed())
    report.verification <- verification_result

    // ============================================
    // CHECKPOINT 8: Code Change Verification (Target: 60 min)
    // ============================================
    code_result <- VerifyNoCodeChanges()?
    report.add_checkpoint("code_check", timer.elapsed())
    report.code_changes <- code_result

    // ============================================
    // FINAL: Generate Report
    // ============================================
    timer.stop()
    report.total_time <- timer.elapsed()
    report.passed <- EvaluateTestResult(report)

    RETURN Ok(report)
END
```

---

## Algorithm: VerifyPreConditions

```
ALGORITHM: VerifyPreConditions
INPUT: stream_id: String
OUTPUT: Result<PreCheckResult, TestError>

BEGIN
    result <- PreCheckResult::new()

    // 1. Verify Silver table exists and has data
    silver_check <- QueryDatabase(format!(
        "SELECT COUNT(*) as row_count,
                MIN(observation_time) as earliest,
                MAX(observation_time) as latest
         FROM silver.{}", stream_id.replace("-", "_")
    ))

    IF silver_check.row_count == 0 THEN
        RETURN Err(TestError::PreConditionFailed {
            check: "silver_table_has_data",
            message: format!("Silver table for {} is empty", stream_id)
        })
    END IF

    result.silver_row_count <- silver_check.row_count
    result.silver_earliest <- silver_check.earliest
    result.silver_latest <- silver_check.latest

    // 2. Verify 3 streams already in Gold
    gold_check <- QueryDatabase(
        "SELECT view_name FROM timescaledb_information.continuous_aggregates
         WHERE view_schema = 'gold'"
    )

    IF gold_check.row_count < 3 THEN
        RETURN Err(TestError::PreConditionFailed {
            check: "existing_gold_streams",
            message: format!("Expected at least 3 Gold streams, found {}", gold_check.row_count)
        })
    END IF

    result.existing_gold_views <- gold_check.rows

    // 3. Verify ndp-gold-ddl works
    tool_check <- ExecuteCommand("ndp-gold-ddl --version")
    IF tool_check.exit_code != 0 THEN
        RETURN Err(TestError::PreConditionFailed {
            check: "ndp_gold_ddl",
            message: "ndp-gold-ddl not available"
        })
    END IF

    result.tool_version <- tool_check.output

    // 4. Verify current config has NO gold_etl section
    config_check <- ReadJsonFile(format!(
        "config/base/streams/{}/config.json", stream_id
    ))

    IF config_check.gold_etl IS Some THEN
        RETURN Err(TestError::PreConditionFailed {
            check: "no_existing_gold_etl",
            message: format!("{} already has gold_etl config", stream_id)
        })
    END IF

    // 5. Verify clean git state
    git_check <- ExecuteCommand("git status --porcelain")
    result.git_clean <- git_check.output.is_empty()

    RETURN Ok(result)
END
```

---

## Algorithm: ReviewDocumentation

```
ALGORITHM: ReviewDocumentation
INPUT:
    stream_id: String
    config_dir: Path
OUTPUT: Result<DocReviewResult, TestError>

BEGIN
    result <- DocReviewResult::new()

    // 1. Read air-quality gold_etl as reference
    reference_config <- ReadJsonFile(format!(
        "{}/base/streams/air-quality/config.json", config_dir
    ))

    result.reference_gold_etl <- reference_config.gold_etl

    // 2. Read domain config structure
    domain_config <- ReadYamlFile(format!(
        "{}/domains/indoor-air-quality/domain.yaml", config_dir
    ))

    result.domain_structure <- domain_config

    // 3. Get available Silver columns for target stream
    silver_columns <- QueryDatabase(format!(
        "SELECT column_name, data_type
         FROM information_schema.columns
         WHERE table_schema = 'silver' AND table_name = '{}'
         ORDER BY ordinal_position",
        stream_id.replace("-", "_")
    ))

    result.silver_columns <- silver_columns.rows
    result.numeric_columns <- silver_columns.rows
        .filter(|c| c.data_type IN ["double precision", "numeric", "integer", "smallint"])
        .map(|c| c.column_name)

    RETURN Ok(result)
END
```

---

## Algorithm: CreateGoldEtlConfig

```
ALGORITHM: CreateGoldEtlConfig
INPUT:
    stream_id: String
    config_dir: Path
OUTPUT: Result<ConfigCreationResult, TestError>

BEGIN
    config_path <- format!("{}/base/streams/{}/config.json", config_dir, stream_id)

    // 1. Read existing config
    existing_config <- ReadJsonFile(config_path)?

    // 2. Build gold_etl section based on available fields
    // For outdoor-air-quality, we know the fields from documentation review
    gold_etl <- GoldEtlConfig {
        enabled: true,
        description: "Gold layer hourly aggregates for outdoor air quality",
        aggregates: AggregatesConfig {
            granularities: ["1 hour"],
            entity_column: "ndp_id",
            fields: {
                "pm25": FieldConfig { metrics: ["mean", "std", "min", "max", "p95"] },
                "pm10": FieldConfig { metrics: ["mean", "std", "min", "max"] },
                "aqi_owm": FieldConfig { metrics: ["mean", "min", "max"] },
                "aqi_epa": FieldConfig { metrics: ["mean", "min", "max"] },
                "o3_ugm3": FieldConfig { metrics: ["mean", "max"] },
                "no2_ugm3": FieldConfig { metrics: ["mean", "max"] }
            }
        },
        features: FeaturesConfig {
            lag: LagConfig {
                enabled: true,
                lags_hours: [1, 6, 24],
                fields: ["pm25", "aqi_epa"]
            },
            rolling: RollingConfig {
                enabled: true,
                windows: ["4 hours", "24 hours"],
                stats: ["mean", "std"],
                fields: ["pm25"]
            }
        },
        refresh_policy: RefreshPolicyConfig {
            schedule_interval: "15 minutes",
            start_offset: "4 hours",
            end_offset: "15 minutes"
        }
    }

    // 3. Add stream_type if not present
    IF existing_config.stream_type IS None THEN
        existing_config.stream_type <- "observation"
    END IF

    // 4. Add gold_etl to config
    existing_config.gold_etl <- gold_etl

    // 5. Write updated config
    WriteJsonFile(config_path, existing_config)?

    // 6. Validate config
    validation <- ExecuteCommand(format!(
        "ndp-gold-ddl validate --stream {}", stream_id
    ))

    IF validation.exit_code != 0 THEN
        RETURN Err(TestError::ConfigValidationFailed {
            stream_id: stream_id,
            output: validation.output
        })
    END IF

    RETURN Ok(ConfigCreationResult {
        config_path: config_path,
        validation_output: validation.output
    })
END
```

---

## Algorithm: UpdateDomainConfig

```
ALGORITHM: UpdateDomainConfig
INPUT:
    domain_id: String
    stream_id: String
    config_dir: Path
OUTPUT: Result<DomainUpdateResult, TestError>

BEGIN
    config_path <- format!("{}/domains/{}/domain.yaml", config_dir, domain_id)

    // 1. Read existing domain config
    domain_config <- ReadYamlFile(config_path)?

    // 2. Add new stream reference
    new_stream_ref <- StreamReference {
        stream_id: stream_id,
        alias: "outdoor_aqi",
        role: "constraint"
    }

    // Check if stream already in domain
    IF domain_config.streams.any(|s| s.stream_id == stream_id) THEN
        RETURN Err(TestError::StreamAlreadyInDomain {
            domain_id: domain_id,
            stream_id: stream_id
        })
    END IF

    domain_config.streams.push(new_stream_ref)

    // 3. Write updated config
    WriteYamlFile(config_path, domain_config)?

    // 4. Validate domain config
    validation <- ExecuteCommand(format!(
        "ndp-gold-ddl validate --domain {}", domain_id
    ))

    IF validation.exit_code != 0 THEN
        RETURN Err(TestError::DomainValidationFailed {
            domain_id: domain_id,
            output: validation.output
        })
    END IF

    RETURN Ok(DomainUpdateResult {
        config_path: config_path,
        streams_count: domain_config.streams.len(),
        validation_output: validation.output
    })
END
```

---

## Algorithm: CreateDeployManifest

```
ALGORITHM: CreateDeployManifest
INPUT:
    stream_id: String
    domain_id: String
OUTPUT: Result<ManifestCreationResult, TestError>

BEGIN
    manifest_path <- ".deploy/test/phase-d-fast-follower.manifest.json"

    manifest <- DeployManifest {
        version: "1.1.0-test",
        description: "Phase D Fast-Follower Test - Add outdoor-air-quality to Gold",
        created: GetCurrentDate(),
        declarations: Declarations {
            etcd_config: [
                EtcdConfigDecl {
                    stream_id: stream_id,
                    path: format!("config/base/streams/{}/config.json", stream_id)
                }
            ],
            gold_tables: [
                GoldTableDecl {
                    stream_id: stream_id,
                    action: "sync"
                }
            ],
            domains: [
                DomainDecl {
                    domain_id: domain_id,
                    action: "recreate"  // Recreate to pick up new stream
                }
            ]
        }
    }

    WriteJsonFile(manifest_path, manifest)?

    RETURN Ok(ManifestCreationResult {
        manifest_path: manifest_path
    })
END
```

---

## Algorithm: RunDeployment

```
ALGORITHM: RunDeployment
INPUT:
    manifest_path: Path
    deploy_tool: DeployTool
OUTPUT: Result<DeploymentResult, TestError>

BEGIN
    // 1. Sync config to etcd (if applicable)
    sync_result <- ExecuteCommand(format!(
        "./scripts/sync-streams-to-etcd.sh outdoor-air-quality"
    ))
    // Note: This may be optional depending on setup

    // 2. Run deployment
    deploy_result <- ExecuteCommand(format!(
        "./deploy/pi/deploy.sh apply {}", manifest_path
    ))

    IF deploy_result.exit_code != 0 THEN
        RETURN Err(TestError::DeploymentFailed {
            manifest: manifest_path,
            output: deploy_result.output,
            exit_code: deploy_result.exit_code
        })
    END IF

    RETURN Ok(DeploymentResult {
        output: deploy_result.output,
        exit_code: deploy_result.exit_code,
        duration_ms: deploy_result.duration_ms
    })
END
```

---

## Algorithm: VerifyDeployment

```
ALGORITHM: VerifyDeployment
INPUT:
    stream_id: String
    domain_id: String
OUTPUT: Result<VerificationResult, TestError>

BEGIN
    result <- VerificationResult::new()

    // 1. Verify continuous aggregate exists
    agg_check <- QueryDatabase(format!(
        "SELECT view_name FROM timescaledb_information.continuous_aggregates
         WHERE view_schema = 'gold' AND view_name = '{}_hourly'",
        stream_id.replace("-", "_")
    ))

    result.aggregate_exists <- agg_check.row_count > 0
    IF NOT result.aggregate_exists THEN
        result.failures.push("Continuous aggregate not found")
    END IF

    // 2. Verify data exists in aggregate
    data_check <- QueryDatabase(format!(
        "SELECT COUNT(*), MIN(bucket), MAX(bucket)
         FROM gold.{}_hourly",
        stream_id.replace("-", "_")
    ))

    result.aggregate_row_count <- data_check.count
    result.aggregate_earliest <- data_check.min_bucket
    result.aggregate_latest <- data_check.max_bucket

    IF result.aggregate_row_count == 0 THEN
        result.failures.push("Continuous aggregate has no data")
    END IF

    // 3. Verify aligned view has new columns
    column_check <- QueryDatabase(format!(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = 'gold'
           AND table_name = '{}_aligned'
           AND column_name LIKE 'outdoor_aqi%'
         ORDER BY column_name",
        domain_id.replace("-", "_")
    ))

    result.aligned_columns <- column_check.rows.map(|r| r.column_name)
    IF result.aligned_columns.is_empty() THEN
        result.failures.push("Aligned view missing outdoor_aqi columns")
    END IF

    // 4. Verify query performance
    perf_check <- QueryDatabase(format!(
        "EXPLAIN ANALYZE
         SELECT * FROM gold.{}_aligned
         WHERE bucket >= NOW() - INTERVAL '30 days'",
        domain_id.replace("-", "_")
    ))

    result.query_plan <- perf_check.output
    result.execution_time_ms <- ExtractExecutionTime(perf_check.output)

    IF result.execution_time_ms > 100 THEN
        result.failures.push(format!(
            "Query too slow: {}ms (target: <100ms)",
            result.execution_time_ms
        ))
    END IF

    // 5. Verify refresh policy exists
    policy_check <- QueryDatabase(
        "SELECT job_id, schedule_interval
         FROM timescaledb_information.jobs
         WHERE proc_name = 'policy_refresh_continuous_aggregate'"
    )

    result.refresh_policies <- policy_check.rows

    // 6. Verify data dictionary populated
    dict_check <- QueryDatabase(format!(
        "SELECT * FROM data_dictionary.gold_tables
         WHERE table_name LIKE '%{}%'",
        stream_id.replace("-", "_")
    ))

    result.data_dictionary_entries <- dict_check.row_count
    IF result.data_dictionary_entries == 0 THEN
        result.failures.push("Data dictionary not populated")
    END IF

    RETURN Ok(result)
END
```

---

## Algorithm: VerifyNoCodeChanges

```
ALGORITHM: VerifyNoCodeChanges
INPUT: None
OUTPUT: Result<CodeChangeResult, TestError>

BEGIN
    result <- CodeChangeResult::new()

    // 1. Get git diff statistics
    diff_result <- ExecuteCommand("git diff --stat")
    result.diff_output <- diff_result.output

    // 2. Check for Rust file changes
    rust_diff <- ExecuteCommand("git diff --stat -- '*.rs'")
    result.rust_files_changed <- CountFilesInDiff(rust_diff.output)

    IF result.rust_files_changed > 0 THEN
        result.failures.push(format!(
            "Rust files changed: {} (target: 0)",
            result.rust_files_changed
        ))
    END IF

    // 3. Check for shell script changes
    shell_diff <- ExecuteCommand("git diff --stat -- '*.sh'")
    result.shell_files_changed <- CountFilesInDiff(shell_diff.output)

    IF result.shell_files_changed > 0 THEN
        result.failures.push(format!(
            "Shell files changed: {} (target: 0)",
            result.shell_files_changed
        ))
    END IF

    // 4. Check for Python file changes
    python_diff <- ExecuteCommand("git diff --stat -- '*.py'")
    result.python_files_changed <- CountFilesInDiff(python_diff.output)

    IF result.python_files_changed > 0 THEN
        result.failures.push(format!(
            "Python files changed: {} (target: 0)",
            result.python_files_changed
        ))
    END IF

    // 5. Verify only config files changed
    config_diff <- ExecuteCommand("git diff --stat -- '*.json' '*.yaml' '*.yml'")
    result.config_files_changed <- CountFilesInDiff(config_diff.output)

    // 6. List modified files
    name_only <- ExecuteCommand("git diff --name-only")
    result.modified_files <- name_only.output.lines().collect()

    // 7. Determine pass/fail
    result.passed <- result.rust_files_changed == 0
                  AND result.shell_files_changed == 0
                  AND result.python_files_changed == 0
                  AND result.config_files_changed >= 2
                  AND result.config_files_changed <= 4

    RETURN Ok(result)
END
```

---

## Algorithm: EvaluateTestResult

```
ALGORITHM: EvaluateTestResult
INPUT: report: TestReport
OUTPUT: bool

BEGIN
    // Check total time
    IF report.total_time > Duration::from_minutes(60) THEN
        RETURN FALSE
    END IF

    // Check all checkpoints passed
    FOR EACH checkpoint IN report.checkpoints DO
        IF checkpoint.failed THEN
            RETURN FALSE
        END IF
    END FOR

    // Check verification passed
    IF NOT report.verification.failures.is_empty() THEN
        RETURN FALSE
    END IF

    // Check code changes verification passed
    IF NOT report.code_changes.passed THEN
        RETURN FALSE
    END IF

    RETURN TRUE
END
```

---

## Data Types

```
STRUCT TestReport:
    stream_id: String
    domain_id: String
    total_time: Duration
    checkpoints: Vec<Checkpoint>
    config_files_modified: Vec<Path>
    deployment_output: String
    verification: VerificationResult
    code_changes: CodeChangeResult
    passed: bool

STRUCT Checkpoint:
    name: String
    elapsed_time: Duration
    target_time: Duration
    failed: bool
    message: Option<String>

STRUCT VerificationResult:
    aggregate_exists: bool
    aggregate_row_count: i64
    aligned_columns: Vec<String>
    execution_time_ms: f64
    refresh_policies: Vec<Policy>
    data_dictionary_entries: i64
    failures: Vec<String>

STRUCT CodeChangeResult:
    rust_files_changed: i32
    shell_files_changed: i32
    python_files_changed: i32
    config_files_changed: i32
    modified_files: Vec<String>
    passed: bool
    failures: Vec<String>
```

---

## Success Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| Total time | < 60 minutes | Stopwatch |
| Rust code changes | 0 | `git diff --stat -- '*.rs'` |
| Shell script changes | 0 | `git diff --stat -- '*.sh'` |
| Config files changed | 2-4 | `git diff --stat -- '*.json' '*.yaml'` |
| Aggregate exists | TRUE | TimescaleDB query |
| Aggregate has data | > 0 rows | Count query |
| Aligned view updated | outdoor_aqi columns exist | Schema query |
| Query performance | < 100ms | EXPLAIN ANALYZE |
| Refresh policy active | TRUE | Jobs table query |
| Data dictionary populated | > 0 entries | Dictionary query |

---

## Test Cases (London TDD)

```
TRAITS TO MOCK:
    - DatabaseExecutor: Return mock query results
    - CommandExecutor: Return mock command outputs
    - FileSystem: Track read/write operations

TEST: PreConditionsVerifySilverData
    GIVEN Silver table with 100 rows
    WHEN VerifyPreConditions() is called
    THEN result.silver_row_count = 100

TEST: PreConditionsFailOnEmptySilver
    GIVEN Silver table with 0 rows
    WHEN VerifyPreConditions() is called
    THEN Err(PreConditionFailed) with "Silver table is empty"

TEST: CreateGoldEtlConfigValidates
    GIVEN valid stream config
    WHEN CreateGoldEtlConfig() is called
    THEN ndp-gold-ddl validate is executed
    AND config is written

TEST: DeploymentVerifiesAllChecks
    GIVEN successful deployment
    WHEN VerifyDeployment() is called
    THEN all 6 verification steps complete
    AND failures is empty

TEST: CodeChangeVerificationCatchesRustChanges
    GIVEN git diff shows 1 .rs file changed
    WHEN VerifyNoCodeChanges() is called
    THEN result.rust_files_changed = 1
    AND result.passed = FALSE

TEST: TestPassesUnder60Minutes
    GIVEN all checkpoints pass
    AND total_time = 45 minutes
    WHEN EvaluateTestResult() is called
    THEN result = TRUE

TEST: TestFailsOver60Minutes
    GIVEN all checkpoints pass
    AND total_time = 65 minutes
    WHEN EvaluateTestResult() is called
    THEN result = FALSE
```

---

## References

- [SPEC-D01-fast-follower-test.md](../specification/SPEC-D01-fast-follower-test.md)
- [SCOPE.md](../../SCOPE.md) - V1.1 scope definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
