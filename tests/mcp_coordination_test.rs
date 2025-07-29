//! Comprehensive tests for MCP Coordination Server
//!
//! This module tests the complete integration of the MCP server with:
//! - DAA orchestrator
//! - Data layer (TimescaleDB + Redis)
//! - Neural prediction system (FANN/ruv ecosystem)
//! - Custom MCP tools

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serial_test::serial;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

// Import our modules
use autonomous_platform::config::{
    DatabaseConfig, MonitoringConfig, NeuralConfig, PlatformConfig, PlatformInfo, RedisConfig,
};
use autonomous_platform::data::{
    DataPipeline, PlatformMetrics, PredictionResult, QualityMetrics, RedisCache, TimeSeriesData,
    TimescaleDBStorage,
};

// MCP-related types we'll implement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub method: String,
    pub params: serde_json::Value,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub result: Option<serde_json::Value>,
    pub error: Option<McpError>,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Mock MCP Coordinator for testing
/// This will be replaced by the real implementation
pub struct MockMCPCoordinator {
    pub data_pipeline: Arc<DataPipeline>,
    pub tools: Vec<McpTool>,
    pub active: bool,
}

impl MockMCPCoordinator {
    pub async fn new(
        storage: TimescaleDBStorage,
        cache: RedisCache,
        config: PlatformConfig,
    ) -> Result<Self> {
        let pipeline = Arc::new(DataPipeline::new(storage, cache, config).await?);
        let tools = Self::create_default_tools();

        Ok(Self {
            data_pipeline: pipeline,
            tools,
            active: false,
        })
    }

    pub async fn start_mcp_server(&mut self, _port: u16) -> Result<()> {
        self.active = true;
        Ok(())
    }

    pub async fn stop_mcp_server(&mut self) -> Result<()> {
        self.active = false;
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn get_tools(&self) -> &Vec<McpTool> {
        &self.tools
    }

    pub async fn handle_external_request(&self, request: McpRequest) -> Result<McpResponse> {
        match request.method.as_str() {
            "query_market_data" => self.handle_query_market_data(request).await,
            "get_cache_data" => self.handle_get_cache_data(request).await,
            "request_prediction" => self.handle_request_prediction(request).await,
            "agent_decision" => self.handle_agent_decision(request).await,
            "system_status" => self.handle_system_status(request).await,
            _ => Ok(McpResponse {
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
                id: request.id,
            }),
        }
    }

    async fn handle_query_market_data(&self, request: McpRequest) -> Result<McpResponse> {
        let symbol = request.params["symbol"].as_str().unwrap_or("BTC/USD");

        // Simulate querying market data
        let market_data = json!({
            "symbol": symbol,
            "timestamp": Utc::now().timestamp(),
            "price": 45000.0,
            "volume": 1000.0,
            "indicators": {
                "sma_20": 44900.0,
                "rsi": 65.5
            }
        });

        Ok(McpResponse {
            result: Some(market_data),
            error: None,
            id: request.id,
        })
    }

    async fn handle_get_cache_data(&self, request: McpRequest) -> Result<McpResponse> {
        let key = request.params["key"].as_str().unwrap_or("default");

        // Simulate cache data retrieval
        let cache_data = json!({
            "key": key,
            "data": "cached_value",
            "ttl": 300,
            "hit": true
        });

        Ok(McpResponse {
            result: Some(cache_data),
            error: None,
            id: request.id,
        })
    }

    async fn handle_request_prediction(&self, request: McpRequest) -> Result<McpResponse> {
        let symbol = request.params["symbol"].as_str().unwrap_or("BTC/USD");
        let horizon = request.params["horizon_minutes"].as_i64().unwrap_or(60);

        // Simulate neural prediction
        let prediction = json!({
            "symbol": symbol,
            "prediction_value": 45200.0,
            "confidence": 0.85,
            "horizon_minutes": horizon,
            "model_id": "NHITS_v1",
            "timestamp": Utc::now().timestamp()
        });

        Ok(McpResponse {
            result: Some(prediction),
            error: None,
            id: request.id,
        })
    }

    async fn handle_agent_decision(&self, request: McpRequest) -> Result<McpResponse> {
        let action_type = request.params["action_type"].as_str().unwrap_or("analyze");

        // Simulate DAA decision making
        let decision = json!({
            "action": action_type,
            "decision": "hold",
            "confidence": 0.75,
            "reasoning": "Market conditions are stable",
            "timestamp": Utc::now().timestamp(),
            "agent_id": "daa_primary"
        });

        Ok(McpResponse {
            result: Some(decision),
            error: None,
            id: request.id,
        })
    }

    async fn handle_system_status(&self, _request: McpRequest) -> Result<McpResponse> {
        let is_healthy = self.data_pipeline.health_check().await?;
        let metrics = self.data_pipeline.collect_metrics().await?;
        let quality = self.data_pipeline.monitor_quality().await?;

        let status = json!({
            "healthy": is_healthy,
            "metrics": {
                "total_records": metrics.total_records,
                "cache_hit_rate": metrics.cache_hit_rate,
                "processing_throughput": metrics.processing_throughput,
                "storage_usage_gb": metrics.storage_usage_gb,
                "active_connections": metrics.active_connections
            },
            "quality": {
                "data_completeness": quality.data_completeness,
                "latency_ms": quality.latency_ms,
                "error_rate": quality.error_rate,
                "overall_quality": quality.overall_quality
            },
            "timestamp": Utc::now().timestamp()
        });

        Ok(McpResponse {
            result: Some(status),
            error: None,
            id: _request.id,
        })
    }

    fn create_default_tools() -> Vec<McpTool> {
        vec![
            McpTool {
                name: "query_market_data".to_string(),
                description: "Query real-time market data from TimescaleDB".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "symbol": {"type": "string", "description": "Trading symbol"},
                        "start_time": {"type": "string", "description": "Start timestamp"},
                        "end_time": {"type": "string", "description": "End timestamp"}
                    },
                    "required": ["symbol"]
                }),
            },
            McpTool {
                name: "get_cache_data".to_string(),
                description: "Retrieve cached data from Redis".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "key": {"type": "string", "description": "Cache key"}
                    },
                    "required": ["key"]
                }),
            },
            McpTool {
                name: "request_prediction".to_string(),
                description: "Get neural network predictions using FANN/ruv-swarm-ml".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "symbol": {"type": "string", "description": "Trading symbol"},
                        "horizon_minutes": {"type": "integer", "description": "Prediction horizon"},
                        "model_id": {"type": "string", "description": "Neural model identifier"}
                    },
                    "required": ["symbol"]
                }),
            },
            McpTool {
                name: "agent_decision".to_string(),
                description: "Request autonomous decision from DAA orchestrator".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "action_type": {"type": "string", "description": "Type of decision needed"},
                        "context": {"type": "object", "description": "Decision context"}
                    },
                    "required": ["action_type"]
                }),
            },
            McpTool {
                name: "system_status".to_string(),
                description: "Get comprehensive system health and performance status".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ]
    }
}

// Helper functions for tests
fn create_test_config() -> PlatformConfig {
    PlatformConfig {
        platform: PlatformInfo {
            name: "mcp-test-platform".to_string(),
            version: "0.1.0".to_string(),
        },
        database: DatabaseConfig {
            url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://test@localhost/neural_trader_test".to_string()),
            max_connections: 10,
            min_connections: 2,
        },
        redis: RedisConfig {
            url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            max_connections: 5,
            default_ttl_seconds: 300,
        },
        neural: NeuralConfig {
            memory_gb: 1.0,
            models: vec![
                "NHITS".to_string(),
                "DeepAR".to_string(),
                "FANN".to_string(),
            ],
            prediction_cache_ttl: 600,
        },
        monitoring: MonitoringConfig {
            metrics_interval_secs: 60,
            quality_threshold: 0.95,
        },
    }
}

async fn create_test_coordinator() -> Result<MockMCPCoordinator> {
    let config = create_test_config();
    let storage = TimescaleDBStorage::new(&config.database.url).await?;
    let cache = RedisCache::new(&config.redis.url).await?;

    MockMCPCoordinator::new(storage, cache, config).await
}

// TESTS

#[tokio::test]
#[serial]
async fn test_mcp_coordinator_creation() -> Result<()> {
    let coordinator = create_test_coordinator().await?;
    assert!(!coordinator.is_active());
    assert_eq!(coordinator.get_tools().len(), 5);
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_mcp_server_start_stop() -> Result<()> {
    let mut coordinator = create_test_coordinator().await?;

    // Start server
    coordinator.start_mcp_server(3001).await?;
    assert!(coordinator.is_active());

    // Stop server
    coordinator.stop_mcp_server().await?;
    assert!(!coordinator.is_active());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_mcp_tool_registration() -> Result<()> {
    let coordinator = create_test_coordinator().await?;
    let tools = coordinator.get_tools();

    // Verify all required tools are registered
    let tool_names: Vec<&String> = tools.iter().map(|t| &t.name).collect();
    assert!(tool_names.contains(&&"query_market_data".to_string()));
    assert!(tool_names.contains(&&"get_cache_data".to_string()));
    assert!(tool_names.contains(&&"request_prediction".to_string()));
    assert!(tool_names.contains(&&"agent_decision".to_string()));
    assert!(tool_names.contains(&&"system_status".to_string()));

    // Verify tool parameters are properly defined
    for tool in tools {
        assert!(!tool.description.is_empty());
        assert!(tool.parameters.is_object());
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_query_market_data_tool() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    let request = McpRequest {
        method: "query_market_data".to_string(),
        params: json!({
            "symbol": "ETH/USD",
            "start_time": "2024-01-01T00:00:00Z",
            "end_time": "2024-01-01T01:00:00Z"
        }),
        id: "test_1".to_string(),
    };

    let response = coordinator.handle_external_request(request).await?;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(result["symbol"], "ETH/USD");
    assert!(result["price"].is_number());
    assert!(result["volume"].is_number());
    assert!(result["indicators"].is_object());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_get_cache_data_tool() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    let request = McpRequest {
        method: "get_cache_data".to_string(),
        params: json!({
            "key": "prediction:BTC/USD:latest"
        }),
        id: "test_2".to_string(),
    };

    let response = coordinator.handle_external_request(request).await?;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(result["key"], "prediction:BTC/USD:latest");
    assert!(result["hit"].is_boolean());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_request_prediction_tool() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    let request = McpRequest {
        method: "request_prediction".to_string(),
        params: json!({
            "symbol": "BTC/USD",
            "horizon_minutes": 120,
            "model_id": "FANN_deep"
        }),
        id: "test_3".to_string(),
    };

    let response = coordinator.handle_external_request(request).await?;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(result["symbol"], "BTC/USD");
    assert_eq!(result["horizon_minutes"], 120);
    assert!(result["prediction_value"].is_number());
    assert!(result["confidence"].is_number());
    assert!(result["model_id"].is_string());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_agent_decision_tool() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    let request = McpRequest {
        method: "agent_decision".to_string(),
        params: json!({
            "action_type": "trade_decision",
            "context": {
                "symbol": "BTC/USD",
                "current_price": 45000.0,
                "portfolio_balance": 10000.0
            }
        }),
        id: "test_4".to_string(),
    };

    let response = coordinator.handle_external_request(request).await?;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(result["action"], "trade_decision");
    assert!(result["decision"].is_string());
    assert!(result["confidence"].is_number());
    assert!(result["reasoning"].is_string());
    assert!(result["agent_id"].is_string());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_system_status_tool() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    let request = McpRequest {
        method: "system_status".to_string(),
        params: json!({}),
        id: "test_5".to_string(),
    };

    let response = coordinator.handle_external_request(request).await?;

    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert!(result["healthy"].is_boolean());
    assert!(result["metrics"].is_object());
    assert!(result["quality"].is_object());
    assert!(result["timestamp"].is_number());

    // Verify metrics structure
    let metrics = &result["metrics"];
    assert!(metrics["total_records"].is_number());
    assert!(metrics["cache_hit_rate"].is_number());
    assert!(metrics["processing_throughput"].is_number());

    // Verify quality structure
    let quality = &result["quality"];
    assert!(quality["data_completeness"].is_number());
    assert!(quality["error_rate"].is_number());
    assert!(quality["overall_quality"].is_number());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_invalid_method_handling() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    let request = McpRequest {
        method: "invalid_method".to_string(),
        params: json!({}),
        id: "test_6".to_string(),
    };

    let response = coordinator.handle_external_request(request).await?;

    assert!(response.result.is_none());
    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, -32601);
    assert!(error.message.contains("Method not found"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_end_to_end_data_flow() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    // Step 1: Process some market data
    let test_data = TimeSeriesData {
        symbol: "BTC/USD".to_string(),
        timestamp: Utc::now(),
        open: 45000.0,
        high: 45500.0,
        low: 44800.0,
        close: 45200.0,
        volume: 1000.0,
        indicators: vec![("sma_20".to_string(), 44900.0), ("rsi".to_string(), 65.5)]
            .into_iter()
            .collect(),
    };

    coordinator
        .data_pipeline
        .process_data(test_data.clone())
        .await?;

    // Step 2: Query the processed data via MCP
    let query_request = McpRequest {
        method: "query_market_data".to_string(),
        params: json!({"symbol": "BTC/USD"}),
        id: "e2e_1".to_string(),
    };

    let query_response = coordinator.handle_external_request(query_request).await?;
    assert!(query_response.error.is_none());

    // Step 3: Request a prediction
    let pred_request = McpRequest {
        method: "request_prediction".to_string(),
        params: json!({
            "symbol": "BTC/USD",
            "horizon_minutes": 60
        }),
        id: "e2e_2".to_string(),
    };

    let pred_response = coordinator.handle_external_request(pred_request).await?;
    assert!(pred_response.error.is_none());

    // Step 4: Request an autonomous decision
    let decision_request = McpRequest {
        method: "agent_decision".to_string(),
        params: json!({
            "action_type": "analyze",
            "context": query_response.result
        }),
        id: "e2e_3".to_string(),
    };

    let decision_response = coordinator
        .handle_external_request(decision_request)
        .await?;
    assert!(decision_response.error.is_none());

    // Step 5: Check system status
    let status_request = McpRequest {
        method: "system_status".to_string(),
        params: json!({}),
        id: "e2e_4".to_string(),
    };

    let status_response = coordinator.handle_external_request(status_request).await?;
    assert!(status_response.error.is_none());

    let status = status_response.result.unwrap();
    assert_eq!(status["healthy"], true);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_concurrent_mcp_requests() -> Result<()> {
    let coordinator = Arc::new(create_test_coordinator().await?);

    let mut handles = vec![];

    // Send multiple concurrent requests
    for i in 0..10 {
        let coordinator_clone = Arc::clone(&coordinator);
        let handle = tokio::spawn(async move {
            let request = McpRequest {
                method: "system_status".to_string(),
                params: json!({}),
                id: format!("concurrent_{}", i),
            };

            coordinator_clone.handle_external_request(request).await
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    for handle in handles {
        let response = handle.await??;
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_error_recovery_and_resilience() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    // Test with malformed parameters
    let bad_request = McpRequest {
        method: "request_prediction".to_string(),
        params: json!({
            "invalid_param": "bad_value"
        }),
        id: "error_test".to_string(),
    };

    let response = coordinator.handle_external_request(bad_request).await?;
    // Should handle gracefully - either return default values or error
    assert!(response.result.is_some() || response.error.is_some());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_performance_monitoring() -> Result<()> {
    let coordinator = create_test_coordinator().await?;

    // Measure response time for system status
    let start = std::time::Instant::now();

    let request = McpRequest {
        method: "system_status".to_string(),
        params: json!({}),
        id: "perf_test".to_string(),
    };

    let response = coordinator.handle_external_request(request).await?;
    let duration = start.elapsed();

    assert!(response.error.is_none());
    assert!(duration.as_millis() < 1000); // Should respond within 1 second

    // Verify performance metrics are within acceptable ranges
    let result = response.result.unwrap();
    let quality = &result["quality"];
    assert!(quality["latency_ms"].as_f64().unwrap() < 1000.0);

    Ok(())
}
