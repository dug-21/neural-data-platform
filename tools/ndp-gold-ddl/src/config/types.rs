//! Configuration types for Gold ETL
//!
//! Defines the structure for gold_etl configuration sections.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Valid aggregate metrics
pub const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];

/// Valid rolling statistics
pub const VALID_ROLLING_STATS: &[&str] = &["mean", "std", "min", "max"];

/// Gold ETL configuration section
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoldEtlConfig {
    /// Whether Gold ETL is enabled for this stream
    #[serde(default)]
    pub enabled: bool,

    /// Aggregate configuration
    #[serde(default)]
    pub aggregates: Option<AggregatesConfig>,

    /// Feature configuration
    #[serde(default)]
    pub features: Option<FeaturesConfig>,

    /// Refresh policy configuration
    #[serde(default)]
    pub refresh_policy: Option<RefreshPolicyConfig>,
}

/// Aggregates configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AggregatesConfig {
    /// Time bucket granularities (e.g., ["1 hour", "1 day"])
    #[serde(default)]
    pub granularities: Vec<String>,

    /// Fields to aggregate with their metrics
    #[serde(default)]
    pub fields: HashMap<String, FieldMetricsConfig>,
}

/// Field metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldMetricsConfig {
    /// List of metrics to compute (mean, std, min, max, count, p95, p99, first, last)
    #[serde(default)]
    pub metrics: Vec<String>,
}

/// Features configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeaturesConfig {
    /// Lag features configuration
    #[serde(default)]
    pub lag: Option<LagConfig>,

    /// Rolling window features configuration
    #[serde(default)]
    pub rolling: Option<RollingConfig>,

    /// Trend features configuration
    #[serde(default)]
    pub trend: Option<TrendConfig>,

    /// State transitions configuration
    #[serde(default)]
    pub transitions: Option<TransitionsConfig>,
}

/// Lag feature configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LagConfig {
    /// Whether lag features are enabled
    #[serde(default)]
    pub enabled: bool,

    /// Lag periods in hours
    #[serde(default)]
    pub lags_hours: Vec<u32>,

    /// Fields to compute lag features for
    #[serde(default)]
    pub fields: Vec<String>,
}

/// Rolling window feature configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RollingConfig {
    /// Whether rolling features are enabled
    #[serde(default)]
    pub enabled: bool,

    /// Window sizes (e.g., ["4 hours", "24 hours"])
    #[serde(default)]
    pub windows: Vec<String>,

    /// Statistics to compute (mean, std, min, max)
    #[serde(default)]
    pub stats: Vec<String>,

    /// Fields to compute rolling features for
    #[serde(default)]
    pub fields: Vec<String>,
}

/// Trend feature configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrendConfig {
    /// Whether trend features are enabled
    #[serde(default)]
    pub enabled: bool,

    /// Window size for trend calculation
    #[serde(default)]
    pub window: String,

    /// Fields to compute trend features for
    #[serde(default)]
    pub fields: Vec<String>,
}

/// State transitions configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransitionsConfig {
    /// Whether transitions are enabled
    #[serde(default)]
    pub enabled: bool,

    /// Field to track transitions for
    #[serde(default)]
    pub field: String,

    /// States to track
    #[serde(default)]
    pub states: Vec<String>,
}

/// Refresh policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshPolicyConfig {
    /// Start offset for refresh (how far back to look)
    #[serde(default = "default_start_offset")]
    pub start_offset: String,

    /// End offset for refresh (how close to now)
    #[serde(default = "default_end_offset")]
    pub end_offset: String,

    /// Schedule interval for refresh
    #[serde(default = "default_schedule_interval")]
    pub schedule_interval: String,
}

fn default_start_offset() -> String {
    "4 hours".to_string()
}

fn default_end_offset() -> String {
    "15 minutes".to_string()
}

fn default_schedule_interval() -> String {
    "15 minutes".to_string()
}

impl Default for RefreshPolicyConfig {
    fn default() -> Self {
        Self {
            start_offset: default_start_offset(),
            end_offset: default_end_offset(),
            schedule_interval: default_schedule_interval(),
        }
    }
}

/// Simplified stream config for Gold DDL generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Stream identifier
    pub stream_id: String,

    /// Field definitions
    #[serde(default)]
    pub fields: Vec<FieldConfig>,

    /// Silver ETL configuration (to get source table)
    #[serde(default)]
    pub silver_etl: Option<SilverEtlConfig>,

    /// Gold ETL configuration
    #[serde(default)]
    pub gold_etl: Option<GoldEtlConfig>,
}

/// Simplified field configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfig {
    /// Field name
    pub name: String,

    /// Field type
    #[serde(rename = "type", default)]
    pub field_type: String,
}

/// Simplified Silver ETL configuration (for source table reference)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SilverEtlConfig {
    /// Target table in Silver layer
    #[serde(default)]
    pub target_table: String,

    /// Timestamp field configuration
    #[serde(default)]
    pub timestamp: Option<TimestampConfig>,
}

/// Timestamp field configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimestampConfig {
    /// Target field name for timestamp
    #[serde(default = "default_timestamp_field")]
    pub target_field: String,
}

fn default_timestamp_field() -> String {
    "observation_time".to_string()
}

/// Action for DDL generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    /// Create if not exists (idempotent)
    #[default]
    Sync,
    /// Drop and recreate
    Recreate,
}

impl std::str::FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sync" => Ok(Action::Sync),
            "recreate" => Ok(Action::Recreate),
            _ => Err(format!("Invalid action '{}'. Expected 'sync' or 'recreate'", s)),
        }
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Sync => write!(f, "sync"),
            Action::Recreate => write!(f, "recreate"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_gold_etl_config_default() {
        let config = GoldEtlConfig::default();
        assert!(!config.enabled);
        assert!(config.aggregates.is_none());
        assert!(config.features.is_none());
    }

    #[test]
    fn test_action_from_str() {
        assert_eq!(Action::from_str("sync").unwrap(), Action::Sync);
        assert_eq!(Action::from_str("recreate").unwrap(), Action::Recreate);
        assert_eq!(Action::from_str("SYNC").unwrap(), Action::Sync);
        assert!(Action::from_str("invalid").is_err());
    }

    #[test]
    fn test_refresh_policy_defaults() {
        let policy = RefreshPolicyConfig::default();
        assert_eq!(policy.start_offset, "4 hours");
        assert_eq!(policy.end_offset, "15 minutes");
        assert_eq!(policy.schedule_interval, "15 minutes");
    }

    #[test]
    fn test_gold_etl_config_deserialize() {
        let json = r#"{
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "pm25": { "metrics": ["mean", "std", "max"] }
                }
            }
        }"#;

        let config: GoldEtlConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        let agg = config.aggregates.unwrap();
        assert_eq!(agg.granularities, vec!["1 hour"]);
        assert_eq!(
            agg.fields.get("pm25").unwrap().metrics,
            vec!["mean", "std", "max"]
        );
    }

    #[test]
    fn test_features_config_deserialize() {
        let json = r#"{
            "lag": {
                "enabled": true,
                "lags_hours": [1, 6, 24],
                "fields": ["pm25_mean"]
            },
            "rolling": {
                "enabled": true,
                "windows": ["4 hours"],
                "stats": ["mean", "std"],
                "fields": ["pm25_mean"]
            },
            "trend": {
                "enabled": true,
                "window": "4 hours",
                "fields": ["co2_mean"]
            }
        }"#;

        let config: FeaturesConfig = serde_json::from_str(json).unwrap();

        let lag = config.lag.unwrap();
        assert!(lag.enabled);
        assert_eq!(lag.lags_hours, vec![1, 6, 24]);

        let rolling = config.rolling.unwrap();
        assert!(rolling.enabled);
        assert_eq!(rolling.windows, vec!["4 hours"]);
        assert_eq!(rolling.stats, vec!["mean", "std"]);

        let trend = config.trend.unwrap();
        assert!(trend.enabled);
        assert_eq!(trend.window, "4 hours");
    }
}
