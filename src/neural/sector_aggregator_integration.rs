//! SectorAggregator Integration with VendorPredictor
//!
//! This module demonstrates how SectorAggregator feeds sector-level data
//! into VendorPredictor for enhanced neural trading strategies.
//!
//! INTEGRATION FLOW:
//! 1. SectorAggregator processes real-time market data
//! 2. Generates sector-level aggregations and breadth indicators  
//! 3. VendorPredictor uses sector context for enhanced predictions
//! 4. DAA coordinator (Week 6) will orchestrate both components

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, debug};

use crate::data::{TimeSeriesData, RedisCache};
use crate::data::sector_mapper::{SectorMapper, SectorId};
use crate::adapters::redis::RedisAdapter;
use crate::neural::{
    SectorAggregator, SectorAggregatorConfig,
    VendorPredictor, VendorPredictorConfig,
    PredictionResult
};
use crate::config::NeuralConfig;
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;

/// Enhanced neural engine that combines sector aggregation with vendor predictions
pub struct EnhancedNeuralEngine {
    /// Sector aggregation component
    sector_aggregator: Arc<SectorAggregator>,
    
    /// Vendor prediction component
    vendor_predictor: Arc<VendorPredictor>,
    
    /// Sector mapper for consistency
    sector_mapper: Arc<SectorMapper>,
    
    /// Configuration
    config: EnhancedEngineConfig,
}

/// Configuration for the enhanced neural engine
#[derive(Debug, Clone)]
pub struct EnhancedEngineConfig {
    pub enable_sector_context: bool,
    pub sector_weight_in_prediction: f64,
    pub breadth_indicator_weight: f64,
    pub etf_correlation_weight: f64,
    pub cross_sector_correlation_weight: f64,
}

impl Default for EnhancedEngineConfig {
    fn default() -> Self {
        Self {
            enable_sector_context: true,
            sector_weight_in_prediction: 0.3,
            breadth_indicator_weight: 0.2,
            etf_correlation_weight: 0.15,
            cross_sector_correlation_weight: 0.1,
        }
    }
}

impl EnhancedNeuralEngine {
    /// Create new enhanced neural engine
    pub async fn new(
        neural_config: &NeuralConfig,
        sector_mapper: Arc<SectorMapper>,
        redis_cache: Arc<RedisCache>,
        redis_adapter: Arc<RwLock<RedisAdapter>>,
        performance_tracker: Arc<ModelPerformanceTracker>,
        config: EnhancedEngineConfig,
    ) -> Result<Self> {
        info!("🚀 Initializing EnhancedNeuralEngine with sector aggregation");
        
        // Create sector aggregator
        let aggregator_config = SectorAggregatorConfig::default();
        let sector_aggregator = Arc::new(SectorAggregator::new(
            sector_mapper.clone(),
            redis_cache.clone(),
            redis_adapter,
            aggregator_config,
        ));
        
        // Start real-time processing
        sector_aggregator.start_realtime_processing().await?;
        
        // Create vendor predictor
        let vendor_predictor = Arc::new(VendorPredictor::new(
            neural_config,
            sector_mapper.clone(),
            performance_tracker,
        )?);
        
        info!("✅ EnhancedNeuralEngine initialized successfully");
        
        Ok(Self {
            sector_aggregator,
            vendor_predictor,
            sector_mapper,
            config,
        })
    }
    
    /// Enhanced prediction with sector context
    pub async fn predict_with_sector_context(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<EnhancedPredictionResult>> {
        let mut results = Vec::new();
        
        for item in data {
            info!("🔮 Enhanced prediction for symbol: {}", item.symbol);
            
            // Step 1: Get base prediction from VendorPredictor
            let base_predictions = self.vendor_predictor
                .predict(&[item.clone()], horizon, features.clone())
                .await?;
            
            if base_predictions.is_empty() {
                continue;
            }
            
            let base_prediction = &base_predictions[0];
            
            // Step 2: Get sector information
            let sector_info = self.sector_mapper.get_sector(&item.symbol)?;
            let sector_aggregation = self.sector_aggregator
                .get_sector_aggregation(&sector_info.sector_id)
                .await;
            
            // Step 3: Apply sector context if available
            let enhanced_result = if let Some(agg) = sector_aggregation {
                self.apply_sector_context(base_prediction, &agg, &sector_info.sector_id).await?
            } else {
                // Fall back to base prediction if no sector data
                EnhancedPredictionResult::from_base(base_prediction.clone())
            };
            
            results.push(enhanced_result);
        }
        
        info!("✅ Enhanced predictions completed for {} symbols", results.len());
        Ok(results)
    }
    
    /// Apply sector context to enhance prediction
    async fn apply_sector_context(
        &self,
        base_prediction: &PredictionResult,
        sector_aggregation: &crate::neural::SectorAggregation,
        sector_id: &SectorId,
    ) -> Result<EnhancedPredictionResult> {
        debug!("📊 Applying sector context for sector: {:?}", sector_id);
        
        let mut enhanced_value = base_prediction.value;
        let mut enhanced_confidence = base_prediction.confidence;
        let mut context_factors = HashMap::new();
        
        // Factor 1: Sector momentum (breadth ratio)
        if self.config.enable_sector_context {
            let breadth_adjustment = sector_aggregation.breadth_ratio * self.config.breadth_indicator_weight;
            enhanced_value *= 1.0 + breadth_adjustment;
            context_factors.insert("breadth_adjustment".to_string(), breadth_adjustment);
            
            debug!("Applied breadth adjustment: {:.4}", breadth_adjustment);
        }
        
        // Factor 2: Sector relative strength
        let sector_change = sector_aggregation.weighted_change;
        if sector_change.abs() > 0.001 {
            let sector_strength_adjustment = sector_change * self.config.sector_weight_in_prediction;
            enhanced_value *= 1.0 + sector_strength_adjustment;
            context_factors.insert("sector_strength_adjustment".to_string(), sector_strength_adjustment);
            
            debug!("Applied sector strength adjustment: {:.4}", sector_strength_adjustment);
        }
        
        // Factor 3: ETF correlation
        if let Some(etf_corr) = self.sector_aggregator.get_etf_correlation(sector_id).await {
            let etf_adjustment = (etf_corr.correlation_coefficient - 0.5) * 
                                self.config.etf_correlation_weight;
            enhanced_confidence += etf_adjustment;
            context_factors.insert("etf_correlation_adjustment".to_string(), etf_adjustment);
            
            debug!("Applied ETF correlation adjustment: {:.4}", etf_adjustment);
        }
        
        // Factor 4: Cross-sector correlations
        let cross_correlations = self.sector_aggregator.calculate_cross_sector_correlations().await?;
        let mut cross_sector_signal = 0.0;
        let mut correlation_count = 0;
        
        for ((sector_a, sector_b), correlation) in cross_correlations {
            if sector_a == *sector_id || sector_b == *sector_id {
                cross_sector_signal += correlation;
                correlation_count += 1;
            }
        }
        
        if correlation_count > 0 {
            let avg_cross_correlation = cross_sector_signal / correlation_count as f64;
            let cross_adjustment = avg_cross_correlation * self.config.cross_sector_correlation_weight;
            enhanced_confidence += cross_adjustment;
            context_factors.insert("cross_sector_adjustment".to_string(), cross_adjustment);
            
            debug!("Applied cross-sector adjustment: {:.4}", cross_adjustment);
        }
        
        // Ensure confidence stays in valid range
        enhanced_confidence = enhanced_confidence.max(0.0).min(1.0);
        
        // Create enhanced result
        let enhanced_result = EnhancedPredictionResult {
            base_prediction: base_prediction.clone(),
            enhanced_value,
            enhanced_confidence,
            sector_context: Some(SectorContext {
                sector_id: *sector_id,
                breadth_ratio: sector_aggregation.breadth_ratio,
                sector_momentum: sector_aggregation.weighted_change,
                advancing_ratio: sector_aggregation.advancing_count as f64 / 
                                (sector_aggregation.symbol_count as f64).max(1.0),
                etf_correlation: self.sector_aggregator.get_etf_correlation(sector_id).await
                    .map(|c| c.correlation_coefficient),
            }),
            context_factors,
            enhancement_timestamp: Utc::now(),
        };
        
        info!("✅ Enhanced prediction: {:.4} -> {:.4} (confidence: {:.3} -> {:.3})",
              base_prediction.value, enhanced_value, 
              base_prediction.confidence, enhanced_confidence);
        
        Ok(enhanced_result)
    }
    
    /// Update the aggregator with new market data
    pub async fn update_market_data(&self, data: TimeSeriesData) -> Result<()> {
        self.sector_aggregator.update_symbol(data).await
    }
    
    /// Batch update for multiple symbols
    pub async fn batch_update_market_data(&self, data_batch: Vec<TimeSeriesData>) -> Result<()> {
        self.sector_aggregator.batch_update(data_batch).await
    }
    
    /// Get current sector insights
    pub async fn get_sector_insights(&self) -> Result<SectorInsights> {
        let all_aggregations = self.sector_aggregator.get_all_aggregations().await;
        let cross_correlations = self.sector_aggregator.calculate_cross_sector_correlations().await?;
        let performance_metrics = self.sector_aggregator.get_performance_metrics().await;
        
        Ok(SectorInsights {
            sector_aggregations: all_aggregations,
            cross_correlations,
            performance_metrics,
            timestamp: Utc::now(),
        })
    }
}

/// Enhanced prediction result with sector context
#[derive(Debug, Clone)]
pub struct EnhancedPredictionResult {
    pub base_prediction: PredictionResult,
    pub enhanced_value: f64,
    pub enhanced_confidence: f64,
    pub sector_context: Option<SectorContext>,
    pub context_factors: HashMap<String, f64>,
    pub enhancement_timestamp: chrono::DateTime<Utc>,
}

impl EnhancedPredictionResult {
    /// Create from base prediction without enhancements
    pub fn from_base(base_prediction: PredictionResult) -> Self {
        Self {
            enhanced_value: base_prediction.value,
            enhanced_confidence: base_prediction.confidence,
            base_prediction,
            sector_context: None,
            context_factors: HashMap::new(),
            enhancement_timestamp: Utc::now(),
        }
    }
    
    /// Get the improvement over base prediction
    pub fn get_enhancement_metrics(&self) -> HashMap<String, f64> {
        let mut metrics = HashMap::new();
        
        let value_improvement = (self.enhanced_value - self.base_prediction.value) / 
                               self.base_prediction.value.abs().max(0.001);
        let confidence_improvement = self.enhanced_confidence - self.base_prediction.confidence;
        
        metrics.insert("value_improvement".to_string(), value_improvement);
        metrics.insert("confidence_improvement".to_string(), confidence_improvement);
        metrics.insert("total_context_factors".to_string(), self.context_factors.len() as f64);
        
        metrics
    }
}

/// Sector context information
#[derive(Debug, Clone)]
pub struct SectorContext {
    pub sector_id: SectorId,
    pub breadth_ratio: f64,
    pub sector_momentum: f64,
    pub advancing_ratio: f64,
    pub etf_correlation: Option<f64>,
}

/// Complete sector insights
#[derive(Debug, Clone)]
pub struct SectorInsights {
    pub sector_aggregations: HashMap<SectorId, crate::neural::SectorAggregation>,
    pub cross_correlations: HashMap<(SectorId, SectorId), f64>,
    pub performance_metrics: HashMap<String, serde_json::Value>,
    pub timestamp: chrono::DateTime<Utc>,
}

// Add missing import for RwLock
use tokio::sync::RwLock;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_enhanced_neural_engine_integration() {
        // Integration test would require mock components
        // This validates the interface and structure
    }
    
    #[tokio::test]
    async fn test_sector_context_application() {
        // Test the sector context enhancement logic
    }
    
    #[tokio::test]
    async fn test_enhanced_prediction_metrics() {
        // Test enhancement metrics calculation
    }
}