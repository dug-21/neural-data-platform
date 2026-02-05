# FE-002: Domain Configuration Standardization - Test Plan

> **Feature ID:** FE-002
> **Version:** 1.0
> **Created:** 2026-02-05
> **Parent Documents:** [TEST-STRATEGY.md](./TEST-STRATEGY.md), [TDD-GUIDE.md](./TDD-GUIDE.md)

---

## 1. Test Plan Overview

This document provides a detailed test plan for FE-002, organized by phase and test category.

### 1.1 Test Categories

| Category | Purpose | Location | When Run |
|----------|---------|----------|----------|
| **Golden Master** | Verify DDL output unchanged | `tests/golden_master_test.rs` | Every build |
| **Unit Tests** | Test individual components | `src/**/*.rs` (inline) | Every build |
| **Integration Tests** | Test component interactions | `tests/integration/` | CI/CD |
| **Manual Verification** | Human validation | N/A | Pre-merge |

### 1.2 Test Counts by Phase

| Phase | Golden Master | Unit | Integration | Manual | Total |
|-------|---------------|------|-------------|--------|-------|
| **0: Baseline** | 12 | 0 | 0 | 2 | 14 |
| **A: Migration** | 12 | 15 | 2 | 3 | 32 |
| **B: Validation** | 12 | 40 | 5 | 3 | 60 |
| **Total** | 12 | 55 | 7 | 8 | 106 |

---

## 2. Phase 0: Baseline Capture Tests

### 2.1 Golden Master Baseline Tests

| Test ID | Test Name | Input | Expected Output | Priority |
|---------|-----------|-------|-----------------|----------|
| GM-001 | `golden_master_domain_sync` | `--domain indoor-air-quality --action sync` | Matches `domain_indoor-air-quality_sync.sql` | Critical |
| GM-002 | `golden_master_domain_recreate` | `--domain indoor-air-quality --action recreate` | Matches `domain_indoor-air-quality_recreate.sql` | Critical |
| GM-003 | `golden_master_stream_air_quality_sync` | `--stream air-quality --action sync` | Matches baseline | Critical |
| GM-004 | `golden_master_stream_air_quality_recreate` | `--stream air-quality --action recreate` | Matches baseline | Critical |
| GM-005 | `golden_master_stream_outdoor_weather_sync` | `--stream outdoor-weather --action sync` | Matches baseline | High |
| GM-006 | `golden_master_stream_outdoor_weather_recreate` | `--stream outdoor-weather --action recreate` | Matches baseline | High |
| GM-007 | `golden_master_stream_home_assistant_state_sync` | `--stream home-assistant-state --action sync` | Matches baseline | High |
| GM-008 | `golden_master_stream_home_assistant_state_recreate` | `--stream home-assistant-state --action recreate` | Matches baseline | High |
| GM-009 | `golden_master_stream_outdoor_aqi_sync` | `--stream outdoor-air-quality --action sync` | Matches baseline | High |
| GM-010 | `golden_master_stream_outdoor_aqi_recreate` | `--stream outdoor-air-quality --action recreate` | Matches baseline | High |
| GM-011 | `golden_master_transitions_sync` | `--stream home-assistant-state --transitions --action sync` | Matches baseline | High |
| GM-012 | `golden_master_transitions_recreate` | `--stream home-assistant-state --transitions --action recreate` | Matches baseline | High |

### 2.2 Manual Verification Tests

| Test ID | Test Name | Steps | Expected | Priority |
|---------|-----------|-------|----------|----------|
| MV-001 | Verify baseline fixtures committed | 1. Check git status after capture | All `.sql` files in `tests/fixtures/golden-master/` committed | Critical |
| MV-002 | Verify checksums match | 1. Run `sha256sum -c CHECKSUMS.sha256` | All files OK | Critical |

---

## 3. Phase A: YAML to JSON Migration Tests

### 3.1 Golden Master Tests (Same as Phase 0)

All 12 golden master tests from Phase 0 MUST pass throughout Phase A. These are the acceptance criteria.

### 3.2 Unit Tests - Config Loader

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UA-001 | `test_load_domain_config_from_json` | Load valid JSON domain config | Config loaded with correct fields |
| UA-002 | `test_load_domain_config_json_not_found` | Load nonexistent JSON file | `ConfigNotFound` error with path |
| UA-003 | `test_load_domain_config_json_parse_error` | Load invalid JSON file | `ConfigParseError` with details |
| UA-004 | `test_domain_config_path_returns_json_extension` | Check config path | Path ends with `.json` |
| UA-005 | `test_load_domain_config_preserves_stream_order` | Load config with multiple streams | Streams in original order |
| UA-006 | `test_load_domain_config_handles_null_handling` | Load config with null_handling | `null_handling` field parsed correctly |
| UA-007 | `test_load_domain_config_handles_objectives` | Load config with objectives | All objectives parsed |
| UA-008 | `test_load_domain_config_handles_alignment` | Load config with alignment | Alignment config correct |

### 3.3 Unit Tests - JSON Parsing Edge Cases

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UA-009 | `test_json_preserves_string_escapes` | JSON with escaped characters | Strings preserved correctly |
| UA-010 | `test_json_handles_unicode` | JSON with unicode characters | Unicode preserved |
| UA-011 | `test_json_numeric_precision` | JSON with decimal numbers | Precision maintained |
| UA-012 | `test_json_empty_arrays` | JSON with `[]` values | Empty arrays handled |
| UA-013 | `test_json_optional_fields_absent` | JSON missing optional fields | Defaults applied |
| UA-014 | `test_json_extra_fields_ignored` | JSON with extra fields | No errors, extras ignored |
| UA-015 | `test_json_field_ordering` | JSON with fields in different order | All fields parsed |

### 3.4 Integration Tests - Phase A

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| IA-001 | `test_end_to_end_domain_generate_with_json` | Full CLI execution with JSON | DDL generated, matches baseline |
| IA-002 | `test_validate_command_with_domain_json` | `validate --domain` with JSON | Validation passes |

### 3.5 Manual Verification Tests - Phase A

| Test ID | Test Name | Steps | Expected |
|---------|-----------|-------|----------|
| MA-001 | Verify domain.yaml deleted | 1. Check file system | `domain.yaml` does not exist |
| MA-002 | Verify domain.json created | 1. `jq . domain.json` | Valid JSON output |
| MA-003 | Verify serde_yaml removed | 1. `grep serde_yaml Cargo.toml` | No matches |

---

## 4. Phase B: Schema Validation Tests

### 4.1 Golden Master Tests (Unchanged)

All 12 golden master tests continue to pass. Phase B adds NEW features but does not change DDL generation.

### 4.2 Unit Tests - CLI

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UB-001 | `test_cli_domain_flag_accepted` | CLI parses `--domain` | No parse error |
| UB-002 | `test_cli_domain_conflicts_with_stream` | `--domain` and `--stream` both given | Error message |
| UB-003 | `test_cli_all_domain_flag_accepted` | CLI parses `--all --domain` | Both flags work |
| UB-004 | `test_cli_schema_only_flag_accepted` | CLI parses `--schema-only` | Flag parsed |
| UB-005 | `test_cli_help_shows_domain_option` | `--help` output | Shows `--domain` with description |

### 4.3 Unit Tests - Schema Validation

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UB-006 | `test_valid_domain_passes_schema` | Valid domain config | Validation passes |
| UB-007 | `test_missing_id_fails_schema` | Config without `id` | Error mentions `id` required |
| UB-008 | `test_missing_streams_fails_schema` | Config without `streams` | Error mentions `streams` |
| UB-009 | `test_missing_alignment_fails_schema` | Config without `alignment` | Error mentions `alignment` |
| UB-010 | `test_invalid_role_fails_schema` | Stream with bad `role` | Error mentions enum |
| UB-011 | `test_invalid_join_strategy_fails_schema` | Bad `join_strategy` | Error mentions enum |
| UB-012 | `test_invalid_null_handling_fails_schema` | Bad `null_handling` | Error mentions enum |
| UB-013 | `test_invalid_granularity_pattern_fails_schema` | Bad granularity format | Pattern validation fails |
| UB-014 | `test_all_valid_roles_pass_schema` | Each valid role | All pass |
| UB-015 | `test_all_valid_join_strategies_pass_schema` | Each valid strategy | All pass |
| UB-016 | `test_all_valid_granularities_pass_schema` | Each valid granularity | All pass |
| UB-017 | `test_schema_error_includes_json_path` | Invalid nested field | Error includes `$.alignment.view_name` |
| UB-018 | `test_schema_error_is_actionable` | Any validation error | Error suggests fix |

### 4.4 Unit Tests - Semantic Validation

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UB-019 | `test_domain_requires_two_streams` | Config with 1 stream | Error: minimum 2 streams |
| UB-020 | `test_domain_requires_primary_stream` | No primary role | Error: need primary |
| UB-021 | `test_domain_allows_single_primary` | Multiple primary streams | Error: only one primary |
| UB-022 | `test_aliases_must_be_unique` | Duplicate aliases | Error: duplicate alias |
| UB-023 | `test_stream_ids_must_exist` | Reference to missing stream | Error: stream not found |
| UB-024 | `test_view_name_must_be_valid_sql` | Invalid view name | Error: invalid identifier |
| UB-025 | `test_objectives_reference_valid_streams` | Objective references missing stream | Error: stream not in domain |
| UB-026 | `test_objectives_reference_valid_metrics` | Objective references missing metric | Error: metric not in stream |
| UB-027 | `test_threshold_must_be_numeric` | String threshold | Error: must be number |
| UB-028 | `test_condition_must_be_valid` | Invalid condition | Error: invalid condition |
| UB-029 | `test_priority_must_be_valid` | Invalid priority | Error: invalid priority |
| UB-030 | `test_semantic_runs_after_schema_passes` | Valid schema, bad semantic | Schema passes, semantic fails |
| UB-031 | `test_semantic_skipped_if_schema_fails` | Invalid schema | Only schema error reported |

### 4.5 Unit Tests - Error Formatting

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UB-032 | `test_schema_error_formatted_clearly` | Schema validation error | Human-readable format |
| UB-033 | `test_semantic_error_formatted_clearly` | Semantic validation error | Human-readable format |
| UB-034 | `test_multiple_errors_all_reported` | Multiple schema errors | All errors listed |
| UB-035 | `test_error_includes_file_path` | Any error | Includes config file path |
| UB-036 | `test_error_exit_code_nonzero` | Validation fails | Exit code 1 |
| UB-037 | `test_success_exit_code_zero` | Validation passes | Exit code 0 |
| UB-038 | `test_success_message_confirms_file` | Valid config | "Config X is valid" message |

### 4.6 Unit Tests - All Domains Validation

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UB-039 | `test_all_domains_finds_configs` | `--all --domain` | Discovers all domain.json files |
| UB-040 | `test_all_domains_validates_each` | Multiple domains | Each validated |
| UB-041 | `test_all_domains_reports_all_errors` | Some invalid | All errors reported |
| UB-042 | `test_all_domains_success_when_all_valid` | All valid | Exit code 0, summary |
| UB-043 | `test_all_domains_fails_when_any_invalid` | Any invalid | Exit code 1 |

### 4.7 Unit Tests - Deploy Integration

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| UB-044 | `test_deploy_calls_domain_validation` | deploy.sh with domain | Validation called |
| UB-045 | `test_deploy_stops_on_validation_failure` | Invalid domain config | Deploy aborted |

### 4.8 Integration Tests - Phase B

| Test ID | Test Name | Description | Expected Result |
|---------|-----------|-------------|-----------------|
| IB-001 | `test_cli_validates_real_domain_config` | CLI with actual domain.json | Passes validation |
| IB-002 | `test_cli_rejects_invalid_domain` | CLI with crafted invalid config | Fails with error |
| IB-003 | `test_cli_all_domains_in_repo` | `--all --domain` on repo | All pass |
| IB-004 | `test_schema_only_skips_semantic` | `--schema-only` with semantic issue | Passes |
| IB-005 | `test_validation_before_ddl_generation` | Invalid config then generate | Validation blocks generation |

### 4.9 Manual Verification Tests - Phase B

| Test ID | Test Name | Steps | Expected |
|---------|-----------|-------|----------|
| MB-001 | Verify IDE autocomplete | 1. Open domain.json in VS Code 2. Type field | Autocomplete suggestions appear |
| MB-002 | Verify deploy.sh integration | 1. Run deploy.sh with invalid domain | Deploy aborts with validation error |
| MB-003 | Verify error messages actionable | 1. Create config with wrong role | Error message tells how to fix |

---

## 5. Test Data and Fixtures

### 5.1 Golden Master Fixtures

| Fixture | Location | Purpose |
|---------|----------|---------|
| `domain_indoor-air-quality_sync.sql` | `tests/fixtures/golden-master/` | Baseline for sync mode |
| `domain_indoor-air-quality_recreate.sql` | `tests/fixtures/golden-master/` | Baseline for recreate mode |
| `stream_air-quality_sync.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_air-quality_recreate.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_outdoor-weather_sync.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_outdoor-weather_recreate.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_home-assistant-state_sync.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_home-assistant-state_recreate.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_outdoor-air-quality_sync.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_outdoor-air-quality_recreate.sql` | `tests/fixtures/golden-master/` | Stream baseline |
| `stream_home-assistant-state_transitions_sync.sql` | `tests/fixtures/golden-master/` | Transitions baseline |
| `stream_home-assistant-state_transitions_recreate.sql` | `tests/fixtures/golden-master/` | Transitions baseline |
| `CHECKSUMS.sha256` | `tests/fixtures/golden-master/` | Integrity verification |

### 5.2 Schema Validation Test Fixtures

| Fixture | Location | Purpose |
|---------|----------|---------|
| `valid_minimal.json` | `tests/fixtures/configs/valid/` | Minimal valid domain |
| `valid_full.json` | `tests/fixtures/configs/valid/` | All fields populated |
| `valid_objectives.json` | `tests/fixtures/configs/valid/` | With objectives |
| `invalid_missing_id.json` | `tests/fixtures/configs/invalid/` | Missing required field |
| `invalid_bad_role.json` | `tests/fixtures/configs/invalid/` | Invalid enum value |
| `invalid_single_stream.json` | `tests/fixtures/configs/invalid/` | Semantic: < 2 streams |
| `invalid_no_primary.json` | `tests/fixtures/configs/invalid/` | Semantic: no primary |
| `invalid_duplicate_alias.json` | `tests/fixtures/configs/invalid/` | Semantic: duplicate aliases |

### 5.3 Fixture Helpers

```rust
// Location: tests/fixtures/mod.rs

pub fn load_valid_config(name: &str) -> Value {
    let path = format!("tests/fixtures/configs/valid/{}.json", name);
    let content = fs::read_to_string(&path)
        .expect(&format!("Fixture not found: {}", path));
    serde_json::from_str(&content)
        .expect(&format!("Invalid JSON in fixture: {}", path))
}

pub fn load_invalid_config(name: &str) -> Value {
    let path = format!("tests/fixtures/configs/invalid/{}.json", name);
    let content = fs::read_to_string(&path)
        .expect(&format!("Fixture not found: {}", path));
    serde_json::from_str(&content)
        .expect(&format!("Invalid JSON in fixture: {}", path))
}

pub fn create_temp_domain(config: &Value) -> PathBuf {
    let temp_dir = TempDir::new().unwrap();
    let domain_dir = temp_dir.path().join("domains").join("temp-domain");
    fs::create_dir_all(&domain_dir).unwrap();
    fs::write(
        domain_dir.join("domain.json"),
        serde_json::to_string_pretty(config).unwrap()
    ).unwrap();
    domain_dir.join("domain.json")
}
```

---

## 6. Test Execution Matrix

### 6.1 Local Development

| Command | Tests Run | When to Use |
|---------|-----------|-------------|
| `cargo test -p ndp-gold-ddl --test golden_master_test` | Golden Master only | After any config change |
| `cargo test -p ndp-gold-ddl --lib` | Unit tests only | During development |
| `cargo test -p ndp-gold-ddl` | All ndp-gold-ddl tests | Before commit |
| `cargo test -p ndp-validate --lib` | Validate unit tests | During Phase B |
| `cargo test -p ndp-validate` | All validate tests | Before commit |

### 6.2 CI/CD Pipeline

| Stage | Tests | Blocking |
|-------|-------|----------|
| Build | Compilation | Yes |
| Unit Tests | All unit tests | Yes |
| Golden Master | All 12 tests | **Yes (Critical)** |
| Integration | Integration tests | Yes |
| Lint | Clippy, format | Yes |

### 6.3 Pre-Merge Checklist

- [ ] All golden master tests pass
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Manual verification complete
- [ ] PR reviewed and approved
- [ ] CI pipeline green

---

## 7. Regression Testing

### 7.1 After Phase A Complete

Run full regression to ensure no DDL changes:

```bash
./scripts/compare-golden-master.sh
cargo test -p ndp-gold-ddl
```

### 7.2 After Phase B Complete

Run full regression to ensure no DDL changes AND new validation works:

```bash
./scripts/compare-golden-master.sh
cargo test -p ndp-gold-ddl
cargo test -p ndp-validate
ndp-validate --all --domain
```

### 7.3 Before FE-002 Closure

Final verification:

1. Golden master comparison: PASS
2. Unit tests: PASS
3. Integration tests: PASS
4. Manual verification: COMPLETE
5. `ndp-validate --domain config/domains/indoor-air-quality/domain.json`: PASS
6. `ndp-gold-ddl generate --domain indoor-air-quality`: Generates valid SQL

---

## 8. Test Failure Handling

### 8.1 Golden Master Failure

```
┌────────────────────────────────────────────────────────────────┐
│ GOLDEN MASTER TEST FAILED                                      │
├────────────────────────────────────────────────────────────────┤
│ 1. STOP all work immediately                                   │
│ 2. Do NOT merge the PR                                         │
│ 3. Investigate the diff                                        │
│ 4. If intentional change: update baseline + document + review  │
│ 5. If unintentional: fix the code                              │
│ 6. Re-run tests until all pass                                 │
└────────────────────────────────────────────────────────────────┘
```

### 8.2 Unit Test Failure

```
┌────────────────────────────────────────────────────────────────┐
│ UNIT TEST FAILED                                               │
├────────────────────────────────────────────────────────────────┤
│ 1. Read the test name to understand what broke                 │
│ 2. Check if test is correct or needs updating                  │
│ 3. Fix the implementation or update the test                   │
│ 4. Re-run the specific test first                              │
│ 5. Then run full suite                                         │
└────────────────────────────────────────────────────────────────┘
```

### 8.3 Integration Test Failure

```
┌────────────────────────────────────────────────────────────────┐
│ INTEGRATION TEST FAILED                                        │
├────────────────────────────────────────────────────────────────┤
│ 1. Check if infrastructure is running (if needed)              │
│ 2. Check test isolation - may need cleanup                     │
│ 3. Review test logs for actual vs expected                     │
│ 4. May indicate a real bug or test environment issue           │
└────────────────────────────────────────────────────────────────┘
```

---

## 9. Coverage Targets

### 9.1 Phase A Coverage

| Component | Target | Priority |
|-----------|--------|----------|
| `config/loader.rs` (JSON loading) | 100% | Critical |
| `config/domain.rs` (parsing) | 90% | High |
| Golden Master tests | 100% | Critical |

### 9.2 Phase B Coverage

| Component | Target | Priority |
|-----------|--------|----------|
| `schema.rs` (validation) | 95% | Critical |
| `semantic/domain.rs` | 90% | High |
| `cli.rs` (new flags) | 85% | High |
| Error formatting | 80% | Medium |

### 9.3 Measuring Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage for ndp-gold-ddl
cargo tarpaulin -p ndp-gold-ddl --out Html --output-dir coverage/gold-ddl/

# Run coverage for ndp-validate
cargo tarpaulin -p ndp-validate --out Html --output-dir coverage/validate/

# View reports
open coverage/gold-ddl/tarpaulin-report.html
open coverage/validate/tarpaulin-report.html
```

---

*Test Plan created: 2026-02-05*
*Feature: FE-002 Domain Configuration Standardization*
*Total Tests Planned: 106*
