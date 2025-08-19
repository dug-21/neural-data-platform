//! Alpaca broker integration
//!
//! Provides both paper trading and live trading implementations for Alpaca

use crate::action_layer::{
    ActionLayerError, BrokerConfig, BrokerInterface, Order, OrderStatus, Position, 
    PositionSide, TradingAccount, OrderSide, OrderType, TimeInForce
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct AlpacaAccount {
    equity: String,
    cash: String,
    buying_power: String,
    day_trading_buying_power: String,
    portfolio_value: String,
}

#[derive(Debug, Deserialize)]
struct AlpacaPosition {
    symbol: String,
    qty: String,
    avg_entry_price: String,
    current_price: Option<String>,
    market_value: String,
    unrealized_pl: String,
    side: String,
}

#[derive(Debug, Serialize)]
struct AlpacaOrderRequest {
    symbol: String,
    qty: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    time_in_force: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_price: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AlpacaOrderResponse {
    id: String,
    status: String,
    symbol: String,
    qty: String,
    filled_qty: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct AlpacaQuote {
    symbol: String,
    #[serde(rename = "c")]
    price: f64,
}

/// Alpaca Paper Trading Broker
pub struct AlpacaPaperBroker {
    client: Client,
    base_url: String,
    api_key: String,
    secret_key: String,
}

impl AlpacaPaperBroker {
    pub async fn new(config: &BrokerConfig) -> Result<Self, ActionLayerError> {
        let client = Client::new();
        
        // Validate credentials by making a test call
        let broker = Self {
            client,
            base_url: config.base_url.clone(),
            api_key: config.api_key.clone(),
            secret_key: config.secret_key.clone(),
        };
        
        // Test connection
        broker.test_connection().await?;
        
        Ok(broker)
    }
    
    async fn test_connection(&self) -> Result<(), ActionLayerError> {
        let response = self.client
            .get(&format!("{}/v2/account", self.base_url))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .send()
            .await
            .map_err(|e| ActionLayerError::Broker(format!("Connection test failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ActionLayerError::Broker(format!(
                "Authentication failed: {}", 
                response.status()
            )));
        }
        
        Ok(())
    }
    
    fn map_order_side(&self, side: &OrderSide) -> &'static str {
        match side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
    }
    
    fn map_order_type(&self, order_type: &OrderType) -> &'static str {
        match order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::StopLoss => "stop",
        }
    }
    
    fn map_time_in_force(&self, tif: &TimeInForce) -> &'static str {
        match tif {
            TimeInForce::Day => "day",
            TimeInForce::GoodTilCancelled => "gtc",
            TimeInForce::ImmediateOrCancel => "ioc",
            TimeInForce::FillOrKill => "fok",
        }
    }
    
    fn map_order_status(&self, status: &str) -> OrderStatus {
        match status.to_lowercase().as_str() {
            "new" | "accepted" => OrderStatus::Submitted,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "cancelled" => OrderStatus::Cancelled,
            "rejected" => OrderStatus::Rejected,
            "expired" => OrderStatus::Expired,
            _ => OrderStatus::Pending,
        }
    }
}

#[async_trait]
impl BrokerInterface for AlpacaPaperBroker {
    async fn submit_order(&self, order: &Order) -> Result<String, ActionLayerError> {
        let request = AlpacaOrderRequest {
            symbol: order.symbol.clone(),
            qty: order.quantity.to_string(),
            side: self.map_order_side(&order.side).to_string(),
            order_type: self.map_order_type(&order.order_type).to_string(),
            time_in_force: self.map_time_in_force(&order.time_in_force).to_string(),
            limit_price: order.price.map(|p| p.to_string()),
            stop_price: None, // TODO: Add stop price support
        };
        
        let response = self.client
            .post(&format!("{}/v2/orders", self.base_url))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ActionLayerError::OrderExecution(format!("Order submission failed: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ActionLayerError::OrderExecution(format!(
                "Order rejected: {} - {}", 
                response.status(),
                error_text
            )));
        }
        
        let order_response: AlpacaOrderResponse = response.json().await
            .map_err(|e| ActionLayerError::OrderExecution(format!("Failed to parse response: {}", e)))?;
        
        Ok(order_response.id)
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), ActionLayerError> {
        let response = self.client
            .delete(&format!("{}/v2/orders/{}", self.base_url, order_id))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .send()
            .await
            .map_err(|e| ActionLayerError::OrderExecution(format!("Order cancellation failed: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ActionLayerError::OrderExecution(format!(
                "Cancel failed: {} - {}", 
                response.status(),
                error_text
            )));
        }
        
        Ok(())
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus, ActionLayerError> {
        let response = self.client
            .get(&format!("{}/v2/orders/{}", self.base_url, order_id))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .send()
            .await
            .map_err(|e| ActionLayerError::Broker(format!("Get order status failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ActionLayerError::Broker(format!(
                "Get order status failed: {}", 
                response.status()
            )));
        }
        
        let order: AlpacaOrderResponse = response.json().await
            .map_err(|e| ActionLayerError::Broker(format!("Failed to parse order: {}", e)))?;
        
        Ok(self.map_order_status(&order.status))
    }
    
    async fn get_account_info(&self) -> Result<TradingAccount, ActionLayerError> {
        let response = self.client
            .get(&format!("{}/v2/account", self.base_url))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .send()
            .await
            .map_err(|e| ActionLayerError::Broker(format!("Get account failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ActionLayerError::Broker(format!(
                "Get account failed: {}", 
                response.status()
            )));
        }
        
        let account: AlpacaAccount = response.json().await
            .map_err(|e| ActionLayerError::Broker(format!("Failed to parse account: {}", e)))?;
        
        let positions = self.get_positions().await?;
        let (unrealized_pnl, realized_pnl) = positions.values()
            .fold((0.0, 0.0), |acc, pos| (acc.0 + pos.unrealized_pnl, acc.1 + pos.realized_pnl));
        
        Ok(TradingAccount {
            equity: account.equity.parse().unwrap_or(0.0),
            buying_power: account.buying_power.parse().unwrap_or(0.0),
            cash: account.cash.parse().unwrap_or(0.0),
            day_trading_buying_power: account.day_trading_buying_power.parse().unwrap_or(0.0),
            portfolio_value: account.portfolio_value.parse().unwrap_or(0.0),
            positions,
            daily_pnl: unrealized_pnl, // Simplified
            unrealized_pnl,
            realized_pnl,
        })
    }
    
    async fn get_positions(&self) -> Result<HashMap<String, Position>, ActionLayerError> {
        let response = self.client
            .get(&format!("{}/v2/positions", self.base_url))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .send()
            .await
            .map_err(|e| ActionLayerError::Broker(format!("Get positions failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ActionLayerError::Broker(format!(
                "Get positions failed: {}", 
                response.status()
            )));
        }
        
        let alpaca_positions: Vec<AlpacaPosition> = response.json().await
            .map_err(|e| ActionLayerError::Broker(format!("Failed to parse positions: {}", e)))?;
        
        let mut positions = HashMap::new();
        
        for alpaca_pos in alpaca_positions {
            let quantity: f64 = alpaca_pos.qty.parse().unwrap_or(0.0);
            if quantity == 0.0 {
                continue;
            }
            
            let position = Position {
                symbol: alpaca_pos.symbol.clone(),
                quantity: quantity.abs(),
                avg_entry_price: alpaca_pos.avg_entry_price.parse().unwrap_or(0.0),
                current_price: alpaca_pos.current_price
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(0.0),
                unrealized_pnl: alpaca_pos.unrealized_pl.parse().unwrap_or(0.0),
                realized_pnl: 0.0, // TODO: Get from trade history
                side: if quantity > 0.0 { PositionSide::Long } else { PositionSide::Short },
                created_at: Utc::now(), // TODO: Get actual creation time
                updated_at: Utc::now(),
            };
            
            positions.insert(alpaca_pos.symbol, position);
        }
        
        Ok(positions)
    }
    
    async fn get_current_price(&self, symbol: &str) -> Result<f64, ActionLayerError> {
        let response = self.client
            .get(&format!("{}/v2/stocks/{}/quotes/latest", self.base_url, symbol))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .send()
            .await
            .map_err(|e| ActionLayerError::Broker(format!("Get price failed: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ActionLayerError::Broker(format!(
                "Get price failed: {}", 
                response.status()
            )));
        }
        
        #[derive(Deserialize)]
        struct QuoteResponse {
            quote: Quote,
        }
        
        #[derive(Deserialize)]
        struct Quote {
            #[serde(rename = "ap")]
            ask_price: f64,
            #[serde(rename = "bp")]
            bid_price: f64,
        }
        
        let quote_response: QuoteResponse = response.json().await
            .map_err(|e| ActionLayerError::Broker(format!("Failed to parse quote: {}", e)))?;
        
        // Return mid-price
        Ok((quote_response.quote.ask_price + quote_response.quote.bid_price) / 2.0)
    }
}

/// Alpaca Live Trading Broker
pub struct AlpacaLiveBroker {
    inner: AlpacaPaperBroker,
}

impl AlpacaLiveBroker {
    pub async fn new(config: &BrokerConfig) -> Result<Self, ActionLayerError> {
        // Use live URL for live trading
        let mut live_config = config.clone();
        live_config.base_url = "https://api.alpaca.markets".to_string();
        
        let inner = AlpacaPaperBroker::new(&live_config).await?;
        Ok(Self { inner })
    }
}

#[async_trait]
impl BrokerInterface for AlpacaLiveBroker {
    async fn submit_order(&self, order: &Order) -> Result<String, ActionLayerError> {
        self.inner.submit_order(order).await
    }
    
    async fn cancel_order(&self, order_id: &str) -> Result<(), ActionLayerError> {
        self.inner.cancel_order(order_id).await
    }
    
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus, ActionLayerError> {
        self.inner.get_order_status(order_id).await
    }
    
    async fn get_account_info(&self) -> Result<TradingAccount, ActionLayerError> {
        self.inner.get_account_info().await
    }
    
    async fn get_positions(&self) -> Result<HashMap<String, Position>, ActionLayerError> {
        self.inner.get_positions().await
    }
    
    async fn get_current_price(&self, symbol: &str) -> Result<f64, ActionLayerError> {
        self.inner.get_current_price(symbol).await
    }
}