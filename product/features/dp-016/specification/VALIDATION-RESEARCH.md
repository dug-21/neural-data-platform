# dp-016: Validation and Error Handling Research

## Executive Summary

This document analyzes the current state of configuration validation and error handling in the Neural Data Platform. The research reveals a **two-tier validation gap**: while structural validation (format, types, ranges) is well-implemented, **semantic validation** (cross-references, prerequisites, runtime dependencies) is largely absent. This creates silent failures that are difficult to diagnose.

---

## 1. Configuration Parsing

### 1.1 Where YAML is Parsed

| Component | File | Parser | Struct |
|-----------|------|--------|--------|
| Air Quality App | `apps/air-quality-app/src/config.rs` | `serde_yaml` | `AppConfig` |
| Config Sync Service | `apps/air-quality-app/src/config_sync/service.rs` | `serde_yaml` | `YamlStreamConfig` |
| Silver ETL | `apps/silver-etl/src/config.rs` | `serde_yaml` | `StreamConfigWithSilver` |
| Config Client | `config-client/src/client.rs` | `serde_json` (from etcd) | Various |

### 1.2 Key Structs and Serde Behavior

#### StreamConfig (`core/src/types/stream_config.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub version: u32,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub fields: Vec<SchemaField>,
    pub sources: Vec<SourceConfig>,
    #[serde(default)]
    pub entity_schemas: HashMap<String, EntitySchema>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,  // Captures unknown fields!
}
```

**Serde Annotations Used:**
- `#[serde(default)]` - Provides empty defaults for optional fields
- `#[serde(flatten)]` - Captures unknown fields into `extra` HashMap
- `#[serde(rename = "...")]` - Field name mapping
- `#[serde(skip_serializing_if = "Option::is_none")]` - Clean serialization

#### Unknown Field Handling

**FINDING**: Unknown fields are silently captured into `extra` HashMap, not rejected.

```rust
#[serde(flatten)]
pub extra: HashMap<String, serde_json::Value>,
```

This means typos like `silver_elt` instead of `silver_etl` are accepted and silently stored in `extra`, never used. The operator has no indication the section was ignored.

#### SilverEtlConfig (`core/src/config/silver_etl.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SilverEtlConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub target_table: String,
    pub timestamp: TimestampConfig,
    #[serde(default)]
    pub field_mappings: Vec<FieldMapping>,
    #[serde(default)]
    pub dq_rules: Vec<DQRule>,
    // ... more fields
}
```

### 1.3 Parsing Error Behavior

| Error Type | Behavior | Visibility |
|------------|----------|------------|
| Invalid YAML syntax | `serde_yaml::Error` returned | Error logged, file skipped |
| Missing required field | `serde_yaml::Error` returned | Error logged, file skipped |
| Wrong field type | `serde_yaml::Error` returned | Error logged, file skipped |
| Unknown field | Captured in `extra` HashMap | **SILENT** - no indication |
| Typo in section name | Captured in `extra` HashMap | **SILENT** - section ignored |

---

## 2. Structural Validation (What EXISTS)

### 2.1 StreamConfig Validation

**File**: `core/src/types/stream_config.rs`

```rust
impl StreamConfig {
    pub fn validate(&self) -> Result<(), StreamConfigError> {
        // 1. Stream ID format (kebab-case, 3-64 chars)
        if !is_valid_stream_id(&self.stream_id) {
            return Err(StreamConfigError::InvalidStreamId(self.stream_id.clone()));
        }

        // 2. Requires at least one field
        if self.fields.is_empty() {
            return Err(StreamConfigError::NoFields);
        }

        // 3. Requires at least one source
        if self.sources.is_empty() {
            return Err(StreamConfigError::NoSources);
        }

        // 4. Validate each field
        for field in &self.fields {
            field.validate()?;
        }

        Ok(())
    }
}
```

**Validation Coverage:**
- [x] Stream ID format (regex: `^[a-z][a-z0-9-]{2,63}$`)
- [x] At least one field defined
- [x] At least one source defined
- [x] Each field passes individual validation

### 2.2 SchemaField Validation

**File**: `core/src/types/stream_config.rs`

```rust
impl SchemaField {
    pub fn validate(&self) -> Result<(), StreamConfigError> {
        // 1. Field name format
        if !is_valid_field_name(&self.name) {
            return Err(StreamConfigError::InvalidFieldName(self.name.clone()));
        }

        // 2. Type-appropriate validation
        match &self.field_type {
            FieldType::Float { min, max, .. } => {
                if let (Some(min_val), Some(max_val)) = (min, max) {
                    if min_val >= max_val {
                        return Err(StreamConfigError::InvalidRange { /* ... */ });
                    }
                }
            }
            FieldType::Integer { min, max, .. } => { /* similar */ }
            FieldType::Enum { values, .. } => {
                if values.is_empty() {
                    return Err(StreamConfigError::EmptyEnumValues(self.name.clone()));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
```

**Validation Coverage:**
- [x] Field name format (snake_case, 1-64 chars)
- [x] Float/Integer ranges are valid (min < max)
- [x] Enum has at least one value

### 2.3 SilverEtlConfig Validation

**File**: `core/src/config/silver_etl.rs`

```rust
impl SilverEtlConfig {
    pub fn validate(&self) -> Result<(), SilverConfigError> {
        // 1. Target table must be in silver schema
        if !self.target_table.starts_with("silver.") {
            return Err(SilverConfigError::InvalidTargetTable {
                table: self.target_table.clone(),
                reason: "Target table must start with 'silver.'".to_string(),
            });
        }

        // 2. Validate timestamp config
        self.timestamp.validate()?;

        // 3. Validate each field mapping
        for mapping in &self.field_mappings {
            mapping.validate()?;
        }

        // 4. Validate DQ rules
        for rule in &self.dq_rules {
            rule.validate()?;
        }

        Ok(())
    }
}
```

**Validation Coverage:**
- [x] Target table starts with `silver.`
- [x] Timestamp config is valid
- [x] Field mappings have valid column types
- [x] DQ rules have valid parameters

### 2.4 DQ Rule Validation

**File**: `core/src/config/silver_etl.rs`

11 DQ rule types are validated:

| Rule Type | Validation |
|-----------|------------|
| `range_check` | min < max, at least one bound set |
| `null_check` | Column name not empty |
| `enum_check` | Non-empty allowed values list |
| `pattern_check` | Valid regex pattern |
| `freshness_check` | Positive max_age_seconds |
| `monotonic_check` | Valid direction (increasing/decreasing) |
| `rate_of_change` | Positive max_change_rate |
| `cross_field_check` | Valid operator, two column names |
| `conditional_check` | Valid condition and nested rule |
| `completeness_check` | min_completeness in 0.0-1.0 |
| `cardinality_check` | min_distinct <= max_distinct |

### 2.5 Where Validation is Called

| Location | When | What |
|----------|------|------|
| `config_sync/service.rs` | YAML loaded | `StreamConfig.validate()` |
| `stream/registry.rs` | Before etcd save | `StreamConfig.validate()` |
| `silver-etl/main.rs` | `validate` command | `SilverEtlConfig.validate()` |
| `silver-etl/main.rs` | Before migration | `SilverEtlConfig.validate()` |

---

## 3. Semantic Validation (What is MISSING)

### 3.1 Field Mapping Source Path Validation

**GAP**: `field_mappings.source_path` is NOT validated against actual Bronze payload structure.

```yaml
# config/base/streams/air-quality/config.yaml
silver_etl:
  field_mappings:
    - source_path: raw_payload.pm02Compensated  # Is this field real?
      target_column: pm25
```

**What happens**: If `pm02Compensated` is misspelled or doesn't exist in the actual payload, the Silver ETL will:
1. Accept the config without error
2. At runtime, produce NULL values for the column
3. No warning or error is logged

**Recommendation**: Add validation that checks `source_path` against:
- The `fields` section of the same config
- Or sample Bronze payloads (via MCP validate_config approach)

### 3.2 Silver Table Existence Validation

**GAP**: No check that `target_table` exists before starting SilverSubscriber.

```rust
// apps/air-quality-app/src/main.rs
match SilverSubscriber::new(/* ... */).await {
    Ok(subscriber) => { /* start it */ }
    Err(e) => {
        tracing::warn!("Failed to create SilverSubscriber: {}", e);
        // Continues without Silver ETL!
    }
}
```

**What happens**: If `silver.air_quality_readings` doesn't exist:
1. SilverSubscriber creation might fail (depends on when connection is made)
2. Or it starts but fails on first INSERT
3. Error is logged but Bronze continues, data is lost to Silver

**Recommendation**: Validate table existence at startup, fail loudly if critical tables missing.

### 3.3 Cross-Schema Reference Validation

**GAP**: No validation between `fields`, `entity_schemas`, and `silver_etl.field_mappings`.

```yaml
# Three sections that SHOULD agree but are not cross-validated:

fields:
  - name: pm25
    field_type: { type: float }

entity_schemas:
  sensor_reading:
    columns:
      - name: pm25_compensated  # Different name!

silver_etl:
  field_mappings:
    - source_path: raw_payload.pm02Compensated  # Yet another name!
      target_column: pm25
```

**What happens**: Each section is validated independently. Inconsistencies are not detected.

**Recommendation**: Add cross-reference validation:
1. All `source_path` references should resolve to `fields` entries
2. All `target_column` values should match actual Silver table schema
3. Entity schema columns should align with Silver table

### 3.4 Entity Schema vs Bronze Payload Validation

**PARTIAL**: The MCP `validate_config` tool does compare entity_schemas to Bronze payloads, but:
1. It must be invoked manually
2. It's not run at config sync time
3. Results are not persisted

```rust
// core/src/mcp/tools/validate_config.rs
pub async fn execute(&self, args: ValidateConfigArgs) -> Result<ValidateResult> {
    // Compares entity_schemas against actual Bronze Parquet payload
    // Returns: mismatch | partial_match | match
}
```

**Recommendation**: Integrate this validation into config sync workflow.

### 3.5 DQ Rule Column Validation

**GAP**: DQ rules reference columns that are not validated to exist.

```yaml
dq_rules:
  - rule_type: range_check
    column: pm25_reading  # Is this the right column name?
    min_value: 0
    max_value: 1000
```

**What happens**: If the column name is wrong, the DQ rule silently applies to NULL values (no column found), or causes a runtime error.

**Recommendation**: Validate DQ rule columns against `field_mappings.target_column` values.

---

## 4. Runtime Validation

### 4.1 Application Startup

**File**: `apps/air-quality-app/src/main.rs`

```rust
// Config loading cascade
let config = match load_from_etcd().await {
    Ok(config) => config,
    Err(_) => match AppConfig::from_yaml(&yaml_path) {
        Ok(config) => config,
        Err(e) => {
            tracing::error!("Failed to load config: {}", e);
            return Err(e.into());
        }
    }
};

// Config sync - FAILURES ARE WARNINGS, NOT ERRORS
match sync_service.sync_all(&registry).await {
    Ok(count) => {
        tracing::info!("Synced {} stream configs to etcd", count);
    }
    Err(e) => {
        tracing::warn!("Config sync failed: {}", e);  // SILENT FAILURE
        // Continues running with stale/missing etcd config!
    }
}
```

**Problem**: Config sync failures are logged as warnings, not errors. The application continues with potentially stale or missing configuration.

### 4.2 Silver ETL Startup

**File**: `apps/silver-etl/src/main.rs`

```rust
// Discovers enabled streams
let enabled_streams = config_loader.load_all_enabled().await?;

for stream_id in enabled_streams {
    let config = config_loader.load_stream_config(&stream_id).await?;

    // Validation happens here
    if let Err(e) = config.validate() {
        tracing::error!("Invalid config for {}: {}", stream_id, e);
        continue;  // Skips this stream, continues others
    }

    // Start subscriber for this stream
}
```

**Behavior**: Invalid configs cause that stream to be skipped, but other streams continue. This is reasonable, but there's no health endpoint to report which streams failed.

### 4.3 Prerequisite Checking

**GAP**: No comprehensive prerequisite check at startup.

**What should be validated:**
- [ ] etcd is reachable
- [ ] MQTT broker is reachable
- [ ] TimescaleDB is reachable
- [ ] Required Silver tables exist
- [ ] Required Parquet directories exist/are writable
- [ ] All stream configs are valid

**What currently happens:**
- [x] etcd connection failure → fallback to YAML
- [x] MQTT connection failure → logged, retries
- [x] TimescaleDB failure → SilverSubscriber not created (warn)
- [ ] Silver table missing → runtime INSERT error
- [ ] Parquet directory issues → runtime write error

---

## 5. Error Surfacing

### 5.1 Logging Patterns

| Error Type | Log Level | Visibility |
|------------|-----------|------------|
| YAML parse error | ERROR | High (startup fails) |
| Config validation error | ERROR | High (stream skipped) |
| etcd sync failure | WARN | Low (continues running) |
| etcd connection failure | WARN | Low (falls back to YAML) |
| Silver table missing | ERROR | Medium (runtime, in logs) |
| Unknown field in YAML | SILENT | None (captured in `extra`) |
| DQ rule runtime failure | WARN | Low (in etl logs) |

### 5.2 Health Endpoints

**GAP**: No health endpoints expose configuration state.

The air-quality-app exposes `/health` but it only checks:
- Basic HTTP server is running
- No config validation status
- No stream status
- No etcd sync status

**Recommendation**: Add `/health/config` endpoint returning:
```json
{
  "streams": {
    "air-quality": {
      "status": "valid",
      "etcd_synced": true,
      "silver_enabled": true,
      "silver_table_exists": true
    }
  },
  "last_sync": "2026-01-31T10:00:00Z",
  "sync_errors": []
}
```

### 5.3 Operator Visibility

**Current State:**
- Operators must grep logs to find config issues
- No dashboard or metrics for config health
- No alerts on config sync failures
- No visibility into which fields were ignored (unknown fields)

**Gaps:**
1. Unknown fields silently captured → no visibility
2. etcd sync warnings → buried in logs
3. Skipped streams → only in logs
4. DQ rule failures → in ETL logs, not aggregated

---

## 6. Known Gaps from air-012 Retrospective

### 6.1 Invalid Field References Accepted

**Confirmed**: `silver_etl.field_mappings.source_path` values are not validated against:
- The `fields` section
- Actual Bronze payload structure

**Impact**: Typos in source paths cause NULL values in Silver with no warning.

### 6.2 Missing Tables Not Caught

**Confirmed**: No startup validation that `silver_etl.target_table` exists in TimescaleDB.

**Impact**: First INSERT attempt fails at runtime. Data may be lost if error handling is poor.

### 6.3 etcd Sync Failure Silent

**Confirmed**: `sync_service.sync_all()` failures are logged as WARN, not ERROR.

```rust
Err(e) => {
    tracing::warn!("Config sync failed: {}", e);
    // Application continues!
}
```

**Impact**: App runs with stale config in etcd. Components reading from etcd see old data.

### 6.4 Unknown Fields Not Reported

**Confirmed**: `#[serde(flatten)]` captures unknown fields but no logging/warning.

**Impact**: Typos like `silver_elt` are silently ignored.

---

## 7. Recommendations

### 7.1 Short-Term (Low Effort, High Impact)

| Recommendation | Location | Effort |
|----------------|----------|--------|
| Log warning for non-empty `extra` HashMap | `config_sync/service.rs` | 1 hour |
| Promote etcd sync failure to ERROR | `main.rs` | 30 min |
| Add `--strict` mode that fails on sync errors | `main.rs` | 2 hours |
| Add Silver table existence check at startup | `SilverSubscriber::new()` | 2 hours |

### 7.2 Medium-Term (Moderate Effort)

| Recommendation | Location | Effort |
|----------------|----------|--------|
| Cross-validate `source_path` against `fields` | `SilverEtlConfig.validate()` | 4 hours |
| Add `/health/config` endpoint | `apps/air-quality-app` | 4 hours |
| DQ rule column validation | `DQRule.validate()` | 2 hours |
| Prerequisite check at startup | `main.rs` | 4 hours |

### 7.3 Long-Term (Significant Effort)

| Recommendation | Location | Effort |
|----------------|----------|--------|
| Integrate MCP validate_config into sync | `config_sync/service.rs` | 1 day |
| Schema evolution validation | New module | 2-3 days |
| Config diff and migration tooling | New CLI | 3-5 days |
| Grafana dashboard for config health | `deploy/grafana` | 1 day |

---

## 8. Validation Code Locations Summary

| Validation Type | File | Function |
|-----------------|------|----------|
| Stream ID format | `core/src/types/stream_config.rs` | `is_valid_stream_id()` |
| StreamConfig structure | `core/src/types/stream_config.rs` | `StreamConfig::validate()` |
| SchemaField format | `core/src/types/stream_config.rs` | `SchemaField::validate()` |
| Field name format | `core/src/types/stream_config.rs` | `is_valid_field_name()` |
| SilverEtlConfig | `core/src/config/silver_etl.rs` | `SilverEtlConfig::validate()` |
| FieldMapping | `core/src/config/silver_etl.rs` | `FieldMapping::validate()` |
| DQRule | `core/src/config/silver_etl.rs` | `DQRule::validate()` |
| TimestampConfig | `core/src/config/silver_etl.rs` | `TimestampConfig::validate()` |
| Config before etcd save | `config-client/src/stream/registry.rs` | `save_stream()` |
| Config after YAML parse | `apps/air-quality-app/src/config_sync/service.rs` | `sync_stream()` |
| Config vs Bronze payload | `core/src/mcp/tools/validate_config.rs` | `ValidateConfigTool::execute()` |

---

## 9. Appendix: Error Type Catalog

### StreamConfigError

```rust
pub enum StreamConfigError {
    InvalidStreamId(String),
    InvalidFieldName(String),
    NoFields,
    NoSources,
    InvalidRange { field: String, min: f64, max: f64 },
    EmptyEnumValues(String),
    DuplicateFieldName(String),
}
```

### SilverConfigError

```rust
pub enum SilverConfigError {
    InvalidTargetTable { table: String, reason: String },
    InvalidTimestampConfig { reason: String },
    InvalidFieldMapping { field: String, reason: String },
    InvalidDQRule { rule: String, reason: String },
    InvalidColumnType { column: String, expected: String, got: String },
    MissingRequiredField { field: String },
}
```

---

*Research completed: 2026-02-01*
*Feature: dp-016 Configuration Architecture Review*
*Phase: Specification*
