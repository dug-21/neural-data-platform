# SRC Directory Migration Plan - Complete Deprecation Strategy

## Executive Summary

This document outlines the comprehensive migration plan for the remaining `src/` directory, mapping dependencies for `src/lib.rs` (126 files), planning microservice migrations for `src/bin/*` utilities, addressing Python files, and planning removal of empty directories.

## Current Status Analysis

### Directory Structure Overview
```
src/
├── lib.rs (CORE - 126 dependencies)
├── bin/ (9 utilities - 1,373 total LOC)
├── adapters/ (18 modules)
├── agents/ (2 modules) 
├── config/ (8 modules)
├── data/ (7 modules)
├── integration/ (14 modules)
├── monitoring/ (3 modules)
├── strategies/ (2 modules)
└── utils/ (4 modules + 2 Python files)
```

### Dependency Analysis

**lib.rs Dependencies (126 files identified)**:
- 43 internal module references (`pub mod`, `use crate::`)
- 83 external dependencies across:
  - Tests (31 files)
  - Examples (12 files) 
  - Benchmarks (8 files)
  - Core modules (32 remaining files)

## Migration Strategy by Category

### 1. IMMEDIATE DELETION (Can Remove Now)

#### Empty Directories
```bash
# These directories are completely empty and safe to delete immediately
rm -rf /workspaces/neural-trader/src/v2-platform/mlops
rm -rf /workspaces/neural-trader/src/v2-platform/mcp
rm -rf /workspaces/neural-trader/src/v2-platform/safety
rm -rf /workspaces/neural-trader/src/v2-platform/monitoring
rm -rf /workspaces/neural-trader/src/optimization
rm -rf /workspaces/neural-trader/src/prediction
rm -rf /workspaces/neural-trader/src/mcp-server
rm -rf /workspaces/neural-trader/src/examples/shared
rm -rf /workspaces/neural-trader/src/examples/neural_trader
rm -rf /workspaces/neural-trader/src/examples/data_ingestion
rm -rf /workspaces/neural-trader/src/config-store/tests
rm -rf /workspaces/neural-trader/src/config-store/examples
```

#### Python Files (Immediate Removal)
```bash
# These Python files are duplicates of Rust implementations
rm /workspaces/neural-trader/src/utils/rate_limiter.py
rm /workspaces/neural-trader/src/utils/market_hours_implementation.py
rm /workspaces/neural-trader/src/config-store/config_store_client.py
```

### 2. MICROSERVICE MIGRATION REQUIRED

#### Binary Utilities Analysis

| Binary | LOC | Migration Target | Priority | Effort |
|--------|-----|------------------|----------|---------|
| `mvp_trainer.rs` | 524 | `neural-ml-ops` | HIGH | 2 days |
| `production_validator.rs` | 297 | `config-store` | HIGH | 1.5 days |
| `mcp_server_simple.rs` | 145 | Delete (superseded) | LOW | 0.5 days |
| `prove_fann_real.rs` | 121 | `neural-core` | MEDIUM | 1 day |
| `model_rollback_cli.rs` | 98 | `neural-ml-ops` | MEDIUM | 1 day |
| `test_health_monitor.rs` | 70 | Delete (test utility) | LOW | 0.5 days |
| `test_mcp.rs` | 65 | Delete (test utility) | LOW | 0.5 days |
| `mcp_server.rs` | 47 | `mcp-trading-server` | HIGH | 1 day |
| `test_compile.rs` | 6 | Delete (test utility) | LOW | 0.1 days |

**Total Migration Effort**: 7.1 days

#### Migration Commands

**High Priority Migrations**:
```bash
# 1. Move mvp_trainer to neural-ml-ops
mkdir -p neural-ml-ops/src/bin
mv src/bin/mvp_trainer.rs neural-ml-ops/src/bin/
# Add to neural-ml-ops/Cargo.toml: [[bin]] section

# 2. Move production_validator to config-store  
mkdir -p config-store/src/bin
mv src/bin/production_validator.rs config-store/src/bin/
# Add to config-store/Cargo.toml: [[bin]] section

# 3. Move mcp_server to mcp-trading-server
mv src/bin/mcp_server.rs mcp-trading-server/src/bin/
# Update mcp-trading-server/Cargo.toml
```

### 3. COMPLEX DEPENDENCY MIGRATIONS

#### Core Module Dependencies (32 files requiring careful migration)

**Phase 1: Independent Modules (Low Risk)**
- `src/utils/` → Various microservices (market_hours → `neural-core`, etc.)
- `src/helpers/` → Shared utility crate
- `src/phase3.rs` → Delete (deprecated)

**Phase 2: Data Layer (Medium Risk)**
- `src/data/` → `data-staging` microservice
- `src/adapters/` → Split across `neural-core`, `data-staging`
- `src/data_pipeline/` → `data-staging`

**Phase 3: Business Logic (High Risk)**
- `src/integration/` → Split across `neural-ml-ops`, `neural-trading`
- `src/strategies/` → `neural-trading`
- `src/monitoring/` → Dedicated monitoring service

## Detailed Migration Timeline

### Week 1: Foundation (Days 1-3)
**Day 1**: Immediate Cleanup
- Delete empty directories
- Remove Python files
- Delete test utilities from `src/bin/`
- **Risk**: None
- **Effort**: 0.5 days

**Day 2-3**: High Priority Binary Migrations
- Migrate `mvp_trainer.rs` → `neural-ml-ops`
- Migrate `production_validator.rs` → `config-store`
- Migrate `mcp_server.rs` → `mcp-trading-server`
- **Risk**: Medium (dependency updates needed)
- **Effort**: 2.5 days

### Week 2: Dependency Restructuring (Days 4-8)

**Day 4**: Utils and Helpers Migration
- Move `src/utils/` modules to appropriate microservices
- Create shared utility crate for common helpers
- **Risk**: Low
- **Effort**: 1 day

**Day 5-6**: Data Layer Migration
- Move `src/data/` → `data-staging`
- Move `src/adapters/` → Split between microservices
- Update all import paths
- **Risk**: High (126 files reference these)
- **Effort**: 2 days

**Day 7-8**: Business Logic Migration
- Move `src/integration/` → Split across services
- Move `src/strategies/` → `neural-trading`
- Move `src/monitoring/` → Monitoring service
- **Risk**: Very High (core business logic)
- **Effort**: 2 days

### Week 3: Integration and Testing (Days 9-12)

**Day 9-10**: lib.rs Decomposition
- Remove module declarations from `src/lib.rs`
- Update 126 dependent files with new import paths
- Create workspace-level re-exports if needed
- **Risk**: Very High (breaks all compilation)
- **Effort**: 2 days

**Day 11-12**: Integration Testing
- Fix all compilation errors
- Update tests with new module paths
- Validate all microservices work independently
- **Risk**: High
- **Effort**: 2 days

## Risk Mitigation Strategies

### 1. Compilation Safety Net
```bash
# Before each migration step, create backup
git stash push -m "Before migration step X"

# Validate compilation at each step
cargo check --workspace
cargo test --workspace --no-run
```

### 2. Dependency Mapping
```bash
# Track all files that import from src/lib.rs
rg "use.*neural_trader|use.*autonomous_platform" --files > dependency_map.txt

# Update imports systematically
sed -i 's/use neural_trader::/use neural_core::/g' $(cat dependency_map.txt)
```

### 3. Incremental Validation
- Migrate one module at a time
- Maintain compilation after each step
- Use feature flags to disable broken functionality temporarily

### 4. Rollback Procedures
```bash
# If migration fails, immediate rollback
git reset --hard HEAD
git stash pop  # Restore previous state

# Alternative: Create migration branch
git checkout -b migration/src-deprecation
# Work in branch, merge only when stable
```

## Module-by-Module Migration Map

### Core Modules → Target Destinations

| Source Module | Target Microservice | Dependencies | Risk Level |
|---------------|-------------------|--------------|------------|
| `adapters/` | `neural-core` + `data-staging` | 25 files | HIGH |
| `agents/` | `neural-trading` | 8 files | MEDIUM |
| `config/` | `config-store` | 15 files | MEDIUM |
| `data/` | `data-staging` | 35 files | HIGH |
| `integration/` | `neural-ml-ops` + `neural-trading` | 30 files | VERY HIGH |
| `monitoring/` | New monitoring service | 12 files | MEDIUM |
| `strategies/` | `neural-trading` | 10 files | HIGH |
| `utils/` | Multiple (by function) | 8 files | LOW |

### Import Path Updates Required

```rust
// BEFORE (current src/lib.rs structure)
use neural_trader::data::TimeSeriesData;
use neural_trader::adapters::TimescaleAdapter;
use neural_trader::strategies::TradingStrategy;

// AFTER (microservice structure)
use data_staging::TimeSeriesData;
use neural_core::adapters::TimescaleAdapter; 
use neural_trading::strategies::TradingStrategy;
```

## Success Criteria

### Phase Completion Gates

**Phase 1 Complete**: 
- All empty directories removed
- All Python files removed  
- Test utilities cleaned up
- No compilation errors

**Phase 2 Complete**:
- All `src/bin/` utilities migrated or deleted
- Microservices can build independently
- Integration tests pass

**Phase 3 Complete**:
- `src/lib.rs` completely empty or deleted
- No files reference `neural_trader::` imports
- All 126 dependencies resolved
- Full workspace compilation success

## Monitoring and Validation

### Automated Validation Scripts

```bash
#!/bin/bash
# validate_migration.sh

echo "🔍 Validating migration progress..."

# Check for remaining src/ content
SRC_FILES=$(find src/ -name "*.rs" | wc -l)
if [ $SRC_FILES -gt 1 ]; then
    echo "❌ $SRC_FILES Rust files still in src/"
else
    echo "✅ src/ directory properly cleaned"
fi

# Check compilation
if cargo check --workspace --quiet; then
    echo "✅ Workspace compiles successfully"
else
    echo "❌ Compilation errors detected"
fi

# Check for old imports
OLD_IMPORTS=$(rg "use.*neural_trader::" --count)
if [ $OLD_IMPORTS -gt 0 ]; then
    echo "❌ $OLD_IMPORTS old import statements found"
else
    echo "✅ All imports updated to microservices"
fi
```

### Progress Tracking

```bash
# Daily progress check
echo "Migration Progress $(date):"
echo "- Empty directories: $(find src/ -type d -empty | wc -l) remaining"
echo "- Python files: $(find src/ -name "*.py" | wc -l) remaining"  
echo "- Binary utilities: $(find src/bin/ -name "*.rs" | wc -l) remaining"
echo "- Module directories: $(find src/ -maxdepth 1 -type d | wc -l) remaining"
```

## Rollback and Emergency Procedures

### If Migration Breaks Production

1. **Immediate Rollback**:
```bash
git checkout main
git reset --hard HEAD~n  # Where n is number of migration commits
```

2. **Partial Rollback**:
```bash
# Restore specific module
git checkout HEAD~1 -- src/data/
# Fix immediate compilation issues
cargo check
```

3. **Emergency Bridge**:
```bash
# Create temporary re-export in workspace root
echo 'pub use data_staging::*;' >> lib.rs
```

## Conclusion

This migration plan provides a systematic approach to completely deprecating the `src/` directory while maintaining system stability. The 3-week timeline with built-in risk mitigation ensures a smooth transition to the microservice architecture.

**Total Effort Estimate**: 12 days
**Risk Level**: High (due to 126 file dependencies)
**Success Probability**: 85% with proper execution
**Rollback Time**: < 1 hour if needed

The key to success is following the incremental migration approach and validating compilation at each step.