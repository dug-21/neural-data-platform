# Configurable Model Activation Strategy

## Problem with Previous Approach

The hardcoded model categories (PRICE_ONLY_MODELS, VOLUME_ENHANCED_MODELS, etc.) make dangerous assumptions:
- Why can't TFT work with just price data?
- Maybe LSTM performs better with sentiment than without?
- What if we want to run DeepAR on price-only first, then enhance with sentiment?

## Solution: Fully Configurable Data Requirements

### 1. Configuration-Driven Model Definitions

```toml
# config/models.toml - Completely configurable
[models.lstm_basic]
architecture = "LSTM"
input_size = 24
hidden_size = 64
num_layers = 2

# Flexible data requirements
[models.lstm_basic.data_requirements]
required = ["price"]           # Must have price
optional = ["volume"]          # Will use volume if available
preferred = ["price", "volume"] # Best performance with both
min_history = 100
fallback_mode = "price_only"   # Can run with just price

[models.tft_minimal]
architecture = "TFT"
d_model = 128
num_heads = 8

[models.tft_minimal.data_requirements]
required = ["price"]           # TFT CAN work with just price!
optional = ["volume", "economic", "sentiment"]
preferred = ["price", "volume", "economic"]
min_history = 200
fallback_mode = "basic_attention"

[models.deepar_flexible]
architecture = "DeepAR"
hidden_size = 100

[models.deepar_flexible.data_requirements]
required = ["price"]
optional = ["sentiment", "volume", "economic"]
preferred = ["price", "sentiment"]
min_history = 150
adaptive_complexity = true     # Adjust model complexity based on available data

# Multi-configuration per architecture
[models.tft_full]
architecture = "TFT"
d_model = 256
num_heads = 16

[models.tft_full.data_requirements]
required = ["price", "volume", "economic"]  # This one needs more data
optional = ["sentiment", "orderbook"]
preferred = ["price", "volume", "economic", "sentiment"]
min_history = 500
```

### 2. Dynamic Model Configuration System

```rust
// src/neural/model_registry.rs
pub struct ModelRegistry {
    /// All configured models from config files
    model_configs: HashMap<String, ModelConfiguration>,
    /// Currently active models per symbol
    active_models: DashMap<String, HashMap<String, Box<dyn BaseModel<f32>>>>,
    /// Models waiting for data
    pending_models: DashMap<String, HashMap<String, ModelConfiguration>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ModelConfiguration {
    pub id: String,
    pub architecture: String,
    pub parameters: HashMap<String, Value>,
    pub data_requirements: DataRequirements,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DataRequirements {
    /// Must have these data types to activate
    pub required: Vec<DataType>,
    /// Will enhance performance if available
    pub optional: Vec<DataType>,
    /// Optimal data combination for best performance
    pub preferred: Vec<DataType>,
    /// Minimum historical data points needed
    pub min_history: usize,
    /// How to handle missing preferred data
    pub fallback_mode: Option<String>,
    /// Adjust model complexity based on available data
    pub adaptive_complexity: bool,
}

impl ModelRegistry {
    /// Load all model configurations from config files
    pub fn load_from_config(config_path: &Path) -> Result<Self> {
        let config_content = std::fs::read_to_string(config_path)?;
        let config: ModelConfigFile = toml::from_str(&config_content)?;
        
        let model_configs = config.models.into_iter()
            .map(|(id, mut config)| {
                config.id = id.clone();
                (id, config)
            })
            .collect();
        
        Ok(Self {
            model_configs,
            active_models: DashMap::new(),
            pending_models: DashMap::new(),
        })
    }
    
    /// Evaluate which models can be activated for given data availability
    pub fn evaluate_model_activation(
        &self,
        symbol: &str,
        available_data: &DataProfile
    ) -> Result<ModelActivationPlan> {
        let mut can_activate = Vec::new();
        let mut should_wait = Vec::new();
        let mut can_fallback = Vec::new();
        
        for (model_id, config) in &self.model_configs {
            match self.can_activate_model(config, available_data) {
                ActivationStatus::Ready => {
                    can_activate.push((model_id.clone(), config.clone()));
                },
                ActivationStatus::MissingRequired(missing) => {
                    should_wait.push((model_id.clone(), config.clone(), missing));
                },
                ActivationStatus::CanFallback(fallback_config) => {
                    can_fallback.push((model_id.clone(), fallback_config));
                },
            }
        }
        
        Ok(ModelActivationPlan {
            can_activate,
            should_wait,
            can_fallback,
        })
    }
    
    /// Check if a model can be activated with available data
    fn can_activate_model(
        &self,
        config: &ModelConfiguration,
        available_data: &DataProfile
    ) -> ActivationStatus {
        let req = &config.data_requirements;
        
        // Check required data
        let missing_required: Vec<_> = req.required.iter()
            .filter(|&data_type| !available_data.has_sufficient_data(data_type, req.min_history))
            .cloned()
            .collect();
        
        if !missing_required.is_empty() {
            return ActivationStatus::MissingRequired(missing_required);
        }
        
        // Check if we have preferred data
        let has_preferred = req.preferred.iter()
            .all(|data_type| available_data.has_data(data_type));
        
        if has_preferred {
            return ActivationStatus::Ready;
        }
        
        // Check if fallback is possible
        if req.fallback_mode.is_some() || req.adaptive_complexity {
            let fallback_config = self.create_fallback_config(config, available_data);
            return ActivationStatus::CanFallback(fallback_config);
        }
        
        // Can run with just required data
        ActivationStatus::Ready
    }
    
    /// Create a fallback configuration for reduced data
    fn create_fallback_config(
        &self,
        base_config: &ModelConfiguration,
        available_data: &DataProfile
    ) -> ModelConfiguration {
        let mut fallback = base_config.clone();
        
        if base_config.data_requirements.adaptive_complexity {
            // Reduce model complexity based on available data
            let data_richness = available_data.richness_score();
            
            if let Some(hidden_size) = fallback.parameters.get_mut("hidden_size") {
                if let Some(size) = hidden_size.as_u64() {
                    let adjusted_size = (size as f64 * data_richness).max(32.0) as u64;
                    *hidden_size = Value::Number(adjusted_size.into());
                }
            }
            
            if let Some(num_layers) = fallback.parameters.get_mut("num_layers") {
                if let Some(layers) = num_layers.as_u64() {
                    let adjusted_layers = (layers as f64 * data_richness).max(1.0) as u64;
                    *num_layers = Value::Number(adjusted_layers.into());
                }
            }
        }
        
        // Apply specific fallback mode
        if let Some(ref fallback_mode) = base_config.data_requirements.fallback_mode {
            match fallback_mode.as_str() {
                "price_only" => {
                    // Configure model for price-only operation
                    fallback.parameters.insert("use_attention".to_string(), Value::Bool(false));
                },
                "basic_attention" => {
                    // Reduce attention complexity
                    if let Some(num_heads) = fallback.parameters.get_mut("num_heads") {
                        *num_heads = Value::Number(4.into());
                    }
                },
                _ => {}
            }
        }
        
        fallback
    }
}

#[derive(Debug)]
pub enum ActivationStatus {
    Ready,
    MissingRequired(Vec<DataType>),
    CanFallback(ModelConfiguration),
}

#[derive(Debug)]
pub struct ModelActivationPlan {
    pub can_activate: Vec<(String, ModelConfiguration)>,
    pub should_wait: Vec<(String, ModelConfiguration, Vec<DataType>)>,
    pub can_fallback: Vec<(String, ModelConfiguration)>,
}
```

### 3. Flexible Data Profile System

```rust
// src/neural/data_profile.rs
pub struct DataProfile {
    /// Available data types with quality scores
    available_data: HashMap<DataType, DataAvailability>,
    /// Symbol-specific metadata
    symbol: String,
    /// Last updated timestamp
    last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DataAvailability {
    pub available: bool,
    pub quality_score: f64,      // 0.0 to 1.0
    pub history_depth: usize,    // Number of data points
    pub update_frequency: Duration, // How often it updates
    pub latency: Duration,       // Data delay
}

impl DataProfile {
    pub fn has_sufficient_data(&self, data_type: &DataType, min_history: usize) -> bool {
        if let Some(availability) = self.available_data.get(data_type) {
            availability.available 
                && availability.history_depth >= min_history
                && availability.quality_score >= 0.7  // Configurable threshold
        } else {
            false
        }
    }
    
    pub fn richness_score(&self) -> f64 {
        let total_possible = 6.0; // Total data modalities
        let available_count = self.available_data.values()
            .filter(|av| av.available && av.quality_score >= 0.5)
            .count() as f64;
        let quality_sum: f64 = self.available_data.values()
            .filter(|av| av.available)
            .map(|av| av.quality_score)
            .sum();
        
        (available_count / total_possible) * (quality_sum / available_count.max(1.0))
    }
}
```

### 4. User-Controlled Model Selection

```toml
# config/model_selection.toml - User can override defaults
[selection_policy]
# Strategy for model activation
activation_strategy = "aggressive"  # "conservative", "aggressive", "adaptive"

# Model priorities (higher = more important)
[selection_policy.model_priorities]
"lstm_basic" = 1.0
"tft_minimal" = 0.8
"deepar_flexible" = 0.9
"tcn_price" = 0.7

# Data type importance weights
[selection_policy.data_weights]
price = 1.0
volume = 0.8
sentiment = 0.6
economic = 0.7
orderbook = 0.5

# Per-symbol overrides
[symbol_overrides."AAPL"]
preferred_models = ["tft_full", "lstm_basic"]
min_models_active = 3
data_requirements_relaxed = true

[symbol_overrides."BTC-USD"]
preferred_models = ["deepar_flexible", "tcn_price"]
# Crypto might not have economic data
ignore_data_types = ["economic"]
```

## Benefits of Configurable Approach

### 1. **No Hardcoded Assumptions**
- Every model's data requirements are explicit in config
- Easy to experiment with different combinations
- No code changes needed to try new model/data combinations

### 2. **User Control**
- Override model selection per symbol
- Adjust data requirements and fallback behavior
- Control activation aggressiveness

### 3. **Adaptive Complexity**
- Models automatically adjust complexity based on available data
- TFT can run simple with price-only, complex with full data
- Performance scales with data richness

### 4. **Future Proof**
- Add new models by editing config files
- No code changes for new data types
- Easy A/B testing of different configurations

### 5. **Transparent Operation**
- Clear logs showing why models activate/deactivate
- Performance tracking per data configuration
- Easy debugging of data requirements

This approach eliminates all hardcoded assumptions and makes the system fully configurable while maintaining the ability to handle evolving data availability gracefully.