# INTEGRATION-FIRST DEVELOPMENT MANDATE

## The Prime Directive
**INTEGRATE, DON'T DUPLICATE**

Every new capability MUST integrate with existing systems. Building parallel systems is FORBIDDEN.

## The Three Laws of Integration

### 1. READ BEFORE YOU BUILD
Before creating ANY new file or module:
- ✅ SEARCH for existing functionality with similar purpose
- ✅ READ the current implementation thoroughly  
- ✅ IDENTIFY integration points, not replacement opportunities
- ❌ NEVER create new modules that duplicate existing functionality

### 2. EXTEND, DON'T REPLACE
When adding capabilities:
- ✅ ADD methods to existing classes/traits
- ✅ ENHANCE existing data structures
- ✅ USE existing communication channels (Redis pub/sub, DAA coordinator)
- ❌ NEVER create parallel implementations "on the side"

### 3. TEST IN PRODUCTION FLOW
Every new feature MUST:
- ✅ BE CALLED by existing code paths
- ✅ SHOW UP in logs during normal operation
- ✅ AFFECT actual trading decisions
- ❌ NEVER exist as isolated, untested modules

## Integration Checkpoints

### Before Starting Any Task
Ask and verify:
1. **Where does this functionality currently exist?**
   - Check: `src/integration/daa_coordinator.rs` for decision-making
   - Check: `src/neural/fann/predictor.rs` for neural models
   - Check: `src/features/` for feature extraction

2. **What existing interface should I extend?**
   - Find the trait/interface that defines this behavior
   - Add new methods to existing traits
   - Extend existing structs with new fields

3. **How will existing code call this?**
   - Trace the execution path from main entry points
   - Ensure your code is in the active call chain
   - Verify it affects real decisions

### Red Flags to Avoid
🚫 Creating new directories like `src/neural/neuralfix/` when `src/neural/` exists  
🚫 Building "enhanced" versions instead of enhancing the original  
🚫 Writing adapters/bridges between duplicate systems  
🚫 Implementing features that are never called  

### Green Flags to Follow
✅ Adding fields to existing structs  
✅ Implementing new methods on existing traits  
✅ Extending existing enums with new variants  
✅ Modifying existing functions to call new logic
✅ Goal is to write less code not more


## The Integration Test

Before ANY commit, verify:
1. **grep** for your new code being called from existing paths
2. **Run** the system and see your code in action
3. **Log** showing your code affecting decisions
4. **No** orphaned files or unused modules

## Example: Adding Multi-Modal Data

❌ **WRONG**: Create `src/features/multi_modal/` separately
✅ **RIGHT**: Extend `FannPredictor::prepare_features()` to include new data types

❌ **WRONG**: Build new `EnhancedNeuralAdapter`  
✅ **RIGHT**: Add methods to existing `enhanced_neural_adapter.rs`

❌ **WRONG**: Create parallel voting system
✅ **RIGHT**: Extend `DAACoordinator::get_strategy_signals()` with new signals

## CRITICAL EXCEPTION: Neural Engine Replacement

**🚨 NEURAL ENGINE EXCEPTION**: The current neural factory system has a fundamental incompatibility that requires complete replacement, not integration.

### Why This Exception is Necessary

The existing `src/neural/fann/` system creates **fake models** (LSTM/TCN are actually basic MLPs) instead of using real vendor models from `vendor/ruv-fann`. This creates an insurmountable architectural incompatibility:

- **Current**: `Network<f32>` (basic FANN networks)
- **Required**: `BaseModel<T>` trait (27+ real neural architectures)

### Neural Engine Exception Scope

**✅ EXEMPT from integration-first mandate:**
- Complete replacement of `src/neural/fann/` neural factory system
- Direct implementation with vendor models (no adapters)
- Neural prediction and training logic

**✅ MUST STILL INTEGRATE with existing systems:**
- **vendor/ruv-fann usage**: MANDATORY - use real vendor models
- **DAA autonomous training**: MANDATORY - preserve autonomous capabilities  
- **Market-time data processing**: MANDATORY - maintain real-time trading
- **Performance tracking**: MANDATORY - feed data to DAA decisions
- **Redis pub/sub communication**: MANDATORY - use existing channels
- **Logging and monitoring**: MANDATORY - integrate with existing health checks

### Implementation Requirements

When replacing the neural engine:

1. **Preserve DAA Integration**: New neural system MUST work with existing `DAACoordinator`
2. **Maintain Performance Tracking**: MUST feed performance data to DAA training decisions
3. **Keep Communication Channels**: MUST use existing Redis pub/sub for coordination
4. **Use Vendor Models**: MUST use real `BaseModel<T>` implementations, not fake networks
5. **Preserve Market Timing**: MUST maintain real-time market data processing capabilities

## Enforcement

Every PR must demonstrate:
1. **Integration points** - Show where existing code calls new functionality
2. **Execution traces** - Logs proving the code runs in production flow
3. **Decision impact** - Evidence that changes affect trading decisions
4. **No duplicates** - Verification that no parallel systems were created

**Exception**: Neural engine replacement PRs must demonstrate preservation of DAA, vendor model usage, and market-time capabilities instead of traditional integration points.

## Remember

The neural-trader is a **production system** making **real autonomous decisions**. Every line of code must integrate into this living system, not exist alongside it.

**When in doubt: EXTEND, don't CREATE**
**Neural Exception: BUILD CORRECTLY with vendor models, preserve DAA autonomy**