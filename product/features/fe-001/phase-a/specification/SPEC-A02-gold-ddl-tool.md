# SPEC-A02: Gold DDL Tool (ndp-gold-ddl)

> **Feature ID:** v11-A02
> **Priority:** Critical
> **Status:** Specification
> **Dependencies:** v11-A01 (Gold ETL Schema)
> **Blocks:** Phase B (Continuous Aggregates), Phase C (Aligned Views)
> **ADR:** [ADR-FE001-001](../../architecture/DECISIONS.md#adr-fe001-001-gold-ddl-generation-in-rust)

---

## User Story

**As a** platform operator,
**I want** a Rust CLI tool that generates TimescaleDB DDL from Gold layer configuration,
**So that** I can deploy Gold layer objects declaratively without writing SQL manually.

---

## Goal

Create `ndp-gold-ddl`, a Rust CLI tool that:
1. Reads stream config (including `gold_etl` section)
2. Generates valid TimescaleDB continuous aggregate DDL
3. Generates refresh policies
4. Supports idempotent deployment (sync vs recreate actions)
5. Integrates with `deploy.sh` as the Gold layer DDL generator

---

## Functional Requirements

### FR-A02-001: CLI Interface

The tool SHALL support the following commands:

```bash
# Generate DDL for a stream's Gold layer
ndp-gold-ddl generate --stream <stream_id> [--action sync|recreate] [--config-dir <path>]

# Generate DDL for a domain (aligned view, unified events)
ndp-gold-ddl generate --domain <domain_id> [--action sync|recreate] [--config-dir <path>]

# Validate config without generating DDL
ndp-gold-ddl validate --stream <stream_id> [--config-dir <path>]
ndp-gold-ddl validate --domain <domain_id> [--config-dir <path>]

# Print version
ndp-gold-ddl --version
```

### FR-A02-002: Stream DDL Generation

For `--stream` mode, the tool SHALL generate:

1. **Continuous Aggregate View**:
```sql
CREATE MATERIALIZED VIEW gold.{stream_id}_{granularity}
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('{granularity}', {timestamp_column}) AS bucket,
    {entity_column},
    {aggregate_expressions}
FROM silver.{source_table}
GROUP BY bucket, {entity_column};
```

2. **Refresh Policy**:
```sql
SELECT add_continuous_aggregate_policy('gold.{view_name}',
    start_offset => INTERVAL '{lookback}',
    end_offset => INTERVAL '{delay}',
    schedule_interval => INTERVAL '{refresh_interval}'
);
```

### FR-A02-003: Aggregate Expression Generation

For each field in `gold_etl.aggregates.fields`, generate expressions:

| Metric | SQL Expression |
|--------|----------------|
| `mean` | `AVG({field}) AS {field}_mean` |
| `std` | `STDDEV({field}) AS {field}_std` |
| `min` | `MIN({field}) AS {field}_min` |
| `max` | `MAX({field}) AS {field}_max` |
| `count` | `COUNT({field}) AS {field}_count` |
| `p95` | `PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY {field}) AS {field}_p95` |
| `p99` | `PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY {field}) AS {field}_p99` |
| `first` | `FIRST({field}, {timestamp_column}) AS {field}_first` |
| `last` | `LAST({field}, {timestamp_column}) AS {field}_last` |

### FR-A02-004: Idempotency (sync vs recreate)

**sync mode** (default):
```sql
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = '{view_name}'
    ) THEN
        -- CREATE MATERIALIZED VIEW ...
    ELSE
        RAISE NOTICE 'gold.{view_name} already exists, skipping';
    END IF;
END $$;
```

**recreate mode**:
```sql
-- Drop existing view and policies
DROP MATERIALIZED VIEW IF EXISTS gold.{view_name} CASCADE;

-- Create new view
CREATE MATERIALIZED VIEW gold.{view_name} ...;

-- Re-add policies
SELECT add_continuous_aggregate_policy(...);
```

### FR-A02-005: Config Loading

The tool SHALL:
1. Load stream config from `{config-dir}/base/streams/{stream_id}/config.json`
2. Extract `gold_etl` section
3. Extract `silver_etl.target_table` for source table reference
4. Extract field types from `fields[]` for validation

Default `config-dir`: `/opt/ndp/config` (Pi) or `./config` (development)

### FR-A02-006: Validation Before Generation

Before generating DDL, the tool SHALL validate:
1. `gold_etl.enabled` is `true`
2. All fields referenced in `gold_etl.aggregates.fields` exist in stream `fields[]`
3. All metrics are valid (from allowed enum)
4. Silver target table is specified
5. At least one granularity is specified

Validation failures SHALL:
- Print errors to stderr
- Exit with code 1
- NOT output any DDL

### FR-A02-007: Output Format

The tool SHALL output valid SQL to stdout, suitable for piping to psql:
```bash
ndp-gold-ddl generate --stream air-quality | psql -U postgres -d ndp
```

All SQL statements SHALL be terminated with semicolons.
Comments SHALL precede each major section for debugging.

### FR-A02-008: Multiple Granularities

When `aggregates.granularities` contains multiple values (e.g., `["1 hour", "1 day"]`):
- Generate separate continuous aggregates for each granularity
- Name pattern: `gold.{stream_id}_hourly`, `gold.{stream_id}_daily`
- Each gets its own refresh policy

### FR-A02-009: Schema Creation

The tool SHALL include schema creation if not exists:
```sql
CREATE SCHEMA IF NOT EXISTS gold;
```

---

## Non-Functional Requirements

### NFR-A02-001: Performance

DDL generation SHALL complete in < 500ms for typical stream configurations.

### NFR-A02-002: Error Messages

All errors SHALL include:
- The specific configuration issue
- The JSON path to the problematic value
- A suggestion for fixing the issue

### NFR-A02-003: Cross-Compilation

The tool SHALL cross-compile for:
- `aarch64-unknown-linux-gnu` (Raspberry Pi 5)
- `x86_64-unknown-linux-gnu` (development/CI)

### NFR-A02-004: Minimal Dependencies

Dependencies SHALL be limited to:
- `clap` - CLI parsing
- `serde`, `serde_json` - Config parsing
- `thiserror` - Error handling

No database connection required for DDL generation.

---

## Acceptance Criteria

### AC-A02-001: Basic DDL Generation

```gherkin
Scenario: Generate continuous aggregate for air-quality stream
  Given a stream config at config/base/streams/air-quality/config.json
  And the gold_etl section specifies aggregates for pm25 with metrics [mean, std, max]
  And the granularity is "1 hour"
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the output SHALL contain CREATE MATERIALIZED VIEW gold.air_quality_hourly
  And the output SHALL contain AVG(pm25) AS pm25_mean
  And the output SHALL contain STDDEV(pm25) AS pm25_std
  And the output SHALL contain MAX(pm25) AS pm25_max
  And the output SHALL contain add_continuous_aggregate_policy
```

### AC-A02-002: Idempotent Sync

```gherkin
Scenario: Sync mode skips existing view
  Given gold.air_quality_hourly already exists in the database
  When I run: ndp-gold-ddl generate --stream air-quality --action sync | psql -d ndp
  Then the existing view SHALL NOT be dropped
  And psql output SHALL contain "NOTICE: gold.air_quality_hourly already exists, skipping"
```

### AC-A02-003: Recreate Mode Drops and Creates

```gherkin
Scenario: Recreate mode replaces existing view
  Given gold.air_quality_hourly exists with different columns
  When I run: ndp-gold-ddl generate --stream air-quality --action recreate | psql -d ndp
  Then the existing view SHALL be dropped
  And a new view SHALL be created with updated columns
  And refresh policy SHALL be re-added
```

### AC-A02-004: Validation Failure Blocks Generation

```gherkin
Scenario: Invalid config prevents DDL generation
  Given a stream config with gold_etl.aggregates.fields.nonexistent_field
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the tool SHALL exit with code 1
  And stderr SHALL contain "Field 'nonexistent_field' not found in stream fields"
  And stdout SHALL be empty (no DDL generated)
```

### AC-A02-005: Multiple Granularities

```gherkin
Scenario: Generate views for multiple granularities
  Given gold_etl.aggregates.granularities = ["1 hour", "1 day"]
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the output SHALL contain CREATE MATERIALIZED VIEW gold.air_quality_hourly
  And the output SHALL contain CREATE MATERIALIZED VIEW gold.air_quality_daily
  And each view SHALL have its own refresh policy
```

### AC-A02-006: Integration with deploy.sh

```gherkin
Scenario: deploy.sh calls ndp-gold-ddl for gold-table declarations
  Given a manifest with gold-tables: [{ stream_id: "air-quality", action: "sync" }]
  When I run: deploy.sh apply <manifest>
  Then deploy.sh SHALL call: ndp-gold-ddl generate --stream air-quality --action sync
  And the output SHALL be piped to psql
```

### AC-A02-007: Percentile Metrics

```gherkin
Scenario: Generate percentile aggregates
  Given gold_etl.aggregates.fields.pm25.metrics = ["p95", "p99"]
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the output SHALL contain PERCENTILE_CONT(0.95)
  And the output SHALL contain PERCENTILE_CONT(0.99)
```

---

## Tool Structure

```
tools/ndp-gold-ddl/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs                      # CLI entry point, argument parsing
│   ├── lib.rs                       # Library exports for testing
│   ├── config/
│   │   ├── mod.rs
│   │   ├── loader.rs                # Load stream/domain configs
│   │   └── types.rs                 # GoldEtlConfig, etc.
│   ├── generators/
│   │   ├── mod.rs                   # Generator trait
│   │   ├── continuous_aggregate.rs  # Continuous aggregate DDL
│   │   ├── refresh_policy.rs        # Refresh policy DDL
│   │   ├── aligned_view.rs          # v11-A04: Aligned view DDL
│   │   └── features.rs              # Feature column expressions
│   ├── validation/
│   │   ├── mod.rs
│   │   └── config_validator.rs      # Pre-generation validation
│   └── output/
│       └── sql_formatter.rs         # SQL output formatting
└── tests/
    ├── continuous_aggregate_test.rs
    ├── refresh_policy_test.rs
    ├── validation_test.rs
    └── fixtures/
        ├── air-quality-config.json
        └── expected-ddl/
            └── air-quality-hourly.sql
```

---

## Integration Test Requirements

### Test: End-to-End DDL Generation

```rust
#[test]
fn test_generate_air_quality_ddl() {
    let config = load_test_config("air-quality");
    let generator = ContinuousAggregateGenerator::new();

    let ddl = generator.generate(&config, "1 hour", Action::Sync).unwrap();

    assert!(ddl.contains("CREATE MATERIALIZED VIEW gold.air_quality_hourly"));
    assert!(ddl.contains("WITH (timescaledb.continuous)"));
    assert!(ddl.contains("time_bucket('1 hour'"));
    assert!(ddl.contains("AVG(pm25) AS pm25_mean"));
}
```

### Test: SQL Execution

```bash
# Generate and execute DDL
ndp-gold-ddl generate --stream air-quality | psql -U postgres -d ndp

# Verify view exists
psql -U postgres -d ndp -c "SELECT count(*) FROM timescaledb_information.continuous_aggregates WHERE view_name = 'air_quality_hourly'"
# Expected: 1
```

### Test: Idempotency

```bash
# Run twice with sync
ndp-gold-ddl generate --stream air-quality --action sync | psql -d ndp
ndp-gold-ddl generate --stream air-quality --action sync | psql -d ndp

# Should succeed both times without error
```

---

## London TDD Interfaces

### Trait: ConfigLoader

```rust
pub trait ConfigLoader {
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig, ConfigError>;
}

// Production: FileSystemConfigLoader
// Test: MockConfigLoader with pre-defined configs
```

### Trait: DdlGenerator

```rust
pub trait DdlGenerator {
    fn generate(&self, config: &GoldEtlConfig, granularity: &str, action: Action) -> Result<String, GeneratorError>;
}
```

### Trait: SqlOutputWriter

```rust
pub trait SqlOutputWriter {
    fn write_statement(&mut self, sql: &str) -> io::Result<()>;
    fn write_comment(&mut self, comment: &str) -> io::Result<()>;
}

// Production: StdoutWriter
// Test: StringWriter for assertion
```

---

## deploy.sh Integration

### New Function: handle_gold_table()

```bash
handle_gold_table() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Gold Table: $stream_id (action=$action)"

    # Generate DDL using Rust tool
    local ddl=$(ndp-gold-ddl generate --stream "$stream_id" --action "$action" 2>&1)
    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        error "Gold DDL generation failed: $ddl"
        return 1
    fi

    # Apply DDL to TimescaleDB
    log "  Applying Gold DDL to TimescaleDB..."
    echo "$ddl" | dcx timescaledb psql -U postgres -d ndp

    return $?
}
```

### Phase 5 in apply()

```bash
apply() {
    # ... existing phases ...

    # Phase 5: Gold Tables
    local gold_tables=$(echo "$manifest" | jq -c '.declarations["gold-tables"] // []')
    if [ "$gold_tables" != "[]" ]; then
        log "Phase 5: Gold Tables"
        echo "$gold_tables" | jq -c '.[]' | while read declaration; do
            handle_gold_table "$declaration" || errors=$((errors + 1))
        done
    fi

    # ... remaining phases ...
}
```

---

## References

- [ADR-FE001-001](../../architecture/DECISIONS.md#adr-fe001-001-gold-ddl-generation-in-rust) - Decision rationale
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Phase 5 integration
- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/) - External docs
- [ddl-generator.sh](../../../../deploy/pi/ddl-generator.sh) - Silver DDL reference (Bash)
