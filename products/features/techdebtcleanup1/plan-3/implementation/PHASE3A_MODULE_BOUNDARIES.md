# Phase 3A Module Boundaries Definition

## 🎯 Module Organization Rules

### Core Principle: Single Responsibility + Size Limit
Each module must:
1. Have a single, well-defined responsibility
2. Be under 500 lines of code
3. Have clear public API boundaries
4. Minimize dependencies on other modules

## 📁 Module Structure Template

```rust
//! Module: [module_name]
//! Purpose: [Single sentence description]
//! Size: [Current lines] / 500 max
//! Dependencies: [List key dependencies]

// Section 1: Imports (max 20 lines)
use std::...;
use tokio::...;
use crate::...;

// Section 2: Constants & Types (max 30 lines)
pub const MODULE_VERSION: &str = "1.0.0";
pub type ModuleResult<T> = Result<T, ModuleError>;

// Section 3: Public Traits (max 50 lines)
pub trait ModuleTrait {
    // Core interface
}

// Section 4: Main Implementation (max 300 lines)
pub struct ModuleImpl {
    // Core fields
}

impl ModuleTrait for ModuleImpl {
    // Core logic
}

// Section 5: Helper Functions (max 100 lines)
mod helpers {
    // Private helpers
}

#[cfg(test)]
mod tests {
    // Unit tests (not counted in 500 line limit)
}
```

## 🏗️ Specific Module Boundaries

### 1. Neural Predictor Modules

#### `neural/fann/core.rs` (400 lines max)
```rust
// Responsibility: Core FANN predictor implementation
pub struct FannPredictor {
    model: Arc<RwLock<FannModel>>,
    config: PredictorConfig,
}

pub trait NeuralPredictorCore {
    async fn predict(&self, input: &[f64]) -> Result<PredictionResult>;
    async fn get_model_info(&self) -> ModelInfo;
}
```

#### `neural/fann/training.rs` (400 lines max)
```rust
// Responsibility: Training logic and algorithms
pub struct FannTrainer {
    algorithm: TrainingAlgorithm,
    parameters: TrainingParams,
}

pub trait NeuralTraining {
    async fn train(&mut self, data: TrainingData) -> Result<TrainingMetrics>;
    async fn validate(&self, data: ValidationData) -> Result<ValidationMetrics>;
}
```

#### `neural/fann/persistence.rs` (300 lines max)
```rust
// Responsibility: Model serialization and storage
pub struct ModelPersistence {
    storage_backend: StorageBackend,
}

pub trait ModelStorage {
    async fn save_model(&self, model: &FannModel, path: &str) -> Result<()>;
    async fn load_model(&self, path: &str) -> Result<FannModel>;
}
```

#### `neural/fann/validation.rs` (300 lines max)
```rust
// Responsibility: Input validation and preprocessing
pub struct InputValidator {
    rules: ValidationRules,
}

pub trait DataValidation {
    fn validate_input(&self, input: &[f64]) -> Result<ValidatedInput>;
    fn preprocess(&self, input: &[f64]) -> Result<ProcessedInput>;
}
```

### 2. DAA Training Modules

#### `daa/training/coordinator.rs` (400 lines max)
```rust
// Responsibility: Coordinate autonomous training workflows
pub struct TrainingCoordinator {
    agents: Vec<TrainingAgent>,
    scheduler: TrainingScheduler,
}

pub trait TrainingCoordination {
    async fn coordinate_training(&mut self) -> Result<CoordinationResult>;
    async fn distribute_work(&self, tasks: Vec<TrainingTask>) -> Result<()>;
}
```

#### `daa/training/strategies.rs` (400 lines max)
```rust
// Responsibility: Training strategy implementations
pub enum TrainingStrategy {
    Evolutionary(EvolutionaryParams),
    Reinforcement(RLParams),
    Ensemble(EnsembleParams),
}

pub trait StrategyExecutor {
    async fn execute(&self, data: TrainingData) -> Result<StrategyResult>;
}
```

#### `daa/training/metrics.rs` (300 lines max)
```rust
// Responsibility: Training metrics collection and analysis
pub struct MetricsCollector {
    buffer: MetricsBuffer,
    aggregator: MetricsAggregator,
}

pub trait MetricsCollection {
    fn collect(&mut self, metric: TrainingMetric);
    fn aggregate(&self) -> AggregatedMetrics;
}
```

### 3. Integration Coordination Modules

#### `integration/coordination/core.rs` (400 lines max)
```rust
// Responsibility: Core coordination logic
pub struct IntegrationCoordinator {
    components: ComponentRegistry,
    event_bus: EventBus,
}

pub trait CoordinationCore {
    async fn coordinate(&mut self, request: CoordinationRequest) -> Result<()>;
    async fn synchronize(&self) -> Result<SyncStatus>;
}
```

#### `integration/coordination/events.rs` (350 lines max)
```rust
// Responsibility: Event handling and routing
pub struct EventRouter {
    handlers: HandlerRegistry,
    queue: EventQueue,
}

pub trait EventHandling {
    async fn route_event(&self, event: SystemEvent) -> Result<()>;
    async fn register_handler(&mut self, handler: Box<dyn EventHandler>);
}
```

#### `integration/coordination/state.rs` (350 lines max)
```rust
// Responsibility: State management and persistence
pub struct StateManager {
    store: StateStore,
    cache: StateCache,
}

pub trait StateManagement {
    async fn get_state(&self, key: &str) -> Result<SystemState>;
    async fn update_state(&mut self, key: &str, state: SystemState) -> Result<()>;
}
```

## 🔄 Refactoring Guidelines

### Step 1: Identify Boundaries
1. List all public functions/types in the module
2. Group by responsibility
3. Define clear interfaces between groups

### Step 2: Extract Modules
```rust
// Original giant module
mod giant_module {
    // 3000+ lines of mixed concerns
}

// Refactored into focused modules
mod giant_module {
    pub mod core;      // Main logic
    pub mod handlers;  // Event/request handlers  
    pub mod storage;   // Persistence layer
    pub mod utils;     // Helper functions
    
    // Re-export public API
    pub use core::{MainStruct, MainTrait};
    pub use handlers::Handler;
}
```

### Step 3: Update Imports
```rust
// Before refactoring
use crate::giant_module::{everything, mixed, together};

// After refactoring  
use crate::giant_module::core::MainStruct;
use crate::giant_module::handlers::Handler;
```

## 📊 Module Dependency Rules

### Allowed Dependencies
```
neural/fann/core → config, error, types
neural/fann/training → core, metrics, config
neural/fann/persistence → core, storage, error
neural/fann/validation → types, error

daa/training/coordinator → strategies, metrics, config
daa/training/strategies → metrics, types
daa/training/metrics → types, storage

integration/coordination/core → events, state, config
integration/coordination/events → types, error
integration/coordination/state → storage, types
```

### Forbidden Circular Dependencies
- No module may depend on a module that depends on it
- Use traits and dependency injection to break cycles
- Event bus pattern for loose coupling

## 🎯 Success Metrics

### Per-Module Metrics
- Lines of code: <500
- Cyclomatic complexity: <10 per function
- Test coverage: >80%
- Public API items: <20

### Overall Metrics  
- Module cohesion: High (single responsibility)
- Module coupling: Low (minimal dependencies)
- Build time improvement: >30%
- Test isolation: 100%

## 🔧 Enforcement Tools

### Pre-commit Checks
```bash
#!/bin/bash
# Check module sizes
for module in $(find src -name "*.rs"); do
    lines=$(wc -l < "$module")
    if [ $lines -gt 500 ]; then
        echo "ERROR: $module has $lines lines (max 500)"
        exit 1
    fi
done
```

### CI/CD Pipeline
```yaml
module-size-check:
  script:
    - cargo install tokei
    - tokei src --files --max-lines 500
```

---

**Enforcement**: These boundaries are mandatory. Any deviation requires:
1. Documented justification
2. Queen approval
3. Refactoring plan with timeline