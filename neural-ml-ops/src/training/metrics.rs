//! Training Metrics Collection and Analysis
//!
//! Provides comprehensive metrics collection, analysis, and reporting
//! for ML training workflows.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Core training metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    pub epoch: Option<u32>,
    pub training_loss: Option<f64>,
    pub validation_loss: Option<f64>,
    pub accuracy: Option<f64>,
    pub learning_rate: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

impl Default for TrainingMetrics {
    fn default() -> Self {
        Self {
            epoch: None,
            training_loss: None,
            validation_loss: None,
            accuracy: None,
            learning_rate: None,
            timestamp: Utc::now(),
        }
    }
}

/// Extended metrics for comprehensive analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedMetrics {
    pub base_metrics: TrainingMetrics,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub f1_score: Option<f64>,
    pub auc_score: Option<f64>,
    pub training_time_seconds: Option<f64>,
    pub memory_usage_mb: Option<f64>,
    pub gpu_utilization_percent: Option<f64>,
    pub batch_size: Option<u32>,
    pub gradient_norm: Option<f64>,
    pub learning_rate_schedule: Option<String>,
    pub custom_metrics: HashMap<String, f64>,
}

/// Metrics time series for tracking over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsTimeSeries {
    pub job_id: Uuid,
    pub workflow_id: String,
    pub model_name: String,
    pub metrics_history: Vec<ExtendedMetrics>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

/// Metrics collector and analyzer
pub struct MetricsCollector {
    metrics_storage: Arc<RwLock<HashMap<Uuid, MetricsTimeSeries>>>,
    aggregated_metrics: Arc<RwLock<HashMap<String, AggregatedMetrics>>>,
    retention_limit: usize,
}

/// Aggregated metrics across multiple training runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub workflow_id: String,
    pub total_runs: u32,
    pub successful_runs: u32,
    pub average_accuracy: f64,
    pub best_accuracy: f64,
    pub worst_accuracy: f64,
    pub average_training_time: f64,
    pub average_loss: f64,
    pub convergence_statistics: ConvergenceStats,
    pub resource_utilization: ResourceStats,
    pub last_updated: DateTime<Utc>,
}

/// Convergence analysis statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceStats {
    pub average_epochs_to_converge: f64,
    pub convergence_rate: f64, // Percentage of models that converged
    pub early_stopping_rate: f64,
    pub overfitting_rate: f64,
}

/// Resource utilization statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStats {
    pub average_memory_usage_mb: f64,
    pub peak_memory_usage_mb: f64,
    pub average_gpu_utilization: f64,
    pub average_training_time_per_epoch: f64,
}

/// Performance comparison between models/configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceComparison {
    pub baseline_metrics: AggregatedMetrics,
    pub comparison_metrics: AggregatedMetrics,
    pub improvement_percentage: f64,
    pub statistical_significance: f64,
    pub recommendation: String,
}

/// Anomaly detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingAnomaly {
    pub job_id: Uuid,
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub suggested_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalyType {
    LossSpike,
    AccuracyDrop,
    ConvergenceFailure,
    ResourceAnomaly,
    NumericalInstability,
    SlowConvergence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            metrics_storage: Arc::new(RwLock::new(HashMap::new())),
            aggregated_metrics: Arc::new(RwLock::new(HashMap::new())),
            retention_limit: 1000, // Keep last 1000 training runs
        }
    }
    
    /// Start collecting metrics for a training job
    pub async fn start_collection(
        &self,
        job_id: Uuid,
        workflow_id: String,
        model_name: String,
    ) -> Result<()> {
        info!("Starting metrics collection for job: {}", job_id);
        
        let time_series = MetricsTimeSeries {
            job_id,
            workflow_id,
            model_name,
            metrics_history: Vec::new(),
            start_time: Utc::now(),
            end_time: None,
        };
        
        self.metrics_storage.write().await.insert(job_id, time_series);
        
        Ok(())
    }
    
    /// Record training metrics for a job
    pub async fn record_metrics(
        &self,
        job_id: Uuid,
        metrics: ExtendedMetrics,
    ) -> Result<()> {
        debug!("Recording metrics for job: {}", job_id);
        
        if let Some(time_series) = self.metrics_storage.write().await.get_mut(&job_id) {
            time_series.metrics_history.push(metrics);
            
            // Detect anomalies in real-time
            if let Some(anomaly) = self.detect_anomalies(&time_series.metrics_history).await {
                info!("Training anomaly detected: {:?}", anomaly);
            }
        }
        
        Ok(())
    }
    
    /// Finish metrics collection for a job
    pub async fn finish_collection(&self, job_id: Uuid) -> Result<()> {
        info!("Finishing metrics collection for job: {}", job_id);
        
        if let Some(time_series) = self.metrics_storage.write().await.get_mut(&job_id) {
            time_series.end_time = Some(Utc::now());
            
            // Update aggregated metrics
            self.update_aggregated_metrics(time_series).await?;
        }
        
        // Clean up old metrics if needed
        self.cleanup_old_metrics().await?;
        
        Ok(())
    }
    
    /// Get metrics for a specific job
    pub async fn get_job_metrics(&self, job_id: Uuid) -> Option<MetricsTimeSeries> {
        self.metrics_storage.read().await.get(&job_id).cloned()
    }
    
    /// Get aggregated metrics for a workflow
    pub async fn get_aggregated_metrics(&self, workflow_id: &str) -> Option<AggregatedMetrics> {
        self.aggregated_metrics.read().await.get(workflow_id).cloned()
    }
    
    /// Get performance comparison between two workflows
    pub async fn compare_performance(
        &self,
        baseline_workflow: &str,
        comparison_workflow: &str,
    ) -> Result<PerformanceComparison> {
        let baseline_metrics = self.get_aggregated_metrics(baseline_workflow).await
            .ok_or_else(|| anyhow::anyhow!("Baseline metrics not found"))?;
        
        let comparison_metrics = self.get_aggregated_metrics(comparison_workflow).await
            .ok_or_else(|| anyhow::anyhow!("Comparison metrics not found"))?;
        
        let improvement_percentage = if baseline_metrics.average_accuracy > 0.0 {
            ((comparison_metrics.average_accuracy - baseline_metrics.average_accuracy) 
                / baseline_metrics.average_accuracy) * 100.0
        } else {
            0.0
        };
        
        let statistical_significance = self.calculate_statistical_significance(
            &baseline_metrics,
            &comparison_metrics,
        );
        
        let recommendation = self.generate_recommendation(
            improvement_percentage,
            statistical_significance,
        );
        
        Ok(PerformanceComparison {
            baseline_metrics,
            comparison_metrics,
            improvement_percentage,
            statistical_significance,
            recommendation,
        })
    }
    
    /// Analyze training trends over time
    pub async fn analyze_training_trends(&self, workflow_id: &str) -> Result<TrendAnalysis> {
        let storage = self.metrics_storage.read().await;
        let workflow_metrics: Vec<&MetricsTimeSeries> = storage
            .values()
            .filter(|ts| ts.workflow_id == workflow_id)
            .collect();
        
        if workflow_metrics.is_empty() {
            return Err(anyhow::anyhow!("No metrics found for workflow: {}", workflow_id));
        }
        
        let total_runs = workflow_metrics.len();
        
        // Calculate trends
        let mut accuracy_trend = Vec::new();
        let mut loss_trend = Vec::new();
        let mut training_time_trend = Vec::new();
        
        for time_series in workflow_metrics {
            if let Some(last_metric) = time_series.metrics_history.last() {
                if let Some(accuracy) = last_metric.base_metrics.accuracy {
                    accuracy_trend.push((time_series.start_time, accuracy));
                }
                
                if let Some(loss) = last_metric.base_metrics.validation_loss {
                    loss_trend.push((time_series.start_time, loss));
                }
                
                if let Some(training_time) = last_metric.training_time_seconds {
                    training_time_trend.push((time_series.start_time, training_time));
                }
            }
        }
        
        Ok(TrendAnalysis {
            workflow_id: workflow_id.to_string(),
            accuracy_trend: self.calculate_trend_direction(&accuracy_trend),
            loss_trend: self.calculate_trend_direction(&loss_trend),
            training_time_trend: self.calculate_trend_direction(&training_time_trend),
            total_runs,
            analysis_date: Utc::now(),
        })
    }
    
    /// Export metrics to external format
    pub async fn export_metrics(
        &self,
        job_id: Option<Uuid>,
        workflow_id: Option<String>,
        format: ExportFormat,
    ) -> Result<String> {
        let storage = self.metrics_storage.read().await;
        
        let metrics_to_export: Vec<&MetricsTimeSeries> = storage
            .values()
            .filter(|ts| {
                if let Some(jid) = job_id {
                    return ts.job_id == jid;
                }
                if let Some(ref wid) = workflow_id {
                    return ts.workflow_id == *wid;
                }
                true // Export all if no filters
            })
            .collect();
        
        match format {
            ExportFormat::JSON => {
                Ok(serde_json::to_string_pretty(&metrics_to_export)?)
            }
            ExportFormat::CSV => {
                self.export_to_csv(&metrics_to_export)
            }
        }
    }
    
    // Private methods
    
    async fn update_aggregated_metrics(&self, time_series: &MetricsTimeSeries) -> Result<()> {
        let mut aggregated = self.aggregated_metrics.write().await;
        
        let workflow_id = &time_series.workflow_id;
        let entry = aggregated.entry(workflow_id.clone()).or_insert_with(|| {
            AggregatedMetrics {
                workflow_id: workflow_id.clone(),
                total_runs: 0,
                successful_runs: 0,
                average_accuracy: 0.0,
                best_accuracy: 0.0,
                worst_accuracy: 1.0,
                average_training_time: 0.0,
                average_loss: 0.0,
                convergence_statistics: ConvergenceStats {
                    average_epochs_to_converge: 0.0,
                    convergence_rate: 0.0,
                    early_stopping_rate: 0.0,
                    overfitting_rate: 0.0,
                },
                resource_utilization: ResourceStats {
                    average_memory_usage_mb: 0.0,
                    peak_memory_usage_mb: 0.0,
                    average_gpu_utilization: 0.0,
                    average_training_time_per_epoch: 0.0,
                },
                last_updated: Utc::now(),
            }
        });
        
        // Update aggregated statistics
        entry.total_runs += 1;
        
        if let Some(final_metrics) = time_series.metrics_history.last() {
            if let Some(accuracy) = final_metrics.base_metrics.accuracy {
                entry.successful_runs += 1;
                
                // Update accuracy statistics
                let old_average = entry.average_accuracy;
                entry.average_accuracy = (old_average * (entry.successful_runs - 1) as f64 + accuracy) 
                    / entry.successful_runs as f64;
                
                entry.best_accuracy = entry.best_accuracy.max(accuracy);
                entry.worst_accuracy = entry.worst_accuracy.min(accuracy);
            }
            
            if let Some(loss) = final_metrics.base_metrics.validation_loss {
                let old_loss_average = entry.average_loss;
                entry.average_loss = (old_loss_average * (entry.total_runs - 1) as f64 + loss)
                    / entry.total_runs as f64;
            }
            
            if let Some(training_time) = final_metrics.training_time_seconds {
                let old_time_average = entry.average_training_time;
                entry.average_training_time = (old_time_average * (entry.total_runs - 1) as f64 + training_time)
                    / entry.total_runs as f64;
            }
        }
        
        entry.last_updated = Utc::now();
        
        Ok(())
    }
    
    async fn detect_anomalies(&self, metrics_history: &[ExtendedMetrics]) -> Option<TrainingAnomaly> {
        if metrics_history.len() < 5 {
            return None; // Need sufficient history for anomaly detection
        }
        
        let recent_metrics = &metrics_history[metrics_history.len() - 5..];
        
        // Check for loss spikes
        if let Some(anomaly) = self.detect_loss_spikes(recent_metrics) {
            return Some(anomaly);
        }
        
        // Check for accuracy drops
        if let Some(anomaly) = self.detect_accuracy_drops(recent_metrics) {
            return Some(anomaly);
        }
        
        // Check for numerical instability
        if let Some(anomaly) = self.detect_numerical_instability(recent_metrics) {
            return Some(anomaly);
        }
        
        None
    }
    
    fn detect_loss_spikes(&self, metrics: &[ExtendedMetrics]) -> Option<TrainingAnomaly> {
        let losses: Vec<f64> = metrics
            .iter()
            .filter_map(|m| m.base_metrics.training_loss)
            .collect();
        
        if losses.len() < 3 {
            return None;
        }
        
        let recent_loss = losses.last()?;
        let previous_losses = &losses[..losses.len() - 1];
        let average_loss = previous_losses.iter().sum::<f64>() / previous_losses.len() as f64;
        
        // Consider a spike if current loss is > 50% higher than average
        if *recent_loss > average_loss * 1.5 && average_loss > 0.0 {
            return Some(TrainingAnomaly {
                job_id: Uuid::new_v4(), // Would be passed as parameter
                anomaly_type: AnomalyType::LossSpike,
                severity: AnomalySeverity::High,
                description: format!("Loss spike detected: {:.4} vs average {:.4}", 
                                   recent_loss, average_loss),
                detected_at: Utc::now(),
                suggested_action: "Consider reducing learning rate or checking data quality".to_string(),
            });
        }
        
        None
    }
    
    fn detect_accuracy_drops(&self, metrics: &[ExtendedMetrics]) -> Option<TrainingAnomaly> {
        let accuracies: Vec<f64> = metrics
            .iter()
            .filter_map(|m| m.base_metrics.accuracy)
            .collect();
        
        if accuracies.len() < 3 {
            return None;
        }
        
        // Check for consistent decline in accuracy
        let mut declining_count = 0;
        for window in accuracies.windows(2) {
            if window[1] < window[0] {
                declining_count += 1;
            }
        }
        
        if declining_count >= 2 && accuracies.len() >= 3 {
            let first_accuracy = accuracies[0];
            let last_accuracy = *accuracies.last()?;
            let drop_percentage = ((first_accuracy - last_accuracy) / first_accuracy) * 100.0;
            
            if drop_percentage > 10.0 { // More than 10% drop
                return Some(TrainingAnomaly {
                    job_id: Uuid::new_v4(),
                    anomaly_type: AnomalyType::AccuracyDrop,
                    severity: AnomalySeverity::Medium,
                    description: format!("Accuracy dropping: {:.2}% decline over recent epochs", 
                                       drop_percentage),
                    detected_at: Utc::now(),
                    suggested_action: "Consider early stopping or adjusting learning rate".to_string(),
                });
            }
        }
        
        None
    }
    
    fn detect_numerical_instability(&self, metrics: &[ExtendedMetrics]) -> Option<TrainingAnomaly> {
        for metric in metrics {
            if let Some(loss) = metric.base_metrics.training_loss {
                if loss.is_nan() || loss.is_infinite() {
                    return Some(TrainingAnomaly {
                        job_id: Uuid::new_v4(),
                        anomaly_type: AnomalyType::NumericalInstability,
                        severity: AnomalySeverity::Critical,
                        description: "NaN or infinite loss detected".to_string(),
                        detected_at: Utc::now(),
                        suggested_action: "Reduce learning rate and check for gradient clipping".to_string(),
                    });
                }
            }
        }
        
        None
    }
    
    async fn cleanup_old_metrics(&self) -> Result<()> {
        let mut storage = self.metrics_storage.write().await;
        
        if storage.len() > self.retention_limit {
            // Keep only the most recent metrics
            let mut entries: Vec<_> = storage.drain().collect();
            entries.sort_by_key(|(_, ts)| ts.start_time);
            
            // Keep only the last N entries
            let keep_count = (self.retention_limit * 80) / 100; // Keep 80% of limit
            entries.truncate(entries.len().saturating_sub(keep_count));
            
            for (job_id, time_series) in entries.into_iter().rev() {
                storage.insert(job_id, time_series);
            }
            
            info!("Cleaned up metrics storage, keeping {} entries", storage.len());
        }
        
        Ok(())
    }
    
    fn calculate_statistical_significance(
        &self,
        baseline: &AggregatedMetrics,
        comparison: &AggregatedMetrics,
    ) -> f64 {
        // Simplified statistical significance calculation
        // In practice, would use proper statistical tests
        let sample_size = (baseline.total_runs + comparison.total_runs) as f64;
        let effect_size = (comparison.average_accuracy - baseline.average_accuracy).abs();
        
        // Rough approximation - would use t-test or similar in practice
        if sample_size > 30.0 && effect_size > 0.05 {
            0.95 // High significance
        } else if sample_size > 10.0 && effect_size > 0.02 {
            0.80 // Medium significance
        } else {
            0.60 // Low significance
        }
    }
    
    fn generate_recommendation(
        &self,
        improvement_percentage: f64,
        statistical_significance: f64,
    ) -> String {
        if statistical_significance < 0.70 {
            return "Insufficient data for reliable recommendation. Collect more samples.".to_string();
        }
        
        if improvement_percentage > 5.0 {
            "Strong positive improvement detected. Recommend adopting new configuration.".to_string()
        } else if improvement_percentage > 1.0 {
            "Moderate improvement detected. Consider A/B testing before full adoption.".to_string()
        } else if improvement_percentage < -5.0 {
            "Performance degradation detected. Recommend reverting to baseline.".to_string()
        } else {
            "No significant performance difference detected.".to_string()
        }
    }
    
    fn calculate_trend_direction(&self, data: &[(DateTime<Utc>, f64)]) -> TrendDirection {
        if data.len() < 3 {
            return TrendDirection::Stable;
        }
        
        // Simple linear regression slope calculation
        let n = data.len() as f64;
        let sum_x: f64 = (0..data.len()).map(|i| i as f64).sum();
        let sum_y: f64 = data.iter().map(|(_, y)| *y).sum();
        let sum_xy: f64 = data.iter().enumerate().map(|(i, (_, y))| i as f64 * y).sum();
        let sum_x2: f64 = (0..data.len()).map(|i| (i as f64).powi(2)).sum();
        
        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x.powi(2));
        
        if slope > 0.01 {
            TrendDirection::Improving
        } else if slope < -0.01 {
            TrendDirection::Declining
        } else {
            TrendDirection::Stable
        }
    }
    
    fn export_to_csv(&self, metrics: &[&MetricsTimeSeries]) -> Result<String> {
        let mut csv = String::new();
        csv.push_str("job_id,workflow_id,model_name,epoch,training_loss,validation_loss,accuracy,learning_rate,timestamp\n");
        
        for time_series in metrics {
            for metric in &time_series.metrics_history {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{}\n",
                    time_series.job_id,
                    time_series.workflow_id,
                    time_series.model_name,
                    metric.base_metrics.epoch.map_or("".to_string(), |e| e.to_string()),
                    metric.base_metrics.training_loss.map_or("".to_string(), |l| l.to_string()),
                    metric.base_metrics.validation_loss.map_or("".to_string(), |l| l.to_string()),
                    metric.base_metrics.accuracy.map_or("".to_string(), |a| a.to_string()),
                    metric.base_metrics.learning_rate.map_or("".to_string(), |lr| lr.to_string()),
                    metric.base_metrics.timestamp
                ));
            }
        }
        
        Ok(csv)
    }
}

/// Trend analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub workflow_id: String,
    pub accuracy_trend: TrendDirection,
    pub loss_trend: TrendDirection,
    pub training_time_trend: TrendDirection,
    pub total_runs: usize,
    pub analysis_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Declining,
    Stable,
}

/// Export format options
#[derive(Debug, Clone)]
pub enum ExportFormat {
    JSON,
    CSV,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new();
        let job_id = Uuid::new_v4();
        
        // Start collection
        collector.start_collection(
            job_id,
            "test-workflow".to_string(),
            "test-model".to_string(),
        ).await.unwrap();
        
        // Record some metrics
        let metrics = ExtendedMetrics {
            base_metrics: TrainingMetrics {
                epoch: Some(1),
                training_loss: Some(0.5),
                validation_loss: Some(0.6),
                accuracy: Some(0.8),
                learning_rate: Some(0.001),
                timestamp: Utc::now(),
            },
            precision: Some(0.75),
            recall: Some(0.82),
            f1_score: Some(0.78),
            auc_score: Some(0.85),
            training_time_seconds: Some(300.0),
            memory_usage_mb: Some(1024.0),
            gpu_utilization_percent: Some(75.0),
            batch_size: Some(32),
            gradient_norm: Some(1.2),
            learning_rate_schedule: Some("cosine".to_string()),
            custom_metrics: HashMap::new(),
        };
        
        collector.record_metrics(job_id, metrics).await.unwrap();
        
        // Finish collection
        collector.finish_collection(job_id).await.unwrap();
        
        // Verify metrics were stored
        let stored_metrics = collector.get_job_metrics(job_id).await;
        assert!(stored_metrics.is_some());
        
        let time_series = stored_metrics.unwrap();
        assert_eq!(time_series.job_id, job_id);
        assert_eq!(time_series.workflow_id, "test-workflow");
        assert_eq!(time_series.metrics_history.len(), 1);
    }
    
    #[tokio::test]
    async fn test_aggregated_metrics() {
        let collector = MetricsCollector::new();
        
        // Simulate multiple training runs
        for i in 0..3 {
            let job_id = Uuid::new_v4();
            collector.start_collection(
                job_id,
                "test-workflow".to_string(),
                "test-model".to_string(),
            ).await.unwrap();
            
            let metrics = ExtendedMetrics {
                base_metrics: TrainingMetrics {
                    epoch: Some(10),
                    training_loss: Some(0.1 + i as f64 * 0.1),
                    validation_loss: Some(0.15 + i as f64 * 0.1),
                    accuracy: Some(0.9 - i as f64 * 0.05),
                    learning_rate: Some(0.001),
                    timestamp: Utc::now(),
                },
                training_time_seconds: Some(600.0),
                ..Default::default()
            };
            
            collector.record_metrics(job_id, metrics).await.unwrap();
            collector.finish_collection(job_id).await.unwrap();
        }
        
        // Check aggregated metrics
        let aggregated = collector.get_aggregated_metrics("test-workflow").await;
        assert!(aggregated.is_some());
        
        let agg = aggregated.unwrap();
        assert_eq!(agg.total_runs, 3);
        assert_eq!(agg.successful_runs, 3);
        assert!(agg.average_accuracy > 0.0);
    }
}

impl Default for ExtendedMetrics {
    fn default() -> Self {
        Self {
            base_metrics: TrainingMetrics::default(),
            precision: None,
            recall: None,
            f1_score: None,
            auc_score: None,
            training_time_seconds: None,
            memory_usage_mb: None,
            gpu_utilization_percent: None,
            batch_size: None,
            gradient_norm: None,
            learning_rate_schedule: None,
            custom_metrics: HashMap::new(),
        }
    }
}