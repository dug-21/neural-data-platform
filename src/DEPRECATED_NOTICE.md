# ⚠️ DEPRECATION NOTICE - Legacy src/ Directory

## Status: DEPRECATED as of Phase 3 Implementation

This directory contains the legacy monolithic implementation of Neural Trader.
It has been replaced by a modern 3-binary architecture:

### New Architecture (Use These Instead):
- **neural-core/** - Shared library for common types and traits
- **neural-ml-ops/** - Domain-agnostic ML operations and training
- **neural-trading/** - Trading execution with DAA Coordinator

## Migration Status

### ✅ Fully Migrated (Safe to Remove)
- main.rs → Replaced by 3 separate binaries
- neural/* → Migrated to neural-ml-ops
- action_layer/* → Migrated to neural-trading
- daa/* → Migrated to neural-trading
- features/* → Migrated to neural-ml-ops

### ⚠️ Partially Migrated (Review Before Removal)
- config/* → Config-store integration pending
- backtesting/* → Separate service planned (Phase 4)
- monitoring/health/* → Infrastructure service planned

### 📋 Still Required (Do Not Remove Yet)
- proto/*.rs → Build dependencies (will move to build.rs)
- config_store_client/* → Active integration
- mcp/* → Claude Code integration

## Removal Timeline
- **Phase 3** (Current): Deprecation markers added
- **Phase 4** (Next Sprint): Begin gradual removal
- **Phase 5** (Future): Complete cleanup

## For Developers

**DO NOT** add new code to this directory.
**DO** use the new binary structure for all new development:
- Training/ML code → neural-ml-ops/
- Trading logic → neural-trading/
- Shared types → neural-core/

See `/docs/architecture/LEGACY_CODE_MIGRATION_REPORT.md` for detailed migration guide.

---
Generated: 2024-08-24
Last Updated: 2024-08-24