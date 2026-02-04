//! Lag feature generator
//!
//! Generates lag features: values at t-N hours.

use crate::error::{GoldDdlError, Result};
use crate::registry::{FeatureConfig, FeatureGenerator, SqlColumn};

/// Lag feature generator
pub struct LagFeatureGenerator;

impl LagFeatureGenerator {
    /// Create a new lag feature generator
    pub fn new() -> Self {
        Self
    }
}

impl Default for LagFeatureGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureGenerator for LagFeatureGenerator {
    fn feature_type(&self) -> &str {
        "lag"
    }

    fn generate_columns(&self, config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>> {
        if config.lags_hours.is_empty() {
            return Err(GoldDdlError::InvalidFeatureConfig {
                feature_type: "lag".to_string(),
                message: "lags_hours cannot be empty".to_string(),
            });
        }

        let mut columns = Vec::new();

        for hours in &config.lags_hours {
            let expression = format!(
                "LAG({}, {}) OVER (PARTITION BY ndp_id ORDER BY bucket)",
                field, hours
            );
            let alias = format!("{}_lag_{}h", field, hours);

            columns.push(SqlColumn::new(
                expression,
                alias,
                "DOUBLE PRECISION".to_string(),
            ));
        }

        Ok(columns)
    }

    fn validate(&self, config: &FeatureConfig) -> Result<()> {
        if config.lags_hours.is_empty() {
            return Err(GoldDdlError::InvalidFeatureConfig {
                feature_type: "lag".to_string(),
                message: "lags_hours cannot be empty".to_string(),
            });
        }

        for hours in &config.lags_hours {
            if *hours < 1 {
                return Err(GoldDdlError::InvalidFeatureConfig {
                    feature_type: "lag".to_string(),
                    message: format!("lag hours must be >= 1, got {}", hours),
                });
            }
        }

        Ok(())
    }

    fn requires_window(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Lag features: values at t-N hours"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lag_generator_feature_type() {
        let generator = LagFeatureGenerator::new();
        assert_eq!(generator.feature_type(), "lag");
    }

    #[test]
    fn test_lag_generator_description() {
        let generator = LagFeatureGenerator::new();
        assert!(generator.description().contains("Lag"));
    }

    #[test]
    fn test_lag_generator_requires_window() {
        let generator = LagFeatureGenerator::new();
        assert!(generator.requires_window());
    }

    #[test]
    fn test_lag_generates_single_column() {
        let generator = LagFeatureGenerator::new();
        let config = FeatureConfig {
            lags_hours: vec![1],
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

        assert_eq!(columns.len(), 1);
        assert!(columns[0].expression.contains("LAG(pm25_mean, 1)"));
        assert!(columns[0].expression.contains("PARTITION BY ndp_id"));
        assert!(columns[0].expression.contains("ORDER BY bucket"));
        assert_eq!(columns[0].alias, "pm25_mean_lag_1h");
    }

    #[test]
    fn test_lag_generates_multiple_columns() {
        let generator = LagFeatureGenerator::new();
        let config = FeatureConfig {
            lags_hours: vec![1, 6, 24],
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].alias, "pm25_mean_lag_1h");
        assert_eq!(columns[1].alias, "pm25_mean_lag_6h");
        assert_eq!(columns[2].alias, "pm25_mean_lag_24h");
    }

    #[test]
    fn test_lag_validation_empty_hours() {
        let generator = LagFeatureGenerator::new();
        let config = FeatureConfig {
            lags_hours: vec![],
            ..Default::default()
        };

        let result = generator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_lag_validation_zero_hours() {
        let generator = LagFeatureGenerator::new();
        let config = FeatureConfig {
            lags_hours: vec![0],
            ..Default::default()
        };

        let result = generator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_lag_validation_valid() {
        let generator = LagFeatureGenerator::new();
        let config = FeatureConfig {
            lags_hours: vec![1, 6, 24],
            ..Default::default()
        };

        assert!(generator.validate(&config).is_ok());
    }
}
