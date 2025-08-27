//! Action Layer for Trading Execution MVP
//!
//! This module provides the minimal viable trading execution capabilities:
//! - Trade execution with Alpaca broker integration
//! - Basic risk management
//! - Position tracking and P&L calculation
//! - Paper trading mode for testing
//! - REST API for order submission
//! - WebSocket for real-time updates
//! - Audit logging and emergency controls

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;

pub mod execution_engine;
pub mod risk_manager;
pub mod position_tracker;
pub mod brokers;
pub mod api_server;
pub mod websocket_server;
pub mod audit_logger;
pub mod emergency_controls;

#[derive(Error, Debug)]
pub enum ActionLayerError {
    #[error("Broker error: {0}")]
    Broker(String),
    
    #[error("Risk limit exceeded: {0}")]
    RiskLimitExceeded(String),
    
    #[error("Position error: {0}")]
    Position(String),
    
    #[error("Order execution failed: {0}")]
    OrderExecution(String),
    
    #[error("Emergency stop active")]
    EmergencyStop,
    
    #[error("Paper trading violation: {0}")]
    PaperTradingViolation(String),
}

// Core trading types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub time_in_force: TimeInForce,
    pub created_at: DateTime<Utc>,
    pub status: OrderStatus,
    pub filled_quantity: f64,
    pub avg_fill_price: Option<f64>,
    pub source: OrderSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopLoss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeInForce {
    Day,
    GoodTilCancelled,
    ImmediateOrCancel,
    FillOrKill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Submitted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSource {
    Neural,
    Manual,
    Risk,
    Emergency,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub avg_entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub side: PositionSide,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum PositionSide {
    Long,
    Short,
    Flat,
}

#[derive(Debug, Clone)]
pub struct TradingAccount {
    pub equity: f64,
    pub buying_power: f64,
    pub cash: f64,
    pub day_trading_buying_power: f64,
    pub portfolio_value: f64,
    pub positions: HashMap<String, Position>,
    pub daily_pnl: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: f64,        // Maximum position size as % of portfolio
    pub max_daily_loss: f64,           // Maximum daily loss limit
    pub max_portfolio_risk: f64,       // Maximum portfolio risk
    pub max_drawdown: f64,             // Maximum drawdown limit
    pub max_correlation_exposure: f64,  // Maximum exposure to correlated positions
    pub stop_loss_percentage: f64,     // Default stop loss percentage
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            max_position_size: 0.05,      // 5% of portfolio per position
            max_daily_loss: 0.02,         // 2% daily loss limit
            max_portfolio_risk: 0.10,     // 10% portfolio risk limit
            max_drawdown: 0.15,           // 15% maximum drawdown
            max_correlation_exposure: 0.30, // 30% max correlated exposure
            stop_loss_percentage: 0.02,   // 2% stop loss
        }
    }
}

// Core traits
#[async_trait]
pub trait BrokerInterface: Send + Sync {
    async fn submit_order(&self, order: &Order) -> Result<String, ActionLayerError>;
    async fn cancel_order(&self, order_id: &str) -> Result<(), ActionLayerError>;
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus, ActionLayerError>;
    async fn get_account_info(&self) -> Result<TradingAccount, ActionLayerError>;
    async fn get_positions(&self) -> Result<HashMap<String, Position>, ActionLayerError>;
    async fn get_current_price(&self, symbol: &str) -> Result<f64, ActionLayerError>;
}

#[async_trait]
pub trait RiskManager: Send + Sync {
    async fn validate_order(&self, order: &Order, account: &TradingAccount) -> Result<bool, ActionLayerError>;
    async fn check_position_limits(&self, symbol: &str, quantity: f64, account: &TradingAccount) -> Result<bool, ActionLayerError>;
    async fn check_daily_limits(&self, account: &TradingAccount) -> Result<bool, ActionLayerError>;
    async fn calculate_position_size(&self, signal_strength: f64, account: &TradingAccount) -> Result<f64, ActionLayerError>;
}

#[async_trait]
pub trait PositionTracker: Send + Sync {
    async fn update_position(&self, symbol: &str, fill: &OrderFill) -> Result<(), ActionLayerError>;
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, ActionLayerError>;
    async fn get_all_positions(&self) -> Result<HashMap<String, Position>, ActionLayerError>;
    async fn calculate_pnl(&self, symbol: &str, current_price: f64) -> Result<f64, ActionLayerError>;
}

#[derive(Debug, Clone)]
pub struct OrderFill {
    pub order_id: Uuid,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub commission: f64,
}

// Configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ActionLayerConfig {
    pub broker: BrokerConfig,
    pub risk: RiskLimits,
    pub paper_trading: bool,
    pub api_port: u16,
    pub websocket_port: u16,
    pub audit_log_path: String,
    pub emergency_stop: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrokerConfig {
    pub name: String,
    pub api_key: String,
    pub secret_key: String,
    pub base_url: String,
    pub paper_trading: bool,
}

impl Default for ActionLayerConfig {
    fn default() -> Self {
        Self {
            broker: BrokerConfig {
                name: "alpaca".to_string(),
                api_key: "".to_string(),
                secret_key: "".to_string(),
                base_url: "https://paper-api.alpaca.markets".to_string(),
                paper_trading: true,
            },
            risk: RiskLimits::default(),
            paper_trading: true,
            api_port: 8080,
            websocket_port: 8081,
            audit_log_path: "./logs/trading_audit.log".to_string(),
            emergency_stop: false,
        }
    }
}

// Main Action Layer coordinator
pub struct ActionLayer {
    pub config: ActionLayerConfig,
    pub broker: Arc<dyn BrokerInterface>,
    pub risk_manager: Arc<dyn RiskManager>,
    pub position_tracker: Arc<dyn PositionTracker>,
    pub audit_logger: Arc<audit_logger::AuditLogger>,
    pub emergency_controls: Arc<emergency_controls::EmergencyControls>,
    pub orders: Arc<RwLock<HashMap<Uuid, Order>>>,
    pub account: Arc<RwLock<TradingAccount>>,
}

impl ActionLayer {
    pub async fn new(config: ActionLayerConfig) -> Result<Self, ActionLayerError> {
        let broker = brokers::create_broker(&config.broker).await?;
        let risk_manager = Arc::new(risk_manager::BasicRiskManager::new(config.risk.clone()));
        let position_tracker = Arc::new(position_tracker::BasicPositionTracker::new());
        let audit_logger = Arc::new(audit_logger::AuditLogger::new(&config.audit_log_path)?);
        let emergency_controls = Arc::new(emergency_controls::EmergencyControls::new());
        
        let initial_account = broker.get_account_info().await?;
        
        Ok(Self {
            config,
            broker,
            risk_manager,
            position_tracker,
            audit_logger,
            emergency_controls,
            orders: Arc::new(RwLock::new(HashMap::new())),
            account: Arc::new(RwLock::new(initial_account)),
        })
    }
    
    /// Submit a new trading order
    pub async fn submit_order(&self, mut order: Order) -> Result<Uuid, ActionLayerError> {
        // Check emergency stop
        if self.emergency_controls.is_stopped().await {
            return Err(ActionLayerError::EmergencyStop);
        }
        
        // Generate order ID if not provided
        if order.id == Uuid::nil() {
            order.id = Uuid::new_v4();
        }
        
        // Validate paper trading constraints
        if self.config.paper_trading && order.source == OrderSource::Manual {
            return Err(ActionLayerError::PaperTradingViolation(
                "Manual orders not allowed in paper trading mode".to_string()
            ));
        }
        
        // Get current account state
        let account = self.account.read().await.clone();
        
        // Risk validation
        if !self.risk_manager.validate_order(&order, &account).await? {
            return Err(ActionLayerError::RiskLimitExceeded(
                "Order failed risk validation".to_string()
            ));
        }
        
        // Log order submission
        self.audit_logger.log_order_submit(&order).await?;
        
        // Submit to broker
        order.status = OrderStatus::Submitted;
        let broker_order_id = self.broker.submit_order(&order).await?;
        
        // Store order
        self.orders.write().await.insert(order.id, order.clone());
        
        // Log successful submission
        self.audit_logger.log_order_accepted(&order, &broker_order_id).await?;
        
        Ok(order.id)
    }
    
    /// Cancel an existing order
    pub async fn cancel_order(&self, order_id: Uuid) -> Result<(), ActionLayerError> {
        let mut orders = self.orders.write().await;
        
        if let Some(order) = orders.get_mut(&order_id) {
            if matches!(order.status, OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected) {
                return Err(ActionLayerError::OrderExecution(
                    "Cannot cancel already completed order".to_string()
                ));
            }
            
            // Cancel with broker (using order ID as broker order ID for simplicity)
            self.broker.cancel_order(&order_id.to_string()).await?;
            
            // Update status
            order.status = OrderStatus::Cancelled;
            
            // Log cancellation
            self.audit_logger.log_order_cancel(order).await?;
            
            Ok(())
        } else {
            Err(ActionLayerError::Position("Order not found".to_string()))
        }
    }
    
    /// Get current positions
    pub async fn get_positions(&self) -> Result<HashMap<String, Position>, ActionLayerError> {
        self.position_tracker.get_all_positions().await
    }
    
    /// Get current account information
    pub async fn get_account(&self) -> Result<TradingAccount, ActionLayerError> {
        Ok(self.account.read().await.clone())
    }
    
    /// Get order status
    pub async fn get_order(&self, order_id: Uuid) -> Result<Option<Order>, ActionLayerError> {
        Ok(self.orders.read().await.get(&order_id).cloned())
    }
    
    /// Emergency stop all trading
    pub async fn emergency_stop(&self, reason: &str) -> Result<(), ActionLayerError> {
        self.emergency_controls.activate_stop(reason).await;
        
        // Cancel all pending orders
        let orders = self.orders.read().await;
        for (_, order) in orders.iter() {
            if matches!(order.status, OrderStatus::Pending | OrderStatus::Submitted | OrderStatus::PartiallyFilled) {
                let _ = self.broker.cancel_order(&order.id.to_string()).await;
            }
        }
        
        // Log emergency stop
        self.audit_logger.log_emergency_stop(reason).await?;
        
        Ok(())
    }
    
    /// Resume trading after emergency stop
    pub async fn resume_trading(&self) -> Result<(), ActionLayerError> {
        self.emergency_controls.deactivate_stop().await;
        self.audit_logger.log_trading_resume().await?;
        Ok(())
    }
    
    /// Update account and positions (called periodically)
    pub async fn update_account_state(&self) -> Result<(), ActionLayerError> {
        let account = self.broker.get_account_info().await?;
        *self.account.write().await = account;
        Ok(())
    }
    
    /// Start the action layer services (API server, WebSocket, etc.)
    pub async fn start_services(&self) -> Result<(), ActionLayerError> {
        let action_layer_arc = Arc::new(self.clone());
        
        // Start API server
        let api_server = api_server::ApiServer::new(
            self.config.api_port,
            action_layer_arc.clone(),
        );
        
        // Start WebSocket server
        let websocket_server = websocket_server::WebSocketServer::new(
            self.config.websocket_port,
            action_layer_arc.clone(),
        );
        
        // Start execution engine
        let execution_engine = execution_engine::ExecutionEngine::new(action_layer_arc.clone());
        execution_engine.start().await?;
        
        // Start services in background tasks
        let api_task = {
            let api_server = api_server;
            tokio::spawn(async move {
                if let Err(e) = api_server.start().await {
                    tracing::error!("API server error: {}", e);
                }
            })
        };
        
        let websocket_task = {
            let websocket_server = websocket_server;
            tokio::spawn(async move {
                if let Err(e) = websocket_server.start().await {
                    tracing::error!("WebSocket server error: {}", e);
                }
            })
        };
        
        // Log successful startup
        self.audit_logger.log_system_event(
            "Action Layer services started",
            Some(serde_json::json!({
                "api_port": self.config.api_port,
                "websocket_port": self.config.websocket_port,
                "paper_trading": self.config.paper_trading,
                "broker": self.config.broker.name
            }))
        ).await?;
        
        tracing::info!("🚀 Neural Trader Action Layer started successfully");
        tracing::info!("   📊 API Server: http://localhost:{}", self.config.api_port);
        tracing::info!("   📡 WebSocket: ws://localhost:{}/ws", self.config.websocket_port);
        tracing::info!("   📄 Paper Trading: {}", self.config.paper_trading);
        tracing::info!("   🏪 Broker: {}", self.config.broker.name);
        
        // Wait for both servers (in production, you might want different handling)
        tokio::select! {
            _ = api_task => {
                tracing::error!("API server task completed unexpectedly");
            }
            _ = websocket_task => {
                tracing::error!("WebSocket server task completed unexpectedly");  
            }
        }
        
        Ok(())
    }
}

// Helper implementations for cloning
impl Clone for ActionLayer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            broker: self.broker.clone(),
            risk_manager: self.risk_manager.clone(),
            position_tracker: self.position_tracker.clone(),
            audit_logger: self.audit_logger.clone(),
            emergency_controls: self.emergency_controls.clone(),
            orders: self.orders.clone(),
            account: self.account.clone(),
        }
    }
}