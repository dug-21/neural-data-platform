use crate::error::{Error, Result};
use crate::models::{PricePrediction, TrendAnalysis, ChartPattern};
use reqwest::{Client, StatusCode};
use serde::{Serialize, Deserialize};
use serde_json::json;
use std::time::Duration;
use chrono::{DateTime, Utc};
use tracing::{info, error, debug};

#[derive(Debug, Clone)]
pub struct NeuralClient {
    client: Client,
    base_url: String,
}

impl NeuralClient {
    pub async fn new(base_url: &str) -> Result<Self> {
        info!("Initializing neural network client...");
        
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Http(e))?;
        
        // Test connection
        let health_url = format!("{}/health", base_url);
        match client.get(&health_url).send().await {
            Ok(response) if response.status() == StatusCode::OK => {
                info!("Neural service connection established");
            }
            Ok(response) => {
                error!("Neural service returned status: {}", response.status());
                return Err(Error::ServiceUnavailable(format!("Neural service returned status: {}", response.status())));
            }
            Err(e) => {
                error!("Failed to connect to neural service: {}", e);
                return Err(Error::ServiceUnavailable(format!("Neural service unavailable: {}", e)));
            }
        }
        
        Ok(Self {
            client,
            base_url: base_url.to_string(),
        })
    }
    
    pub async fn predict_price(
        &self,
        symbol: &str,
        timeframe: &str,
        periods: usize,
    ) -> Result<PricePrediction> {
        debug!("Requesting price prediction for {} ({}, {} periods)", symbol, timeframe, periods);
        
        let url = format!("{}/predict/price", self.base_url);
        let request = json!({
            "symbol": symbol,
            "timeframe": timeframe,
            "periods": periods,
        });
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Neural service error {}: {}", status, error_text)));
        }
        
        let prediction: PricePrediction = response.json().await
            .map_err(|e| Error::Http(e))?;
        
        Ok(prediction)
    }
    
    pub async fn analyze_trend(&self, symbol: &str) -> Result<TrendAnalysis> {
        debug!("Requesting trend analysis for {}", symbol);
        
        let url = format!("{}/analyze/trend", self.base_url);
        let request = json!({
            "symbol": symbol,
        });
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Neural service error {}: {}", status, error_text)));
        }
        
        let analysis: TrendAnalysis = response.json().await
            .map_err(|e| Error::Http(e))?;
        
        Ok(analysis)
    }
    
    pub async fn recognize_patterns(
        &self,
        symbol: &str,
        timeframe: &str,
    ) -> Result<Vec<ChartPattern>> {
        debug!("Requesting pattern recognition for {} ({})", symbol, timeframe);
        
        let url = format!("{}/analyze/patterns", self.base_url);
        let request = json!({
            "symbol": symbol,
            "timeframe": timeframe,
        });
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Neural service error {}: {}", status, error_text)));
        }
        
        let patterns: Vec<ChartPattern> = response.json().await
            .map_err(|e| Error::Http(e))?;
        
        Ok(patterns)
    }
    
    pub async fn assess_risk(
        &self,
        symbol: &str,
        position_size: f64,
        position_type: &str,
    ) -> Result<RiskAssessment> {
        debug!("Requesting risk assessment for {} ({} {})", symbol, position_size, position_type);
        
        let url = format!("{}/analyze/risk", self.base_url);
        let request = json!({
            "symbol": symbol,
            "position_size": position_size,
            "position_type": position_type,
        });
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Neural service error {}: {}", status, error_text)));
        }
        
        let risk: RiskAssessment = response.json().await
            .map_err(|e| Error::Http(e))?;
        
        Ok(risk)
    }
    
    pub async fn get_model_info(&self) -> Result<ModelInfo> {
        debug!("Requesting neural model information");
        
        let url = format!("{}/model/info", self.base_url);
        
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| Error::Http(e))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::ServiceUnavailable(format!("Neural service error {}: {}", status, error_text)));
        }
        
        let info: ModelInfo = response.json().await
            .map_err(|e| Error::Http(e))?;
        
        Ok(info)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub symbol: String,
    pub position_size: f64,
    pub position_type: String,
    pub risk_score: f64, // 0-100
    pub stop_loss: f64,
    pub take_profit: f64,
    pub max_drawdown: f64,
    pub probability_of_profit: f64,
    pub expected_return: f64,
    pub risk_reward_ratio: f64,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub version: String,
    pub architecture: String,
    pub training_date: DateTime<Utc>,
    pub accuracy_metrics: AccuracyMetrics,
    pub supported_symbols: Vec<String>,
    pub supported_timeframes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyMetrics {
    pub directional_accuracy: f64,
    pub price_mae: f64,
    pub price_rmse: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
}