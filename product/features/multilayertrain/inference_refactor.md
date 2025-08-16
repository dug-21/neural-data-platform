# Inference Pipeline Refactor Plan

## Overview

This document details the refactoring of the inference/prediction pipeline to utilize sector-based models with symbol specialization layers. The current system routes predictions through individual per-symbol models, while the target architecture combines sector models with symbol-specific specialization layers for efficient and accurate predictions.

## Current Inference Flow Analysis

### Existing Architecture
```rust
// Current: VendorPredictor.predict() uses per-symbol models
VendorPredictor::predict(data: &[TimeSeriesData], horizon: usize)
    -> get_models_for_symbol("NVDA") -> [ModelKey("technology", "lstm", "NVDA")]
    -> Direct prediction with individual model
```

### Problems Identified
1. **Model Lookup Inefficiency**: Each symbol requires individual model lookup
2. **Memory Overhead**: Each symbol model loaded independently
3. **No Sector Knowledge Sharing**: Symbols don't benefit from sector trends
4. **Specialization Layer Unused**: Present but not integrated in prediction flow

## Target Inference Architecture

### New Inference Flow
```rust
// Target: Sector model + specialization layer inference
SectorInferenceEngine::predict("NVDA", data: &[TimeSeriesData])
    -> Load sector model: "technology" 
    -> Load specialization layer: "NVDA"
    -> Combined inference: sector_prediction + specialization_adaptation
    -> Return enhanced prediction with sector context
```

## Implementation Plan

### 1. Sector Inference Engine

#### 1.1 Create SectorInferenceEngine
```rust
// File: src/neural/sector_inference_engine.rs
pub struct SectorInferenceEngine {
    sector_mapper: Arc<SectorMapper>,
    model_storage: Arc<ModelStorage>,
    sector_models: Arc<DashMap<String, Arc<SectorModel>>>,
    specialization_layers: Arc<DashMap<String, Arc<SymbolSpecializationLayer>>>,
    inference_config: SectorInferenceConfig,
    performance_tracker: Arc<ModelPerformanceTracker>,
}

impl SectorInferenceEngine {
    pub async fn predict_with_sector_model(
        &self,
        symbol: &str,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<EnhancedPredictionResult> {
        // 1. Identify sector for symbol
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        
        // 2. Load or get cached sector model
        let sector_model = self.get_or_load_sector_model(&sector_info.id).await?;
        
        // 3. Load or get cached specialization layer
        let specialization_layer = self.get_or_load_specialization_layer(symbol).await?;
        
        // 4. Prepare features for sector inference
        let sector_features = self.prepare_sector_features(data, &sector_info).await?;
        
        // 5. Execute sector model inference
        let sector_prediction = sector_model.predict(&sector_features, horizon).await?;
        
        // 6. Apply symbol specialization
        let specialized_prediction = specialization_layer.adapt_prediction(
            &sector_prediction,
            data,
            &self.inference_config.specialization_config,
        ).await?;
        
        // 7. Combine predictions with confidence weighting
        let final_prediction = self.combine_predictions(
            &sector_prediction,
            &specialized_prediction,
            symbol,
        ).await?;
        
        Ok(EnhancedPredictionResult {
            predictions: final_prediction,
            sector_used: sector_info.id.clone(),
            specialization_applied: true,
            confidence_breakdown: self.calculate_confidence_breakdown(&sector_prediction, &specialized_prediction),
            sector_context: Some(self.get_sector_context(&sector_info.id).await?),
        })
    }
}
```

#### 1.2 Sector Model Loading and Caching
```rust
impl SectorInferenceEngine {
    async fn get_or_load_sector_model(
        &self,
        sector_id: &str,
    ) -> Result<Arc<SectorModel>> {
        // Check cache first
        if let Some(cached_model) = self.sector_models.get(sector_id) {
            return Ok(cached_model.clone());
        }
        
        // Load from storage
        let sector_model = self.model_storage.load_sector_model(sector_id, None).await?;
        let arc_model = Arc::new(sector_model);
        
        // Cache the model
        self.sector_models.insert(sector_id.to_string(), arc_model.clone());
        
        info!("Loaded and cached sector model: {}", sector_id);
        Ok(arc_model)
    }
    
    async fn get_or_load_specialization_layer(
        &self,
        symbol: &str,
    ) -> Result<Arc<SymbolSpecializationLayer>> {
        // Check cache first
        if let Some(cached_layer) = self.specialization_layers.get(symbol) {
            return Ok(cached_layer.clone());
        }
        
        // Determine sector for the symbol
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        
        // Load specialization layer from storage
        let specialization_layer = self.model_storage.load_specialization_layer(
            &sector_info.id,
            symbol,
            None,
        ).await?;
        
        let arc_layer = Arc::new(specialization_layer);
        
        // Cache the specialization layer
        self.specialization_layers.insert(symbol.to_string(), arc_layer.clone());
        
        info!("Loaded and cached specialization layer: {}", symbol);
        Ok(arc_layer)
    }
}
```

### 2. Enhanced Sector Model Inference

#### 2.1 Sector Model Prediction
```rust
// File: src/neural/sector_model.rs (Enhanced for inference)
impl SectorModel {
    pub async fn predict(
        &self,
        sector_features: &SectorFeatures,
        horizon: usize,
    ) -> Result<SectorPredictionResult> {
        // 1. Validate input features
        self.validate_input_features(sector_features)?;
        
        // 2. Apply sector-specific preprocessing
        let processed_features = self.preprocess_sector_features(sector_features)?;
        
        // 3. Execute base model prediction
        let raw_predictions = self.base_model.predict(&processed_features.values)?;
        
        // 4. Apply sector-specific post-processing
        let sector_predictions = self.postprocess_predictions(
            &raw_predictions,
            horizon,
            &processed_features.metadata,
        )?;
        
        // 5. Calculate sector-level confidence
        let sector_confidence = self.calculate_sector_confidence(
            &sector_predictions,
            &processed_features,
        )?;
        
        // 6. Extract sector context information
        let sector_context = self.extract_sector_context(&processed_features)?;
        
        Ok(SectorPredictionResult {
            predictions: sector_predictions,
            confidence: sector_confidence,
            sector_context,
            model_metadata: self.training_metadata.clone(),
            prediction_metadata: self.create_prediction_metadata(horizon),
        })
    }
    
    fn preprocess_sector_features(
        &self,
        features: &SectorFeatures,
    ) -> Result<ProcessedSectorFeatures> {
        // Apply sector-specific normalization
        let normalized_features = self.apply_sector_normalization(&features.raw_features)?;
        
        // Calculate sector momentum indicators
        let momentum_features = self.calculate_sector_momentum(&features.sector_statistics)?;
        
        // Apply correlation-based feature engineering
        let correlation_features = self.engineer_correlation_features(&features.correlation_matrix)?;
        
        Ok(ProcessedSectorFeatures {
            values: [normalized_features, momentum_features, correlation_features].concat(),
            metadata: SectorFeatureMetadata {
                normalization_stats: self.get_normalization_stats(),
                feature_importance: self.get_feature_importance(),
                sector_weights: self.calculate_sector_weights(&features.sector_statistics)?,
            },
        })
    }
}
```

### 3. Symbol Specialization Integration

#### 3.1 Enhanced SymbolSpecializationLayer for Inference
```rust
// File: src/features/symbol_specialization.rs (Enhanced)
impl SymbolSpecializationLayer {
    pub async fn adapt_prediction(
        &self,
        sector_prediction: &SectorPredictionResult,
        symbol_data: &[TimeSeriesData],
        config: &SpecializationConfig,
    ) -> Result<SpecializedPredictionResult> {
        // 1. Extract symbol-specific features
        let symbol_features = self.extract_symbol_features(symbol_data).await?;
        
        // 2. Calculate symbol-sector deviation patterns
        let deviation_patterns = self.calculate_deviation_patterns(
            &symbol_features,
            &sector_prediction.sector_context,
        )?;
        
        // 3. Apply adaptation layers to sector prediction
        let adapted_predictions = self.apply_adaptation_layers(
            &sector_prediction.predictions,
            &deviation_patterns,
            config,
        ).await?;
        
        // 4. Calculate specialization confidence
        let specialization_confidence = self.calculate_specialization_confidence(
            &adapted_predictions,
            &sector_prediction,
            &symbol_features,
        )?;
        
        // 5. Generate symbol-specific insights
        let symbol_insights = self.generate_symbol_insights(
            &symbol_features,
            &adapted_predictions,
        )?;
        
        Ok(SpecializedPredictionResult {
            adapted_predictions,
            specialization_confidence,
            symbol_insights,
            adaptation_metadata: AdaptationMetadata {
                adaptation_strength: self.calculate_adaptation_strength(&deviation_patterns),
                symbol_specific_factors: symbol_insights.key_factors.clone(),
                sector_alignment_score: self.calculate_sector_alignment(&deviation_patterns),
            },
        })
    }
    
    async fn apply_adaptation_layers(
        &self,
        sector_predictions: &[f32],
        deviation_patterns: &DeviationPatterns,
        config: &SpecializationConfig,
    ) -> Result<Vec<f32>> {
        let mut adapted_predictions = sector_predictions.to_vec();
        
        // Apply each adaptation layer sequentially
        for (layer_idx, adaptation_layer) in self.adaptation_layers.iter().enumerate() {
            // Prepare input for this adaptation layer
            let layer_input = self.prepare_adaptation_input(
                &adapted_predictions,
                deviation_patterns,
                layer_idx,
            )?;
            
            // Apply adaptation layer
            let layer_output = adaptation_layer.forward(&layer_input)?;
            
            // Blend with previous predictions based on adaptation strength
            adapted_predictions = self.blend_predictions(
                &adapted_predictions,
                &layer_output,
                config.adaptation_strength,
            )?;
        }
        
        Ok(adapted_predictions)
    }
}
```

### 4. Modified VendorPredictor Integration

#### 4.1 Updated VendorPredictor Inference
```rust
// File: src/neural/vendor_predictor.rs (Modified)
impl VendorPredictor {
    // New sector-based prediction method
    pub async fn predict_with_sector_models(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        // 1. Initialize sector inference engine if not already done
        let sector_engine = self.get_or_create_sector_inference_engine().await?;
        
        let mut results = Vec::new();
        
        // 2. Process each symbol in the input data
        for item in data {
            let symbol = &item.symbol;
            
            // 3. Use sector-based inference
            let enhanced_result = sector_engine.predict_with_sector_model(
                symbol,
                &[item.clone()],
                horizon,
            ).await;
            
            match enhanced_result {
                Ok(enhanced_prediction) => {
                    // Convert to standard PredictionResult format
                    let standard_result = self.convert_to_standard_prediction_result(
                        enhanced_prediction,
                        item,
                        horizon,
                    )?;
                    results.push(standard_result);
                }
                Err(e) => {
                    // Fallback to legacy model if sector-based prediction fails
                    warn!("Sector-based prediction failed for {}, falling back to legacy model: {}", symbol, e);
                    let fallback_result = self.fallback_to_legacy_prediction(item, horizon).await?;
                    results.push(fallback_result);
                }
            }
        }
        
        Ok(results)
    }
    
    // Modified ensemble prediction to use sector models
    pub async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        let sector_engine = self.get_or_create_sector_inference_engine().await?;
        let mut ensemble_results = Vec::new();
        
        for item in data {
            let symbol = &item.symbol;
            let mut symbol_predictions = Vec::new();
            
            // Get sector for this symbol
            let sector_info = self.sector_mapper.get_sector(symbol)?;
            
            // For each requested model type, try sector-based prediction
            for model_type in models {
                let sector_result = sector_engine.predict_with_sector_model_type(
                    symbol,
                    &[item.clone()],
                    horizon,
                    model_type,
                ).await;
                
                match sector_result {
                    Ok(prediction) => {
                        let standard_result = self.convert_to_standard_prediction_result(
                            prediction,
                            item,
                            horizon,
                        )?;
                        symbol_predictions.push(standard_result);
                    }
                    Err(e) => {
                        warn!("Sector-based ensemble prediction failed for {} with model {}: {}", 
                              symbol, model_type, e);
                    }
                }
            }
            
            // Ensemble the predictions
            if !symbol_predictions.is_empty() {
                let ensemble_result = self.ensemble_predictions(&symbol_predictions, item)?;
                ensemble_results.push(ensemble_result);
            } else {
                // Ultimate fallback
                let fallback_result = self.fallback_to_legacy_prediction(item, horizon).await?;
                ensemble_results.push(fallback_result);
            }
        }
        
        Ok(ensemble_results)
    }
    
    // Backward compatibility: redirect old method to new architecture
    async fn ensemble_predict(
        &self,
        symbol: &str,
        data: &TimeSeriesData,
    ) -> Result<PredictionResult> {
        // Try sector-based prediction first
        match self.predict_with_sector_models(&[data.clone()], 1, None).await {
            Ok(mut results) if !results.is_empty() => Ok(results.remove(0)),
            Ok(_) => Err(anyhow!("No prediction results")),
            Err(e) => {
                warn!("Sector-based prediction failed, using legacy ensemble: {}", e);
                // Fallback to existing ensemble logic
                self.fallback_ensemble_predict(symbol, data).await
            }
        }
    }
}
```

### 5. Feature Pipeline Integration

#### 5.1 Sector Feature Extraction
```rust
// File: src/neural/sector_feature_extractor.rs
pub struct SectorFeatureExtractor {
    shared_extractor: Arc<SharedFeatureExtractor>,
    sector_mapper: Arc<SectorMapper>,
    technical_indicators: TechnicalIndicators,
}

impl SectorFeatureExtractor {
    pub async fn extract_sector_features(
        &self,
        symbol_data: &[TimeSeriesData],
        sector_id: &str,
    ) -> Result<SectorFeatures> {
        // 1. Extract shared features across sector
        let shared_features = self.shared_extractor.extract_shared_features(
            symbol_data,
            &SectorId::from_str(sector_id)?,
        ).await?;
        
        // 2. Calculate sector-specific technical indicators
        let sector_indicators = self.calculate_sector_technical_indicators(
            symbol_data,
            sector_id,
        ).await?;
        
        // 3. Extract sector momentum and correlation features
        let sector_momentum = self.extract_sector_momentum_features(symbol_data).await?;
        let correlation_features = self.extract_correlation_features(symbol_data, sector_id).await?;
        
        // 4. Combine all features
        let raw_features = [
            shared_features.features,
            sector_indicators,
            sector_momentum,
            correlation_features,
        ].concat();
        
        // 5. Calculate sector statistics
        let sector_statistics = self.calculate_sector_statistics(&raw_features)?;
        
        // 6. Build correlation matrix
        let correlation_matrix = self.build_sector_correlation_matrix(symbol_data, sector_id).await?;
        
        Ok(SectorFeatures {
            raw_features,
            sector_statistics,
            correlation_matrix,
            feature_metadata: SectorFeatureMetadata {
                extraction_timestamp: Utc::now(),
                sector_id: sector_id.to_string(),
                symbol_count: symbol_data.len(),
                feature_count: raw_features.len(),
            },
        })
    }
}
```

### 6. Prediction Combination and Confidence

#### 6.1 Prediction Combination Logic
```rust
impl SectorInferenceEngine {
    async fn combine_predictions(
        &self,
        sector_prediction: &SectorPredictionResult,
        specialized_prediction: &SpecializedPredictionResult,
        symbol: &str,
    ) -> Result<Vec<PredictionResult>> {
        let mut combined_predictions = Vec::new();
        
        for (i, (&sector_val, &specialized_val)) in sector_prediction.predictions
            .iter()
            .zip(specialized_prediction.adapted_predictions.iter())
            .enumerate()
        {
            // Calculate dynamic weighting based on confidence and historical performance
            let sector_weight = self.calculate_sector_weight(
                sector_prediction.confidence,
                symbol,
                i,
            ).await?;
            
            let specialization_weight = self.calculate_specialization_weight(
                specialized_prediction.specialization_confidence,
                &specialized_prediction.adaptation_metadata,
                i,
            ).await?;
            
            // Normalize weights
            let total_weight = sector_weight + specialization_weight;
            let normalized_sector_weight = sector_weight / total_weight;
            let normalized_spec_weight = specialization_weight / total_weight;
            
            // Combine predictions
            let combined_value = (sector_val * normalized_sector_weight) + 
                               (specialized_val * normalized_spec_weight);
            
            // Calculate combined confidence
            let combined_confidence = self.calculate_combined_confidence(
                sector_prediction.confidence,
                specialized_prediction.specialization_confidence,
                normalized_sector_weight,
                normalized_spec_weight,
            );
            
            // Create prediction result
            let prediction_result = PredictionResult {
                value: combined_value as f64,
                confidence: combined_confidence,
                model_name: format!("sector_{}_specialized", &sector_prediction.sector_context.sector_id),
                interval_low: combined_value as f64 - (combined_confidence * combined_value.abs() as f64),
                interval_high: combined_value as f64 + (combined_confidence * combined_value.abs() as f64),
                timestamp: Utc::now(),
                metadata: Some(self.create_combined_metadata(
                    sector_prediction,
                    specialized_prediction,
                    normalized_sector_weight,
                    normalized_spec_weight,
                )),
            };
            
            combined_predictions.push(prediction_result);
        }
        
        Ok(combined_predictions)
    }
}
```

### 7. Memory Management and Caching

#### 7.1 Intelligent Model Caching
```rust
// File: src/neural/sector_model_cache.rs
pub struct SectorModelCache {
    sector_models: Arc<DashMap<String, CachedSectorModel>>,
    specialization_layers: Arc<DashMap<String, CachedSpecializationLayer>>,
    cache_config: SectorCacheConfig,
    memory_monitor: Arc<MemoryMonitor>,
}

#[derive(Debug)]
struct CachedSectorModel {
    model: Arc<SectorModel>,
    last_accessed: DateTime<Utc>,
    access_count: u64,
    memory_size_mb: f64,
}

impl SectorModelCache {
    pub async fn get_sector_model(
        &self,
        sector_id: &str,
    ) -> Result<Arc<SectorModel>> {
        // Check if model is cached
        if let Some(mut cached) = self.sector_models.get_mut(sector_id) {
            cached.last_accessed = Utc::now();
            cached.access_count += 1;
            return Ok(cached.model.clone());
        }
        
        // Check memory limits before loading
        self.ensure_memory_limits().await?;
        
        // Load model (this would call the storage layer)
        let model = self.load_sector_model_from_storage(sector_id).await?;
        let model_arc = Arc::new(model);
        
        // Cache the model
        let cached_model = CachedSectorModel {
            model: model_arc.clone(),
            last_accessed: Utc::now(),
            access_count: 1,
            memory_size_mb: self.estimate_model_memory_size(&model_arc),
        };
        
        self.sector_models.insert(sector_id.to_string(), cached_model);
        
        Ok(model_arc)
    }
    
    async fn ensure_memory_limits(&self) -> Result<()> {
        let current_usage = self.calculate_current_memory_usage().await;
        let memory_limit = self.cache_config.max_memory_mb;
        
        if current_usage > memory_limit {
            self.evict_least_recently_used().await?;
        }
        
        Ok(())
    }
    
    async fn evict_least_recently_used(&self) -> Result<()> {
        // Find least recently used sector model
        let mut oldest_access = Utc::now();
        let mut lru_sector = String::new();
        
        for entry in self.sector_models.iter() {
            if entry.value().last_accessed < oldest_access {
                oldest_access = entry.value().last_accessed;
                lru_sector = entry.key().clone();
            }
        }
        
        if !lru_sector.is_empty() {
            self.sector_models.remove(&lru_sector);
            info!("Evicted LRU sector model: {}", lru_sector);
        }
        
        // Similar logic for specialization layers
        // ... (implementation details)
        
        Ok(())
    }
}
```

## Implementation Timeline

### Week 1: Core Infrastructure
- [ ] Create SectorInferenceEngine
- [ ] Implement sector model loading and caching
- [ ] Basic sector feature extraction

### Week 2: Specialization Integration
- [ ] Enhance SymbolSpecializationLayer for inference
- [ ] Implement prediction adaptation logic
- [ ] Create prediction combination framework

### Week 3: VendorPredictor Integration
- [ ] Modify VendorPredictor prediction methods
- [ ] Implement backward compatibility
- [ ] Add fallback mechanisms

### Week 4: Optimization and Testing
- [ ] Implement memory management and caching
- [ ] Performance optimization
- [ ] Comprehensive testing

## Success Metrics

1. **Memory Efficiency**: 64% reduction in prediction memory usage
2. **Inference Speed**: <200ms for batch predictions
3. **Cache Hit Rate**: >90% for frequently used sector models
4. **Prediction Accuracy**: Maintain ≥95% of current accuracy
5. **Specialization Effectiveness**: >10% improvement in symbol-specific accuracy

## Risks and Mitigation

### High Risks
1. **Prediction Accuracy Drop**: Extensive A/B testing and gradual rollout
2. **Memory Regression**: Careful memory monitoring and caching strategy
3. **Performance Degradation**: Profiling and optimization at each step

### Medium Risks
1. **Cache Complexity**: Clear eviction policies and monitoring
2. **Integration Issues**: Comprehensive test coverage
3. **Backward Compatibility**: Feature flags and fallback mechanisms

This inference refactor plan provides a detailed roadmap for implementing sector-based prediction with symbol specialization while maintaining system performance and reliability.