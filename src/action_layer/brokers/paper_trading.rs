//! Paper Trading Broker Implementation
//!
//! Simulates real trading without actual money for testing purposes

use crate::action_layer::{
    ActionLayerError, BrokerConfig, BrokerInterface, Order, OrderStatus, Position, 
    PositionSide, TradingAccount, OrderSide, OrderType, TimeInForce
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::{RwLock, Mutex};
use uuid::Uuid;

pub struct PaperTradingBroker {
    config: BrokerConfig,
    account: RwLock<PaperAccount>,
    orders: RwLock<HashMap<String, PaperOrder>>,
    positions: RwLock<HashMap<String, PaperPosition>>,
    market_data: RwLock<HashMap<String, MarketPrice>>,
    order_counter: Mutex<u64>,
}

#[derive(Debug, Clone)]
struct PaperAccount {
    initial_equity: f64,
    cash: f64,
    equity: f64,
    buying_power: f64,
    day_trading_buying_power: f64,
    unrealized_pnl: f64,
    realized_pnl: f64,
}

#[derive(Debug, Clone)]
struct PaperOrder {
    id: String,
    symbol: String,
    side: OrderSide,
    quantity: f64,
    order_type: OrderType,
    price: Option<f64>,
    time_in_force: TimeInForce,
    status: OrderStatus,
    created_at: DateTime<Utc>,
    filled_at: Option<DateTime<Utc>>,
    filled_price: Option<f64>,
    filled_quantity: f64,
}

#[derive(Debug, Clone)]
struct PaperPosition {
    symbol: String,
    quantity: f64,
    avg_entry_price: f64,
    side: PositionSide,
    unrealized_pnl: f64,
    realized_pnl: f64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct MarketPrice {
    symbol: String,
    price: f64,
    bid: f64,
    ask: f64,
    timestamp: DateTime<Utc>,
}

impl PaperTradingBroker {
    pub async fn new(config: &BrokerConfig) -> Result<Self, ActionLayerError> {
        let initial_equity = 100000.0; // Start with $100k paper money
        
        let account = PaperAccount {
            initial_equity,
            cash: initial_equity,
            equity: initial_equity,
            buying_power: initial_equity * 4.0, // 4:1 day trading buying power
            day_trading_buying_power: initial_equity * 4.0,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
        };
        
        Ok(Self {
            config: config.clone(),
            account: RwLock::new(account),
            orders: RwLock::new(HashMap::new()),
            positions: RwLock::new(HashMap::new()),
            market_data: RwLock::new(HashMap::new()),
            order_counter: Mutex::new(1),
        })
    }
    
    async fn get_next_order_id(&self) -> String {
        let mut counter = self.order_counter.lock().await;
        let id = format!("PAPER_{:08}", *counter);
        *counter += 1;
        id
    }
    
    /// Simulate market price (in real system, this would come from market data feed)
    async fn get_simulated_price(&self, symbol: &str) -> Result<MarketPrice, ActionLayerError> {
        let mut market_data = self.market_data.write().await;
        
        if let Some(existing_price) = market_data.get(symbol) {
            // Add some random movement to simulate market fluctuations
            let change_factor = 1.0 + (fastrand::f64() - 0.5) * 0.001; // ±0.05% random movement
            let new_price = existing_price.price * change_factor;
            let spread = new_price * 0.0001; // 0.01% spread
            
            let updated_price = MarketPrice {
                symbol: symbol.to_string(),
                price: new_price,
                bid: new_price - spread / 2.0,
                ask: new_price + spread / 2.0,
                timestamp: Utc::now(),
            };
            
            market_data.insert(symbol.to_string(), updated_price.clone());
            Ok(updated_price)
        } else {
            // Initialize with a base price for new symbols
            let base_price = match symbol {
                "AAPL" => 175.0,
                "MSFT" => 380.0,
                "GOOGL" => 140.0,
                "AMZN" => 145.0,
                "TSLA" => 180.0,
                "SPY" => 450.0,
                "QQQ" => 400.0,
                "XLF" => 38.0,
                "XLK" => 180.0,
                "XLY" => 155.0,
                _ => 100.0, // Default price for unknown symbols
            };
            
            let spread = base_price * 0.0001;
            let price = MarketPrice {
                symbol: symbol.to_string(),
                price: base_price,
                bid: base_price - spread / 2.0,
                ask: base_price + spread / 2.0,
                timestamp: Utc::now(),
            };
            
            market_data.insert(symbol.to_string(), price.clone());
            Ok(price)
        }
    }
    
    /// Simulate order execution
    async fn execute_paper_order(&self, order: &mut PaperOrder) -> Result<(), ActionLayerError> {
        let market_price = self.get_simulated_price(&order.symbol).await?;
        
        // Determine execution price
        let execution_price = match (&order.order_type, &order.side) {
            (OrderType::Market, OrderSide::Buy) => market_price.ask,
            (OrderType::Market, OrderSide::Sell) => market_price.bid,
            (OrderType::Limit, _) => {
                let limit_price = order.price.unwrap();
                match order.side {
                    OrderSide::Buy if limit_price >= market_price.ask => market_price.ask,
                    OrderSide::Sell if limit_price <= market_price.bid => market_price.bid,
                    _ => return Ok(()), // Limit order not executed
                }
            }
            (OrderType::StopLoss, OrderSide::Sell) => {
                let stop_price = order.price.unwrap();
                if market_price.bid <= stop_price {
                    market_price.bid
                } else {
                    return Ok(()); // Stop not triggered
                }
            }
        };
        
        // Update order
        order.status = OrderStatus::Filled;
        order.filled_at = Some(Utc::now());
        order.filled_price = Some(execution_price);
        order.filled_quantity = order.quantity;
        
        // Update positions
        self.update_paper_position(&order.symbol, &order.side, order.quantity, execution_price).await?;
        
        Ok(())
    }
    
    /// Update paper trading position
    async fn update_paper_position(
        &self, 
        symbol: &str, 
        side: &OrderSide, 
        quantity: f64, 
        price: f64
    ) -> Result<(), ActionLayerError> {
        let mut positions = self.positions.write().await;
        
        if let Some(existing_pos) = positions.get_mut(symbol) {
            match (&existing_pos.side, side) {
                // Increasing long position
                (PositionSide::Long, OrderSide::Buy) => {
                    let total_cost = (existing_pos.quantity * existing_pos.avg_entry_price) + (quantity * price);
                    let total_quantity = existing_pos.quantity + quantity;
                    existing_pos.avg_entry_price = total_cost / total_quantity;
                    existing_pos.quantity = total_quantity;
                }
                // Reducing/closing long position
                (PositionSide::Long, OrderSide::Sell) => {
                    let realized_pnl = (price - existing_pos.avg_entry_price) * quantity.min(existing_pos.quantity);
                    existing_pos.realized_pnl += realized_pnl;
                    existing_pos.quantity -= quantity.min(existing_pos.quantity);
                    
                    if existing_pos.quantity <= 0.0 {
                        // Position closed, remove if quantity is zero
                        if existing_pos.quantity == 0.0 {
                            positions.remove(symbol);
                            return Ok(());
                        } else {
                            // Position reversed to short
                            existing_pos.side = PositionSide::Short;
                            existing_pos.quantity = -existing_pos.quantity;
                            existing_pos.avg_entry_price = price;
                        }
                    }
                }
                // Similar logic for short positions would go here
                _ => {
                    // For MVP, we'll handle simple long-only positions
                    return Err(ActionLayerError::Position("Complex position management not implemented in MVP".to_string()));
                }
            }
            existing_pos.updated_at = Utc::now();
        } else {
            // Create new position
            let new_position = PaperPosition {
                symbol: symbol.to_string(),
                quantity,
                avg_entry_price: price,
                side: match side {
                    OrderSide::Buy => PositionSide::Long,
                    OrderSide::Sell => PositionSide::Short,
                },
                unrealized_pnl: 0.0,
                realized_pnl: 0.0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            positions.insert(symbol.to_string(), new_position);
        }
        
        Ok(())
    }
    
    /// Update unrealized P&L for all positions
    async fn update_unrealized_pnl(&self) -> Result<(), ActionLayerError> {
        let mut positions = self.positions.write().await;
        
        for (symbol, position) in positions.iter_mut() {
            let market_price = self.get_simulated_price(symbol).await?;
            
            position.unrealized_pnl = match position.side {
                PositionSide::Long => (market_price.price - position.avg_entry_price) * position.quantity,
                PositionSide::Short => (position.avg_entry_price - market_price.price) * position.quantity,
                PositionSide::Flat => 0.0,
            };
        }
        
        Ok(())
    }
    
    /// Update account equity based on positions
    async fn update_account_equity(&self) -> Result<(), ActionLayerError> {
        self.update_unrealized_pnl().await?;
        
        let positions = self.positions.read().await;
        let mut account = self.account.write().await;
        
        let total_unrealized_pnl: f64 = positions.values().map(|p| p.unrealized_pnl).sum();
        let total_realized_pnl: f64 = positions.values().map(|p| p.realized_pnl).sum();
        
        account.unrealized_pnl = total_unrealized_pnl;
        account.realized_pnl = total_realized_pnl;
        account.equity = account.initial_equity + total_unrealized_pnl + total_realized_pnl;
        
        // Update buying power (simplified)
        let position_value: f64 = positions.values()
            .map(|p| p.quantity * p.avg_entry_price)
            .sum();
        account.cash = account.initial_equity - position_value + total_realized_pnl;
        account.buying_power = account.cash * 4.0; // 4:1 leverage
        account.day_trading_buying_power = account.cash * 4.0;
        
        Ok(())
    }
}

#[async_trait]
impl BrokerInterface for PaperTradingBroker {
    async fn submit_order(&self, order: &Order) -> Result<String, ActionLayerError> {
        let order_id = self.get_next_order_id().await;
        
        let mut paper_order = PaperOrder {
            id: order_id.clone(),
            symbol: order.symbol.clone(),
            side: order.side.clone(),
            quantity: order.quantity,
            order_type: order.order_type.clone(),
            price: order.price,
            time_in_force: order.time_in_force.clone(),
            status: OrderStatus::Submitted,
            created_at: Utc::now(),
            filled_at: None,
            filled_price: None,
            filled_quantity: 0.0,
        };
        
        // Simulate immediate execution for market orders, delay for limit orders
        if matches!(order.order_type, OrderType::Market) {
            self.execute_paper_order(&mut paper_order).await?;
        }
        
        // Store order
        self.orders.write().await.insert(order_id.clone(), paper_order);
        
        // Update account
        self.update_account_equity().await?;
        
        Ok(order_id)
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), ActionLayerError> {
        let mut orders = self.orders.write().await;
        
        if let Some(order) = orders.get_mut(order_id) {
            if matches!(order.status, OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Rejected) {
                return Err(ActionLayerError::OrderExecution(
                    "Cannot cancel completed order".to_string()
                ));
            }
            order.status = OrderStatus::Cancelled;
            Ok(())
        } else {
            Err(ActionLayerError::Position("Order not found".to_string()))
        }
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus, ActionLayerError> {
        let orders = self.orders.read().await;
        
        orders.get(order_id)
            .map(|order| order.status.clone())
            .ok_or_else(|| ActionLayerError::Position("Order not found".to_string()))
    }
    
    async fn get_account_info(&self) -> Result<TradingAccount, ActionLayerError> {
        self.update_account_equity().await?;
        
        let account = self.account.read().await;
        let positions = self.get_positions().await?;
        
        Ok(TradingAccount {
            equity: account.equity,
            buying_power: account.buying_power,
            cash: account.cash,
            day_trading_buying_power: account.day_trading_buying_power,
            portfolio_value: account.equity,
            positions,
            daily_pnl: account.unrealized_pnl + account.realized_pnl, // Simplified
            unrealized_pnl: account.unrealized_pnl,
            realized_pnl: account.realized_pnl,
        })
    }
    
    async fn get_positions(&self) -> Result<HashMap<String, Position>, ActionLayerError> {
        self.update_unrealized_pnl().await?;
        let positions = self.positions.read().await;
        
        let result = positions.iter()
            .filter(|(_, pos)| pos.quantity > 0.0)
            .map(|(symbol, paper_pos)| {
                let position = Position {
                    symbol: symbol.clone(),
                    quantity: paper_pos.quantity,
                    avg_entry_price: paper_pos.avg_entry_price,
                    current_price: 0.0, // Will be updated with market price
                    unrealized_pnl: paper_pos.unrealized_pnl,
                    realized_pnl: paper_pos.realized_pnl,
                    side: paper_pos.side.clone(),
                    created_at: paper_pos.created_at,
                    updated_at: paper_pos.updated_at,
                };
                (symbol.clone(), position)
            })
            .collect();
        
        Ok(result)
    }
    
    async fn get_current_price(&self, symbol: &str) -> Result<f64, ActionLayerError> {
        let market_price = self.get_simulated_price(symbol).await?;
        Ok(market_price.price)
    }
}