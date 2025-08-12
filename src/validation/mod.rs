//! Production Validation Module
//!
//! This module contains comprehensive validation tools for ensuring the neural trader
//! system is production-ready and correctly separates symbols from model types.

pub mod symbol_model_separation;

pub use symbol_model_separation::SymbolModelSeparationValidator;

use anyhow::Result;
use tracing::info;

/// Run comprehensive production validation
pub async fn run_production_validation() -> Result<()> {
    info!("🚀 Starting comprehensive production validation");
    
    // Initialize validator
    let validator = SymbolModelSeparationValidator::new()?;
    
    // Generate and log report
    let report = validator.generate_validation_report();
    info!("{}", report);
    
    // Test basic functionality
    info!("✅ Production validation module initialized successfully");
    
    Ok(())
}