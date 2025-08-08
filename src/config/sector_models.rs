//! Sector Models Configuration
//!
//! This module provides configuration loading and management for the sector-based
//! neural architecture, integrating with the existing VendorPredictor and DAA systems.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Individual sector model definition for factory creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorModelDefinition {
    pub model_type: String,
    pub sector: String,
    pub parameters: Option<HashMap<String, Value>>,
}
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Main sector models configuration - loaded from config/sector_models.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorModelsConfig {
    pub metadata: Metadata,
    pub sectors: HashMap<String, SectorConfig>,
    pub models: HashMap<String, ModelConfig>, 
    pub data_requirements: DataRequirements,
    pub performance: PerformanceConfig,
    pub daa_coordination: DAACoordinationConfig,
    pub integration: IntegrationConfig,
    pub lazy_loading: LazyLoadingConfig,
    pub testing: TestingConfig,
}

impl Default for SectorModelsConfig {
    fn default() -> Self {
        Self {
            metadata: Metadata::default(),
            sectors: HashMap::new(),
            models: HashMap::new(),
            data_requirements: DataRequirements::default(),
            performance: PerformanceConfig::default(),
            daa_coordination: DAACoordinationConfig::default(),
            integration: IntegrationConfig::default(),
            lazy_loading: LazyLoadingConfig::default(),
            testing: TestingConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub version: String,
    pub created_by: String,
    pub description: String,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            version: "2.0.0".to_string(),
            created_by: "daa_hierarchy_developer".to_string(),
            description: "Sector-based neural architecture configuration".to_string(),
        }
    }
}

/// Configuration for a specific sector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorConfig {
    pub etf_representative: String,
    pub sector_name: String,
    pub description: String,
    pub symbols: Vec<String>,
    pub shared_memory_mb: u32,
    pub specialization_memory_mb: u32,
    pub max_symbols: u32,
    pub correlation_threshold: f64,
    pub sector_weight: f64,
}

/// Configuration for a specific model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_type: String,
    pub sector: String,
    pub description: String,
    pub required_data: Vec<String>,
    pub optional_data: Vec<String>,
    pub preferred_data: Vec<String>,
    pub max_memory_mb: u32,
    pub min_accuracy: f64,
    pub max_latency_ms: u32,
    pub ensemble_weight: f64,
    pub lazy_load_conditions: Vec<String>,
    pub specialization_layers: u32,
}

/// Data requirements configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRequirements {
    pub description: String,
    pub required: HashMap<String, DataSpec>,
    pub optional: HashMap<String, DataSpec>,
    pub preferred: HashMap<String, DataSpec>,
}

impl Default for DataRequirements {
    fn default() -> Self {
        Self {
            description: "Global data requirements and availability thresholds".to_string(),
            required: HashMap::new(),
            optional: HashMap::new(),
            preferred: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSpec {
    pub min_history_days: u32,
    pub update_frequency: String,
    pub quality_threshold: f64,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub description: String,
    pub memory_optimization: MemoryOptimization,
    pub prediction_latency: PredictionLatency,
    pub accuracy_thresholds: AccuracyThresholds,
    pub resource_limits: ResourceLimits,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            description: "Performance thresholds and optimization settings".to_string(),
            memory_optimization: MemoryOptimization::default(),
            prediction_latency: PredictionLatency::default(),
            accuracy_thresholds: AccuracyThresholds::default(),
            resource_limits: ResourceLimits::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOptimization {
    pub enable_shared_features: bool,
    pub feature_cache_ttl_seconds: u32,
    pub max_cache_size_mb: u32,
    pub enable_lazy_loading: bool,
    pub unload_inactive_models_minutes: u32,
    pub memory_pressure_threshold: f64,
}

impl Default for MemoryOptimization {
    fn default() -> Self {
        Self {
            enable_shared_features: true,
            feature_cache_ttl_seconds: 300,
            max_cache_size_mb: 1024,
            enable_lazy_loading: true,
            unload_inactive_models_minutes: 15,
            memory_pressure_threshold: 0.85,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionLatency {
    pub max_prediction_latency_ms: u32,
    pub max_ensemble_latency_ms: u32,
    pub timeout_strategy: String,
    pub enable_parallel_inference: bool,
    pub max_concurrent_predictions: u32,
}

impl Default for PredictionLatency {
    fn default() -> Self {
        Self {
            max_prediction_latency_ms: 100,
            max_ensemble_latency_ms: 150,
            timeout_strategy: "fallback_to_fastest".to_string(),
            enable_parallel_inference: true,
            max_concurrent_predictions: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyThresholds {
    pub min_sector_accuracy: f64,
    pub min_symbol_accuracy: f64,
    pub ensemble_confidence_threshold: f64,
    pub consensus_threshold: f64,
    pub performance_degradation_threshold: f64,
}

impl Default for AccuracyThresholds {
    fn default() -> Self {
        Self {
            min_sector_accuracy: 0.70,
            min_symbol_accuracy: 0.65,
            ensemble_confidence_threshold: 0.75,
            consensus_threshold: 0.70,
            performance_degradation_threshold: 0.05,
        }
    }
}

pub type PerformanceThresholds = AccuracyThresholds; // Alias for backward compatibility

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_total_memory_gb: f64,
    pub max_cpu_percent: f64,
    pub max_gpu_percent: f64,
    pub disk_cache_limit_gb: f64,
    pub network_bandwidth_limit_mbps: f64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_total_memory_gb: 4.0,
            max_cpu_percent: 80.0,
            max_gpu_percent: 90.0,
            disk_cache_limit_gb: 10.0,
            network_bandwidth_limit_mbps: 100.0,
        }
    }
}

/// DAA coordination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAACoordinationConfig {
    pub description: String,
    pub master_coordinator: MasterCoordinator,
    pub sector_coordinators: SectorCoordinators,
    pub voting_mechanisms: VotingMechanisms,
}

impl Default for DAACoordinationConfig {
    fn default() -> Self {
        Self {
            description: "Hierarchical DAA coordination settings".to_string(),
            master_coordinator: MasterCoordinator::default(),
            sector_coordinators: SectorCoordinators::default(),
            voting_mechanisms: VotingMechanisms::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterCoordinator {
    pub portfolio_consensus_threshold: f64,
    pub cross_sector_risk_weight: f64,
    pub meta_predictor_weight: f64,
    pub max_portfolio_positions: u32,
    pub rebalancing_frequency: String,
}

impl Default for MasterCoordinator {
    fn default() -> Self {
        Self {
            portfolio_consensus_threshold: 0.70,
            cross_sector_risk_weight: 0.30,
            meta_predictor_weight: 0.20,
            max_portfolio_positions: 20,
            rebalancing_frequency: "hourly".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorCoordinators {
    pub sector_consensus_threshold: f64,
    pub intra_sector_correlation_limit: f64,
    pub max_sector_positions: u32,
    pub position_sizing_method: String,
    pub risk_per_sector: f64,
}

impl Default for SectorCoordinators {
    fn default() -> Self {
        Self {
            sector_consensus_threshold: 0.65,
            intra_sector_correlation_limit: 0.80,
            max_sector_positions: 4,
            position_sizing_method: "risk_parity".to_string(),
            risk_per_sector: 0.02,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingMechanisms {
    pub voting_strategy: String,
    pub minimum_model_agreement: f64,
    pub uncertainty_penalty_factor: f64,
    pub recency_bias_factor: f64,
    pub performance_weight_decay: f64,
}

impl Default for VotingMechanisms {
    fn default() -> Self {
        Self {
            voting_strategy: "weighted_confidence".to_string(),
            minimum_model_agreement: 0.60,
            uncertainty_penalty_factor: 0.15,
            recency_bias_factor: 0.10,
            performance_weight_decay: 0.95,
        }
    }
}

/// Integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    pub description: String,
    pub redis_channels: RedisChannels,
    pub daa_bridge: DAABridge,
    pub health_monitoring: HealthMonitoring,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            description: "Integration with existing systems".to_string(),
            redis_channels: RedisChannels::default(),
            daa_bridge: DAABridge::default(),
            health_monitoring: HealthMonitoring::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisChannels {
    pub preserve_symbol_channels: bool,
    pub add_sector_aggregation: bool,
    pub sector_metrics_ttl: u32,
    pub enable_cross_sector_messaging: bool,
}

impl Default for RedisChannels {
    fn default() -> Self {
        Self {
            preserve_symbol_channels: true,
            add_sector_aggregation: true,
            sector_metrics_ttl: 180,
            enable_cross_sector_messaging: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAABridge {
    pub enable_legacy_compatibility: bool,
    pub enhanced_decision_weight: f64,
    pub legacy_decision_weight: f64,
    pub performance_feedback_enabled: bool,
}

impl Default for DAABridge {
    fn default() -> Self {
        Self {
            enable_legacy_compatibility: true,
            enhanced_decision_weight: 0.70,
            legacy_decision_weight: 0.30,
            performance_feedback_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMonitoring {
    pub enable_sector_health_checks: bool,
    pub health_check_interval_seconds: u32,
    pub sector_degradation_threshold: f64,
    pub auto_failover_enabled: bool,
}

impl Default for HealthMonitoring {
    fn default() -> Self {
        Self {
            enable_sector_health_checks: true,
            health_check_interval_seconds: 30,
            sector_degradation_threshold: 0.10,
            auto_failover_enabled: true,
        }
    }
}

/// Lazy loading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyLoadingConfig {
    pub description: String,
    pub activation_conditions: HashMap<String, serde_json::Value>,
    pub deactivation_conditions: HashMap<String, serde_json::Value>,
}

impl Default for LazyLoadingConfig {
    fn default() -> Self {
        Self {
            description: "Conditions for model activation and deactivation".to_string(),
            activation_conditions: HashMap::new(),
            deactivation_conditions: HashMap::new(),
        }
    }
}

/// Testing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingConfig {
    pub description: String,
    pub backtesting: BacktestingConfig,
    pub performance_benchmarks: PerformanceBenchmarks,
    pub model_validation: ModelValidation,
}

impl Default for TestingConfig {
    fn default() -> Self {
        Self {
            description: "Configuration for testing and validation".to_string(),
            backtesting: BacktestingConfig::default(),
            performance_benchmarks: PerformanceBenchmarks::default(),
            model_validation: ModelValidation::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestingConfig {
    pub min_backtest_days: u32,
    pub validation_split: f64,
    pub walk_forward_windows: u32,
    pub monte_carlo_simulations: u32,
}

impl Default for BacktestingConfig {
    fn default() -> Self {
        Self {
            min_backtest_days: 252,
            validation_split: 0.20,
            walk_forward_windows: 12,
            monte_carlo_simulations: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmarks {
    pub benchmark_symbols: Vec<String>,
    pub performance_metrics: Vec<String>,
    pub min_benchmark_outperformance: f64,
}

impl Default for PerformanceBenchmarks {
    fn default() -> Self {
        Self {
            benchmark_symbols: vec!["SPY".to_string(), "QQQ".to_string()],
            performance_metrics: vec!["sharpe_ratio".to_string(), "max_drawdown".to_string()],
            min_benchmark_outperformance: 0.02,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelValidation {
    pub cross_validation_folds: u32,
    pub holdout_test_percentage: f64,
    pub statistical_significance_level: f64,
    pub minimum_sample_size: u32,
}

impl Default for ModelValidation {
    fn default() -> Self {
        Self {
            cross_validation_folds: 5,
            holdout_test_percentage: 0.15,
            statistical_significance_level: 0.05,
            minimum_sample_size: 1000,
        }
    }
}

impl SectorModelsConfig {
    /// Load sector models configuration from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read sector config file: {}", path.as_ref().display()))?;
        
        let config: Self = toml::from_str(&config_str)
            .with_context(|| "Failed to parse sector config file as TOML")?;
        
        Ok(config)
    }
    
    /// Load from the default location
    pub fn load_default() -> Result<Self> {
        // Check environment variable first, fallback to default path
        let config_path = std::env::var("SECTOR_CONFIG_PATH")
            .unwrap_or_else(|_| "config/sector_models.toml".to_string());
        Self::load_from_file(&config_path)
    }
    
    /// Get sector configuration by name
    pub fn get_sector(&self, sector_name: &str) -> Option<&SectorConfig> {
        self.sectors.get(sector_name)
    }
    
    /// Get model configuration by name
    pub fn get_model(&self, model_name: &str) -> Option<&ModelConfig> {
        self.models.get(model_name)
    }
    
    /// Get all sectors
    pub fn all_sectors(&self) -> impl Iterator<Item = (&String, &SectorConfig)> {
        self.sectors.iter()
    }
    
    /// Get all models for a sector
    pub fn models_for_sector<'a>(&'a self, sector_name: &'a str) -> impl Iterator<Item = (&String, &ModelConfig)> + '_ {
        self.models.iter().filter(move |(_, model)| model.sector == sector_name)
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Check that all model sectors reference valid sectors
        for (model_name, model) in &self.models {
            if !self.sectors.contains_key(&model.sector) {
                return Err(anyhow::anyhow!(
                    "Model {} references unknown sector: {}", 
                    model_name, 
                    model.sector
                ));
            }
        }
        
        // Validate accuracy thresholds
        if self.performance.accuracy_thresholds.min_sector_accuracy < 0.0 
            || self.performance.accuracy_thresholds.min_sector_accuracy > 1.0 {
            return Err(anyhow::anyhow!("Invalid min_sector_accuracy threshold"));
        }
        
        // Validate memory limits
        if self.performance.resource_limits.max_total_memory_gb <= 0.0 {
            return Err(anyhow::anyhow!("Invalid memory limit"));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = SectorModelsConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.metadata.version, "2.0.0");
    }
    
    #[test]
    fn test_performance_config_defaults() {
        let perf = PerformanceConfig::default();
        assert_eq!(perf.memory_optimization.enable_shared_features, true);
        assert_eq!(perf.prediction_latency.max_prediction_latency_ms, 100);
        assert_eq!(perf.accuracy_thresholds.min_sector_accuracy, 0.70);
    }
    
    #[test]
    fn test_daa_coordination_defaults() {
        let daa = DAACoordinationConfig::default();
        assert_eq!(daa.master_coordinator.portfolio_consensus_threshold, 0.70);
        assert_eq!(daa.sector_coordinators.sector_consensus_threshold, 0.65);
        assert_eq!(daa.voting_mechanisms.voting_strategy, "weighted_confidence");
    }
}