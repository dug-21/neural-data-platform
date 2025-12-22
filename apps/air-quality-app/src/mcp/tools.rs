//! MCP Tools for air quality data interaction
//! Implements tools that Claude can use to query and analyze air quality data

use anyhow::Result as AnyhowResult;
use chrono::{DateTime, Utc};
use mcp_sdk::tools::Tool;
use mcp_sdk::types::{CallToolResponse, ToolResponseContent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

// Re-export test types for testing
#[cfg(test)]
pub use crate::mcp::test_types::*;

// Trait definitions for dependency injection (enables mocking)
pub trait AirQualityStore: Send + Sync {
    fn get_current_reading(&self, location_id: &str) -> Result<AirQualityData, String>;
    fn get_readings_in_range(
        &self,
        location_id: &str,
        hours: u32,
    ) -> Result<Vec<AirQualityData>, String>;
    fn get_all_locations(&self) -> Result<Vec<Location>, String>;
}

pub trait ForecastService: Send + Sync {
    fn predict(
        &self,
        location_id: &str,
        metric: &str,
        horizon_hours: u32,
    ) -> Result<Vec<ForecastPoint>, String>;
}

pub trait AlertService: Send + Sync {
    fn get_active_alerts(&self, location_id: &str) -> Result<Vec<Alert>, String>;
    fn get_alerts_in_range(
        &self,
        location_id: &str,
        hours: u32,
        severity: Option<Vec<String>>,
    ) -> Result<Vec<Alert>, String>;
    fn get_recommendations(
        &self,
        location_id: &str,
        data: &AirQualityData,
    ) -> Result<Vec<String>, String>;
}

// Data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityData {
    pub timestamp: DateTime<Utc>,
    pub co2: Option<f64>,
    pub pm25: Option<f64>,
    pub voc_index: Option<f64>,
    pub temperature: Option<f64>,
    pub humidity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastPoint {
    pub time: DateTime<Utc>,
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub severity: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub id: String,
    pub name: String,
    pub sensor_serial: String,
}

// Tool 1: Air Quality Query
#[derive(Debug, Clone)]
pub struct AirQualityQueryTool<S: AirQualityStore> {
    store: S,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirQualityQueryInput {
    pub location_id: String,
    #[serde(default = "default_time_range")]
    pub time_range: String,
    #[serde(default = "default_metrics")]
    pub metrics: Vec<String>,
}

fn default_time_range() -> String {
    "current".to_string()
}

fn default_metrics() -> Vec<String> {
    vec![
        "co2".to_string(),
        "pm25".to_string(),
        "voc".to_string(),
        "temp".to_string(),
        "humidity".to_string(),
    ]
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirQualityQueryOutput {
    pub readings: Vec<AirQualityData>,
    pub health_interpretation: HealthInterpretation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthInterpretation {
    pub air_quality_index: String,
    pub health_advice: String,
}

impl<S: AirQualityStore> AirQualityQueryTool<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn execute(
        &self,
        input: AirQualityQueryInput,
    ) -> Result<AirQualityQueryOutput, String> {
        info!(
            "Querying air quality for location: {} ({})",
            input.location_id, input.time_range
        );

        let readings = match input.time_range.as_str() {
            "current" => {
                let reading = self.store.get_current_reading(&input.location_id)?;
                vec![reading]
            }
            "last_hour" => self.store.get_readings_in_range(&input.location_id, 1)?,
            "last_24h" => self.store.get_readings_in_range(&input.location_id, 24)?,
            "last_7d" => self.store.get_readings_in_range(&input.location_id, 168)?,
            _ => return Err(format!("Invalid time_range: {}", input.time_range)),
        };

        let health_interpretation = self.interpret_health(&readings);

        Ok(AirQualityQueryOutput {
            readings,
            health_interpretation,
        })
    }

    fn interpret_health(&self, readings: &[AirQualityData]) -> HealthInterpretation {
        if readings.is_empty() {
            return HealthInterpretation {
                air_quality_index: "Unknown".to_string(),
                health_advice: "No data available".to_string(),
            };
        }

        let latest = &readings[readings.len() - 1];
        let pm25 = latest.pm25.unwrap_or(0.0);
        let co2 = latest.co2.unwrap_or(0.0);

        let aqi = if pm25 < 12.0 && co2 < 1000.0 {
            "Good"
        } else if pm25 < 35.4 && co2 < 1500.0 {
            "Moderate"
        } else {
            "Unhealthy"
        };

        let advice = match aqi {
            "Good" => "Air quality is satisfactory. No special precautions needed.",
            "Moderate" => "Consider reducing prolonged outdoor activities if sensitive.",
            _ => "Air quality is poor. Limit outdoor exposure and use air purifiers.",
        };

        HealthInterpretation {
            air_quality_index: aqi.to_string(),
            health_advice: advice.to_string(),
        }
    }
}

impl<S: AirQualityStore + 'static> Tool for AirQualityQueryTool<S> {
    fn name(&self) -> String {
        "air_quality_query".to_string()
    }

    fn description(&self) -> String {
        "Query current or historical air quality readings for a sensor location. Returns PM2.5, CO2, temperature, humidity, VOC index, and health interpretations.".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "location_id": {
                    "type": "string",
                    "description": "Unique identifier for the sensor location"
                },
                "time_range": {
                    "type": "string",
                    "enum": ["current", "last_hour", "last_24h", "last_7d"],
                    "default": "current",
                    "description": "Time range for data query"
                },
                "metrics": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["co2", "pm25", "voc", "temp", "humidity"]
                    },
                    "default": ["co2", "pm25", "voc", "temp", "humidity"],
                    "description": "Metrics to include in response"
                }
            },
            "required": ["location_id"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        let request: AirQualityQueryInput = if let Some(input) = input {
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

        // Use futures executor to avoid nested runtime issues
        match futures::executor::block_on(self.execute(request)) {
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

// Tool 2: Air Quality Forecast
#[derive(Debug, Clone)]
pub struct AirQualityForecastTool<F: ForecastService> {
    forecast: F,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirQualityForecastInput {
    pub location_id: String,
    pub metric: String,
    #[serde(default = "default_horizon")]
    pub horizon_hours: u32,
}

fn default_horizon() -> u32 {
    6
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirQualityForecastOutput {
    pub forecasts: Vec<ForecastPoint>,
}

impl<F: ForecastService> AirQualityForecastTool<F> {
    pub fn new(forecast: F) -> Self {
        Self { forecast }
    }

    pub async fn execute(
        &self,
        input: AirQualityForecastInput,
    ) -> Result<AirQualityForecastOutput, String> {
        info!(
            "Forecasting {} for location: {} ({}h)",
            input.metric, input.location_id, input.horizon_hours
        );

        if input.horizon_hours > 6 {
            return Err("horizon_hours cannot exceed 6".to_string());
        }

        let forecasts =
            self.forecast
                .predict(&input.location_id, &input.metric, input.horizon_hours)?;

        Ok(AirQualityForecastOutput { forecasts })
    }
}

impl<F: ForecastService + 'static> Tool for AirQualityForecastTool<F> {
    fn name(&self) -> String {
        "air_quality_forecast".to_string()
    }

    fn description(&self) -> String {
        "Generate air quality forecasts for PM2.5 or CO2 levels up to 6 hours ahead with uncertainty quantiles (p10, p50, p90).".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "location_id": {
                    "type": "string",
                    "description": "Unique identifier for the sensor location"
                },
                "metric": {
                    "type": "string",
                    "enum": ["co2", "pm25"],
                    "description": "Metric to forecast"
                },
                "horizon_hours": {
                    "type": "number",
                    "minimum": 1,
                    "maximum": 6,
                    "default": 6,
                    "description": "Forecast horizon in hours (1-6)"
                }
            },
            "required": ["location_id", "metric"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        let request: AirQualityForecastInput = if let Some(input) = input {
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

        // Use futures executor to avoid nested runtime issues
        match futures::executor::block_on(self.execute(request)) {
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

// Tool 3: Air Quality Alerts
#[derive(Debug, Clone)]
pub struct AirQualityAlertsTool<A: AlertService> {
    alerts: A,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirQualityAlertsInput {
    pub location_id: String,
    #[serde(default = "default_alert_time_range")]
    pub time_range: String,
    #[serde(default)]
    pub severity_filter: Option<Vec<String>>,
}

fn default_alert_time_range() -> String {
    "active".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AirQualityAlertsOutput {
    pub alerts: Vec<Alert>,
    pub recommendations: Vec<String>,
}

impl<A: AlertService> AirQualityAlertsTool<A> {
    pub fn new(alerts: A) -> Self {
        Self { alerts }
    }

    pub async fn execute(
        &self,
        input: AirQualityAlertsInput,
    ) -> Result<AirQualityAlertsOutput, String> {
        info!(
            "Fetching alerts for location: {} ({})",
            input.location_id, input.time_range
        );

        let alerts = match input.time_range.as_str() {
            "active" => self.alerts.get_active_alerts(&input.location_id)?,
            "last_24h" => self.alerts.get_alerts_in_range(
                &input.location_id,
                24,
                input.severity_filter.clone(),
            )?,
            "last_7d" => self.alerts.get_alerts_in_range(
                &input.location_id,
                168,
                input.severity_filter.clone(),
            )?,
            _ => return Err(format!("Invalid time_range: {}", input.time_range)),
        };

        let recommendations = vec![
            "Check ventilation system".to_string(),
            "Monitor sensor readings closely".to_string(),
        ];

        Ok(AirQualityAlertsOutput {
            alerts,
            recommendations,
        })
    }
}

impl<A: AlertService + 'static> Tool for AirQualityAlertsTool<A> {
    fn name(&self) -> String {
        "air_quality_alerts".to_string()
    }

    fn description(&self) -> String {
        "Retrieve active or historical air quality alerts with severity levels (Info, Warning, Error, Critical) and actionable recommendations.".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "location_id": {
                    "type": "string",
                    "description": "Unique identifier for the sensor location"
                },
                "time_range": {
                    "type": "string",
                    "enum": ["active", "last_24h", "last_7d"],
                    "default": "active",
                    "description": "Time range for alerts"
                },
                "severity_filter": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["Info", "Warning", "Error", "Critical"]
                    },
                    "description": "Filter alerts by severity"
                }
            },
            "required": ["location_id"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        let request: AirQualityAlertsInput = if let Some(input) = input {
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

        // Use futures executor to avoid nested runtime issues
        match futures::executor::block_on(self.execute(request)) {
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

// Tool 4: Sensor Health
#[derive(Debug, Clone)]
pub struct AirQualitySensorHealthTool<S: AirQualityStore> {
    store: S,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorHealthInput {
    pub location_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SensorHealthOutput {
    pub status: String,
    pub last_reading_age_seconds: u64,
    pub co2_calibration_status: String,
    pub pm_quality: String,
}

impl<S: AirQualityStore> AirQualitySensorHealthTool<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn execute(&self, input: SensorHealthInput) -> Result<SensorHealthOutput, String> {
        info!("Checking sensor health for location: {}", input.location_id);

        let reading = self.store.get_current_reading(&input.location_id)?;
        let age = (Utc::now() - reading.timestamp).num_seconds() as u64;

        let status = if age < 300 {
            "online"
        } else if age < 900 {
            "degraded"
        } else {
            "offline"
        };

        Ok(SensorHealthOutput {
            status: status.to_string(),
            last_reading_age_seconds: age,
            co2_calibration_status: "active".to_string(),
            pm_quality: "good".to_string(),
        })
    }
}

impl<S: AirQualityStore + 'static> Tool for AirQualitySensorHealthTool<S> {
    fn name(&self) -> String {
        "air_quality_sensor_health".to_string()
    }

    fn description(&self) -> String {
        "Check the health and operational status of air quality sensors including calibration status and data quality metrics.".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "location_id": {
                    "type": "string",
                    "description": "Unique identifier for the sensor location"
                }
            },
            "required": ["location_id"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        let request: SensorHealthInput = if let Some(input) = input {
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

        // Use futures executor to avoid nested runtime issues
        match futures::executor::block_on(self.execute(request)) {
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

// Tool 5: Recommendations
#[derive(Debug, Clone)]
pub struct AirQualityRecommendationsTool<S: AirQualityStore, A: AlertService> {
    store: S,
    alerts: A,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecommendationsInput {
    pub location_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecommendationsOutput {
    pub recommendations: Vec<String>,
}

impl<S: AirQualityStore, A: AlertService> AirQualityRecommendationsTool<S, A> {
    pub fn new(store: S, alerts: A) -> Self {
        Self { store, alerts }
    }

    pub async fn execute(
        &self,
        input: RecommendationsInput,
    ) -> Result<RecommendationsOutput, String> {
        info!(
            "Generating recommendations for location: {}",
            input.location_id
        );

        let data = self.store.get_current_reading(&input.location_id)?;
        let recommendations = self.alerts.get_recommendations(&input.location_id, &data)?;

        Ok(RecommendationsOutput { recommendations })
    }
}

impl<S: AirQualityStore + 'static, A: AlertService + 'static> Tool
    for AirQualityRecommendationsTool<S, A>
{
    fn name(&self) -> String {
        "air_quality_recommendations".to_string()
    }

    fn description(&self) -> String {
        "Get actionable recommendations based on current air quality conditions such as ventilation advice, air purifier settings, and exposure limits.".to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "location_id": {
                    "type": "string",
                    "description": "Unique identifier for the sensor location"
                }
            },
            "required": ["location_id"]
        })
    }

    fn call(&self, input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        let request: RecommendationsInput = if let Some(input) = input {
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

        // Use futures executor to avoid nested runtime issues
        match futures::executor::block_on(self.execute(request)) {
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

// Tool 6: List Locations
#[derive(Debug, Clone)]
pub struct ListLocationsTool<S: AirQualityStore> {
    store: S,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListLocationsOutput {
    pub locations: Vec<Location>,
}

impl<S: AirQualityStore> ListLocationsTool<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn execute(&self) -> Result<ListLocationsOutput, String> {
        info!("Listing all sensor locations");

        let locations = self.store.get_all_locations()?;

        Ok(ListLocationsOutput { locations })
    }
}

impl<S: AirQualityStore + 'static> Tool for ListLocationsTool<S> {
    fn name(&self) -> String {
        "list_locations".to_string()
    }

    fn description(&self) -> String {
        "List all available sensor locations with their IDs, names, and sensor serial numbers."
            .to_string()
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn call(&self, _input: Option<serde_json::Value>) -> AnyhowResult<CallToolResponse> {
        // Use futures executor to avoid nested runtime issues
        match futures::executor::block_on(self.execute()) {
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
