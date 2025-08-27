# Deprecation Tracking - Phase 3 to Phase 4 Migration

## Overview
This document tracks the deprecation status of legacy modules as we migrate from the monolithic architecture to the 3-binary architecture.

## Deprecation Categories

### 🔴 RED - Remove Immediately (80% of codebase)
These files have been fully replaced and can be safely deleted.

#### Main Entry Points
- [ ] src/main.rs (1,371 lines) → DELETE
- [ ] src/lib.rs → DELETE  
- [ ] src/phase3.rs → DELETE

#### Neural Network Modules
- [ ] src/neural/mvp_predictor.rs → DELETE
- [ ] src/neural/vendor_predictor.rs (3,300 lines!) → DELETE
- [ ] src/neural/enhanced_predictor.rs → DELETE
- [ ] src/neural/batch_optimizer.rs → DELETE
- [ ] src/neural/training_coordinator.rs → DELETE
- [ ] src/neural/model_factory.rs → DELETE
- [ ] src/neural/emergency_model.rs → DELETE
- [ ] src/neural/memory_optimized_predictor.rs → DELETE

#### Feature Engineering
- [ ] src/features/mvp_features.rs → DELETE
- [ ] src/features/training_features.rs → DELETE
- [ ] src/features/feature_store.rs → DELETE
- [ ] src/features/feature_selection.rs → DELETE

#### Action/Execution Layer
- [ ] src/action_layer/* (entire directory) → DELETE

#### DAA Components
- [ ] src/daa/autonomous_training.rs → DELETE
- [ ] src/daa/compatibility_adapter.rs → DELETE
- [ ] src/daa/test_compatibility.rs → DELETE

#### Backtesting (Defer to Phase 4 - Rebuild Later)
- [ ] src/backtesting/monte_carlo.rs → DELETE
- [ ] src/backtesting/walk_forward.rs → DELETE
- [ ] src/backtesting/ab_testing.rs → DELETE
- [ ] src/backtesting/mvp_backtester.rs → DELETE
- [ ] src/backtesting/engine.rs → DELETE
- [ ] src/backtesting/mod.rs → DELETE

### 🟡 YELLOW - Migrate Then Remove (13% of codebase)
These files contain valuable logic that needs migration.

#### Configuration
- [ ] src/config/sector_models.rs → MIGRATE to config-store
- [ ] src/config/feature_flags.rs → MIGRATE to config-store
- [ ] src/config/enhanced_neural_config.rs → MIGRATE to neural-ml-ops

#### Data Processing
- [ ] src/data/sector_mapper.rs → MIGRATE to neural-ml-ops
- [ ] src/data/sector_aggregator.rs → MIGRATE to neural-ml-ops

#### Technical Indicators
- [ ] src/features/technical_indicators/* → MIGRATE to neural-ml-ops

#### Utilities
- [ ] src/utils/market_hours/* → MIGRATE to neural-core
- [ ] src/utils/symbol_loader.rs → MIGRATE to neural-core

### 🟢 GREEN - Keep For Now (7% of codebase)
These files are still actively used.

#### Proto Generation
- [x] src/proto/*.rs → KEEP (move to build.rs later)

#### Config Store Integration  
- [x] src/config_store_client/* → KEEP

#### MCP Integration
- [x] src/mcp/* → KEEP

## Migration Script

```bash
#!/bin/bash
# Phase 3 Deprecation Script

# Add deprecation warnings
echo "Adding deprecation warnings..."
for file in src/main.rs src/neural/*.rs src/features/*.rs src/action_layer/*.rs; do
  if [ -f "$file" ]; then
    sed -i '1i// ⚠️ DEPRECATED: See neural-core/, neural-ml-ops/, neural-trading/' "$file"
  fi
done

# Create backup before deletion
echo "Creating backup..."
tar -czf legacy_src_backup_$(date +%Y%m%d).tar.gz src/

# Track files for removal
echo "Generating removal list..."
find src -type f -name "*.rs" | while read file; do
  if grep -q "DEPRECATED" "$file"; then
    echo "$file" >> docs/migration/files_to_remove.txt
  fi
done

echo "Deprecation marking complete!"
```

## Validation Checklist

Before removing any file, ensure:
- [ ] Functionality is covered in new binaries
- [ ] Tests pass in new architecture
- [ ] No active imports from other modules
- [ ] Documentation updated
- [ ] Backup created

## Timeline

### Week 1 (Current)
- Add deprecation notices ✅
- Create migration report ✅
- Identify reusable components ✅

### Week 2
- Migrate technical indicators
- Migrate sector logic
- Extract utilities to neural-core

### Week 3
- Create backtesting service
- Migrate configuration to config-store
- Update build process for proto files

### Week 4
- Remove RED category files
- Test complete system
- Update documentation

### Week 5
- Final cleanup
- Remove all deprecated code
- Archive legacy codebase

## Metrics

- **Files to Remove**: 140
- **Files to Migrate**: 30
- **Files to Keep**: 12
- **Total LOC Reduction**: 37,000 lines (82%)

## Success Criteria

- [ ] All tests pass with new architecture
- [ ] No regression in functionality
- [ ] Performance metrics maintained or improved
- [ ] Clean separation between binaries
- [ ] No circular dependencies
- [ ] All modules < 500 lines

## Notes

- Priority: Remove largest files first (vendor_predictor.rs)
- Risk: Backtesting functionality is unique, needs careful migration
- Opportunity: 82% code reduction will significantly improve maintainability