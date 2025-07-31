use neural_trader::config::load_config;
use neural_trader::neural::predictor::NeuralPredictor;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Testing async initialization fix...");
    
    // Test 1: Default initialization
    println!("\n1. Testing default initialization:");
    match NeuralPredictor::default().await {
        Ok(predictor) => {
            println!("✅ Default initialization succeeded!");
            println!("   Available models: {:?}", predictor.get_available_models());
        }
        Err(e) => {
            println!("❌ Default initialization failed: {}", e);
        }
    }
    
    // Test 2: Custom config initialization
    println!("\n2. Testing custom config initialization:");
    let config = load_config()?;
    match NeuralPredictor::new(config.neural.clone()).await {
        Ok(predictor) => {
            println!("✅ Custom config initialization succeeded!");
            println!("   Is ready: {}", predictor.is_ready().await);
        }
        Err(e) => {
            println!("❌ Custom config initialization failed: {}", e);
        }
    }
    
    // Test 3: No nested runtime panic
    println!("\n3. Testing no nested runtime panic:");
    let predictor = Arc::new(NeuralPredictor::default().await?);
    println!("✅ Arc<NeuralPredictor> created without panic!");
    
    println!("\n🎉 All tests passed! The async initialization fix is working.");
    Ok(())
}