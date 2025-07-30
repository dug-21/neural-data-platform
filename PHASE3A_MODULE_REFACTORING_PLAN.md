# Phase 3A Module Refactoring Plan

## Current Module Analysis

### Critical Issues Identified
1. **Oversized Modules** (exceeding 500 line limit):
   - `neural/fann_predictor.rs`: 3507 lines (7x over limit!)
   - `neural/enhanced_predictor.rs`: 1067 lines (2x over limit)
   - `adapters/model_rollback.rs`: 1144 lines (2x over limit)
   - `adapters/enhanced_neural_adapter.rs`: 973 lines (2x over limit)
   - `neural/fann_model_adapter.rs`: 837 lines
   - `neural/online_validator.rs`: 741 lines
   - `adapters/errors.rs`: 694 lines
   - `adapters/health_monitor.rs`: 712 lines
   - `adapters/fallback_manager.rs`: 805 lines
   - `adapters/model_storage.rs`: 897 lines

2. **Module Structure Issues**:
   - Mixed responsibilities in single files
   - Deep nesting without proper module hierarchy
   - Unclear boundaries between neural/adapters/integration

## Target Module Architecture (Phase 3A)

```
src/
├── neural/
│   ├── mod.rs (minimal, only re-exports)
│   ├── predictor.rs (main prediction logic, <500 lines)
│   ├── fann/
│   │   ├── mod.rs
│   │   ├── wrapper.rs (FANN-specific code)
│   │   ├── types.rs
│   │   ├── network.rs (network management)
│   │   ├── training.rs (training logic)
│   │   └── conversion.rs (data conversion)
│   └── monitoring/
│       ├── mod.rs
│       ├── metrics.rs (performance tracking)
│       ├── channel.rs (event bus implementation)
│       └── notifications.rs (training notifications)
│
├── adapters/
│   ├── mod.rs
│   └── neural/
│       ├── mod.rs
│       ├── core.rs (core adapter logic, <500 lines)
│       ├── fallback.rs (fallback mechanisms)
│       ├── health.rs (health monitoring)
│       ├── performance.rs (performance event emission)
│       └── errors.rs (error types)
│
└── integration/
    ├── notifications/
    │   ├── mod.rs
    │   ├── channel.rs (notification bus)
    │   └── types.rs (notification types)
    └── daa_coordinator.rs
```

## Detailed Refactoring Steps

### Step 1: Split `neural/fann_predictor.rs` (3507 lines)
**Current**: Single massive file containing all FANN logic
**Target**: Split into 8 focused modules

1. **Extract to `neural/fann/wrapper.rs`** (~400 lines)
   - Core FANN FFI wrapper
   - Network creation/destruction
   - Basic operations

2. **Extract to `neural/fann/types.rs`** (~200 lines)
   - FANN-specific types
   - Configuration structures
   - Enums and constants

3. **Extract to `neural/fann/network.rs`** (~450 lines)
   - Network management
   - Model loading/saving
   - Network state management

4. **Extract to `neural/fann/training.rs`** (~450 lines)
   - Training algorithms
   - Online learning
   - Training metrics

5. **Extract to `neural/fann/conversion.rs`** (~300 lines)
   - Input/output conversion
   - Data normalization
   - Feature mapping

6. **Keep in `neural/predictor.rs`** (~400 lines)
   - High-level prediction API
   - Trait implementations
   - Public interface

7. **Move to `neural/fann/validation.rs`** (~300 lines)
   - Model validation
   - Performance checks
   - Sanity tests

8. **Move test code to separate test files**

### Step 2: Split `neural/enhanced_predictor.rs` (1067 lines)
**Current**: Monolithic enhanced predictor
**Target**: Split into 3 modules

1. **Keep core in `neural/predictor.rs`** (~400 lines)
   - Main prediction logic
   - Public API
   - Trait implementations

2. **Extract to `neural/ensemble.rs`** (~350 lines)
   - Ensemble logic
   - Model combination
   - Voting mechanisms

3. **Extract to `neural/optimization.rs`** (~300 lines)
   - Performance optimization
   - Caching logic
   - Resource management

### Step 3: Split `adapters/enhanced_neural_adapter.rs` (973 lines)
**Current**: Large adapter with mixed concerns
**Target**: Split into focused modules

1. **Keep in `adapters/neural/core.rs`** (~400 lines)
   - Core adapter logic
   - Main API
   - Coordination

2. **Extract to `adapters/neural/circuit_breaker.rs`** (~200 lines)
   - Circuit breaker implementation
   - State management
   - Recovery logic

3. **Extract to `adapters/neural/metrics.rs`** (~200 lines)
   - Performance metrics
   - Event emission
   - Monitoring integration

4. **Move error handling to `adapters/neural/errors.rs`** (~150 lines)

### Step 4: Split `adapters/model_rollback.rs` (1144 lines)
**Current**: Complex rollback system
**Target**: Split into smaller components

1. **Keep core in `adapters/neural/rollback.rs`** (~400 lines)
   - Rollback API
   - Coordination logic

2. **Extract to `adapters/neural/versioning.rs`** (~350 lines)
   - Version management
   - Model history
   - Comparison logic

3. **Extract to `adapters/neural/storage.rs`** (~350 lines)
   - Model persistence
   - File operations
   - Database integration

### Step 5: Reorganize Other Oversized Modules

1. **Split `adapters/errors.rs`** (694 lines)
   - Move to `adapters/neural/errors.rs` (~350 lines)
   - Create `adapters/common/errors.rs` (~300 lines)

2. **Split `adapters/health_monitor.rs`** (712 lines)
   - Keep core in `adapters/neural/health.rs` (~400 lines)
   - Extract metrics to `adapters/neural/health_metrics.rs` (~300 lines)

3. **Split `adapters/fallback_manager.rs`** (805 lines)
   - Keep core in `adapters/neural/fallback.rs` (~400 lines)
   - Extract strategies to `adapters/neural/fallback_strategies.rs` (~400 lines)

## File Movement Plan

### Phase 1: Create New Module Structure
```bash
mkdir -p src/neural/fann
mkdir -p src/neural/monitoring
mkdir -p src/adapters/neural
mkdir -p src/integration/notifications
```

### Phase 2: Split and Move Files
1. Start with `neural/fann_predictor.rs` - highest priority
2. Then `adapters/enhanced_neural_adapter.rs`
3. Continue with other oversized modules
4. Update imports after each move
5. Run `cargo check` after each module split

### Phase 3: Update Module Declarations
1. Update `src/neural/mod.rs`
2. Update `src/adapters/mod.rs`
3. Update `src/lib.rs`
4. Fix all import paths

## Success Metrics
- [ ] All modules under 500 lines
- [ ] Clear module boundaries
- [ ] Zero compilation errors
- [ ] All tests passing
- [ ] Performance channel fully integrated
- [ ] Training notifications implemented

## Risk Mitigation
1. **Incremental Changes**: One module at a time
2. **Frequent Checks**: Run `cargo check` after each change
3. **Git Commits**: Commit after each successful module split
4. **Test Coverage**: Ensure tests still pass after refactoring
5. **Backup**: Keep original files until refactoring complete

## Timeline Estimate
- Day 1: Split fann_predictor.rs and enhanced_predictor.rs
- Day 2: Split adapter modules and fix compilation errors
- Day 3: Complete performance channel integration
- Day 4: Implement training notifications and final testing

## Next Steps
1. Begin with splitting `neural/fann_predictor.rs`
2. Create new module structure
3. Extract code into focused modules
4. Update imports and module declarations
5. Verify compilation and tests