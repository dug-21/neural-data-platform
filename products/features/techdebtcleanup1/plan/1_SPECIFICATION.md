# Technical Debt Cleanup Phase 1 - Specification

## Project Overview

**Goal**: Fix critical architectural violations in the neural-trader system to ensure all neural predictions route through ruv-fann, establish proper DAA orchestration, and connect the performance-training feedback loop.

## Problem Statement

The neural-trader system has three critical architectural violations:

1. **Model Routing Bypass**: Neural predictions bypass ruv-fann through mock adapters
2. **DAA Orchestration Failure**: Autonomous training decisions are not orchestrated
3. **Broken Feedback Loop**: Performance metrics never reach training decisions

## Requirements

### Functional Requirements

#### FR1: Centralized Neural Routing
- All neural model predictions MUST route through ruv-fann
- No direct adapter access allowed
- Remove all mock model implementations
- Enforce routing at compile time

#### FR2: DAA Training Orchestration
- DAA Coordinator MUST orchestrate training vs trading decisions
- Market timing awareness MUST influence training decisions
- Training scheduler MUST be integrated with DAA
- Autonomous training engine MUST be initialized

#### FR3: Performance Feedback Loop
- Performance metrics MUST reach training engine
- Data structure conversion MUST be implemented
- Continuous evaluation loop MUST be established
- Event channels MUST connect components

#### FR4: Mock Adapter Removal
- Remove `/src/adapters/neuro_divergent.rs` completely
- Remove all references to MockDeepAR and MockTCN
- Ensure vendor library is accessed only through ruv-fann

### Non-Functional Requirements

#### NFR1: Performance
- No degradation in prediction latency
- Maintain sub-second response times
- Efficient memory usage

#### NFR2: Reliability
- Graceful fallback mechanisms
- No single point of failure
- Comprehensive error handling

#### NFR3: Maintainability
- Clear separation of concerns
- Well-documented interfaces
- Testable components

#### NFR4: Observability
- Performance metrics collection
- Training decision logging
- Model routing traceability

## Constraints

1. **Technical Constraints**
   - Must maintain backward compatibility with existing APIs
   - Cannot modify vendor/ruv-fann library code
   - Must work with existing data structures

2. **Time Constraints**
   - Implementation must be completed in phases
   - Critical fixes must be prioritized
   - Testing must not be skipped

3. **Resource Constraints**
   - Limited refactoring scope
   - Existing team knowledge
   - Current infrastructure

## Success Criteria

1. **Routing Verification**
   - 100% of neural predictions go through ruv-fann
   - Zero direct adapter calls
   - Compile-time enforcement

2. **DAA Integration**
   - Autonomous training decisions based on performance
   - Market timing respected in training
   - Continuous orchestration loop running

3. **Feedback Loop**
   - Performance metrics reach training engine
   - Training triggered by performance degradation
   - Metrics stored and retrievable

4. **Code Quality**
   - All mock implementations removed
   - No unwrap/panic in critical paths
   - Comprehensive test coverage

## Acceptance Criteria

### AC1: Model Routing
```rust
// This should be the ONLY way to get predictions
let predictions = fann_predictor.predict(data, horizon, features).await?;
// Direct adapter access should not compile
```

### AC2: DAA Orchestration
```rust
// DAA should decide autonomously
match daa_coordinator.orchestrate_operations().await? {
    AutonomousAction::InitiateTraining => // Start training
    AutonomousAction::ContinueTrading => // Keep trading
}
```

### AC3: Performance Feedback
```rust
// Metrics should flow automatically
performance_monitor.collect() → bridge.convert() → training_engine.evaluate()
```

### AC4: Clean Architecture
```
src/
├── adapters/
│   ├── neuro_divergent.rs  // REMOVED
│   └── enhanced_neural_adapter.rs  // UPDATED to use fann_predictor only
├── neural/
│   └── fann_predictor.rs  // CENTRAL routing point
└── integration/
    └── daa_coordinator.rs  // UPDATED with training orchestration
```

## Risk Assessment

### High Risk
- Breaking existing prediction APIs
- Performance degradation during refactoring
- Incomplete mock removal causing runtime errors

### Medium Risk
- Data structure incompatibilities
- Complex integration testing
- Team learning curve

### Low Risk
- Documentation updates
- Configuration changes
- Monitoring additions

## Dependencies

1. **Internal Dependencies**
   - ruv-fann library (vendor)
   - FannPredictor implementation
   - DAA Coordinator
   - Performance monitoring

2. **External Dependencies**
   - None (all vendor code is local)

## Timeline Estimate

- Phase 1: Mock Adapter Removal (3 days)
- Phase 2: Routing Centralization (5 days)
- Phase 3: DAA Integration (5 days)
- Phase 4: Feedback Loop (4 days)
- Phase 5: Testing & Validation (3 days)
- **Total: 20 days (4 weeks)**