# Technical Debt Cleanup Phase 1 - SPARC Implementation Plan

## Overview

This directory contains the complete SPARC methodology implementation plan for fixing critical architectural violations in the neural-trader system.

## SPARC Documents

1. **[1_SPECIFICATION.md](1_SPECIFICATION.md)** - Requirements, constraints, and success criteria
2. **[2_PSEUDOCODE.md](2_PSEUDOCODE.md)** - High-level implementation approach
3. **[3_ARCHITECTURE.md](3_ARCHITECTURE.md)** - Technical architecture and design
4. **[4_REFINEMENT.md](4_REFINEMENT.md)** - Detailed implementation steps
5. **[5_COMPLETION.md](5_COMPLETION.md)** - Final guide and verification checklist

## Critical Issues Addressed

### 🔴 Issue 1: Model Routing Violations
- **Problem**: Neural predictions bypass ruv-fann through mock adapters
- **Solution**: Remove mock adapters, enforce central routing through FannPredictor
- **Files**: `src/adapters/neuro_divergent.rs` (remove), `src/neural/fann_predictor.rs` (update)

### 🔴 Issue 2: DAA Orchestration Gaps
- **Problem**: Autonomous training not orchestrated, components uninitialized
- **Solution**: Initialize all DAA components, implement orchestration loop
- **Files**: `src/integration/daa_coordinator.rs` (update)

### 🔴 Issue 3: Broken Feedback Loop
- **Problem**: Performance metrics never reach training decisions
- **Solution**: Implement PerformanceTrainingBridge, connect event channels
- **Files**: `src/integration/performance_training_bridge.rs` (new)

### 🔴 Issue 4: Mock Code Pollution
- **Problem**: NeuroDivergentAdapter contains only stubbed implementations
- **Solution**: Complete removal of mock adapter and all references
- **Files**: Multiple files require import updates

## Implementation Timeline

| Phase | Days | Description | Priority |
|-------|------|-------------|----------|
| 1 | 1-3 | Mock Adapter Removal | 🔴 Critical |
| 2 | 4-8 | Routing Centralization | 🔴 Critical |
| 3 | 9-13 | DAA Integration | 🔴 Critical |
| 4 | 14-17 | Feedback Loop Connection | 🟡 High |
| 5 | 18-20 | Testing & Validation | 🟡 High |

**Total Duration: 20 days (4 weeks)**

## Quick Start

1. **Review Requirements**: Start with [1_SPECIFICATION.md](1_SPECIFICATION.md)
2. **Understand Approach**: Read [2_PSEUDOCODE.md](2_PSEUDOCODE.md)
3. **Study Architecture**: Review [3_ARCHITECTURE.md](3_ARCHITECTURE.md)
4. **Follow Implementation**: Use [4_REFINEMENT.md](4_REFINEMENT.md) for coding
5. **Verify Completion**: Check against [5_COMPLETION.md](5_COMPLETION.md)

## Key Changes

### Before
```rust
// Multiple paths to predictions
enhanced_adapter -> neuro_divergent_adapter -> Mock implementations
enhanced_adapter -> fann_predictor -> ruv-fann
// DAA components optional
autonomous_training: Option<Arc<AutonomousTrainingEngine>> // Often None
```

### After
```rust
// Single enforced path
enhanced_adapter -> fann_predictor -> ruv-fann (ONLY PATH)
// DAA components required
autonomous_training: Arc<AutonomousTrainingEngine> // Always initialized
```

## Success Metrics

- ✅ 100% of neural predictions routed through ruv-fann
- ✅ Zero mock adapter references remain
- ✅ DAA orchestration running continuously
- ✅ Performance metrics trigger training decisions
- ✅ All tests passing

## Risk Mitigation

- **Feature Flags**: Gradual rollout with environment variables
- **Rollback Plan**: Quick disable via configuration
- **Testing**: Comprehensive unit and integration tests
- **Monitoring**: New metrics and alerts for verification

## Related Documentation

- Parent: [products/features/techdebtcleanup1/README.md](../README.md)
- Analysis: [products/features/techdebtcleanup1/NEURAL_TRADING_SYSTEM_ARCHITECTURE.md](../NEURAL_TRADING_SYSTEM_ARCHITECTURE.md)
- Feedback: [products/features/techdebtcleanup1/feedback_loop_analysis.md](../feedback_loop_analysis.md)

## Questions?

For questions about this implementation plan:
1. Review the specific SPARC document for your phase
2. Check the completion checklist in [5_COMPLETION.md](5_COMPLETION.md)
3. Consult the architecture diagrams in [3_ARCHITECTURE.md](3_ARCHITECTURE.md)

---

*Generated using SPARC methodology for systematic implementation planning*