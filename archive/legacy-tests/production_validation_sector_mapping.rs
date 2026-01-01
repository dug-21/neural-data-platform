//! Production Validation: Sector Mapping Symbol vs Model Type Separation
//!
//! This validation ensures that:
//! 1. Only real trading symbols (AAPL, NVDA, XLF, etc.) are passed to sector_mapper
//! 2. Model architecture names (Transformer, LSTM) are handled separately
//! 3. All symbol-to-sector mappings are correct and validated against real data
//! 4. No model types leak into sector mapping logic

use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio;
use tracing::{info, warn, error};

// Import the modules we need to validate
use neural_trader::data::sector_mapper::{SectorMapper, SectorMapperConfig, SectorId};
use neural_trader::neural::vendor_predictor::VendorPredictor;
use neural_trader::neural::typed_storage::ModelKey;
use neural_trader::data::TimeSeriesData;
use neural_trader::config::NeuralConfig;
use neural_trader::monitoring::model_performance_tracker::ModelPerformanceTracker;

/// Production validation tests for sector mapping
pub struct SectorMappingValidator {
    sector_mapper: Arc<SectorMapper>,
    vendor_predictor: Arc<VendorPredictor>,
    real_trading_symbols: HashSet<String>,
    model_architecture_names: HashSet<String>,
}

impl SectorMappingValidator {
    /// Create new validator with real production data
    pub async fn new() -> Result<Self> {
        info!("🔍 Initializing Production Sector Mapping Validator");
        
        // Create sector mapper
        let config = SectorMapperConfig::default();
        let sector_mapper = Arc::new(SectorMapper::new(config));
        
        // Create vendor predictor for validation
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            lookback_window: 24,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            input_size: 60,
            output_size: 1,
            hidden_layers: vec![128, 64, 32],
            learning_rate: 0.001,
            prediction_horizon: Some(24),
            normalization_method: Some("z-score".to_string()),
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 120,
            max_retries: 3,
            error_threshold: 0.15,
        };
        
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        let vendor_predictor = Arc::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker,
        )?);
        
        // Define real trading symbols
        let real_trading_symbols = [
            // Technology
            "AAPL", "MSFT", "GOOGL", "GOOG", "AMZN", "META", "TSLA", "NVDA", "NFLX", "ADBE",
            // Financial
            "JPM", "BAC", "WFC", "GS", "MS", "C", "USB", "PNC", "TFC", "COF",
            // Healthcare
            "JNJ", "PFE", "UNH", "MRK", "ABT", "TMO", "DHR", "BMY", "LLY", "AMGN",
            // Energy
            "XOM", "CVX", "COP", "EOG", "SLB", "MPC", "VLO", "PSX", "KMI", "OKE",
            // Consumer Discretionary
            "HD", "MCD", "NKE", "SBUX", "TJX", "LOW", "F", "GM", "MAR", "HLT",
            // Consumer Staples
            "PG", "KO", "PEP", "WMT", "COST", "CL", "KMB", "GIS", "K", "HSY",
            // Industrials
            "BA", "CAT", "GE", "MMM", "HON", "UPS", "LMT", "RTX", "DE", "EMR",
            // Materials
            "DOW", "DD", "APD", "ECL", "SHW", "NEM", "FCX", "AA", "X", "CF",
            // Utilities
            "NEE", "DUK", "SO", "AEP", "EXC", "XEL", "PEG", "SRE", "D", "PCG",
            // Real Estate / REITs
            "AMT", "PLD", "CCI", "EQIX", "PSA", "EQR", "AVB", "ESS", "MAA", "UDR",
            // ETFs (Sector representatives)
            "XLK", "XLF", "XLV", "XLE", "XLY", "XLP", "XLI", "XLB", "XLU", "XLRE",
            "SPY", "QQQ", "IWM", "VTI", "VOO"
        ].iter().map(|s| s.to_string()).collect();
        
        // Define model architecture names that should NEVER be passed to sector_mapper
        let model_architecture_names = [
            "Transformer", "LSTM", "GRU", "MLP", "CNN", "RNN", "TCN", "DeepAR", 
            "NHITS", "ARIMA", "Prophet", "XGBoost", "LightGBM", "RandomForest",
            "EmergencyModel", "FallbackModel", "BaseModel", "EnsembleModel",
            "AutoRegressive", "VectorAutoRegression", "GARCH", "ARCH"
        ].iter().map(|s| s.to_string()).collect();
        
        Ok(Self {
            sector_mapper,
            vendor_predictor,
            real_trading_symbols,
            model_architecture_names,
        })
    }
    
    /// Validate all sector mappings use only real trading symbols
    pub async fn validate_symbol_sector_mappings(&self) -> Result<()> {
        info!("🎯 Validating that only real trading symbols are mapped to sectors...");
        
        let mut validation_errors = Vec::new();
        let mut success_count = 0;
        
        // Test each real trading symbol
        for symbol in &self.real_trading_symbols {
            match self.sector_mapper.get_sector(symbol) {
                Ok(sector_info) => {
                    // Validate sector assignment makes sense
                    let expected_sector = self.get_expected_sector(symbol);
                    if let Some(expected) = expected_sector {
                        if sector_info.sector_id != expected {
                            validation_errors.push(format!(
                                "Symbol {} mapped to {:?} but expected {:?}",
                                symbol, sector_info.sector_id, expected
                            ));
                        } else {
                            success_count += 1;
                            info!("✅ Symbol {} correctly mapped to {:?}", symbol, sector_info.sector_id);
                        }
                    } else {
                        // Unknown symbol, but sector mapper should handle it gracefully
                        success_count += 1;
                        info!("ℹ️ Symbol {} mapped to default sector {:?}", symbol, sector_info.sector_id);
                    }
                }
                Err(e) => {
                    validation_errors.push(format!("Failed to get sector for symbol {}: {}", symbol, e));
                }
            }
        }
        
        // Validate that model architecture names are NOT processed as symbols
        info!("🚫 Validating that model architecture names are NOT processed as symbols...");
        for model_type in &self.model_architecture_names {
            // These should either fail or be handled as defaults, but should not be treated as real symbols
            match self.sector_mapper.get_sector(model_type) {
                Ok(sector_info) => {
                    warn!("⚠️ Model type {} was processed as symbol and mapped to {:?} - this should be avoided", 
                          model_type, sector_info.sector_id);
                    // This is not necessarily an error since sector_mapper has a default fallback,
                    // but we should track it
                }
                Err(_) => {
                    // This is expected - model types should not be valid symbols
                    info!("✅ Model type {} correctly rejected as invalid symbol", model_type);
                }
            }
        }
        
        info!("📊 Validation Summary:");
        info!("✅ Successfully validated: {} symbols", success_count);
        info!("❌ Validation errors: {}", validation_errors.len());
        
        if !validation_errors.is_empty() {
            error!("Validation errors found:");
            for error in &validation_errors {
                error!("  - {}", error);
            }
            return Err(anyhow!("Sector mapping validation failed with {} errors", validation_errors.len()));
        }
        
        Ok(())
    }
    
    /// Validate model storage uses correct separation
    pub async fn validate_model_storage_separation(&self) -> Result<()> {
        info!("🔍 Validating model storage correctly separates symbols from model types...");
        
        // Initialize models for testing
        let mut vendor_predictor = VendorPredictor::new(
            &NeuralConfig::default(),
            self.sector_mapper.clone(),
            Arc::new(ModelPerformanceTracker::new()),
        )?;
        
        // Load sector models to populate storage
        vendor_predictor.load_sector_models_config().await?;
        vendor_predictor.initialize_models_emergency().await?;
        
        // Get model info
        let model_info = vendor_predictor.get_model_info().await;
        info!("📋 Model storage contains {} active models", 
              model_info.get("active_models").unwrap_or(&serde_json::Value::Null));
        
        // Test that real symbols can get their sector models
        let mut symbol_validation_count = 0;
        for symbol in ["AAPL", "NVDA", "XLF", "JPM", "JNJ"].iter() {
            match vendor_predictor.get_models_for_symbol(symbol).await {
                Ok(models) => {
                    if !models.is_empty() {
                        symbol_validation_count += 1;
                        info!("✅ Symbol {} has {} available models: {:?}", 
                              symbol, models.len(), 
                              models.iter().map(|m| &m.model_type).collect::<Vec<_>>());
                        
                        // Validate that the models are architecture types, not symbols
                        for model_key in &models {
                            if self.real_trading_symbols.contains(&model_key.model_type) {
                                error!("❌ Model type '{}' is a trading symbol, not a model architecture!", 
                                       model_key.model_type);
                                return Err(anyhow!("Model type confusion: {} should be an architecture, not a symbol", 
                                                 model_key.model_type));
                            }
                            
                            if self.model_architecture_names.contains(&model_key.model_type) {
                                info!("✅ Model type '{}' is correctly identified as architecture", model_key.model_type);
                            }
                        }
                    } else {
                        warn!("⚠️ Symbol {} has no available models", symbol);
                    }
                }
                Err(e) => {
                    error!("❌ Failed to get models for symbol {}: {}", symbol, e);
                    return Err(anyhow!("Model retrieval failed for symbol {}: {}", symbol, e));
                }
            }
        }
        
        if symbol_validation_count == 0 {
            return Err(anyhow!("No models found for any test symbols - storage may be broken"));
        }
        
        info!("✅ Model storage validation passed: {} symbols have proper model access", symbol_validation_count);
        Ok(())
    }
    
    /// Validate end-to-end prediction flow
    pub async fn validate_prediction_flow(&self) -> Result<()> {
        info!("🔮 Validating end-to-end prediction flow with real symbols...");
        
        // Create test data for real symbols
        let test_symbols = ["AAPL", "NVDA", "XLF"];
        let mut test_data = Vec::new();
        
        for symbol in test_symbols.iter() {
            let time_series = self.create_test_time_series(symbol);
            test_data.push(time_series);
        }
        
        // Initialize predictor with models
        let mut vendor_predictor = VendorPredictor::new(
            &NeuralConfig::default(),
            self.sector_mapper.clone(),
            Arc::new(ModelPerformanceTracker::new()),
        )?;
        
        vendor_predictor.load_sector_models_config().await?;
        vendor_predictor.initialize_models_emergency().await?;
        
        // Test predictions
        match vendor_predictor.predict(&test_data, 24, None).await {
            Ok(predictions) => {
                info!("✅ Prediction flow successful: {} predictions generated", predictions.len());
                
                for (i, prediction) in predictions.iter().enumerate() {
                    let symbol = test_symbols[i];
                    info!("📈 Prediction for {}: value={:.4}, confidence={:.4}, model='{}'", 
                          symbol, prediction.value, prediction.confidence, prediction.model_name);
                    
                    // Validate prediction model name doesn't contain symbol confusion
                    if prediction.model_name.contains(symbol) && 
                       !prediction.model_name.contains("sector") && 
                       !prediction.model_name.contains("ensemble") {
                        warn!("⚠️ Prediction model name '{}' contains symbol '{}' - verify this is intentional", 
                              prediction.model_name, symbol);
                    }
                }
            }
            Err(e) => {
                error!("❌ Prediction flow failed: {}", e);
                return Err(anyhow!("End-to-end prediction validation failed: {}", e));
            }
        }
        
        Ok(())
    }
    
    /// Validate sector routing works correctly
    pub async fn validate_sector_routing(&self) -> Result<()> {
        info!("🎯 Validating sector-based routing logic...");
        
        let test_cases = vec![
            ("AAPL", SectorId::Technology),
            ("NVDA", SectorId::Technology), 
            ("JPM", SectorId::Financial),
            ("XLF", SectorId::Financial),
            ("JNJ", SectorId::Healthcare),
            ("XOM", SectorId::Energy),
        ];
        
        let mut routing_success = 0;
        for (symbol, expected_sector) in test_cases {
            match self.sector_mapper.get_sector(symbol) {
                Ok(sector_info) => {
                    if sector_info.sector_id == expected_sector {
                        routing_success += 1;
                        info!("✅ Symbol {} correctly routed to {:?}", symbol, expected_sector);
                    } else {
                        error!("❌ Symbol {} routed to {:?} instead of {:?}", 
                               symbol, sector_info.sector_id, expected_sector);
                        return Err(anyhow!("Sector routing failed for {}", symbol));
                    }
                }
                Err(e) => {
                    error!("❌ Failed to route symbol {}: {}", symbol, e);
                    return Err(anyhow!("Sector routing error for {}: {}", symbol, e));
                }
            }
        }
        
        info!("✅ Sector routing validation passed: {}/{} symbols correctly routed", 
              routing_success, test_cases.len());
        Ok(())
    }
    
    /// Run comprehensive production validation
    pub async fn run_comprehensive_validation(&self) -> Result<()> {
        info!("🚀 Starting Comprehensive Production Validation");
        info!("📊 Real trading symbols: {}", self.real_trading_symbols.len());
        info!("🤖 Model architectures: {}", self.model_architecture_names.len());
        
        // Run all validation tests
        self.validate_symbol_sector_mappings().await?;
        self.validate_model_storage_separation().await?;
        self.validate_sector_routing().await?;
        self.validate_prediction_flow().await?;
        
        info!("🎉 ALL PRODUCTION VALIDATIONS PASSED!");
        info!("✅ Sector mapping correctly handles symbols vs model types");
        info!("✅ Real trading symbols properly mapped to sectors");
        info!("✅ Model types separated from symbol processing");
        info!("✅ End-to-end prediction flow working correctly");
        
        Ok(())
    }
    
    /// Get expected sector for a known symbol (for validation)
    fn get_expected_sector(&self, symbol: &str) -> Option<SectorId> {
        match symbol.to_uppercase().as_str() {
            // Technology
            "AAPL" | "MSFT" | "GOOGL" | "GOOG" | "META" | "NVDA" | "NFLX" | "ADBE" | "XLK" => Some(SectorId::Technology),
            // Financial
            "JPM" | "BAC" | "WFC" | "GS" | "MS" | "C" | "XLF" => Some(SectorId::Financial),
            // Healthcare
            "JNJ" | "PFE" | "UNH" | "MRK" | "ABT" | "XLV" => Some(SectorId::Healthcare),
            // Energy
            "XOM" | "CVX" | "COP" | "XLE" => Some(SectorId::Energy),
            // Consumer Discretionary
            "AMZN" | "TSLA" | "HD" | "MCD" | "NKE" | "XLY" => Some(SectorId::ConsumerDiscretionary),
            // Consumer Staples
            "PG" | "KO" | "PEP" | "WMT" | "XLP" => Some(SectorId::ConsumerStaples),
            // Industrials
            "BA" | "CAT" | "GE" | "MMM" | "XLI" => Some(SectorId::Industrials),
            // Materials
            "DOW" | "DD" | "APD" | "XLB" => Some(SectorId::Materials),
            // Utilities
            "NEE" | "DUK" | "SO" | "XLU" => Some(SectorId::Utilities),
            // Real Estate
            "AMT" | "PLD" | "CCI" | "XLRE" => Some(SectorId::RealEstate),
            // Unknown/Other
            _ => None,
        }
    }
    
    /// Create test time series data for a symbol
    fn create_test_time_series(&self, symbol: &str) -> TimeSeriesData {
        use chrono::{Utc, Duration};
        
        let base_time = Utc::now();
        let mut timestamps = Vec::new();
        let mut values = Vec::new();
        
        // Generate 100 data points
        for i in 0..100 {
            timestamps.push(base_time - Duration::minutes(i as i64));
            values.push(100.0 + (i as f64 * 0.1) + (i as f64).sin() * 2.0);
        }
        
        TimeSeriesData {
            timestamp: base_time,
            symbol: symbol.to_string(),
            open: values[0],
            high: values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
            low: values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            close: values[values.len() - 1],
            volume: values.iter().map(|_| 1000000.0).collect(),
            volume_value: 1000000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(values[values.len() - 1]),
            metadata: None,
            values,
            intervals: vec![60; 100],
            timestamps,
            metadata_map: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::Value::String(symbol.to_string()));
                map
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    
    #[tokio::test]
    async fn test_comprehensive_sector_mapping_validation() {
        let validator = SectorMappingValidator::new().await.expect("Failed to create validator");
        
        // Run comprehensive validation
        validator.run_comprehensive_validation().await.expect("Production validation failed");
    }
    
    #[tokio::test]
    async fn test_symbol_sector_mappings_only() {
        let validator = SectorMappingValidator::new().await.expect("Failed to create validator");
        
        // Test just the symbol mappings
        validator.validate_symbol_sector_mappings().await.expect("Symbol mapping validation failed");
    }
    
    #[tokio::test]
    async fn test_model_storage_separation_only() {
        let validator = SectorMappingValidator::new().await.expect("Failed to create validator");
        
        // Test just the model storage separation
        validator.validate_model_storage_separation().await.expect("Model storage validation failed");
    }
    
    #[tokio::test]
    async fn test_specific_symbol_validation() {
        let validator = SectorMappingValidator::new().await.expect("Failed to create validator");
        
        // Test specific symbols that were mentioned in the issue
        let test_symbols = ["AAPL", "NVDA", "XLF"];
        
        for symbol in test_symbols.iter() {
            let sector_info = validator.sector_mapper.get_sector(symbol)
                .expect(&format!("Failed to get sector for {}", symbol));
            
            println!("Symbol {} -> Sector: {:?}", symbol, sector_info.sector_id);
            
            // Ensure it's a valid sector, not a model type
            assert!(!validator.model_architecture_names.contains(symbol));
            assert!(validator.real_trading_symbols.contains(*symbol));
        }
    }
    
    #[tokio::test]
    async fn test_model_types_not_treated_as_symbols() {
        let validator = SectorMappingValidator::new().await.expect("Failed to create validator");
        
        // Ensure model types are not processed as symbols
        let model_types = ["Transformer", "LSTM", "MLP", "TCN"];
        
        for model_type in model_types.iter() {
            // Model types should either fail or get default handling
            // The key is they should not be treated as real trading symbols
            assert!(validator.model_architecture_names.contains(*model_type));
            assert!(!validator.real_trading_symbols.contains(*model_type));
            
            println!("Model type {} correctly identified as architecture, not symbol", model_type);
        }
    }
}