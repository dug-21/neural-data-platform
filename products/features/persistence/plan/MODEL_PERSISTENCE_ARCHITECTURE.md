# MINIMAL Model Persistence - Neural-Trader Only

## Executive Summary
This document defines the MINIMAL VIABLE model persistence implementation for neural-trader, focusing only on essential checkpoint save/load functionality.

## Current State Analysis

### Problems
- Models exist only in memory (DashMap storage) 
- No checkpoint/save functionality implemented
- Complete loss of trained models on restart
- 2-3 hour recovery time for trading capability

### Impact
- Production deployment impossible without 24/7 uptime
- No disaster recovery capability
- Unable to perform maintenance without data loss

## MINIMAL Solution Approach

### 1. Simple Checkpoint Storage

#### Basic Binary Format
```rust
// Simple bincode serialization - NO VERSIONING
#[derive(Serialize, Deserialize)]
struct ModelCheckpoint {
    timestamp: DateTime<Utc>,
    model_data: HashMap<String, Vec<f32>>,  // Raw model weights
    metadata: BasicMetadata,
}
```

#### Simple Storage Structure
```
/opt/neural-trader/models/
├── checkpoint.bin       # Single checkpoint file
├── checkpoint.backup    # Previous checkpoint backup
└── metadata.json        # Simple JSON metadata (NO DATABASE)
```

### 2. Simple Checkpoint Triggers

#### Basic Trigger Strategy
```rust
// MINIMAL triggers only
enum CheckpointTrigger {
    Periodic(Duration),      // Every 4 hours
    PreShutdown,            // Graceful shutdown hook
}
```

#### Simple Save Points
- **Regular Intervals**: Every 4 hours during market hours
- **Before Maintenance**: Triggered by shutdown signal
- **NO COMPLEX LOGIC**: No performance-based triggers

### 3. NO COMPRESSION (MINIMAL APPROACH)

#### Simple Binary Storage
```rust
// NO COMPRESSION - Keep it simple
async fn save_checkpoint(data: &ModelCheckpoint) -> Result<()> {
    let serialized = bincode::serialize(data)?;
    tokio::fs::write("/opt/neural-trader/models/checkpoint.bin", serialized).await?;
    Ok(())
}
```

### 5. Recovery Architecture

#### Startup Sequence
```rust
async fn initialize_models() -> Result<()> {
    // 1. Check for active models
    if let Ok(models) = load_active_models().await {
        info!("Loaded {} active models", models.len());
        return Ok(());
    }
    
    // 2. Fall back to latest checkpoint
    if let Ok(checkpoint) = load_latest_checkpoint().await {
        restore_from_checkpoint(checkpoint).await?;
        return Ok(());
    }
    
    // 3. Initialize fresh models
    warn!("No persisted models found, initializing fresh");
    initialize_default_models().await
}
```

#### Rollback Capability
```rust
async fn rollback_model(model_id: &str, version: SemanticVersion) -> Result<()> {
    // Load specific version
    let archived = load_archived_model(model_id, version).await?;
    
    // Validate before replacement
    validate_model_integrity(&archived)?;
    
    // Atomic swap
    replace_active_model(model_id, archived).await?;
    
    // Log rollback event
    audit_log_rollback(model_id, version).await?;
    
    Ok(())
}
```

### 6. Implementation Classes

#### Core Components
```rust
// Main persistence manager
pub struct ModelPersistenceManager {
    storage_path: PathBuf,
    compression: CompressionLevel,
    version_keeper: VersionKeeper,
    scheduler: CheckpointScheduler,
}

// Checkpoint scheduler
pub struct CheckpointScheduler {
    periodic_interval: Duration,
    next_checkpoint: DateTime<Utc>,
    triggers: Vec<CheckpointTrigger>,
}

// Version management
pub struct VersionKeeper {
    max_versions: usize,
    archive_policy: ArchivePolicy,
    metadata_db: SqliteConnection,
}
```

### 7. Performance Considerations

#### Async I/O
- All save/load operations use tokio::fs for async I/O
- Parallel model loading on startup
- Background checkpointing without blocking predictions

#### Memory Management
```rust
// Stream large models to disk
async fn save_large_model(model: &[u8]) -> Result<()> {
    let mut file = tokio::fs::File::create(path).await?;
    let mut chunks = model.chunks(CHUNK_SIZE);
    
    while let Some(chunk) = chunks.next() {
        file.write_all(chunk).await?;
        tokio::task::yield_now().await; // Prevent blocking
    }
    
    file.sync_all().await?;
    Ok(())
}
```

### 8. Monitoring & Observability

#### Metrics
```rust
// Prometheus metrics
static CHECKPOINT_DURATION: Histogram = register_histogram!(
    "model_checkpoint_duration_seconds",
    "Time taken to save checkpoint"
);

static MODEL_SIZE_BYTES: Gauge = register_gauge!(
    "model_size_bytes",
    "Size of model in bytes"
);

static CHECKPOINT_FAILURES: Counter = register_counter!(
    "model_checkpoint_failures_total",
    "Number of failed checkpoint attempts"
);
```

### 9. Security Considerations

#### Integrity Verification
- SHA-256 checksums for all saved models
- Signature verification for critical models
- Encrypted storage for sensitive architectures

#### Access Control
```rust
// File permissions
fn set_model_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600); // Read/write for owner only
    fs::set_permissions(path, perms)?;
    Ok(())
}
```

## Migration Path

### Phase 1: Infrastructure (Week 1)
1. Create directory structure
2. Implement basic save/load functions
3. Add compression support

### Phase 2: Integration (Week 2)
1. Integrate with VendorPredictor
2. Add checkpoint scheduler
3. Implement version management

### Phase 3: Recovery (Week 3)
1. Startup model loading
2. Rollback functionality
3. Integrity verification

### Phase 4: Production (Week 4)
1. Monitoring integration
2. Performance optimization
3. Security hardening

## Success Metrics

- **Save Performance**: < 1 second for 100MB model
- **Load Performance**: < 5 seconds for all models
- **Compression Ratio**: > 50% size reduction
- **Recovery Time**: < 30 seconds from cold start
- **Storage Growth**: < 1GB per week with rotation

## Risk Mitigation

1. **Corruption Risk**: Multiple backup locations, checksums
2. **Performance Impact**: Async I/O, background operations
3. **Storage Overflow**: Automatic rotation, compression
4. **Version Conflicts**: Semantic versioning, metadata tracking

## Conclusion

This architecture provides enterprise-grade model persistence with:
- Zero data loss guarantee
- Rapid recovery capability
- Version control and rollback
- Minimal performance impact
- Production-ready monitoring

Implementation will transform neural-trader from a prototype to a production-ready trading platform.