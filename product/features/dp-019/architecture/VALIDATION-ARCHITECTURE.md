# dp-019: Config Validation Pipeline - Architecture

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-019 Config Validation Pipeline
**Parent ADRs**: ADR-016-001 (JSON Source of Truth), ADR-018-001 (JSON Pass-Through)

---

## Context

### The Problem

dp-016's research (VALIDATION-RESEARCH.md) identified a **two-tier validation gap**:

| Gap | Current State | Impact |
|-----|---------------|--------|
| **Unknown fields silent** | `#[serde(flatten)]` captures typos | `silver_elt` silently ignored |
| **No schema validation** | serde parsing only | Malformed JSON accepted |
| **No cross-reference validation** | `source_path` not checked against `fields` | Silver ETL silently produces NULLs |
| **No table existence check** | Discovered at INSERT time | Runtime failure, data loss |
| **Unsupported types accepted** | No enum constraints | `type: decimal` accepted but fails later |

### Architectural Foundation

dp-018 established **JSON as the platform configuration standard** with **pass-through architecture**:

```
JSON file (source of truth)
    |
    +-- JSON Schema validation (NEW in dp-019)
    |
    +-- Semantic validation (NEW in dp-019)
    |
    v
etcd (stores JSON blob as-is)
    |
    v
StreamConfig (deserialize from etcd)
```

dp-019 adds the validation gates that prevent invalid configurations from entering etcd.

---

## Decision

**Implement Two-Layer Validation with deploy-time gating and runtime defense-in-depth.**

### Layer 1: JSON Schema Validation (Declarative)

JSON Schema validation handles **structural correctness**:

| Validation | Schema Feature | Example |
|------------|----------------|---------|
| Required fields | `required: [...]` | `stream_id` must exist |
| Type checking | `type: "string"` | `enabled` must be boolean |
| Format validation | `pattern: "regex"` | `stream_id` must be kebab-case |
| Unknown field rejection | `additionalProperties: false` | Typos rejected |
| Enum constraints | `enum: [...]` | `type` must be valid NDP type |
| Range validation | `minimum`, `maximum` | `retention_days >= 0` |
| Array constraints | `minItems`, `maxItems` | At least one field required |

**Why JSON Schema for structural validation:**
- Declarative (no code to write per field)
- IDE integration (autocomplete, inline errors)
- Pre-deploy validation without Rust compilation
- Industry standard with mature tooling
- MCP-native (MCP speaks JSON, can validate directly)

### Layer 2: Semantic Validation (Rust Code)

Semantic validation handles **application rules** that JSON Schema cannot express:

| Validation | Why Not Schema | Implementation |
|------------|----------------|----------------|
| `source_path` references `fields` | Cross-field reference | Rust HashSet lookup |
| Silver table exists in TimescaleDB | External system check | SQL query |
| DQ rule expression is valid SQL | Domain-specific language | sqlparser crate |
| Transform formula coefficients match type | Complex logic | Rust match |
| Endpoint URL is reachable | Network check (optional) | reqwest probe |
| MQTT broker is reachable | Network check (optional) | MQTT probe |

### Architecture Diagram

```
                          Deploy Time                          Runtime
                    +------------------------+           +------------------+
                    |                        |           |                  |
JSON Config File ---+-> Layer 1: Schema      |           | Defensive Check  |
                    |   (jsonschema crate)   |           | (same Validator) |
                    |   - Structure          |           |                  |
                    |   - Types              |           +------------------+
                    |   - Enums              |                    ^
                    |   - Unknown fields     |                    |
                    +------------------------+                    |
                             | pass                               |
                             v                                    |
                    +------------------------+                    |
                    |                        |                    |
                    +-> Layer 2: Semantic    +--------------------+
                    |   (Rust code)          |
                    |   - Cross-references   |
                    |   - Table existence    |
                    |   - DQ rule parsing    |
                    +------------------------+
                             | pass
                             v
                    +------------------------+
                    | Sync to etcd           |
                    +------------------------+
```

---

## Implementation

### Validator Component

A new Rust binary (`ndp-validate`) that can be used:
1. From `deploy.sh` (gates deployment)
2. From app startup (defensive check)
3. From MCP tools (validate before save)
4. From CI/CD (pre-commit validation)

```rust
// tools/ndp-validate/src/lib.rs

pub struct Validator {
    schema: jsonschema::JSONSchema,
    db_pool: Option<PgPool>,  // For table existence checks
}

impl Validator {
    /// Create validator with schema only (no DB checks)
    pub fn schema_only(schema_path: &Path) -> Result<Self, ValidatorError>;

    /// Create validator with full semantic checks
    pub fn with_database(schema_path: &Path, db_url: &str) -> Result<Self, ValidatorError>;

    /// Validate a config file
    pub async fn validate(&self, config_path: &Path) -> ValidationResult;

    /// Validate a config string (for MCP integration)
    pub async fn validate_string(&self, json: &str) -> ValidationResult;
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

pub struct ValidationError {
    pub layer: ValidationLayer,
    pub path: String,        // JSONPath: "$.silver_etl.field_mappings[2].source_path"
    pub message: String,
    pub severity: Severity,  // Error | Warning
}

pub enum ValidationLayer {
    Schema,     // JSON Schema validation
    Semantic,   // Application rules
}
```

### Layer 1: Schema Validation Implementation

Use the `jsonschema` crate with `draft2020-12`:

```rust
use jsonschema::{Draft, JSONSchema, ValidationError};

pub fn validate_schema(json: &Value, schema: &JSONSchema) -> Vec<ValidationError> {
    schema
        .validate(json)
        .err()
        .map(|errors| errors.collect())
        .unwrap_or_default()
}
```

**Schema enhancements for dp-019:**

The existing `stream-config.v1.1.schema.json` already includes:
- `additionalProperties: false` (catches unknown fields)
- `required` arrays for mandatory fields
- `pattern` for field naming conventions
- `enum` for supported types in `fields[].type` and `silver_etl.field_mappings[].type`

dp-019 should:
1. Audit that ALL enum constraints match actual NDP support
2. Add any missing `additionalProperties: false` to nested objects
3. Document which enums are authoritative (schema-driven)

### Layer 2: Semantic Validation Implementation

```rust
// tools/ndp-validate/src/semantic.rs

pub struct SemanticValidator {
    db_pool: Option<PgPool>,
}

impl SemanticValidator {
    pub async fn validate(&self, config: &StreamConfig) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // 2.7: Cross-reference validation
        errors.extend(self.validate_source_paths(config));

        // 2.8: Silver table existence (if DB available)
        if let Some(pool) = &self.db_pool {
            errors.extend(self.validate_table_exists(config, pool).await);
        }

        // 2.9: DQ rule syntax
        errors.extend(self.validate_dq_rules(config));

        // 2.10: Source config validation
        errors.extend(self.validate_sources(config));

        errors
    }

    /// Validate that all source_path values reference fields defined in config.fields
    fn validate_source_paths(&self, config: &StreamConfig) -> Vec<ValidationError> {
        let Some(silver_etl) = &config.silver_etl else {
            return vec![];
        };

        // Build set of valid field names
        let field_names: HashSet<&str> = config
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect();

        let mut errors = Vec::new();

        for (idx, mapping) in silver_etl.field_mappings.iter().enumerate() {
            // source_path format: "raw_payload.field_name" or "raw_payload.nested.field"
            if let Some(field_ref) = mapping.source_path.strip_prefix("raw_payload.") {
                // For nested paths, check the root field
                let root_field = field_ref.split('.').next().unwrap_or(field_ref);

                if !field_names.contains(root_field) {
                    errors.push(ValidationError {
                        layer: ValidationLayer::Semantic,
                        path: format!("$.silver_etl.field_mappings[{}].source_path", idx),
                        message: format!(
                            "source_path '{}' references field '{}' which is not defined in config.fields",
                            mapping.source_path, root_field
                        ),
                        severity: Severity::Error,
                    });
                }
            }
        }

        errors
    }

    /// Validate that target_table exists in TimescaleDB
    async fn validate_table_exists(&self, config: &StreamConfig, pool: &PgPool) -> Vec<ValidationError> {
        let Some(silver_etl) = &config.silver_etl else {
            return vec![];
        };

        // Parse "silver.table_name" format
        let parts: Vec<&str> = silver_etl.target_table.split('.').collect();
        if parts.len() != 2 {
            return vec![ValidationError {
                layer: ValidationLayer::Semantic,
                path: "$.silver_etl.target_table".to_string(),
                message: format!("Invalid table format: expected 'schema.table', got '{}'", silver_etl.target_table),
                severity: Severity::Error,
            }];
        }

        let (schema, table) = (parts[0], parts[1]);

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = $1 AND table_name = $2)"
        )
        .bind(schema)
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !exists {
            return vec![ValidationError {
                layer: ValidationLayer::Semantic,
                path: "$.silver_etl.target_table".to_string(),
                message: format!("Silver table '{}' does not exist in TimescaleDB", silver_etl.target_table),
                severity: Severity::Error,
            }];
        }

        vec![]
    }

    /// Validate DQ rule syntax
    fn validate_dq_rules(&self, config: &StreamConfig) -> Vec<ValidationError> {
        let Some(silver_etl) = &config.silver_etl else {
            return vec![];
        };

        let mut errors = Vec::new();

        for (idx, rule) in silver_etl.dq_rules.iter().enumerate() {
            if let Some(expr) = &rule.expression {
                // Validate SQL expression syntax
                match sqlparser::parser::Parser::parse_sql(
                    &sqlparser::dialect::PostgreSqlDialect {},
                    &format!("SELECT {}", expr)
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        errors.push(ValidationError {
                            layer: ValidationLayer::Semantic,
                            path: format!("$.silver_etl.dq_rules[{}].expression", idx),
                            message: format!("Invalid SQL expression: {}", e),
                            severity: Severity::Error,
                        });
                    }
                }
            }
        }

        errors
    }

    /// Validate source configurations
    fn validate_sources(&self, config: &StreamConfig) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        for (idx, source) in config.sources.iter().enumerate() {
            match source.source_type.as_str() {
                "mqtt" => {
                    if source.broker_url.is_none() {
                        errors.push(ValidationError {
                            layer: ValidationLayer::Semantic,
                            path: format!("$.sources[{}]", idx),
                            message: "MQTT source requires 'broker_url'".to_string(),
                            severity: Severity::Error,
                        });
                    }
                }
                "http_poll" => {
                    if source.endpoints.is_empty() {
                        errors.push(ValidationError {
                            layer: ValidationLayer::Semantic,
                            path: format!("$.sources[{}]", idx),
                            message: "HTTP poll source requires at least one endpoint".to_string(),
                            severity: Severity::Error,
                        });
                    }
                }
                _ => {}
            }
        }

        errors
    }
}
```

### Integration Points

#### 1. Deploy-Time Validation (gates deployment)

```bash
# deploy/pi/deploy.sh

validate_configs() {
    echo "Validating configurations..."

    # Schema-only validation (no DB required)
    if ! ndp-validate --schema-only --all; then
        echo "ERROR: Schema validation failed"
        exit 1
    fi

    # Full validation with database checks (if DB available)
    if [ -n "$DATABASE_URL" ]; then
        if ! ndp-validate --all; then
            echo "ERROR: Semantic validation failed"
            exit 1
        fi
    fi

    echo "Configuration validation passed"
}

# Call before sync
validate_configs
sync_to_etcd
```

#### 2. Runtime Defensive Validation (app startup)

```rust
// apps/air-quality-app/src/main.rs

async fn startup_validation(config: &AppConfig, db_pool: &PgPool) -> Result<(), StartupError> {
    let validator = Validator::with_database(SCHEMA_PATH, db_pool)?;

    for stream_id in &config.enabled_streams {
        let stream_config = registry.load_stream(stream_id).await?;
        let result = validator.validate_config(&stream_config).await;

        if !result.valid {
            tracing::error!(
                stream_id = %stream_id,
                errors = ?result.errors,
                "Stream config validation failed at startup"
            );

            // In strict mode, fail startup
            if config.strict_validation {
                return Err(StartupError::ValidationFailed {
                    stream_id: stream_id.clone(),
                    errors: result.errors,
                });
            }
        }
    }

    Ok(())
}
```

#### 3. MCP Tool Integration

```rust
// core/src/mcp/tools/validate_config.rs

pub struct ValidateConfigTool {
    validator: Validator,
}

impl McpTool for ValidateConfigTool {
    async fn execute(&self, args: ValidateConfigArgs) -> Result<McpResponse> {
        let result = self.validator.validate_string(&args.config_json).await;

        Ok(McpResponse::json(serde_json::json!({
            "valid": result.valid,
            "errors": result.errors,
            "warnings": result.warnings
        })))
    }
}
```

---

## Validation Responsibilities by Layer

### What JSON Schema Validates (Layer 1)

| Category | Examples | Schema Feature |
|----------|----------|----------------|
| **Structure** | Required fields, array vs object | `required`, `type` |
| **Naming** | stream_id format, field name format | `pattern` |
| **Types** | String where string expected | `type` |
| **Enums** | `fields[].type` in allowed set | `enum` |
| **Ranges** | retention_days >= 0 | `minimum`, `maximum` |
| **Unknown fields** | Typos rejected | `additionalProperties: false` |
| **Deprecated** | entity_schemas marked deprecated | `deprecated: true` |

### What Rust Code Validates (Layer 2)

| Category | Examples | Why Not Schema |
|----------|----------|----------------|
| **Cross-references** | source_path -> fields | Inter-object reference |
| **External state** | Table exists in DB | External system |
| **Complex rules** | Transform coefficients valid for type | Domain logic |
| **Network probes** | MQTT broker reachable | Runtime check |
| **SQL syntax** | DQ expression is valid SQL | DSL parsing |

---

## Error Reporting Format

### CLI Output (Human-Readable)

```
$ ndp-validate config/base/streams/air-quality/config.json

VALIDATION FAILED: 3 errors, 1 warning

ERRORS:
  [schema] $.fields[0].type
    Value 'decimal' is not one of: float, integer, string, boolean, timestamp, json

  [semantic] $.silver_etl.field_mappings[2].source_path
    source_path 'raw_payload.pm02_typo' references field 'pm02_typo' which is not defined in config.fields

  [semantic] $.silver_etl.target_table
    Silver table 'silver.air_quality_readings' does not exist in TimescaleDB

WARNINGS:
  [semantic] $.entity_schemas
    entity_schemas is deprecated in v1.1. Use fields[].description instead.
```

### JSON Output (Machine-Readable)

```json
{
  "valid": false,
  "config_file": "config/base/streams/air-quality/config.json",
  "errors": [
    {
      "layer": "schema",
      "path": "$.fields[0].type",
      "message": "Value 'decimal' is not one of: float, integer, string, boolean, timestamp, json",
      "severity": "error"
    },
    {
      "layer": "semantic",
      "path": "$.silver_etl.field_mappings[2].source_path",
      "message": "source_path 'raw_payload.pm02_typo' references field 'pm02_typo' which is not defined in config.fields",
      "severity": "error"
    },
    {
      "layer": "semantic",
      "path": "$.silver_etl.target_table",
      "message": "Silver table 'silver.air_quality_readings' does not exist in TimescaleDB",
      "severity": "error"
    }
  ],
  "warnings": [
    {
      "layer": "semantic",
      "path": "$.entity_schemas",
      "message": "entity_schemas is deprecated in v1.1. Use fields[].description instead.",
      "severity": "warning"
    }
  ]
}
```

---

## Consequences

### Positive

1. **Fail fast** - Invalid configs caught at deploy time, not runtime
2. **Clear errors** - JSONPath location + actionable message
3. **Defense in depth** - Deploy-time + runtime validation
4. **MCP integration** - Validate before save in admin tools
5. **IDE support** - JSON Schema enables autocomplete
6. **Layered approach** - Schema handles structure, code handles semantics

### Negative

1. **Build complexity** - New binary to maintain
2. **DB dependency for full validation** - Schema-only mode as fallback
3. **Schema maintenance** - Enums must stay in sync with code

### Neutral

1. **Validation time** - Sub-second for schema, depends on DB for semantic
2. **CI integration** - Can run schema-only in pre-commit

---

## Alternatives Considered

### Alternative 1: Schema-Only Validation

Encode all rules in JSON Schema using complex conditionals.

**Rejected because:**
- JSON Schema conditionals are verbose and hard to maintain
- Cannot check external state (table existence)
- Cross-reference validation extremely complex in schema

### Alternative 2: Code-Only Validation

Do all validation in Rust, no JSON Schema.

**Rejected because:**
- Loses IDE integration
- Every field requires code
- Duplicates type/required checks that serde already does
- No declarative documentation of constraints

### Alternative 3: Validation at Runtime Only

Validate when app loads config from etcd.

**Rejected because:**
- Bad config already in etcd
- Fails at worst time (production startup)
- No pre-deploy safety net

---

## Implementation Phases

| Phase | Scope | Effort |
|-------|-------|--------|
| **1** | Schema validation with `jsonschema` crate | 2 days |
| **2** | Semantic validation: cross-references | 2 days |
| **3** | Semantic validation: table existence | 1 day |
| **4** | CLI tool and deploy.sh integration | 1 day |
| **5** | Runtime startup validation | 1 day |
| **6** | MCP tool integration | 1 day |

---

## Related Decisions

- **ADR-016-001**: JSON as source of truth (enables JSON Schema)
- **ADR-016-002**: Declarative deploy (validation gates deployment)
- **ADR-018-001**: Pass-through architecture (validation before sync)

---

## References

- `/workspaces/neural-data-platform/product/features/dp-016/specification/VALIDATION-RESEARCH.md` - Current gaps
- `/workspaces/neural-data-platform/product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md` - JSON standard
- `/workspaces/neural-data-platform/product/features/dp-018/architecture/ADR-018-001-config-loader-design.md` - Pass-through
- `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json` - Current schema

---

*Architecture document created: 2026-02-02*
*Feature: dp-019 Config Validation Pipeline*
