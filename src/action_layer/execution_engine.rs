//! Trade Execution Engine
//!
//! Core execution engine that orchestrates order processing, risk checks, and position management

use crate::action_layer::{
    ActionLayer, ActionLayerError, Order, OrderFill, OrderSide, OrderStatus, OrderSource
};
use chrono::Utc;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use uuid::Uuid;

pub struct ExecutionEngine {
    action_layer: Arc<ActionLayer>,
}

impl ExecutionEngine {
    pub fn new(action_layer: Arc<ActionLayer>) -> Self {
        Self { action_layer }
    }
    
    /// Start the execution engine background tasks
    pub async fn start(&self) -> Result<(), ActionLayerError> {
        let action_layer = self.action_layer.clone();
        
        // Start order monitoring task
        let order_monitor = action_layer.clone();
        tokio::spawn(async move {
            ExecutionEngine::order_monitoring_loop(order_monitor).await;
        });
        
        // Start position update task
        let position_updater = action_layer.clone();
        tokio::spawn(async move {
            ExecutionEngine::position_update_loop(position_updater).await;
        });
        
        // Start account sync task
        let account_syncer = action_layer.clone();
        tokio::spawn(async move {
            ExecutionEngine::account_sync_loop(account_syncer).await;
        });
        
        Ok(())
    }
    
    /// Process a trading signal from the neural network
    pub async fn process_neural_signal(
        &self,
        signal: TradingSignal,
    ) -> Result<Option<Uuid>, ActionLayerError> {
        // Check emergency stop
        if self.action_layer.emergency_controls.is_stopped().await {
            return Err(ActionLayerError::EmergencyStop);
        }
        
        // Validate signal strength threshold
        if signal.confidence < 0.6 {
            // Signal too weak, don't trade
            self.action_layer.audit_logger.log_system_event(
                &format!("Neural signal ignored: confidence {} below threshold", signal.confidence),
                Some(serde_json::json!({
                    "signal": signal,
                    "threshold": 0.6
                }))
            ).await?;
            return Ok(None);
        }
        
        // Get current account state
        let account = self.action_layer.get_account().await?;
        
        // Calculate position size based on signal strength
        let position_size_ratio = self.action_layer.risk_manager
            .calculate_position_size(signal.confidence, &account).await?;
        
        let position_value = position_size_ratio * account.portfolio_value;
        let current_price = self.action_layer.broker.get_current_price(&signal.symbol).await?;
        let quantity = (position_value / current_price).floor(); // Round down to avoid fractional shares
        
        if quantity < 1.0 {
            self.action_layer.audit_logger.log_system_event(
                "Position size too small, skipping trade",
                Some(serde_json::json!({
                    "signal": signal,
                    "calculated_quantity": quantity,
                    "position_value": position_value
                }))
            ).await?;
            return Ok(None);
        }
        
        // Create order
        let order = Order {
            id: Uuid::new_v4(),
            symbol: signal.symbol.clone(),
            side: match signal.action {
                SignalAction::Buy => OrderSide::Buy,
                SignalAction::Sell => OrderSide::Sell,
            },
            quantity,
            order_type: signal.order_type,
            price: signal.target_price,
            time_in_force: crate::action_layer::TimeInForce::Day,
            created_at: Utc::now(),
            status: OrderStatus::Pending,
            filled_quantity: 0.0,
            avg_fill_price: None,
            source: OrderSource::Neural,
        };
        
        // Submit order
        let order_id = self.action_layer.submit_order(order).await?;
        
        self.action_layer.audit_logger.log_system_event(
            &format!("Neural signal processed: {} order submitted", signal.action),
            Some(serde_json::json!({
                "signal": signal,
                "order_id": order_id,
                "quantity": quantity,
                "estimated_cost": position_value
            }))
        ).await?;
        
        Ok(Some(order_id))
    }
    
    /// Order monitoring loop - checks order status and handles fills
    async fn order_monitoring_loop(action_layer: Arc<ActionLayer>) {
        let mut interval = interval(Duration::from_secs(1)); // Check every second
        
        loop {
            interval.tick().await;
            
            if let Err(e) = Self::check_pending_orders(&action_layer).await {
                tracing::error!("Order monitoring error: {}", e);
                let _ = action_layer.audit_logger.log_error(
                    &format!("Order monitoring error: {}", e),
                    None
                ).await;
            }
        }
    }
    
    /// Check status of pending orders and handle fills
    async fn check_pending_orders(action_layer: &Arc<ActionLayer>) -> Result<(), ActionLayerError> {
        let orders = action_layer.orders.read().await;
        
        for (order_id, order) in orders.iter() {
            if matches!(order.status, OrderStatus::Submitted | OrderStatus::PartiallyFilled) {
                // Check order status with broker
                match action_layer.broker.get_order_status(&order_id.to_string()).await {
                    Ok(broker_status) => {
                        if !matches!(broker_status, OrderStatus::Submitted | OrderStatus::PartiallyFilled) {
                            // Order status changed, handle it
                            Self::handle_order_status_change(action_layer, order, broker_status).await?;
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get order status for {}: {}", order_id, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle order status changes (fills, cancellations, rejections)
    async fn handle_order_status_change(
        action_layer: &Arc<ActionLayer>,
        order: &Order,
        new_status: OrderStatus,
    ) -> Result<(), ActionLayerError> {
        match new_status {
            OrderStatus::Filled => {
                // For MVP, simulate fill price as current market price
                let fill_price = action_layer.broker.get_current_price(&order.symbol).await?;
                
                let fill = OrderFill {
                    order_id: order.id,
                    symbol: order.symbol.clone(),
                    side: order.side.clone(),
                    quantity: order.quantity,
                    price: fill_price,
                    timestamp: Utc::now(),
                    commission: Self::calculate_commission(order.quantity, fill_price),
                };
                
                // Update position
                action_layer.position_tracker.update_position(&order.symbol, &fill).await?;
                
                // Log fill
                action_layer.audit_logger.log_order_fill(order, fill_price, order.quantity).await?;
                
                // Update order in memory (in production, this would be persisted)
                let mut orders = action_layer.orders.write().await;
                if let Some(stored_order) = orders.get_mut(&order.id) {
                    stored_order.status = OrderStatus::Filled;
                    stored_order.filled_quantity = order.quantity;
                    stored_order.avg_fill_price = Some(fill_price);
                }
            }
            OrderStatus::Cancelled => {
                action_layer.audit_logger.log_order_cancel(order).await?;
                // Update order status
                let mut orders = action_layer.orders.write().await;
                if let Some(stored_order) = orders.get_mut(&order.id) {
                    stored_order.status = OrderStatus::Cancelled;
                }
            }
            OrderStatus::Rejected => {
                action_layer.audit_logger.log_order_reject(order, "Order rejected by broker").await?;
                // Update order status
                let mut orders = action_layer.orders.write().await;
                if let Some(stored_order) = orders.get_mut(&order.id) {
                    stored_order.status = OrderStatus::Rejected;
                }
            }
            _ => {} // Other statuses don't require action
        }
        
        Ok(())
    }
    
    /// Position update loop - updates current prices and P&L
    async fn position_update_loop(action_layer: Arc<ActionLayer>) {
        let mut interval = interval(Duration::from_secs(5)); // Update every 5 seconds
        
        loop {
            interval.tick().await;
            
            if let Err(e) = Self::update_position_prices(&action_layer).await {
                tracing::error!("Position update error: {}", e);
            }
        }
    }
    
    /// Update current prices and P&L for all positions
    async fn update_position_prices(action_layer: &Arc<ActionLayer>) -> Result<(), ActionLayerError> {
        let positions = action_layer.position_tracker.get_all_positions().await?;
        
        if positions.is_empty() {
            return Ok(());
        }
        
        let mut market_prices = std::collections::HashMap::new();
        
        for symbol in positions.keys() {
            match action_layer.broker.get_current_price(symbol).await {
                Ok(price) => {
                    market_prices.insert(symbol.clone(), price);
                }
                Err(e) => {
                    tracing::warn!("Failed to get price for {}: {}", symbol, e);
                }
            }
        }
        
        // Update position tracker with current prices
        if let Some(tracker) = action_layer.position_tracker.as_any()
            .downcast_ref::<crate::action_layer::position_tracker::BasicPositionTracker>() {
            tracker.update_unrealized_pnl(market_prices).await?;
        }
        
        Ok(())
    }
    
    /// Account sync loop - syncs account data with broker
    async fn account_sync_loop(action_layer: Arc<ActionLayer>) {
        let mut interval = interval(Duration::from_secs(30)); // Sync every 30 seconds
        
        loop {
            interval.tick().await;
            
            if let Err(e) = action_layer.update_account_state().await {
                tracing::error!("Account sync error: {}", e);
            }
        }
    }
    
    /// Calculate commission for a trade (simplified)
    fn calculate_commission(quantity: f64, price: f64) -> f64 {
        // Simplified commission calculation
        // In production, this would vary by broker and account type
        let per_share = 0.005; // $0.005 per share
        let min_commission = 1.00; // $1.00 minimum
        let max_commission = 10.00; // $10.00 maximum
        
        let calculated = quantity * per_share;
        calculated.max(min_commission).min(max_commission)
    }
    
    /// Emergency liquidation of all positions
    pub async fn emergency_liquidate_all(&self, reason: &str) -> Result<Vec<Uuid>, ActionLayerError> {
        self.action_layer.audit_logger.log_system_event(
            &format!("EMERGENCY LIQUIDATION: {}", reason),
            None
        ).await?;
        
        let positions = self.action_layer.get_positions().await?;
        let mut liquidation_orders = Vec::new();
        
        for (symbol, position) in positions {
            if position.quantity > 0.0 {
                let order = Order {
                    id: Uuid::new_v4(),
                    symbol: symbol.clone(),
                    side: match position.side {
                        crate::action_layer::PositionSide::Long => OrderSide::Sell,
                        crate::action_layer::PositionSide::Short => OrderSide::Buy,
                        crate::action_layer::PositionSide::Flat => continue,
                    },
                    quantity: position.quantity,
                    order_type: crate::action_layer::OrderType::Market,
                    price: None,
                    time_in_force: crate::action_layer::TimeInForce::ImmediateOrCancel,
                    created_at: Utc::now(),
                    status: OrderStatus::Pending,
                    filled_quantity: 0.0,
                    avg_fill_price: None,
                    source: OrderSource::Emergency,
                };
                
                match self.action_layer.submit_order(order).await {
                    Ok(order_id) => {
                        liquidation_orders.push(order_id);
                        tracing::info!("Emergency liquidation order submitted for {}: {}", symbol, order_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to submit emergency liquidation for {}: {}", symbol, e);
                    }
                }
            }
        }
        
        Ok(liquidation_orders)
    }
}

// Additional trait to allow downcasting for position tracker
trait AsAny {
    fn as_any(&self) -> &dyn std::any::Any;
}

impl AsAny for crate::action_layer::position_tracker::BasicPositionTracker {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AsAny for dyn crate::action_layer::PositionTracker {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Trading signal from neural network
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub action: SignalAction,
    pub confidence: f64,
    pub target_price: Option<f64>,
    pub order_type: crate::action_layer::OrderType,
    pub reasoning: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SignalAction {
    Buy,
    Sell,
}

impl std::fmt::Display for SignalAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalAction::Buy => write!(f, "BUY"),
            SignalAction::Sell => write!(f, "SELL"),
        }
    }
}