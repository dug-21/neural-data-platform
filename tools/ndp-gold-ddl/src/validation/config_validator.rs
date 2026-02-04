//! Configuration validation for Gold DDL generation
//!
//! Validates gold_etl configuration before generating DDL.

use crate::config::{FeaturesConfig, StreamConfig, VALID_METRICS, VALID_ROLLING_STATS};
use crate::error::{GoldDdlError, Result};
use std::collections::HashSet;

/// Validates Gold ETL configuration
pub struct ConfigValidator;

impl ConfigValidator {
    /// Create a new validator
    pub fn new() -> Self {
        Self
    }

    /// Validate the entire stream configuration for Gold DDL generation
    pub fn validate(&self, config: &StreamConfig) -> Result<()> {
        // Check gold_etl exists and is enabled
        let gold_etl = config.gold_etl.as_ref().ok_or_else(|| {
            GoldDdlError::MissingRequiredField {
                field: "gold_etl".to_string(),
                context: format!("stream '{}'", config.stream_id),
            }
        })?;

        if !gold_etl.enabled {
            return Err(GoldDdlError::GoldEtlDisabled {
                stream_id: config.stream_id.clone(),
            });
        }

        // Get available field names from stream
        let stream_fields: HashSet<_> = config.fields.iter().map(|f| f.name.as_str()).collect();

        // Validate aggregates
        if let Some(ref aggregates) = gold_etl.aggregates {
            self.validate_granularities(&aggregates.granularities)?;
            self.validate_aggregate_fields(
                &aggregates.fields,
                &stream_fields,
                &config.stream_id,
            )?;
        }

        // Validate features
        if let Some(ref features) = gold_etl.features {
            self.validate_features(features, &stream_fields, &config.stream_id)?;
        }

        Ok(())
    }

    /// Validate granularity formats
    fn validate_granularities(&self, granularities: &[String]) -> Result<()> {
        for granularity in granularities {
            parse_granularity(granularity)?;
        }
        Ok(())
    }

    /// Validate aggregate field references and metrics
    fn validate_aggregate_fields(
        &self,
        fields: &std::collections::HashMap<String, crate::config::FieldMetricsConfig>,
        stream_fields: &HashSet<&str>,
        stream_id: &str,
    ) -> Result<()> {
        for (field_name, field_config) in fields {
            // Check field exists in stream
            if !stream_fields.contains(field_name.as_str()) {
                return Err(GoldDdlError::FieldNotFound {
                    field: field_name.clone(),
                    stream_id: stream_id.to_string(),
                    available: stream_fields.iter().map(|s| s.to_string()).collect(),
                });
            }

            // Check all metrics are valid
            for metric in &field_config.metrics {
                if !VALID_METRICS.contains(&metric.as_str()) {
                    return Err(GoldDdlError::InvalidMetric {
                        metric: metric.clone(),
                        field: field_name.clone(),
                        valid: VALID_METRICS.iter().map(|s| s.to_string()).collect(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Validate feature configurations
    fn validate_features(
        &self,
        features: &FeaturesConfig,
        _stream_fields: &HashSet<&str>,
        _stream_id: &str,
    ) -> Result<()> {
        // Validate lag features
        if let Some(ref lag) = features.lag {
            if lag.enabled {
                if lag.lags_hours.is_empty() {
                    return Err(GoldDdlError::InvalidFeatureConfig {
                        feature_type: "lag".to_string(),
                        message: "lags_hours cannot be empty when enabled".to_string(),
                    });
                }

                for hours in &lag.lags_hours {
                    if *hours < 1 {
                        return Err(GoldDdlError::InvalidFeatureConfig {
                            feature_type: "lag".to_string(),
                            message: format!("lag hours must be >= 1, got {}", hours),
                        });
                    }
                }

                // Note: lag.fields references aggregate output columns, not stream fields
                // so we skip field existence check here
            }
        }

        // Validate rolling features
        if let Some(ref rolling) = features.rolling {
            if rolling.enabled {
                if rolling.windows.is_empty() {
                    return Err(GoldDdlError::InvalidFeatureConfig {
                        feature_type: "rolling".to_string(),
                        message: "windows cannot be empty when enabled".to_string(),
                    });
                }

                for window in &rolling.windows {
                    parse_window(window)?;
                }

                for stat in &rolling.stats {
                    if !VALID_ROLLING_STATS.contains(&stat.as_str()) {
                        return Err(GoldDdlError::InvalidFeatureConfig {
                            feature_type: "rolling".to_string(),
                            message: format!(
                                "Invalid stat '{}'. Valid stats: {:?}",
                                stat, VALID_ROLLING_STATS
                            ),
                        });
                    }
                }
            }
        }

        // Validate trend features
        if let Some(ref trend) = features.trend {
            if trend.enabled {
                if trend.window.is_empty() {
                    return Err(GoldDdlError::InvalidFeatureConfig {
                        feature_type: "trend".to_string(),
                        message: "window cannot be empty when enabled".to_string(),
                    });
                }

                parse_window(&trend.window)?;
            }
        }

        Ok(())
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate Gold ETL configuration
pub fn validate_gold_config(config: &StreamConfig) -> Result<()> {
    ConfigValidator::new().validate(config)
}

/// Parse a granularity string and validate its format
/// Returns (value, unit) if valid
pub fn parse_granularity(granularity: &str) -> Result<(u32, String)> {
    let parts: Vec<&str> = granularity.trim().split_whitespace().collect();

    if parts.len() != 2 {
        return Err(GoldDdlError::InvalidGranularity {
            granularity: granularity.to_string(),
        });
    }

    let value: u32 = parts[0].parse().map_err(|_| GoldDdlError::InvalidGranularity {
        granularity: granularity.to_string(),
    })?;

    let unit = parts[1].to_lowercase();
    match unit.as_str() {
        "hour" | "hours" | "day" | "days" | "minute" | "minutes" | "week" | "weeks" => {
            Ok((value, unit))
        }
        _ => Err(GoldDdlError::InvalidGranularity {
            granularity: granularity.to_string(),
        }),
    }
}

/// Parse a window string to number of hourly rows
/// "4 hours" -> 4
/// "1 day" -> 24
pub fn parse_window(window: &str) -> Result<u32> {
    let parts: Vec<&str> = window.trim().split_whitespace().collect();

    if parts.len() != 2 {
        return Err(GoldDdlError::InvalidWindow {
            window: window.to_string(),
        });
    }

    let value: u32 = parts[0].parse().map_err(|_| GoldDdlError::InvalidWindow {
        window: window.to_string(),
    })?;

    let unit = parts[1].to_lowercase();
    match unit.as_str() {
        "hour" | "hours" => Ok(value),
        "day" | "days" => Ok(value * 24),
        _ => Err(GoldDdlError::InvalidWindow {
            window: window.to_string(),
        }),
    }
}

/// Convert granularity to view name suffix
pub fn granularity_to_suffix(granularity: &str) -> String {
    let (value, unit) = parse_granularity(granularity).unwrap_or((1, "hour".to_string()));

    match unit.as_str() {
        "hour" | "hours" => {
            if value == 1 {
                "hourly".to_string()
            } else {
                format!("{}hourly", value)
            }
        }
        "day" | "days" => {
            if value == 1 {
                "daily".to_string()
            } else {
                format!("{}daily", value)
            }
        }
        "minute" | "minutes" => format!("{}min", value),
        "week" | "weeks" => {
            if value == 1 {
                "weekly".to_string()
            } else {
                format!("{}weekly", value)
            }
        }
        _ => "custom".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AggregatesConfig, FieldConfig, FieldMetricsConfig, GoldEtlConfig, LagConfig, RollingConfig,
    };
    use std::collections::HashMap;

    fn create_test_stream_config() -> StreamConfig {
        StreamConfig {
            stream_id: "air-quality".to_string(),
            fields: vec![
                FieldConfig {
                    name: "pm25".to_string(),
                    field_type: "float".to_string(),
                },
                FieldConfig {
                    name: "co2".to_string(),
                    field_type: "int".to_string(),
                },
            ],
            silver_etl: None,
            gold_etl: Some(GoldEtlConfig {
                enabled: true,
                aggregates: Some(AggregatesConfig {
                    granularities: vec!["1 hour".to_string()],
                    fields: {
                        let mut map = HashMap::new();
                        map.insert(
                            "pm25".to_string(),
                            FieldMetricsConfig {
                                metrics: vec!["mean".to_string(), "std".to_string()],
                            },
                        );
                        map
                    },
                }),
                features: None,
                refresh_policy: None,
            }),
        }
    }

    // =========================================================================
    // Granularity Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_granularity_1_hour() {
        let (value, unit) = parse_granularity("1 hour").unwrap();
        assert_eq!(value, 1);
        assert_eq!(unit, "hour");
    }

    #[test]
    fn test_parse_granularity_4_hours() {
        let (value, unit) = parse_granularity("4 hours").unwrap();
        assert_eq!(value, 4);
        assert_eq!(unit, "hours");
    }

    #[test]
    fn test_parse_granularity_1_day() {
        let (value, unit) = parse_granularity("1 day").unwrap();
        assert_eq!(value, 1);
        assert_eq!(unit, "day");
    }

    #[test]
    fn test_parse_granularity_invalid_format() {
        assert!(parse_granularity("hourly").is_err());
        assert!(parse_granularity("1").is_err());
        assert!(parse_granularity("one hour").is_err());
        assert!(parse_granularity("1 second").is_err());
    }

    // =========================================================================
    // Window Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_window_4_hours() {
        let rows = parse_window("4 hours").unwrap();
        assert_eq!(rows, 4);
    }

    #[test]
    fn test_parse_window_1_day() {
        let rows = parse_window("1 day").unwrap();
        assert_eq!(rows, 24);
    }

    #[test]
    fn test_parse_window_invalid() {
        assert!(parse_window("4hours").is_err());
        assert!(parse_window("4 minutes").is_err());
    }

    // =========================================================================
    // Granularity Suffix Tests
    // =========================================================================

    #[test]
    fn test_granularity_to_suffix() {
        assert_eq!(granularity_to_suffix("1 hour"), "hourly");
        assert_eq!(granularity_to_suffix("4 hours"), "4hourly");
        assert_eq!(granularity_to_suffix("1 day"), "daily");
        assert_eq!(granularity_to_suffix("7 days"), "7daily");
        assert_eq!(granularity_to_suffix("15 minutes"), "15min");
    }

    // =========================================================================
    // Config Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_valid_config() {
        let config = create_test_stream_config();
        let validator = ConfigValidator::new();
        assert!(validator.validate(&config).is_ok());
    }

    #[test]
    fn test_validate_rejects_disabled_gold_etl() {
        let mut config = create_test_stream_config();
        config.gold_etl.as_mut().unwrap().enabled = false;

        let result = validate_gold_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::GoldEtlDisabled { stream_id } => {
                assert_eq!(stream_id, "air-quality");
            }
            _ => panic!("Expected GoldEtlDisabled error"),
        }
    }

    #[test]
    fn test_validate_rejects_missing_gold_etl() {
        let mut config = create_test_stream_config();
        config.gold_etl = None;

        let result = validate_gold_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::MissingRequiredField { field, .. } => {
                assert_eq!(field, "gold_etl");
            }
            _ => panic!("Expected MissingRequiredField error"),
        }
    }

    #[test]
    fn test_validate_rejects_unknown_field() {
        let mut config = create_test_stream_config();
        let gold_etl = config.gold_etl.as_mut().unwrap();
        let agg = gold_etl.aggregates.as_mut().unwrap();
        agg.fields.insert(
            "nonexistent".to_string(),
            FieldMetricsConfig {
                metrics: vec!["mean".to_string()],
            },
        );

        let result = validate_gold_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::FieldNotFound { field, .. } => {
                assert_eq!(field, "nonexistent");
            }
            _ => panic!("Expected FieldNotFound error"),
        }
    }

    #[test]
    fn test_validate_rejects_invalid_metric() {
        let mut config = create_test_stream_config();
        let gold_etl = config.gold_etl.as_mut().unwrap();
        let agg = gold_etl.aggregates.as_mut().unwrap();
        agg.fields.get_mut("pm25").unwrap().metrics.push("average".to_string()); // invalid

        let result = validate_gold_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::InvalidMetric { metric, .. } => {
                assert_eq!(metric, "average");
            }
            _ => panic!("Expected InvalidMetric error"),
        }
    }

    #[test]
    fn test_validate_rejects_invalid_granularity() {
        let mut config = create_test_stream_config();
        let gold_etl = config.gold_etl.as_mut().unwrap();
        let agg = gold_etl.aggregates.as_mut().unwrap();
        agg.granularities.push("hourly".to_string()); // invalid format

        let result = validate_gold_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::InvalidGranularity { granularity } => {
                assert_eq!(granularity, "hourly");
            }
            _ => panic!("Expected InvalidGranularity error"),
        }
    }

    // =========================================================================
    // Feature Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_lag_config_valid() {
        let mut config = create_test_stream_config();
        let gold_etl = config.gold_etl.as_mut().unwrap();
        gold_etl.features = Some(FeaturesConfig {
            lag: Some(LagConfig {
                enabled: true,
                lags_hours: vec![1, 6, 24],
                fields: vec!["pm25_mean".to_string()],
            }),
            rolling: None,
            trend: None,
            transitions: None,
        });

        assert!(validate_gold_config(&config).is_ok());
    }

    #[test]
    fn test_validate_lag_config_empty_hours() {
        let mut config = create_test_stream_config();
        let gold_etl = config.gold_etl.as_mut().unwrap();
        gold_etl.features = Some(FeaturesConfig {
            lag: Some(LagConfig {
                enabled: true,
                lags_hours: vec![], // empty!
                fields: vec!["pm25_mean".to_string()],
            }),
            rolling: None,
            trend: None,
            transitions: None,
        });

        let result = validate_gold_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::InvalidFeatureConfig { feature_type, .. } => {
                assert_eq!(feature_type, "lag");
            }
            _ => panic!("Expected InvalidFeatureConfig error"),
        }
    }

    #[test]
    fn test_validate_rolling_config_invalid_stat() {
        let mut config = create_test_stream_config();
        let gold_etl = config.gold_etl.as_mut().unwrap();
        gold_etl.features = Some(FeaturesConfig {
            lag: None,
            rolling: Some(RollingConfig {
                enabled: true,
                windows: vec!["4 hours".to_string()],
                stats: vec!["average".to_string()], // invalid - should be "mean"
                fields: vec!["pm25_mean".to_string()],
            }),
            trend: None,
            transitions: None,
        });

        let result = validate_gold_config(&config);
        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::InvalidFeatureConfig { feature_type, message } => {
                assert_eq!(feature_type, "rolling");
                assert!(message.contains("average"));
            }
            _ => panic!("Expected InvalidFeatureConfig error"),
        }
    }
}
