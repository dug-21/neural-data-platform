//! Outcome tracking for prediction evaluation
//!
//! Evaluates pending predictions against actual Gold layer values
//! to determine prediction accuracy.

use std::sync::Arc;

use chrono::Utc;
use deadpool_postgres::Pool;
use tracing::{debug, info, warn};

use super::{EvaluationSummary, ObjectiveMetric, ThresholdDirection};
use crate::error::{IntelligenceError, Result};
use crate::storage::StorageBackend;

/// Tracks prediction outcomes by comparing predictions to actual values.
pub struct OutcomeTracker {
    db_pool: Arc<Pool>,
    storage: Arc<dyn StorageBackend>,
    objective_metrics: Vec<ObjectiveMetric>,
}

impl OutcomeTracker {
    /// Create a new OutcomeTracker.
    pub fn new(
        db_pool: Arc<Pool>,
        storage: Arc<dyn StorageBackend>,
        objective_metrics: Vec<ObjectiveMetric>,
    ) -> Self {
        Self {
            db_pool,
            storage,
            objective_metrics,
        }
    }

    /// Evaluate all pending predictions whose horizons have elapsed.
    pub async fn evaluate_pending(&self, domain_id: &str) -> Result<EvaluationSummary> {
        let pending = self
            .storage
            .get_pending_outcomes(domain_id)
            .await
            .map_err(IntelligenceError::Storage)?;

        let client = self
            .db_pool
            .get()
            .await
            .map_err(|e| IntelligenceError::Database(format!("Pool error: {}", e)))?;

        let view_name = format!(
            "gold.{}_aligned_hourly",
            domain_id.replace('-', "_")
        );
        let now = Utc::now();
        let mut summary = EvaluationSummary::default();

        for prediction in pending {
            let horizon = super::parse_horizon(&prediction.horizon);
            let outcome_time = prediction.bucket + horizon;

            // Only evaluate if the horizon has elapsed
            if outcome_time > now {
                continue;
            }

            let sql = format!(
                "SELECT {} FROM {} WHERE bucket = $1 LIMIT 1",
                super::sanitize_field_name(&prediction.metric),
                view_name
            );
            let row = client
                .query_opt(&sql, &[&outcome_time])
                .await
                .map_err(|e| IntelligenceError::Database(format!("Query error: {}", e)))?;

            if let Some(row) = row {
                let actual_value: Option<f64> = row.get(0);
                if let Some(value) = actual_value {
                    // Determine if breach actually occurred using objective config
                    let actual_breach =
                        self.determine_breach_from_config(&prediction.metric, value);

                    let correct = prediction.predicted_breach == Some(actual_breach);

                    let outcome = crate::storage::ActualOutcome {
                        actual_value: value,
                        actual_breach,
                        evaluated_at: now,
                    };

                    if let Some(pred_id) = prediction.id {
                        match self.storage.record_outcome(pred_id, &outcome).await {
                            Ok(()) => {
                                summary.evaluated += 1;
                                if correct {
                                    summary.correct += 1;
                                } else {
                                    summary.incorrect += 1;
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to record outcome for prediction {}: {}",
                                    pred_id, e
                                );
                            }
                        }
                    }
                }
            } else {
                debug!(
                    "No Gold data at {} for prediction evaluation",
                    outcome_time
                );
            }
        }

        if summary.evaluated > 0 {
            info!("{}", summary);
        }

        Ok(summary)
    }

    /// Determine if a breach occurred for a given metric and value.
    fn determine_breach_from_config(&self, metric: &str, value: f64) -> bool {
        for objective in &self.objective_metrics {
            if objective.field == metric {
                return match objective.direction {
                    ThresholdDirection::Above => value > objective.threshold,
                    ThresholdDirection::Below => value < objective.threshold,
                };
            }
        }
        // If no matching objective found, default to false (no breach)
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_objectives() -> Vec<ObjectiveMetric> {
        vec![
            ObjectiveMetric {
                field: "pm25_mean".to_string(),
                threshold: 35.0,
                direction: ThresholdDirection::Above,
                label: "PM2.5 unhealthy".to_string(),
            },
            ObjectiveMetric {
                field: "co2_mean".to_string(),
                threshold: 1000.0,
                direction: ThresholdDirection::Above,
                label: "CO2 high".to_string(),
            },
        ]
    }

    #[test]
    fn test_determine_breach_above() {
        // We need a pool to create OutcomeTracker, but we can test the breach logic directly
        let objectives = test_objectives();
        let tracker_objectives = objectives.clone();

        // PM2.5 above 35 is a breach
        assert!(determine_breach_from_objectives(
            &tracker_objectives,
            "pm25_mean",
            36.0
        ));
        assert!(!determine_breach_from_objectives(
            &tracker_objectives,
            "pm25_mean",
            34.0
        ));
    }

    #[test]
    fn test_determine_breach_unknown_metric() {
        let objectives = test_objectives();
        // Unknown metric defaults to false
        assert!(!determine_breach_from_objectives(
            &objectives,
            "unknown_metric",
            100.0
        ));
    }

    // Helper to test breach logic without needing a pool
    fn determine_breach_from_objectives(
        objectives: &[ObjectiveMetric],
        metric: &str,
        value: f64,
    ) -> bool {
        for objective in objectives {
            if objective.field == metric {
                return match objective.direction {
                    ThresholdDirection::Above => value > objective.threshold,
                    ThresholdDirection::Below => value < objective.threshold,
                };
            }
        }
        false
    }
}
