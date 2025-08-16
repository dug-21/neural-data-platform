# Vendor Predictor Architecture Documentation

## 🏗️ Critical Architecture Overview

The VendorPredictor is the core of neural-trader's ML system, implementing a sophisticated two-layer sector-based architecture that we've been carefully maintaining and fixing.

### Two-Layer Model Hierarchy

```
Layer 1: Sector Models (ETF Representatives)
├── XLK_primary.fann (320-512MB) - Technology Sector
├── XLF_primary.fann (320-512MB) - Financial Services  
├── XLV_primary.fann (320-512MB) - Healthcare
└── ... (11 total sector models)

Layer 2: Symbol Specializations  
├── AAPL_specialization.fann (6-8MB) - Adapts XLK model
├── MSFT_specialization.fann (6-8MB) - Adapts XLK model
├── JPM_specialization.fann (6-8MB) - Adapts XLF model
└── ... (individual stock adaptations)
```

## 🎯 Core Functionality

### 1. Sector Model Training (Layer 1)
**ONLY ETFs train primary models**

```rust
// vendor_predictor.rs:2155
fn get_training_symbols_for_model(&self, symbol: &str) -> Result<Vec<String>> {
    if symbol_loader::is_sector_etf(symbol) {
        // ETF trains on its own data only
        Ok(vec![symbol.to_string()])
    } else {
        // Individual stocks DON'T train primary models
        Ok(vec![])  
    }
}
```

**Critical ETF Mappings:**
- XLK → Technology (AAPL, MSFT, GOOGL, NVDA)
- XLF → Financial Services (JPM, BAC, WFC, GS)
- XLV → Healthcare (JNJ, PFE, UNH, CVS)
- XLE → Energy (XOM, CVX, COP, SLB)
- XLI → Industrials (BA, CAT, UNP, HON)

### 2. Symbol Specialization (Layer 2)
Individual stocks use small adaptation layers on top of sector models:

```rust
// Specialization training uses differential learning
pub async fn train_symbol_specialization(
    &self,
    symbol: &str,
    sector_model: &BaseModel
) -> Result<SpecializationLayer>
```

### 3. Autonomous Training System

**Environment Variables (CRITICAL):**
```bash
ENABLE_AUTONOMOUS_TRAINING=true           # Master switch
AUTONOMOUS_TRAINING_INTERVAL_MINUTES=60   # How often to check
MIN_DATA_POINTS_FOR_TRAINING=100         # Minimum data requirement
TRAINING_HISTORY_DAYS=90                  # Data window (NOT 7!)
MAX_TRAINING_HISTORY_DAYS=180            # Upper limit
MIN_TRAINING_HISTORY_DAYS=7              # Lower limit (rarely used)
```

**Market Hours Priority:**
```rust
// Training deferred during market hours (9:30 AM - 4:00 PM EST)
if is_market_hours() {
    // TRADING PRIORITY: Only critical updates
    return Ok(TrainingDeferred);
} else {
    // TRAINING PRIORITY: Full retraining allowed
    self.retrain_all_models().await?;
}
```

### 4. Data Loading (Fixed Bug)

**CRITICAL BUG WE FIXED:**
```rust
// BAD (old code):
let days = std::cmp::min(min_training_history_days, 7); // Always 7!

// GOOD (fixed):
let days = training_history_days; // Use actual env var (90)
```

### 5. DST Timezone Handling (Fixed Bug)

**CRITICAL BUG WE FIXED:**
```rust
// BAD (old code):
let local_time = self.timezone_converter.convert_to_exchange_time(time, exchange);

// GOOD (fixed):
let local_time = self.timezone_converter.convert_with_dst(time, exchange);
```

## 📊 Cluster Model Pool

The ClusterModelPool manages all models with versioning and rotation:

```rust
pub struct ClusterModelPool {
    models: Arc<RwLock<HashMap<String, ClusterModel>>>,
    config: ClusterPoolConfig,
    storage_path: PathBuf,  // /opt/neural-trader/sector-models
    backup_path: PathBuf,   // /opt/neural-trader/sector-models/backups
}
```

**Key Features:**
- Hot-swapping models without downtime
- Automatic backup before retraining
- Version history maintenance
- Memory-mapped loading for performance

## 🔄 Training Workflow

### Phase 1: Data Validation
```rust
// Validation gates (must pass all)
MIN_R2_SCORE=0.6        // Model quality threshold
MAX_MSE=0.01           // Error threshold
MIN_DATA_QUALITY=0.95  // Data completeness
```

### Phase 2: Model Training
1. **Sector Models**: Train on ETF data (90 days)
2. **Specializations**: Adapt to individual stocks (differential)
3. **Validation**: Test on holdout data
4. **Deployment**: Atomic swap if better than current

### Phase 3: Persistence
```bash
# Docker volume configuration (FIXED)
/opt/neural-trader/
├── sector-models/     # Primary models
├── models/           # Legacy compatibility
└── config/          # Configuration
```

## 🚨 Critical Issues We've Fixed

### 1. Data Loading (36 records vs 225,119)
- **Issue**: Using wrong table fallback
- **Fix**: Changed from `market_data_1m` to `market_data`
- **Fix**: Required minimum 100 records for training

### 2. Training Window (7 days vs 90 days)
- **Issue**: `min()` function always returned 7
- **Fix**: Use `training_history_days` directly

### 3. Architecture Deviation
- **Issue**: Individual stocks training full models
- **Fix**: Return empty vector for non-ETF symbols

### 4. DST Timezone Bug
- **Issue**: Markets "closed" at 3 PM during DST
- **Fix**: Use `convert_with_dst()` function

### 5. Model Persistence
- **Issue**: Models in two locations, not persisting
- **Fix**: Consolidated to `/opt/neural-trader`

## 🧪 Testing Coverage

The emergency test suite now covers:

1. **Two-Layer Architecture** (`test_two_layer_architecture`)
   - Verify ETF models are 320-512MB
   - Verify specializations are 6-8MB
   - Ensure individual stocks don't have primary models

2. **Autonomous Training** (`test_autonomous_training_triggers`)
   - Check environment variables
   - Verify market hours logic
   - Test data sufficiency checks

3. **Training Data Window** (`test_training_data_window`)
   - Verify 90-day window is used
   - Check min/max bounds
   - Test query generation

4. **Sector Assignment** (`test_sector_model_assignment`)
   - Verify ETF detection logic
   - Check stock→sector mapping
   - Ensure correct model selection

5. **Market Hours Priority** (`test_market_hours_priority`)
   - Test DST handling
   - Verify training deferral
   - Check priority switching

6. **Model Persistence** (`test_model_persistence_integrity`)
   - Verify Docker volumes
   - Check write permissions
   - Test backup mechanism

## 📈 Performance Characteristics

### Sector Models (Layer 1)
- **Size**: 320-512MB per model
- **Training Time**: 2-4 hours per model
- **Inference**: 5-10ms per prediction
- **Memory**: ~2GB for all sectors loaded

### Specializations (Layer 2)
- **Size**: 6-8MB per symbol
- **Training Time**: 5-10 minutes
- **Inference**: 1-2ms additional
- **Memory**: ~50MB per 10 symbols

## 🔧 Maintenance Guidelines

### Daily Operations
1. Monitor training logs for failures
2. Check model R² scores stay above 0.6
3. Verify data pipeline is flowing
4. Ensure models persist across restarts

### Weekly Tasks
1. Review model performance metrics
2. Check disk space for model storage
3. Validate backup integrity
4. Analyze training patterns

### Critical Alerts
- Model R² < 0.5: Immediate investigation
- Training failures > 3 consecutive: Alert
- Data gap > 1 hour: Check pipeline
- Memory usage > 80%: Scale or optimize

## 🚀 Future Improvements (Preserve During Refactoring)

### Must Preserve
- Two-layer architecture logic
- ETF→Sector mappings
- 90-day training window
- DST-aware market hours
- Autonomous training triggers
- Model persistence paths

### Can Refactor
- Split 3,300 lines into modules
- Extract validation logic
- Separate training from inference
- Create trait abstractions
- Improve error handling

### Proposed Module Structure
```
vendor_predictor/
├── mod.rs                    // Public API
├── config.rs                 // Configuration
├── models/
│   ├── sector.rs            // Layer 1 logic
│   ├── specialization.rs    // Layer 2 logic
│   └── pool.rs              // ClusterModelPool
├── training/
│   ├── autonomous.rs        // Auto training
│   ├── validation.rs        // Gates & checks
│   └── scheduler.rs         // Market hours
├── data/
│   ├── loader.rs           // Data loading
│   ├── normalization.rs    // Preprocessing
│   └── windowing.rs        // Time windows
└── inference/
    ├── predictor.rs        // Predictions
    └── decision.rs         // Trading decisions
```

---

**⚠️ WARNING**: This architecture is the result of extensive debugging and fixes. Any refactoring MUST preserve the core logic documented here, especially:
1. Two-layer sector model hierarchy
2. ETF-only primary training
3. 90-day data windows
4. DST-aware market hours
5. Consolidated persistence paths