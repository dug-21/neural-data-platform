//! Enhanced Neural Configuration for Phase 6
//! 
//! Provides comprehensive configuration management for the enhanced neural prediction system
//! with support for dynamic updates, validation, and environment-specific settings.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::{Result, Context};

/// Main configuration structure for enhanced neural predictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedNeuralConfig {
    /// Base neural network configuration
    pub base_neural: BaseNeuralConfig,
    /// Confidence scoring configuration
    pub confidence: ConfidenceConfig,
    /// Retraining configuration
    pub retraining: RetrainingConfig,
    /// Performance tracking configuration
    pub performance: PerformanceConfig,
    /// Ensemble management configuration
    pub ensemble: EnsembleConfig,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Security configuration
    pub security: SecurityConfig,
}

/// Base neural network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseNeuralConfig {
    /// Memory allocation in GB for neural networks
    pub memory_gb: f64,
    /// List of models to use in ensemble
    pub models: Vec<String>,
    /// Prediction cache TTL in seconds
    pub prediction_cache_ttl: u64,
    /// Model load timeout in seconds
    pub model_load_timeout: u64,
    /// Maximum concurrent predictions
    pub max_concurrent_predictions: usize,
    /// Enable model monitoring
    pub enable_model_monitoring: bool,
    /// Accuracy threshold for model performance
    pub accuracy_threshold: f64,
    /// Model-specific configurations
    pub model_configs: HashMap<String, ModelSpecificConfig>,
}

/// Model-specific configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpecificConfig {
    /// Learning rate for the model
    pub learning_rate: f64,
    /// Training epochs
    pub max_epochs: usize,
    /// Hidden layer sizes
    pub hidden_layers: Vec<usize>,
    /// Input window size
    pub input_window: usize,
    /// Output horizon
    pub output_horizon: usize,
    /// Use cascade training
    pub use_cascade: bool,
    /// Model weight in ensemble
    pub ensemble_weight: f64,
}

/// Confidence scoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceConfig {
    /// Weight for ensemble agreement in confidence calculation
    pub ensemble_agreement_weight: f64,
    /// Weight for historical accuracy in confidence calculation
    pub historical_accuracy_weight: f64,
    /// Weight for market regime adjustment
    pub market_regime_weight: f64,
    /// Weight for data quality factor
    pub data_quality_weight: f64,
    /// Weight for volatility penalty
    pub volatility_penalty_weight: f64,
    /// Maximum confidence boost from ensemble agreement
    pub max_ensemble_boost: f64,
    /// Maximum confidence penalty from volatility
    pub max_volatility_penalty: f64,
    /// Confidence decay factor for temporal distance
    pub temporal_decay_factor: f64,
    /// Minimum confidence threshold for predictions
    pub min_confidence_threshold: f64,
    /// Market regime confidence adjustments
    pub regime_adjustments: HashMap<String, f64>,
}

/// Retraining configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrainingConfig {
    /// Enable autonomous retraining
    pub enable_autonomous_retraining: bool,
    /// Accuracy threshold below which retraining is triggered
    pub accuracy_threshold: f64,
    /// Hours threshold for time-based retraining
    pub hours_threshold: i64,
    /// Sample count threshold for data-based retraining
    pub sample_threshold: usize,
    /// Maximum retraining frequency per day
    pub max_retrains_per_day: usize,
    /// Urgency score multiplier for retraining prioritization
    pub urgency_multiplier: f64,
    /// Cool-down period between retraining attempts (hours)
    pub retraining_cooldown_hours: i64,
    /// Training data retention period (days)
    pub training_data_retention_days: i64,
    /// Parallel training configuration
    pub parallel_training: ParallelTrainingConfig,
}

/// Parallel training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelTrainingConfig {
    /// Maximum number of models to train in parallel
    pub max_parallel_models: usize,
    /// Training thread pool size
    pub thread_pool_size: usize,
    /// GPU acceleration settings
    pub gpu_acceleration: GpuConfig,
}

/// GPU acceleration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    /// Enable GPU acceleration
    pub enabled: bool,
    /// GPU device IDs to use
    pub device_ids: Vec<usize>,
    /// Memory allocation per GPU (GB)
    pub memory_per_gpu_gb: f64,
}

/// Performance tracking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum prediction history size
    pub max_history_size: usize,
    /// Performance decay factor for exponential averaging
    pub decay_factor: f64,
    /// Metrics collection interval (seconds)
    pub metrics_interval_seconds: u64,
    /// Enable detailed performance logging
    pub enable_detailed_logging: bool,
    /// Performance alert thresholds
    pub alert_thresholds: AlertThresholds,
}

/// Performance alert thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Accuracy drop threshold for alerts
    pub accuracy_drop_threshold: f64,
    /// Prediction latency threshold (milliseconds)
    pub latency_threshold_ms: u64,
    /// Memory usage threshold (percentage)
    pub memory_usage_threshold_percent: f64,
    /// Error rate threshold (percentage)
    pub error_rate_threshold_percent: f64,
}

/// Ensemble management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleConfig {
    /// Dynamic weight adjustment frequency
    pub weight_update_frequency: usize,
    /// Performance threshold for model inclusion
    pub performance_threshold: f64,
    /// Diversity bonus factor
    pub diversity_bonus_factor: f64,
    /// Regime-based weight adjustments
    pub regime_weight_adjustments: HashMap<String, f64>,
    /// Volatility adaptation settings
    pub volatility_adaptation: VolatilityAdaptationConfig,
}

/// Volatility adaptation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityAdaptationConfig {
    /// Enable volatility-based model selection
    pub enabled: bool,
    /// High volatility threshold
    pub high_volatility_threshold: f64,
    /// Low volatility threshold
    pub low_volatility_threshold: f64,
    /// Model preferences for different volatility regimes
    pub volatility_model_preferences: HashMap<String, Vec<String>>,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Prediction cache size (number of entries)
    pub prediction_cache_size: usize,
    /// Model state cache size
    pub model_state_cache_size: usize,
    /// Performance metrics cache TTL (seconds)
    pub metrics_cache_ttl: u64,
    /// Market regime cache TTL (seconds)
    pub regime_cache_ttl: u64,
    /// Cache cleanup interval (seconds)
    pub cleanup_interval_seconds: u64,
    /// Enable cache compression
    pub enable_compression: bool,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable model integrity verification
    pub enable_model_verification: bool,
    /// Model checksum validation
    pub validate_model_checksums: bool,
    /// Secure model storage settings
    pub secure_storage: SecureStorageConfig,
    /// Access control settings
    pub access_control: AccessControlConfig,
}

/// Secure storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureStorageConfig {
    /// Enable encryption at rest
    pub encrypt_at_rest: bool,
    /// Encryption algorithm
    pub encryption_algorithm: String,
    /// Key rotation interval (days)
    pub key_rotation_days: u64,
    /// Backup configuration
    pub backup_config: BackupConfig,
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Enable automated backups
    pub enabled: bool,
    /// Backup interval (hours)
    pub interval_hours: u64,
    /// Maximum backup retention (days)
    pub retention_days: u64,
    /// Backup storage location
    pub storage_location: String,
}

/// Access control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// Require authentication for model updates
    pub require_auth_for_updates: bool,
    /// Admin roles allowed to modify configuration
    pub admin_roles: Vec<String>,
    /// Audit logging configuration
    pub audit_logging: AuditLoggingConfig,
}

/// Audit logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLoggingConfig {
    /// Enable audit logging
    pub enabled: bool,
    /// Log level for audit events
    pub log_level: String,
    /// Audit log rotation settings
    pub rotation_settings: LogRotationConfig,
}

/// Log rotation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRotationConfig {
    /// Maximum log file size (MB)
    pub max_file_size_mb: u64,
    /// Maximum number of log files to keep
    pub max_files: usize,
    /// Compression for old log files
    pub compress_old_files: bool,
}

impl Default for EnhancedNeuralConfig {
    fn default() -> Self {
        Self {
            base_neural: BaseNeuralConfig::default(),
            confidence: ConfidenceConfig::default(),
            retraining: RetrainingConfig::default(),
            performance: PerformanceConfig::default(),
            ensemble: EnsembleConfig::default(),
            cache: CacheConfig::default(),
            security: SecurityConfig::default(),
        }
    }
}

impl Default for BaseNeuralConfig {
    fn default() -> Self {
        let mut model_configs = HashMap::new();
        
        // DeepAR configuration
        model_configs.insert("DeepAR".to_string(), ModelSpecificConfig {
            learning_rate: 0.0003,
            max_epochs: 2500,
            hidden_layers: vec![100, 50, 25],
            input_window: 60,
            output_horizon: 8,
            use_cascade: true,
            ensemble_weight: 1.5,
        });
        
        // LSTM configuration
        model_configs.insert("LSTM".to_string(), ModelSpecificConfig {
            learning_rate: 0.0002,
            max_epochs: 2000,
            hidden_layers: vec![128, 64, 64, 32],
            input_window: 100,
            output_horizon: 10,
            use_cascade: true,
            ensemble_weight: 1.4,
        });
        
        // Transformer configuration
        model_configs.insert("Transformer".to_string(), ModelSpecificConfig {
            learning_rate: 0.0001,
            max_epochs: 3000,
            hidden_layers: vec![256, 128, 64, 32],
            input_window: 80,
            output_horizon: 12,
            use_cascade: true,
            ensemble_weight: 1.3,
        });
        
        Self {
            memory_gb: 2.0,
            models: vec![
                "DeepAR".to_string(),
                "LSTM".to_string(),
                "Transformer".to_string(),
                "GRU".to_string(),
                "NHITS".to_string(),
                "TCN".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 300,
            max_concurrent_predictions: 50,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            model_configs,
        }
    }
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        let mut regime_adjustments = HashMap::new();
        regime_adjustments.insert("bullish".to_string(), 0.05);
        regime_adjustments.insert("bearish".to_string(), 0.02);
        regime_adjustments.insert("sideways".to_string(), 0.08);
        regime_adjustments.insert("high_volatility".to_string(), -0.05);
        regime_adjustments.insert("low_volatility".to_string(), 0.05);
        
        Self {
            ensemble_agreement_weight: 0.6,
            historical_accuracy_weight: 0.4,
            market_regime_weight: 0.2,
            data_quality_weight: 0.4,
            volatility_penalty_weight: 3.0,
            max_ensemble_boost: 0.3,
            max_volatility_penalty: 0.15,
            temporal_decay_factor: 0.02,
            min_confidence_threshold: 0.6,
            regime_adjustments,
        }
    }
}

impl Default for RetrainingConfig {
    fn default() -> Self {
        Self {
            enable_autonomous_retraining: true,
            accuracy_threshold: 0.7,
            hours_threshold: 24,
            sample_threshold: 10000,
            max_retrains_per_day: 3,
            urgency_multiplier: 1.0,
            retraining_cooldown_hours: 4,
            training_data_retention_days: 30,
            parallel_training: ParallelTrainingConfig::default(),
        }
    }
}

impl Default for ParallelTrainingConfig {
    fn default() -> Self {
        Self {
            max_parallel_models: 3,
            thread_pool_size: 4,
            gpu_acceleration: GpuConfig::default(),
        }
    }
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for compatibility
            device_ids: vec![0],
            memory_per_gpu_gb: 4.0,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_history_size: 1000,
            decay_factor: 0.95,
            metrics_interval_seconds: 60,
            enable_detailed_logging: false,
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            accuracy_drop_threshold: 0.1, // 10% accuracy drop
            latency_threshold_ms: 5000,    // 5 second latency
            memory_usage_threshold_percent: 85.0, // 85% memory usage
            error_rate_threshold_percent: 5.0,    // 5% error rate
        }
    }
}

impl Default for EnsembleConfig {
    fn default() -> Self {
        let mut regime_weight_adjustments = HashMap::new();
        regime_weight_adjustments.insert("high_volatility".to_string(), 1.2);
        regime_weight_adjustments.insert("low_volatility".to_string(), 1.1);
        regime_weight_adjustments.insert("bullish".to_string(), 1.05);
        regime_weight_adjustments.insert("bearish".to_string(), 1.03);
        
        Self {
            weight_update_frequency: 10,
            performance_threshold: 0.6,
            diversity_bonus_factor: 0.2,
            regime_weight_adjustments,
            volatility_adaptation: VolatilityAdaptationConfig::default(),
        }
    }
}

impl Default for VolatilityAdaptationConfig {
    fn default() -> Self {
        let mut volatility_model_preferences = HashMap::new();
        volatility_model_preferences.insert(
            "high".to_string(),
            vec!["DeepAR".to_string(), "LSTM".to_string()]
        );
        volatility_model_preferences.insert(
            "low".to_string(),
            vec!["TCN".to_string(), "Transformer".to_string()]
        );
        volatility_model_preferences.insert(
            "medium".to_string(),
            vec!["GRU".to_string(), "NHITS".to_string()]
        );
        
        Self {
            enabled: true,
            high_volatility_threshold: 0.03,
            low_volatility_threshold: 0.01,
            volatility_model_preferences,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            prediction_cache_size: 10000,
            model_state_cache_size: 1000,
            metrics_cache_ttl: 300,
            regime_cache_ttl: 1800, // 30 minutes
            cleanup_interval_seconds: 3600, // 1 hour
            enable_compression: true,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_model_verification: true,
            validate_model_checksums: true,
            secure_storage: SecureStorageConfig::default(),
            access_control: AccessControlConfig::default(),
        }
    }
}

impl Default for SecureStorageConfig {
    fn default() -> Self {
        Self {
            encrypt_at_rest: false, // Disabled by default for development
            encryption_algorithm: "AES-256-GCM".to_string(),
            key_rotation_days: 90,
            backup_config: BackupConfig::default(),
        }
    }
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 24,
            retention_days: 7,
            storage_location: "./backups/neural_models".to_string(),
        }
    }
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            require_auth_for_updates: false, // Disabled by default for development
            admin_roles: vec!["admin".to_string(), "ml_engineer".to_string()],
            audit_logging: AuditLoggingConfig::default(),
        }
    }
}

impl Default for AuditLoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: "INFO".to_string(),
            rotation_settings: LogRotationConfig::default(),
        }
    }
}

impl Default for LogRotationConfig {
    fn default() -> Self {
        Self {
            max_file_size_mb: 100,
            max_files: 10,
            compress_old_files: true,
        }
    }
}

impl EnhancedNeuralConfig {
    /// Load configuration from TOML file
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;
        
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path))?;
        
        config.validate()?;
        Ok(config)
    }
    
    /// Save configuration to TOML file
    pub fn to_file(&self, path: &str) -> Result<()> {
        self.validate()?;
        
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize configuration")?;
        
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path))?;
        
        Ok(())
    }
    
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        // Validate base neural config
        if self.base_neural.memory_gb <= 0.0 {
            return Err(anyhow::anyhow!("Neural memory allocation must be positive"));
        }
        
        if self.base_neural.models.is_empty() {
            return Err(anyhow::anyhow!("At least one model must be specified"));
        }
        
        if self.base_neural.accuracy_threshold < 0.0 || self.base_neural.accuracy_threshold > 1.0 {
            return Err(anyhow::anyhow!("Accuracy threshold must be between 0.0 and 1.0"));
        }
        
        // Validate confidence config
        if self.confidence.min_confidence_threshold < 0.0 || self.confidence.min_confidence_threshold > 1.0 {
            return Err(anyhow::anyhow!("Minimum confidence threshold must be between 0.0 and 1.0"));
        }
        
        // Validate retraining config
        if self.retraining.accuracy_threshold < 0.0 || self.retraining.accuracy_threshold > 1.0 {
            return Err(anyhow::anyhow!("Retraining accuracy threshold must be between 0.0 and 1.0"));
        }
        
        if self.retraining.hours_threshold <= 0 {
            return Err(anyhow::anyhow!("Retraining hours threshold must be positive"));
        }
        
        if self.retraining.sample_threshold == 0 {
            return Err(anyhow::anyhow!("Retraining sample threshold must be positive"));
        }
        
        // Validate performance config
        if self.performance.decay_factor <= 0.0 || self.performance.decay_factor > 1.0 {
            return Err(anyhow::anyhow!("Performance decay factor must be between 0.0 and 1.0"));
        }
        
        Ok(())
    }
    
    /// Create a development configuration with relaxed settings
    pub fn development() -> Self {
        let mut config = Self::default();
        
        // Reduce resource requirements for development
        config.base_neural.memory_gb = 1.0;
        config.base_neural.models = vec!["MLP".to_string(), "NHITS".to_string()];
        config.base_neural.max_concurrent_predictions = 10;
        config.retraining.sample_threshold = 1000;
        config.cache.prediction_cache_size = 1000;
        config.security.secure_storage.encrypt_at_rest = false;
        config.security.access_control.require_auth_for_updates = false;
        
        config
    }
    
    /// Create a production configuration with optimized settings
    pub fn production() -> Self {
        let mut config = Self::default();
        
        // Optimize for production workload
        config.base_neural.memory_gb = 8.0;
        config.base_neural.max_concurrent_predictions = 100;
        config.retraining.sample_threshold = 50000;
        config.cache.prediction_cache_size = 50000;
        config.security.secure_storage.encrypt_at_rest = true;
        config.security.access_control.require_auth_for_updates = true;
        config.performance.enable_detailed_logging = true;
        
        // Enable GPU acceleration in production if available
        config.retraining.parallel_training.gpu_acceleration.enabled = true;
        config.retraining.parallel_training.max_parallel_models = 6;
        
        config
    }
    
    /// Get model-specific configuration
    pub fn get_model_config(&self, model_name: &str) -> Option<&ModelSpecificConfig> {
        self.base_neural.model_configs.get(model_name)
    }
    
    /// Update model-specific configuration
    pub fn update_model_config(&mut self, model_name: String, config: ModelSpecificConfig) {
        self.base_neural.model_configs.insert(model_name, config);
    }
    
    /// Get ensemble weight for a model
    pub fn get_ensemble_weight(&self, model_name: &str) -> f64 {
        self.base_neural.model_configs
            .get(model_name)
            .map(|config| config.ensemble_weight)
            .unwrap_or(1.0)
    }
    
    /// Check if autonomous retraining is enabled
    pub fn is_autonomous_retraining_enabled(&self) -> bool {
        self.retraining.enable_autonomous_retraining
    }
    
    /// Get volatility model preferences for current regime
    pub fn get_volatility_model_preferences(&self, volatility_level: &str) -> Vec<String> {
        self.ensemble.volatility_adaptation
            .volatility_model_preferences
            .get(volatility_level)
            .cloned()
            .unwrap_or_else(|| self.base_neural.models.clone())
    }
}

/// Configuration builder for dynamic configuration creation
pub struct ConfigBuilder {
    config: EnhancedNeuralConfig,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: EnhancedNeuralConfig::default(),
        }
    }
    
    pub fn models(mut self, models: Vec<String>) -> Self {
        self.config.base_neural.models = models;
        self
    }
    
    pub fn memory_gb(mut self, memory_gb: f64) -> Self {
        self.config.base_neural.memory_gb = memory_gb;
        self
    }
    
    pub fn accuracy_threshold(mut self, threshold: f64) -> Self {
        self.config.base_neural.accuracy_threshold = threshold;
        self.config.retraining.accuracy_threshold = threshold - 0.05; // Set retraining threshold slightly lower
        self
    }
    
    pub fn enable_gpu(mut self, enabled: bool) -> Self {
        self.config.retraining.parallel_training.gpu_acceleration.enabled = enabled;
        self
    }
    
    pub fn cache_size(mut self, size: usize) -> Self {
        self.config.cache.prediction_cache_size = size;
        self
    }
    
    pub fn build(self) -> Result<EnhancedNeuralConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_default_config_validation() {
        let config = EnhancedNeuralConfig::default();
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_development_config() {
        let config = EnhancedNeuralConfig::development();
        assert!(config.validate().is_ok());
        assert_eq!(config.base_neural.memory_gb, 1.0);
        assert_eq!(config.base_neural.models.len(), 2);
    }
    
    #[test]
    fn test_production_config() {
        let config = EnhancedNeuralConfig::production();
        assert!(config.validate().is_ok());
        assert_eq!(config.base_neural.memory_gb, 8.0);
        assert!(config.security.secure_storage.encrypt_at_rest);
    }
    
    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .models(vec!["LSTM".to_string(), "DeepAR".to_string()])
            .memory_gb(4.0)
            .accuracy_threshold(0.8)
            .enable_gpu(true)
            .build()
            .unwrap();
        
        assert_eq!(config.base_neural.models.len(), 2);
        assert_eq!(config.base_neural.memory_gb, 4.0);
        assert_eq!(config.base_neural.accuracy_threshold, 0.8);
        assert!(config.retraining.parallel_training.gpu_acceleration.enabled);
    }
    
    #[test]
    fn test_config_file_serialization() {
        let config = EnhancedNeuralConfig::development();
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap();
        
        // Save to file
        config.to_file(file_path).unwrap();
        
        // Load from file
        let loaded_config = EnhancedNeuralConfig::from_file(file_path).unwrap();
        
        // Compare key values
        assert_eq!(config.base_neural.memory_gb, loaded_config.base_neural.memory_gb);
        assert_eq!(config.base_neural.models, loaded_config.base_neural.models);
    }
    
    #[test]
    fn test_validation_errors() {
        let mut config = EnhancedNeuralConfig::default();
        
        // Test invalid memory allocation
        config.base_neural.memory_gb = -1.0;
        assert!(config.validate().is_err());
        
        // Reset and test empty models
        config = EnhancedNeuralConfig::default();
        config.base_neural.models.clear();
        assert!(config.validate().is_err());
        
        // Reset and test invalid accuracy threshold
        config = EnhancedNeuralConfig::default();
        config.base_neural.accuracy_threshold = 1.5;
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_model_config_operations() {
        let mut config = EnhancedNeuralConfig::default();
        
        // Test getting existing model config
        let lstm_config = config.get_model_config("LSTM").unwrap();
        assert_eq!(lstm_config.ensemble_weight, 1.4);
        
        // Test updating model config
        let new_config = ModelSpecificConfig {
            learning_rate: 0.001,
            max_epochs: 1000,
            hidden_layers: vec![64, 32],
            input_window: 50,
            output_horizon: 5,
            use_cascade: false,
            ensemble_weight: 2.0,
        };
        
        config.update_model_config("TestModel".to_string(), new_config);
        assert_eq!(config.get_ensemble_weight("TestModel"), 2.0);
    }
}