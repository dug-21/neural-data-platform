//! Neural Trading Execution Binary

use anyhow::Result;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, error};

mod daa;
mod execution;
mod risk;
mod inference;
mod events;

use daa::coordinator::DAACoordinator;
use execution::engine::ExecutionEngine;
use risk::manager::RiskManager;
use inference::predictor::NeuralPredictor;
use events::consumer::EventConsumer;

#[derive(Debug, Clone)]
pub struct TradingConfig {
    pub redis_url: String,
    pub postgres_url: String,
    pub broker_endpoint: String,
    pub neural_model_path: String,
    pub risk_limits: RiskLimits,
    pub execution_params: ExecutionParams,
}

#[derive(Debug, Clone)]
pub struct RiskLimits {
    pub max_position_size: f64,
    pub max_daily_loss: f64,
    pub max_drawdown: f64,
    pub max_correlation_exposure: f64,
}

#[derive(Debug, Clone)]
pub struct ExecutionParams {
    pub order_timeout_ms: u64,
    pub max_slippage_bps: u32,
    pub min_confidence_threshold: f64,
    pub max_orders_per_minute: u32,
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            postgres_url: "postgresql://localhost/neural_trader".to_string(),
            broker_endpoint: "http://localhost:8080".to_string(),
            neural_model_path: "./models/trading_model.safetensors".to_string(),
            risk_limits: RiskLimits {
                max_position_size: 0.05,
                max_daily_loss: 0.02,
                max_drawdown: 0.10,
                max_correlation_exposure: 0.20,
            },
            execution_params: ExecutionParams {
                order_timeout_ms: 30000,
                max_slippage_bps: 20,
                min_confidence_threshold: 0.65,
                max_orders_per_minute: 10,
            },
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting Neural Trading Engine v0.1.0");
    
    let config = load_config()?;
    info!("Configuration loaded successfully");

    let trading_system = TradingSystem::new(config).await?;
    info!("Trading system initialized");

    trading_system.start().await?;
    info!("All services started successfully");

    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully...");
        }
        _ = trading_system.wait_for_error() => {
            error!("System error detected, initiating shutdown");
        }
    }

    trading_system.shutdown().await?;
    info!("System shutdown completed");

    Ok(())
}

pub struct TradingSystem {
    daa_coordinator: Arc<DAACoordinator>,
    execution_engine: Arc<ExecutionEngine>,
    risk_manager: Arc<RiskManager>,
    neural_predictor: Arc<NeuralPredictor>,
    event_consumer: Arc<EventConsumer>,
}

impl TradingSystem {
    pub async fn new(config: TradingConfig) -> Result<Self> {
        let neural_predictor = Arc::new(
            NeuralPredictor::new(&config.neural_model_path).await?
        );

        let risk_manager = Arc::new(
            RiskManager::new(config.risk_limits.clone())
        );

        let execution_engine = Arc::new(
            ExecutionEngine::new(
                config.broker_endpoint.clone(),
                config.execution_params.clone(),
                risk_manager.clone(),
            ).await?
        );

        let daa_coordinator = Arc::new(
            DAACoordinator::new(
                config.redis_url.clone(),
                neural_predictor.clone(),
                execution_engine.clone(),
                risk_manager.clone(),
            ).await?
        );

        let event_consumer = Arc::new(
            EventConsumer::new(
                config.redis_url.clone(),
                daa_coordinator.clone(),
            ).await?
        );

        Ok(Self {
            daa_coordinator,
            execution_engine,
            risk_manager,
            neural_predictor,
            event_consumer,
        })
    }

    pub async fn start(&self) -> Result<()> {
        self.event_consumer.start().await?;
        self.daa_coordinator.start().await?;
        self.execution_engine.start().await?;
        self.risk_manager.start_monitoring().await?;

        info!("All trading system components started");
        Ok(())
    }

    pub async fn wait_for_error(&self) -> Result<()> {
        tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating system shutdown");
        
        self.event_consumer.stop().await?;
        self.execution_engine.stop().await?;
        self.daa_coordinator.stop().await?;
        self.risk_manager.stop().await?;

        info!("System shutdown completed");
        Ok(())
    }
}

fn load_config() -> Result<TradingConfig> {
    let mut config = TradingConfig::default();
    
    if let Ok(redis_url) = std::env::var("REDIS_URL") {
        config.redis_url = redis_url;
    }
    
    if let Ok(postgres_url) = std::env::var("DATABASE_URL") {
        config.postgres_url = postgres_url;
    }
    
    if let Ok(broker_endpoint) = std::env::var("BROKER_ENDPOINT") {
        config.broker_endpoint = broker_endpoint;
    }

    if let Ok(model_path) = std::env::var("NEURAL_MODEL_PATH") {
        config.neural_model_path = model_path;
    }

    Ok(config)
}