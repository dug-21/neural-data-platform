//! Intelligence configuration types
//!
//! Configuration structures for the intelligence layer, used by both
//! ndp-lib (for schema generation) and ndp-intelligence (for runtime).

use serde::{Deserialize, Serialize};

/// Top-level intelligence configuration for a domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceConfig {
    /// Whether intelligence features are enabled
    pub enabled: bool,
    /// Embedding configuration
    pub embedding: EmbeddingConfig,
    /// Search configuration
    pub search: SearchConfig,
    /// Anomaly detection configuration (optional)
    #[serde(default)]
    pub anomaly: Option<AnomalyConfig>,
}

/// Embedding generation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Type of embedding (currently only "metric")
    #[serde(rename = "type")]
    pub embedding_type: EmbeddingType,
    /// Field configuration for embedding generation
    pub fields: EmbeddingFieldsConfig,
}

/// Type of embedding to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingType {
    /// Metric-based embedding using z-score normalized Gold row values
    Metric,
}

/// Configuration for which fields to include in embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingFieldsConfig {
    /// Temporal encoding fields (e.g., "hour_sin", "hour_cos", "is_weekend")
    #[serde(default)]
    pub temporal: Vec<String>,
    /// Direct metric fields with null strategies
    #[serde(default)]
    pub direct: Vec<DirectFieldConfig>,
    /// Derived/computed fields
    #[serde(default)]
    pub derived: Vec<String>,
}

impl EmbeddingFieldsConfig {
    /// Count the total number of embedding dimensions.
    pub fn total_dimensions(&self) -> usize {
        self.temporal.len() + self.direct.len() + self.derived.len()
    }
}

/// Configuration for a direct metric field in an embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectFieldConfig {
    /// Field name in the Gold row
    pub field: String,
    /// Strategy for handling null values
    pub null_strategy: NullStrategyConfig,
}

/// Null handling strategy configuration (matches runtime NullStrategy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullStrategyConfig {
    /// Replace null with 0.0
    Zero,
    /// Replace null with last known value
    LastKnown,
    /// Replace null with running mean
    Mean,
}

/// Similarity search configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Number of nearest neighbors to retrieve
    pub k: usize,
    /// Minimum similarity threshold (0.0 to 1.0)
    pub min_similarity: f64,
    /// Prediction horizons (e.g., ["1 hour", "6 hours", "24 hours"])
    pub prediction_horizons: Vec<String>,
}

/// Anomaly detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyConfig {
    /// Whether anomaly detection is enabled
    pub enabled: bool,
    /// Distance threshold in standard deviations for anomaly flagging
    pub distance_threshold_sigma: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config_json() -> &'static str {
        r#"{
            "enabled": true,
            "embedding": {
                "type": "metric",
                "fields": {
                    "temporal": ["hour_sin", "hour_cos", "is_weekend"],
                    "direct": [
                        {"field": "pm25_mean", "null_strategy": "zero"},
                        {"field": "co2_mean", "null_strategy": "last_known"},
                        {"field": "temperature_c_mean", "null_strategy": "mean"}
                    ],
                    "derived": ["pm25_co2_ratio"]
                }
            },
            "search": {
                "k": 10,
                "min_similarity": 0.85,
                "prediction_horizons": ["1 hour", "6 hours", "24 hours"]
            },
            "anomaly": {
                "enabled": true,
                "distance_threshold_sigma": 3.0
            }
        }"#
    }

    #[test]
    fn test_full_json_deserialization() {
        let config: IntelligenceConfig =
            serde_json::from_str(sample_config_json()).unwrap();
        assert!(config.enabled);
        assert_eq!(config.embedding.embedding_type, EmbeddingType::Metric);
        assert_eq!(config.embedding.fields.temporal.len(), 3);
        assert_eq!(config.embedding.fields.direct.len(), 3);
        assert_eq!(config.embedding.fields.derived.len(), 1);
        assert_eq!(config.search.k, 10);
        assert!((config.search.min_similarity - 0.85).abs() < f64::EPSILON);
        assert_eq!(config.search.prediction_horizons.len(), 3);
        assert!(config.anomaly.is_some());
        let anomaly = config.anomaly.unwrap();
        assert!(anomaly.enabled);
        assert!((anomaly.distance_threshold_sigma - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_omitted_anomaly_defaults_to_none() {
        let json = r#"{
            "enabled": true,
            "embedding": {
                "type": "metric",
                "fields": {
                    "temporal": [],
                    "direct": [],
                    "derived": []
                }
            },
            "search": {
                "k": 5,
                "min_similarity": 0.7,
                "prediction_horizons": ["1 hour"]
            }
        }"#;
        let config: IntelligenceConfig = serde_json::from_str(json).unwrap();
        assert!(config.anomaly.is_none());
    }

    #[test]
    fn test_round_trip_serialize_deserialize() {
        let config: IntelligenceConfig =
            serde_json::from_str(sample_config_json()).unwrap();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: IntelligenceConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(
            deserialized.embedding.embedding_type,
            config.embedding.embedding_type
        );
        assert_eq!(
            deserialized.embedding.fields.temporal.len(),
            config.embedding.fields.temporal.len()
        );
        assert_eq!(deserialized.search.k, config.search.k);
        assert_eq!(
            deserialized.anomaly.is_some(),
            config.anomaly.is_some()
        );
    }

    #[test]
    fn test_null_strategy_config_serialization() {
        // Test each variant serializes to snake_case
        let zero = serde_json::to_string(&NullStrategyConfig::Zero).unwrap();
        assert_eq!(zero, "\"zero\"");

        let last_known = serde_json::to_string(&NullStrategyConfig::LastKnown).unwrap();
        assert_eq!(last_known, "\"last_known\"");

        let mean = serde_json::to_string(&NullStrategyConfig::Mean).unwrap();
        assert_eq!(mean, "\"mean\"");
    }

    #[test]
    fn test_null_strategy_config_deserialization() {
        let zero: NullStrategyConfig = serde_json::from_str("\"zero\"").unwrap();
        assert_eq!(zero, NullStrategyConfig::Zero);

        let last_known: NullStrategyConfig = serde_json::from_str("\"last_known\"").unwrap();
        assert_eq!(last_known, NullStrategyConfig::LastKnown);

        let mean: NullStrategyConfig = serde_json::from_str("\"mean\"").unwrap();
        assert_eq!(mean, NullStrategyConfig::Mean);
    }

    #[test]
    fn test_embedding_type_serialization() {
        let metric = serde_json::to_string(&EmbeddingType::Metric).unwrap();
        assert_eq!(metric, "\"metric\"");
    }

    #[test]
    fn test_total_dimensions() {
        let fields = EmbeddingFieldsConfig {
            temporal: vec!["hour_sin".to_string(), "hour_cos".to_string(), "is_weekend".to_string()],
            direct: vec![
                DirectFieldConfig {
                    field: "pm25".to_string(),
                    null_strategy: NullStrategyConfig::Zero,
                },
                DirectFieldConfig {
                    field: "co2".to_string(),
                    null_strategy: NullStrategyConfig::Mean,
                },
            ],
            derived: vec!["ratio".to_string()],
        };
        assert_eq!(fields.total_dimensions(), 6);
    }

    #[test]
    fn test_empty_fields_config() {
        let json = r#"{"temporal": [], "direct": [], "derived": []}"#;
        let fields: EmbeddingFieldsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(fields.total_dimensions(), 0);
    }

    #[test]
    fn test_fields_config_default_empty() {
        // With serde(default), missing fields default to empty vecs
        let json = r#"{}"#;
        let fields: EmbeddingFieldsConfig = serde_json::from_str(json).unwrap();
        assert!(fields.temporal.is_empty());
        assert!(fields.direct.is_empty());
        assert!(fields.derived.is_empty());
    }
}
