use crate::{response::ApiResponse, ApiResult};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct AlertsQuery {
    pub location_id: String,
    #[serde(default = "default_time_range")]
    pub time_range: String, // active, last_24h, last_7d
}

fn default_time_range() -> String {
    "active".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AlertStatus {
    Active,
    Resolved,
    Acknowledged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub location_id: String,
    pub metric: String,
    pub severity: AlertSeverity,
    pub status: AlertStatus,
    pub threshold: f64,
    pub current_value: f64,
    pub triggered_at: String,
    pub resolved_at: Option<String>,
    pub message: String,
}

pub struct AlertStore {
    alerts: Vec<Alert>,
}

impl AlertStore {
    pub fn new() -> Self {
        Self { alerts: Vec::new() }
    }

    pub fn add_alert(&mut self, alert: Alert) {
        self.alerts.push(alert);
    }

    pub fn get_alerts(&self, location_id: &str, time_range: &str) -> Vec<Alert> {
        let now = chrono::Utc::now();
        let filter_time = match time_range {
            "active" => {
                return self
                    .alerts
                    .iter()
                    .filter(|a| a.location_id == location_id && a.status == AlertStatus::Active)
                    .cloned()
                    .collect()
            }
            "last_24h" => now - chrono::Duration::hours(24),
            "last_7d" => now - chrono::Duration::days(7),
            _ => return Vec::new(),
        };

        self.alerts
            .iter()
            .filter(|a| {
                a.location_id == location_id
                    && chrono::DateTime::parse_from_rfc3339(&a.triggered_at)
                        .map(|t| t.with_timezone(&chrono::Utc) >= filter_time)
                        .unwrap_or(false)
            })
            .cloned()
            .collect()
    }
}

pub async fn alerts_handler(
    State(alert_store): State<Arc<AlertStore>>,
    Query(query): Query<AlertsQuery>,
) -> ApiResult<Json<ApiResponse<Vec<Alert>>>> {
    let alerts = alert_store.get_alerts(&query.location_id, &query.time_range);
    Ok(Json(ApiResponse::success(alerts)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_alert(
        id: &str,
        location_id: &str,
        status: AlertStatus,
        severity: AlertSeverity,
        hours_ago: i64,
    ) -> Alert {
        let triggered_at = chrono::Utc::now() - chrono::Duration::hours(hours_ago);
        Alert {
            id: id.to_string(),
            location_id: location_id.to_string(),
            metric: "pm25".to_string(),
            severity,
            status,
            threshold: 35.0,
            current_value: 40.0,
            triggered_at: triggered_at.to_rfc3339(),
            resolved_at: None,
            message: "PM2.5 levels exceeded threshold".to_string(),
        }
    }

    #[tokio::test]
    async fn test_alerts_handler_active_only() {
        let mut store = AlertStore::new();
        store.add_alert(create_test_alert(
            "alert-1",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Warning,
            1,
        ));
        store.add_alert(create_test_alert(
            "alert-2",
            "test-loc",
            AlertStatus::Resolved,
            AlertSeverity::Info,
            2,
        ));
        store.add_alert(create_test_alert(
            "alert-3",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Critical,
            3,
        ));

        let query = AlertsQuery {
            location_id: "test-loc".to_string(),
            time_range: "active".to_string(),
        };

        let result = alerts_handler(State(Arc::new(store)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 2);
        assert!(response
            .data
            .iter()
            .all(|a| a.status == AlertStatus::Active));
    }

    #[tokio::test]
    async fn test_alerts_handler_last_24h() {
        let mut store = AlertStore::new();
        store.add_alert(create_test_alert(
            "alert-1",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Warning,
            1,
        ));
        store.add_alert(create_test_alert(
            "alert-2",
            "test-loc",
            AlertStatus::Resolved,
            AlertSeverity::Info,
            12,
        ));
        store.add_alert(create_test_alert(
            "alert-3",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Critical,
            48, // 2 days ago
        ));

        let query = AlertsQuery {
            location_id: "test-loc".to_string(),
            time_range: "last_24h".to_string(),
        };

        let result = alerts_handler(State(Arc::new(store)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 2);
        assert!(response.data.iter().all(|a| a.id != "alert-3"));
    }

    #[tokio::test]
    async fn test_alerts_handler_last_7d() {
        let mut store = AlertStore::new();
        store.add_alert(create_test_alert(
            "alert-1",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Warning,
            24,
        ));
        store.add_alert(create_test_alert(
            "alert-2",
            "test-loc",
            AlertStatus::Resolved,
            AlertSeverity::Info,
            72,
        ));
        store.add_alert(create_test_alert(
            "alert-3",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Critical,
            200, // > 7 days
        ));

        let query = AlertsQuery {
            location_id: "test-loc".to_string(),
            time_range: "last_7d".to_string(),
        };

        let result = alerts_handler(State(Arc::new(store)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 2);
        assert!(response.data.iter().all(|a| a.id != "alert-3"));
    }

    #[tokio::test]
    async fn test_alerts_handler_different_locations() {
        let mut store = AlertStore::new();
        store.add_alert(create_test_alert(
            "alert-1",
            "location-a",
            AlertStatus::Active,
            AlertSeverity::Warning,
            1,
        ));
        store.add_alert(create_test_alert(
            "alert-2",
            "location-b",
            AlertStatus::Active,
            AlertSeverity::Info,
            2,
        ));

        let query = AlertsQuery {
            location_id: "location-a".to_string(),
            time_range: "active".to_string(),
        };

        let result = alerts_handler(State(Arc::new(store)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].id, "alert-1");
        assert_eq!(response.data[0].location_id, "location-a");
    }

    #[tokio::test]
    async fn test_alerts_handler_severity_filtering() {
        let mut store = AlertStore::new();
        store.add_alert(create_test_alert(
            "alert-1",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Critical,
            1,
        ));
        store.add_alert(create_test_alert(
            "alert-2",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Warning,
            1,
        ));
        store.add_alert(create_test_alert(
            "alert-3",
            "test-loc",
            AlertStatus::Active,
            AlertSeverity::Info,
            1,
        ));

        let query = AlertsQuery {
            location_id: "test-loc".to_string(),
            time_range: "active".to_string(),
        };

        let result = alerts_handler(State(Arc::new(store)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 3);

        let critical = response.data.iter().find(|a| a.id == "alert-1").unwrap();
        assert_eq!(critical.severity, AlertSeverity::Critical);
    }

    #[tokio::test]
    async fn test_alerts_handler_empty_result() {
        let store = AlertStore::new();

        let query = AlertsQuery {
            location_id: "nonexistent".to_string(),
            time_range: "active".to_string(),
        };

        let result = alerts_handler(State(Arc::new(store)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 0);
    }
}
