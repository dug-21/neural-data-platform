# Minimal Implementation Specification - Neural-Trader Persistence

## Scope
MINIMAL VIABLE implementation for neural-trader model persistence ONLY.
- NO database schema changes
- NO data-ingestion modifications  
- NO enterprise features

## Total Implementation: 8 Hours Over 3 Days

### Day 1: Core Implementation (4 hours)

#### Hour 1: Fix Docker Volumes
**File**: `docker/production/docker-compose.prod.yml`
```yaml
# Change from:
- neural_trader_data:/var/lib/neural-trader  # WRONG

# To:
- neural_trader_models:/opt/neural-trader/models
- neural_trader_checkpoints:/opt/neural-trader/checkpoints
```

**File**: `docker/production/Dockerfile`
```dockerfile
# Ensure directories exist with correct permissions
RUN mkdir -p /opt/neural-trader/{models,checkpoints} \
    && chown -R neural:neural /opt/neural-trader
```

#### Hours 2-3: Implement Checkpoint Functions
**File**: `src/neural/vendor_predictor.rs`
```rust
// Replace stub implementations (lines 1022-1030)
pub async fn save_checkpoint(&self, model_name: &str) -> Result<()> {
    let path = PathBuf::from("/opt/neural-trader/models")
        .join(format!("{}.bin", model_name));
    
    // Simple serialization using bincode
    let model_data = bincode::serialize(&self.models)?;
    tokio::fs::write(&path, model_data).await?;
    
    info!("Saved checkpoint: {}", path.display());
    Ok(())
}

pub async fn load_checkpoint(&self, model_name: &str) -> Result<()> {
    let path = PathBuf::from("/opt/neural-trader/models")
        .join(format!("{}.bin", model_name));
    
    if path.exists() {
        let data = tokio::fs::read(&path).await?;
        let models = bincode::deserialize(&data)?;
        self.models = models;
        info!("Loaded checkpoint: {}", path.display());
    }
    Ok(())
}
```

#### Hour 4: Add Historical Data Loading
**File**: `src/main.rs`
```rust
// Add after TimescaleDB initialization (around line 260)
async fn load_historical_data(
    storage: &TimescaleDBStorage,
    event_bus: &EventBus
) -> Result<()> {
    // Query EXISTING continuous aggregate
    let query = r#"
        SELECT bucket, symbol, open, high, low, close, volume
        FROM market_data_1h  -- Already exists!
        WHERE bucket > NOW() - INTERVAL '4 hours'
        ORDER BY bucket DESC
        LIMIT 1000
    "#;
    
    let rows = storage.query_raw(query).await?;
    
    // Convert to events and populate bus
    for row in rows {
        let event = MarketEvent::from_row(row)?;
        event_bus.publish_market_event(event).await?;
    }
    
    info!("Loaded {} historical data points", rows.len());
    Ok(())
}

// Call during startup
load_historical_data(&storage, &event_bus).await?;
```

### Day 2: Testing & Validation (2 hours)

#### Hour 1: Basic Functionality Tests
```bash
# Test checkpoint save/load
docker exec neural_trader_app ls -la /opt/neural-trader/models/

# Test persistence across restart
docker restart neural_trader_app
docker logs neural_trader_app | grep "Loaded checkpoint"

# Verify historical data loading
docker logs neural_trader_app | grep "Loaded .* historical data points"
```

#### Hour 2: Integration Verification
- Verify data-ingestion is unchanged
- Confirm TimescaleDB queries work
- Check startup time < 30 seconds

### Day 3: Deployment (2 hours)

#### Hour 1: Staging Deployment
```bash
# Update docker-compose.prod.yml
cd docker/production
./build.sh

# Deploy to staging
docker-compose -f docker-compose.prod.yml up -d neural-trader

# Monitor logs
docker logs -f neural_trader_app
```

#### Hour 2: Production Deployment
- Deploy during maintenance window
- Verify checkpoints are saving
- Confirm fast startup with historical data

## Files Modified

1. `docker/production/docker-compose.prod.yml` - Fix volume mounts
2. `docker/production/Dockerfile` - Ensure directories exist
3. `src/neural/vendor_predictor.rs` - Implement save/load checkpoint
4. `src/main.rs` - Add historical data loading on startup

## Dependencies

Add to `Cargo.toml` if not present:
```toml
bincode = "1.3"  # For model serialization
```

## Success Criteria

- [ ] Models persist after restart
- [ ] Startup time < 30 seconds
- [ ] No changes to data-ingestion
- [ ] No database schema modifications
- [ ] No breaking changes to other services

## What This Does NOT Include

- ❌ Complex versioning systems
- ❌ Compression algorithms  
- ❌ Repository patterns
- ❌ Distributed transactions
- ❌ Saga patterns
- ❌ Data archival
- ❌ Enterprise monitoring
- ❌ 6-week implementation

## Risk Mitigation

- **Rollback Plan**: Revert docker-compose.yml and redeploy
- **Testing**: Full testing in dev before staging
- **Monitoring**: Watch logs during first 24 hours

## Total Effort: 8 Hours

This is a tactical fix to enable basic persistence, not an enterprise transformation.