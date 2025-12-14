# MCP Server Implementation Analysis

**Analysis Date:** December 14, 2025
**Scope:** `/workspaces/neural-data-platform/mcp-trading-server/`

---

## 1. Current MCP Server Architecture

The MCP trading server provides a pattern for exposing tools to Claude. It uses MCP SDK v0.0.3 and has a clean modular architecture.

### Directory Structure

```
mcp-trading-server/
├── src/
│   ├── main.rs                 # Entry point
│   ├── lib.rs                  # Server struct & initialization
│   ├── config.rs               # Configuration
│   ├── error.rs                # Error types
│   ├── models.rs               # Data structures (304 lines)
│   ├── tools/                  # MCP Tool implementations
│   │   ├── market_data.rs      # OHLCV queries (153 lines) ✓ MCP-exposed
│   │   ├── cache.rs            # Redis ops (176 lines) ✓ MCP-exposed
│   │   ├── trading.rs          # Decisions (169 lines)
│   │   ├── neural.rs           # Predictions (81 lines)
│   │   ├── health.rs           # Monitoring (54 lines)
│   │   └── training_triggers.rs # Training (828 lines)
│   ├── integrations/           # Backend clients
│   │   ├── database.rs         # PostgreSQL
│   │   ├── redis.rs            # Cache
│   │   ├── neural.rs           # ML service
│   │   ├── agent.rs            # Trading signals
│   │   └── monitor.rs          # Health
│   └── handlers/               # Request handlers
│       └── training_handler.rs # Training triggers

Total: ~3,340 lines of implementation
```

---

## 2. MCP Tool Pattern

### Tool Trait Implementation

```rust
// From mcp-trading-server/src/tools/market_data.rs

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
                "symbol": { "type": "string" },
                "timeframe": { "type": "string", "default": "1h" },
                "limit": { "type": "number", "default": 100 }
            },
            "required": ["symbol"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        // 1. Deserialize input
        // 2. Create tokio runtime
        // 3. Call async execute()
        // 4. Return CallToolResponse
    }
}
```

### Key Patterns
- Request/Response enums with tagged serialization
- Async execution wrapped in sync `Tool::call()`
- Schema-driven input validation
- Error propagation via `CallToolResponse::is_error`

---

## 3. Currently Exposed Tools

| Tool Name | Description | Status |
|-----------|-------------|--------|
| `get_market_data` | Fetch OHLCV data | EXPOSED |
| `cache_operation` | Redis get/set/delete | EXPOSED |
| Trading/Neural/Health | Internal tools | NOT EXPOSED |

---

## 4. Air Quality Tool Requirements (FR-6)

Per the air-001 specification, these 5 MCP tools are required:

### FR-6.1: Air Quality Query Tool

```rust
// Proposed: src/tools/air_quality.rs

pub struct AirQualityQueryTool {
    storage: Arc<ParquetStore>,
}

impl Tool for AirQualityQueryTool {
    fn name(&self) -> String { "air_quality_query".to_string() }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "location_id": { "type": "string" },
                "time_range": {
                    "type": "string",
                    "enum": ["current", "last_hour", "last_24h", "last_7d"]
                },
                "metrics": {
                    "type": "array",
                    "items": { "enum": ["co2", "pm25", "voc", "temp", "humidity"] }
                }
            },
            "required": ["location_id"]
        })
    }
}

// Output example:
{
    "co2_ppm": 850,
    "co2_level": "Acceptable",
    "pm25_ugm3": 8.2,
    "pm25_level": "Good",
    "voc_index": 100,
    "temperature_c": 22.5,
    "humidity_pct": 45
}
```

### FR-6.2: Forecast Tool

```rust
pub struct AirQualityForecastTool {
    forecaster: Arc<AirQualityForecaster>,
}

impl Tool for AirQualityForecastTool {
    fn name(&self) -> String { "air_quality_forecast".to_string() }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "location_id": { "type": "string" },
                "metric": { "enum": ["co2", "pm25"] },
                "horizon_hours": { "type": "integer", "minimum": 1, "maximum": 6 }
            },
            "required": ["location_id", "metric"]
        })
    }
}

// Output example:
[
    {"time": "15:00", "pm25_p50": 9.5, "pm25_p10": 7.2, "pm25_p90": 12.8},
    {"time": "16:00", "pm25_p50": 10.2, "pm25_p10": 8.0, "pm25_p90": 14.1}
]
```

### FR-6.3: Alert Retrieval Tool

```rust
pub struct AirQualityAlertsTool {
    alert_store: Arc<AlertStore>,
}

impl Tool for AirQualityAlertsTool {
    fn name(&self) -> String { "air_quality_alerts".to_string() }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "location_id": { "type": "string" },
                "time_range": { "enum": ["active", "last_24h", "last_7d"] },
                "severity_filter": {
                    "type": "array",
                    "items": { "enum": ["Moderate", "Poor", "VeryPoor"] }
                }
            },
            "required": ["location_id"]
        })
    }
}
```

### FR-6.4: Sensor Health Tool

```rust
pub struct SensorHealthTool {
    storage: Arc<ParquetStore>,
}

impl Tool for SensorHealthTool {
    fn name(&self) -> String { "air_quality_sensor_health".to_string() }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "location_id": { "type": "string" }
            },
            "required": ["location_id"]
        })
    }
}

// Output example:
{
    "status": "online",
    "last_reading_age_seconds": 45,
    "wifi_signal_dbm": -46,
    "co2_calibration_status": "active",
    "pm_quality": "good",
    "firmware": "3.1.4"
}
```

### FR-6.5: Recommendation Tool

```rust
pub struct RecommendationTool {
    storage: Arc<ParquetStore>,
    forecaster: Arc<AirQualityForecaster>,
}

impl Tool for RecommendationTool {
    fn name(&self) -> String { "air_quality_recommendations".to_string() }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "location_id": { "type": "string" }
            },
            "required": ["location_id"]
        })
    }
}

// Output example:
[
    "Open windows for 15 minutes to reduce CO2 from 1200 to <1000 ppm",
    "High PM2.5 detected - use range hood or air purifier",
    "Mold risk moderate (65% RH) - consider dehumidifier"
]
```

---

## 5. Implementation Effort

### Reusable Patterns from Trading Server

| Pattern | Source | Target |
|---------|--------|--------|
| Tool trait implementation | `market_data.rs` | All 5 tools |
| Request/Response enums | `models.rs` | Air quality models |
| Error handling | `error.rs` | Air quality errors |
| Async wrappers | `market_data.rs:call()` | All tools |
| Client integration | `integrations/*.rs` | Storage client |

### Estimated Lines of Code

| Tool | Est. Lines |
|------|------------|
| `air_quality_query` | 150 |
| `air_quality_forecast` | 120 |
| `air_quality_alerts` | 180 |
| `air_quality_sensor_health` | 140 |
| `air_quality_recommendations` | 200 |
| **Total** | **790** |

### Estimated Effort: 2-3 days

---

## 6. Integration Architecture

### Option A: Separate MCP Server

```
┌───────────────┐     ┌──────────────────┐
│  air-quality  │     │  mcp-air-quality │
│     app       │◄───►│     server       │
│  (REST API)   │     │  (MCP Protocol)  │
└───────────────┘     └──────────────────┘
```

**Pros:** Clean separation, independent scaling
**Cons:** Extra service, IPC overhead

### Option B: Embedded in Air Quality App

```
┌────────────────────────────────────┐
│        air-quality-app             │
│  ┌─────────────┐  ┌─────────────┐  │
│  │  REST API   │  │  MCP Server │  │
│  │  (port 8080)│  │  (stdio)    │  │
│  └─────────────┘  └─────────────┘  │
└────────────────────────────────────┘
```

**Pros:** Single binary, shared state
**Cons:** Coupled deployment

### Recommendation: Option B (Embedded)

The air-quality-app Cargo.toml already has optional MCP support:

```toml
[features]
mcp = ["dep:mcp-sdk"]

[[bin]]
name = "air-quality-mcp"
path = "src/mcp_main.rs"
required-features = ["mcp"]
```

Build with `cargo build --features mcp` to include MCP tools.

---

## 7. MCP Server Entry Point

### Proposed: `apps/air-quality-app/src/mcp_main.rs`

```rust
use mcp_sdk::{Server, transport::StdioTransport};
use air_quality_app::{
    tools::{
        AirQualityQueryTool,
        AirQualityForecastTool,
        AirQualityAlertsTool,
        SensorHealthTool,
        RecommendationTool,
    },
    storage::ParquetStore,
    alerting::AlertStore,
    forecasting::AirQualityForecaster,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::init();

    // Load configuration
    let config = load_config()?;

    // Initialize shared state
    let storage = Arc::new(ParquetStore::new(&config.storage)?);
    let alert_store = Arc::new(AlertStore::new());
    let forecaster = Arc::new(AirQualityForecaster::load(&config.models)?);

    // Create MCP server
    let mut server = Server::new();

    // Register tools
    server.add_tool(Box::new(AirQualityQueryTool::new(storage.clone())));
    server.add_tool(Box::new(AirQualityForecastTool::new(forecaster.clone())));
    server.add_tool(Box::new(AirQualityAlertsTool::new(alert_store.clone())));
    server.add_tool(Box::new(SensorHealthTool::new(storage.clone())));
    server.add_tool(Box::new(RecommendationTool::new(storage.clone(), forecaster)));

    // Start server with stdio transport
    let transport = StdioTransport::new();
    server.run(transport).await?;

    Ok(())
}
```

---

## 8. Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_air_quality_query_schema() {
        let tool = AirQualityQueryTool::new(mock_storage());
        let schema = tool.input_schema();
        assert!(schema["properties"]["location_id"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("location_id")));
    }

    #[tokio::test]
    async fn test_air_quality_query_execution() {
        let tool = AirQualityQueryTool::new(mock_storage_with_data());
        let input = json!({
            "location_id": "living_room",
            "time_range": "current"
        });

        let response = tool.call(Some(input)).unwrap();
        assert!(!response.is_error);

        let content: serde_json::Value = serde_json::from_str(&response.content[0].text).unwrap();
        assert!(content["co2_ppm"].is_number());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_mcp_server_tool_discovery() {
    let server = create_test_server();
    let tools = server.list_tools();

    assert!(tools.iter().any(|t| t.name == "air_quality_query"));
    assert!(tools.iter().any(|t| t.name == "air_quality_forecast"));
    assert!(tools.iter().any(|t| t.name == "air_quality_alerts"));
    assert!(tools.iter().any(|t| t.name == "air_quality_sensor_health"));
    assert!(tools.iter().any(|t| t.name == "air_quality_recommendations"));
}
```

---

## 9. E2E Readiness

### Current Status: NOT READY

- No MCP tools implemented
- MCP binary entry point exists but is empty
- Trading server patterns available for reference

### To Reach E2E Ready

1. Implement 5 MCP tools (~800 lines)
2. Wire tools to shared storage/forecaster
3. Add unit tests for each tool
4. Integrate with Docker E2E test suite

### Priority: MEDIUM

MCP integration is additive - the core air quality platform works without it. However, Claude integration is a key differentiator for the product vision.
