# ColumnOrientedParser Design Document

**Feature:** air-007 - NWS Gridpoints Column-Oriented Parser
**Author:** parser-designer agent
**Date:** 2025-12-24
**Status:** SPECIFICATION

---

## Overview

The `ColumnOrientedParser` is a new parser type for handling column-oriented JSON data structures where each metric has its own dedicated values array. This parser is required for NWS raw gridpoint data and will support future data sources like Open-Meteo.

### Key Characteristics

- **Column-Oriented Structure:** Each metric is stored in a separate object with its own values array
- **ISO 8601 Duration Timestamps:** Support for timestamp formats like `"2025-12-23T00:00:00+00:00/PT1H"`
- **Parallel Array Support:** Future support for separate time arrays (Open-Meteo pattern)
- **Flexible Path Navigation:** JSON path-based navigation to locate metric containers
- **Unit Conversion:** Built-in unit conversion for non-standard units

### Use Cases

| Data Source | Format | Status |
|-------------|--------|--------|
| NWS Gridpoints (`/gridpoints/{wfo}/{x},{y}`) | ISO 8601 Duration | Primary (40+ metrics) |
| Open-Meteo Forecast | Parallel arrays | Future support |
| Custom weather APIs | Column-oriented | Extensible |

---

## Existing Parser Pattern Analysis

### ArrayIteratorParser Pattern (Reference)

The `ArrayIteratorParser` provides the established pattern for our implementation:

```rust
pub struct ArrayIteratorParser {
    config: ParserConfig,      // Base parser configuration
    array_config: ArrayIteratorConfig,  // Parser-specific config
}

impl Parser for ArrayIteratorParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        // 1. Extract location ID
        // 2. Extract metadata tags
        // 3. Extract metadata metrics
        // 4. Iterate over array
        // 5. Extract element timestamp
        // 6. Extract values per mapping
        // 7. Create TimeSeriesPoint for each metric
        // 8. Return all points
    }

    fn name(&self) -> &str { "array_iterator" }
    fn config(&self) -> &ParserConfig { &self.config }
}
```

**Key Patterns to Follow:**
- Separate config structs (`ParserConfig` + specific config)
- JSON path navigation via `extract_at_path()`
- Metadata extraction for shared fields
- Graceful error handling with warnings for optional fields
- Comprehensive logging with `tracing` macros
- Extensive unit tests

---

## Architecture Design

### Trait Implementation

```rust
use crate::error::{CoreError, CoreResult};
use crate::parsers::config::ParserConfig;
use crate::parsers::traits::Parser;
use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn, error};

/// Parser for column-oriented JSON data structures
///
/// Handles data where each metric is stored in a separate object with
/// its own values array. Each value includes a timestamp and numeric value.
///
/// Example structure (NWS Gridpoints):
/// ```json
/// {
///   "properties": {
///     "temperature": {
///       "values": [
///         {"validTime": "2025-12-23T00:00:00+00:00/PT1H", "value": 15.5},
///         {"validTime": "2025-12-23T01:00:00+00:00/PT1H", "value": 14.8}
///       ]
///     },
///     "relativeHumidity": {
///       "values": [
///         {"validTime": "2025-12-23T00:00:00+00:00/PT1H", "value": 68}
///       ]
///     }
///   }
/// }
/// ```
#[derive(Debug)]
pub struct ColumnOrientedParser {
    config: ParserConfig,
    column_config: ColumnOrientedConfig,
}

impl ColumnOrientedParser {
    /// Create from ParserConfig
    pub fn from_config(config: ParserConfig) -> CoreResult<Self> {
        let column_config = config.column_config.clone().ok_or_else(|| {
            CoreError::Config(
                "ColumnOrientedParser requires 'column_config' in ParserConfig".to_string()
            )
        })?;

        Ok(Self {
            config,
            column_config,
        })
    }

    /// Create from explicit configs (for testing)
    #[cfg(test)]
    pub fn from_configs(
        config: ParserConfig,
        column_config: ColumnOrientedConfig,
    ) -> CoreResult<Self> {
        Ok(Self {
            config,
            column_config,
        })
    }

    /// Extract value at JSON path (supports dot notation)
    fn extract_at_path<'a>(&self, root: &'a Value, path: &str) -> Option<&'a Value> {
        let mut current = root;
        for segment in path.split('.') {
            current = current.get(segment)?;
        }
        Some(current)
    }

    /// Parse ISO 8601 duration timestamp
    ///
    /// Handles NWS format: "2025-12-23T00:00:00+00:00/PT1H"
    /// Returns the datetime component (before the "/" separator)
    fn parse_iso8601_duration(&self, timestamp_str: &str) -> CoreResult<DateTime<Utc>> {
        let parts: Vec<&str> = timestamp_str.split('/').collect();
        if parts.is_empty() {
            return Err(CoreError::Source(format!(
                "Invalid ISO 8601 duration format: {}",
                timestamp_str
            )));
        }

        DateTime::parse_from_rfc3339(parts[0])
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| CoreError::Source(format!(
                "Failed to parse timestamp '{}': {}",
                parts[0], e
            )))
    }

    /// Extract timestamp from value entry
    fn extract_timestamp(
        &self,
        value_entry: &Value,
        mapping: &ColumnMapping,
    ) -> CoreResult<DateTime<Utc>> {
        let timestamp_path = mapping.timestamp_path.as_deref().unwrap_or("validTime");

        let timestamp_value = self
            .extract_at_path(value_entry, timestamp_path)
            .ok_or_else(|| {
                CoreError::Source(format!(
                    "Timestamp field '{}' not found in value entry",
                    timestamp_path
                ))
            })?;

        let timestamp_str = timestamp_value
            .as_str()
            .ok_or_else(|| CoreError::Source("Timestamp value is not a string".to_string()))?;

        match &self.column_config.timestamp_format {
            TimestampFormat::Iso8601Duration => self.parse_iso8601_duration(timestamp_str),
            TimestampFormat::ParallelArray { .. } => {
                // For parallel array format, this is handled differently
                DateTime::parse_from_rfc3339(timestamp_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| CoreError::Source(format!("Invalid RFC3339 timestamp: {}", e)))
            }
        }
    }

    /// Extract numeric value from value entry
    fn extract_value(
        &self,
        value_entry: &Value,
        mapping: &ColumnMapping,
    ) -> Option<f64> {
        let value_path = mapping.value_path.as_deref().unwrap_or("value");

        let value = self.extract_at_path(value_entry, value_path)?;

        // Try numeric extraction
        if let Some(num) = value.as_f64() {
            return Some(num);
        }
        if let Some(num) = value.as_i64() {
            return Some(num as f64);
        }
        if let Some(num) = value.as_u64() {
            return Some(num as f64);
        }

        None
    }

    /// Apply unit conversion if configured
    fn apply_unit_conversion(&self, value: f64, field_name: &str) -> f64 {
        if let Some(conversion) = self.column_config.unit_conversions.get(field_name) {
            conversion.convert(value)
        } else {
            value
        }
    }

    /// Extract location ID from payload
    fn extract_location_id(&self, payload: &Value) -> CoreResult<String> {
        if let Some(value) = self.extract_at_path(payload, &self.config.location_id_field) {
            if let Some(s) = value.as_str() {
                return Ok(s.to_string());
            }
            if let Some(num) = value.as_f64() {
                return Ok(num.to_string());
            }
        }

        self.config
            .default_location_id
            .clone()
            .ok_or_else(|| CoreError::Source("Could not extract location ID".into()))
    }
}

impl Parser for ColumnOrientedParser {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        // Extract location ID
        let location_id = self.extract_location_id(payload)?;

        // Get base metadata tags
        let mut default_tags = self.config.default_tags.clone();

        // Navigate to metrics base path
        let metrics_base = self
            .extract_at_path(payload, &self.column_config.metrics_base_path)
            .ok_or_else(|| {
                CoreError::Source(format!(
                    "Metrics base path '{}' not found",
                    self.column_config.metrics_base_path
                ))
            })?;

        let mut all_points = Vec::new();

        // Iterate over each column mapping
        for mapping in &self.column_config.columns {
            // Navigate to metric object
            let metric_obj = match self.extract_at_path(metrics_base, &mapping.metric_path) {
                Some(obj) => obj,
                None => {
                    warn!(
                        "Metric path '{}' not found, skipping column '{}'",
                        mapping.metric_path, mapping.field_name
                    );
                    continue;
                }
            };

            // Navigate to values array
            let values_path = mapping.values_path.as_deref().unwrap_or("values");
            let values_array = match self.extract_at_path(metric_obj, values_path) {
                Some(val) => val.as_array(),
                None => {
                    warn!(
                        "Values path '{}' not found in metric '{}', skipping",
                        values_path, mapping.metric_path
                    );
                    continue;
                }
            };

            let values = match values_array {
                Some(arr) => arr,
                None => {
                    warn!(
                        "Values at path '{}' is not an array in metric '{}', skipping",
                        values_path, mapping.metric_path
                    );
                    continue;
                }
            };

            debug!(
                "Processing {} values for metric '{}'",
                values.len(),
                mapping.field_name
            );

            // Process each value entry
            for (idx, value_entry) in values.iter().enumerate() {
                // Extract timestamp
                let element_timestamp = match self.extract_timestamp(value_entry, mapping) {
                    Ok(ts) => ts,
                    Err(e) => {
                        warn!(
                            "Skipping value {} in metric '{}': {}",
                            idx, mapping.field_name, e
                        );
                        continue;
                    }
                };

                // Extract numeric value
                let raw_value = match self.extract_value(value_entry, mapping) {
                    Some(v) => v,
                    None => {
                        warn!(
                            "Could not extract value {} in metric '{}', skipping",
                            idx, mapping.field_name
                        );
                        continue;
                    }
                };

                // Apply unit conversion
                let converted_value = self.apply_unit_conversion(raw_value, &mapping.field_name);

                // Build tags
                let mut tags = default_tags.clone();
                tags.insert("metric".to_string(), mapping.field_name.clone());
                tags.insert(
                    "forecast_valid_time".to_string(),
                    element_timestamp.timestamp().to_string(),
                );

                // Create point
                all_points.push(TimeSeriesPoint {
                    timestamp, // Ingestion timestamp
                    location_id: location_id.clone(),
                    value: converted_value,
                    tags,
                });

                debug!(
                    "Extracted {} = {} (converted from {}) at {}",
                    mapping.field_name, converted_value, raw_value, element_timestamp
                );
            }
        }

        if all_points.is_empty() {
            warn!("No points extracted from column-oriented data");
        } else {
            debug!(
                "Extracted {} total points from {} columns",
                all_points.len(),
                self.column_config.columns.len()
            );
        }

        Ok(all_points)
    }

    fn name(&self) -> &str {
        "column_oriented"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
}
```

---

## Configuration Structures

### ColumnOrientedConfig

```rust
/// Configuration for column-oriented parser
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnOrientedConfig {
    /// Base path to metrics container (e.g., "properties" for NWS)
    pub metrics_base_path: String,

    /// Column mappings: metric_path -> field_name
    pub columns: Vec<ColumnMapping>,

    /// Timestamp format variant
    pub timestamp_format: TimestampFormat,

    /// Unit conversions
    #[serde(default)]
    pub unit_conversions: HashMap<String, UnitConversion>,
}

/// Mapping for a single column/metric
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColumnMapping {
    /// Path within metrics base (e.g., "temperature" for NWS)
    pub metric_path: String,

    /// Output field name in TimeSeriesPoint
    pub field_name: String,

    /// Path to values array within metric (default: "values")
    #[serde(default)]
    pub values_path: Option<String>,

    /// Path to timestamp within value entry (default: "validTime")
    #[serde(default)]
    pub timestamp_path: Option<String>,

    /// Path to value within entry (default: "value")
    #[serde(default)]
    pub value_path: Option<String>,
}

/// Timestamp format variants
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimestampFormat {
    /// NWS format: "2025-12-23T00:00:00+00:00/PT1H"
    /// Split on "/" and parse first component
    Iso8601Duration,

    /// Open-Meteo format: Separate time array
    /// Time values are in a parallel array at specified path
    ParallelArray {
        /// Path to time array (e.g., "hourly.time")
        time_path: String,
    },
}

/// Unit conversion configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UnitConversion {
    /// Source unit identifier
    pub from: String,

    /// Target unit identifier
    pub to: String,

    /// Optional conversion factor (for simple multiplication)
    #[serde(default)]
    pub factor: Option<f64>,

    /// Optional conversion formula (for complex conversions)
    #[serde(default)]
    pub formula: Option<ConversionFormula>,
}

impl UnitConversion {
    /// Apply conversion to value
    pub fn convert(&self, value: f64) -> f64 {
        if let Some(factor) = self.factor {
            value * factor
        } else if let Some(formula) = &self.formula {
            formula.apply(value)
        } else {
            value // No conversion
        }
    }
}

/// Conversion formula types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversionFormula {
    /// Linear: (value * scale) + offset
    Linear { scale: f64, offset: f64 },

    /// Custom Rust code (future enhancement)
    Custom { code: String },
}

impl ConversionFormula {
    /// Apply formula to value
    pub fn apply(&self, value: f64) -> f64 {
        match self {
            ConversionFormula::Linear { scale, offset } => (value * scale) + offset,
            ConversionFormula::Custom { .. } => {
                // Future: compile and execute custom code
                value
            }
        }
    }
}
```

### ParserConfig Extension

Add to `core/src/parsers/config.rs`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParserConfig {
    // ... existing fields ...

    /// For ColumnOrientedParser: column-specific configuration
    #[serde(default)]
    pub column_config: Option<ColumnOrientedConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserType {
    FlatJson,
    JsonPath,
    ArrayIterator,
    ColumnOriented,  // NEW
    Custom(String),
}
```

---

## YAML Configuration Examples

### NWS Gridpoints Configuration

```yaml
parser:
  type: column_oriented
  location_id_field: "geometry.coordinates"
  default_location_id: "nws_grid_jax_88_67"
  default_tags:
    source: "nws"
    data_type: "gridpoint"

  column_config:
    metrics_base_path: "properties"

    timestamp_format:
      type: iso8601_duration

    columns:
      # Temperature
      - metric_path: "temperature"
        field_name: "temperature_c"
        # values_path defaults to "values"
        # timestamp_path defaults to "validTime"
        # value_path defaults to "value"

      # Dewpoint
      - metric_path: "dewpoint"
        field_name: "dewpoint_c"

      # Relative Humidity
      - metric_path: "relativeHumidity"
        field_name: "relative_humidity_pct"

      # Sky Cover
      - metric_path: "skyCover"
        field_name: "sky_cover_pct"

      # Wind Speed
      - metric_path: "windSpeed"
        field_name: "wind_speed_kmh"

      # Wind Direction
      - metric_path: "windDirection"
        field_name: "wind_direction_deg"

      # ... (40+ total columns for complete NWS data)

    unit_conversions:
      # Example conversions (if needed)
      temperature_c:
        from: "wmoUnit:degC"
        to: "celsius"
        # No conversion needed, just validation
```

### Open-Meteo Configuration (Future)

```yaml
parser:
  type: column_oriented
  location_id_field: "latitude"
  default_location_id: "open_meteo_30.22_-81.63"

  column_config:
    metrics_base_path: "hourly"

    timestamp_format:
      type: parallel_array
      time_path: "time"

    columns:
      - metric_path: "temperature_2m"
        field_name: "temperature_c"

      - metric_path: "relative_humidity_2m"
        field_name: "relative_humidity_pct"

      - metric_path: "cloud_cover"
        field_name: "sky_cover_pct"
```

---

## Error Handling Strategy

### Graceful Degradation

Following the `ArrayIteratorParser` pattern:

1. **Missing Columns:** Warn and skip, don't fail entire parse
   ```rust
   warn!("Metric path '{}' not found, skipping", mapping.metric_path);
   continue;
   ```

2. **Invalid Timestamps:** Log error, skip individual entry
   ```rust
   Err(e) => {
       warn!("Skipping value {}: {}", idx, e);
       continue;
   }
   ```

3. **Type Mismatches:** Attempt coercion, skip if fails
   ```rust
   None => {
       warn!("Could not extract value, skipping");
       continue;
   }
   ```

4. **Fatal Errors:** Only fail on structural issues
   - Missing metrics_base_path
   - Invalid configuration

### Logging Levels

| Level | Use Case |
|-------|----------|
| `error!` | Fatal configuration errors |
| `warn!` | Missing optional data, skipped entries |
| `info!` | Parse completion summary |
| `debug!` | Individual value extraction details |

---

## ISO 8601 Duration Parsing

### NWS Format

```
"2025-12-23T00:00:00+00:00/PT1H"
 └─────┬──────────────────┘ └─┬─┘
       │                      │
   Start datetime        Duration
```

### Parsing Algorithm

```rust
fn parse_iso8601_duration(&self, timestamp_str: &str) -> CoreResult<DateTime<Utc>> {
    // Split on "/" separator
    let parts: Vec<&str> = timestamp_str.split('/').collect();

    if parts.is_empty() {
        return Err(CoreError::Source(format!(
            "Invalid ISO 8601 duration format: {}",
            timestamp_str
        )));
    }

    // Parse the datetime component (before the "/")
    DateTime::parse_from_rfc3339(parts[0])
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| CoreError::Source(format!(
            "Failed to parse timestamp '{}': {}",
            parts[0], e
        )))
}
```

**Note:** The duration component (e.g., "PT1H") indicates the validity period but is not used for timestamp extraction. We use the start datetime as the timestamp for the data point.

### Duration Values

| Duration | Meaning |
|----------|---------|
| PT1H | 1 hour |
| PT3H | 3 hours |
| PT6H | 6 hours |
| P1D | 1 day |

---

## Integration Points

### Parser Factory

Add to `core/src/parsers/factory.rs`:

```rust
use crate::parsers::column_oriented::ColumnOrientedParser;

pub fn create_parser(config: ParserConfig) -> CoreResult<Box<dyn Parser>> {
    match config.parser_type {
        ParserType::FlatJson => Ok(Box::new(FlatJsonParser::from_config(config)?)),
        ParserType::JsonPath => Ok(Box::new(JsonPathParser::from_config(config)?)),
        ParserType::ArrayIterator => Ok(Box::new(ArrayIteratorParser::from_config(config)?)),
        ParserType::ColumnOriented => Ok(Box::new(ColumnOrientedParser::from_config(config)?)),
        ParserType::Custom(name) => {
            Err(CoreError::Config(format!("Unknown custom parser: {}", name)))
        }
    }
}
```

### Module Structure

```
core/src/parsers/
├── mod.rs                    # Export ColumnOrientedParser
├── traits.rs                 # Parser trait (unchanged)
├── config.rs                 # Add ColumnOrientedConfig, TimestampFormat, UnitConversion
├── factory.rs                # Add ColumnOriented case
├── flat_json.rs              # Existing
├── json_path.rs              # Existing
├── array_iterator.rs         # Existing
└── column_oriented.rs        # NEW - Implementation
```

---

## Testing Strategy

### Unit Tests

Following `ArrayIteratorParser` test patterns:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso8601_duration_parsing() {
        // Test NWS timestamp format
    }

    #[test]
    fn test_column_extraction_produces_correct_points() {
        // Verify point count: columns × values
    }

    #[test]
    fn test_missing_column_gracefully_skipped() {
        // Warn and continue
    }

    #[test]
    fn test_invalid_timestamp_skips_entry() {
        // Skip bad entry, process others
    }

    #[test]
    fn test_unit_conversion_applied() {
        // Verify conversion math
    }

    #[test]
    fn test_nested_path_navigation() {
        // Test metrics_base_path
    }

    #[test]
    fn test_default_paths_work() {
        // values_path, timestamp_path, value_path defaults
    }

    #[test]
    fn test_custom_paths_override() {
        // Custom paths override defaults
    }

    #[test]
    fn test_parallel_array_timestamp_format() {
        // Future: Open-Meteo format
    }
}
```

### Integration Tests

Create test fixture with real NWS gridpoint response:

```rust
#[test]
fn test_real_nws_gridpoint_data() {
    let payload = include_str!("../../tests/fixtures/nws_gridpoint_jax.json");
    let payload_value: Value = serde_json::from_str(payload).unwrap();

    let parser = create_test_parser_with_40_columns();
    let points = parser.parse(&payload_value, Utc::now()).unwrap();

    // Verify we got points for all 40+ metrics
    assert!(points.len() > 100); // 40 metrics × ~156 hours
}
```

---

## Performance Considerations

### Optimizations

1. **Pre-allocate Point Vector:** Estimate capacity based on columns × expected values
   ```rust
   let estimated_capacity = self.column_config.columns.len() * 200; // Assume 200 values per column
   let mut all_points = Vec::with_capacity(estimated_capacity);
   ```

2. **Clone Avoidance:** Reuse location_id and default_tags references where possible

3. **Regex Caching:** Not needed for column-oriented (no string parsing)

4. **Path Caching:** Consider caching parsed paths if performance testing shows bottleneck

### Expected Performance

| Metric | Expected |
|--------|----------|
| 40 columns × 156 values | ~6,240 points |
| Parse time | < 50ms |
| Memory allocation | ~500 KB |

---

## Migration Path

### Phase 1: Core Implementation
1. Add `ColumnOrientedConfig` to `config.rs`
2. Implement `ColumnOrientedParser` in `column_oriented.rs`
3. Add to `ParserType` enum
4. Update `factory.rs`
5. Write unit tests

### Phase 2: NWS Integration
1. Create NWS gridpoint stream configuration
2. Add all 40+ column mappings
3. Test with real NWS data
4. Deploy to Pi

### Phase 3: Future Extensions
1. Add parallel array timestamp support (Open-Meteo)
2. Add custom conversion formulas
3. Add column filtering/selection

---

## Reference Files

| File | Purpose |
|------|---------|
| `/workspaces/neural-data-platform/core/src/parsers/traits.rs` | Parser trait definition |
| `/workspaces/neural-data-platform/core/src/parsers/config.rs` | Configuration structures |
| `/workspaces/neural-data-platform/core/src/parsers/array_iterator.rs` | Reference implementation pattern |
| `/workspaces/neural-data-platform/product/research/weatherresources/NWS-COMPLETE-ANALYSIS.md` | NWS data structure analysis |

---

## Success Criteria

- [ ] Parses NWS gridpoint data correctly (40+ metrics)
- [ ] Handles ISO 8601 duration timestamps
- [ ] Gracefully skips missing columns with warnings
- [ ] Applies unit conversions correctly
- [ ] Follows existing parser patterns (ArrayIteratorParser)
- [ ] Passes all unit tests (>90% coverage)
- [ ] Integrates with parser factory
- [ ] Performance: < 50ms for 6,240 points

---

## Next Steps

1. **Implementation:** `ndp-rust-dev` to implement `ColumnOrientedParser`
2. **Testing:** `ndp-tester` to write comprehensive tests
3. **Integration:** Add to parser factory and stream configuration
4. **Documentation:** Update parser documentation with examples
