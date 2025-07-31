# Phase 2 Implementation Checklist

## For Implementation Agents

### Step 2.1: Core Routing Implementation
- [ ] Add `execute_model()` method to FannPredictor
  - [ ] Public visibility
  - [ ] Performance timer start/stop
  - [ ] Error handling with event emission
  - [ ] Delegate to private `route_model_request()`

- [ ] Add `execute_ensemble()` method
  - [ ] Public visibility  
  - [ ] Parallel execution using tokio::spawn
  - [ ] Aggregate performance events
  - [ ] Weighted result combination

- [ ] Make network methods private
  - [ ] `create_fann_network()` → private
  - [ ] `get_or_create_network()` → private
  - [ ] `prepare_training_data()` → private
  - [ ] Remove any pub(crate) network access

### Step 2.2: Performance Channel Integration
- [ ] Add performance_channel field to FannPredictor
  ```rust
  performance_channel: Option<Arc<PerformanceChannel>>,
  ```

- [ ] Add constructor with performance channel
  ```rust
  pub fn new_with_performance_channel(
      config: NeuralConfig,
      performance_channel: Arc<PerformanceChannel>,
  ) -> Result<Self>
  ```

- [ ] Implement `emit_performance_event()`
  - [ ] Calculate accuracy from predictions
  - [ ] Calculate confidence scores
  - [ ] Build performance metrics
  - [ ] Emit to channel

- [ ] Add error event emission
  - [ ] Capture prediction failures
  - [ ] Include error details
  - [ ] Maintain error metrics

### Step 2.3: Update NeuralPredictorTrait Implementation
- [ ] Update `predict()` to use `execute_model()`
- [ ] Update `predict_ensemble()` to use `execute_ensemble()`
- [ ] Remove any direct network access
- [ ] Ensure all paths go through central router

### Step 2.4: Module Export Updates (src/neural/mod.rs)
- [ ] Export only public API types
- [ ] Hide internal configs
- [ ] Hide network types
- [ ] Export performance types

### Step 2.5: Testing Implementation
- [ ] Routing verification tests
  - [ ] Test all predictions use execute_model
  - [ ] Test cannot access private methods
  - [ ] Test performance events emitted

- [ ] Performance benchmarks
  - [ ] Measure routing overhead
  - [ ] Measure event emission cost
  - [ ] Compare with direct execution

- [ ] Integration tests
  - [ ] Test with performance channel
  - [ ] Test without performance channel
  - [ ] Test error scenarios

## Validation Criteria

### Must Pass
1. All existing tests still pass
2. Cannot compile code that bypasses routing
3. Performance events emitted for every prediction
4. Less than 1% performance overhead
5. Feature flag properly controls behavior

### Performance Targets
- Routing overhead: < 100 microseconds
- Event emission: < 50 microseconds  
- Memory overhead: < 1KB per prediction
- No additional allocations in hot path

## Code Review Checklist

### Security
- [ ] No public access to network creation
- [ ] No export of internal types
- [ ] Proper error handling
- [ ] No sensitive data in events

### Performance  
- [ ] Inline hints on hot paths
- [ ] No unnecessary allocations
- [ ] Efficient event batching
- [ ] Proper async/await usage

### Maintainability
- [ ] Clear method naming
- [ ] Comprehensive documentation
- [ ] Proper error messages
- [ ] Consistent code style

## Rollback Plan

If issues arise:
1. Performance regression > 5%
   - Revert to direct execution
   - Add feature flag override
   
2. Event channel overload
   - Add event filtering
   - Implement sampling
   
3. API compatibility issues
   - Add compatibility shim
   - Deprecation warnings

## Communication Protocol

### Between Agents
- Use memory keys: `swarm/phase2/[agent]/[component]`
- Notify on major milestones
- Request review before integration
- Share performance metrics

### Key Memory Keys
- `swarm/phase2/router/implementation` - Router code
- `swarm/phase2/performance/integration` - Performance channel
- `swarm/phase2/tests/results` - Test outcomes
- `swarm/phase2/benchmarks/results` - Performance data

## Success Metrics

1. **Code Coverage**: 95%+ on routing logic
2. **Performance**: <1% overhead confirmed
3. **Events**: 100% prediction coverage
4. **Compilation**: Bypass attempts fail to compile
5. **Integration**: All components updated