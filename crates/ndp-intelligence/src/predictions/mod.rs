//! Prediction engine and outcome tracking
//!
//! Provides the `PredictionEngine` for generating breach predictions based on
//! K-NN neighbor outcome lookup, and `OutcomeTracker` for evaluating prediction accuracy.

pub mod outcome;

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::{IntelligenceError, Result};
use crate::similarity::SearchResult;
use crate::similarity::pgvector::parse_bucket_from_id;
use crate::storage::Prediction;
use ndp_lib::gold::embeddings::config::IntelligenceConfig;

/// Direction of threshold comparison for objective metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThresholdDirection {
    /// Breach occurs when value is above threshold
    Above,
    /// Breach occurs when value is below threshold
    Below,
}

/// An objective metric to predict breaches for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveMetric {
    /// Field name in the Gold aligned view
    pub field: String,
    /// Threshold value for breach detection
    pub threshold: f64,
    /// Direction of comparison
    pub direction: ThresholdDirection,
    /// Human-readable label
    pub label: String,
}

/// Summary of prediction evaluations.
#[derive(Debug, Clone, Default)]
pub struct EvaluationSummary {
    /// Number of predictions evaluated
    pub evaluated: usize,
    /// Number of correct predictions
    pub correct: usize,
    /// Number of incorrect predictions
    pub incorrect: usize,
}

impl std::fmt::Display for EvaluationSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EvaluationSummary {{ evaluated: {}, correct: {}, incorrect: {} }}",
            self.evaluated, self.correct, self.incorrect
        )
    }
}

/// Engine for generating breach predictions based on K-NN neighbor outcomes.
///
/// For each prediction horizon and objective metric, queries what happened
/// at neighbor_bucket + horizon to determine if a breach occurred. Confidence
/// is computed as k_supporting / k_total (minimum 3 neighbors required).
pub struct PredictionEngine {
    db_pool: Arc<Pool>,
    horizons: Vec<Duration>,
    min_confidence: f64,
    objective_metrics: Vec<ObjectiveMetric>,
    /// Primary stream alias used to prefix column names in SQL queries.
    /// The Gold aligned view uses `{alias}_{column}` naming.
    column_prefix: String,
}

impl PredictionEngine {
    /// Create a new PredictionEngine from configuration.
    pub fn new(
        db_pool: Arc<Pool>,
        config: &IntelligenceConfig,
        objectives: &[ObjectiveMetric],
        column_prefix: String,
    ) -> Self {
        let horizons = config
            .search
            .prediction_horizons
            .iter()
            .map(|h| parse_horizon(h))
            .collect();
        Self {
            db_pool,
            horizons,
            min_confidence: 0.5,
            objective_metrics: objectives.to_vec(),
            column_prefix,
        }
    }

    /// Map a logical field name to the view column name by prepending the
    /// primary stream alias. E.g., "co2_mean" → "indoor_co2_mean".
    fn view_column_name(&self, field: &str) -> String {
        if self.column_prefix.is_empty() {
            field.to_string()
        } else {
            format!("{}_{}", self.column_prefix, field)
        }
    }

    /// Generate predictions for all objectives and horizons based on K-NN neighbors.
    pub async fn generate_predictions(
        &self,
        current_bucket: DateTime<Utc>,
        domain_id: &str,
        neighbors: &[SearchResult],
    ) -> Result<Vec<Prediction>> {
        if self.objective_metrics.is_empty() {
            warn!("No objective metrics configured; skipping predictions");
            return Ok(vec![]);
        }

        let mut predictions = Vec::new();
        let client = self
            .db_pool
            .get()
            .await
            .map_err(|e| IntelligenceError::Database(format!("Pool error: {}", e)))?;
        let view_name = format!(
            "gold.{}_aligned",
            domain_id.replace('-', "_")
        );

        for horizon in &self.horizons {
            for objective in &self.objective_metrics {
                let mut supporting = 0usize;
                let mut total_with_outcome = 0usize;

                for neighbor in neighbors {
                    let neighbor_bucket = match parse_bucket_from_id(&neighbor.id) {
                        Ok(b) => b,
                        Err(e) => {
                            debug!("Skipping neighbor with invalid ID '{}': {}", neighbor.id, e);
                            continue;
                        }
                    };
                    let future_bucket = neighbor_bucket + *horizon;

                    let view_col = self.view_column_name(&objective.field);
                    let sql = format!(
                        "SELECT {}::double precision FROM {} WHERE bucket = $1 LIMIT 1",
                        sanitize_field_name(&view_col),
                        view_name
                    );
                    let row = client
                        .query_opt(&sql, &[&future_bucket])
                        .await
                        .map_err(|e| IntelligenceError::Database(format!("Query error: {}", e)))?;

                    if let Some(row) = row {
                        let value: Option<f64> = row.get(0);
                        if let Some(v) = value {
                            total_with_outcome += 1;
                            let breached = match objective.direction {
                                ThresholdDirection::Above => v > objective.threshold,
                                ThresholdDirection::Below => v < objective.threshold,
                            };
                            if breached {
                                supporting += 1;
                            }
                        }
                    }
                }

                if total_with_outcome >= 3 {
                    let confidence = supporting as f64 / total_with_outcome as f64;
                    if confidence >= self.min_confidence {
                        predictions.push(Prediction {
                            id: None,
                            bucket: current_bucket,
                            domain_id: domain_id.to_string(),
                            metric: objective.field.clone(),
                            horizon: format_duration(horizon),
                            predicted_value: None,
                            predicted_breach: Some(confidence > 0.5),
                            confidence,
                            k_neighbors: total_with_outcome as i32,
                            k_supporting: supporting as i32,
                            actual_value: None,
                            actual_breach: None,
                            correct: None,
                            evaluated_at: None,
                        });
                    }
                }
            }
        }

        info!(
            "Generated {} predictions for {} objectives across {} horizons",
            predictions.len(),
            self.objective_metrics.len(),
            self.horizons.len()
        );
        Ok(predictions)
    }

    /// Get the configured objective metrics.
    pub fn objective_metrics(&self) -> &[ObjectiveMetric] {
        &self.objective_metrics
    }
}

/// Parse a horizon string like "1 hour", "6 hours", "24 hours" into a Duration.
pub fn parse_horizon(s: &str) -> Duration {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let value: i64 = parts
        .first()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    match parts.get(1).map(|s| s.trim_end_matches('s')) {
        Some("hour") => Duration::hours(value),
        Some("minute") => Duration::minutes(value),
        Some("day") => Duration::days(value),
        _ => Duration::hours(value), // default to hours
    }
}

/// Format a Duration as a human-readable string suitable for SQL interval.
pub fn format_duration(d: &Duration) -> String {
    let hours = d.num_hours();
    if hours == 1 {
        "1 hour".to_string()
    } else {
        format!("{} hours", hours)
    }
}

/// Sanitize a field name for use in dynamic SQL.
///
/// Only allows alphanumeric characters and underscores to prevent SQL injection.
fn sanitize_field_name(field: &str) -> String {
    field
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Determine if a breach occurred based on the prediction's objective.
pub fn determine_breach(
    value: f64,
    predicted_breach: Option<bool>,
    threshold: f64,
    direction: ThresholdDirection,
) -> bool {
    let _ = predicted_breach;
    match direction {
        ThresholdDirection::Above => value > threshold,
        ThresholdDirection::Below => value < threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_horizon_hours() {
        assert_eq!(parse_horizon("1 hour"), Duration::hours(1));
        assert_eq!(parse_horizon("6 hours"), Duration::hours(6));
        assert_eq!(parse_horizon("24 hours"), Duration::hours(24));
    }

    #[test]
    fn test_parse_horizon_minutes() {
        assert_eq!(parse_horizon("30 minutes"), Duration::minutes(30));
    }

    #[test]
    fn test_parse_horizon_days() {
        assert_eq!(parse_horizon("7 days"), Duration::days(7));
    }

    #[test]
    fn test_parse_horizon_default() {
        // Unknown unit defaults to hours
        assert_eq!(parse_horizon("3 unknown"), Duration::hours(3));
    }

    #[test]
    fn test_format_duration_single_hour() {
        assert_eq!(format_duration(&Duration::hours(1)), "1 hour");
    }

    #[test]
    fn test_format_duration_multiple_hours() {
        assert_eq!(format_duration(&Duration::hours(6)), "6 hours");
        assert_eq!(format_duration(&Duration::hours(24)), "24 hours");
    }

    #[test]
    fn test_sanitize_field_name() {
        assert_eq!(sanitize_field_name("pm25_mean"), "pm25_mean");
        assert_eq!(sanitize_field_name("co2_mean"), "co2_mean");
        // Prevents injection
        assert_eq!(sanitize_field_name("field; DROP TABLE"), "fieldDROPTABLE");
    }

    #[test]
    fn test_threshold_direction_above() {
        assert!(determine_breach(36.0, None, 35.0, ThresholdDirection::Above));
        assert!(!determine_breach(34.0, None, 35.0, ThresholdDirection::Above));
    }

    #[test]
    fn test_threshold_direction_below() {
        assert!(determine_breach(39.0, None, 40.0, ThresholdDirection::Below));
        assert!(!determine_breach(41.0, None, 40.0, ThresholdDirection::Below));
    }

    #[test]
    fn test_evaluation_summary_default() {
        let summary = EvaluationSummary::default();
        assert_eq!(summary.evaluated, 0);
        assert_eq!(summary.correct, 0);
        assert_eq!(summary.incorrect, 0);
    }

    #[test]
    fn test_evaluation_summary_display() {
        let summary = EvaluationSummary {
            evaluated: 10,
            correct: 7,
            incorrect: 3,
        };
        let s = format!("{}", summary);
        assert!(s.contains("10"));
        assert!(s.contains("7"));
        assert!(s.contains("3"));
    }

    #[test]
    fn test_objective_metric_serde() {
        let json = r#"{"field":"pm25_mean","threshold":35.0,"direction":"above","label":"PM2.5 unhealthy"}"#;
        let metric: ObjectiveMetric = serde_json::from_str(json).unwrap();
        assert_eq!(metric.field, "pm25_mean");
        assert_eq!(metric.direction, ThresholdDirection::Above);
    }
}
