//! Configuration loading for the intelligence app
//!
//! Loads domain-specific intelligence configuration from etcd via config-client,
//! and converts domain objectives to the ObjectiveMetric format.

use ndp_intelligence::error::IntelligenceError;
use ndp_intelligence::predictions::{ObjectiveMetric, ThresholdDirection};
use ndp_intelligence::service::AppConfig;
use ndp_lib::gold::embeddings::config::IntelligenceConfig;
use tracing::info;

/// Domain configuration loaded from etcd.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct DomainConfig {
    pub id: String,
    #[serde(default)]
    pub intelligence: Option<IntelligenceConfig>,
    #[serde(default)]
    pub objectives: Vec<DomainObjective>,
}

/// An objective from the domain config.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct DomainObjective {
    pub id: String,
    pub target: ObjectiveTarget,
    #[serde(default)]
    pub description: String,
}

/// Target specification for a domain objective.
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct ObjectiveTarget {
    pub metric: String,
    pub condition: String,
    pub threshold: f64,
    #[serde(default)]
    pub stream: String,
}

/// Load intelligence configuration and objectives from etcd.
///
/// Returns the IntelligenceConfig and a list of ObjectiveMetrics derived
/// from the domain's objectives array.
pub async fn load_intelligence_config(
    app_config: &AppConfig,
) -> Result<(IntelligenceConfig, Vec<ObjectiveMetric>), IntelligenceError> {
    let endpoints: Vec<&str> = app_config.etcd_endpoints.iter().map(|s| s.as_str()).collect();
    let config_client = config_client::ConfigClient::new(&endpoints)
        .await
        .map_err(|e| IntelligenceError::Config {
            message: format!("Failed to connect to etcd: {}", e),
        })?;

    let domain_config: DomainConfig = config_client
        .get(&format!("/domains/{}/config", app_config.domain_id))
        .await
        .map_err(|e| IntelligenceError::Config {
            message: format!(
                "Failed to load domain config for '{}' from etcd: {}",
                app_config.domain_id, e
            ),
        })?;

    let intel_config = domain_config
        .intelligence
        .ok_or_else(|| IntelligenceError::Config {
            message: format!(
                "No intelligence block in domain config for '{}'",
                app_config.domain_id
            ),
        })?;

    // Convert domain objectives to ObjectiveMetrics
    let objectives = convert_objectives(&domain_config.objectives);
    info!(
        "Loaded intelligence config from etcd: domain={}, {} objectives",
        app_config.domain_id,
        objectives.len()
    );

    Ok((intel_config, objectives))
}

/// Convert domain objectives to ObjectiveMetric format for PredictionEngine.
fn convert_objectives(objectives: &[DomainObjective]) -> Vec<ObjectiveMetric> {
    objectives
        .iter()
        .filter_map(|obj| {
            // Map the Gold aligned view field name: metric from stream becomes {metric}_mean
            let field = format!("{}_mean", obj.target.metric);
            let direction = match obj.target.condition.as_str() {
                ">" | ">=" => ThresholdDirection::Above,
                "<" | "<=" => ThresholdDirection::Below,
                _ => return None,
            };
            Some(ObjectiveMetric {
                field,
                threshold: obj.target.threshold,
                direction,
                label: obj.description.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_objectives_above() {
        let objectives = vec![DomainObjective {
            id: "healthy_co2".to_string(),
            description: "Keep CO2 below 800 ppm".to_string(),
            target: ObjectiveTarget {
                metric: "co2".to_string(),
                condition: "<".to_string(),
                threshold: 800.0,
                stream: "air-quality".to_string(),
            },
        }];
        let result = convert_objectives(&objectives);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].field, "co2_mean");
        assert_eq!(result[0].threshold, 800.0);
        assert_eq!(result[0].direction, ThresholdDirection::Below);
    }

    #[test]
    fn test_convert_objectives_below() {
        let objectives = vec![DomainObjective {
            id: "temp_max".to_string(),
            description: "Keep temp under 24C".to_string(),
            target: ObjectiveTarget {
                metric: "temperature_c".to_string(),
                condition: "<=".to_string(),
                threshold: 24.0,
                stream: "air-quality".to_string(),
            },
        }];
        let result = convert_objectives(&objectives);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].field, "temperature_c_mean");
        assert_eq!(result[0].direction, ThresholdDirection::Below);
    }

    #[test]
    fn test_convert_objectives_skips_unknown_condition() {
        let objectives = vec![DomainObjective {
            id: "unknown".to_string(),
            description: "Unknown condition".to_string(),
            target: ObjectiveTarget {
                metric: "x".to_string(),
                condition: "==".to_string(), // not supported
                threshold: 1.0,
                stream: "test".to_string(),
            },
        }];
        let result = convert_objectives(&objectives);
        assert!(result.is_empty());
    }

    #[test]
    fn test_domain_config_deserialization() {
        let json = r#"{
            "id": "indoor-air-quality",
            "objectives": [
                {
                    "id": "healthy_co2",
                    "description": "Keep CO2 below 800 ppm",
                    "target": {
                        "stream": "air-quality",
                        "metric": "co2",
                        "condition": "<",
                        "threshold": 800
                    }
                }
            ]
        }"#;
        let config: DomainConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.id, "indoor-air-quality");
        assert!(config.intelligence.is_none());
        assert_eq!(config.objectives.len(), 1);
    }
}
