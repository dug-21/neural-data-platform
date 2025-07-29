# Model Storage System for ruv-fann Neural Networks

This module provides a comprehensive versioned storage system for ruv-fann `Network<f32>` models with Docker-compatible persistence, automatic versioning, rollback capabilities, and checkpointing.

## Features

- **Versioned Storage**: Automatic semantic versioning (major.minor.patch) for model versions
- **Atomic Saves**: Models are saved atomically to prevent corruption during save operations
- **Rollback Support**: Roll back to previous model versions when performance degrades
- **Checkpointing**: Save training checkpoints with metrics during long training sessions
- **Metadata Tracking**: Store comprehensive metadata including performance metrics, training parameters, and data information
- **Integrity Verification**: SHA256 checksums ensure model file integrity
- **Docker Compatible**: Works in containerized environments with persistent volumes
- **Version Limits**: Automatic cleanup of old versions to manage disk space

## Quick Start

### 1. Initialize Storage

```rust
use neural_trader::adapters::model_storage::{ModelStorage, ModelStorageConfig};
use std::path::PathBuf;

let config = ModelStorageConfig {
    base_path: PathBuf::from("models"),
    max_versions_per_model: 10,
    enable_compression: true,
    enable_encryption: false,
    checkpoint_frequency: 100,
};

let storage = ModelStorage::new(config).await?;
```

### 2. Save a Model

```rust
use ruv_fann::Network;
use neural_trader::adapters::model_storage::{ModelMetadata, VersionIncrement};

// Create your network
let network = Network::new(&[3, 5, 1]);

// Create metadata
let metadata = ModelMetadata {
    model_type: "price_predictor".to_string(),
    version: SemanticVersion::new(1, 0, 0),
    timestamp: Utc::now(),
    accuracy: 0.87,
    loss: 0.13,
    // ... other fields
};

// Save the model
let version = storage.save_model(
    &network, 
    "price_predictor", 
    metadata, 
    VersionIncrement::Patch
).await?;

println!("Saved model version {} at {:?}", version.version, version.path);
```

### 3. Load a Model

```rust
// Load latest version
let (network, metadata) = storage.load_model("price_predictor", None).await?;

// Load specific version
let (network, metadata) = storage.load_model(
    "price_predictor", 
    Some(SemanticVersion::new(1, 0, 5))
).await?;
```

### 4. Rollback to Previous Version

```rust
// Rollback 2 versions
let (network, metadata) = storage.rollback("price_predictor", 2).await?;
println!("Rolled back to version {}", metadata.version);
```

### 5. Save Training Checkpoints

```rust
use neural_trader::adapters::model_storage::CheckpointMetrics;

let checkpoint_metrics = CheckpointMetrics {
    epoch: 100,
    training_loss: 0.05,
    validation_loss: 0.06,
    learning_rate: 0.001,
    timestamp: Utc::now(),
};

storage.save_checkpoint(&network, "price_predictor", 100, checkpoint_metrics).await?;
```

### 6. Load Checkpoints

```rust
let (network, metrics) = storage.load_checkpoint("price_predictor", 100).await?;
println!("Loaded checkpoint from epoch {} with loss {}", metrics.epoch, metrics.training_loss);
```

## Configuration Options

### ModelStorageConfig

```rust
pub struct ModelStorageConfig {
    /// Base directory for model storage
    pub base_path: PathBuf,
    
    /// Maximum number of versions to keep per model type
    pub max_versions_per_model: usize,
    
    /// Enable compression for model files (not yet implemented)
    pub enable_compression: bool,
    
    /// Enable encryption for model files (not yet implemented) 
    pub enable_encryption: bool,
    
    /// Save checkpoint every N epochs during training
    pub checkpoint_frequency: usize,
}
```

### Version Increment Strategies

```rust
pub enum VersionIncrement {
    Patch,  // Bug fixes, minor improvements (1.0.0 -> 1.0.1)
    Minor,  // New features, backward compatible (1.0.0 -> 1.1.0)  
    Major,  // Breaking changes (1.0.0 -> 2.0.0)
    Auto,   // Automatically determine based on metrics
}
```

## Directory Structure

The storage system creates a hierarchical directory structure:

```
models/
├── price_predictor/
│   ├── 1.0.0/
│   │   ├── model.ruv          # Serialized network
│   │   └── metadata.json      # Model metadata
│   ├── 1.0.1/
│   │   ├── model.ruv
│   │   └── metadata.json
│   └── checkpoints/
│       ├── checkpoint_epoch_100.ruv
│       ├── checkpoint_epoch_100.json
│       ├── checkpoint_epoch_200.ruv
│       └── checkpoint_epoch_200.json
└── trend_predictor/
    └── 1.0.0/
        ├── model.ruv
        └── metadata.json
```

## Metadata Schema

Each model version includes comprehensive metadata:

```rust
pub struct ModelMetadata {
    pub model_type: String,
    pub version: SemanticVersion,
    pub timestamp: DateTime<Utc>,
    pub accuracy: f64,
    pub loss: f64,
    pub training_params: TrainingParams,
    pub performance_metrics: PerformanceMetrics,
    pub checksum: String,
    pub training_duration_secs: u64,
    pub data_info: DataInfo,
}
```

## Docker Integration

For Docker deployments, mount a persistent volume to the models directory:

### docker-compose.yml
```yaml
version: '3.8'
services:
  neural-trader:
    build: .
    volumes:
      - model_storage:/app/models
    environment:
      - MODEL_STORAGE_PATH=/app/models
      - MODEL_MAX_VERSIONS=10

volumes:
  model_storage:
```

### Dockerfile
```dockerfile
FROM rust:1.70

WORKDIR /app
COPY . .

# Create models directory
RUN mkdir -p /app/models

# Set environment variables
ENV MODEL_STORAGE_PATH=/app/models
ENV MODEL_MAX_VERSIONS=10

RUN cargo build --release

CMD ["./target/release/neural-trader"]
```

## Best Practices

### 1. Version Management
- Use semantic versioning consistently
- Set appropriate `max_versions_per_model` to balance disk usage and rollback capability
- Consider the model improvement threshold for version increments

### 2. Checkpointing
- Save checkpoints at regular intervals during long training runs
- Include meaningful metrics in checkpoint metadata
- Clean up old checkpoints periodically

### 3. Error Handling
- Always handle potential I/O errors when loading/saving models
- Verify checksums when loading critical models
- Implement fallback mechanisms for model loading failures

### 4. Performance
- Use appropriate batch sizes based on available memory
- Consider enabling compression for large models (when implemented)
- Monitor disk usage and implement cleanup strategies

## Examples

See the following files for complete examples:
- `examples/model_storage_usage.rs` - Basic usage demonstration
- `tests/model_storage_integration_test.rs` - Comprehensive integration tests

## API Reference

### ModelStorage

#### Methods

- `new(config: ModelStorageConfig) -> Result<Self>` - Create new storage instance
- `save_model(&self, network: &Network<f32>, model_type: &str, metadata: ModelMetadata, increment_type: VersionIncrement) -> Result<ModelVersion>` - Save a model with versioning
- `load_model(&self, model_type: &str, version: Option<SemanticVersion>) -> Result<(Network<f32>, ModelMetadata)>` - Load a model version
- `rollback(&self, model_type: &str, versions_back: usize) -> Result<(Network<f32>, ModelMetadata)>` - Rollback to previous version
- `save_checkpoint(&self, network: &Network<f32>, model_type: &str, epoch: usize, metrics: CheckpointMetrics) -> Result<()>` - Save training checkpoint
- `load_checkpoint(&self, model_type: &str, epoch: usize) -> Result<(Network<f32>, CheckpointMetrics)>` - Load training checkpoint
- `list_versions(&self, model_type: &str) -> Vec<(SemanticVersion, DateTime<Utc>)>` - List all versions for a model type
- `get_storage_metrics(&self) -> StorageMetrics` - Get storage usage statistics

## Troubleshooting

### Common Issues

1. **Permission Errors**: Ensure the process has read/write access to the storage directory
2. **Disk Space**: Monitor disk usage and adjust `max_versions_per_model` accordingly
3. **Checksum Mismatches**: May indicate file corruption; try loading from backup/checkpoint
4. **Version Not Found**: Check version numbers and ensure the model was saved correctly

### Logging

The system uses `tracing` for logging. Enable debug logging to see detailed operations:

```rust
tracing_subscriber::init();
```

## Future Enhancements

- [ ] Compression support for model files
- [ ] Encryption support for sensitive models  
- [ ] Remote storage backends (S3, GCS, etc.)
- [ ] Model comparison and diff tools
- [ ] Automatic performance-based version management
- [ ] Integration with MLOps platforms