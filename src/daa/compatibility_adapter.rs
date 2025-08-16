//! Compatibility Adapter for Enhanced Performance Snapshots
//!
//! This module provides seamless integration between the original DAA system
//! and the enhanced performance snapshots, ensuring no breaking changes to
//! existing decision flows while enabling new data type discovery features.

use anyhow::Result;
use crate::daa::autonomous_training::{PerformanceSnapshot, TrainingDecision, AutonomousTrainingEngine};
use crate::daa::enhanced_performance_snapshot::{EnhancedPerformanceSnapshot, DataTypeMetrics};

/// Adapter that wraps the original AutonomousTrainingEngine to work with enhanced snapshots
pub struct EnhancedTrainingEngineAdapter {
    /// Original training engine - preserved for backward compatibility
    pub base_engine: AutonomousTrainingEngine,
    /// Whether to use enhanced features or fall back to base functionality
    pub enhanced_mode: bool,
}

impl EnhancedTrainingEngineAdapter {
    /// Create adapter from existing training engine
    pub fn new(base_engine: AutonomousTrainingEngine) -> Self {
        Self {
            base_engine,
            enhanced_mode: true,
        }
    }
    
    /// Create adapter with enhanced mode disabled (pure backward compatibility)
    pub fn new_legacy_mode(base_engine: AutonomousTrainingEngine) -> Self {
        Self {
            base_engine,
            enhanced_mode: false,
        }
    }
    
    /// Evaluate training need using enhanced snapshot
    pub async fn evaluate_training_need_enhanced(
        &self,
        enhanced_snapshot: &EnhancedPerformanceSnapshot,
    ) -> Result<TrainingDecision> {
        // Always use the base snapshot for decision making to maintain compatibility
        let base_snapshot = enhanced_snapshot.base().clone();
        
        // Get the original decision
        let mut decision = self.base_engine.evaluate_training_need(base_snapshot).await?;
        
        if self.enhanced_mode {
            // Enhance the decision with additional insights from data type metrics
            self.enhance_decision_with_patterns(&mut decision, enhanced_snapshot);
        }
        
        Ok(decision)
    }
    
    /// Evaluate training need using base snapshot (full backward compatibility)
    pub async fn evaluate_training_need(
        &self,
        snapshot: PerformanceSnapshot,
    ) -> Result<TrainingDecision> {
        self.base_engine.evaluate_training_need(snapshot).await
    }
    
    /// Enhance training decision with data type insights
    fn enhance_decision_with_patterns(
        &self,
        decision: &mut TrainingDecision,
        enhanced_snapshot: &EnhancedPerformanceSnapshot,
    ) {
        // Add reasoning based on data quality issues
        let critical_issues = enhanced_snapshot.critical_quality_issues();
        if !critical_issues.is_empty() {
            decision.reasoning.push(format!(
                "Data quality concerns detected: {} critical issues",
                critical_issues.len()
            ));
            
            // Suggest more thorough training if data quality is poor
            if critical_issues.len() >= 3 {
                decision.reasoning.push(
                    "Multiple data quality issues suggest need for comprehensive retraining".to_string()
                );
            }
        }
        
        // Add reasoning based on pattern discovery needs
        if enhanced_snapshot.needs_pattern_discovery() {
            decision.reasoning.push(
                "Data type patterns require discovery or have low confidence".to_string()
            );
            
            if enhanced_snapshot.data_type_metrics.pattern_confidence < 0.5 {
                decision.reasoning.push(
                    "Low pattern confidence suggests need for enhanced training data".to_string()
                );
            }
        }
        
        // Add reasoning based on data completeness
        if enhanced_snapshot.data_completeness_score < 0.7 {
            decision.reasoning.push(format!(
                "Data completeness below threshold: {:.1}%",
                enhanced_snapshot.data_completeness_score * 100.0
            ));
        }
        
        // Add pattern-specific insights
        for (field, pattern) in &enhanced_snapshot.data_type_metrics.discovered_patterns {
            match pattern {
                crate::daa::enhanced_performance_snapshot::DataTypePattern::Numerical { distribution_type, .. } => {
                    if matches!(distribution_type, crate::daa::enhanced_performance_snapshot::DistributionType::Unknown) {
                        decision.reasoning.push(format!(
                            "Unknown distribution pattern in {}: may need specialized training",
                            field
                        ));
                    }
                }
                crate::daa::enhanced_performance_snapshot::DataTypePattern::TimeSeries { stationarity_p_value, .. } => {
                    if let Some(p_val) = stationarity_p_value {
                        if *p_val > 0.05 {
                            decision.reasoning.push(format!(
                                "Non-stationary time series detected in {}: consider trend analysis",
                                field
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        
        // Adjust confidence based on enhancement score
        let enhancement_score = enhanced_snapshot.enhancement_score();
        if enhancement_score < 0.5 {
            decision.confidence = (decision.confidence * 0.8).max(0.1);
            decision.reasoning.push(
                "Reduced confidence due to low data enhancement score".to_string()
            );
        }
    }
    
    /// Convert enhanced snapshot to base snapshot for legacy compatibility
    pub fn to_base_snapshot(enhanced: &EnhancedPerformanceSnapshot) -> PerformanceSnapshot {
        enhanced.base().clone()
    }
    
    /// Convert base snapshot to enhanced snapshot with default metrics
    pub fn to_enhanced_snapshot(base: PerformanceSnapshot) -> EnhancedPerformanceSnapshot {
        EnhancedPerformanceSnapshot::from_base_snapshot(base)
    }
    
    /// Check if enhanced features are available and working
    pub fn enhanced_features_available(&self) -> bool {
        self.enhanced_mode
    }
    
    /// Get decision history (delegates to base engine)
    pub async fn get_decision_history(&self) -> Vec<crate::daa::autonomous_training::TrainingDecisionRecord> {
        self.base_engine.get_decision_history().await
    }
}

/// Utility functions for seamless integration
impl EnhancedTrainingEngineAdapter {
    /// Process a batch of snapshots with mixed types
    pub async fn process_mixed_snapshots(
        &self,
        snapshots: Vec<SnapshotType>,
    ) -> Result<Vec<TrainingDecision>> {
        let mut decisions = Vec::new();
        
        for snapshot in snapshots {
            let decision = match snapshot {
                SnapshotType::Base(base) => {
                    self.evaluate_training_need(base).await?
                }
                SnapshotType::Enhanced(enhanced) => {
                    self.evaluate_training_need_enhanced(&enhanced).await?
                }
            };
            decisions.push(decision);
        }
        
        Ok(decisions)
    }
    
    /// Migrate from base to enhanced snapshot with data discovery
    pub async fn migrate_to_enhanced(
        &self,
        base_snapshot: PerformanceSnapshot,
        data_type_metrics: Option<DataTypeMetrics>,
    ) -> EnhancedPerformanceSnapshot {
        let mut enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base_snapshot);
        
        if let Some(metrics) = data_type_metrics {
            enhanced.data_type_metrics = metrics;
            
            // Calculate completeness score based on patterns
            let completeness = if enhanced.data_type_metrics.discovered_patterns.is_empty() {
                0.0
            } else {
                enhanced.data_type_metrics.field_completeness
                    .values()
                    .sum::<f64>() / enhanced.data_type_metrics.field_completeness.len() as f64
            };
            
            enhanced.data_completeness_score = completeness;
        }
        
        enhanced
    }
}

/// Enum for handling mixed snapshot types
#[derive(Debug, Clone)]
pub enum SnapshotType {
    Base(PerformanceSnapshot),
    Enhanced(EnhancedPerformanceSnapshot),
}

impl From<PerformanceSnapshot> for SnapshotType {
    fn from(snapshot: PerformanceSnapshot) -> Self {
        SnapshotType::Base(snapshot)
    }
}

impl From<EnhancedPerformanceSnapshot> for SnapshotType {
    fn from(snapshot: EnhancedPerformanceSnapshot) -> Self {
        SnapshotType::Enhanced(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daa::autonomous_training::TrainingTriggerConfig;
    use chrono::Utc;

    fn create_test_base_snapshot() -> PerformanceSnapshot {
        PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85,
            latency_ms: 100,
            error_rate: 0.15,
            recent_predictions: 50,
            confidence: 0.8,
            price_error: 0.05,
            sharpe_ratio: 1.2,
            max_drawdown: 0.05,
            volatility: 0.1,
            model_agreement: 0.9,
            consecutive_failures: 0,
            trading_volume: 1000.0,
            profit_loss: 50.0,
            data_type_metrics: None,
            event_count: 100,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 30.0,
            memory_usage: 128.0,
            active_connections: 5,
            requests_per_second: 15.0,
            average_response_time: 60.0,
            cache_hit_rate: 0.80,
        }
    }

    #[tokio::test]
    async fn test_backward_compatibility() {
        let config = TrainingTriggerConfig::default();
        let base_engine = AutonomousTrainingEngine::new(config).unwrap();
        let adapter = EnhancedTrainingEngineAdapter::new_legacy_mode(base_engine);
        
        let base_snapshot = create_test_base_snapshot();
        let decision = adapter.evaluate_training_need(base_snapshot).await.unwrap();
        
        // Should work exactly like the original system
        assert!(decision.confidence > 0.0);
        assert!(!decision.reasoning.is_empty());
    }

    #[tokio::test]
    async fn test_enhanced_mode() {
        let config = TrainingTriggerConfig::default();
        let base_engine = AutonomousTrainingEngine::new(config).unwrap();
        let adapter = EnhancedTrainingEngineAdapter::new(base_engine);
        
        let base_snapshot = create_test_base_snapshot();
        let enhanced_snapshot = EnhancedPerformanceSnapshot::from_base_snapshot(base_snapshot);
        
        let decision = adapter.evaluate_training_need_enhanced(&enhanced_snapshot).await.unwrap();
        
        // Should include enhanced reasoning
        assert!(decision.confidence > 0.0);
        assert!(!decision.reasoning.is_empty());
    }

    #[tokio::test]
    async fn test_mixed_snapshot_processing() {
        let config = TrainingTriggerConfig::default();
        let base_engine = AutonomousTrainingEngine::new(config).unwrap();
        let adapter = EnhancedTrainingEngineAdapter::new(base_engine);
        
        let base_snapshot = create_test_base_snapshot();
        let enhanced_snapshot = EnhancedPerformanceSnapshot::from_base_snapshot(base_snapshot.clone());
        
        let snapshots = vec![
            SnapshotType::Base(base_snapshot),
            SnapshotType::Enhanced(enhanced_snapshot),
        ];
        
        let decisions = adapter.process_mixed_snapshots(snapshots).await.unwrap();
        assert_eq!(decisions.len(), 2);
    }

    #[test]
    fn test_snapshot_conversions() {
        let base = create_test_base_snapshot();
        let original_accuracy = base.accuracy;
        
        // Convert to enhanced and back
        let enhanced = EnhancedTrainingEngineAdapter::to_enhanced_snapshot(base);
        let recovered = EnhancedTrainingEngineAdapter::to_base_snapshot(&enhanced);
        
        assert_eq!(recovered.accuracy, original_accuracy);
    }
}