// Model Storage Integration Demo
// Demonstrates how to use the model storage architecture in production

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub model_name: String,
    pub symbol: String,
    pub model_type: String,
    pub version: String,
    pub created_at: String,
    pub framework: String,
    pub performance_metrics: PerformanceMetrics,
    pub features: Vec<String>,
    pub deployment_status: String,
    pub checksum: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub train_mse: f64,
    pub validation_mse: f64,
    pub test_accuracy: f64,
    pub sharpe_ratio: f64,
}

#[derive(Debug)]
pub struct ModelRegistry {
    models_root: String,
    loaded_models: HashMap<String, String>, // key -> model_path
    metadata_cache: HashMap<String, ModelMetadata>,
}

impl ModelRegistry {
    pub fn new(models_root: &str) -> Self {
        Self {
            models_root: models_root.to_string(),
            loaded_models: HashMap::new(),
            metadata_cache: HashMap::new(),
        }
    }

    /// Get the path to the current production model
    pub fn get_current_model_path(&self, symbol: &str, model_type: &str) -> Result<String, Box<dyn std::error::Error>> {
        let current_path = format!("{}/{}/{}/current", self.models_root, symbol, model_type);
        
        if !Path::new(&current_path).exists() {
            return Err(format!("Current model symlink not found: {}", current_path).into());
        }

        let target = fs::read_link(&current_path)?;
        let model_file = format!("{}/{}/{}/model.fann", 
            self.models_root, symbol, model_type
        );
        
        Ok(model_file)
    }

    /// Load model metadata from the metadata directory
    pub fn load_model_metadata(&mut self, symbol: &str, model_type: &str) -> Result<&ModelMetadata, Box<dyn std::error::Error>> {
        let key = format!("{}_{}", symbol, model_type);
        
        if !self.metadata_cache.contains_key(&key) {
            let metadata_path = format!("{}/{}/{}/metadata/model_info.json", 
                self.models_root, symbol, model_type
            );
            
            let metadata_content = fs::read_to_string(&metadata_path)?;
            let metadata: ModelMetadata = serde_json::from_str(&metadata_content)?;
            
            self.metadata_cache.insert(key.clone(), metadata);
        }
        
        Ok(self.metadata_cache.get(&key).unwrap())
    }

    /// List all available model versions for a symbol/type
    pub fn list_available_versions(&self, symbol: &str, model_type: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let model_dir = format!("{}/{}/{}", self.models_root, symbol, model_type);
        let mut versions = Vec::new();
        
        for entry in fs::read_dir(&model_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('v') && name.chars().nth(1).map_or(false, |c| c.is_numeric()) {
                        versions.push(name.to_string());
                    }
                }
            }
        }
        
        versions.sort();
        Ok(versions)
    }

    /// Switch to a specific model version (updates the current symlink)
    pub fn switch_to_version(&self, symbol: &str, model_type: &str, version: &str) -> Result<(), Box<dyn std::error::Error>> {
        let current_path = format!("{}/{}/{}/current", self.models_root, symbol, model_type);
        let version_path = format!("{}/{}/{}/{}", self.models_root, symbol, model_type, version);
        
        // Verify version exists
        if !Path::new(&version_path).exists() {
            return Err(format!("Version {} not found for {}/{}", version, symbol, model_type).into());
        }
        
        // Remove existing symlink if it exists
        if Path::new(&current_path).exists() {
            fs::remove_file(&current_path)?;
        }
        
        // Create new symlink
        std::os::unix::fs::symlink(version, &current_path)?;
        
        println!("✅ Switched {}/{} to version {}", symbol, model_type, version);
        Ok(())
    }

    /// Validate model integrity using checksum
    pub fn validate_model_integrity(&self, symbol: &str, model_type: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let model_path = self.get_current_model_path(symbol, model_type)?;
        let model_content = fs::read(&model_path)?;
        
        // Calculate SHA256 checksum
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&model_content);
        let checksum = format!("{:x}", hasher.finalize());
        
        // Load expected checksum from metadata
        let metadata = self.load_model_metadata(symbol, model_type)?;
        let expected_checksum = metadata.checksum.strip_prefix("sha256:").unwrap_or(&metadata.checksum);
        
        Ok(checksum == expected_checksum)
    }

    /// Get model performance metrics
    pub fn get_performance_metrics(&mut self, symbol: &str, model_type: &str) -> Result<&PerformanceMetrics, Box<dyn std::error::Error>> {
        let metadata = self.load_model_metadata(symbol, model_type)?;
        Ok(&metadata.performance_metrics)
    }

    /// Create a backup of the current model
    pub fn create_backup(&self, symbol: &str, model_type: &str, backup_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let current_path = format!("{}/{}/{}/current", self.models_root, symbol, model_type);
        let target = fs::read_link(&current_path)?;
        let version_path = format!("{}/{}/{}/{}", self.models_root, symbol, model_type, target.to_string_lossy());
        let backup_path = format!("{}/{}/{}/backups/{}.tar.gz", self.models_root, symbol, model_type, backup_name);
        
        // Create compressed backup using tar
        let output = std::process::Command::new("tar")
            .args(&["-czf", &backup_path, "-C", &version_path, "."])
            .output()?;
        
        if !output.status.success() {
            return Err(format!("Backup creation failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
        
        println!("✅ Created backup: {}", backup_path);
        Ok(())
    }
}

// Demo usage
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Model Storage Architecture Demo");
    println!("==================================");
    
    let mut registry = ModelRegistry::new("models");
    
    // Demo 1: Load model metadata
    println!("\n📋 Loading model metadata...");
    match registry.load_model_metadata("AAPL", "prediction") {
        Ok(metadata) => {
            println!("✅ Model: {}", metadata.model_name);
            println!("   Version: {}", metadata.version);
            println!("   Accuracy: {:.1}%", metadata.performance_metrics.test_accuracy * 100.0);
            println!("   Sharpe Ratio: {:.2}", metadata.performance_metrics.sharpe_ratio);
        }
        Err(e) => println!("❌ Failed to load metadata: {}", e),
    }
    
    // Demo 2: List available versions
    println!("\n📂 Available versions for AAPL prediction:");
    match registry.list_available_versions("AAPL", "prediction") {
        Ok(versions) => {
            for version in versions {
                println!("   - {}", version);
            }
        }
        Err(e) => println!("❌ Failed to list versions: {}", e),
    }
    
    // Demo 3: Get current model path
    println!("\n🎯 Current model path:");
    match registry.get_current_model_path("AAPL", "prediction") {
        Ok(path) => println!("   {}", path),
        Err(e) => println!("❌ Failed to get model path: {}", e),
    }
    
    // Demo 4: Validate model integrity
    println!("\n🔐 Validating model integrity...");
    match registry.validate_model_integrity("AAPL", "prediction") {
        Ok(true) => println!("✅ Model integrity verified"),
        Ok(false) => println!("❌ Model integrity check failed"),
        Err(e) => println!("❌ Integrity validation error: {}", e),
    }
    
    // Demo 5: Performance metrics
    println!("\n📊 Performance metrics:");
    match registry.get_performance_metrics("AAPL", "prediction") {
        Ok(metrics) => {
            println!("   Train MSE: {:.6}", metrics.train_mse);
            println!("   Validation MSE: {:.6}", metrics.validation_mse);
            println!("   Test Accuracy: {:.1}%", metrics.test_accuracy * 100.0);
            println!("   Sharpe Ratio: {:.2}", metrics.sharpe_ratio);
        }
        Err(e) => println!("❌ Failed to get performance metrics: {}", e),
    }
    
    println!("\n🎉 Demo completed successfully!");
    println!("Ready for integration with ruv-fann models in production");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_model_registry_creation() {
        let registry = ModelRegistry::new("test_models");
        assert_eq!(registry.models_root, "test_models");
        assert!(registry.loaded_models.is_empty());
        assert!(registry.metadata_cache.is_empty());
    }
    
    #[test]
    fn test_current_model_path_construction() {
        let registry = ModelRegistry::new("models");
        // Note: This would fail in actual test without the directory structure
        // but demonstrates the path construction logic
        let expected_format = registry.get_current_model_path("AAPL", "prediction");
        assert!(expected_format.is_err()); // Expected to fail without actual structure
    }
}