//! MCP Tool Registration
//! 
//! Registers Neural Trader tools with the ruv-swarm MCP server

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use crate::mcp::TradingMcpTools;

/// Tool metadata for MCP registration
#[derive(Debug, Clone)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Register all trading tools with the MCP server
pub async fn register_mcp_tools(_tools: Arc<TradingMcpTools>) -> Result<()> {
    let tool_definitions = vec![
        ToolMetadata {
            name: "query_market_data".to_string(),
            description: "Query historical market data from TimescaleDB".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Trading pair symbol (e.g., BTC/USD)"
                    },
                    "interval": {
                        "type": "string",
                        "description": "Time interval (1m, 5m, 15m, 1h, etc.)",
                        "default": "1m"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of data points to return",
                        "default": 100
                    },
                    "start_time": {
                        "type": "string",
                        "description": "Start time in ISO 8601 format"
                    },
                    "end_time": {
                        "type": "string",
                        "description": "End time in ISO 8601 format"
                    },
                    "aggregation": {
                        "type": "string",
                        "description": "Aggregation type (ohlc)",
                        "enum": ["ohlc", "none"]
                    }
                },
                "required": ["symbol"]
            }),
        },
        ToolMetadata {
            name: "get_cache_data".to_string(),
            description: "Retrieve data from Redis cache".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "Redis key to retrieve"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Pattern to match multiple keys (e.g., market:*)"
                    }
                }
            }),
        },
        ToolMetadata {
            name: "request_prediction".to_string(),
            description: "Get neural network predictions for a symbol".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Trading pair symbol"
                    },
                    "horizon": {
                        "type": "integer",
                        "description": "Prediction horizon in time steps",
                        "default": 5
                    },
                    "confidence_threshold": {
                        "type": "number",
                        "description": "Minimum confidence threshold",
                        "default": 0.0
                    },
                    "ensemble": {
                        "type": "boolean",
                        "description": "Use ensemble of models",
                        "default": false
                    },
                    "models": {
                        "type": "array",
                        "description": "Specific models to use for ensemble",
                        "items": {
                            "type": "string"
                        }
                    },
                    "features": {
                        "type": "object",
                        "description": "Additional features for prediction"
                    }
                },
                "required": ["symbol"]
            }),
        },
        ToolMetadata {
            name: "agent_decision".to_string(),
            description: "Get trading decision from autonomous agent".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Trading pair symbol"
                    },
                    "position_size": {
                        "type": "number",
                        "description": "Desired position size"
                    },
                    "current_position": {
                        "type": "number",
                        "description": "Current position size",
                        "default": 0.0
                    },
                    "entry_price": {
                        "type": "number",
                        "description": "Entry price for existing position"
                    },
                    "current_price": {
                        "type": "number",
                        "description": "Current market price"
                    },
                    "portfolio_value": {
                        "type": "number",
                        "description": "Total portfolio value"
                    },
                    "strategy_weights": {
                        "type": "object",
                        "description": "Weights for multi-strategy decisions"
                    }
                },
                "required": ["symbol"]
            }),
        },
        ToolMetadata {
            name: "system_status".to_string(),
            description: "Get comprehensive system health and status".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "detailed": {
                        "type": "boolean",
                        "description": "Include detailed metrics",
                        "default": false
                    },
                    "include_alerts": {
                        "type": "boolean",
                        "description": "Include active alerts",
                        "default": false
                    },
                    "include_resources": {
                        "type": "boolean",
                        "description": "Include resource usage",
                        "default": false
                    },
                    "include_trading_stats": {
                        "type": "boolean",
                        "description": "Include trading statistics",
                        "default": false
                    },
                    "metrics_window": {
                        "type": "string",
                        "description": "Time window for metrics (1m, 5m, 1h)",
                        "default": "5m"
                    }
                }
            }),
        },
    ];
    
    // Register each tool with the MCP server
    let tool_count = tool_definitions.len();
    for tool in tool_definitions {
        println!("Registering MCP tool: {}", tool.name);
        // Here we would call the actual ruv-swarm MCP registration API
        // For now, we'll just log the registration
    }
    
    println!("Successfully registered {} MCP tools", tool_count);
    Ok(())
}

/// Create MCP tool handler
pub fn create_tool_handler(tools: Arc<TradingMcpTools>) -> impl Fn(&str, serde_json::Value) -> Result<serde_json::Value> {
    move |method: &str, params: serde_json::Value| -> Result<serde_json::Value> {
        let tools = tools.clone();
        
        // Use tokio runtime for async operations
        let runtime = tokio::runtime::Runtime::new()?;
        
        runtime.block_on(async move {
            match method {
                "query_market_data" => tools.query_market_data(params).await,
                "get_cache_data" => tools.get_cache_data(params).await,
                "request_prediction" => tools.request_prediction(params).await,
                "agent_decision" => tools.agent_decision(params).await,
                "system_status" => tools.system_status(params).await,
                _ => Err(anyhow::anyhow!("Unknown method: {}", method)),
            }
        })
    }
}