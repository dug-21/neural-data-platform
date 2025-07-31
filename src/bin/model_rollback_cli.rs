//! Model Rollback CLI Tool
//!
//! Command-line interface for managing neural model rollbacks in production.

use anyhow::Result;
use autonomous_platform::adapters::model_rollback::{
    cli::{Cli, execute_command},
    ModelRollbackManager, RollbackConfig,
};
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,model_rollback=debug")
        .init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Load configuration
    let config = load_config()?;

    // Create rollback manager
    let rollback_config = RollbackConfig {
        model_base_dir: PathBuf::from(&config.model_base_dir),
        metadata_backup_path: PathBuf::from(&config.metadata_backup_path),
        max_versions: config.max_model_versions.unwrap_or(5),
        degradation_threshold: config.degradation_threshold.unwrap_or(10.0),
        evaluation_period: config.evaluation_period.unwrap_or(300),
        sample_count: config.sample_count.unwrap_or(20),
        enable_auto_rollback: config.enable_auto_rollback.unwrap_or(true),
        health_check_interval: std::time::Duration::from_secs(
            config.health_check_interval.unwrap_or(30)
        ),
        grace_period: std::time::Duration::from_secs(
            config.grace_period.unwrap_or(60)
        ),
        enable_metadata_backup: config.enable_metadata_backup.unwrap_or(true),
    };

    let manager = ModelRollbackManager::new(rollback_config)?;

    // Execute command
    match execute_command(&manager, cli.command).await {
        Ok(()) => {
            info!("Command executed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Command failed: {}", e);
            Err(e)
        }
    }
}

/// Configuration structure for the CLI tool
#[derive(serde::Deserialize)]
struct CliConfig {
    model_base_dir: String,
    metadata_backup_path: String,
    max_model_versions: Option<usize>,
    degradation_threshold: Option<f32>,
    evaluation_period: Option<u64>,
    sample_count: Option<usize>,
    enable_auto_rollback: Option<bool>,
    health_check_interval: Option<u64>,
    grace_period: Option<u64>,
    enable_metadata_backup: Option<bool>,
}

/// Load configuration from file or environment
fn load_config() -> Result<CliConfig> {
    // Try to load from environment variable first
    if let Ok(config_path) = std::env::var("ROLLBACK_CONFIG_PATH") {
        let config_str = std::fs::read_to_string(config_path)?;
        let config: CliConfig = toml::from_str(&config_str)?;
        return Ok(config);
    }

    // Default configuration
    Ok(CliConfig {
        model_base_dir: std::env::var("MODEL_BASE_DIR")
            .unwrap_or_else(|_| "/opt/neural-trader/models".to_string()),
        metadata_backup_path: std::env::var("METADATA_BACKUP_PATH")
            .unwrap_or_else(|_| "/opt/neural-trader/metadata".to_string()),
        max_model_versions: None,
        degradation_threshold: None,
        evaluation_period: None,
        sample_count: None,
        enable_auto_rollback: None,
        health_check_interval: None,
        grace_period: None,
        enable_metadata_backup: None,
    })
}