# FE-001 Pattern Research: Gold Layer Implementation Guide

**Research Date**: 2026-02-04
**Researcher**: Research Agent (FE-001 Planning Swarm)
**Purpose**: Document existing NDP patterns that Gold layer implementation MUST follow

---

## Executive Summary

This document captures the comprehensive research into existing NDP codebase patterns that the Gold layer (FE-001) implementation must follow. The research covers:

1. Silver ETL event-driven model
2. Config loading flow
3. Two-layer validation pattern
4. DDL generation patterns
5. Deployment flow
6. Test patterns (London TDD)

**Key Finding**: The Gold layer should follow the exact same config-driven, event-driven patterns established by Silver ETL (dp-006, dp-012), with `gold_etl` embedded in StreamConfig alongside existing `silver_etl`.

---

## 1. Silver ETL Event-Driven Model Summary

### Location
- **Binary**: `/workspaces/neural-data-platform/apps/silver-etl/src/main.rs`
- **Library**: `/workspaces/neural-data-platform/apps/silver-etl/src/lib.rs`
- **Core Types**: `/workspaces/neural-data-platform/core/src/silver/`
- **Config Types**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`

### Architecture Pattern

```
Bronze (Parquet)  -->  Silver ETL CLI  -->  TimescaleDB
                           |
                           v
                    SilverEtlConfig (from etcd/YAML)
```

### Key Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `SilverEtlConfig` | `core/src/config/silver_etl.rs` | Configuration types for Bronze-to-Silver ETL |
| `ConfigLoader` | `apps/silver-etl/src/config.rs` | Loads config from etcd with YAML fallback |
| `EtlRunner` | `apps/silver-etl/src/etl.rs` | Orchestrates ETL execution |
| `SchemaGenerator` | `apps/silver-etl/src/schema_gen.rs` | Generates DDL from config |
| `DqSqlGenerator` | `apps/silver-etl/src/dq.rs` | Generates DQ rule SQL |
| `SqlGenerator` | `apps/silver-etl/src/sql_gen.rs` | Generates ETL SQL |

### Execution Modes

From `main.rs` Commands enum:
- **Run**: One-time ETL execution
- **Backfill**: Historical data migration (dp-012)
- **Daemon**: Continuous processing with interval
- **Migrate**: Config-driven schema creation
- **DryRun**: SQL generation without execution
- **Validate**: Configuration validation
- **Status**: Show watermarks and progress

### Pattern for Gold Layer

Gold ETL should implement similar commands:
```rust
enum GoldCommands {
    Run { stream: Option<String> },      // Execute aggregations
    Backfill { stream: Option<String>, since: Option<String>, until: Option<String> },
    Daemon { interval: u64 },            // Continuous aggregation
    Migrate { stream: Option<String>, dry_run: bool },  // Create continuous aggregates
    DryRun { stream: String },           // Show generated SQL
    Validate { stream: Option<String> }, // Validate gold_etl config
    Status { stream: Option<String> },   // Show aggregate status
}
```

---

## 2. Config Loading Flow Documentation

### Source Priority (from `ConfigLoader`)

```
1. etcd endpoint (primary) -> /streams/{stream_id}/config
2. YAML file fallback     -> config/base/streams/{stream_id}/config.yaml
3. JSON file fallback     -> config/base/streams/{stream_id}/config.json
```

### StreamConfig Structure

From `config/base/streams/air-quality/config.json`:

```json
{
  "stream_id": "air-quality",
  "description": "AirGradient sensor readings from MQTT",
  "version": "1.0.0",
  "enabled": true,
  "retention_days": 365,
  "compression_after_days": 7,
  "partitioning_strategy": "daily",
  "fields": [...],
  "sources": [...],
  "storage": {...},
  "silver_etl": {
    "enabled": true,
    "target_table": "silver.air_quality_observations",
    "description": "Indoor air quality measurements",
    "grain": "One row per sensor reading (~1 minute intervals)",
    "timestamp": {...},
    "identity_fields": [...],
    "field_mappings": [...],
    "dq_rules": [...],
    "dq_output": {...},
    "deduplication": {...},
    "incremental": {...}
  }
}
```

### Gold Layer Config Extension

**DECISION**: Embed `gold_etl` in StreamConfig following established pattern:

```json
{
  "stream_id": "air-quality",
  "...existing fields...",
  "silver_etl": {...},
  "gold_etl": {
    "enabled": true,
    "aggregations": [
      {
        "name": "hourly_air_quality",
        "source_table": "silver.air_quality_observations",
        "target_view": "gold.air_quality_hourly",
        "granularity": "1 hour",
        "aggregates": [
          {"column": "pm25", "function": "avg", "alias": "pm25_avg"},
          {"column": "pm25", "function": "max", "alias": "pm25_max"},
          {"column": "pm25", "function": "min", "alias": "pm25_min"}
        ],
        "group_by": ["ndp_id"],
        "retention": "90 days",
        "refresh_interval": "1 hour"
      }
    ]
  }
}
```

### Config Registry Pattern

From `/workspaces/neural-data-platform/config-client/src/stream/registry.rs`:

```rust
pub struct StreamRegistry {
    client: ConfigClient,
    cache: Arc<RwLock<HashMap<String, StreamConfig>>>,
}

impl StreamRegistry {
    pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
    pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;
    pub async fn load_all_streams(&self) -> Result<HashMap<String, StreamConfig>, ConfigError>;
}
```

Gold should use the same registry - config types extended, registry pattern unchanged.

---

## 3. Validation Pattern Summary

### Location
- `/workspaces/neural-data-platform/tools/ndp-validate/src/`

### Two-Layer Validation Architecture (dp-019)

| Layer | Type | Purpose | Tools |
|-------|------|---------|-------|
| **Layer 1** | JSON Schema | Structural validation | `valico`, JSON Schema |
| **Layer 2** | Semantic | Business rules validation | Custom Rust code |

### Layer 1: JSON Schema Validation

From `tools/ndp-validate/src/schema.rs`:
- Type checking
- Required fields
- Enum values
- Pattern matching
- Unknown field detection (`additionalProperties: false`)

### Layer 2: Semantic Validation

From `tools/ndp-validate/src/semantic/mod.rs`:

```rust
pub struct SemanticValidator;

impl SemanticValidator {
    pub fn validate(&self, config: &Value) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // FR-020: Validate sources
        errors.extend(validate_sources(sources));

        // FR-022: Validate source_path references
        errors.extend(validate_source_paths(&field_names, &field_mappings));

        // FR-023: Validate table exists (graceful degradation)
        errors.extend(validate_table_exists(target_table, None));

        // Validate DQ rules
        errors.extend(validate_dq_rules(&dq_rules, &silver_columns));

        errors
    }
}
```

### Semantic Validation Rules

| Module | Purpose |
|--------|---------|
| `sources.rs` | Source type and required field validation |
| `source_path.rs` | Cross-reference validation with Levenshtein suggestions |
| `table_exists.rs` | Target table existence check |
| `dq_rules.rs` | DQ rule syntax and column reference validation |

### Gold Layer Validation Requirements

Gold must add semantic validators for:
1. **Source table existence**: Verify Silver source tables exist
2. **Aggregate function validation**: Valid functions (avg, sum, count, min, max, percentile, etc.)
3. **Column reference validation**: Target columns exist in source
4. **Granularity validation**: Valid TimescaleDB intervals
5. **Cross-aggregate consistency**: No circular dependencies

---

## 4. DDL Generation Patterns

### Location
- `/workspaces/neural-data-platform/deploy/pi/ddl-generator.sh`
- `/workspaces/neural-data-platform/apps/silver-etl/src/schema_gen.rs`

### DDL Generation Flow

```
StreamConfig.silver_etl
         |
         v
   SchemaGenerator
         |
    +---------+---------+---------+
    |         |         |         |
    v         v         v         v
CREATE   CREATE    SELECT       GRANT
SCHEMA   TABLE     create_      permissions
         IF NOT    hypertable
         EXISTS
```

### Bash DDL Generator Functions (ddl-generator.sh)

```bash
# Type mapping
map_type()              # Config type -> PostgreSQL type

# DDL generation
generate_create_table_ddl()      # CREATE TABLE IF NOT EXISTS
generate_indexes_ddl()           # CREATE INDEX IF NOT EXISTS
generate_hypertable_ddl()        # SELECT create_hypertable()
generate_policies_ddl()          # Compression and retention
generate_permissions_ddl()       # GRANT statements
generate_add_column_ddl()        # Schema evolution (ALTER TABLE)
generate_schema_evolution_ddl()  # Diff-based column addition
```

### Rust SchemaGenerator (schema_gen.rs)

```rust
pub struct SchemaGenerator;

impl SchemaGenerator {
    pub fn generate_create_schema(&self, config: &SilverEtlConfig) -> Result<String, SchemaError>;
    pub fn generate_create_table(&self, config: &SilverEtlConfig) -> Result<String, SchemaError>;
    pub fn generate_hypertable(&self, config: &SilverEtlConfig) -> Result<String, SchemaError>;
    pub fn generate_indexes(&self, config: &SilverEtlConfig) -> Result<Vec<String>, SchemaError>;
    pub fn generate_add_columns(&self, config: &SilverEtlConfig, existing: &[String]) -> Result<Vec<String>, SchemaError>;
}
```

### Gold Layer DDL Generation

Gold should generate:

```sql
-- 1. Schema creation
CREATE SCHEMA IF NOT EXISTS gold;

-- 2. Continuous aggregate creation (NOT regular views)
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    AVG(pm25) AS pm25_avg,
    MAX(pm25) AS pm25_max,
    MIN(pm25) AS pm25_min,
    COUNT(*) AS sample_count
FROM silver.air_quality_observations
GROUP BY bucket, ndp_id
WITH NO DATA;

-- 3. Refresh policy
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '3 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');

-- 4. Retention policy (optional)
SELECT add_retention_policy('gold.air_quality_hourly',
    drop_after => INTERVAL '90 days');

-- 5. Permissions
GRANT SELECT ON gold.air_quality_hourly TO grafana_reader;
```

---

## 5. Deployment Flow Documentation

### Location
- `/workspaces/neural-data-platform/deploy/pi/deploy.sh`

### Deployment Phases (from deploy.sh)

| Phase | Command | Action |
|-------|---------|--------|
| 0 | `build` | Docker image build |
| 1 | `start` | Start services |
| 2 | `sync` | Sync configs to etcd |
| 3 | `silver-migrate` | Create Silver schema |
| 4 | `silver-etl` | Run ETL |
| 5 | `sync-dictionary` | Update data dictionary |

### Declarative Deploy (dp-020)

From deploy.sh `apply()` function - 9 phase orchestration:

```
Phase 1: Validation      - Manifest validation
Phase 2: Container Builds - Docker builds
Phase 3: Migrations      - SQL migrations
Phase 4: Silver Tables   - DDL generation + execution
Phase 5: Streams         - Config sync to etcd
Phase 6: Dimensions      - Dimension table sync
Phase 7: Dictionary      - Data dictionary sync
Phase 8: Restarts        - Container restarts
Phase 9: Device State    - Version tracking
```

### Manifest Format

From `.deploy/manifest.json`:

```json
{
  "version": "1.0",
  "release_version": "1.1.0",
  "changes": [
    {"type": "stream", "id": "air-quality", "action": "update"},
    {"type": "silver-table", "stream_id": "air-quality", "action": "sync"},
    {"type": "migration", "file": "deploy/migrations/001_gold_schema.sql"},
    {"type": "dictionary", "action": "sync"}
  ]
}
```

### Gold Layer Deploy Integration

Add to manifest:
```json
{"type": "gold-aggregate", "stream_id": "air-quality", "action": "sync"}
```

Add to deploy.sh phases:
```
Phase 4.5: Gold Aggregates - Continuous aggregate DDL
```

---

## 6. Test Patterns Found

### Location
- Tests embedded in source files (`#[cfg(test)]` modules)
- Integration tests with `#[tokio::test]` and `#[ignore]`

### London TDD Pattern (from registry.rs tests)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ========== LONDON SCHOOL TDD: UNIT TESTS ==========
    // Note: These tests require a running etcd instance for integration testing
    // For true unit tests, we would mock ConfigClient

    fn create_test_config(stream_id: &str) -> StreamConfig {
        StreamConfig {
            stream_id: stream_id.to_string(),
            description: "Test stream".to_string(),
            // ... minimal valid config
        }
    }

    #[test]
    fn test_stream_config_validation_before_save() {
        // Arrange
        let mut invalid_config = create_test_config("test");
        invalid_config.stream_id = "Invalid_ID".to_string();

        // Act
        let result = invalid_config.validate();

        // Assert
        assert!(result.is_err());
    }

    // Integration tests marked with #[ignore]
    #[tokio::test]
    #[ignore]
    async fn test_registry_save_and_load() {
        // Requires running etcd
    }
}
```

### Test Categories

| Category | Pattern | Location |
|----------|---------|----------|
| Unit | `#[test]` | Inline in source files |
| Async Unit | `#[tokio::test]` | Inline in source files |
| Integration | `#[tokio::test] #[ignore]` | Inline, require infra |
| CLI | `Cli::parse_from([...])` | `main.rs` test module |
| DDL Parsing | Multi-line SQL parsing | `main.rs` test module |

### Silver ETL Test Structure (from main.rs)

```rust
#[cfg(test)]
mod tests {
    // CLI parsing tests
    #[test]
    fn test_cli_parsing() { Cli::command().debug_assert(); }

    #[test]
    fn test_run_command_parsing() { ... }

    // DDL parsing tests
    #[test]
    fn test_parse_ddl_simple_statements() { ... }

    #[test]
    fn test_parse_ddl_with_comments_before_statements() { ... }

    #[test]
    fn test_parse_ddl_multiline_create_table() { ... }

    #[test]
    fn test_parse_ddl_realistic_migration() { ... }

    // Daemon tests
    #[test]
    fn test_daemon_command_parsing() { ... }

    // Backfill tests (dp-012)
    #[test]
    fn test_backfill_command_basic() { ... }
}
```

### Config Type Tests (from silver_etl.rs)

Pattern: Test each config variant and validation rule:

```rust
// Test 1: Parse minimal valid config
#[test]
fn test_parse_minimal_silver_etl_config() { ... }

// Test 2: Parse complete config
#[test]
fn test_parse_complete_silver_etl_config() { ... }

// Test 3-15: Parse each DQ rule type
#[test]
fn test_parse_dq_rule_range_check() { ... }

// Test 16-20: Validation rejection tests
#[test]
fn test_validate_rejects_invalid_column_type() { ... }

// Test 21-38: Transform and complex type tests
#[test]
fn test_parse_pre_transform_config_array_explosion() { ... }

// Test 39: Serialization round-trip
#[test]
fn test_serialization_round_trip() { ... }
```

### Gold Layer Test Requirements

Gold ETL tests should follow same structure:
1. Config parsing tests (all variants)
2. Validation tests (positive and negative)
3. DDL generation tests
4. CLI parsing tests
5. Serialization round-trip tests
6. Integration tests with `#[ignore]`

---

## 7. Recommended Patterns for Gold Layer

### 7.1 Config Structure

```rust
// In core/src/config/gold_etl.rs (new file)

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GoldEtlConfig {
    pub enabled: bool,
    pub aggregations: Vec<AggregationConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AggregationConfig {
    pub name: String,
    pub source_table: String,        // e.g., "silver.air_quality_observations"
    pub target_view: String,         // e.g., "gold.air_quality_hourly"
    pub granularity: String,         // TimescaleDB interval
    pub aggregates: Vec<AggregateSpec>,
    pub group_by: Vec<String>,
    pub retention: Option<String>,   // Optional retention policy
    pub refresh_interval: String,    // Continuous aggregate refresh
    pub start_offset: Option<String>,
    pub end_offset: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AggregateSpec {
    pub column: String,
    pub function: AggregateFunction,
    pub alias: String,
    #[serde(default)]
    pub filter: Option<String>,      // Optional WHERE clause
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AggregateFunction {
    Avg,
    Sum,
    Count,
    Min,
    Max,
    First,
    Last,
    Percentile { p: f64 },
    StdDev,
    Variance,
}
```

### 7.2 Crate Layout

Following `architecture:crate-layout` pattern from AgentDB:

**Option A (Preferred)**: Module in Core
```
core/src/
  config/
    mod.rs          <- Add gold_etl export
    silver_etl.rs   <- Existing
    gold_etl.rs     <- NEW: GoldEtlConfig types
  gold/
    mod.rs          <- NEW: Gold layer types
    aggregate.rs    <- NEW: Aggregation logic
```

**Option B**: New App
```
apps/
  silver-etl/       <- Existing
  gold-etl/         <- NEW: Gold ETL binary
    src/
      main.rs
      lib.rs
      schema_gen.rs
      sql_gen.rs
```

### 7.3 DDL Generator Integration

Add to `deploy/pi/ddl-generator.sh`:

```bash
# Gold aggregate generation
generate_gold_aggregate_ddl() {
    local stream_id="$1"
    local config_file="$2"

    # Read gold_etl.aggregations from config
    # Generate CREATE MATERIALIZED VIEW WITH (timescaledb.continuous)
    # Generate add_continuous_aggregate_policy
    # Generate retention policy if configured
}

# In deploy.sh, add new handler
handle_gold_aggregate() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')

    generate_gold_aggregate_ddl "$stream_id" "full" | \
        dcx timescaledb psql -U postgres -d ndp
}
```

### 7.4 Validation Extension

Add to `tools/ndp-validate/src/semantic/`:

```rust
// gold_aggregates.rs (new file)

pub fn validate_gold_aggregations(config: &Value) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if let Some(gold_etl) = config.get("gold_etl") {
        // Validate source table exists
        // Validate aggregate functions
        // Validate column references
        // Validate granularity format
        // Validate no circular dependencies
    }

    errors
}
```

---

## 8. Key Patterns Summary

| Pattern | Location | Gold Application |
|---------|----------|------------------|
| Config-driven | `SilverEtlConfig` | Create `GoldEtlConfig` following same structure |
| Embedded in StreamConfig | `silver_etl` field | Add `gold_etl` field alongside |
| Two-layer validation | `ndp-validate` | Add gold semantic validators |
| Idempotent DDL | `IF NOT EXISTS` | Use `CREATE ... IF NOT EXISTS` |
| Schema evolution | `ALTER TABLE ADD COLUMN` | Use `CREATE OR REPLACE` for views |
| Declarative deploy | `manifest.json` | Add `gold-aggregate` declaration type |
| London TDD | Inline `#[cfg(test)]` | Follow same test structure |
| CLI subcommands | Clap derive | Same pattern for gold-etl CLI |

---

## 9. References

### AgentDB Patterns Found

| Pattern ID | Pattern Name | Relevance |
|------------|--------------|-----------|
| 30 | architecture:ndp-config-lifecycle | Config flow from YAML to etcd to app |
| 32 | architecture:config-deployment-flow | 9-phase deployment orchestration |
| 5 | config:silver-etl-config-struct | SilverEtlConfig structure |
| 26 | architecture:etl-sql-generation | Config-to-SQL generation |
| 28 | architecture:event-driven-silver-etl | Event-driven ETL model |
| 22 | architecture:gold-data-dictionary | Gold layer data dictionary extension |
| 23 | architecture:gold-etl-config-placement | Embed gold_etl in StreamConfig |
| 21 | architecture:crate-layout | Module vs crate organization |
| 16 | testing:ndp-types-london-tdd | London TDD test patterns |
| 27 | deprecated:duckdb-usage | DuckDB is DEPRECATED - use TimescaleDB |

### Key Files for Implementation

```
/workspaces/neural-data-platform/
├── core/src/config/silver_etl.rs     # Template for gold_etl.rs
├── apps/silver-etl/src/main.rs       # Template for gold-etl CLI
├── apps/silver-etl/src/schema_gen.rs # Template for DDL generation
├── deploy/pi/ddl-generator.sh        # Template for bash DDL
├── deploy/pi/deploy.sh               # Add gold deploy phase
├── tools/ndp-validate/src/semantic/  # Add gold validators
└── config/base/streams/*/config.json # Add gold_etl section
```

---

## 10. Next Steps for Planning

1. **Architecture Decision**: Module in core vs separate crate
2. **Config Schema**: Define `GoldEtlConfig` types
3. **Validation Rules**: Define gold-specific semantic validators
4. **DDL Templates**: Design continuous aggregate SQL patterns
5. **Deploy Integration**: Add gold phase to declarative deploy
6. **Test Plan**: London TDD test structure for gold

---

*This research document captures patterns as of 2026-02-04. Patterns may evolve - always verify against current codebase.*
