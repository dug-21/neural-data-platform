//! Integration tests for MCP functionality
//! Testing the full MCP server with default implementations

#[cfg(feature = "mcp")]
mod tests {
    use air_quality_app::mcp::server::McpServer;
    use serde_json::json;

    #[tokio::test]
    async fn test_list_locations_integration() {
        let server = McpServer::new().await.unwrap();

        let result = server.call_tool("list_locations", Some(json!({}))).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));
        assert!(!response.content.is_empty());

        // Verify response contains JSON with locations array
        if let Some(text) = response.content.first() {
            match text {
                mcp_sdk::types::ToolResponseContent::Text { text } => {
                    let data: serde_json::Value = serde_json::from_str(text).unwrap();
                    assert!(data.get("locations").is_some());
                }
                _ => panic!("Expected text content"),
            }
        }
    }

    #[tokio::test]
    async fn test_air_quality_query_integration() {
        let server = McpServer::new().await.unwrap();

        let input = json!({
            "location_id": "location-1",
            "time_range": "current",
            "metrics": ["co2", "pm25"]
        });

        let result = server.call_tool("air_quality_query", Some(input)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_sensor_health_integration() {
        let server = McpServer::new().await.unwrap();

        let input = json!({
            "location_id": "location-1"
        });

        let result = server
            .call_tool("air_quality_sensor_health", Some(input))
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));

        // Verify response structure
        if let Some(text) = response.content.first() {
            match text {
                mcp_sdk::types::ToolResponseContent::Text { text } => {
                    let data: serde_json::Value = serde_json::from_str(text).unwrap();
                    assert!(data.get("status").is_some());
                    assert!(data.get("last_reading_age_seconds").is_some());
                }
                _ => panic!("Expected text content"),
            }
        }
    }

    #[tokio::test]
    async fn test_forecast_integration() {
        let server = McpServer::new().await.unwrap();

        let input = json!({
            "location_id": "location-1",
            "metric": "pm25",
            "horizon_hours": 3
        });

        let result = server.call_tool("air_quality_forecast", Some(input)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_alerts_integration() {
        let server = McpServer::new().await.unwrap();

        let input = json!({
            "location_id": "location-1",
            "time_range": "active"
        });

        let result = server.call_tool("air_quality_alerts", Some(input)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_recommendations_integration() {
        let server = McpServer::new().await.unwrap();

        let input = json!({
            "location_id": "location-1"
        });

        let result = server
            .call_tool("air_quality_recommendations", Some(input))
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.is_error, Some(false));

        // Verify response has recommendations
        if let Some(text) = response.content.first() {
            match text {
                mcp_sdk::types::ToolResponseContent::Text { text } => {
                    let data: serde_json::Value = serde_json::from_str(text).unwrap();
                    assert!(data.get("recommendations").is_some());
                }
                _ => panic!("Expected text content"),
            }
        }
    }

    #[tokio::test]
    async fn test_tool_error_handling() {
        let server = McpServer::new().await.unwrap();

        // Test invalid time_range
        let input = json!({
            "location_id": "location-1",
            "time_range": "invalid_range"
        });

        let result = server.call_tool("air_quality_query", Some(input)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        // Should have error flag or error content
        assert!(response.is_error.unwrap_or(false) ||
                response.content.iter().any(|c| {
                    matches!(c, mcp_sdk::types::ToolResponseContent::Text { text } if text.contains("Error"))
                }));
    }

    #[tokio::test]
    async fn test_forecast_horizon_validation() {
        let server = McpServer::new().await.unwrap();

        // Test horizon > 6
        let input = json!({
            "location_id": "location-1",
            "metric": "co2",
            "horizon_hours": 10
        });

        let result = server.call_tool("air_quality_forecast", Some(input)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn test_all_tools_have_descriptions() {
        let server = McpServer::new().await.unwrap();
        let tools = server.list_tools();

        for tool in tools {
            let desc = tool.description();
            assert!(!desc.is_empty(), "{} should have description", tool.name());
            assert!(
                desc.contains("air quality")
                    || desc.contains("sensor")
                    || desc.contains("location"),
                "{} description should be relevant: {}",
                tool.name(),
                desc
            );
        }
    }
}
