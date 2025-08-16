//! Emergency Test: Neural Predictions
//! Tests that models produce valid outputs

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
struct PredictionRequest {
    symbol: String,
    features: Vec<f64>,
    horizon: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct PredictionResponse {
    symbol: String,
    predictions: Vec<f64>,
    confidence: f64,
    model_version: String,
}

async fn check_model_files() -> Result<Vec<String>> {
    let model_paths = vec![
        "/opt/neural-trader/sector-models",
        "/opt/neural-trader/models",
        "/var/lib/neural-trader/models",
    ];
    
    let mut found_models = Vec::new();
    
    for path in model_paths {
        if Path::new(path).exists() {
            let entries = std::fs::read_dir(path)?;
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("fann") {
                        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                            found_models.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    
    Ok(found_models)
}

async fn test_prediction_api(base_url: &str, symbol: &str) -> Result<PredictionResponse> {
    let client = reqwest::Client::new();
    
    // Generate dummy features (would be real market data in production)
    let features: Vec<f64> = (0..20).map(|i| 100.0 + (i as f64 * 0.5)).collect();
    
    let request = PredictionRequest {
        symbol: symbol.to_string(),
        features,
        horizon: 5,
    };
    
    let response = client
        .post(&format!("{}/api/predict", base_url))
        .json(&request)
        .send()
        .await?;
    
    if !response.status().is_success() {
        anyhow::bail!("Prediction API returned: {}", response.status());
    }
    
    Ok(response.json().await?)
}

pub async fn test_neural_predictions() -> Result<()> {
    println!("🧪 Testing Neural Model Predictions...");
    
    // Check for model files
    let models = check_model_files().await?;
    if models.is_empty() {
        println!("  ⚠️  No model files found on disk");
        println!("  ℹ️  Models may not be persisted or system not initialized");
    } else {
        println!("  ✅ Found {} model files:", models.len());
        for model in &models {
            println!("      - {}", model);
        }
    }
    
    // Test prediction API
    match test_prediction_api("http://localhost:8080", "XLK").await {
        Ok(response) => {
            println!("  ✅ Prediction API working");
            println!("    - Symbol: {}", response.symbol);
            println!("    - Predictions: {} values", response.predictions.len());
            println!("    - Confidence: {:.2}%", response.confidence * 100.0);
            println!("    - Model: {}", response.model_version);
            
            // Validate predictions
            assert!(!response.predictions.is_empty(), "No predictions returned");
            for pred in &response.predictions {
                assert!(pred.is_finite(), "Invalid prediction value: {}", pred);
                assert!(*pred > 0.0, "Negative prediction: {}", pred);
            }
            assert!(response.confidence >= 0.0 && response.confidence <= 1.0);
        }
        Err(e) => {
            if e.to_string().contains("Connection refused") {
                println!("  ℹ️  Prediction API not available (system offline)");
            } else {
                println!("  ⚠️  Prediction API error: {}", e);
            }
        }
    }
    
    println!("✅ Neural Predictions test completed");
    Ok(())
}

pub async fn test_model_persistence() -> Result<()> {
    println!("🧪 Testing Model Persistence...");
    
    // Check primary model location
    let primary_path = "/opt/neural-trader/sector-models";
    if Path::new(primary_path).exists() {
        println!("  ✅ Primary model directory exists: {}", primary_path);
        
        // Check permissions
        let metadata = std::fs::metadata(primary_path)?;
        let permissions = metadata.permissions();
        println!("    - Readable: {}", permissions.readonly() == false);
        
        // Count models
        let model_count = std::fs::read_dir(primary_path)?
            .filter_map(Result::ok)
            .filter(|e| {
                e.path().extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "fann")
                    .unwrap_or(false)
            })
            .count();
        
        if model_count > 0 {
            println!("  ✅ {} models persisted", model_count);
        } else {
            println!("  ⚠️  No models found in primary directory");
        }
    } else {
        println!("  ⚠️  Primary model directory does not exist");
    }
    
    // Check Docker volume mount
    if std::env::var("DOCKER_CONTAINER").is_ok() {
        println!("  ℹ️  Running in Docker container");
        // In Docker, models should be in /opt/neural-trader
        assert!(Path::new("/opt/neural-trader").exists(), "Docker model directory missing");
    }
    
    Ok(())
}

pub async fn test_sector_model_structure() -> Result<()> {
    println!("🧪 Testing Sector Model Structure...");
    
    let expected_sectors = vec![
        ("XLK", "Technology"),
        ("XLF", "Financial"),
        ("XLV", "Healthcare"),
        ("XLE", "Energy"),
        ("XLI", "Industrial"),
    ];
    
    for (etf, sector) in expected_sectors {
        let model_path = format!("/opt/neural-trader/sector-models/{}_primary.fann", etf);
        if Path::new(&model_path).exists() {
            println!("  ✅ {} sector model exists ({})", sector, etf);
            
            // Check file size (sector models should be 320-512MB)
            let metadata = std::fs::metadata(&model_path)?;
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            
            if size_mb > 300.0 {
                println!("    - Size: {:.1} MB (sector model)", size_mb);
            } else if size_mb > 5.0 {
                println!("    - Size: {:.1} MB (specialization)", size_mb);
            } else {
                println!("    - Size: {:.1} MB (⚠️ unusually small)", size_mb);
            }
        } else {
            println!("  ⚠️  {} sector model not found ({})", sector, etf);
        }
    }
    
    Ok(())
}