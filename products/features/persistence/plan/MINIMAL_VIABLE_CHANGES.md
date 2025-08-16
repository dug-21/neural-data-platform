# Minimal Viable Changes for Neural-Trader Persistence

## Scope: ONLY Neural-Trader Application

**NO CHANGES TO:**
- ❌ TimescaleDB schema (already has aggregates)
- ❌ Data-ingestion service
- ❌ Redis configuration
- ❌ Network configuration
- ❌ Other services

## Three Core Changes Only

### 1. Fix Docker Volume Mounts (1 hour)

**File**: `docker/production/docker-compose.prod.yml`

**Current (BROKEN)**:
```yaml
neural-trader:
  volumes:
    - neural_trader_data:/var/lib/neural-trader  # WRONG PATH
```

**Fixed**:
```yaml
neural-trader:
  volumes:
    - neural_trader_models:/opt/neural-trader/models
    - neural_trader_checkpoints:/opt/neural-trader/checkpoints
    - neural_trader_logs:/opt/neural-trader/logs
```

### 2. Implement Checkpoint Functions (2 hours)

**File**: `src/neural/vendor_predictor.rs`

**Current (lines 1022-1030)**:
```rust
pub async fn save_checkpoint(&self, _model_name: &str) -> Result<()> {
    debug!("Save checkpoint requested - not yet implemented");
    Ok(())
}

pub async fn load_checkpoint(&self, _model_name: &str) -> Result<()> {
    debug!("Load checkpoint requested - not yet implemented");
    Ok(())
}
```

**Implementation**:
```rust
pub async fn save_checkpoint(&self, model_name: &str) -> Result<()> {
    let path = Path::new("/opt/neural-trader/models").join(format!("{}.bin", model_name));
    
    // Serialize models from DashMap
    let models_data = bincode::serialize(&self.models)?;
    
    // Write to disk
    tokio::fs::write(path, models_data).await?;
    info!("Saved checkpoint for {}", model_name);
    Ok(())
}

pub async fn load_checkpoint(&self, model_name: &str) -> Result<()> {
    let path = Path::new("/opt/neural-trader/models").join(format!("{}.bin", model_name));
    
    if path.exists() {
        let data = tokio::fs::read(path).await?;
        let models = bincode::deserialize(&data)?;
        self.models = models;
        info!("Loaded checkpoint for {}", model_name);
    }
    Ok(())
}
```

### 3. Query Historical Data on Startup (1 hour)

**File**: `src/main.rs`

**Add to startup sequence**:
```rust
// After TimescaleDB connection established
async fn load_historical_on_startup(
    storage: &TimescaleDBStorage,
    event_bus: &EventBus
) -> Result<()> {
    // Query EXISTING aggregates (no schema changes!)
    let query = r#"
        SELECT bucket, symbol, open, high, low, close, volume
        FROM market_data_1h  -- This table already exists!
        WHERE bucket > NOW() - INTERVAL '4 hours'
        ORDER BY bucket DESC
    "#;
    
    let historical = storage.query(query).await?;
    event_bus.populate_from_historical(historical).await?;
    
    info!("Loaded {} historical data points", historical.len());
    Ok(())
}
```

## Implementation Timeline

**Day 1 (4 hours total)**:
- Hour 1: Fix Docker volume configuration
- Hour 2-3: Implement save/load checkpoint
- Hour 4: Add historical data query

**Day 2 (2 hours)**:
- Test persistence across restarts
- Verify no impact on other services

**Day 3 (2 hours)**:
- Deploy to staging
- Production deployment

## Testing Checklist

- [ ] Models persist after `docker restart neural_trader_app`
- [ ] Historical data loads in < 30 seconds on startup
- [ ] Data-ingestion service unchanged and working
- [ ] TimescaleDB queries use existing aggregates
- [ ] No new database tables or views created

## What We're NOT Doing

- ❌ Complex versioning systems
- ❌ Compression algorithms
- ❌ Metadata databases
- ❌ Repository patterns
- ❌ Enterprise architecture
- ❌ 6-week implementation plans
- ❌ Database schema modifications
- ❌ Data-ingestion changes

## Success Criteria

1. **Models persist**: Survive container restart
2. **Fast startup**: < 30 seconds to trading capability
3. **No breaking changes**: Other services unaffected

## Risk Assessment

- **Low Risk**: Simple file I/O operations
- **No Database Risk**: Using existing schema only
- **No Service Risk**: Data-ingestion untouched

## Total Effort: 8 hours over 3 days

This is a tactical fix, not an enterprise transformation.