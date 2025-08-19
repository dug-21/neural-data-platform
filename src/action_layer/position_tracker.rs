//! Position Tracking and P&L Calculation
//!
//! Provides real-time position management and profit/loss tracking

use crate::action_layer::{
    ActionLayerError, OrderFill, OrderSide, Position, PositionSide, PositionTracker
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct BasicPositionTracker {
    positions: RwLock<HashMap<String, Position>>,
    trade_history: RwLock<Vec<TradeRecord>>,
}

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub price: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub commission: f64,
    pub realized_pnl: f64,
}

impl BasicPositionTracker {
    pub fn new() -> Self {
        Self {
            positions: RwLock::new(HashMap::new()),
            trade_history: RwLock::new(Vec::new()),
        }
    }
    
    /// Calculate realized P&L when closing a position
    fn calculate_realized_pnl(&self, existing_pos: &Position, fill: &OrderFill) -> f64 {
        let fill_quantity = fill.quantity.min(existing_pos.quantity);
        
        match (&existing_pos.side, &fill.side) {
            (PositionSide::Long, OrderSide::Sell) => {
                // Selling long position
                (fill.price - existing_pos.avg_entry_price) * fill_quantity - fill.commission
            }
            (PositionSide::Short, OrderSide::Buy) => {
                // Covering short position
                (existing_pos.avg_entry_price - fill.price) * fill_quantity - fill.commission
            }
            _ => 0.0, // No realized P&L for position increases
        }
    }
    
    /// Update position with new fill
    fn update_position_with_fill(&self, existing_pos: &mut Position, fill: &OrderFill) -> f64 {
        let realized_pnl = match (&existing_pos.side, &fill.side) {
            // Closing or reducing position
            (PositionSide::Long, OrderSide::Sell) | (PositionSide::Short, OrderSide::Buy) => {
                let realized = self.calculate_realized_pnl(existing_pos, fill);
                existing_pos.realized_pnl += realized;
                
                if fill.quantity >= existing_pos.quantity {
                    // Position fully closed or reversed
                    let remaining_quantity = fill.quantity - existing_pos.quantity;
                    if remaining_quantity > 0.0 {
                        // Position reversed
                        existing_pos.side = match fill.side {
                            OrderSide::Buy => PositionSide::Long,
                            OrderSide::Sell => PositionSide::Short,
                        };
                        existing_pos.quantity = remaining_quantity;
                        existing_pos.avg_entry_price = fill.price;
                    } else {
                        // Position fully closed
                        existing_pos.side = PositionSide::Flat;
                        existing_pos.quantity = 0.0;
                    }
                } else {
                    // Position reduced
                    existing_pos.quantity -= fill.quantity;
                }
                
                realized
            }
            // Increasing position
            (PositionSide::Long, OrderSide::Buy) | (PositionSide::Short, OrderSide::Sell) => {
                // Calculate new average entry price
                let total_cost = (existing_pos.quantity * existing_pos.avg_entry_price) + 
                                (fill.quantity * fill.price);
                let total_quantity = existing_pos.quantity + fill.quantity;
                
                existing_pos.avg_entry_price = total_cost / total_quantity;
                existing_pos.quantity = total_quantity;
                
                0.0 // No realized P&L
            }
            _ => 0.0,
        };
        
        existing_pos.updated_at = Utc::now();
        realized_pnl
    }
    
    /// Create new position from fill
    fn create_position_from_fill(&self, fill: &OrderFill) -> Position {
        Position {
            symbol: fill.symbol.clone(),
            quantity: fill.quantity,
            avg_entry_price: fill.price,
            current_price: fill.price,
            unrealized_pnl: 0.0,
            realized_pnl: -fill.commission, // Start with commission cost
            side: match fill.side {
                OrderSide::Buy => PositionSide::Long,
                OrderSide::Sell => PositionSide::Short,
            },
            created_at: fill.timestamp,
            updated_at: fill.timestamp,
        }
    }
    
    /// Update unrealized P&L for all positions
    pub async fn update_unrealized_pnl(&self, market_prices: HashMap<String, f64>) -> Result<(), ActionLayerError> {
        let mut positions = self.positions.write().await;
        
        for (symbol, position) in positions.iter_mut() {
            if let Some(&current_price) = market_prices.get(symbol) {
                position.current_price = current_price;
                
                position.unrealized_pnl = match position.side {
                    PositionSide::Long => {
                        (current_price - position.avg_entry_price) * position.quantity
                    }
                    PositionSide::Short => {
                        (position.avg_entry_price - current_price) * position.quantity
                    }
                    PositionSide::Flat => 0.0,
                };
            }
        }
        
        Ok(())
    }
    
    /// Get position summary
    pub async fn get_position_summary(&self) -> Result<PositionSummary, ActionLayerError> {
        let positions = self.positions.read().await;
        let trade_history = self.trade_history.read().await;
        
        let total_unrealized_pnl: f64 = positions.values()
            .map(|p| p.unrealized_pnl)
            .sum();
        
        let total_realized_pnl: f64 = positions.values()
            .map(|p| p.realized_pnl)
            .sum();
        
        let total_positions = positions.len();
        let long_positions = positions.values()
            .filter(|p| matches!(p.side, PositionSide::Long))
            .count();
        let short_positions = positions.values()
            .filter(|p| matches!(p.side, PositionSide::Short))
            .count();
        
        let total_exposure: f64 = positions.values()
            .map(|p| p.quantity * p.current_price)
            .sum();
        
        Ok(PositionSummary {
            total_positions,
            long_positions,
            short_positions,
            total_unrealized_pnl,
            total_realized_pnl,
            total_pnl: total_unrealized_pnl + total_realized_pnl,
            total_exposure,
            largest_position: positions.values()
                .max_by(|a, b| (a.quantity * a.current_price).partial_cmp(&(b.quantity * b.current_price)).unwrap())
                .map(|p| p.symbol.clone()),
            total_trades: trade_history.len(),
        })
    }
    
    /// Get daily P&L
    pub async fn get_daily_pnl(&self) -> Result<f64, ActionLayerError> {
        let trade_history = self.trade_history.read().await;
        let positions = self.positions.read().await;
        
        let today = Utc::now().date_naive();
        
        // Realized P&L from today's trades
        let daily_realized: f64 = trade_history.iter()
            .filter(|trade| trade.timestamp.date_naive() == today)
            .map(|trade| trade.realized_pnl)
            .sum();
        
        // Unrealized P&L from current positions
        let daily_unrealized: f64 = positions.values()
            .map(|p| p.unrealized_pnl)
            .sum();
        
        Ok(daily_realized + daily_unrealized)
    }
    
    /// Clean up closed positions
    pub async fn cleanup_closed_positions(&self) -> Result<(), ActionLayerError> {
        let mut positions = self.positions.write().await;
        positions.retain(|_, pos| !matches!(pos.side, PositionSide::Flat) && pos.quantity > 0.0);
        Ok(())
    }
}

#[async_trait]
impl PositionTracker for BasicPositionTracker {
    async fn update_position(&self, symbol: &str, fill: &OrderFill) -> Result<(), ActionLayerError> {
        let mut positions = self.positions.write().await;
        let mut trade_history = self.trade_history.write().await;
        
        let realized_pnl = if let Some(existing_pos) = positions.get_mut(symbol) {
            self.update_position_with_fill(existing_pos, fill)
        } else {
            // Create new position
            let new_position = self.create_position_from_fill(fill);
            positions.insert(symbol.to_string(), new_position);
            -fill.commission // Initial commission cost
        };
        
        // Record trade
        let trade_record = TradeRecord {
            symbol: fill.symbol.clone(),
            side: fill.side.clone(),
            quantity: fill.quantity,
            price: fill.price,
            timestamp: fill.timestamp,
            commission: fill.commission,
            realized_pnl,
        };
        
        trade_history.push(trade_record);
        
        Ok(())
    }
    
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>, ActionLayerError> {
        let positions = self.positions.read().await;
        Ok(positions.get(symbol).cloned())
    }
    
    async fn get_all_positions(&self) -> Result<HashMap<String, Position>, ActionLayerError> {
        let positions = self.positions.read().await;
        Ok(positions.clone())
    }
    
    async fn calculate_pnl(&self, symbol: &str, current_price: f64) -> Result<f64, ActionLayerError> {
        let positions = self.positions.read().await;
        
        if let Some(position) = positions.get(symbol) {
            let unrealized_pnl = match position.side {
                PositionSide::Long => (current_price - position.avg_entry_price) * position.quantity,
                PositionSide::Short => (position.avg_entry_price - current_price) * position.quantity,
                PositionSide::Flat => 0.0,
            };
            Ok(position.realized_pnl + unrealized_pnl)
        } else {
            Ok(0.0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct PositionSummary {
    pub total_positions: usize,
    pub long_positions: usize,
    pub short_positions: usize,
    pub total_unrealized_pnl: f64,
    pub total_realized_pnl: f64,
    pub total_pnl: f64,
    pub total_exposure: f64,
    pub largest_position: Option<String>,
    pub total_trades: usize,
}