use crate::error::{Error, Result};
use crate::models::{TradingSignal, Order, Portfolio, Position};
use reqwest::{Client, StatusCode};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::time::Duration;
use chrono::{DateTime, Utc};
use tracing::{info, error, debug};

#[derive(Clone)]
pub struct AgentClient {
    client: Client,
    base_url: String,
}

impl AgentClient {
    pub async fn new(base_url: &str) -> Result<Self> {
        info!("Initializing trading agent client...");
        
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Http(e))?;
        
        // Test connection
        let health_url = format!("{}/health", base_url);
        match client.get(&health_url).send().await {
            Ok(response) if response.status() == StatusCode::OK => {
                info!("Agent service connection established");
            }
            Ok(response) => {
                error!("Agent service returned status: {}", response.status());
                return Err(Error::ServiceUnavailable(format!("Agent service returned status: {}", response.status())));
            }
            Err(e) => {
                error!("Failed to connect to agent service: {}", e);
                return Err(Error::ServiceUnavailable(format!("Agent service unavailable: {}", e)));
            }
        }
        
        Ok(Self {
            client,
            base_url: base_url.to_string(),
        })
    }
    
    pub async fn get_trading_signal(&self, symbol: &str) -> Result<TradingSignal> {
        debug!("Requesting trading signal for {}", symbol);
        
        let url = format!("{}/signal/{}", self.base_url, symbol);
        
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Agent service error {}: {}", status, error_text)));
        }
        
        let signal: TradingSignal = response.json().await
            .map_err(|e| Error::Serialization(e))?;
        
        Ok(signal)
    }
    
    pub async fn execute_trade(
        &self,
        symbol: &str,
        side: &str,
        quantity: f64,
        price: Option<f64>,
        take_profit: Option<f64>,
        stop_loss: Option<f64>,
    ) -> Result<Order> {
        debug!("Executing {} order for {} ({} units)", side, symbol, quantity);
        
        let url = format!("{}/orders", self.base_url);
        let order_type = if price.is_some() { "limit" } else { "market" };
        
        let request = json!({
            "symbol": symbol,
            "side": side,
            "quantity": quantity,
            "order_type": order_type,
            "price": price,
            "take_profit": take_profit,
            "stop_loss": stop_loss,
        });
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Agent service error {}: {}", status, error_text)));
        }
        
        let order: Order = response.json().await
            .map_err(|e| Error::Serialization(e))?;
        
        Ok(order)
    }
    
    pub async fn get_portfolio(&self) -> Result<Portfolio> {
        debug!("Requesting portfolio status");
        
        let url = format!("{}/portfolio", self.base_url);
        
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Agent service error {}: {}", status, error_text)));
        }
        
        let portfolio: Portfolio = response.json().await
            .map_err(|e| Error::Serialization(e))?;
        
        Ok(portfolio)
    }
    
    pub async fn get_active_orders(&self) -> Result<Vec<Order>> {
        debug!("Requesting active orders");
        
        let url = format!("{}/orders/active", self.base_url);
        
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Agent service error {}: {}", status, error_text)));
        }
        
        let orders: Vec<Order> = response.json().await
            .map_err(|e| Error::Serialization(e))?;
        
        Ok(orders)
    }
    
    pub async fn cancel_order(&self, order_id: &str) -> Result<Order> {
        debug!("Cancelling order {}", order_id);
        
        let url = format!("{}/orders/{}/cancel", self.base_url, order_id);
        
        let response = self.client.post(&url)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Agent service error {}: {}", status, error_text)));
        }
        
        let order: Order = response.json().await
            .map_err(|e| Error::Serialization(e))?;
        
        Ok(order)
    }
    
    pub async fn get_strategy(&self, symbol: &str) -> Result<TradingStrategy> {
        debug!("Requesting trading strategy for {}", symbol);
        
        let url = format!("{}/strategy/{}", self.base_url, symbol);
        
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Agent service error {}: {}", status, error_text)));
        }
        
        let strategy: TradingStrategy = response.json().await
            .map_err(|e| Error::Serialization(e))?;
        
        Ok(strategy)
    }
    
    pub async fn update_strategy(&self, symbol: &str, parameters: StrategyParameters) -> Result<TradingStrategy> {
        debug!("Updating trading strategy for {}", symbol);
        
        let url = format!("{}/strategy/{}", self.base_url, symbol);
        
        let response = self.client.put(&url)
            .json(&parameters)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Agent service error {}: {}", status, error_text)));
        }
        
        let strategy: TradingStrategy = response.json().await
            .map_err(|e| Error::Serialization(e))?;
        
        Ok(strategy)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingStrategy {
    pub strategy_name: String,
    pub symbol: String,
    pub parameters: StrategyParameters,
    pub risk_parameters: RiskParameters,
    pub performance_metrics: PerformanceMetrics,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyParameters {
    pub entry_conditions: Vec<String>,
    pub exit_conditions: Vec<String>,
    pub position_sizing: PositionSizing,
    pub timeframe: String,
    pub indicators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskParameters {
    pub max_position_size: f64,
    pub max_drawdown: f64,
    pub risk_per_trade: f64,
    pub stop_loss_percentage: f64,
    pub take_profit_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSizing {
    pub method: String, // fixed, kelly, volatility_based
    pub base_size: f64,
    pub scaling_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub win_rate: f64,
    pub average_win: f64,
    pub average_loss: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub total_return: f64,
}