//! Example of integrating services with config-store
//! Shows how to load configuration, handle hot-reloading, and manage secrets

use config_store::{ConfigStoreBuilder, ConfigStore, ServiceConfig, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Configuration structure for data ingestion service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataIngestionConfig {
    pub sources: DataSourcesConfig,
    pub validation: ValidationConfig,
    pub processing: ProcessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourcesConfig {
    pub primary: DataSourceConfig,
    pub fallback: Option<DataSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    pub provider: String,
    pub api_url: String,
    pub websocket_url: String,
    pub symbols: Vec<String>,
    pub rate_limits: RateLimitsConfig,
    pub retry_policy: RetryPolicyConfig,
    // Secrets loaded separately from environment
    #[serde(skip)]
    pub api_key: Option<String>,
    #[serde(skip)]
    pub api_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitsConfig {
    pub requests_per_minute: u32,
    pub websocket_connections: u32,
    pub burst_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicyConfig {
    pub max_attempts: u32,
    pub backoff_multiplier: f64,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub price_range: PriceRangeConfig,
    pub timestamp_tolerance_ms: u64,
    pub required_fields: Vec<String>,
    pub data_quality_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceRangeConfig {
    pub min_price: f64,
    pub max_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub batch_size: u32,
    pub flush_interval_ms: u64,
    pub enable_compression: bool,
    pub parallelism: u32,
}

/// Service that demonstrates config-store integration
pub struct DataIngestionService {
    config: Arc<RwLock<DataIngestionConfig>>,
    config_watcher: Option<tokio::task::JoinHandle<()>>,
}

impl DataIngestionService {
    /// Create new service instance with configuration from config-store
    pub async fn new(config_store_url: &str) -> Result<Self> {
        println!("Initializing DataIngestionService with config-store integration");
        
        // Connect to config-store
        let config_store = ConfigStore::connect(config_store_url).await?;
        
        // Load configuration from namespace
        let namespace = "neural-trading/data-ingestion";
        let mut config: DataIngestionConfig = config_store
            .get_namespace(namespace)
            .await?
            .try_into()?;
        
        // Apply secrets from environment variables
        Self::apply_secrets(&mut config)?;
        
        // Validate configuration
        Self::validate_config(&config)?;
        
        let config_arc = Arc::new(RwLock::new(config));
        
        // Start configuration watcher for hot-reloading
        let watcher_config = config_arc.clone();
        let watcher_store = config_store.clone();
        let watcher_namespace = namespace.to_string();
        
        let config_watcher = tokio::spawn(async move {
            Self::config_watcher_task(watcher_store, watcher_namespace, watcher_config).await;
        });
        
        Ok(Self {
            config: config_arc,
            config_watcher: Some(config_watcher),
        })
    }
    
    /// Apply secrets from environment variables
    fn apply_secrets(config: &mut DataIngestionConfig) -> Result<()> {
        println!("Applying secrets from environment variables");
        
        // Load API keys from environment
        if let Ok(api_key) = std::env::var("ALPACA_API_KEY") {
            config.sources.primary.api_key = Some(api_key);
        }
        
        if let Ok(api_secret) = std::env::var("ALPACA_API_SECRET") {
            config.sources.primary.api_secret = Some(api_secret);
        }
        
        // Apply URL overrides from environment if provided
        if let Ok(api_url) = std::env::var("ALPACA_API_URL") {
            config.sources.primary.api_url = api_url;
        }
        
        if let Ok(ws_url) = std::env::var("ALPACA_WS_URL") {
            config.sources.primary.websocket_url = ws_url;
        }
        
        // Validate that required secrets are present
        if config.sources.primary.api_key.is_none() {
            return Err("ALPACA_API_KEY environment variable is required".into());
        }
        
        if config.sources.primary.api_secret.is_none() {
            return Err("ALPACA_API_SECRET environment variable is required".into());
        }
        
        Ok(())
    }
    
    /// Validate configuration structure and values
    fn validate_config(config: &DataIngestionConfig) -> Result<()> {
        println!("Validating configuration");
        
        // Validate price range
        if config.validation.price_range.min_price >= config.validation.price_range.max_price {
            return Err("Invalid price range: min_price must be less than max_price".into());
        }
        
        // Validate symbols list
        if config.sources.primary.symbols.is_empty() {
            return Err("Symbols list cannot be empty".into());
        }
        
        // Validate rate limits
        if config.sources.primary.rate_limits.requests_per_minute == 0 {
            return Err("Rate limit requests_per_minute must be greater than 0".into());
        }
        
        // Validate processing settings
        if config.processing.batch_size == 0 {
            return Err("Processing batch_size must be greater than 0".into());
        }
        
        if config.processing.parallelism == 0 {
            return Err("Processing parallelism must be greater than 0".into());
        }
        
        println!("Configuration validation passed");
        Ok(())
    }
    
    /// Background task to watch for configuration changes
    async fn config_watcher_task(
        config_store: ConfigStore,
        namespace: String,
        config: Arc<RwLock<DataIngestionConfig>>,
    ) {
        println!("Starting configuration watcher for namespace: {}", namespace);
        
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            match config_store.get_namespace(&namespace).await {
                Ok(new_config_value) => {
                    match new_config_value.try_into::<DataIngestionConfig>() {
                        Ok(mut new_config) => {
                            // Apply secrets to new configuration
                            if let Err(e) = Self::apply_secrets(&mut new_config) {
                                eprintln!("Failed to apply secrets to new configuration: {}", e);
                                continue;
                            }
                            
                            // Validate new configuration
                            if let Err(e) = Self::validate_config(&new_config) {
                                eprintln!("New configuration failed validation: {}", e);
                                continue;
                            }
                            
                            // Check if configuration actually changed
                            let current_config = config.read().await;
                            if !Self::config_changed(&*current_config, &new_config) {
                                continue;
                            }
                            drop(current_config);
                            
                            // Update configuration
                            let mut config_guard = config.write().await;
                            *config_guard = new_config;
                            println!("Configuration updated successfully");
                            
                            // Trigger any necessary reconfigurations
                            Self::on_config_change(&*config_guard).await;
                        }
                        Err(e) => {
                            eprintln!("Failed to deserialize new configuration: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to fetch configuration update: {}", e);
                }
            }
        }
    }
    
    /// Check if configuration has changed significantly
    fn config_changed(old: &DataIngestionConfig, new: &DataIngestionConfig) -> bool {
        // Compare key fields that would require service reconfiguration
        old.sources.primary.api_url != new.sources.primary.api_url
            || old.sources.primary.websocket_url != new.sources.primary.websocket_url
            || old.sources.primary.symbols != new.sources.primary.symbols
            || old.sources.primary.rate_limits.requests_per_minute != new.sources.primary.rate_limits.requests_per_minute
            || old.processing.batch_size != new.processing.batch_size
            || old.processing.parallelism != new.processing.parallelism
            || old.validation.data_quality_threshold != new.validation.data_quality_threshold
    }
    
    /// Handle configuration changes
    async fn on_config_change(config: &DataIngestionConfig) {
        println!("Applying configuration changes:");
        println!("  Primary provider: {}", config.sources.primary.provider);
        println!("  API URL: {}", config.sources.primary.api_url);
        println!("  Symbols: {:?}", config.sources.primary.symbols);
        println!("  Batch size: {}", config.processing.batch_size);
        println!("  Parallelism: {}", config.processing.parallelism);
        
        // Here you would reconfigure your service components
        // For example:
        // - Update data source connections
        // - Reconfigure processing pipelines
        // - Update validation rules
        // - Restart workers with new parallelism settings
    }
    
    /// Get current configuration (read-only access)
    pub async fn get_config(&self) -> DataIngestionConfig {
        self.config.read().await.clone()
    }
    
    /// Start the service with current configuration
    pub async fn start(&self) -> Result<()> {
        let config = self.get_config().await;
        
        println!("Starting DataIngestionService with configuration:");
        println!("  Provider: {}", config.sources.primary.provider);
        println!("  Symbols: {:?}", config.sources.primary.symbols);
        println!("  Rate limit: {} req/min", config.sources.primary.rate_limits.requests_per_minute);
        println!("  Batch size: {}", config.processing.batch_size);
        
        // Initialize your service components here
        // - Set up data source connections
        // - Configure processing pipelines
        // - Start worker tasks
        
        Ok(())
    }
    
    /// Stop the service and cleanup resources
    pub async fn stop(&mut self) -> Result<()> {
        println!("Stopping DataIngestionService");
        
        // Stop configuration watcher
        if let Some(watcher) = self.config_watcher.take() {
            watcher.abort();
        }
        
        // Stop your service components here
        // - Close data source connections
        // - Stop worker tasks
        // - Flush remaining data
        
        Ok(())
    }
}

impl Drop for DataIngestionService {
    fn drop(&mut self) {
        // Ensure watcher task is cleaned up
        if let Some(watcher) = self.config_watcher.take() {
            watcher.abort();
        }
    }
}

/// Example of a simple config store client for testing
pub struct ConfigStoreClient {
    store: ConfigStore,
}

impl ConfigStoreClient {
    pub async fn new(url: &str) -> Result<Self> {
        let store = ConfigStore::connect(url).await?;
        Ok(Self { store })
    }
    
    /// Update configuration in the store (for testing/admin use)
    pub async fn update_data_ingestion_config(&self, config: &DataIngestionConfig) -> Result<()> {
        self.store
            .set_namespace("neural-trading/data-ingestion", config)
            .await?;
        
        println!("Configuration updated in store");
        Ok(())
    }
    
    /// Get current configuration from store
    pub async fn get_data_ingestion_config(&self) -> Result<DataIngestionConfig> {
        let config: DataIngestionConfig = self.store
            .get_namespace("neural-trading/data-ingestion")
            .await?
            .try_into()?;
        
        Ok(config)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("Config Store Integration Example");
    
    // Example 1: Initialize service with config-store
    let mut service = DataIngestionService::new("http://localhost:8003").await?;
    
    // Example 2: Start service with loaded configuration
    service.start().await?;
    
    // Example 3: Demonstrate configuration update (in real scenario, this would be done by admin)
    let client = ConfigStoreClient::new("http://localhost:8003").await?;
    
    // Get current config
    let mut current_config = client.get_data_ingestion_config().await?;
    
    // Modify some settings
    current_config.processing.batch_size = 2000;
    current_config.sources.primary.symbols.push("DDOG".to_string());
    
    // Update in store (this will trigger hot-reload in the service)
    client.update_data_ingestion_config(&current_config).await?;
    
    // Wait a bit to see hot-reload in action
    println!("Waiting for hot-reload to take effect...");
    tokio::time::sleep(tokio::time::Duration::from_secs(35)).await;
    
    // Verify configuration was updated in service
    let service_config = service.get_config().await;
    println!("Service batch size after update: {}", service_config.processing.batch_size);
    
    // Example 4: Clean shutdown
    service.stop().await?;
    
    println!("Example completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_config_validation() {
        let mut config = DataIngestionConfig {
            sources: DataSourcesConfig {
                primary: DataSourceConfig {
                    provider: "alpaca".to_string(),
                    api_url: "https://api.alpaca.markets".to_string(),
                    websocket_url: "wss://stream.alpaca.markets".to_string(),
                    symbols: vec!["AAPL".to_string()],
                    rate_limits: RateLimitsConfig {
                        requests_per_minute: 200,
                        websocket_connections: 5,
                        burst_limit: 50,
                    },
                    retry_policy: RetryPolicyConfig {
                        max_attempts: 3,
                        backoff_multiplier: 2.0,
                        initial_delay_ms: 1000,
                        max_delay_ms: 30000,
                    },
                    api_key: None,
                    api_secret: None,
                },
                fallback: None,
            },
            validation: ValidationConfig {
                price_range: PriceRangeConfig {
                    min_price: 0.01,
                    max_price: 10000.0,
                },
                timestamp_tolerance_ms: 300000,
                required_fields: vec!["symbol".to_string(), "price".to_string()],
                data_quality_threshold: 0.95,
            },
            processing: ProcessingConfig {
                batch_size: 1000,
                flush_interval_ms: 5000,
                enable_compression: true,
                parallelism: 4,
            },
        };
        
        // Valid configuration should pass
        assert!(DataIngestionService::validate_config(&config).is_ok());
        
        // Invalid price range should fail
        config.validation.price_range.min_price = 100.0;
        config.validation.price_range.max_price = 50.0;
        assert!(DataIngestionService::validate_config(&config).is_err());
        
        // Fix price range
        config.validation.price_range.min_price = 0.01;
        config.validation.price_range.max_price = 10000.0;
        
        // Empty symbols should fail
        config.sources.primary.symbols.clear();
        assert!(DataIngestionService::validate_config(&config).is_err());
        
        // Zero batch size should fail
        config.sources.primary.symbols.push("AAPL".to_string());
        config.processing.batch_size = 0;
        assert!(DataIngestionService::validate_config(&config).is_err());
    }
    
    #[test]
    fn test_config_change_detection() {
        let config1 = create_test_config();
        let mut config2 = config1.clone();
        
        // Same configuration should not trigger change
        assert!(!DataIngestionService::config_changed(&config1, &config2));
        
        // Different API URL should trigger change
        config2.sources.primary.api_url = "https://different.api.url".to_string();
        assert!(DataIngestionService::config_changed(&config1, &config2));
        
        // Different symbols should trigger change
        config2.sources.primary.api_url = config1.sources.primary.api_url.clone();
        config2.sources.primary.symbols.push("TSLA".to_string());
        assert!(DataIngestionService::config_changed(&config1, &config2));
    }
    
    fn create_test_config() -> DataIngestionConfig {
        DataIngestionConfig {
            sources: DataSourcesConfig {
                primary: DataSourceConfig {
                    provider: "alpaca".to_string(),
                    api_url: "https://api.alpaca.markets".to_string(),
                    websocket_url: "wss://stream.alpaca.markets".to_string(),
                    symbols: vec!["AAPL".to_string(), "GOOGL".to_string()],
                    rate_limits: RateLimitsConfig {
                        requests_per_minute: 200,
                        websocket_connections: 5,
                        burst_limit: 50,
                    },
                    retry_policy: RetryPolicyConfig {
                        max_attempts: 3,
                        backoff_multiplier: 2.0,
                        initial_delay_ms: 1000,
                        max_delay_ms: 30000,
                    },
                    api_key: None,
                    api_secret: None,
                },
                fallback: None,
            },
            validation: ValidationConfig {
                price_range: PriceRangeConfig {
                    min_price: 0.01,
                    max_price: 10000.0,
                },
                timestamp_tolerance_ms: 300000,
                required_fields: vec!["symbol".to_string(), "price".to_string()],
                data_quality_threshold: 0.95,
            },
            processing: ProcessingConfig {
                batch_size: 1000,
                flush_interval_ms: 5000,
                enable_compression: true,
                parallelism: 4,
            },
        }
    }
}