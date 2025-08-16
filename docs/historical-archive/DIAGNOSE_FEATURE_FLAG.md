# Diagnosing Feature Flag Inconsistency

## The Issue
You're seeing both `use_real_models=true` and `use_real_models=false` in your logs, which means multiple `FannPredictor` instances exist with different configurations.

## Diagnostic Steps

### 1. Check Running Containers
```bash
# List all running containers
docker ps -a | grep neural

# Check if multiple instances are running
docker-compose -f docker/production/docker-compose.prod.yml ps
```

### 2. Verify Environment Inside Container
```bash
# Check the actual environment variable
docker exec neural-trader env | grep NEURAL_USE_REAL_MODELS

# Check the running process
docker exec neural-trader ps aux | grep neural
```

### 3. Add Debug Logging
To understand where each instance is created, add this temporary debug code to `src/neural/fann_predictor.rs`:

```rust
impl FannPredictor {
    pub fn new(config: NeuralConfig) -> Result<Self> {
        // Add this debug line
        eprintln!("🔍 DEBUG: Creating FannPredictor with use_real_models={}, caller: {:?}", 
                  config.use_real_models, std::backtrace::Backtrace::capture());
        
        // ... rest of the constructor
    }
}
```

### 4. Check for Multiple Services
The system might be creating multiple predictors for:
- Main prediction service
- Ensemble predictions
- Test/benchmark code
- Health check endpoints
- Background training tasks

### 5. Verify Configuration Sources
```bash
# Check all .env files
find /workspaces/neural-trader -name ".env*" -exec grep -H "NEURAL_USE_REAL_MODELS" {} \;

# Check docker-compose files
grep -r "NEURAL_USE_REAL_MODELS" /workspaces/neural-trader/docker/
```

## Likely Causes

### 1. Multiple FannPredictor Instances
The system creates several instances:
- Main `NeuralPredictor` → `FannPredictor` (respects config)
- `EnhancedNeuralAdapter` → `FannPredictor` (respects config after our fix)
- Test/benchmark code → `FannPredictor` (might use default config)

### 2. Configuration Override
Some code might be explicitly setting `use_real_models=false`:
```rust
let mut config = NeuralConfig::default();
config.use_real_models = false;  // Override
```

### 3. Environment Variable Not Propagated
The environment variable might not be reaching all parts of the system.

## Solution Options

### Option 1: Force All Instances to Use Environment
Modify `NeuralConfig::default()` to always check environment:

```rust
impl Default for NeuralConfig {
    fn default() -> Self {
        let use_real_models = std::env::var("NEURAL_USE_REAL_MODELS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);  // Default to true
            
        Self {
            // ... other fields
            use_real_models,
            // ... rest
        }
    }
}
```

### Option 2: Singleton Configuration
Use a single configuration instance across the entire application.

### Option 3: Explicit Configuration Loading
Ensure all components load configuration from the same source.

## Quick Fix
If you need a quick fix, you can force all instances to use real models by modifying the `FannPredictor::new` constructor:

```rust
pub fn new(mut config: NeuralConfig) -> Result<Self> {
    // Force use_real_models from environment
    if let Ok(env_value) = std::env::var("NEURAL_USE_REAL_MODELS") {
        config.use_real_models = env_value.parse().unwrap_or(true);
    }
    // ... rest of constructor
}
```

This ensures every instance respects the environment variable regardless of the config passed in.</content>
</invoke>