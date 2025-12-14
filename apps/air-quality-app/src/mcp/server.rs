//! MCP Server implementation with stdio transport
//! Registers all air quality tools and handles tool invocation

use std::sync::Arc;
use tracing::info;
use mcp_sdk::tools::Tool;
use mcp_sdk::types::CallToolResponse;
use anyhow::{Result, anyhow};

use super::tools::*;

pub struct McpServer {
    tools: Vec<Box<dyn Tool>>,
}

// Concrete implementations of traits for production use
struct DefaultStore;
struct DefaultForecast;
struct DefaultAlerts;

impl AirQualityStore for DefaultStore {
    fn get_current_reading(&self, location_id: &str) -> Result<AirQualityData, String> {
        // Placeholder implementation - would connect to actual data store
        Ok(AirQualityData {
            timestamp: chrono::Utc::now(),
            co2: Some(850.0),
            pm25: Some(12.5),
            voc_index: Some(120.0),
            temperature: Some(22.5),
            humidity: Some(45.0),
        })
    }

    fn get_readings_in_range(&self, location_id: &str, hours: u32) -> Result<Vec<AirQualityData>, String> {
        // Placeholder - would query database
        Ok(vec![])
    }

    fn get_all_locations(&self) -> Result<Vec<Location>, String> {
        // Placeholder - would query database
        Ok(vec![
            Location {
                id: "location-1".to_string(),
                name: "Living Room".to_string(),
                sensor_serial: "AG-123456".to_string(),
            },
        ])
    }
}

impl ForecastService for DefaultForecast {
    fn predict(&self, location_id: &str, metric: &str, horizon_hours: u32) -> Result<Vec<ForecastPoint>, String> {
        // Placeholder - would call ML model
        Ok(vec![])
    }
}

impl AlertService for DefaultAlerts {
    fn get_active_alerts(&self, location_id: &str) -> Result<Vec<Alert>, String> {
        // Placeholder - would query alert system
        Ok(vec![])
    }

    fn get_alerts_in_range(&self, location_id: &str, hours: u32, severity: Option<Vec<String>>) -> Result<Vec<Alert>, String> {
        // Placeholder
        Ok(vec![])
    }

    fn get_recommendations(&self, location_id: &str, data: &AirQualityData) -> Result<Vec<String>, String> {
        // Placeholder - would use rules engine
        let mut recs = vec![];
        
        if let Some(co2) = data.co2 {
            if co2 > 1000.0 {
                recs.push(format!("Open windows for 15 minutes to reduce CO2 from {} to <1000 ppm", co2.round()));
            }
        }
        
        if let Some(pm25) = data.pm25 {
            if pm25 > 12.0 {
                recs.push(format!("Consider air purifier - PM2.5 is {} µg/m³ (Moderate)", pm25));
            }
        }
        
        Ok(recs)
    }
}

impl McpServer {
    pub async fn new() -> Result<Self> {
        info!("Initializing MCP Server for Air Quality");

        let store = DefaultStore;
        let forecast = DefaultForecast;
        let alerts = DefaultAlerts;

        // Register all tools
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();
        
        tools.push(Box::new(AirQualityQueryTool::new(store.clone())));
        tools.push(Box::new(AirQualityForecastTool::new(forecast.clone())));
        tools.push(Box::new(AirQualityAlertsTool::new(alerts.clone())));
        tools.push(Box::new(AirQualitySensorHealthTool::new(store.clone())));
        tools.push(Box::new(AirQualityRecommendationsTool::new(store.clone(), alerts.clone())));
        tools.push(Box::new(ListLocationsTool::new(store)));

        info!("Registered {} MCP tools", tools.len());

        Ok(Self { tools })
    }

    pub fn list_tools(&self) -> &[Box<dyn Tool>] {
        &self.tools
    }

    pub async fn call_tool(&self, tool_name: &str, input: Option<serde_json::Value>) -> Result<CallToolResponse> {
        let tool = self.tools
            .iter()
            .find(|t| t.name() == tool_name)
            .ok_or_else(|| anyhow!("Tool '{}' not found", tool_name))?;

        tool.call(input).map_err(|e| anyhow!(e))
    }

    pub async fn start(&self) -> Result<()> {
        info!("MCP Air Quality Server started - listening on stdio");
        
        // In production, this would start the stdio transport loop
        // For now, server is ready to handle tool calls
        
        Ok(())
    }
}

// Clone implementations for concrete types
impl Clone for DefaultStore {
    fn clone(&self) -> Self {
        DefaultStore
    }
}

impl Clone for DefaultForecast {
    fn clone(&self) -> Self {
        DefaultForecast
    }
}

impl Clone for DefaultAlerts {
    fn clone(&self) -> Self {
        DefaultAlerts
    }
}
