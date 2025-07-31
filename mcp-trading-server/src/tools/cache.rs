use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::Result;
use crate::integrations::redis::RedisClient;
use anyhow::Result as AnyhowResult;
use mcp_sdk::tools::Tool;
use mcp_sdk::types::{CallToolResponse, ToolResponseContent};

#[derive(Debug, Clone)]
pub struct CacheTool {
    redis_client: Arc<RedisClient>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum CacheRequest {
    Get {
        key: String,
    },
    Set {
        key: String,
        value: String,
        ttl: Option<u64>,
    },
    Delete {
        key: String,
    },
    Clear {
        pattern: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheResponse {
    pub success: bool,
    pub message: String,
    pub value: Option<String>,
}

impl CacheTool {
    pub fn new(redis_client: Arc<RedisClient>) -> Self {
        Self { redis_client }
    }

    pub async fn execute(&self, request: CacheRequest) -> Result<CacheResponse> {
        match request {
            CacheRequest::Get { key } => {
                info!("Getting cache value for key: {}", key);
                // Redis client needs to be mutable - clone the Arc content
                let mut redis_conn = self.redis_client.as_ref().clone();
                match redis_conn.get::<String>(&key).await? {
                    Some(value) => Ok(CacheResponse {
                        success: true,
                        message: "Value retrieved".to_string(),
                        value: Some(value),
                    }),
                    None => Ok(CacheResponse {
                        success: false,
                        message: "Key not found".to_string(),
                        value: None,
                    }),
                }
            }
            CacheRequest::Set { key, value, ttl } => {
                info!("Setting cache value for key: {}", key);
                let mut redis_conn = self.redis_client.as_ref().clone();
                let ttl_duration = std::time::Duration::from_secs(ttl.unwrap_or(3600));
                redis_conn.set(&key, &value, ttl_duration).await?;
                Ok(CacheResponse {
                    success: true,
                    message: "Value set successfully".to_string(),
                    value: None,
                })
            }
            CacheRequest::Delete { key } => {
                info!("Deleting cache key: {}", key);
                let mut redis_conn = self.redis_client.as_ref().clone();
                redis_conn.delete(&key).await?;
                Ok(CacheResponse {
                    success: true,
                    message: "Key deleted".to_string(),
                    value: None,
                })
            }
            CacheRequest::Clear { pattern } => {
                info!("Clearing cache with pattern: {:?}", pattern);
                // Clear cache not directly supported - would need to scan and delete
                return Err(crate::error::Error::NotImplemented(
                    "Clear cache operation not implemented".to_string(),
                ));
                Ok(CacheResponse {
                    success: true,
                    message: "Cache cleared".to_string(),
                    value: None,
                })
            }
        }
    }
}

impl Tool for CacheTool {
    fn name(&self) -> String {
        "cache_operation".to_string()
    }

    fn description(&self) -> String {
        "Perform cache operations (get, set, delete, clear)".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["Get", "Set", "Delete", "Clear"],
                    "description": "Cache operation to perform"
                },
                "key": {
                    "type": "string",
                    "description": "Cache key"
                },
                "value": {
                    "type": "string",
                    "description": "Value to set (for Set operation)"
                },
                "ttl": {
                    "type": "number",
                    "description": "Time to live in seconds (for Set operation)"
                },
                "pattern": {
                    "type": "string",
                    "description": "Pattern for clearing cache (for Clear operation)"
                }
            },
            "required": ["operation"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        let request: CacheRequest = if let Some(input) = input {
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
            Err(e) => Ok(CallToolResponse {
                content: vec![ToolResponseContent::Text {
                    text: format!("Error: {}", e),
                }],
                is_error: Some(true),
                meta: None,
            }),
        }
    }
}
