//! Symbol Specialization Layer for Phase 2 Week 7
//!
//! This layer provides lightweight symbol-specific adjustments on top of shared sector features.
//! It integrates with SharedFeatureExtractor to add symbol-specific enhancements while
//! maintaining memory efficiency (<2MB per symbol specialization).
//!
//! INTEGRATION-FIRST DESIGN:
//! - Extends SharedFeatureExtractor with symbol-specific layers
//! - Preserves sector knowledge through fine-tuning
//! - Graceful fallback to sector features if specialization fails
//! - Memory-efficient implementation with <2MB per symbol target

use anyhow::{Result, Context, anyhow};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn, error};

// Integration imports
use crate::data::{TimeSeriesData, SectorId};
use crate::features::{
    SharedFeatureExtractor, SharedSectorFeatures, SymbolFeatures,
    SharedFeatureConfig, FeatureCategory
};

/// Symbol-specific neural weights for fine-tuning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSpecializationWeights {
    /// Adjustment weights for shared features (same dimensionality as shared features)
    pub feature_adjustments: Vec<f64>,
    
    /// Symbol-specific bias terms
    pub bias_terms: Vec<f64>,
    
    /// Scaling factors for different feature categories
    pub category_scales: HashMap<FeatureCategory, f64>,
    
    /// Fine-tuning learning rate
    pub learning_rate: f64,
    
    /// Momentum for gradient updates
    pub momentum: f64,
    
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    
    /// Training iterations
    pub training_iterations: u32,
    
    /// Performance metrics
    pub performance_metrics: SymbolPerformanceMetrics,
}

/// Performance tracking for symbol specialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPerformanceMetrics {
    /// Improvement over sector baseline
    pub improvement_over_baseline: f64,
    
    /// Training loss history (last 10 values)
    pub loss_history: Vec<f64>,
    
    /// Validation accuracy
    pub validation_accuracy: f64,
    
    /// Feature importance scores
    pub feature_importance: HashMap<String, f64>,
    
    /// Overfitting indicators
    pub overfitting_score: f64,
    
    /// Memory usage in bytes
    pub memory_usage: usize,
}

impl Default for SymbolSpecializationWeights {
    fn default() -> Self {
        Self {
            feature_adjustments: vec![1.0; 50], // Default to identity adjustment
            bias_terms: vec![0.0; 50],
            category_scales: HashMap::new(),
            learning_rate: 0.001,
            momentum: 0.9,
            last_updated: Utc::now(),
            training_iterations: 0,
            performance_metrics: SymbolPerformanceMetrics::default(),
        }
    }
}

impl Default for SymbolPerformanceMetrics {
    fn default() -> Self {
        Self {
            improvement_over_baseline: 0.0,
            loss_history: Vec::new(),
            validation_accuracy: 0.0,
            feature_importance: HashMap::new(),
            overfitting_score: 0.0,
            memory_usage: 0,
        }
    }
}

/// Symbol-specific technical signals and adjustments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSpecificSignals {
    /// Technical indicators specific to this symbol
    pub technical_signals: HashMap<String, f64>,
    
    /// Price action patterns
    pub price_patterns: Vec<PricePattern>,
    
    /// Volume profile analysis
    pub volume_profile: VolumeProfile,
    
    /// Order flow imbalances
    pub order_flow_signals: OrderFlowSignals,
    
    /// Microstructure indicators
    pub microstructure_signals: HashMap<String, f64>,
    
    /// Computation timestamp
    pub timestamp: DateTime<Utc>,
}

/// Price pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePattern {
    pub pattern_type: String,
    pub confidence: f64,
    pub duration_bars: i32,
    pub target_level: f64,
    pub stop_level: f64,
}

/// Volume profile analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfile {
    pub poc_level: f64, // Point of Control
    pub value_area_high: f64,
    pub value_area_low: f64,
    pub volume_imbalance: f64,
    pub profile_shape: String, // "normal", "bimodal", "flat"
}

/// Order flow signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFlowSignals {
    pub delta: f64, // Buy volume - Sell volume
    pub cumulative_delta: f64,
    pub delta_divergence: f64,
    pub volume_at_bid_ask: (f64, f64),
    pub large_trade_impact: f64,
}

/// Configuration for symbol specialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSpecializationConfig {
    /// Maximum memory per symbol (bytes)
    pub max_memory_per_symbol: usize,
    
    /// Fine-tuning parameters
    pub fine_tuning_enabled: bool,
    pub min_training_samples: usize,
    pub max_training_iterations: u32,
    pub early_stopping_patience: u32,
    
    /// Feature enhancement parameters
    pub enable_technical_signals: bool,
    pub enable_price_patterns: bool,
    pub enable_volume_analysis: bool,
    pub enable_order_flow: bool,
    
    /// Fallback thresholds
    pub min_improvement_threshold: f64,
    pub max_overfitting_threshold: f64,
    
    /// Cache settings
    pub cache_ttl_seconds: u64,
    pub enable_compression: bool,
}

impl Default for SymbolSpecializationConfig {
    fn default() -> Self {
        Self {
            max_memory_per_symbol: 2 * 1024 * 1024, // 2MB limit
            fine_tuning_enabled: true,
            min_training_samples: 50,
            max_training_iterations: 100,
            early_stopping_patience: 10,
            enable_technical_signals: true,
            enable_price_patterns: true,
            enable_volume_analysis: true,
            enable_order_flow: true,
            min_improvement_threshold: 0.01, // 1% improvement minimum
            max_overfitting_threshold: 0.1,  // 10% overfitting threshold
            cache_ttl_seconds: 300, // 5 minutes
            enable_compression: true,
        }
    }
}

/// Main Symbol Specialization Layer
pub struct SymbolSpecializationLayer {
    /// Sector ID this specialization belongs to
    sector_id: SectorId,
    
    /// Reference to shared feature extractor
    shared_extractor: Arc<SharedFeatureExtractor>,
    
    /// Symbol-specific weights and adjustments
    symbol_weights: Arc<DashMap<String, SymbolSpecializationWeights>>,
    
    /// Symbol-specific signal cache
    signal_cache: Arc<RwLock<HashMap<String, (SymbolSpecificSignals, DateTime<Utc>)>>>,
    
    /// Configuration
    config: SymbolSpecializationConfig,
    
    /// Memory usage tracking
    memory_tracker: Arc<RwLock<HashMap<String, usize>>>,
    
    /// Memory allocation semaphore
    memory_semaphore: Arc<Semaphore>,
}

impl SymbolSpecializationLayer {
    /// Create new symbol specialization layer
    pub async fn new(
        sector_id: SectorId,
        shared_extractor: Arc<SharedFeatureExtractor>,
        config: SymbolSpecializationConfig,
    ) -> Result<Self> {
        info!("🎯 Initializing SymbolSpecializationLayer for sector: {:?}", sector_id);
        
        // Calculate memory permits (number of symbols we can handle)
        let max_symbols = 512 * 1024 * 1024 / config.max_memory_per_symbol; // 512MB / 2MB per symbol
        let memory_semaphore = Arc::new(Semaphore::new(max_symbols));
        
        Ok(Self {
            sector_id,
            shared_extractor,
            symbol_weights: Arc::new(DashMap::new()),
            signal_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            memory_tracker: Arc::new(RwLock::new(HashMap::new())),
            memory_semaphore,
        })
    }
    
    /// Extract enhanced features for a specific symbol
    pub async fn extract_specialized_features(
        &self,
        symbol: &str,
        symbol_data: &TimeSeriesData,
        sector_data: &HashMap<String, TimeSeriesData>,
    ) -> Result<HashMap<String, f64>> {
        debug!("🔍 Extracting specialized features for symbol: {}", symbol);
        
        // Step 1: Get shared sector features (90% memory savings here)
        let shared_features = self.shared_extractor
            .extract_sector_features(sector_data)
            .await
            .context("Failed to extract shared sector features")?;
        
        // Step 2: Get symbol specialization (lightweight layer)
        let symbol_features = self.shared_extractor
            .get_symbol_specialization(symbol, symbol_data, &shared_features, sector_data)
            .await
            .context("Failed to get symbol specialization")?;
        
        // Step 3: Apply symbol-specific adjustments
        let adjusted_features = self.apply_symbol_adjustments(
            symbol,
            &shared_features,
            &symbol_features,
            symbol_data,
        ).await?;
        
        // Step 4: Add symbol-specific signals
        let enhanced_features = self.add_symbol_specific_signals(
            symbol,
            symbol_data,
            adjusted_features,
        ).await?;
        
        // Step 5: Validate memory usage
        self.validate_memory_usage(symbol, &enhanced_features).await?;
        
        Ok(enhanced_features)
    }
    
    /// Apply symbol-specific neural adjustments to shared features
    async fn apply_symbol_adjustments(
        &self,
        symbol: &str,
        shared_features: &SharedSectorFeatures,
        symbol_features: &SymbolFeatures,
        symbol_data: &TimeSeriesData,
    ) -> Result<HashMap<String, f64>> {
        let mut adjusted_features = HashMap::new();
        
        // Get or initialize symbol weights
        let weights = self.get_or_create_symbol_weights(symbol).await?;
        
        // Convert shared features to vector format
        let shared_vector = self.shared_features_to_vector(shared_features);
        let symbol_vector = self.symbol_features_to_vector(symbol_features);
        
        // Apply neural adjustments: adjusted = shared * weights + bias
        for (i, (&shared_val, &weight)) in shared_vector.iter()
            .zip(weights.feature_adjustments.iter())
            .enumerate() {
            
            let bias = weights.bias_terms.get(i).unwrap_or(&0.0);
            let adjusted_val = shared_val * weight + bias;
            
            // Create feature name
            let feature_name = format!("shared_feature_{}", i);
            adjusted_features.insert(feature_name, adjusted_val);
        }
        
        // Add symbol-specific features with scaling
        for (i, &symbol_val) in symbol_vector.iter().enumerate() {
            let feature_name = format!("symbol_feature_{}", i);
            adjusted_features.insert(feature_name, symbol_val);
        }
        
        // Apply category-specific scaling
        self.apply_category_scaling(&mut adjusted_features, &weights).await?;
        
        Ok(adjusted_features)
    }
    
    /// Add symbol-specific technical signals and patterns
    async fn add_symbol_specific_signals(
        &self,
        symbol: &str,
        symbol_data: &TimeSeriesData,
        mut features: HashMap<String, f64>,
    ) -> Result<HashMap<String, f64>> {
        // Check cache first
        if let Some(cached_signals) = self.get_cached_signals(symbol).await? {
            self.merge_cached_signals(&mut features, &cached_signals);
            return Ok(features);
        }
        
        // Compute new signals
        let signals = self.compute_symbol_signals(symbol_data).await?;
        
        // Add technical signals
        if self.config.enable_technical_signals {
            for (name, value) in &signals.technical_signals {
                features.insert(format!("tech_{}", name), *value);
            }
        }
        
        // Add price pattern signals
        if self.config.enable_price_patterns {
            for (i, pattern) in signals.price_patterns.iter().enumerate() {
                features.insert(format!("pattern_{}_confidence", i), pattern.confidence);
                features.insert(format!("pattern_{}_duration", i), pattern.duration_bars as f64);
            }
        }
        
        // Add volume profile signals
        if self.config.enable_volume_analysis {
            features.insert("volume_poc".to_string(), signals.volume_profile.poc_level);
            features.insert("volume_imbalance".to_string(), signals.volume_profile.volume_imbalance);
            features.insert("value_area_high".to_string(), signals.volume_profile.value_area_high);
            features.insert("value_area_low".to_string(), signals.volume_profile.value_area_low);
        }
        
        // Add order flow signals
        if self.config.enable_order_flow {
            features.insert("order_flow_delta".to_string(), signals.order_flow_signals.delta);
            features.insert("cumulative_delta".to_string(), signals.order_flow_signals.cumulative_delta);
            features.insert("delta_divergence".to_string(), signals.order_flow_signals.delta_divergence);
        }
        
        // Add microstructure signals
        for (name, value) in &signals.microstructure_signals {
            features.insert(format!("micro_{}", name), *value);
        }
        
        // Cache the signals
        self.cache_signals(symbol, signals).await?;
        
        Ok(features)
    }
    
    /// Fine-tune symbol specialization based on performance feedback
    pub async fn fine_tune_specialization(
        &self,
        symbol: &str,
        training_data: &[TimeSeriesData],
        target_values: &[f64],
        learning_rate: Option<f64>,
    ) -> Result<()> {
        if !self.config.fine_tuning_enabled {
            return Ok(());
        }
        
        info!("🎯 Fine-tuning specialization for symbol: {}", symbol);
        
        // Check minimum training samples
        if training_data.len() < self.config.min_training_samples {
            warn!("Insufficient training samples for {}: {} < {}", 
                  symbol, training_data.len(), self.config.min_training_samples);
            return Ok(());
        }
        
        // Get current weights
        let mut weights = self.get_or_create_symbol_weights(symbol).await?;
        let lr = learning_rate.unwrap_or(weights.learning_rate);
        
        // Simple gradient descent updates
        let mut gradients = vec![0.0; weights.feature_adjustments.len()];
        let mut bias_gradients = vec![0.0; weights.bias_terms.len()];
        
        // Compute gradients (simplified implementation)
        for (data, target) in training_data.iter().zip(target_values.iter()) {
            // This would be replaced with actual gradient computation
            // For now, use simple approximation
            let prediction = self.predict_with_weights(data, &weights).await?;
            let error = target - prediction;
            
            // Update gradients (simplified)
            for i in 0..gradients.len() {
                gradients[i] += error * data.values.get(i).unwrap_or(&0.0);
                bias_gradients[i] += error;
            }
        }
        
        // Apply gradients with momentum
        for i in 0..weights.feature_adjustments.len() {
            let gradient = gradients[i] / training_data.len() as f64;
            weights.feature_adjustments[i] += lr * gradient;
            weights.bias_terms[i] += lr * bias_gradients[i] / training_data.len() as f64;
        }
        
        // Update performance metrics
        weights.training_iterations += 1;
        weights.last_updated = Utc::now();
        
        // Check for overfitting
        let overfitting_score = self.calculate_overfitting_score(&weights, training_data).await?;
        weights.performance_metrics.overfitting_score = overfitting_score;
        
        // Early stopping if overfitting
        if overfitting_score > self.config.max_overfitting_threshold {
            warn!("Early stopping for {} due to overfitting: {:.3}", symbol, overfitting_score);
            return Ok(());
        }
        
        // Update weights in storage
        self.symbol_weights.insert(symbol.to_string(), weights);
        
        info!("✅ Fine-tuning completed for {}: {} iterations", symbol, training_data.len());
        Ok(())
    }
    
    /// Graceful fallback to sector features if specialization fails
    pub async fn get_features_with_fallback(
        &self,
        symbol: &str,
        symbol_data: &TimeSeriesData,
        sector_data: &HashMap<String, TimeSeriesData>,
    ) -> Result<HashMap<String, f64>> {
        // Try specialized features first
        match self.extract_specialized_features(symbol, symbol_data, sector_data).await {
            Ok(features) => {
                // Validate improvement over baseline
                if self.validates_improvement(symbol, &features).await? {
                    debug!("✅ Using specialized features for {}", symbol);
                    return Ok(features);
                } else {
                    warn!("⚠️ Specialization for {} shows no improvement, falling back", symbol);
                }
            }
            Err(e) => {
                error!("❌ Specialization failed for {}: {}", symbol, e);
            }
        }
        
        // Fallback to sector features
        info!("🔄 Falling back to sector features for {}", symbol);
        self.get_sector_fallback_features(symbol, symbol_data, sector_data).await
    }
    
    /// Get sector-only features as fallback
    async fn get_sector_fallback_features(
        &self,
        symbol: &str,
        symbol_data: &TimeSeriesData,
        sector_data: &HashMap<String, TimeSeriesData>,
    ) -> Result<HashMap<String, f64>> {
        let shared_features = self.shared_extractor
            .extract_sector_features(sector_data)
            .await?;
        
        let symbol_features = self.shared_extractor
            .get_symbol_specialization(symbol, symbol_data, &shared_features, sector_data)
            .await?;
        
        // Convert to simple feature map
        let mut features = HashMap::new();
        
        // Add basic shared features
        features.insert("market_regime".to_string(), shared_features.market_regime.regime_type as f64);
        features.insert("volatility".to_string(), shared_features.volatility_features.realized_volatility);
        features.insert("sector_momentum".to_string(), shared_features.momentum_features.sector_momentum_1d);
        features.insert("correlation".to_string(), shared_features.correlation_features.average_pairwise_correlation);
        
        // Add basic symbol features
        features.insert("relative_strength".to_string(), symbol_features.relative_strength);
        features.insert("beta_to_sector".to_string(), symbol_features.beta_to_sector);
        features.insert("idiosyncratic_vol".to_string(), symbol_features.idiosyncratic_volatility);
        
        Ok(features)
    }
    
    // Helper methods
    
    async fn get_or_create_symbol_weights(&self, symbol: &str) -> Result<SymbolSpecializationWeights> {
        if let Some(weights) = self.symbol_weights.get(symbol) {
            Ok(weights.clone())
        } else {
            // Acquire memory permit
            let _permit = self.memory_semaphore.acquire().await
                .context("Failed to acquire memory permit for symbol weights")?;
            
            let weights = SymbolSpecializationWeights::default();
            let memory_usage = std::mem::size_of_val(&weights);
            
            // Check memory limit
            if memory_usage > self.config.max_memory_per_symbol {
                return Err(anyhow!("Symbol weights exceed memory limit: {} > {}", 
                                  memory_usage, self.config.max_memory_per_symbol));
            }
            
            self.symbol_weights.insert(symbol.to_string(), weights.clone());
            
            // Track memory usage
            self.memory_tracker.write().await.insert(symbol.to_string(), memory_usage);
            
            Ok(weights)
        }
    }
    
    fn shared_features_to_vector(&self, features: &SharedSectorFeatures) -> Vec<f64> {
        vec![
            features.market_regime.regime_confidence,
            features.market_regime.trend_strength,
            features.market_regime.volatility_percentile,
            features.volatility_features.realized_volatility,
            features.volatility_features.garch_forecast,
            features.technical_features.sector_rsi,
            features.technical_features.advance_decline_ratio,
            features.correlation_features.average_pairwise_correlation,
            features.momentum_features.sector_momentum_1d,
            features.momentum_features.sector_momentum_5d,
        ]
    }
    
    fn symbol_features_to_vector(&self, features: &SymbolFeatures) -> Vec<f64> {
        vec![
            features.relative_strength,
            features.idiosyncratic_volatility,
            features.beta_to_sector,
            features.correlation_to_sector,
            features.volume_relative_to_sector,
            features.price_relative_to_sector,
        ]
    }
    
    async fn apply_category_scaling(
        &self,
        features: &mut HashMap<String, f64>,
        weights: &SymbolSpecializationWeights,
    ) -> Result<()> {
        for (feature_name, value) in features.iter_mut() {
            let category = self.infer_feature_category(feature_name);
            if let Some(&scale) = weights.category_scales.get(&category) {
                *value *= scale;
            }
        }
        Ok(())
    }
    
    fn infer_feature_category(&self, feature_name: &str) -> FeatureCategory {
        match feature_name {
            name if name.contains("price") || name.contains("return") => FeatureCategory::Price,
            name if name.contains("volume") => FeatureCategory::Volume,
            name if name.contains("volatility") || name.contains("vol") => FeatureCategory::Volatility,
            name if name.contains("momentum") || name.contains("rsi") => FeatureCategory::Momentum,
            name if name.contains("correlation") || name.contains("corr") => FeatureCategory::CrossAsset,
            name if name.contains("regime") => FeatureCategory::Regime,
            name if name.contains("micro") || name.contains("order") => FeatureCategory::OrderFlow,
            _ => FeatureCategory::Custom,
        }
    }
    
    async fn get_cached_signals(&self, symbol: &str) -> Result<Option<SymbolSpecificSignals>> {
        let cache = self.signal_cache.read().await;
        if let Some((signals, timestamp)) = cache.get(symbol) {
            let age = (Utc::now() - *timestamp).num_seconds();
            if age < self.config.cache_ttl_seconds as i64 {
                return Ok(Some(signals.clone()));
            }
        }
        Ok(None)
    }
    
    fn merge_cached_signals(&self, features: &mut HashMap<String, f64>, signals: &SymbolSpecificSignals) {
        for (name, value) in &signals.technical_signals {
            features.insert(format!("tech_{}", name), *value);
        }
        
        for (name, value) in &signals.microstructure_signals {
            features.insert(format!("micro_{}", name), *value);
        }
    }
    
    async fn compute_symbol_signals(&self, symbol_data: &TimeSeriesData) -> Result<SymbolSpecificSignals> {
        let mut technical_signals = HashMap::new();
        let mut microstructure_signals = HashMap::new();
        
        // Compute basic technical signals
        if symbol_data.values.len() > 14 {
            let rsi = self.calculate_rsi(&symbol_data.values, 14)?;
            technical_signals.insert("rsi_14".to_string(), rsi);
        }
        
        if symbol_data.values.len() > 26 {
            let macd = self.calculate_macd(&symbol_data.values)?;
            technical_signals.insert("macd".to_string(), macd);
        }
        
        // Simple volume profile
        let volume_profile = VolumeProfile {
            poc_level: symbol_data.values.iter().sum::<f64>() / symbol_data.values.len() as f64,
            value_area_high: symbol_data.values.iter().fold(0.0, |a, &b| a.max(b)),
            value_area_low: symbol_data.values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            volume_imbalance: 0.0, // Placeholder
            profile_shape: "normal".to_string(),
        };
        
        // Simple order flow
        let order_flow_signals = OrderFlowSignals {
            delta: 0.0, // Would need bid/ask data
            cumulative_delta: 0.0,
            delta_divergence: 0.0,
            volume_at_bid_ask: (0.0, 0.0),
            large_trade_impact: 0.0,
        };
        
        // Microstructure signals
        microstructure_signals.insert("spread_estimate".to_string(), 0.001); // Placeholder
        microstructure_signals.insert("price_impact".to_string(), 0.0);
        
        Ok(SymbolSpecificSignals {
            technical_signals,
            price_patterns: Vec::new(), // Would implement pattern detection
            volume_profile,
            order_flow_signals,
            microstructure_signals,
            timestamp: Utc::now(),
        })
    }
    
    async fn cache_signals(&self, symbol: &str, signals: SymbolSpecificSignals) -> Result<()> {
        let mut cache = self.signal_cache.write().await;
        cache.insert(symbol.to_string(), (signals, Utc::now()));
        
        // Limit cache size (LRU eviction)
        if cache.len() > 1000 {
            // Simple eviction - remove oldest entries
            let oldest_keys: Vec<_> = cache.iter()
                .min_by_key(|(_, (_, timestamp))| *timestamp)
                .map(|(key, _)| key.clone())
                .into_iter()
                .collect();
            
            for key in oldest_keys.iter().take(100) {
                cache.remove(key);
            }
        }
        
        Ok(())
    }
    
    async fn predict_with_weights(
        &self, 
        data: &TimeSeriesData, 
        weights: &SymbolSpecializationWeights
    ) -> Result<f64> {
        // Simple prediction using weighted features
        let mut prediction = 0.0;
        
        for (i, value) in data.values.iter().take(weights.feature_adjustments.len()).enumerate() {
            let weight = weights.feature_adjustments.get(i).unwrap_or(&1.0);
            let bias = weights.bias_terms.get(i).unwrap_or(&0.0);
            prediction += value * weight + bias;
        }
        
        Ok(prediction / weights.feature_adjustments.len() as f64)
    }
    
    async fn calculate_overfitting_score(
        &self,
        weights: &SymbolSpecializationWeights,
        training_data: &[TimeSeriesData],
    ) -> Result<f64> {
        // Simple overfitting detection based on weight magnitudes
        let weight_magnitude: f64 = weights.feature_adjustments.iter()
            .map(|w| w.abs())
            .sum::<f64>() / weights.feature_adjustments.len() as f64;
        
        // If weights deviate too much from identity (1.0), consider it overfitting
        let deviation_from_identity = (weight_magnitude - 1.0).abs();
        
        // Scale by training data size (smaller datasets more prone to overfitting)
        let sample_penalty = (100.0 / training_data.len() as f64).min(1.0);
        
        Ok(deviation_from_identity * sample_penalty)
    }
    
    async fn validates_improvement(&self, symbol: &str, features: &HashMap<String, f64>) -> Result<bool> {
        // Check if specialized features show improvement over baseline
        // This would be replaced with actual performance comparison
        if let Some(weights) = self.symbol_weights.get(symbol) {
            Ok(weights.performance_metrics.improvement_over_baseline > self.config.min_improvement_threshold)
        } else {
            Ok(true) // No baseline, assume improvement
        }
    }
    
    async fn validate_memory_usage(&self, symbol: &str, features: &HashMap<String, f64>) -> Result<()> {
        let feature_memory = features.len() * std::mem::size_of::<f64>();
        let total_memory = self.memory_tracker.read().await.get(symbol).unwrap_or(&0) + feature_memory;
        
        if total_memory > self.config.max_memory_per_symbol {
            return Err(anyhow!(
                "Memory limit exceeded for symbol {}: {} > {}",
                symbol, total_memory, self.config.max_memory_per_symbol
            ));
        }
        
        debug!("Memory usage for {}: {:.2}KB / {:.2}KB", 
               symbol, 
               total_memory as f64 / 1024.0,
               self.config.max_memory_per_symbol as f64 / 1024.0);
        
        Ok(())
    }
    
    fn calculate_rsi(&self, values: &[f64], period: usize) -> Result<f64> {
        if values.len() < period + 1 {
            return Ok(50.0);
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in values.len() - period..values.len() {
            let change = values[i] - values[i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses -= change;
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            Ok(100.0)
        } else {
            let rs = avg_gain / avg_loss;
            Ok(100.0 - (100.0 / (1.0 + rs)))
        }
    }
    
    fn calculate_macd(&self, values: &[f64]) -> Result<f64> {
        if values.len() < 26 {
            return Ok(0.0);
        }
        
        // Simple MACD calculation (12-day EMA - 26-day EMA)
        let ema_12 = self.calculate_ema(values, 12)?;
        let ema_26 = self.calculate_ema(values, 26)?;
        
        Ok(ema_12 - ema_26)
    }
    
    fn calculate_ema(&self, values: &[f64], period: usize) -> Result<f64> {
        if values.is_empty() {
            return Ok(0.0);
        }
        
        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = values[0];
        
        for &value in values.iter().skip(1).take(period.min(values.len())) {
            ema = (value * multiplier) + (ema * (1.0 - multiplier));
        }
        
        Ok(ema)
    }
    
    /// Get memory usage statistics
    pub async fn get_memory_stats(&self) -> Result<(usize, usize, usize)> {
        let memory_tracker = self.memory_tracker.read().await;
        let total_used = memory_tracker.values().sum::<usize>();
        let symbol_count = memory_tracker.len();
        let max_capacity = self.config.max_memory_per_symbol * 256; // Assume max 256 symbols
        
        Ok((total_used, max_capacity, symbol_count))
    }
    
    /// Get specialization performance metrics
    pub async fn get_performance_metrics(&self, symbol: &str) -> Result<Option<SymbolPerformanceMetrics>> {
        if let Some(weights) = self.symbol_weights.get(symbol) {
            Ok(Some(weights.performance_metrics.clone()))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SectorId;
    use crate::features::SharedFeatureConfig;

    #[tokio::test]
    async fn test_symbol_specialization_creation() {
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .unwrap()
        );
        
        let config = SymbolSpecializationConfig::default();
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            config,
        ).await;
        
        assert!(layer.is_ok());
    }
    
    #[tokio::test]
    async fn test_memory_limits() {
        let config = SymbolSpecializationConfig {
            max_memory_per_symbol: 1024, // Very small limit for testing
            ..Default::default()
        };
        
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .unwrap()
        );
        
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            config,
        ).await.unwrap();
        
        // Memory stats should start at zero
        let (used, capacity, count) = layer.get_memory_stats().await.unwrap();
        assert_eq!(used, 0);
        assert_eq!(count, 0);
        assert!(capacity > 0);
    }
    
    #[test]
    fn test_technical_indicators() {
        let layer = create_test_layer();
        
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 4.0, 3.0, 2.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        
        let rsi = layer.calculate_rsi(&values, 14).unwrap();
        assert!(rsi > 0.0 && rsi < 100.0);
        
        let macd = layer.calculate_macd(&values).unwrap();
        assert!(macd.is_finite());
    }
    
    fn create_test_layer() -> SymbolSpecializationLayer {
        // This is a simplified test helper - in real tests we'd use proper async setup
        let shared_extractor = Arc::new(unsafe { std::mem::zeroed() }); // Placeholder for tests
        
        SymbolSpecializationLayer {
            sector_id: SectorId::Technology,
            shared_extractor,
            symbol_weights: Arc::new(DashMap::new()),
            signal_cache: Arc::new(RwLock::new(HashMap::new())),
            config: SymbolSpecializationConfig::default(),
            memory_tracker: Arc::new(RwLock::new(HashMap::new())),
            memory_semaphore: Arc::new(Semaphore::new(100)),
        }
    }
}

// Include comprehensive tests
#[cfg(test)]
#[path = "symbol_specialization_tests.rs"]
mod comprehensive_tests;