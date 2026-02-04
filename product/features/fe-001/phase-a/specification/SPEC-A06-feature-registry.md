# SPEC-A06: Feature Type Registry

> **Feature ID:** v11-A06
> **Priority:** High
> **Status:** Specification
> **Dependencies:** v11-A02 (Gold DDL Tool)
> **Blocks:** Phase B (Feature Computation)

---

## User Story

**As a** platform developer,
**I want** an extensible registry of feature computation types,
**So that** I can add new feature types (e.g., rolling stats, lag features, trends) via trait implementation without modifying the core interpreter.

---

## Goal

Create a Feature Type Registry that:
1. Defines a trait for feature generators
2. Registers built-in feature types (lag, rolling, trend)
3. Allows new feature types to be added via trait implementation
4. Generates SQL expressions for each feature type
5. Validates feature configurations

---

## Functional Requirements

### FR-A06-001: Feature Generator Trait

Define a trait that all feature generators must implement:

```rust
pub trait FeatureGenerator: Send + Sync {
    /// Unique identifier for this feature type
    fn feature_type(&self) -> &str;

    /// Generate SQL column expressions for this feature
    fn generate_columns(&self, config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError>;

    /// Validate the feature configuration
    fn validate(&self, config: &FeatureConfig) -> Result<(), FeatureError>;

    /// Return required window functions or CTEs
    fn requires_window(&self) -> bool;

    /// Human-readable description
    fn description(&self) -> &str;
}

pub struct SqlColumn {
    pub expression: String,  // SQL expression
    pub alias: String,       // Column name
    pub data_type: String,   // PostgreSQL type
}
```

### FR-A06-002: Feature Registry

The registry SHALL manage registered feature generators:

```rust
pub struct FeatureRegistry {
    generators: HashMap<String, Box<dyn FeatureGenerator>>,
}

impl FeatureRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, generator: Box<dyn FeatureGenerator>);
    pub fn get(&self, feature_type: &str) -> Option<&dyn FeatureGenerator>;
    pub fn list_types(&self) -> Vec<&str>;
    pub fn generate_all(&self, config: &GoldEtlConfig, fields: &[String]) -> Result<Vec<SqlColumn>, FeatureError>;
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(LagFeatureGenerator::new()));
        registry.register(Box::new(RollingFeatureGenerator::new()));
        registry.register(Box::new(TrendFeatureGenerator::new()));
        registry
    }
}
```

### FR-A06-003: Lag Feature Generator

Generate lag features (values at t-N hours):

```rust
pub struct LagFeatureGenerator;

impl FeatureGenerator for LagFeatureGenerator {
    fn feature_type(&self) -> &str { "lag" }

    fn generate_columns(&self, config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError> {
        let lag_config = config.lag.as_ref().ok_or(FeatureError::MissingConfig("lag"))?;

        let mut columns = Vec::new();
        for hours in &lag_config.lags_hours {
            columns.push(SqlColumn {
                expression: format!(
                    "LAG({field}, {hours}) OVER (PARTITION BY ndp_id ORDER BY bucket)",
                    field = field,
                    hours = hours
                ),
                alias: format!("{}_lag_{}h", field, hours),
                data_type: "DOUBLE PRECISION".to_string(),
            });
        }
        Ok(columns)
    }

    fn validate(&self, config: &FeatureConfig) -> Result<(), FeatureError> {
        if let Some(lag) = &config.lag {
            if lag.lags_hours.is_empty() {
                return Err(FeatureError::InvalidConfig("lags_hours cannot be empty"));
            }
            for hours in &lag.lags_hours {
                if *hours < 1 {
                    return Err(FeatureError::InvalidConfig("lag hours must be >= 1"));
                }
            }
        }
        Ok(())
    }

    fn requires_window(&self) -> bool { true }

    fn description(&self) -> &str { "Lag features: values at t-N hours" }
}
```

**Generated SQL Example**:
```sql
LAG(pm25_mean, 1) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_lag_1h,
LAG(pm25_mean, 6) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_lag_6h,
LAG(pm25_mean, 24) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_lag_24h
```

### FR-A06-004: Rolling Feature Generator

Generate rolling window statistics:

```rust
pub struct RollingFeatureGenerator;

impl FeatureGenerator for RollingFeatureGenerator {
    fn feature_type(&self) -> &str { "rolling" }

    fn generate_columns(&self, config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError> {
        let rolling = config.rolling.as_ref().ok_or(FeatureError::MissingConfig("rolling"))?;

        let mut columns = Vec::new();
        for window in &rolling.windows {
            let window_rows = parse_window_to_rows(window)?;

            for stat in &rolling.stats {
                let (expr, alias) = match stat.as_str() {
                    "mean" => (
                        format!("AVG({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)", field, window_rows - 1),
                        format!("{}_rolling_mean_{}", field, window.replace(" ", "_"))
                    ),
                    "std" => (
                        format!("STDDEV({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)", field, window_rows - 1),
                        format!("{}_rolling_std_{}", field, window.replace(" ", "_"))
                    ),
                    "min" => (
                        format!("MIN({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)", field, window_rows - 1),
                        format!("{}_rolling_min_{}", field, window.replace(" ", "_"))
                    ),
                    "max" => (
                        format!("MAX({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)", field, window_rows - 1),
                        format!("{}_rolling_max_{}", field, window.replace(" ", "_"))
                    ),
                    _ => return Err(FeatureError::UnknownStat(stat.clone())),
                };
                columns.push(SqlColumn {
                    expression: expr,
                    alias,
                    data_type: "DOUBLE PRECISION".to_string(),
                });
            }
        }
        Ok(columns)
    }

    fn requires_window(&self) -> bool { true }

    fn description(&self) -> &str { "Rolling window statistics" }
}
```

**Generated SQL Example**:
```sql
AVG(pm25_mean) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN 3 PRECEDING AND CURRENT ROW) AS pm25_rolling_mean_4_hours,
STDDEV(pm25_mean) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN 3 PRECEDING AND CURRENT ROW) AS pm25_rolling_std_4_hours
```

### FR-A06-005: Trend Feature Generator

Generate trend (slope) features:

```rust
pub struct TrendFeatureGenerator;

impl FeatureGenerator for TrendFeatureGenerator {
    fn feature_type(&self) -> &str { "trend" }

    fn generate_columns(&self, config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError> {
        let trend = config.trend.as_ref().ok_or(FeatureError::MissingConfig("trend"))?;
        let window_rows = parse_window_to_rows(&trend.window)?;

        // Simple slope approximation: (last - first) / window
        Ok(vec![SqlColumn {
            expression: format!(
                "(LAST_VALUE({field}) OVER w - FIRST_VALUE({field}) OVER w) / {window}.0",
                field = field,
                window = window_rows
            ),
            alias: format!("{}_trend_{}", field, trend.window.replace(" ", "_")),
            data_type: "DOUBLE PRECISION".to_string(),
        }])
    }

    fn requires_window(&self) -> bool { true }

    fn description(&self) -> &str { "Trend (slope) over window" }
}
```

**Generated SQL Example**:
```sql
(LAST_VALUE(co2_mean) OVER w - FIRST_VALUE(co2_mean) OVER w) / 4.0 AS co2_trend_4_hours
```

### FR-A06-006: Feature Configuration Structure

```rust
pub struct FeaturesConfig {
    pub lag: Option<LagConfig>,
    pub rolling: Option<RollingConfig>,
    pub trend: Option<TrendConfig>,
}

pub struct LagConfig {
    pub enabled: bool,
    pub lags_hours: Vec<u32>,
    pub fields: Vec<String>,
}

pub struct RollingConfig {
    pub enabled: bool,
    pub windows: Vec<String>,  // ["4 hours", "24 hours"]
    pub stats: Vec<String>,     // ["mean", "std"]
    pub fields: Vec<String>,
}

pub struct TrendConfig {
    pub enabled: bool,
    pub window: String,
    pub fields: Vec<String>,
}
```

### FR-A06-007: Registry Integration with DDL Generator

The continuous aggregate generator SHALL use the registry:

```rust
impl ContinuousAggregateGenerator {
    pub fn generate(&self, config: &StreamConfig) -> Result<String, GeneratorError> {
        let gold_etl = config.gold_etl.as_ref().ok_or(GeneratorError::NoGoldConfig)?;

        // Generate base aggregates
        let mut columns = self.generate_aggregates(gold_etl)?;

        // Generate features using registry
        if let Some(features) = &gold_etl.features {
            let feature_columns = self.feature_registry.generate_all(features, &config.fields)?;
            columns.extend(feature_columns);
        }

        self.build_sql(columns)
    }
}
```

### FR-A06-008: Unknown Feature Type Error

When a configuration references an unregistered feature type:

```rust
ErrorCode::InvalidFeatureType (405)
```

Error message SHALL include:
- The unknown feature type
- List of available feature types

### FR-A06-009: Field Validation

Feature generators SHALL validate that referenced fields:
1. Exist in the stream configuration
2. Are numeric type (for statistical features)

---

## Non-Functional Requirements

### NFR-A06-001: Extensibility

Adding a new feature type SHALL only require:
1. Implementing the `FeatureGenerator` trait
2. Registering with the registry

No changes to existing generators or the DDL generator required.

### NFR-A06-002: Thread Safety

The `FeatureGenerator` trait requires `Send + Sync` to allow parallel feature generation.

### NFR-A06-003: Error Propagation

Feature generation errors SHALL propagate with full context:
- Which feature type failed
- Which field was being processed
- The specific configuration issue

---

## Acceptance Criteria

### AC-A06-001: Generate Lag Features

```gherkin
Scenario: Generate lag features from config
  Given gold_etl.features.lag.enabled = true
  And gold_etl.features.lag.lags_hours = [1, 6, 24]
  And gold_etl.features.lag.fields = ["pm25"]
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the output SHALL contain "LAG(pm25_mean, 1)"
  And the output SHALL contain "AS pm25_lag_1h"
  And the output SHALL contain "LAG(pm25_mean, 6)"
  And the output SHALL contain "AS pm25_lag_6h"
  And the output SHALL contain "LAG(pm25_mean, 24)"
  And the output SHALL contain "AS pm25_lag_24h"
```

### AC-A06-002: Generate Rolling Features

```gherkin
Scenario: Generate rolling window statistics
  Given gold_etl.features.rolling.enabled = true
  And gold_etl.features.rolling.windows = ["4 hours"]
  And gold_etl.features.rolling.stats = ["mean", "std"]
  And gold_etl.features.rolling.fields = ["pm25"]
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the output SHALL contain "AVG(pm25_mean) OVER"
  And the output SHALL contain "ROWS BETWEEN 3 PRECEDING"
  And the output SHALL contain "AS pm25_rolling_mean_4_hours"
```

### AC-A06-003: Generate Trend Features

```gherkin
Scenario: Generate trend (slope) features
  Given gold_etl.features.trend.enabled = true
  And gold_etl.features.trend.window = "4 hours"
  And gold_etl.features.trend.fields = ["co2"]
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the output SHALL contain "LAST_VALUE(co2_mean)"
  And the output SHALL contain "FIRST_VALUE(co2_mean)"
  And the output SHALL contain "AS co2_trend_4_hours"
```

### AC-A06-004: Custom Feature Type Registration

```gherkin
Scenario: Register and use a custom feature type
  Given a custom PercentileFeatureGenerator implementing FeatureGenerator
  And it is registered with feature_type = "percentile"
  When I configure gold_etl.features.percentile.enabled = true
  And I run: ndp-gold-ddl generate --stream air-quality
  Then the custom generator SHALL be invoked
  And its SQL expressions SHALL appear in the output
```

### AC-A06-005: Invalid Feature Type Rejected

```gherkin
Scenario: Unknown feature type fails with helpful error
  Given gold_etl.features.unknown_feature.enabled = true
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the tool SHALL exit with code 1
  And stderr SHALL contain "Unknown feature type 'unknown_feature'"
  And stderr SHALL list available types: "lag, rolling, trend"
```

### AC-A06-006: Field Validation

```gherkin
Scenario: Feature referencing non-existent field fails
  Given gold_etl.features.lag.fields = ["nonexistent"]
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the tool SHALL exit with code 1
  And stderr SHALL contain "Field 'nonexistent' not found in stream"
```

### AC-A06-007: Multiple Feature Types Combined

```gherkin
Scenario: Generate multiple feature types in one pass
  Given lag, rolling, and trend features are all enabled
  When I run: ndp-gold-ddl generate --stream air-quality
  Then the output SHALL contain lag columns
  And the output SHALL contain rolling columns
  And the output SHALL contain trend columns
  And all feature columns SHALL appear in a single view
```

---

## Module Structure

```
tools/ndp-gold-ddl/src/
├── registry/
│   ├── mod.rs              # FeatureRegistry
│   ├── trait.rs            # FeatureGenerator trait
│   ├── lag.rs              # LagFeatureGenerator
│   ├── rolling.rs          # RollingFeatureGenerator
│   └── trend.rs            # TrendFeatureGenerator
├── generators/
│   ├── continuous_aggregate.rs  # Uses FeatureRegistry
│   └── features.rs              # Feature SQL building
└── config/
    └── types.rs            # FeaturesConfig, LagConfig, etc.
```

---

## Integration Test Requirements

### Test: Registry Default Generators

```rust
#[test]
fn test_default_registry_has_builtin_types() {
    let registry = FeatureRegistry::default();

    assert!(registry.get("lag").is_some());
    assert!(registry.get("rolling").is_some());
    assert!(registry.get("trend").is_some());
    assert!(registry.get("unknown").is_none());
}
```

### Test: Custom Generator Registration

```rust
#[test]
fn test_register_custom_generator() {
    let mut registry = FeatureRegistry::new();

    struct CustomGenerator;
    impl FeatureGenerator for CustomGenerator {
        fn feature_type(&self) -> &str { "custom" }
        // ... other methods
    }

    registry.register(Box::new(CustomGenerator));

    assert!(registry.get("custom").is_some());
}
```

### Test: SQL Generation

```rust
#[test]
fn test_lag_sql_generation() {
    let generator = LagFeatureGenerator::new();
    let config = FeatureConfig {
        lag: Some(LagConfig {
            enabled: true,
            lags_hours: vec![1, 6],
            fields: vec!["pm25".to_string()],
        }),
        ..Default::default()
    };

    let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

    assert_eq!(columns.len(), 2);
    assert!(columns[0].expression.contains("LAG(pm25_mean, 1)"));
    assert_eq!(columns[0].alias, "pm25_mean_lag_1h");
}
```

---

## London TDD Interfaces

### Trait: FeatureGenerator (already defined above)

### Mock: MockFeatureGenerator

```rust
pub struct MockFeatureGenerator {
    pub feature_type: String,
    pub columns: Vec<SqlColumn>,
    pub validate_result: Result<(), FeatureError>,
}

impl FeatureGenerator for MockFeatureGenerator {
    fn feature_type(&self) -> &str { &self.feature_type }

    fn generate_columns(&self, _config: &FeatureConfig, _field: &str) -> Result<Vec<SqlColumn>, FeatureError> {
        Ok(self.columns.clone())
    }

    fn validate(&self, _config: &FeatureConfig) -> Result<(), FeatureError> {
        self.validate_result.clone()
    }

    fn requires_window(&self) -> bool { true }
    fn description(&self) -> &str { "Mock feature generator" }
}
```

---

## Window Parsing Utility

```rust
/// Parse window string to number of hourly rows
/// "4 hours" -> 4
/// "1 day" -> 24
/// "30 minutes" -> 0 (error: sub-hourly not supported)
fn parse_window_to_rows(window: &str) -> Result<u32, FeatureError> {
    let parts: Vec<&str> = window.trim().split_whitespace().collect();
    if parts.len() != 2 {
        return Err(FeatureError::InvalidWindow(window.to_string()));
    }

    let value: u32 = parts[0].parse()
        .map_err(|_| FeatureError::InvalidWindow(window.to_string()))?;

    match parts[1] {
        "hour" | "hours" => Ok(value),
        "day" | "days" => Ok(value * 24),
        _ => Err(FeatureError::InvalidWindow(window.to_string())),
    }
}
```

---

## References

- [SPEC-A02](./SPEC-A02-gold-ddl-tool.md) - Gold DDL Tool (integrates registry)
- [SPEC-A01](./SPEC-A01-gold-etl-schema.md) - Gold ETL Schema (feature config structure)
- [SCOPE.md](../../SCOPE.md) - v11-008, v11-009: Feature computation requirements
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 5: SQL Generation Pattern
