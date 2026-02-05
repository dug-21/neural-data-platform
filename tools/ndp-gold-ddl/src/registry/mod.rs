//! Feature type registry
//!
//! Provides an extensible registry of feature computation types.
//! New feature types can be added by implementing the FeatureGenerator trait.

pub mod lag;
pub mod rolling;
pub mod trait_def;
pub mod trend;

pub use lag::LagFeatureGenerator;
pub use rolling::RollingFeatureGenerator;
pub use trait_def::{FeatureConfig, FeatureGenerator, SqlColumn};
pub use trend::TrendFeatureGenerator;

use crate::config::FeaturesConfig;
use crate::error::Result;
use std::collections::HashMap;

/// Feature type registry
pub struct FeatureRegistry {
    generators: HashMap<String, Box<dyn FeatureGenerator>>,
}

impl FeatureRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            generators: HashMap::new(),
        }
    }

    /// Register a feature generator
    pub fn register(&mut self, generator: Box<dyn FeatureGenerator>) {
        let feature_type = generator.feature_type().to_string();
        self.generators.insert(feature_type, generator);
    }

    /// Get a generator by feature type
    pub fn get(&self, feature_type: &str) -> Option<&dyn FeatureGenerator> {
        self.generators.get(feature_type).map(|g| g.as_ref())
    }

    /// List all registered feature types
    pub fn list_types(&self) -> Vec<&str> {
        self.generators.keys().map(|s| s.as_str()).collect()
    }

    /// Generate all feature columns from config
    pub fn generate_all(&self, features: &FeaturesConfig) -> Result<Vec<SqlColumn>> {
        let mut columns = Vec::new();

        // Generate lag features
        if let Some(ref lag) = features.lag {
            if lag.enabled {
                if let Some(generator) = self.get("lag") {
                    let config = FeatureConfig::from_lag(lag);
                    for field in &lag.fields {
                        let field_columns = generator.generate_columns(&config, field)?;
                        columns.extend(field_columns);
                    }
                }
            }
        }

        // Generate rolling features
        if let Some(ref rolling) = features.rolling {
            if rolling.enabled {
                if let Some(generator) = self.get("rolling") {
                    let config = FeatureConfig::from_rolling(rolling);
                    for field in &rolling.fields {
                        let field_columns = generator.generate_columns(&config, field)?;
                        columns.extend(field_columns);
                    }
                }
            }
        }

        // Generate trend features
        if let Some(ref trend) = features.trend {
            if trend.enabled {
                if let Some(generator) = self.get("trend") {
                    let config = FeatureConfig::from_trend(trend);
                    for field in &trend.fields {
                        let field_columns = generator.generate_columns(&config, field)?;
                        columns.extend(field_columns);
                    }
                }
            }
        }

        Ok(columns)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LagConfig, RollingConfig, TrendConfig};

    #[test]
    fn test_default_registry_has_builtin_types() {
        let registry = FeatureRegistry::default();

        assert!(registry.get("lag").is_some());
        assert!(registry.get("rolling").is_some());
        assert!(registry.get("trend").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_registry_list_types() {
        let registry = FeatureRegistry::default();
        let types = registry.list_types();

        assert!(types.contains(&"lag"));
        assert!(types.contains(&"rolling"));
        assert!(types.contains(&"trend"));
    }

    #[test]
    fn test_generate_all_lag_features() {
        let registry = FeatureRegistry::default();
        let features = FeaturesConfig {
            lag: Some(LagConfig {
                enabled: true,
                lags_hours: vec![1, 6, 24],
                fields: vec!["pm25_mean".to_string()],
            }),
            rolling: None,
            trend: None,
            transitions: None,
        };

        let columns = registry.generate_all(&features).unwrap();

        assert_eq!(columns.len(), 3); // 3 lags for 1 field
        assert!(columns.iter().any(|c| c.alias == "pm25_mean_lag_1h"));
        assert!(columns.iter().any(|c| c.alias == "pm25_mean_lag_6h"));
        assert!(columns.iter().any(|c| c.alias == "pm25_mean_lag_24h"));
    }

    #[test]
    fn test_generate_all_rolling_features() {
        let registry = FeatureRegistry::default();
        let features = FeaturesConfig {
            lag: None,
            rolling: Some(RollingConfig {
                enabled: true,
                windows: vec!["4 hours".to_string()],
                stats: vec!["mean".to_string(), "std".to_string()],
                fields: vec!["pm25_mean".to_string()],
            }),
            trend: None,
            transitions: None,
        };

        let columns = registry.generate_all(&features).unwrap();

        assert_eq!(columns.len(), 2); // 2 stats for 1 window, 1 field
        assert!(columns.iter().any(|c| c.alias.contains("rolling_mean")));
        assert!(columns.iter().any(|c| c.alias.contains("rolling_std")));
    }

    #[test]
    fn test_generate_all_combined_features() {
        let registry = FeatureRegistry::default();
        let features = FeaturesConfig {
            lag: Some(LagConfig {
                enabled: true,
                lags_hours: vec![1],
                fields: vec!["pm25_mean".to_string()],
            }),
            rolling: Some(RollingConfig {
                enabled: true,
                windows: vec!["4 hours".to_string()],
                stats: vec!["mean".to_string()],
                fields: vec!["pm25_mean".to_string()],
            }),
            trend: Some(TrendConfig {
                enabled: true,
                window: "4 hours".to_string(),
                fields: vec!["co2_mean".to_string()],
            }),
            transitions: None,
        };

        let columns = registry.generate_all(&features).unwrap();

        // 1 lag + 1 rolling + 1 trend = 3 columns
        assert_eq!(columns.len(), 3);
    }

    #[test]
    fn test_disabled_features_not_generated() {
        let registry = FeatureRegistry::default();
        let features = FeaturesConfig {
            lag: Some(LagConfig {
                enabled: false, // disabled
                lags_hours: vec![1, 6, 24],
                fields: vec!["pm25_mean".to_string()],
            }),
            rolling: None,
            trend: None,
            transitions: None,
        };

        let columns = registry.generate_all(&features).unwrap();

        assert!(columns.is_empty());
    }
}
