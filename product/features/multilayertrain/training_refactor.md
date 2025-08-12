# Training Flow Refactor Plan

## Overview

This document details the refactoring of the training pipeline from per-symbol model creation to sector-based models with symbol specialization layers. The current system creates individual models for each symbol, while the target architecture uses 10 sector models with specialized layers.

## Current Training Flow Analysis

### Existing Architecture
```rust
// Current: VendorPredictor.train_model() creates per-symbol models
VendorPredictor::train_model(model_name: "NVDA_lstm", data: &[TimeSeriesData])
    -> Creates ModelKey("technology", "lstm", "NVDA")
    -> Stores individual model for NVDA
```

### Problems Identified
1. **Model Proliferation**: 100+ individual models created
2. **Memory Inefficiency**: Each symbol has full model overhead
3. **Training Inefficiency**: No sector-level knowledge sharing
4. **Unused Architecture**: SymbolSpecializationLayer present but not integrated

## Target Training Architecture

### New Training Flow
```rust
// Target: Sector-based training with specialization
SectorTrainingCoordinator::train_sector("technology", symbols: ["NVDA", "AAPL", "MSFT"])
    -> Creates shared sector model: ModelKey("technology", "lstm", "shared")
    -> Creates specialization layers for each symbol
    -> Combines for inference: sector_model + symbol_specialization
```

## Implementation Plan

### 1. Sector Training Coordinator

#### 1.1 Create SectorTrainingCoordinator
```rust
// File: src/neural/sector_training_coordinator.rs
pub struct SectorTrainingCoordinator {
    sector_mapper: Arc<SectorMapper>,
    model_storage: Arc<ModelStorage>,
    vendor_predictor: Arc<VendorPredictor>,
    training_config: SectorTrainingConfig,
}

impl SectorTrainingCoordinator {
    pub async fn train_sector(
        &mut self,
        sector_id: &str,
        symbols: &[String],
        training_data: &HashMap<String, Vec<TimeSeriesData>>,
    ) -> Result<SectorTrainingResult> {
        // 1. Aggregate sector-level features
        let sector_features = self.aggregate_sector_features(training_data).await?;
        
        // 2. Train shared sector model
        let sector_model = self.train_shared_sector_model(sector_id, &sector_features).await?;
        
        // 3. Train symbol-specific specialization layers
        let specialization_layers = self.train_specialization_layers(
            symbols, 
            training_data,
            &sector_model
        ).await?;
        
        // 4. Store models and layers
        self.store_sector_model(sector_id, sector_model, specialization_layers).await?;
        
        Ok(SectorTrainingResult { /* ... */ })
    }
}
```

#### 1.2 Sector Feature Aggregation
```rust
impl SectorTrainingCoordinator {
    async fn aggregate_sector_features(
        &self,
        training_data: &HashMap<String, Vec<TimeSeriesData>>,
    ) -> Result<SectorFeatures> {
        let mut aggregated_features = Vec::new();
        
        // Aggregate features across all symbols in sector
        for (symbol, data) in training_data {
            let symbol_features = self.extract_symbol_features(data).await?;
            aggregated_features.extend(symbol_features);
        }
        
        // Calculate sector-level statistics
        let sector_stats = self.calculate_sector_statistics(&aggregated_features)?;
        
        Ok(SectorFeatures {
            raw_features: aggregated_features,
            sector_statistics: sector_stats,
            correlation_matrix: self.calculate_sector_correlations(training_data).await?,
        })
    }
}
```

### 2. Shared Sector Model Training

#### 2.1 Sector Model Architecture
```rust
// File: src/neural/sector_model.rs
pub struct SectorModel {
    base_model: Box<dyn BaseModel<f32>>,
    sector_id: String,
    architecture_config: SectorModelConfig,
    training_metadata: SectorModelMetadata,
}

#[derive(Serialize, Deserialize)]
pub struct SectorModelConfig {
    input_size: usize,
    hidden_layers: Vec<usize>,
    output_size: usize,
    sector_specific_features: Vec<String>,
    regularization: RegularizationConfig,
}
```

#### 2.2 Sector Model Training Implementation
```rust
impl SectorTrainingCoordinator {
    async fn train_shared_sector_model(
        &mut self,
        sector_id: &str,
        sector_features: &SectorFeatures,
    ) -> Result<SectorModel> {
        // 1. Configure sector-specific architecture
        let config = self.get_sector_model_config(sector_id)?;
        
        // 2. Prepare training data
        let training_data = self.prepare_sector_training_data(sector_features, &config)?;
        
        // 3. Create and train sector model
        let mut sector_model = self.create_sector_model(sector_id, &config).await?;
        
        // 4. Train with sector-aggregated data
        let training_result = sector_model.train_with_aggregated_data(
            &training_data,
            &self.training_config.sector_training_params,
        ).await?;
        
        // 5. Validate sector model performance
        self.validate_sector_model(&sector_model, sector_features).await?;
        
        Ok(sector_model)
    }
}
```

### 3. Symbol Specialization Layer Training

#### 3.1 Enhanced SymbolSpecializationLayer
```rust
// File: src/features/symbol_specialization.rs (Enhanced)
pub struct SymbolSpecializationLayer {
    symbol: String,
    sector_id: String,
    
    // Specialization components
    symbol_embeddings: HashMap<String, Vec<f32>>,
    adaptation_layers: Vec<AdaptationLayer>,
    symbol_specific_features: SymbolSpecificFeatures,
    
    // Training state
    training_metadata: SpecializationTrainingMetadata,
    performance_metrics: SpecializationPerformanceMetrics,
}

impl SymbolSpecializationLayer {
    pub async fn train_specialization(
        &mut self,
        symbol_data: &[TimeSeriesData],
        sector_model: &SectorModel,
        config: &SpecializationTrainingConfig,
    ) -> Result<SpecializationTrainingResult> {
        // 1. Extract symbol-specific patterns
        let symbol_patterns = self.extract_symbol_patterns(symbol_data).await?;
        
        // 2. Identify sector model gaps for this symbol
        let adaptation_targets = self.identify_adaptation_targets(
            symbol_data,
            sector_model,
        ).await?;
        
        // 3. Train adaptation layers
        let training_results = self.train_adaptation_layers(
            &symbol_patterns,
            &adaptation_targets,
            config,
        ).await?;
        
        // 4. Optimize specialization layer weights
        self.optimize_specialization_weights(
            symbol_data,
            sector_model,
            config,
        ).await?;
        
        Ok(training_results)
    }
}
```

#### 3.2 Adaptation Layer Training
```rust
impl SymbolSpecializationLayer {
    async fn train_adaptation_layers(
        &mut self,
        symbol_patterns: &SymbolPatterns,
        adaptation_targets: &AdaptationTargets,
        config: &SpecializationTrainingConfig,
    ) -> Result<Vec<AdaptationLayerResult>> {
        let mut results = Vec::new();
        
        for (layer_idx, target) in adaptation_targets.iter().enumerate() {
            // Create adaptation layer for specific sector model layer
            let mut adaptation_layer = AdaptationLayer::new(
                target.input_size,
                target.output_size,
                config.adaptation_layer_config.clone(),
            );
            
            // Prepare training data for this adaptation layer
            let layer_training_data = self.prepare_adaptation_data(
                symbol_patterns,
                target,
            )?;
            
            // Train adaptation layer
            let training_result = adaptation_layer.train(
                &layer_training_data,
                &config.adaptation_training_params,
            ).await?;
            
            self.adaptation_layers.push(adaptation_layer);
            results.push(training_result);
        }
        
        Ok(results)
    }
}
```

### 4. Training Pipeline Integration

#### 4.1 Modified VendorPredictor Training
```rust
// File: src/neural/vendor_predictor.rs (Modified)
impl VendorPredictor {
    // New sector-based training method
    pub async fn train_sector_based_models(
        &mut self,
        training_data: HashMap<String, Vec<TimeSeriesData>>,
    ) -> Result<SectorTrainingResults> {
        // 1. Group symbols by sector
        let sector_groups = self.group_symbols_by_sector(&training_data)?;
        
        // 2. Create sector training coordinator
        let mut sector_coordinator = SectorTrainingCoordinator::new(
            self.sector_mapper.clone(),
            self.storage.clone(),
            Arc::new(self.clone()), // Need to make VendorPredictor cloneable
        );
        
        // 3. Train each sector
        let mut sector_results = HashMap::new();
        for (sector_id, sector_symbols) in sector_groups {
            let sector_data: HashMap<String, Vec<TimeSeriesData>> = sector_symbols
                .iter()
                .filter_map(|symbol| {
                    training_data.get(symbol).map(|data| (symbol.clone(), data.clone()))
                })
                .collect();
            
            let sector_result = sector_coordinator.train_sector(
                &sector_id,
                &sector_symbols,
                &sector_data,
            ).await?;
            
            sector_results.insert(sector_id, sector_result);
        }
        
        // 4. Update model registry
        self.update_model_registry_for_sectors(&sector_results).await?;
        
        Ok(SectorTrainingResults {
            sector_results,
            training_summary: self.generate_training_summary(&sector_results),
        })
    }
    
    // Backward compatibility: redirect old method to new architecture
    pub async fn train_model(
        &self,
        model_name: &str,
        data: &[TimeSeriesData],
    ) -> Result<()> {
        warn!("Using legacy train_model - consider migrating to train_sector_based_models");
        
        // Extract symbol from model_name
        let symbol = self.extract_symbol_from_model_name(model_name);
        
        // Get sector for symbol
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        
        // Create single-symbol training data
        let mut training_data = HashMap::new();
        training_data.insert(symbol.to_string(), data.to_vec());
        
        // Use new sector-based training
        let _results = self.train_sector_based_models(training_data).await?;
        
        Ok(())
    }
}
```

### 5. Configuration Updates

#### 5.1 Sector Training Configuration
```rust
// File: src/config/sector_training.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorTrainingConfig {
    pub sector_model_configs: HashMap<String, SectorModelConfig>,
    pub specialization_config: SpecializationTrainingConfig,
    pub training_params: SectorTrainingParams,
    pub validation_config: SectorValidationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorTrainingParams {
    pub max_epochs: usize,
    pub learning_rate: f32,
    pub batch_size: usize,
    pub early_stopping_patience: usize,
    pub sector_aggregation_method: AggregationMethod,
    pub cross_sector_regularization: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecializationTrainingConfig {
    pub adaptation_layer_config: AdaptationLayerConfig,
    pub adaptation_training_params: AdaptationTrainingParams,
    pub specialization_strength: f32, // How much to adapt from sector model
    pub symbol_embedding_size: usize,
}
```

### 6. Model Storage Updates

#### 6.1 Sector Model Storage Schema
```rust
// File: src/adapters/model_storage.rs (Enhanced)
impl ModelStorage {
    pub async fn save_sector_model(
        &self,
        sector_id: &str,
        sector_model: &SectorModel,
        specialization_layers: &HashMap<String, SymbolSpecializationLayer>,
    ) -> Result<SectorModelVersion> {
        // 1. Save sector base model
        let sector_model_path = self.save_base_sector_model(sector_id, sector_model).await?;
        
        // 2. Save specialization layers
        let mut specialization_paths = HashMap::new();
        for (symbol, layer) in specialization_layers {
            let layer_path = self.save_specialization_layer(
                sector_id,
                symbol,
                layer,
            ).await?;
            specialization_paths.insert(symbol.clone(), layer_path);
        }
        
        // 3. Create sector model manifest
        let manifest = SectorModelManifest {
            sector_id: sector_id.to_string(),
            sector_model_path,
            specialization_paths,
            version: self.generate_sector_model_version(),
            metadata: SectorModelMetadata::from_training_results(/* ... */),
        };
        
        // 4. Save manifest
        self.save_sector_model_manifest(&manifest).await?;
        
        Ok(SectorModelVersion {
            sector_id: sector_id.to_string(),
            version: manifest.version,
            path: self.get_sector_model_directory(sector_id),
        })
    }
}
```

### 7. Training Validation and Testing

#### 7.1 Sector Model Validation
```rust
// File: src/neural/sector_validation.rs
pub struct SectorModelValidator {
    validation_config: SectorValidationConfig,
    test_data_provider: Arc<TestDataProvider>,
}

impl SectorModelValidator {
    pub async fn validate_sector_model(
        &self,
        sector_model: &SectorModel,
        specialization_layers: &HashMap<String, SymbolSpecializationLayer>,
        validation_data: &HashMap<String, Vec<TimeSeriesData>>,
    ) -> Result<SectorValidationResult> {
        // 1. Validate sector model base performance
        let sector_performance = self.validate_sector_base_performance(
            sector_model,
            validation_data,
        ).await?;
        
        // 2. Validate specialization layer performance
        let mut specialization_performance = HashMap::new();
        for (symbol, layer) in specialization_layers {
            let symbol_data = validation_data.get(symbol)
                .ok_or_else(|| anyhow!("No validation data for symbol: {}", symbol))?;
            
            let performance = self.validate_specialization_performance(
                sector_model,
                layer,
                symbol_data,
            ).await?;
            
            specialization_performance.insert(symbol.clone(), performance);
        }
        
        // 3. Validate memory efficiency
        let memory_metrics = self.measure_memory_efficiency(
            sector_model,
            specialization_layers,
        ).await?;
        
        Ok(SectorValidationResult {
            sector_performance,
            specialization_performance,
            memory_metrics,
            overall_score: self.calculate_overall_score(&sector_performance, &specialization_performance),
        })
    }
}
```

## Implementation Timeline

### Week 1: Foundation
- [ ] Create SectorTrainingCoordinator
- [ ] Implement sector feature aggregation
- [ ] Update SectorModel architecture

### Week 2: Specialization Integration
- [ ] Enhance SymbolSpecializationLayer
- [ ] Implement adaptation layer training
- [ ] Create training validation framework

### Week 3: Pipeline Integration
- [ ] Modify VendorPredictor training methods
- [ ] Update model storage for sector models
- [ ] Implement backward compatibility

### Week 4: Testing and Optimization
- [ ] Comprehensive unit testing
- [ ] Performance optimization
- [ ] Memory usage validation

## Success Metrics

1. **Training Efficiency**: 40% reduction in total training time
2. **Memory Usage**: 64% reduction in model memory footprint
3. **Model Count**: Reduce from 100+ to 10 sector models + specialization layers
4. **Accuracy**: Maintain ≥95% of current model performance
5. **Training Stability**: <2% variance in training results

## Risks and Mitigation

### High Risks
1. **Performance Degradation**: Extensive validation and A/B testing
2. **Training Complexity**: Modular implementation with clear interfaces
3. **Memory Regression**: Continuous monitoring during development

### Medium Risks
1. **Integration Complexity**: Comprehensive test coverage
2. **Configuration Complexity**: Clear documentation and examples
3. **Backward Compatibility**: Feature flags and gradual migration

This training refactor plan provides a detailed roadmap for transitioning from per-symbol to sector-based model training while maintaining system stability and improving efficiency.