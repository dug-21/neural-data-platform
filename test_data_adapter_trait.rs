// Test file to verify DataAdapter trait implementation compiles
use std::sync::Arc;

// Import the necessary modules
#[path = "src/adapters/mod.rs"]
mod adapters;

#[path = "src/adapters/enhanced_neural_adapter.rs"]
mod enhanced_neural_adapter;

#[path = "src/config.rs"]
mod config;

#[path = "src/neural/fann_predictor.rs"]
mod fann_predictor;

#[path = "src/data/mod.rs"]
mod data;

#[path = "src/neural/mod.rs"]
mod neural;

use adapters::{DataAdapter, AdapterMetadata, ConnectionStatus};
use enhanced_neural_adapter::{EnhancedNeuralAdapter, EnhancedNeuralConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing DataAdapter trait implementation...");
    
    // Create a config with minimal settings to avoid missing fields
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    };
    
    // Create the enhanced neural adapter
    let mut adapter = EnhancedNeuralAdapter::new(config).await?;
    
    // Test DataAdapter trait methods
    println!("Testing DataAdapter trait methods:");
    
    // Test name() method
    let name = adapter.name();
    println!("Adapter name: {}", name);
    assert_eq!(name, "EnhancedNeuralAdapter");
    
    // Test is_connected() method
    let connected = adapter.is_connected();
    println!("Initially connected: {}", connected);
    
    // Test connect() method
    adapter.connect().await?;
    println!("Connected successfully");
    assert!(adapter.is_connected());
    
    // Test metadata() method
    let metadata = adapter.metadata();
    println!("Metadata: {:?}", metadata);
    assert_eq!(metadata.name, "EnhancedNeuralAdapter");
    assert_eq!(metadata.version, "1.0.0");
    assert_eq!(metadata.adapter_type, "neural");
    assert!(matches!(metadata.connection_status, ConnectionStatus::Connected));
    
    // Test disconnect() method
    adapter.disconnect().await?;
    println!("Disconnected successfully");
    assert!(!adapter.is_connected());
    
    println!("✅ All DataAdapter trait methods implemented and working correctly!");
    
    Ok(())
}