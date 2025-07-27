use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::Result;
use crate::integrations::database::DatabaseClient;
use mcp_sdk::tools::Tool;
use mcp_sdk::types::{CallToolResponse, ToolResponseContent};
use anyhow::Result as AnyhowResult;

#[derive(Debug, Clone)]
pub struct MarketDataTool {
    db_client: Arc<DatabaseClient>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketDataRequest {
    pub symbol: String,
    #[serde(default = "default_timeframe")]
    pub timeframe: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_timeframe() -> String {
    "1h".to_string()
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketDataResponse {
    pub symbol: String,
    pub timeframe: String,
    pub data: Vec<ToolPriceData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolPriceData {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl MarketDataTool {
    pub fn new(db_client: Arc<DatabaseClient>) -> Self {
        Self { db_client }
    }

    pub async fn execute(&self, request: MarketDataRequest) -> Result<MarketDataResponse> {
        info!("Fetching market data for {} ({})", request.symbol, request.timeframe);
        
        let prices = self.db_client
            .get_latest_prices(&request.symbol, request.limit)
            .await?;
            
        let data = prices.into_iter()
            .map(|price| ToolPriceData {
                timestamp: price.timestamp,
                open: price.open.unwrap_or(price.price),
                high: price.high.unwrap_or(price.price),
                low: price.low.unwrap_or(price.price),
                close: price.close.unwrap_or(price.price),
                volume: price.volume.unwrap_or(0.0),
            })
            .collect();
            
        Ok(MarketDataResponse {
            symbol: request.symbol,
            timeframe: request.timeframe,
            data,
        })
    }
}

impl Tool for MarketDataTool {
    fn name(&self) -> String {
        "get_market_data".to_string()
    }

    fn description(&self) -> String {
        "Get market data for a trading symbol".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Trading symbol (e.g., BTC/USD)"
                },
                "timeframe": {
                    "type": "string",
                    "description": "Timeframe (e.g., 1h, 1d)",
                    "default": "1h"
                },
                "limit": {
                    "type": "number",
                    "description": "Number of data points to return",
                    "default": 100
                }
            },
            "required": ["symbol"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        let request: MarketDataRequest = if let Some(input) = input {
            serde_json::from_value(input)?
        } else {
            return Ok(CallToolResponse {
                content: vec![ToolResponseContent::Text {
                    text: "Missing input parameters".to_string(),
                }],
                is_error: Some(true),
                meta: None,
            });
        };

        // Use tokio runtime to run async function
        let rt = tokio::runtime::Runtime::new()?;
        match rt.block_on(self.execute(request)) {
            Ok(response) => {
                let json_response = serde_json::to_string(&response)?;
                Ok(CallToolResponse {
                    content: vec![ToolResponseContent::Text {
                        text: json_response,
                    }],
                    is_error: Some(false),
                    meta: None,
                })
            }
            Err(e) => {
                Ok(CallToolResponse {
                    content: vec![ToolResponseContent::Text {
                        text: format!("Error: {}", e),
                    }],
                    is_error: Some(true),
                    meta: None,
                })
            }
        }
    }
}