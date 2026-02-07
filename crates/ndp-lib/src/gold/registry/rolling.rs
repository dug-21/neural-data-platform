//! Rolling window feature generator
//!
//! Generates rolling window statistics: mean, std, min, max over N rows.

use crate::gold::error::{GoldDdlError, Result};
use crate::gold::registry::{FeatureConfig, FeatureGenerator, SqlColumn};
use crate::gold::validation::parse_window;

/// Rolling window feature generator
pub struct RollingFeatureGenerator;

impl RollingFeatureGenerator {
    /// Create a new rolling feature generator
    pub fn new() -> Self {
        Self
    }
}

impl Default for RollingFeatureGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureGenerator for RollingFeatureGenerator {
    fn feature_type(&self) -> &str {
        "rolling"
    }

    fn generate_columns(&self, config: &FeatureConfig, field: &str) -> Result<Vec<SqlColumn>> {
        if config.windows.is_empty() {
            return Err(GoldDdlError::InvalidFeatureConfig {
                feature_type: "rolling".to_string(),
                message: "windows cannot be empty".to_string(),
            });
        }

        if config.stats.is_empty() {
            return Err(GoldDdlError::InvalidFeatureConfig {
                feature_type: "rolling".to_string(),
                message: "stats cannot be empty".to_string(),
            });
        }

        let mut columns = Vec::new();

        for window in &config.windows {
            let window_rows = parse_window(window)?;
            // window_rows is the total window size, so we need rows-1 for PRECEDING
            let preceding = if window_rows > 0 { window_rows - 1 } else { 0 };

            let window_suffix = window.replace(' ', "_");

            for stat in &config.stats {
                let (sql_func, alias_suffix) = match stat.as_str() {
                    "mean" => ("AVG", "mean"),
                    "std" => ("STDDEV", "std"),
                    "min" => ("MIN", "min"),
                    "max" => ("MAX", "max"),
                    _ => {
                        return Err(GoldDdlError::InvalidFeatureConfig {
                            feature_type: "rolling".to_string(),
                            message: format!("Invalid stat '{}'. Valid: mean, std, min, max", stat),
                        })
                    }
                };

                let expression = format!(
                    "{}({}) OVER (PARTITION BY ndp_id ORDER BY bucket ROWS BETWEEN {} PRECEDING AND CURRENT ROW)",
                    sql_func, field, preceding
                );
                let alias = format!("{}_rolling_{}_{}", field, alias_suffix, window_suffix);

                columns.push(SqlColumn::new(
                    expression,
                    alias,
                    "DOUBLE PRECISION".to_string(),
                ));
            }
        }

        Ok(columns)
    }

    fn validate(&self, config: &FeatureConfig) -> Result<()> {
        if config.windows.is_empty() {
            return Err(GoldDdlError::InvalidFeatureConfig {
                feature_type: "rolling".to_string(),
                message: "windows cannot be empty".to_string(),
            });
        }

        for window in &config.windows {
            parse_window(window)?;
        }

        for stat in &config.stats {
            if !["mean", "std", "min", "max"].contains(&stat.as_str()) {
                return Err(GoldDdlError::InvalidFeatureConfig {
                    feature_type: "rolling".to_string(),
                    message: format!("Invalid stat '{}'. Valid: mean, std, min, max", stat),
                });
            }
        }

        Ok(())
    }

    fn requires_window(&self) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Rolling window statistics: mean, std, min, max over N hours"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_generator_feature_type() {
        let generator = RollingFeatureGenerator::new();
        assert_eq!(generator.feature_type(), "rolling");
    }

    #[test]
    fn test_rolling_generator_requires_window() {
        let generator = RollingFeatureGenerator::new();
        assert!(generator.requires_window());
    }

    #[test]
    fn test_rolling_generates_mean_column() {
        let generator = RollingFeatureGenerator::new();
        let config = FeatureConfig {
            windows: vec!["4 hours".to_string()],
            stats: vec!["mean".to_string()],
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

        assert_eq!(columns.len(), 1);
        assert!(columns[0].expression.contains("AVG(pm25_mean)"));
        assert!(columns[0].expression.contains("ROWS BETWEEN 3 PRECEDING"));
        assert!(columns[0].expression.contains("PARTITION BY ndp_id"));
        assert_eq!(columns[0].alias, "pm25_mean_rolling_mean_4_hours");
    }

    #[test]
    fn test_rolling_generates_multiple_stats() {
        let generator = RollingFeatureGenerator::new();
        let config = FeatureConfig {
            windows: vec!["4 hours".to_string()],
            stats: vec!["mean".to_string(), "std".to_string()],
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

        assert_eq!(columns.len(), 2);
        assert!(columns[0].expression.contains("AVG"));
        assert!(columns[1].expression.contains("STDDEV"));
    }

    #[test]
    fn test_rolling_generates_multiple_windows() {
        let generator = RollingFeatureGenerator::new();
        let config = FeatureConfig {
            windows: vec!["4 hours".to_string(), "24 hours".to_string()],
            stats: vec!["mean".to_string()],
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

        assert_eq!(columns.len(), 2);
        assert!(columns[0].alias.contains("4_hours"));
        assert!(columns[1].alias.contains("24_hours"));
    }

    #[test]
    fn test_rolling_generates_min_max() {
        let generator = RollingFeatureGenerator::new();
        let config = FeatureConfig {
            windows: vec!["4 hours".to_string()],
            stats: vec!["min".to_string(), "max".to_string()],
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

        assert_eq!(columns.len(), 2);
        assert!(columns[0].expression.contains("MIN"));
        assert!(columns[1].expression.contains("MAX"));
    }

    #[test]
    fn test_rolling_validation_empty_windows() {
        let generator = RollingFeatureGenerator::new();
        let config = FeatureConfig {
            windows: vec![],
            stats: vec!["mean".to_string()],
            ..Default::default()
        };

        let result = generator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_rolling_validation_invalid_stat() {
        let generator = RollingFeatureGenerator::new();
        let config = FeatureConfig {
            windows: vec!["4 hours".to_string()],
            stats: vec!["average".to_string()], // invalid
            ..Default::default()
        };

        let result = generator.validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_rolling_1_day_window() {
        let generator = RollingFeatureGenerator::new();
        let config = FeatureConfig {
            windows: vec!["1 day".to_string()],
            stats: vec!["mean".to_string()],
            ..Default::default()
        };

        let columns = generator.generate_columns(&config, "pm25_mean").unwrap();

        // 1 day = 24 hours, so 23 PRECEDING
        assert!(columns[0].expression.contains("ROWS BETWEEN 23 PRECEDING"));
    }
}
