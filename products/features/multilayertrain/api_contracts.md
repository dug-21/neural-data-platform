# API Contracts: Multilayer Ensemble System

## Overview

This document defines the interface contracts between layers in the multilayer ensemble neural system, ensuring clean separation of concerns and maintainable integration points.

## Core Interface Definitions

### Layer Interface Trait
```rust
#[async_trait]
pub trait LayerInterface<I, O> {
    /// Process input data through this layer
    async fn process(&self, input: I) -> Result<O>;
    
    /// Get layer configuration
    fn get_config(&self) -> &LayerConfig;
    
    /// Validate input data format
    async fn validate_input(&self, input: &I) -> Result<()>;
    
    /// Get layer performance metrics
    async fn get_metrics(&self) -> LayerMetrics;
    
    /// Health check for this layer
    async fn health_check(&self) -> HealthStatus;
}
```

## Layer 1: Symbol-Level Interfaces

### Symbol Processor Interface
```rust
/// Primary interface for symbol-level processing
#[async_trait]
pub trait SymbolProcessor {
    async fn process_symbol_data(
        &self,
        symbol: &str,
        data: TimeSeriesData
    ) -> Result<SymbolPrediction>;
    
    async fn batch_process_symbols(
        &self,
        symbol_data: Vec<(String, TimeSeriesData)>
    ) -> Result<Vec<SymbolPrediction>>;
    
    async fn get_symbol_model(&self, symbol: &str) -> Result<Arc<dyn BaseModel<f32>>>;
    
    async fn update_symbol_model(
        &self,
        symbol: &str,
        training_data: &TrainingData<f32>
    ) -> Result<()>;
}

/// Implementation for VendorPredictor
impl SymbolProcessor for VendorPredictor {
    async fn process_symbol_data(
        &self,
        symbol: &str,
        data: TimeSeriesData
    ) -> Result<SymbolPrediction> {
        // Route to appropriate symbol model
        let sector = self.sector_mapper.get_sector(symbol)?;
        let model_ref = self.get_model_for_prediction(symbol, "primary").await?
            .ok_or_else(|| anyhow!("No model available for symbol: {}", symbol))?;
        
        // Convert data and predict
        let (vendor_data, _metadata) = self.convert_to_vendor_format(&data, symbol).await?;
        let data_values: Vec<f32> = vendor_data.values.iter().map(|&v| v as f32).collect();
        
        let prediction_values = model_ref.value().predict(&data_values)?;
        let primary_value = prediction_values.get(0).copied().unwrap_or(0.0);
        
        Ok(SymbolPrediction {
            symbol: symbol.to_string(),
            prediction_value: primary_value as f64,
            confidence: 0.8, // Default confidence
            timestamp: Utc::now(),
            features_used: vec!["price".to_string(), "volume".to_string()],
            model_info: SymbolModelInfo {
                model_type: "FANN".to_string(),
                version: "1.0.0".to_string(),
                training_samples: 1000,
                last_trained: Utc::now(),
            },
            metadata: HashMap::new(),
        })
    }
    
    async fn batch_process_symbols(
        &self,
        symbol_data: Vec<(String, TimeSeriesData)>
    ) -> Result<Vec<SymbolPrediction>> {
        let mut predictions = Vec::new();
        
        for (symbol, data) in symbol_data {
            match self.process_symbol_data(&symbol, data).await {
                Ok(pred) => predictions.push(pred),
                Err(e) => {
                    warn!("Failed to process symbol {}: {}", symbol, e);
                    // Create fallback prediction
                    predictions.push(SymbolPrediction::fallback(&symbol));
                }
            }
        }
        
        Ok(predictions)
    }
    
    async fn get_symbol_model(&self, symbol: &str) -> Result<Arc<dyn BaseModel<f32>>> {
        let model_ref = self.get_model_for_prediction(symbol, "primary").await?
            .ok_or_else(|| anyhow!("No model available for symbol: {}", symbol))?;
        
        // Note: This requires cloning the model, which may not be ideal
        // In practice, we'd return a reference or handle
        Err(anyhow!("Model reference extraction not supported"))
    }
    
    async fn update_symbol_model(
        &self,
        symbol: &str,
        training_data: &TrainingData<f32>
    ) -> Result<()> {
        // Trigger retraining for the symbol
        let training_config = TrainingConfig {
            max_epochs: 1000,
            learning_rate: 0.01,
            batch_size: 32,
            validation_size: 0.2,
            early_stopping_patience: 50,
            save_best_model: true,
            verbose: false,
            use_gpu: false,
            gradient_clipping: Some(1.0),
            weight_decay: Some(0.0001),
            scheduler_config: None,
        };
        
        // Convert to TimeSeriesData format for training
        let time_series_data = self.convert_training_data_to_time_series(training_data, symbol)?;
        
        self.train_model(symbol, &[time_series_data]).await?;
        Ok(())
    }
}
```

### Symbol Data Structures
```rust
/// Symbol-level prediction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolPrediction {
    pub symbol: String,
    pub prediction_value: f64,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
    pub features_used: Vec<String>,
    pub model_info: SymbolModelInfo,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolModelInfo {
    pub model_type: String,
    pub version: String,
    pub training_samples: usize,
    pub last_trained: DateTime<Utc>,
}

impl SymbolPrediction {
    pub fn fallback(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            prediction_value: 0.0,
            confidence: 0.1,
            timestamp: Utc::now(),
            features_used: vec!["fallback".to_string()],
            model_info: SymbolModelInfo {
                model_type: "Fallback".to_string(),
                version: "1.0.0".to_string(),
                training_samples: 0,
                last_trained: Utc::now(),
            },
            metadata: HashMap::new(),
        }
    }
}
```

## Layer 2: Sector Aggregation Interfaces

### Sector Aggregator Interface
```rust
/// Interface for sector-level aggregation
#[async_trait]
pub trait SectorAggregator {
    async fn aggregate_sector_predictions(
        &self,
        sector: SectorId,
        symbol_predictions: Vec<SymbolPrediction>
    ) -> Result<SectorPrediction>;
    
    async fn calculate_sector_weights(
        &self,
        sector: SectorId,
        symbol_predictions: &[SymbolPrediction]
    ) -> Result<SectorWeights>;
    
    async fn extract_sector_features(
        &self,
        sector: SectorId,
        symbol_data: &[TimeSeriesData]
    ) -> Result<SectorFeatures>;
    
    async fn get_sector_model(&self, sector: SectorId) -> Result<Arc<dyn BaseModel<f32>>>;
}

/// Implementation for sector aggregation
pub struct SectorAggregatorImpl {
    sector_mapper: Arc<SectorMapper>,
    feature_extractor: Arc<SharedFeatureExtractor>,
    model_pools: HashMap<SectorId, Arc<ClusterModelPool>>,
}

impl SectorAggregator for SectorAggregatorImpl {
    async fn aggregate_sector_predictions(
        &self,
        sector: SectorId,
        symbol_predictions: Vec<SymbolPrediction>
    ) -> Result<SectorPrediction> {
        // Calculate dynamic weights
        let weights = self.calculate_sector_weights(sector, &symbol_predictions).await?;
        
        // Weighted ensemble
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        let mut confidence_sum = 0.0;
        
        for (prediction, weight) in symbol_predictions.iter().zip(weights.iter()) {
            weighted_sum += prediction.prediction_value * weight.value;
            confidence_sum += prediction.confidence * weight.value;
            weight_sum += weight.value;
        }
        
        let aggregated_value = if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            0.0
        };
        
        let aggregated_confidence = if weight_sum > 0.0 {
            confidence_sum / weight_sum
        } else {
            0.1
        };
        
        // Extract sector-level features
        let sector_features = self.extract_sector_features(
            sector,
            &[] // Would need actual market data here
        ).await?;
        
        Ok(SectorPrediction {
            sector,
            prediction_value: aggregated_value,
            confidence: aggregated_confidence,
            timestamp: Utc::now(),
            contributing_symbols: symbol_predictions.iter()
                .map(|p| p.symbol.clone())
                .collect(),
            weights: weights.into_iter().collect(),
            sector_features,
            metadata: HashMap::new(),
        })
    }
    
    async fn calculate_sector_weights(
        &self,
        sector: SectorId,
        symbol_predictions: &[SymbolPrediction]
    ) -> Result<SectorWeights> {
        let mut weights = Vec::new();
        
        for prediction in symbol_predictions {
            // Base weight from market cap (would need actual market cap data)
            let market_cap_weight = 1.0 / symbol_predictions.len() as f64; // Equal weight for now
            
            // Performance adjustment based on confidence
            let performance_adj = prediction.confidence;
            
            // Volatility adjustment (higher confidence for stable predictions)
            let volatility_adj = 1.0; // Would calculate from actual volatility
            
            let final_weight = market_cap_weight * performance_adj * volatility_adj;
            
            weights.push(SymbolWeight {
                symbol: prediction.symbol.clone(),
                value: final_weight,
                components: WeightComponents {
                    market_cap: market_cap_weight,
                    performance: performance_adj,
                    volatility: volatility_adj,
                },
            });
        }
        
        // Normalize weights
        let total_weight: f64 = weights.iter().map(|w| w.value).sum();
        if total_weight > 0.0 {
            for weight in &mut weights {
                weight.value /= total_weight;
            }
        }
        
        Ok(SectorWeights { weights })
    }
    
    async fn extract_sector_features(
        &self,
        sector: SectorId,
        _symbol_data: &[TimeSeriesData]
    ) -> Result<SectorFeatures> {
        // Use SharedFeatureExtractor to get sector-level features
        let features = self.feature_extractor
            .extract_sector_features(sector, &[])
            .await?;
        
        Ok(SectorFeatures {
            sector_momentum: features.momentum_indicators.get("sector_momentum").unwrap_or(&0.0).clone(),
            sector_volatility: features.volatility_indicators.get("sector_volatility").unwrap_or(&0.0).clone(),
            correlation_strength: features.correlation_indicators.get("avg_correlation").unwrap_or(&0.0).clone(),
            market_cap_concentration: 0.5, // Would calculate from actual data
            volume_concentration: 0.3,     // Would calculate from actual data
            cross_correlations: HashMap::new(), // Would populate with actual correlations
        })
    }
    
    async fn get_sector_model(&self, sector: SectorId) -> Result<Arc<dyn BaseModel<f32>>> {
        let pool = self.model_pools.get(&sector)
            .ok_or_else(|| anyhow!("No model pool for sector: {:?}", sector))?;
        
        let model_ref = pool.get_model_for_prediction("sector_model")
            .ok_or_else(|| anyhow!("No sector model available for: {:?}", sector))?;
        
        // Note: This would need proper model reference handling
        Err(anyhow!("Sector model reference extraction not implemented"))
    }
}
```

### Sector Data Structures
```rust
/// Sector-level prediction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorPrediction {
    pub sector: SectorId,
    pub prediction_value: f64,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
    pub contributing_symbols: Vec<String>,
    pub weights: Vec<SymbolWeight>,
    pub sector_features: SectorFeatures,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorWeights {
    pub weights: Vec<SymbolWeight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolWeight {
    pub symbol: String,
    pub value: f64,
    pub components: WeightComponents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightComponents {
    pub market_cap: f64,
    pub performance: f64,
    pub volatility: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorFeatures {
    pub sector_momentum: f64,
    pub sector_volatility: f64,
    pub correlation_strength: f64,
    pub market_cap_concentration: f64,
    pub volume_concentration: f64,
    pub cross_correlations: HashMap<String, f64>,
}

impl SectorWeights {
    pub fn iter(&self) -> impl Iterator<Item = &SymbolWeight> {
        self.weights.iter()
    }
    
    pub fn into_iter(self) -> impl Iterator<Item = SymbolWeight> {
        self.weights.into_iter()
    }
}
```

## Layer 3: Specialization Interfaces

### Specialization Processor Interface
```rust
/// Interface for specialization layer processing
#[async_trait]
pub trait SpecializationProcessor {
    async fn apply_specializations(
        &self,
        sector_prediction: SectorPrediction,
        market_context: MarketContext
    ) -> Result<FinalPrediction>;
    
    async fn detect_market_regime(
        &self,
        market_context: &MarketContext
    ) -> Result<MarketRegime>;
    
    async fn apply_specialist(
        &self,
        prediction: SectorPrediction,
        specialist_type: SpecialistType,
        context: &MarketContext
    ) -> Result<SpecialistOutput>;
}

/// Market regime detection
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MarketRegime {
    HighVolatility,
    Trending,
    MeanReverting,
    Transitional,
}

/// Specialist types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SpecialistType {
    Volatility,
    Trend,
    Momentum,
    MeanReversion,
}

/// Implementation for specialization processing
pub struct SpecializationProcessorImpl {
    regime_detector: RegimeDetector,
    volatility_specialist: VolatilitySpecialist,
    trend_specialist: TrendSpecialist,
    momentum_specialist: MomentumSpecialist,
    mean_reversion_specialist: MeanReversionSpecialist,
    combiner: SpecializationCombiner,
}

impl SpecializationProcessor for SpecializationProcessorImpl {
    async fn apply_specializations(
        &self,
        sector_prediction: SectorPrediction,
        market_context: MarketContext
    ) -> Result<FinalPrediction> {
        // Detect market regime
        let regime = self.detect_market_regime(&market_context).await?;
        
        // Apply relevant specialists based on regime
        let specialist_outputs = match regime {
            MarketRegime::HighVolatility => {
                vec![
                    self.apply_specialist(
                        sector_prediction.clone(),
                        SpecialistType::Volatility,
                        &market_context
                    ).await?,
                    self.apply_specialist(
                        sector_prediction.clone(),
                        SpecialistType::MeanReversion,
                        &market_context
                    ).await?,
                ]
            },
            MarketRegime::Trending => {
                vec![
                    self.apply_specialist(
                        sector_prediction.clone(),
                        SpecialistType::Trend,
                        &market_context
                    ).await?,
                    self.apply_specialist(
                        sector_prediction.clone(),
                        SpecialistType::Momentum,
                        &market_context
                    ).await?,
                ]
            },
            MarketRegime::MeanReverting => {
                vec![
                    self.apply_specialist(
                        sector_prediction.clone(),
                        SpecialistType::MeanReversion,
                        &market_context
                    ).await?,
                ]
            },
            MarketRegime::Transitional => {
                vec![
                    self.apply_specialist(
                        sector_prediction.clone(),
                        SpecialistType::Volatility,
                        &market_context
                    ).await?,
                ]
            }
        };
        
        // Combine specialist outputs
        let final_prediction = self.combiner
            .combine_outputs(specialist_outputs, regime, sector_prediction)
            .await?;
        
        Ok(final_prediction)
    }
    
    async fn detect_market_regime(
        &self,
        market_context: &MarketContext
    ) -> Result<MarketRegime> {
        self.regime_detector.detect_regime(market_context).await
    }
    
    async fn apply_specialist(
        &self,
        prediction: SectorPrediction,
        specialist_type: SpecialistType,
        context: &MarketContext
    ) -> Result<SpecialistOutput> {
        match specialist_type {
            SpecialistType::Volatility => {
                self.volatility_specialist.process(prediction, context).await
            },
            SpecialistType::Trend => {
                self.trend_specialist.process(prediction, context).await
            },
            SpecialistType::Momentum => {
                self.momentum_specialist.process(prediction, context).await
            },
            SpecialistType::MeanReversion => {
                self.mean_reversion_specialist.process(prediction, context).await
            },
        }
    }
}
```

### Specialization Data Structures
```rust
/// Final prediction output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalPrediction {
    pub symbol: String,
    pub prediction_value: f64,
    pub confidence: f64,
    pub prediction_intervals: PredictionIntervals,
    pub timestamp: DateTime<Utc>,
    pub market_regime: MarketRegime,
    pub specialist_contributions: Vec<SpecialistContribution>,
    pub layer_breakdown: LayerBreakdown,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionIntervals {
    pub lower_80: f64,
    pub upper_80: f64,
    pub lower_95: f64,
    pub upper_95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialistContribution {
    pub specialist_type: SpecialistType,
    pub adjustment: f64,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerBreakdown {
    pub symbol_layer_contribution: f64,
    pub sector_layer_contribution: f64,
    pub specialization_layer_contribution: f64,
}

/// Market context for specialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketContext {
    pub volatility_metrics: VolatilityMetrics,
    pub trend_metrics: TrendMetrics,
    pub momentum_metrics: MomentumMetrics,
    pub microstructure_metrics: MicrostructureMetrics,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityMetrics {
    pub vix: f64,
    pub realized_volatility: f64,
    pub garch_forecast: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendMetrics {
    pub adx: f64,
    pub trend_strength: f64,
    pub direction: TrendDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Up,
    Down,
    Sideways,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumMetrics {
    pub rsi: f64,
    pub macd_signal: f64,
    pub momentum_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrostructureMetrics {
    pub bid_ask_spread: f64,
    pub order_flow_imbalance: f64,
    pub volume_profile: VolumeProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfile {
    pub total_volume: f64,
    pub buy_volume_ratio: f64,
    pub large_trade_ratio: f64,
}

/// Specialist output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialistOutput {
    pub specialist_type: SpecialistType,
    pub original_prediction: f64,
    pub adjusted_prediction: f64,
    pub adjustment_magnitude: f64,
    pub confidence: f64,
    pub reasoning: String,
    pub supporting_metrics: HashMap<String, f64>,
}
```

## Integration Layer Interface

### MultilayerEnsemble Main Interface
```rust
/// Main interface for the multilayer ensemble system
#[async_trait]
pub trait MultilayerEnsemble {
    async fn predict_multilayer(
        &self,
        symbol: &str,
        data: TimeSeriesData,
        horizon: usize
    ) -> Result<FinalPrediction>;
    
    async fn batch_predict_multilayer(
        &self,
        requests: Vec<PredictionRequest>
    ) -> Result<Vec<FinalPrediction>>;
    
    async fn get_system_status(&self) -> SystemStatus;
    
    async fn get_layer_metrics(&self) -> LayerMetrics;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionRequest {
    pub symbol: String,
    pub data: TimeSeriesData,
    pub horizon: usize,
    pub options: PredictionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionOptions {
    pub include_layer_breakdown: bool,
    pub include_specialist_details: bool,
    pub confidence_level: f64, // 0.8, 0.95, etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub layer_1_status: LayerStatus,
    pub layer_2_status: LayerStatus,
    pub layer_3_status: LayerStatus,
    pub overall_health: HealthStatus,
    pub active_models: usize,
    pub memory_usage_mb: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerStatus {
    pub status: HealthStatus,
    pub active_models: usize,
    pub avg_latency_ms: f64,
    pub error_rate: f64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Offline,
}

/// Comprehensive layer metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerMetrics {
    pub symbol_layer_metrics: SymbolLayerMetrics,
    pub sector_layer_metrics: SectorLayerMetrics,
    pub specialization_layer_metrics: SpecializationLayerMetrics,
    pub overall_metrics: OverallMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLayerMetrics {
    pub active_symbol_models: usize,
    pub avg_prediction_latency_ms: f64,
    pub prediction_accuracy: f64,
    pub memory_usage_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorLayerMetrics {
    pub active_sector_pools: usize,
    pub avg_aggregation_latency_ms: f64,
    pub ensemble_accuracy: f64,
    pub weight_stability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecializationLayerMetrics {
    pub regime_detection_accuracy: f64,
    pub specialist_usage_distribution: HashMap<SpecialistType, f64>,
    pub avg_enhancement_magnitude: f64,
    pub confidence_calibration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallMetrics {
    pub end_to_end_latency_ms: f64,
    pub overall_accuracy: f64,
    pub throughput_predictions_per_second: f64,
    pub memory_efficiency_score: f64,
}
```

This comprehensive API contract design ensures clean, well-defined interfaces between all layers of the multilayer ensemble system, enabling maintainable integration and clear separation of concerns.