//! Example: ruv-fann Model Serialization
//! 
//! This example demonstrates how to save and load ruv-fann neural network models
//! using different serialization formats, including compression for production deployment.

use anyhow::{Context, Result};
use ruv_fann::{Network, NetworkBuilder, ActivationFunction, TrainingData};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// Model metadata for tracking and versioning
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ModelMetadata {
    /// Model version
    version: String,
    /// Model type (e.g., "LSTM", "GRU", "MLP")
    model_type: String,
    /// Training timestamp
    trained_at: chrono::DateTime<chrono::Utc>,
    /// Training accuracy
    accuracy: f64,
    /// Training parameters
    training_params: TrainingParams,
    /// Additional notes
    notes: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TrainingParams {
    epochs: usize,
    learning_rate: f32,
    batch_size: usize,
    optimizer: String,
}

/// Complete model package with network and metadata
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ModelPackage {
    /// The neural network
    network: Network<f32>,
    /// Model metadata
    metadata: ModelMetadata,
}

/// Save a ruv-fann model using different formats
fn save_model_examples() -> Result<()> {
    // Create a sample network
    let network = NetworkBuilder::<f32>::new()
        .input_layer(10)
        .hidden_layer(20)
        .hidden_layer(10)
        .output_layer(5)
        .activation_func(ActivationFunction::SigmoidSymmetric)
        .output_activation_func(ActivationFunction::Linear)
        .build();

    // Create metadata
    let metadata = ModelMetadata {
        version: "1.0.0".to_string(),
        model_type: "MLP".to_string(),
        trained_at: chrono::Utc::now(),
        accuracy: 0.95,
        training_params: TrainingParams {
            epochs: 1000,
            learning_rate: 0.001,
            batch_size: 32,
            optimizer: "Adam".to_string(),
        },
        notes: Some("Production model for BTCUSD prediction".to_string()),
    };

    // Create model package
    let package = ModelPackage {
        network: network.clone(),
        metadata,
    };

    // Example 1: Save as JSON (human-readable, good for debugging)
    save_as_json(&package, "models/model_v1.json")?;

    // Example 2: Save as binary (smaller size, faster loading)
    save_as_binary(&package, "models/model_v1.bin")?;

    // Example 3: Save as compressed binary (smallest size, production)
    save_as_compressed_binary(&package, "models/model_v1.bin.gz")?;

    // Example 4: Save in native FANN format (compatibility)
    save_as_fann_format(&network, "models/model_v1.fann")?;

    // Example 5: Save with custom compression level
    save_with_custom_compression(&package, "models/model_v1_fast.bin.gz", 1)?; // Fast compression
    save_with_custom_compression(&package, "models/model_v1_best.bin.gz", 9)?; // Best compression

    println!("✅ Models saved successfully!");
    Ok(())
}

/// Save model as JSON
fn save_as_json(package: &ModelPackage, path: &str) -> Result<()> {
    use ruv_fann::io::json::JsonWriter;
    
    std::fs::create_dir_all(Path::new(path).parent().unwrap())?;
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    
    let json_writer = JsonWriter::new();
    json_writer.write(package, &mut writer)
        .context("Failed to write JSON")?;
    
    println!("📝 Saved as JSON: {}", path);
    Ok(())
}

/// Save model as binary
fn save_as_binary(package: &ModelPackage, path: &str) -> Result<()> {
    use ruv_fann::io::binary::BinaryWriter;
    
    std::fs::create_dir_all(Path::new(path).parent().unwrap())?;
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    
    let binary_writer = BinaryWriter::new();
    binary_writer.write(package, &mut writer)
        .context("Failed to write binary")?;
    
    println!("💾 Saved as binary: {}", path);
    Ok(())
}

/// Save model as compressed binary
fn save_as_compressed_binary(package: &ModelPackage, path: &str) -> Result<()> {
    use ruv_fann::io::{binary::write_binary, compression::CompressedWriter};
    
    std::fs::create_dir_all(Path::new(path).parent().unwrap())?;
    let file = File::create(path)?;
    let compressed_writer = CompressedWriter::new(file);
    
    write_binary(package, &mut BufWriter::new(compressed_writer))
        .context("Failed to write compressed binary")?;
    
    println!("🗜️ Saved as compressed binary: {}", path);
    Ok(())
}

/// Save with custom compression level
fn save_with_custom_compression(package: &ModelPackage, path: &str, level: u32) -> Result<()> {
    use ruv_fann::io::{binary::write_binary, compression::{CompressedWriter, CompressionConfig}};
    
    std::fs::create_dir_all(Path::new(path).parent().unwrap())?;
    let file = File::create(path)?;
    
    let config = CompressionConfig::with_level(level);
    let compressed_writer = CompressedWriter::with_config(file, config);
    
    write_binary(package, &mut BufWriter::new(compressed_writer))
        .context("Failed to write compressed binary")?;
    
    println!("🗜️ Saved with compression level {}: {}", level, path);
    Ok(())
}

/// Save in native FANN format
fn save_as_fann_format(network: &Network<f32>, path: &str) -> Result<()> {
    use ruv_fann::io::fann_format::FannWriter;
    
    std::fs::create_dir_all(Path::new(path).parent().unwrap())?;
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    
    let fann_writer = FannWriter::new();
    fann_writer.write_network(network, &mut writer)
        .context("Failed to write FANN format")?;
    
    println!("🧠 Saved in FANN format: {}", path);
    Ok(())
}

/// Load a ruv-fann model from different formats
fn load_model_examples() -> Result<()> {
    // Example 1: Load from JSON
    let package_json = load_from_json("models/model_v1.json")?;
    println!("📖 Loaded from JSON: {:?}", package_json.metadata.model_type);

    // Example 2: Load from binary
    let package_bin = load_from_binary("models/model_v1.bin")?;
    println!("💿 Loaded from binary: {:?}", package_bin.metadata.model_type);

    // Example 3: Load from compressed binary
    let package_compressed = load_from_compressed_binary("models/model_v1.bin.gz")?;
    println!("📦 Loaded from compressed: {:?}", package_compressed.metadata.model_type);

    // Example 4: Load from FANN format
    let network_fann = load_from_fann_format("models/model_v1.fann")?;
    println!("🧠 Loaded from FANN format: {} layers", network_fann.num_layers());

    Ok(())
}

/// Load model from JSON
fn load_from_json(path: &str) -> Result<ModelPackage> {
    use ruv_fann::io::json::JsonReader;
    
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    let json_reader = JsonReader::new();
    let package = json_reader.read(&mut reader)
        .context("Failed to read JSON")?;
    
    Ok(package)
}

/// Load model from binary
fn load_from_binary(path: &str) -> Result<ModelPackage> {
    use ruv_fann::io::binary::BinaryReader;
    
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    let binary_reader = BinaryReader::new();
    let package = binary_reader.read(&mut reader)
        .context("Failed to read binary")?;
    
    Ok(package)
}

/// Load model from compressed binary
fn load_from_compressed_binary(path: &str) -> Result<ModelPackage> {
    use ruv_fann::io::{binary::read_binary, compression::CompressedReader};
    
    let file = File::open(path)?;
    let compressed_reader = CompressedReader::new(file);
    let mut reader = BufReader::new(compressed_reader);
    
    let package = read_binary(&mut reader)
        .context("Failed to read compressed binary")?;
    
    Ok(package)
}

/// Load from FANN format
fn load_from_fann_format(path: &str) -> Result<Network<f32>> {
    use ruv_fann::io::fann_format::FannReader;
    
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    let fann_reader = FannReader::new();
    let network = fann_reader.read_network(&mut reader)
        .context("Failed to read FANN format")?;
    
    Ok(network)
}

/// Production deployment example with optimized loading
fn production_deployment_example() -> Result<()> {
    println!("\n🚀 Production Deployment Example");
    
    // In production, use compressed binary for optimal size and loading speed
    let model_path = "models/production/model_latest.bin.gz";
    
    // Load model with error handling and validation
    let package = load_production_model(model_path)?;
    
    // Validate model version
    if package.metadata.version != "1.0.0" {
        anyhow::bail!("Unexpected model version: {}", package.metadata.version);
    }
    
    // Check model accuracy threshold
    if package.metadata.accuracy < 0.90 {
        anyhow::bail!("Model accuracy too low: {}", package.metadata.accuracy);
    }
    
    println!("✅ Production model loaded successfully!");
    println!("   Version: {}", package.metadata.version);
    println!("   Type: {}", package.metadata.model_type);
    println!("   Accuracy: {:.2}%", package.metadata.accuracy * 100.0);
    
    Ok(())
}

/// Load production model with caching
fn load_production_model(path: &str) -> Result<ModelPackage> {
    use std::time::Instant;
    
    let start = Instant::now();
    
    // Check if model exists
    if !Path::new(path).exists() {
        anyhow::bail!("Model file not found: {}", path);
    }
    
    // Load compressed model
    let package = load_from_compressed_binary(path)?;
    
    let load_time = start.elapsed();
    println!("⏱️ Model loaded in {:.2}ms", load_time.as_millis());
    
    Ok(package)
}

/// Compare serialization formats
fn compare_formats() -> Result<()> {
    use ruv_fann::io::compression::analyze::test_compression;
    
    println!("\n📊 Format Comparison");
    
    // Create test model
    let network = NetworkBuilder::<f32>::new()
        .input_layer(100)
        .hidden_layer(200)
        .hidden_layer(100)
        .output_layer(50)
        .build();
    
    let package = ModelPackage {
        network,
        metadata: ModelMetadata {
            version: "1.0.0".to_string(),
            model_type: "Large MLP".to_string(),
            trained_at: chrono::Utc::now(),
            accuracy: 0.95,
            training_params: TrainingParams {
                epochs: 5000,
                learning_rate: 0.0001,
                batch_size: 64,
                optimizer: "AdamW".to_string(),
            },
            notes: None,
        },
    };
    
    // Save in different formats
    save_as_json(&package, "models/compare.json")?;
    save_as_binary(&package, "models/compare.bin")?;
    save_as_compressed_binary(&package, "models/compare.bin.gz")?;
    
    // Compare file sizes
    let json_size = std::fs::metadata("models/compare.json")?.len();
    let bin_size = std::fs::metadata("models/compare.bin")?.len();
    let compressed_size = std::fs::metadata("models/compare.bin.gz")?.len();
    
    println!("📄 JSON size: {} bytes", json_size);
    println!("💾 Binary size: {} bytes ({:.1}% of JSON)", bin_size, (bin_size as f64 / json_size as f64) * 100.0);
    println!("🗜️ Compressed size: {} bytes ({:.1}% of JSON)", compressed_size, (compressed_size as f64 / json_size as f64) * 100.0);
    
    // Test compression effectiveness
    let binary_data = std::fs::read("models/compare.bin")?;
    let stats = test_compression(&binary_data)?;
    println!("\n📈 Compression Analysis:");
    println!("   Compression ratio: {:.2}", stats.ratio);
    println!("   Space savings: {:.1}%", stats.savings_percent);
    
    Ok(())
}

fn main() -> Result<()> {
    println!("🧠 ruv-fann Model Serialization Examples\n");
    
    // Create models directory
    std::fs::create_dir_all("models/production")?;
    
    // Run examples
    save_model_examples()?;
    println!();
    load_model_examples()?;
    println!();
    production_deployment_example()?;
    compare_formats()?;
    
    Ok(())
}