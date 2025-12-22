//! Tests for MCP server initialization and tool registration
//! Following TDD London School - testing collaborations and interactions

#[cfg(feature = "mcp")]
mod tests {
    use air_quality_app::mcp::server::McpServer;

    #[tokio::test]
    async fn test_server_initialization() {
        // Test that server initializes without panicking
        let result = McpServer::new().await;
        assert!(result.is_ok(), "Server should initialize successfully");
    }

    #[tokio::test]
    async fn test_tool_registration() {
        // Test that all expected tools are registered
        let server = McpServer::new().await.unwrap();
        let tools = server.list_tools();

        let expected_tools = vec![
            "air_quality_query",
            "air_quality_forecast",
            "air_quality_alerts",
            "air_quality_sensor_health",
            "air_quality_recommendations",
            "list_locations",
        ];

        for tool_name in expected_tools {
            assert!(
                tools.iter().any(|t| t.name() == tool_name),
                "Tool {} should be registered",
                tool_name
            );
        }

        assert_eq!(tools.len(), 6, "Should have exactly 6 tools registered");
    }

    #[tokio::test]
    async fn test_tool_invocation() {
        // Test that tools can be invoked through the server
        let server = McpServer::new().await.unwrap();

        let input = serde_json::json!({});
        let result = server.call_tool("list_locations", Some(input)).await;

        assert!(result.is_ok(), "Tool invocation should succeed");
    }

    #[tokio::test]
    async fn test_invalid_tool_name() {
        // Test error handling for invalid tool names
        let server = McpServer::new().await.unwrap();

        let input = serde_json::json!({});
        let result = server.call_tool("invalid_tool_name", Some(input)).await;

        assert!(result.is_err(), "Should return error for invalid tool name");
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn test_missing_required_params() {
        // Test that tools validate required parameters
        let server = McpServer::new().await.unwrap();

        // air_quality_query requires location_id
        let input = serde_json::json!({
            "time_range": "current"
            // Missing location_id
        });

        let result = server.call_tool("air_quality_query", Some(input)).await;

        // Should either error or return error response
        let has_error = result.is_err() || {
            if let Ok(response) = result {
                response.is_error.unwrap_or(false)
            } else {
                false
            }
        };
        assert!(
            has_error,
            "Should fail validation for missing required params"
        );
    }

    #[tokio::test]
    async fn test_response_formatting() {
        // Test that responses are properly formatted as MCP CallToolResponse
        let server = McpServer::new().await.unwrap();

        let input = serde_json::json!({});
        let result = server.call_tool("list_locations", Some(input)).await;

        if let Ok(response) = result {
            assert!(!response.content.is_empty(), "Response should have content");
            assert!(
                response.is_error.is_some(),
                "Response should indicate error status"
            );
        }
    }

    #[tokio::test]
    async fn test_tool_descriptions() {
        // Test that tools have proper descriptions for Claude
        let server = McpServer::new().await.unwrap();
        let tools = server.list_tools();

        for tool in tools {
            assert!(
                !tool.description().is_empty(),
                "Tool {} should have a description",
                tool.name()
            );

            // Verify input schema is valid JSON
            let schema = tool.input_schema();
            assert!(
                schema.is_object(),
                "Tool {} schema should be a JSON object",
                tool.name()
            );
        }
    }

    #[tokio::test]
    async fn test_tool_schemas_have_required_fields() {
        // Verify tool schemas properly define required fields
        let server = McpServer::new().await.unwrap();
        let tools = server.list_tools();

        // air_quality_query should require location_id
        let query_tool = tools
            .iter()
            .find(|t| t.name() == "air_quality_query")
            .expect("Should find air_quality_query tool");

        let schema = query_tool.input_schema();
        let required = schema.get("required").and_then(|v| v.as_array());

        assert!(
            required.is_some(),
            "Schema should have required fields array"
        );
        assert!(
            required
                .unwrap()
                .iter()
                .any(|v| v.as_str() == Some("location_id")),
            "location_id should be required"
        );
    }
}
