# ALGO-feature-registry: Extensible Feature Generator System

> **Algorithm ID:** A04
> **Feature:** v11-A06 (Feature Type Registry)
> **Created:** 2026-02-04

---

## Purpose

Provide an extensible registry for feature computation types. The registry allows new feature types (lag, rolling, trend, etc.) to be added via trait implementation without modifying the core DDL generator.

---

## Core Trait: FeatureGenerator

```
TRAIT FeatureGenerator:
    REQUIRES: Send + Sync  // Thread-safe

    // Unique identifier for this feature type
    METHOD feature_type() -> &str

    // Human-readable description
    METHOD description() -> &str

    // Generate SQL column expressions for this feature
    METHOD generate_columns(
        config: &FeatureConfig,
        field: &str
    ) -> Result<Vec<SqlColumn>, FeatureError>

    // Validate the feature configuration
    METHOD validate(config: &FeatureConfig) -> Result<(), FeatureError>

    // Whether this feature requires window functions
    METHOD requires_window() -> bool
```

---

## Data Structures

### SqlColumn

```
STRUCT SqlColumn:
    expression: String    // SQL expression (e.g., "LAG(pm25_mean, 1) OVER (...)")
    alias: String         // Column name (e.g., "pm25_mean_lag_1h")
    data_type: String     // PostgreSQL type (e.g., "DOUBLE PRECISION")
```

### FeatureConfig

```
STRUCT FeatureConfig:
    lag: Option<LagConfig>
    rolling: Option<RollingConfig>
    trend: Option<TrendConfig>

STRUCT LagConfig:
    enabled: bool
    lags_hours: Vec<u32>      // [1, 6, 24]
    fields: Vec<String>       // ["pm25", "co2"]

STRUCT RollingConfig:
    enabled: bool
    windows: Vec<String>      // ["4 hours", "24 hours"]
    stats: Vec<String>        // ["mean", "std"]
    fields: Vec<String>

STRUCT TrendConfig:
    enabled: bool
    window: String            // "4 hours"
    fields: Vec<String>
```

---

## Algorithm: Feature Registry

```
ALGORITHM: FeatureRegistry
STATE:
    generators: HashMap<String, Box<dyn FeatureGenerator>>

METHOD new() -> Self:
    RETURN Self { generators: HashMap::new() }

METHOD register(generator: Box<dyn FeatureGenerator>):
    key <- generator.feature_type().to_string()
    self.generators.insert(key, generator)

METHOD get(feature_type: &str) -> Option<&dyn FeatureGenerator>:
    RETURN self.generators.get(feature_type).map(|g| g.as_ref())

METHOD list_types() -> Vec<&str>:
    RETURN self.generators.keys().collect()

METHOD default() -> Self:
    registry <- Self::new()
    registry.register(Box::new(LagFeatureGenerator::new()))
    registry.register(Box::new(RollingFeatureGenerator::new()))
    registry.register(Box::new(TrendFeatureGenerator::new()))
    RETURN registry
```

---

## Algorithm: Generate All Features

```
ALGORITHM: GenerateAllFeatures
INPUT:
    registry: &FeatureRegistry
    config: &FeaturesConfig
    valid_fields: &HashSet<String>
OUTPUT: Result<Vec<SqlColumn>, FeatureError>

BEGIN
    all_columns <- Vec::new()

    // Process each enabled feature type
    IF config.lag IS Some(lag) AND lag.enabled THEN
        // Validate field references
        validate_field_references(lag.fields, valid_fields, "lag")?

        lag_gen <- registry.get("lag")
            .ok_or(FeatureError::UnknownType("lag"))?

        lag_gen.validate(config)?

        FOR EACH field IN lag.fields DO
            columns <- lag_gen.generate_columns(config, field)?
            all_columns.extend(columns)
        END FOR
    END IF

    IF config.rolling IS Some(rolling) AND rolling.enabled THEN
        validate_field_references(rolling.fields, valid_fields, "rolling")?

        rolling_gen <- registry.get("rolling")
            .ok_or(FeatureError::UnknownType("rolling"))?

        rolling_gen.validate(config)?

        FOR EACH field IN rolling.fields DO
            columns <- rolling_gen.generate_columns(config, field)?
            all_columns.extend(columns)
        END FOR
    END IF

    IF config.trend IS Some(trend) AND trend.enabled THEN
        validate_field_references(trend.fields, valid_fields, "trend")?

        trend_gen <- registry.get("trend")
            .ok_or(FeatureError::UnknownType("trend"))?

        trend_gen.validate(config)?

        FOR EACH field IN trend.fields DO
            columns <- trend_gen.generate_columns(config, field)?
            all_columns.extend(columns)
        END FOR
    END IF

    RETURN Ok(all_columns)
END
```

---

## Algorithm: Lag Feature Generator

```
ALGORITHM: LagFeatureGenerator
IMPLEMENTS: FeatureGenerator

METHOD feature_type() -> &str:
    RETURN "lag"

METHOD description() -> &str:
    RETURN "Lag features: values at t-N hours"

METHOD requires_window() -> bool:
    RETURN true

METHOD validate(config: &FeatureConfig) -> Result<(), FeatureError>:
    IF config.lag.is_none() THEN
        RETURN Ok(())  // Not configured, nothing to validate
    END IF

    lag <- config.lag.unwrap()

    IF lag.lags_hours.is_empty() THEN
        RETURN Err(FeatureError::InvalidConfig {
            feature: "lag",
            message: "lags_hours cannot be empty",
            suggestion: "Add at least one lag value, e.g., lags_hours: [1, 6, 24]"
        })
    END IF

    FOR EACH hours IN lag.lags_hours DO
        IF hours < 1 THEN
            RETURN Err(FeatureError::InvalidConfig {
                feature: "lag",
                message: format!("lag hours must be >= 1, got {}", hours),
                suggestion: "Use positive integer values for lag hours"
            })
        END IF
    END FOR

    RETURN Ok(())

METHOD generate_columns(config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError>:
    lag <- config.lag.as_ref().ok_or(FeatureError::MissingConfig("lag"))?

    columns <- Vec::new()

    // Generate a LAG expression for each configured lag period
    FOR EACH hours IN lag.lags_hours DO
        // LAG operates on hourly buckets, so hours = row offset
        expression <- format!(
            "LAG({field}, {hours}) OVER (PARTITION BY ndp_id ORDER BY bucket)",
            field = field,
            hours = hours
        )

        alias <- format!("{}_lag_{}h", field, hours)

        columns.push(SqlColumn {
            expression: expression,
            alias: alias,
            data_type: "DOUBLE PRECISION"
        })
    END FOR

    RETURN Ok(columns)
```

**Generated SQL Example:**
```sql
LAG(pm25_mean, 1) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_mean_lag_1h,
LAG(pm25_mean, 6) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_mean_lag_6h,
LAG(pm25_mean, 24) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_mean_lag_24h
```

---

## Algorithm: Rolling Feature Generator

```
ALGORITHM: RollingFeatureGenerator
IMPLEMENTS: FeatureGenerator

METHOD feature_type() -> &str:
    RETURN "rolling"

METHOD description() -> &str:
    RETURN "Rolling window statistics (mean, std, min, max)"

METHOD requires_window() -> bool:
    RETURN true

METHOD validate(config: &FeatureConfig) -> Result<(), FeatureError>:
    IF config.rolling.is_none() THEN
        RETURN Ok(())
    END IF

    rolling <- config.rolling.unwrap()

    IF rolling.windows.is_empty() THEN
        RETURN Err(FeatureError::InvalidConfig {
            feature: "rolling",
            message: "windows cannot be empty"
        })
    END IF

    IF rolling.stats.is_empty() THEN
        RETURN Err(FeatureError::InvalidConfig {
            feature: "rolling",
            message: "stats cannot be empty"
        })
    END IF

    // Validate each window
    FOR EACH window IN rolling.windows DO
        parse_window_to_rows(window)?  // Will error on invalid format
    END FOR

    // Validate each stat
    valid_stats <- ["mean", "std", "min", "max"]
    FOR EACH stat IN rolling.stats DO
        IF NOT valid_stats.contains(stat.to_lowercase()) THEN
            RETURN Err(FeatureError::UnknownStat {
                stat: stat,
                valid_stats: valid_stats
            })
        END IF
    END FOR

    RETURN Ok(())

METHOD generate_columns(config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError>:
    rolling <- config.rolling.as_ref().ok_or(FeatureError::MissingConfig("rolling"))?

    columns <- Vec::new()

    FOR EACH window IN rolling.windows DO
        window_rows <- parse_window_to_rows(window)?

        FOR EACH stat IN rolling.stats DO
            (expression, data_type) <- MATCH stat.to_lowercase() WITH
                | "mean" =>
                    (format!(
                        "AVG({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)",
                        field,
                        window_rows - 1
                    ), "DOUBLE PRECISION")

                | "std" =>
                    (format!(
                        "STDDEV({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)",
                        field,
                        window_rows - 1
                    ), "DOUBLE PRECISION")

                | "min" =>
                    (format!(
                        "MIN({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)",
                        field,
                        window_rows - 1
                    ), "DOUBLE PRECISION")

                | "max" =>
                    (format!(
                        "MAX({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)",
                        field,
                        window_rows - 1
                    ), "DOUBLE PRECISION")

            // Generate alias: pm25_mean_rolling_mean_4_hours
            window_suffix <- window.replace(" ", "_")
            alias <- format!("{}_rolling_{}_{}", field, stat, window_suffix)

            columns.push(SqlColumn {
                expression: expression,
                alias: alias,
                data_type: data_type
            })
        END FOR
    END FOR

    RETURN Ok(columns)
```

**Generated SQL Example:**
```sql
AVG(pm25_mean) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN 3 PRECEDING AND CURRENT ROW) AS pm25_mean_rolling_mean_4_hours,
STDDEV(pm25_mean) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN 3 PRECEDING AND CURRENT ROW) AS pm25_mean_rolling_std_4_hours,
AVG(pm25_mean) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN 23 PRECEDING AND CURRENT ROW) AS pm25_mean_rolling_mean_24_hours
```

---

## Algorithm: Trend Feature Generator

```
ALGORITHM: TrendFeatureGenerator
IMPLEMENTS: FeatureGenerator

METHOD feature_type() -> &str:
    RETURN "trend"

METHOD description() -> &str:
    RETURN "Trend (slope) over window using simple difference"

METHOD requires_window() -> bool:
    RETURN true

METHOD validate(config: &FeatureConfig) -> Result<(), FeatureError>:
    IF config.trend.is_none() THEN
        RETURN Ok(())
    END IF

    trend <- config.trend.unwrap()

    IF trend.window.is_empty() THEN
        RETURN Err(FeatureError::InvalidConfig {
            feature: "trend",
            message: "window cannot be empty"
        })
    END IF

    // Validate window format
    parse_window_to_rows(trend.window)?

    RETURN Ok(())

METHOD generate_columns(config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError>:
    trend <- config.trend.as_ref().ok_or(FeatureError::MissingConfig("trend"))?

    window_rows <- parse_window_to_rows(trend.window)?

    // Simple slope approximation: (last - first) / window
    // For a 4-hour window with hourly data, this gives units of "change per hour"
    expression <- format!(r#"
(
    LAST_VALUE({field}) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN {preceding} PRECEDING AND CURRENT ROW
    ) -
    FIRST_VALUE({field}) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN {preceding} PRECEDING AND CURRENT ROW
    )
) / {window_rows}.0"#,
        field = field,
        preceding = window_rows - 1,
        window_rows = window_rows
    )

    window_suffix <- trend.window.replace(" ", "_")
    alias <- format!("{}_trend_{}", field, window_suffix)

    columns <- vec![SqlColumn {
        expression: expression.trim().to_string(),
        alias: alias,
        data_type: "DOUBLE PRECISION"
    }]

    RETURN Ok(columns)
```

**Generated SQL Example:**
```sql
(
    LAST_VALUE(co2_mean) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN 3 PRECEDING AND CURRENT ROW
    ) -
    FIRST_VALUE(co2_mean) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN 3 PRECEDING AND CURRENT ROW
    )
) / 4.0 AS co2_mean_trend_4_hours
```

---

## Algorithm: Parse Window to Rows

```
ALGORITHM: ParseWindowToRows
INPUT: window: String (e.g., "4 hours", "1 day")
OUTPUT: Result<u32, FeatureError> (number of hourly rows)

BEGIN
    parts <- window.trim().split_whitespace().collect::<Vec<_>>()

    IF parts.len() != 2 THEN
        RETURN Err(FeatureError::InvalidWindow {
            window: window,
            message: "Expected format: '<number> <unit>'",
            examples: ["4 hours", "1 day", "24 hours"]
        })
    END IF

    // Parse the numeric value
    value <- TRY parts[0].parse::<u32>()
        CATCH => RETURN Err(FeatureError::InvalidWindow {
            window: window,
            message: format!("'{}' is not a valid number", parts[0])
        })

    IF value < 1 THEN
        RETURN Err(FeatureError::InvalidWindow {
            window: window,
            message: "Window value must be >= 1"
        })
    END IF

    // Convert to hourly rows based on unit
    rows <- MATCH parts[1].to_lowercase() WITH
        | "hour" | "hours" =>
            value

        | "day" | "days" =>
            value * 24

        | "minute" | "minutes" =>
            // Sub-hourly not supported for hourly aggregates
            RETURN Err(FeatureError::InvalidWindow {
                window: window,
                message: "Sub-hourly windows not supported for hourly aggregates",
                suggestion: "Use hours or days for window specification"
            })

        | "week" | "weeks" =>
            value * 24 * 7

        | unknown =>
            RETURN Err(FeatureError::InvalidWindow {
                window: window,
                message: format!("Unknown time unit: '{}'", unknown),
                examples: ["hours", "days", "weeks"]
            })

    RETURN Ok(rows)
END
```

---

## Algorithm: Validate Field References

```
ALGORITHM: ValidateFieldReferences
INPUT:
    fields: Vec<String>          // Fields referenced in feature config
    valid_fields: HashSet<String> // Fields from stream config
    feature_type: &str
OUTPUT: Result<(), FeatureError>

BEGIN
    FOR EACH field IN fields DO
        // Check if base field exists (without _mean suffix)
        base_field <- field.trim_end_matches("_mean")

        // Check variations: pm25, pm25_mean, or in aggregates
        field_exists <- valid_fields.contains(field)
            OR valid_fields.contains(base_field)
            OR valid_fields.contains(format!("{}_mean", base_field))

        IF NOT field_exists THEN
            // Find similar field for suggestion
            similar <- find_similar_field(field, valid_fields)

            RETURN Err(FeatureError::InvalidField {
                code: 400,
                field: field,
                feature_type: feature_type,
                suggestion: similar.map(|s| format!("Did you mean '{}'?", s))
            })
        END IF
    END FOR

    RETURN Ok(())
END
```

---

## Algorithm: Custom Feature Registration

```
ALGORITHM: RegisterCustomFeature
INPUT:
    registry: &mut FeatureRegistry
    generator: Box<dyn FeatureGenerator>
OUTPUT: Result<(), RegistryError>

BEGIN
    feature_type <- generator.feature_type()

    // Check for collision
    IF registry.generators.contains_key(feature_type) THEN
        RETURN Err(RegistryError::TypeAlreadyRegistered {
            feature_type: feature_type,
            suggestion: "Use a unique feature_type identifier"
        })
    END IF

    registry.generators.insert(feature_type.to_string(), generator)

    RETURN Ok(())
END
```

---

## Example: Custom Percentile Feature Generator

```
ALGORITHM: PercentileFeatureGenerator
IMPLEMENTS: FeatureGenerator

STATE:
    percentile: f64  // e.g., 0.95 for p95

METHOD new(percentile: f64) -> Self:
    RETURN Self { percentile }

METHOD feature_type() -> &str:
    RETURN "percentile"

METHOD description() -> &str:
    RETURN format!("Percentile {} feature", self.percentile)

METHOD requires_window() -> bool:
    RETURN true

METHOD validate(config: &FeatureConfig) -> Result<(), FeatureError>:
    IF self.percentile < 0.0 OR self.percentile > 1.0 THEN
        RETURN Err(FeatureError::InvalidConfig {
            feature: "percentile",
            message: "percentile must be between 0 and 1"
        })
    END IF
    RETURN Ok(())

METHOD generate_columns(config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>, FeatureError>:
    // Generate rolling percentile using PERCENTILE_CONT
    expression <- format!(
        "PERCENTILE_CONT({}) WITHIN GROUP (ORDER BY {}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN 23 PRECEDING AND CURRENT ROW)",
        self.percentile,
        field
    )

    percentile_name <- format!("p{}", (self.percentile * 100.0) as u32)
    alias <- format!("{}_{}_24h", field, percentile_name)

    RETURN Ok(vec![SqlColumn {
        expression: expression,
        alias: alias,
        data_type: "DOUBLE PRECISION"
    }])
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Registry lookup | O(1) hash lookup | O(1) |
| Register feature | O(1) | O(1) |
| List types | O(n) | O(n) |
| Generate columns (lag) | O(l) per field | O(l) |
| Generate columns (rolling) | O(w * s) per field | O(w * s) |
| Generate columns (trend) | O(1) per field | O(1) |
| Parse window | O(1) | O(1) |

Where:
- n = number of registered feature types
- l = number of lag periods
- w = number of windows
- s = number of stats

---

## Error Handling

```
ENUM FeatureError:
    // Missing feature configuration
    MissingConfig {
        feature: String
    }

    // Invalid configuration value
    InvalidConfig {
        feature: String,
        message: String,
        suggestion: Option<String>
    }

    // Unknown feature type (code 405)
    UnknownType {
        feature_type: String,
        available_types: Vec<String>
    }

    // Unknown statistic
    UnknownStat {
        stat: String,
        valid_stats: Vec<String>
    }

    // Invalid window format
    InvalidWindow {
        window: String,
        message: String,
        examples: Vec<String>
    }

    // Invalid field reference (code 400)
    InvalidField {
        code: u16,
        field: String,
        feature_type: String,
        suggestion: Option<String>
    }

    // Type already registered
    TypeAlreadyRegistered {
        feature_type: String
    }
```

---

## Invariants

1. **Thread Safety**: All generators implement `Send + Sync`
2. **Idempotent Registration**: Registering same type twice returns error
3. **Default Registry**: Default registry includes lag, rolling, trend
4. **Window Validation**: All window strings must parse to valid row counts
5. **Field Validation**: All field references must exist in stream config
6. **Column Naming**: Aliases follow pattern `{field}_{feature}_{params}`

---

## Test Cases (London TDD)

```
TEST: DefaultRegistryHasBuiltinTypes
    GIVEN registry = FeatureRegistry::default()
    THEN registry.get("lag").is_some()
    AND registry.get("rolling").is_some()
    AND registry.get("trend").is_some()
    AND registry.get("unknown").is_none()

TEST: RegisterCustomGenerator
    GIVEN empty registry
    AND custom generator with feature_type = "custom"
    WHEN registry.register(custom_generator)
    THEN registry.get("custom").is_some()

TEST: LagGeneratesCorrectColumns
    GIVEN lag config with lags_hours = [1, 6, 24]
    AND field = "pm25_mean"
    WHEN lag_generator.generate_columns() is called
    THEN result.len() == 3
    AND result[0].alias == "pm25_mean_lag_1h"
    AND result[0].expression contains "LAG(pm25_mean, 1)"

TEST: RollingGeneratesWindowedStats
    GIVEN rolling config with windows = ["4 hours"], stats = ["mean", "std"]
    AND field = "pm25_mean"
    WHEN rolling_generator.generate_columns() is called
    THEN result.len() == 2
    AND result contains "ROWS BETWEEN 3 PRECEDING"
    AND result contains "AVG" AND "STDDEV"

TEST: TrendCalculatesSlope
    GIVEN trend config with window = "4 hours"
    AND field = "co2_mean"
    WHEN trend_generator.generate_columns() is called
    THEN expression contains "LAST_VALUE" AND "FIRST_VALUE"
    AND expression contains "/ 4.0"

TEST: ParseWindowToRows
    ASSERT parse_window_to_rows("4 hours") == 4
    ASSERT parse_window_to_rows("1 day") == 24
    ASSERT parse_window_to_rows("2 weeks") == 336
    ASSERT parse_window_to_rows("30 minutes").is_err()

TEST: ValidateRejectsEmptyLags
    GIVEN lag config with lags_hours = []
    WHEN lag_generator.validate() is called
    THEN result.is_err()
    AND error.message contains "cannot be empty"

TEST: ValidateRejectsInvalidStat
    GIVEN rolling config with stats = ["invalid"]
    WHEN rolling_generator.validate() is called
    THEN result.is_err()
    AND error contains available stats

TEST: FieldValidationWithSuggestion
    GIVEN valid_fields = ["pm25", "co2", "temperature"]
    AND feature references "pm52"
    WHEN validate_field_references() is called
    THEN error.suggestion contains "pm25"
```

---

## Integration with Continuous Aggregate Generator

The Feature Registry integrates with the Continuous Aggregate Generator (A02) as follows:

1. **During Generation**:
   ```
   generator.generate_continuous_aggregate(config, action) ->
       // Generate base aggregates
       columns = generate_aggregates(config)

       // Add features if configured
       IF config.features IS Some THEN
           feature_columns = registry.generate_all(config.features, valid_fields)
           columns.extend(feature_columns)
       END IF
   ```

2. **Window Function Compatibility**:
   - All feature generators use window functions
   - TimescaleDB 2.10+ supports window functions in continuous aggregates
   - For older versions, features computed in secondary view

3. **Column Naming**:
   - Base aggregates: `{field}_{metric}` (e.g., `pm25_mean`)
   - Lag features: `{field}_lag_{N}h` (e.g., `pm25_mean_lag_1h`)
   - Rolling features: `{field}_rolling_{stat}_{window}` (e.g., `pm25_mean_rolling_std_4_hours`)
   - Trend features: `{field}_trend_{window}` (e.g., `co2_mean_trend_4_hours`)

---

## References

- [SPEC-A06](../specification/SPEC-A06-feature-registry.md) - Full specification
- [DECISIONS.md](../../architecture/DECISIONS.md) - Q3: Trend Computation Method
