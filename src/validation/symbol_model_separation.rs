//! Symbol and Model Type Separation Validation
//!
//! This module ensures that trading symbols (AAPL, NVDA, XLF) and model architecture names 
//! (Transformer, LSTM) are properly separated throughout the system.

use anyhow::{anyhow, Result};
use std::collections::HashSet;
use tracing::{debug, info, warn, error};
use regex::Regex;

/// Validator for symbol and model type separation
pub struct SymbolModelSeparationValidator {
    trading_symbols: HashSet<String>,
    model_architectures: HashSet<String>,
    etf_symbols: HashSet<String>,
    symbol_pattern: Regex,
    model_pattern: Regex,
}

impl SymbolModelSeparationValidator {
    /// Create new validator with predefined symbol and model lists
    pub fn new() -> Result<Self> {
        // Real trading symbols (major US equities)
        let trading_symbols: HashSet<String> = [
            // Major Tech
            "AAPL", "MSFT", "GOOGL", "GOOG", "AMZN", "META", "TSLA", "NVDA", "NFLX", "ADBE",
            "CRM", "ORCL", "INTC", "AMD", "QCOM", "AVGO", "TXN", "CSCO", "ACN", "IBM",
            
            // Major Financial
            "JPM", "BAC", "WFC", "GS", "MS", "C", "USB", "PNC", "TFC", "COF", "AXP", "BLK",
            "SCHW", "CB", "MMC", "ICE", "CME", "SPGI", "MCO", "AON", "TRV", "ALL",
            
            // Major Healthcare
            "JNJ", "PFE", "UNH", "MRK", "ABT", "TMO", "DHR", "BMY", "LLY", "AMGN", "GILD",
            "BIIB", "REGN", "VRTX", "ISRG", "ZTS", "ILMN", "DXCM", "EW", "ALGN",
            
            // Major Energy
            "XOM", "CVX", "COP", "EOG", "SLB", "MPC", "VLO", "PSX", "KMI", "OKE", "WMB",
            "TRGP", "EPD", "ET", "ENB", "TC", "PPL", "SO", "NEE", "DUK",
            
            // Major Consumer
            "HD", "MCD", "NKE", "SBUX", "TJX", "LOW", "F", "GM", "MAR", "HLT", "DIS", "CMCSA",
            "PG", "KO", "PEP", "WMT", "COST", "CL", "KMB", "GIS", "K", "HSY", "MO", "PM",
            
            // Major Industrial
            "BA", "CAT", "GE", "MMM", "HON", "UPS", "LMT", "RTX", "DE", "EMR", "ETN", "ITW",
            "PH", "CMI", "CARR", "OTIS", "GD", "NOC", "LHX", "HWM",
            
            // Materials & Utilities
            "DOW", "DD", "APD", "ECL", "SHW", "NEM", "FCX", "AA", "X", "CF", "MOS", "IFF",
            "NEE", "DUK", "SO", "AEP", "EXC", "XEL", "PEG", "SRE", "D", "PCG", "ES", "FE",
            
            // REITs
            "AMT", "PLD", "CCI", "EQIX", "PSA", "EQR", "AVB", "ESS", "MAA", "UDR", "SPG", "O",
        ].iter().map(|s| s.to_string()).collect();
        
        // ETF symbols (sector and broad market)
        let etf_symbols: HashSet<String> = [
            // Sector ETFs
            "XLK", "XLF", "XLV", "XLE", "XLY", "XLP", "XLI", "XLB", "XLU", "XLRE",
            // Broad Market ETFs
            "SPY", "QQQ", "IWM", "VTI", "VOO", "VEA", "VWO", "BND", "AGG", "LQD",
            // Factor ETFs
            "VUG", "VTV", "VBR", "VBK", "MTUM", "QUAL", "SIZE", "USMV",
        ].iter().map(|s| s.to_string()).collect();
        
        // Model architecture names (should never be treated as symbols)
        let model_architectures: HashSet<String> = [
            // Deep Learning Architectures
            "Transformer", "LSTM", "GRU", "RNN", "CNN", "MLP", "TCN", "ResNet", "DenseNet",
            
            // Time Series Specific Models
            "DeepAR", "NHITS", "TFT", "N-BEATS", "Informer", "Autoformer", "FEDformer",
            
            // Traditional ML Models
            "ARIMA", "Prophet", "XGBoost", "LightGBM", "RandomForest", "SVM", "LinearRegression",
            
            // Ensemble Methods
            "VotingClassifier", "BaggingRegressor", "AdaBoost", "GradientBoosting",
            
            // Emergency/Fallback Models
            "EmergencyModel", "FallbackModel", "BaseModel", "EnsembleModel", "SimpleMovingAverage",
            
            // Neural Network Components
            "Attention", "MultiHeadAttention", "PositionalEncoding", "LayerNorm", "Dropout",
        ].iter().map(|s| s.to_string()).collect();
        
        // Create regex patterns for validation
        let symbol_pattern = Regex::new(r"^[A-Z]{1,5}$")?; // 1-5 uppercase letters
        let model_pattern = Regex::new(r"^[A-Za-z][A-Za-z0-9_-]*$")?; // Alphanumeric with underscores/dashes
        
        Ok(Self {
            trading_symbols,
            model_architectures,
            etf_symbols,
            symbol_pattern,
            model_pattern,
        })
    }
    
    /// Validate that a string is a trading symbol, not a model type
    pub fn validate_trading_symbol(&self, input: &str) -> Result<()> {
        let normalized = input.to_uppercase();
        
        // Check if it's a known trading symbol or ETF
        if self.trading_symbols.contains(&normalized) || self.etf_symbols.contains(&normalized) {
            debug!("✅ '{}' validated as trading symbol", input);
            return Ok(());
        }
        
        // Check if it looks like a symbol (format validation)
        if !self.symbol_pattern.is_match(&normalized) {
            return Err(anyhow!("'{}' does not match trading symbol format", input));
        }
        
        // Check if it's accidentally a model type
        if self.model_architectures.contains(input) || self.model_architectures.contains(&normalized) {
            return Err(anyhow!("'{}' is a model architecture, not a trading symbol", input));
        }
        
        // It's not in our known list but has valid format - allow with warning
        warn!("⚠️ '{}' is not in known trading symbols but has valid format", input);
        Ok(())
    }
    
    /// Validate that a string is a model architecture, not a trading symbol
    pub fn validate_model_architecture(&self, input: &str) -> Result<()> {
        // Check if it's a known model architecture
        if self.model_architectures.contains(input) {
            debug!("✅ '{}' validated as model architecture", input);
            return Ok(());
        }
        
        // Check if it looks like a model name (format validation)
        if !self.model_pattern.is_match(input) {
            return Err(anyhow!("'{}' does not match model architecture format", input));
        }
        
        // Check if it's accidentally a trading symbol
        let normalized = input.to_uppercase();
        if self.trading_symbols.contains(&normalized) || self.etf_symbols.contains(&normalized) {
            return Err(anyhow!("'{}' is a trading symbol, not a model architecture", input));
        }
        
        // It's not in our known list but has valid format - allow with warning
        warn!("⚠️ '{}' is not in known model architectures but has valid format", input);
        Ok(())
    }
    
    /// Validate symbol context (should only receive trading symbols)
    pub fn validate_symbol_context(&self, symbols: &[String], context: &str) -> Result<()> {
        info!("🔍 Validating {} symbols in context: {}", symbols.len(), context);
        
        let mut validation_errors = Vec::new();
        let mut symbol_count = 0;
        let mut model_type_count = 0;
        
        for symbol in symbols {
            match self.validate_trading_symbol(symbol) {
                Ok(_) => {
                    symbol_count += 1;
                    debug!("✅ Symbol '{}' valid in context '{}'", symbol, context);
                }
                Err(e) => {
                    // Check if it's accidentally a model type
                    if self.model_architectures.contains(symbol) {
                        model_type_count += 1;
                        validation_errors.push(format!(
                            "CRITICAL: Model architecture '{}' found in symbol context '{}' - this is incorrect!",
                            symbol, context
                        ));
                    } else {
                        validation_errors.push(format!(
                            "Symbol validation failed for '{}' in context '{}': {}",
                            symbol, context, e
                        ));
                    }
                }
            }
        }
        
        // Report results
        info!("📊 Validation results for context '{}':", context);
        info!("  ✅ Valid symbols: {}", symbol_count);
        info!("  ❌ Model types incorrectly used as symbols: {}", model_type_count);
        info!("  ⚠️ Other validation errors: {}", validation_errors.len() - model_type_count);
        
        if !validation_errors.is_empty() {
            error!("Validation errors in context '{}':", context);
            for error in &validation_errors {
                error!("  - {}", error);
            }
            return Err(anyhow!("Symbol context validation failed with {} errors", validation_errors.len()));
        }
        
        Ok(())
    }
    
    /// Validate model context (should only receive model architectures)
    pub fn validate_model_context(&self, models: &[String], context: &str) -> Result<()> {
        info!("🔍 Validating {} models in context: {}", models.len(), context);
        
        let mut validation_errors = Vec::new();
        let mut model_count = 0;
        let mut symbol_count = 0;
        
        for model in models {
            match self.validate_model_architecture(model) {
                Ok(_) => {
                    model_count += 1;
                    debug!("✅ Model '{}' valid in context '{}'", model, context);
                }
                Err(e) => {
                    // Check if it's accidentally a trading symbol
                    let normalized = model.to_uppercase();
                    if self.trading_symbols.contains(&normalized) || self.etf_symbols.contains(&normalized) {
                        symbol_count += 1;
                        validation_errors.push(format!(
                            "CRITICAL: Trading symbol '{}' found in model context '{}' - this is incorrect!",
                            model, context
                        ));
                    } else {
                        validation_errors.push(format!(
                            "Model validation failed for '{}' in context '{}': {}",
                            model, context, e
                        ));
                    }
                }
            }
        }
        
        // Report results
        info!("📊 Validation results for context '{}':", context);
        info!("  ✅ Valid models: {}", model_count);
        info!("  ❌ Trading symbols incorrectly used as models: {}", symbol_count);
        info!("  ⚠️ Other validation errors: {}", validation_errors.len() - symbol_count);
        
        if !validation_errors.is_empty() {
            error!("Validation errors in context '{}':", context);
            for error in &validation_errors {
                error!("  - {}", error);
            }
            return Err(anyhow!("Model context validation failed with {} errors", validation_errors.len()));
        }
        
        Ok(())
    }
    
    /// Detect potential confusion between symbols and models in text
    pub fn detect_symbol_model_confusion(&self, text: &str, context: &str) -> Vec<String> {
        let mut warnings = Vec::new();
        
        // Look for model architectures used where symbols are expected
        for model_arch in &self.model_architectures {
            if text.contains(model_arch) {
                // Check if context suggests this should be a symbol
                if context.contains("symbol") || context.contains("sector_mapper") || 
                   context.contains("trading") || context.contains("market") {
                    warnings.push(format!(
                        "Model architecture '{}' found in symbol context '{}' - potential confusion",
                        model_arch, context
                    ));
                }
            }
        }
        
        // Look for trading symbols used where models are expected
        for symbol in &self.trading_symbols {
            if text.contains(symbol) {
                // Check if context suggests this should be a model
                if context.contains("model") || context.contains("architecture") || 
                   context.contains("prediction") || context.contains("neural") {
                    warnings.push(format!(
                        "Trading symbol '{}' found in model context '{}' - potential confusion",
                        symbol, context
                    ));
                }
            }
        }
        
        warnings
    }
    
    /// Get all known trading symbols
    pub fn get_trading_symbols(&self) -> &HashSet<String> {
        &self.trading_symbols
    }
    
    /// Get all known model architectures
    pub fn get_model_architectures(&self) -> &HashSet<String> {
        &self.model_architectures
    }
    
    /// Get all known ETF symbols
    pub fn get_etf_symbols(&self) -> &HashSet<String> {
        &self.etf_symbols
    }
    
    /// Check if input is a trading symbol
    pub fn is_trading_symbol(&self, input: &str) -> bool {
        let normalized = input.to_uppercase();
        self.trading_symbols.contains(&normalized) || self.etf_symbols.contains(&normalized)
    }
    
    /// Check if input is a model architecture
    pub fn is_model_architecture(&self, input: &str) -> bool {
        self.model_architectures.contains(input)
    }
    
    /// Generate validation report
    pub fn generate_validation_report(&self) -> String {
        format!(
            "Symbol-Model Separation Validator Report\n\
             =====================================\n\
             Trading Symbols: {} (including {} ETFs)\n\
             Model Architectures: {}\n\
             \n\
             Use validate_trading_symbol() for symbol contexts\n\
             Use validate_model_architecture() for model contexts\n\
             Use detect_symbol_model_confusion() for general text analysis",
            self.trading_symbols.len(),
            self.etf_symbols.len(),
            self.model_architectures.len()
        )
    }
}

impl Default for SymbolModelSeparationValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create default SymbolModelSeparationValidator")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_trading_symbol_validation() {
        let validator = SymbolModelSeparationValidator::new().unwrap();
        
        // Valid trading symbols
        assert!(validator.validate_trading_symbol("AAPL").is_ok());
        assert!(validator.validate_trading_symbol("NVDA").is_ok());
        assert!(validator.validate_trading_symbol("XLF").is_ok());
        
        // Invalid (model architectures)
        assert!(validator.validate_trading_symbol("Transformer").is_err());
        assert!(validator.validate_trading_symbol("LSTM").is_err());
        assert!(validator.validate_trading_symbol("MLP").is_err());
    }
    
    #[test]
    fn test_model_architecture_validation() {
        let validator = SymbolModelSeparationValidator::new().unwrap();
        
        // Valid model architectures
        assert!(validator.validate_model_architecture("Transformer").is_ok());
        assert!(validator.validate_model_architecture("LSTM").is_ok());
        assert!(validator.validate_model_architecture("MLP").is_ok());
        
        // Invalid (trading symbols)
        assert!(validator.validate_model_architecture("AAPL").is_err());
        assert!(validator.validate_model_architecture("NVDA").is_err());
        assert!(validator.validate_model_architecture("XLF").is_err());
    }
    
    #[test]
    fn test_symbol_context_validation() {
        let validator = SymbolModelSeparationValidator::new().unwrap();
        
        // Valid symbol context
        let valid_symbols = vec!["AAPL".to_string(), "NVDA".to_string(), "XLF".to_string()];
        assert!(validator.validate_symbol_context(&valid_symbols, "sector_mapper").is_ok());
        
        // Invalid symbol context (contains model types)
        let invalid_symbols = vec!["AAPL".to_string(), "Transformer".to_string(), "LSTM".to_string()];
        assert!(validator.validate_symbol_context(&invalid_symbols, "sector_mapper").is_err());
    }
    
    #[test]
    fn test_model_context_validation() {
        let validator = SymbolModelSeparationValidator::new().unwrap();
        
        // Valid model context
        let valid_models = vec!["Transformer".to_string(), "LSTM".to_string(), "MLP".to_string()];
        assert!(validator.validate_model_context(&valid_models, "neural_predictor").is_ok());
        
        // Invalid model context (contains symbols)
        let invalid_models = vec!["LSTM".to_string(), "AAPL".to_string(), "NVDA".to_string()];
        assert!(validator.validate_model_context(&invalid_models, "neural_predictor").is_err());
    }
    
    #[test]
    fn test_confusion_detection() {
        let validator = SymbolModelSeparationValidator::new().unwrap();
        
        // Text with model in symbol context
        let warnings1 = validator.detect_symbol_model_confusion(
            "sector_mapper.get_sector('Transformer')", 
            "symbol_processing"
        );
        assert!(!warnings1.is_empty());
        
        // Text with symbol in model context
        let warnings2 = validator.detect_symbol_model_confusion(
            "neural_model = AAPL_predictor.load()", 
            "model_loading"
        );
        assert!(!warnings2.is_empty());
    }
}