//! Trend feature generator
//!
//! Generates trend (slope) features using simple approximation:
//! (last_value - first_value) / window

use crate::gold::error::{GoldDdlError, Result};
use crate::gold::registry::{FeatureConfig, FeatureGenerator, SqlColumn};
use crate::gold::validation::parse_window;

/// Trend feature generator
pub struct TrendFeatureGenerator;

impl TrendFeatureGenerator {
    /// Create a new trend feature generator
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrendFeatureGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureGenerator for TrendFeatureGenerator {
    fn feature_type(&self) -> &str {
        "trend"
    }

    fn generate_columns(&self, config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>> {
        let window =
            config
                .trend_window
                .as_ref()
                .ok_or_else(|| GoldDdlError::InvalidFeatureConfig {
                    feature_type: "trend".to_string(),
                    message: "trend_window is required".to_string(),
                })?;

        if window.is_empty() {
            return Err(GoldDdlError::InvalidFeatureConfig {
                feature_type: "trend".to_string(),
                message: "window cannot be empty".to_string(),
            });
        }

        let window_rows = parse_window(window)?;
        let window_suffix = window.replace(' ', "_");

        // Simple slope: (last - first) / window_size
        // Using FIRST_VALUE and LAST_VALUE with a window frame
        let expression = format!(
            r#"(
        LAST_VALUE({field}) OVER w - FIRST_VALUE({field}) OVER w
    ) / {window}.0"#,
            field = field,
            window = window_rows
        );

        let alias = format!("{}_trend_{}", field, window_suffix);

        Ok(vec![SqlColumn::new(
            expression,
            alias,
            "DOUBLE PRECISION".to_string(),
        )])
    }

    fn validate(&self, config: &FeatureConfig) -> Result<()> {
        let window =
            config
                .trend_window
                .as_ref()
                .ok_or_else(|| GoldDdlError::InvalidFeatureConfig {
                    feature_type: "trend".to_string(),
                    message: "trend_window is required".to_string(),
                })?;

        if window.is_empty() {
            return Err(GoldDdlError::InvalidFeatureConfig {
                feature_type: "trend".to_string(),
                message: "window cannot be empty".to_string(),
            });
        }

        parse_window(window)?;
        Ok(())
    }

    fn requires_window(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Trend (slope) feature over window"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_generator_feature_type() {
        let generator = TrendFeatureGenerator::new();
        assert_eq!(generator.feature_type(), "trend");
    }

    #[test]
    fn test_trend_generator_requires_window() {
        let generator = TrendFeatureGenerator::new();
        assert!(generator.requires_window());
    }

    #[test]
    fn test_trend_generates_column() {
        let generator = TrendFeatureGenerator::new();
        let config = FeatureConfig {
            trend_window: Some("4 hours".to_string()),
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "co2_mean").unwrap();

        assert_eq!(columns.len(), 1);
        assert!(columns[0].expression.contains("LAST_VALUE(co2_mean)"));
        assert!(columns[0].expression.contains("FIRST_VALUE(co2_mean)"));
        assert!(columns[0].expression.contains("/ 4.0"));
        assert_eq!(columns[0].alias, "co2_mean_trend_4_hours");
    }

    #[test]
    fn test_trend_1_day_window() {
        let generator = TrendFeatureGenerator::new();
        let config = FeatureConfig {
            trend_window: Some("1 day".to_string()),
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "co2_mean").unwrap();

        // 1 day = 24 hours
        assert!(columns[0].expression.contains("/ 24.0"));
        assert!(columns[0].alias.contains("1_day"));
    }

    #[test]
    fn test_trend_validation_missing_window() {
        let generator = TrendFeatureGenerator::new();
        let config = FeatureConfig {
            trend_window: None,
            ..Default::default()
        };

        let result = generator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_trend_validation_empty_window() {
        let generator = TrendFeatureGenerator::new();
        let config = FeatureConfig {
            trend_window: Some(String::new()),
            ..Default::default()
        };

        let result = generator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_trend_validation_invalid_window_format() {
        let generator = TrendFeatureGenerator::new();
        let config = FeatureConfig {
            trend_window: Some("4hours".to_string()), // missing space
            ..Default::default()
        };

        let result = generator.validate(&config);
        assert!(result.is_err());
    }
}
