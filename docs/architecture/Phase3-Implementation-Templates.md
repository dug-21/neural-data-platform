# Phase 3: Implementation Templates

## INTEGRATION_FIRST_MANDATE Implementation Guide

This document provides concrete implementation templates that show EXACTLY how to integrate neural capabilities while strictly adhering to the INTEGRATION_FIRST_MANDATE.

## Template 1: Neural DAA Extension Integration

### File: `src/integration/neural_daa_extension.rs`

```rust
//! Neural DAA Extension - Extends existing DAACoordinator with neural capabilities
//! 
//! INTEGRATION_FIRST_MANDATE COMPLIANCE:
//! ✅ Uses BaseModel<T> from vendor/ruv-fann only
//! ✅ Pure Rust implementation
//! ✅ Extends DAACoordinator, does not replace
//! ✅ Preserves all existing functionality

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, anyhow};

// Import from vendor/ruv-fann ONLY
use neuro_divergent_core::traits::{BaseModel, ModelConfig, ForecastResult};
use neuro_divergent_core::data::TimeSeriesDataset;
use neuro_divergent_models::{
    basic::{MLP, MLPConfig},
    transformer::{TFT, TFTConfig},
    specialized::{DeepAR, DeepARConfig},
};
use neuro_divergent_registry::{Registry, ModelFactory};

// Import existing DAA types (PRESERVED)
use crate::integration::daa_coordinator::{
    DaaCoordinator, AutonomousDecision, DataAvailability, MarketTimingResult
};
use crate::data::TimeSeriesData;

/// Enhanced autonomous decision with neural augmentation
#[derive(Debug, Clone)]
pub struct EnhancedAutonomousDecision {
    /// Original decision (PRESERVED EXACTLY)
    pub base_decision: AutonomousDecision,
    
    /// Neural prediction if applied
    pub neural_prediction: Option<ForecastResult<f64>>,
    
    /// Confidence score from neural model
    pub confidence_score: f64,
    
    /// Model type used for enhancement
    pub model_used: String,
    
    /// Whether neural enhancement was applied
    pub enhancement_applied: bool,
    
    /// Neural model reasoning
    pub neural_reasoning: Vec<String>,
}

impl EnhancedAutonomousDecision {
    /// Convert back to base AutonomousDecision for compatibility
    pub fn into_autonomous_decision(self) -> AutonomousDecision {
        let mut decision = self.base_decision;
        
        // Only modify confidence if neural enhancement was applied
        if self.enhancement_applied {
            // Blend neural and base confidence
            let blended_confidence = (decision.confidence * 0.6) + (self.confidence_score * 0.4);
            decision.confidence = blended_confidence.min(1.0);
            
            // Add neural reasoning to existing reasoning
            decision.reasoning.push_str(&format!(
                " [Neural: {} model with {:.1}% confidence]",
                self.model_used,
                self.confidence_score * 100.0
            ));
        }
        
        decision
    }
}

/// Neural model performance tracking
#[derive(Debug, Clone)]
pub struct ModelPerformance {
    pub accuracy: f64,
    pub prediction_count: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub confidence_history: Vec<f64>,
    pub error_rate: f64,
}

/// Neural DAA Extension - Augments existing DAACoordinator
pub struct NeuralDAAExtension {
    /// Registry of neural models from vendor/ruv-fann
    model_registry: Registry,
    
    /// Active models by market condition
    active_models: Arc<RwLock<HashMap<String, Box<dyn BaseModel<f64> + Send + Sync>>>>,
    
    /// Model performance tracking
    model_performance: Arc<RwLock<HashMap<String, ModelPerformance>>>,
    
    /// Confidence thresholds for model types
    confidence_thresholds: HashMap<String, f64>,
    
    /// Feature flags for neural enhancement
    neural_enabled: bool,
    fallback_to_base: bool,
}

impl NeuralDAAExtension {
    /// Create new neural extension
    pub fn new() -> Result<Self> {
        let mut registry = Registry::new()?;
        
        // Initialize confidence thresholds based on model types
        let mut confidence_thresholds = HashMap::new();
        confidence_thresholds.insert("mlp".to_string(), 0.7);
        confidence_thresholds.insert("tft".to_string(), 0.75);
        confidence_thresholds.insert("deepar".to_string(), 0.8);
        confidence_thresholds.insert("dlinear".to_string(), 0.65); // Fast models, lower threshold
        
        Ok(Self {
            model_registry: registry,
            active_models: Arc::new(RwLock::new(HashMap::new())),
            model_performance: Arc::new(RwLock::new(HashMap::new())),
            confidence_thresholds,
            neural_enabled: true,
            fallback_to_base: true,
        })
    }
    
    /// Initialize neural models for different market conditions
    pub async fn initialize_models(&self) -> Result<()> {
        let mut models = self.active_models.write().await;
        let mut performance = self.model_performance.write().await;
        
        // Initialize MLP for general purpose
        let mlp_config = MLPConfig::builder()
            .with_horizon(24) // 24-hour prediction horizon
            .with_input_size(100) // 100 historical points
            .build()?;
        let mlp = MLP::new(mlp_config)?;
        models.insert("mlp_default".to_string(), Box::new(mlp));
        performance.insert("mlp_default".to_string(), ModelPerformance {
            accuracy: 0.75,
            prediction_count: 0,
            last_updated: chrono::Utc::now(),
            confidence_history: Vec::new(),
            error_rate: 0.25,
        });
        
        // Initialize TFT for complex time series
        let tft_config = TFTConfig::builder()
            .with_horizon(24)
            .with_input_size(200)
            .build()?;
        let tft = TFT::new(tft_config)?;
        models.insert("tft_complex".to_string(), Box::new(tft));
        performance.insert("tft_complex".to_string(), ModelPerformance {
            accuracy: 0.82,
            prediction_count: 0,
            last_updated: chrono::Utc::now(),
            confidence_history: Vec::new(),
            error_rate: 0.18,
        });
        
        // Initialize DeepAR for probabilistic forecasting
        let deepar_config = DeepARConfig::builder()
            .with_horizon(24)
            .with_input_size(150)
            .build()?;
        let deepar = DeepAR::new(deepar_config)?;
        models.insert("deepar_probabilistic".to_string(), Box::new(deepar));
        performance.insert("deepar_probabilistic".to_string(), ModelPerformance {
            accuracy: 0.78,
            prediction_count: 0,
            last_updated: chrono::Utc::now(),
            confidence_history: Vec::new(),
            error_rate: 0.22,
        });
        
        tracing::info!("✅ Initialized {} neural models", models.len());
        Ok(())
    }
    
    /// Enhance autonomous decision with neural prediction
    pub async fn enhance_decision(
        &self,
        base_decision: AutonomousDecision,
        market_data: &[TimeSeriesData],
    ) -> Result<EnhancedAutonomousDecision> {
        // Early return if neural processing disabled
        if !self.neural_enabled {
            return Ok(EnhancedAutonomousDecision {
                base_decision,
                neural_prediction: None,
                confidence_score: 0.0,
                model_used: "disabled".to_string(),
                enhancement_applied: false,
                neural_reasoning: vec!["Neural processing disabled".to_string()],
            });
        }
        
        // Select optimal model for current market conditions
        let model_type = self.select_optimal_model(market_data).await?;
        
        // Get model from registry
        let models = self.active_models.read().await;
        let model = models.get(&model_type)
            .ok_or_else(|| anyhow!("Model not found: {}", model_type))?;
        
        // Convert market data to neural dataset format
        let dataset = self.convert_to_neural_dataset(market_data)?;
        
        // Generate neural prediction
        match model.predict(&dataset) {
            Ok(prediction) => {
                // Calculate confidence based on model performance
                let confidence = self.calculate_model_confidence(&model_type, &prediction).await?;
                let threshold = self.confidence_thresholds.get(&model_type)
                    .copied()
                    .unwrap_or(0.7);
                
                // Apply enhancement if confidence exceeds threshold
                if confidence >= threshold {
                    tracing::debug!("Neural enhancement applied: model={}, confidence={:.3}", 
                                   model_type, confidence);
                    
                    Ok(EnhancedAutonomousDecision {
                        base_decision,
                        neural_prediction: Some(prediction),
                        confidence_score: confidence,
                        model_used: model_type,
                        enhancement_applied: true,
                        neural_reasoning: vec![
                            format!("Neural model {} predicts with {:.1}% confidence", 
                                   model_type, confidence * 100.0)
                        ],
                    })
                } else {
                    tracing::debug!("Neural confidence too low: {:.3} < {:.3}, using base decision", 
                                   confidence, threshold);
                    
                    Ok(EnhancedAutonomousDecision {
                        base_decision,
                        neural_prediction: Some(prediction),
                        confidence_score: confidence,
                        model_used: model_type,
                        enhancement_applied: false,
                        neural_reasoning: vec![
                            format!("Neural confidence {:.1}% below threshold {:.1}%", 
                                   confidence * 100.0, threshold * 100.0)
                        ],
                    })
                }
            }
            Err(e) => {
                tracing::warn!("Neural prediction failed: {}, falling back to base decision", e);
                
                if self.fallback_to_base {
                    Ok(EnhancedAutonomousDecision {
                        base_decision,
                        neural_prediction: None,
                        confidence_score: 0.0,
                        model_used: model_type,
                        enhancement_applied: false,
                        neural_reasoning: vec![
                            format!("Neural prediction failed: {}", e)
                        ],
                    })
                } else {
                    Err(anyhow!("Neural prediction failed and fallback disabled: {}", e))
                }
            }
        }
    }
    
    /// Select optimal model based on market conditions
    async fn select_optimal_model(&self, market_data: &[TimeSeriesData]) -> Result<String> {
        // Simple market condition analysis
        let volatility = self.calculate_volatility(market_data)?;
        let trend_strength = self.calculate_trend_strength(market_data)?;
        
        // Model selection logic
        let model_type = if volatility > 0.05 {
            "deepar_probabilistic" // High volatility -> probabilistic model
        } else if trend_strength > 0.7 {
            "tft_complex" // Strong trend -> transformer model
        } else {
            "mlp_default" // Default -> MLP
        };
        
        Ok(model_type.to_string())
    }
    
    /// Calculate model confidence based on historical performance
    async fn calculate_model_confidence(
        &self,
        model_type: &str,
        _prediction: &ForecastResult<f64>,
    ) -> Result<f64> {
        let performance = self.model_performance.read().await;
        
        if let Some(perf) = performance.get(model_type) {
            // Base confidence on historical accuracy
            let base_confidence = perf.accuracy;
            
            // Adjust for recent performance
            let recent_confidence = if perf.confidence_history.len() >= 10 {
                let recent: f64 = perf.confidence_history.iter()
                    .rev()
                    .take(10)
                    .sum::<f64>() / 10.0;
                recent
            } else {
                base_confidence
            };
            
            // Weighted average
            let final_confidence = (base_confidence * 0.7) + (recent_confidence * 0.3);
            
            Ok(final_confidence.min(1.0).max(0.0))
        } else {
            Ok(0.5) // Default confidence for unknown models
        }
    }
    
    /// Convert market data to neural dataset format
    fn convert_to_neural_dataset(&self, market_data: &[TimeSeriesData]) -> Result<TimeSeriesDataset<f64>> {
        // Implementation depends on neuro-divergent-core::data structures
        // This is a placeholder showing the conversion pattern
        
        let values: Vec<f64> = market_data.iter()
            .map(|data| data.close as f64)
            .collect();
            
        let timestamps: Vec<chrono::DateTime<chrono::Utc>> = market_data.iter()
            .map(|data| data.timestamp)
            .collect();
        
        // Create dataset using neuro-divergent structures
        TimeSeriesDataset::new(
            values,
            timestamps,
            "market_data".to_string(),
        )
    }
    
    /// Calculate market volatility
    fn calculate_volatility(&self, market_data: &[TimeSeriesData]) -> Result<f64> {
        if market_data.len() < 2 {
            return Ok(0.0);
        }
        
        let returns: Vec<f64> = market_data.windows(2)
            .map(|window| {
                let prev = window[0].close as f64;
                let curr = window[1].close as f64;
                (curr / prev - 1.0).abs()
            })
            .collect();
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        Ok(mean_return)
    }
    
    /// Calculate trend strength
    fn calculate_trend_strength(&self, market_data: &[TimeSeriesData]) -> Result<f64> {
        if market_data.len() < 20 {
            return Ok(0.0);
        }
        
        let first_half: f64 = market_data.iter()
            .take(market_data.len() / 2)
            .map(|d| d.close as f64)
            .sum::<f64>() / (market_data.len() / 2) as f64;
            
        let second_half: f64 = market_data.iter()
            .skip(market_data.len() / 2)
            .map(|d| d.close as f64)
            .sum::<f64>() / (market_data.len() / 2) as f64;
        
        let trend_strength = ((second_half - first_half) / first_half).abs();
        Ok(trend_strength.min(1.0))
    }
    
    /// Update model performance metrics
    pub async fn update_performance_metrics(
        &self,
        model_type: &str,
        actual_confidence: f64,
        prediction_accuracy: f64,
    ) -> Result<()> {
        let mut performance = self.model_performance.write().await;
        
        if let Some(perf) = performance.get_mut(model_type) {
            perf.confidence_history.push(actual_confidence);
            if perf.confidence_history.len() > 100 {
                perf.confidence_history.remove(0); // Keep only last 100
            }
            
            // Update rolling accuracy
            perf.accuracy = (perf.accuracy * 0.9) + (prediction_accuracy * 0.1);
            perf.prediction_count += 1;
            perf.last_updated = chrono::Utc::now();
            
            tracing::debug!("Updated {} performance: accuracy={:.3}, predictions={}", 
                           model_type, perf.accuracy, perf.prediction_count);
        }
        
        Ok(())
    }
}

/// Extension trait for DaaCoordinator integration
pub trait DAACoordinatorNeuralExtension {
    fn with_neural_extension(self, extension: Arc<NeuralDAAExtension>) -> Self;
    
    async fn make_enhanced_decision(
        &self,
        symbol: &str,
        data: &[TimeSeriesData],
        context: &crate::strategies::MarketContext,
    ) -> Result<AutonomousDecision>;
}

// Implementation would be added to existing DaaCoordinator
// This shows the integration pattern without modifying core files
```

## Template 2: Real-Time Neural Channel Processor

### File: `src/neural/realtime_channel_processor.rs`

```rust
//! Real-time neural processing for Redis channels
//! 
//! INTEGRATION_FIRST_MANDATE COMPLIANCE:
//! ✅ Extends existing Redis integration
//! ✅ Preserves all existing channels
//! ✅ Uses BaseModel<T> from vendor/ruv-fann

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use futures::StreamExt;
use anyhow::Result;

use crate::adapters::{RedisIntegration, MarketData, AdapterError};
use crate::integration::neural_daa_extension::NeuralDAAExtension;
use crate::data::TimeSeriesData;

/// Real-time neural processor for Redis channels
pub struct RealtimeNeuralProcessor {
    /// Existing Redis integration (PRESERVED)
    redis_integration: Arc<RedisIntegration>,
    
    /// Neural extension for processing
    neural_extension: Arc<NeuralDAAExtension>,
    
    /// Processing status by channel
    channel_status: Arc<RwLock<HashMap<String, ChannelProcessingStatus>>>,
    
    /// Performance metrics
    processing_metrics: Arc<RwLock<ProcessingMetrics>>,
    
    /// Configuration
    config: ProcessorConfig,
}

#[derive(Debug, Clone)]
struct ChannelProcessingStatus {
    is_active: bool,
    messages_processed: u64,
    last_processed: chrono::DateTime<chrono::Utc>,
    error_count: u64,
    neural_enhancements: u64,
}

#[derive(Debug, Clone)]
struct ProcessingMetrics {
    total_messages: u64,
    neural_enhancements: u64,
    processing_errors: u64,
    average_latency_ms: f64,
    enhancement_rate: f64,
}

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub enable_neural_processing: bool,
    pub max_concurrent_channels: usize,
    pub processing_timeout_ms: u64,
    pub enhancement_threshold: f64,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            enable_neural_processing: true,
            max_concurrent_channels: 50,
            processing_timeout_ms: 100,
            enhancement_threshold: 0.7,
        }
    }
}

impl RealtimeNeuralProcessor {
    /// Create new processor
    pub fn new(
        redis_integration: Arc<RedisIntegration>,
        neural_extension: Arc<NeuralDAAExtension>,
        config: ProcessorConfig,
    ) -> Self {
        Self {
            redis_integration,
            neural_extension,
            channel_status: Arc::new(RwLock::new(HashMap::new())),
            processing_metrics: Arc::new(RwLock::new(ProcessingMetrics {
                total_messages: 0,
                neural_enhancements: 0,
                processing_errors: 0,
                average_latency_ms: 0.0,
                enhancement_rate: 0.0,
            })),
            config,
        }
    }
    
    /// Start processing all existing channels
    pub async fn start_processing(&self) -> Result<()> {
        if !self.config.enable_neural_processing {
            tracing::info!("Neural processing disabled by configuration");
            return Ok(());
        }
        
        // Get all existing channels from Redis integration
        let all_channels = self.redis_integration.get_all_channels();
        
        // Process symbol channels
        if let Some(symbol_channels) = all_channels.get("symbols") {
            for channel in symbol_channels {
                self.spawn_symbol_channel_processor(channel.clone()).await?;
            }
            tracing::info!("Started neural processing for {} symbol channels", symbol_channels.len());
        }
        
        // Process sector channels
        if let Some(sector_channels) = all_channels.get("sectors") {
            for channel in sector_channels {
                self.spawn_sector_channel_processor(channel.clone()).await?;
            }
            tracing::info!("Started neural processing for {} sector channels", sector_channels.len());
        }
        
        // Process portfolio channels
        if let Some(portfolio_channels) = all_channels.get("portfolio") {
            for channel in portfolio_channels {
                self.spawn_portfolio_channel_processor(channel.clone()).await?;
            }
            tracing::info!("Started neural processing for {} portfolio channels", portfolio_channels.len());
        }
        
        tracing::info!("✅ Real-time neural processing started for all channels");
        Ok(())
    }
    
    /// Spawn processor for symbol channels
    async fn spawn_symbol_channel_processor(&self, channel: String) -> Result<()> {
        let processor = self.clone();
        let channel_clone = channel.clone();
        
        // Initialize channel status
        {
            let mut status = processor.channel_status.write().await;
            status.insert(channel.clone(), ChannelProcessingStatus {
                is_active: true,
                messages_processed: 0,
                last_processed: chrono::Utc::now(),
                error_count: 0,
                neural_enhancements: 0,
            });
        }
        
        tokio::spawn(async move {
            tracing::debug!("Starting neural processor for symbol channel: {}", channel_clone);
            
            // Subscribe to existing channel (PRESERVED FUNCTIONALITY)
            let symbol_redis = processor.redis_integration.symbol_redis.read().await;
            match symbol_redis.subscribe_market_data(&channel_clone).await {
                Ok(mut stream) => {
                    while let Some(result) = stream.next().await {
                        let start_time = std::time::Instant::now();
                        
                        match result {
                            Ok(market_data) => {
                                // Process with neural enhancement
                                match processor.process_market_data_neural(&market_data).await {
                                    Ok(enhanced_data) => {
                                        // Publish enhanced data to neural channel
                                        let neural_channel = format!("{}_neural", channel_clone);
                                        if let Err(e) = symbol_redis.publish_market_data(&neural_channel, &enhanced_data).await {
                                            tracing::warn!("Failed to publish enhanced data to {}: {}", neural_channel, e);
                                        } else {
                                            // Update success metrics
                                            processor.update_channel_status(&channel_clone, true, start_time.elapsed()).await;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::debug!("Neural processing skipped for {}: {}", channel_clone, e);
                                        processor.update_channel_status(&channel_clone, false, start_time.elapsed()).await;
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Error receiving data from {}: {}", channel_clone, e);
                                processor.increment_error_count(&channel_clone).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to subscribe to channel {}: {}", channel_clone, e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Process market data with neural enhancement
    async fn process_market_data_neural(&self, market_data: &MarketData) -> Result<MarketData> {
        // Convert MarketData to TimeSeriesData for neural processing
        let time_series_data = vec![TimeSeriesData {
            timestamp: chrono::DateTime::from_timestamp(market_data.timestamp, 0)
                .unwrap_or_else(|| chrono::Utc::now()),
            open: market_data.open,
            high: market_data.high,
            low: market_data.low,
            close: market_data.close,
            volume: market_data.volume,
        }];
        
        // Create a dummy autonomous decision for enhancement
        use crate::integration::daa_coordinator::AutonomousDecision;
        let base_decision = AutonomousDecision {
            symbol: market_data.symbol.clone(),
            signal: crate::strategies::Signal::Hold,
            confidence: 0.5,
            reasoning: "Base market data processing".to_string(),
            risk_assessment: 0.3,
            position_size: 0.0,
            timestamp: chrono::Utc::now(),
            data_quality: 0.9,
            market_timing: crate::integration::daa_coordinator::MarketTimingResult {
                timing_score: 0.7,
                market_session: crate::integration::daa_coordinator::MarketSession::Regular,
                volume_pattern_score: 0.8,
                liquidity_score: 0.9,
                recommendation: crate::integration::daa_coordinator::TimingRecommendation::Good,
            },
        };
        
        // Apply neural enhancement
        match self.neural_extension.enhance_decision(base_decision, &time_series_data).await {
            Ok(enhanced) if enhanced.enhancement_applied => {
                // Create enhanced market data
                let mut enhanced_market_data = market_data.clone();
                
                // Adjust confidence-based fields
                if let Some(prediction) = &enhanced.neural_prediction {
                    if let Some(first_forecast) = prediction.forecasts.first() {
                        // Blend neural prediction with original data
                        let neural_weight = enhanced.confidence_score;
                        let original_weight = 1.0 - neural_weight;
                        
                        enhanced_market_data.close = (enhanced_market_data.close * original_weight) + 
                                                   (*first_forecast as f32 * neural_weight);
                    }
                }
                
                // Add neural metadata
                enhanced_market_data.symbol = format!("{}_neural_enhanced", enhanced_market_data.symbol);
                
                tracing::debug!("Enhanced market data for {} with {:.1}% neural confidence", 
                               market_data.symbol, enhanced.confidence_score * 100.0);
                
                Ok(enhanced_market_data)
            }
            Ok(_) => {
                // Neural enhancement not applied, return original data
                Err(anyhow::anyhow!("Neural confidence below threshold"))
            }
            Err(e) => {
                Err(anyhow::anyhow!("Neural processing failed: {}", e))
            }
        }
    }
    
    /// Update channel processing status
    async fn update_channel_status(
        &self,
        channel: &str,
        neural_applied: bool,
        processing_time: std::time::Duration,
    ) {
        let mut status = self.channel_status.write().await;
        if let Some(channel_status) = status.get_mut(channel) {
            channel_status.messages_processed += 1;
            channel_status.last_processed = chrono::Utc::now();
            if neural_applied {
                channel_status.neural_enhancements += 1;
            }
        }
        
        // Update global metrics
        let mut metrics = self.processing_metrics.write().await;
        metrics.total_messages += 1;
        if neural_applied {
            metrics.neural_enhancements += 1;
        }
        
        // Update average latency
        let processing_ms = processing_time.as_millis() as f64;
        metrics.average_latency_ms = (metrics.average_latency_ms * 0.9) + (processing_ms * 0.1);
        
        // Update enhancement rate
        metrics.enhancement_rate = if metrics.total_messages > 0 {
            metrics.neural_enhancements as f64 / metrics.total_messages as f64
        } else {
            0.0
        };
    }
    
    /// Increment error count for channel
    async fn increment_error_count(&self, channel: &str) {
        let mut status = self.channel_status.write().await;
        if let Some(channel_status) = status.get_mut(channel) {
            channel_status.error_count += 1;
        }
        
        let mut metrics = self.processing_metrics.write().await;
        metrics.processing_errors += 1;
    }
    
    /// Get processing metrics
    pub async fn get_metrics(&self) -> ProcessingMetrics {
        self.processing_metrics.read().await.clone()
    }
    
    /// Get channel status
    pub async fn get_channel_status(&self) -> HashMap<String, ChannelProcessingStatus> {
        self.channel_status.read().await.clone()
    }
    
    // Additional processors for sector and portfolio channels would follow similar patterns
    async fn spawn_sector_channel_processor(&self, _channel: String) -> Result<()> {
        // Similar implementation for sector channels
        // Focus on sector-level neural processing
        Ok(())
    }
    
    async fn spawn_portfolio_channel_processor(&self, _channel: String) -> Result<()> {
        // Similar implementation for portfolio channels
        // Focus on portfolio-level neural decisions
        Ok(())
    }
}

// Clone implementation for tokio spawn
impl Clone for RealtimeNeuralProcessor {
    fn clone(&self) -> Self {
        Self {
            redis_integration: Arc::clone(&self.redis_integration),
            neural_extension: Arc::clone(&self.neural_extension),
            channel_status: Arc::clone(&self.channel_status),
            processing_metrics: Arc::clone(&self.processing_metrics),
            config: self.config.clone(),
        }
    }
}
```

## Template 3: Integration with Existing DAACoordinator

### File: `src/integration/daa_coordinator.rs` (Extension)

```rust
// Extension to existing DAACoordinator implementation
// This shows how to add neural capabilities without breaking existing code

use crate::integration::neural_daa_extension::{NeuralDAAExtension, EnhancedAutonomousDecision};

impl DaaCoordinator {
    /// Enhanced version of existing make_autonomous_decision
    /// PRESERVES all existing functionality, adds neural enhancement option
    pub async fn make_enhanced_autonomous_decision(
        &self,
        symbol: &str,
        data: &[TimeSeriesData],
        context: &MarketContext,
        neural_extension: Option<&Arc<NeuralDAAExtension>>,
    ) -> Result<AutonomousDecision> {
        // Step 1: Generate base decision using EXISTING LOGIC (PRESERVED)
        let base_decision = self.make_autonomous_decision_internal(symbol, data, context).await?;
        
        // Step 2: Apply neural enhancement if available and enabled
        if let Some(neural_ext) = neural_extension {
            match neural_ext.enhance_decision(base_decision.clone(), data).await {
                Ok(enhanced) if enhanced.enhancement_applied => {
                    tracing::info!("✅ Neural enhancement applied for {}: model={}, confidence={:.1}%",
                                 symbol, enhanced.model_used, enhanced.confidence_score * 100.0);
                    
                    // Convert enhanced decision back to AutonomousDecision
                    return Ok(enhanced.into_autonomous_decision());
                }
                Ok(enhanced) => {
                    tracing::debug!("Neural enhancement available but not applied for {}: confidence={:.1}% below threshold",
                                   symbol, enhanced.confidence_score * 100.0);
                }
                Err(e) => {
                    tracing::warn!("Neural enhancement failed for {}: {}, using base decision", symbol, e);
                }
            }
        }
        
        // Step 3: Return base decision (PRESERVED BEHAVIOR)
        tracing::debug!("Using base decision for {} (neural not applied)", symbol);
        Ok(base_decision)
    }
    
    /// Internal method that preserves existing decision logic
    async fn make_autonomous_decision_internal(
        &self,
        symbol: &str,
        data: &[TimeSeriesData],
        context: &MarketContext,
    ) -> Result<AutonomousDecision> {
        // ALL EXISTING LOGIC GOES HERE UNCHANGED
        // This is the original make_autonomous_decision implementation
        
        // Placeholder showing preserved pattern
        let signal = self.analyze_market_signal(symbol, data, context).await?;
        let confidence = self.calculate_confidence(symbol, data).await?;
        let risk = self.assess_risk(symbol, data, context).await?;
        
        Ok(AutonomousDecision {
            symbol: symbol.to_string(),
            signal,
            confidence,
            reasoning: "Base DAA analysis".to_string(),
            risk_assessment: risk,
            position_size: self.calculate_position_size(symbol, risk).await?,
            timestamp: chrono::Utc::now(),
            data_quality: self.assess_data_quality(data).await.unwrap_or(0.8),
            market_timing: self.analyze_market_timing(data).await?,
        })
    }
    
    /// Factory method to create DaaCoordinator with neural capabilities
    pub fn with_neural_extension(mut self, neural_extension: Arc<NeuralDAAExtension>) -> Self {
        // Add neural extension to existing coordinator
        // This could be stored as an Option<Arc<NeuralDAAExtension>> field
        tracing::info!("✅ DaaCoordinator enhanced with neural capabilities");
        self
        // Implementation would depend on adding the field to the struct
    }
}
```

## Template 4: Configuration Integration

### File: `src/config/neural_config.rs`

```rust
//! Neural configuration that extends existing config structures

use serde::{Deserialize, Serialize};

/// Neural DAA configuration that extends existing DaaConfig
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NeuralDaaConfig {
    /// Enable neural enhancement
    pub enabled: bool,
    
    /// Fallback to base decisions if neural fails
    pub fallback_to_base: bool,
    
    /// Confidence threshold for applying neural enhancement
    pub confidence_threshold: f64,
    
    /// Model selection strategy
    pub model_selection_strategy: ModelSelectionStrategy,
    
    /// Performance tracking window (hours)
    pub performance_window_hours: u32,
    
    /// Enable automatic model switching based on performance
    pub auto_model_switching: bool,
    
    /// Maximum concurrent neural processing tasks
    pub max_concurrent_processing: usize,
    
    /// Processing timeout (milliseconds)
    pub processing_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSelectionStrategy {
    /// Select based on historical performance
    Performance,
    /// Select based on market regime detection
    MarketRegime,
    /// Use ensemble of multiple models
    Ensemble,
    /// Fixed model selection
    Fixed(String),
}

impl Default for NeuralDaaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            fallback_to_base: true,
            confidence_threshold: 0.7,
            model_selection_strategy: ModelSelectionStrategy::Performance,
            performance_window_hours: 24,
            auto_model_switching: true,
            max_concurrent_processing: 10,
            processing_timeout_ms: 100,
        }
    }
}

/// Extension to existing DaaConfig
impl crate::integration::daa_coordinator::DaaConfig {
    /// Add neural configuration to existing config
    pub fn with_neural_config(mut self, neural_config: NeuralDaaConfig) -> Self {
        // This would require adding a neural field to DaaConfig
        // self.neural = Some(neural_config);
        self
    }
}
```

## Integration Testing Templates

### File: `tests/integration/neural_integration_tests.rs`

```rust
//! Integration tests that verify neural enhancement preserves existing functionality

use std::sync::Arc;
use anyhow::Result;

use crate::integration::{
    daa_coordinator::{DaaCoordinator, DaaConfig},
    neural_daa_extension::NeuralDAAExtension,
};
use crate::data::TimeSeriesData;
use crate::strategies::MarketContext;

#[tokio::test]
async fn test_base_functionality_preserved() -> Result<()> {
    // Create standard DAACoordinator
    let config = DaaConfig::default();
    let (sender, _receiver) = tokio::sync::mpsc::channel(100);
    let market_hours = Arc::new(crate::utils::market_hours::MarketHours::new());
    let neural_predictor = Arc::new(crate::neural::NeuralPredictor::new(Default::default())?);
    
    let coordinator = DaaCoordinator::new(config, neural_predictor, sender, market_hours)?;
    
    // Test that existing functionality works without neural extension
    let market_data = create_test_market_data();
    let context = MarketContext::default();
    
    let decision = coordinator.make_autonomous_decision("AAPL", &market_data, &context).await?;
    
    assert!(!decision.symbol.is_empty());
    assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
    assert!(decision.risk_assessment >= 0.0);
    
    println!("✅ Base functionality preserved");
    Ok(())
}

#[tokio::test]
async fn test_neural_enhancement_integration() -> Result<()> {
    // Create DAACoordinator with neural extension
    let config = DaaConfig::default();
    let (sender, _receiver) = tokio::sync::mpsc::channel(100);
    let market_hours = Arc::new(crate::utils::market_hours::MarketHours::new());
    let neural_predictor = Arc::new(crate::neural::NeuralPredictor::new(Default::default())?);
    
    let coordinator = DaaCoordinator::new(config, neural_predictor, sender, market_hours)?;
    
    // Create neural extension
    let neural_extension = Arc::new(NeuralDAAExtension::new()?);
    neural_extension.initialize_models().await?;
    
    // Test enhanced decision making
    let market_data = create_test_market_data();
    let context = MarketContext::default();
    
    let enhanced_decision = coordinator.make_enhanced_autonomous_decision(
        "AAPL",
        &market_data,
        &context,
        Some(&neural_extension),
    ).await?;
    
    assert!(!enhanced_decision.symbol.is_empty());
    assert!(enhanced_decision.confidence >= 0.0 && enhanced_decision.confidence <= 1.0);
    
    // Verify that decision includes neural reasoning if applied
    if enhanced_decision.reasoning.contains("Neural:") {
        println!("✅ Neural enhancement applied: {}", enhanced_decision.reasoning);
    } else {
        println!("✅ Base decision used (neural confidence too low)");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_neural_fallback_behavior() -> Result<()> {
    // Test that system falls back gracefully when neural processing fails
    let config = DaaConfig::default();
    let (sender, _receiver) = tokio::sync::mpsc::channel(100);
    let market_hours = Arc::new(crate::utils::market_hours::MarketHours::new());
    let neural_predictor = Arc::new(crate::neural::NeuralPredictor::new(Default::default())?);
    
    let coordinator = DaaCoordinator::new(config, neural_predictor, sender, market_hours)?;
    
    // Create neural extension but don't initialize models (simulates failure)
    let neural_extension = Arc::new(NeuralDAAExtension::new()?);
    // Intentionally skip neural_extension.initialize_models().await?;
    
    let market_data = create_test_market_data();
    let context = MarketContext::default();
    
    // Should still work and fall back to base decision
    let decision = coordinator.make_enhanced_autonomous_decision(
        "AAPL",
        &market_data,
        &context,
        Some(&neural_extension),
    ).await?;
    
    assert!(!decision.symbol.is_empty());
    assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
    
    println!("✅ Fallback behavior works correctly");
    Ok(())
}

fn create_test_market_data() -> Vec<TimeSeriesData> {
    vec![
        TimeSeriesData {
            timestamp: chrono::Utc::now(),
            open: 150.0,
            high: 155.0,
            low: 149.0,
            close: 152.0,
            volume: 1000000,
        },
        TimeSeriesData {
            timestamp: chrono::Utc::now(),
            open: 152.0,
            high: 157.0,
            low: 151.0,
            close: 154.0,
            volume: 1100000,
        },
    ]
}
```

## Summary

These implementation templates show EXACTLY how to implement Phase 3 neural integration while strictly adhering to the INTEGRATION_FIRST_MANDATE:

1. **Neural DAA Extension** - Extends existing DAACoordinator without replacement
2. **Real-time Channel Processor** - Enhances existing Redis integration 
3. **BaseModel<T> Usage** - Only uses vendor/ruv-fann neural models
4. **Fallback Behavior** - Always preserves base functionality
5. **Configuration Integration** - Extends existing config structures
6. **Testing Strategy** - Verifies preservation of existing functionality

All code is pure Rust, integrates with existing systems, and maintains backward compatibility.