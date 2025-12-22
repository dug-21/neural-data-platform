use crate::{response::ApiResponse, ApiResult};
use axum::{extract::State, Json};
use neural_core::{HealthStatus, Source, Store};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ComponentStatus {
    Connected,
    Disconnected,
    Ok,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub mqtt: ComponentStatus,
    pub storage: ComponentStatus,
    pub last_reading_age_seconds: u64,
}

pub struct AppState {
    pub store: Arc<dyn Store>,
    pub source: Arc<dyn Source>,
    pub start_time: chrono::DateTime<chrono::Utc>,
}

pub async fn health_handler(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ApiResponse<HealthResponse>>> {
    // Check MQTT/Source health
    let mqtt_health = state.source.health_check().await;
    let mqtt_status = match mqtt_health {
        Ok(HealthStatus { healthy: true, .. }) => ComponentStatus::Connected,
        _ => ComponentStatus::Disconnected,
    };

    // Check Storage health
    let storage_health = state.store.health_check().await;
    let storage_status = match storage_health {
        Ok(HealthStatus { healthy: true, .. }) => ComponentStatus::Ok,
        _ => ComponentStatus::Error,
    };

    // Calculate last reading age
    let now = chrono::Utc::now();
    let age_seconds = (now - state.start_time).num_seconds().max(0) as u64;

    // Determine overall status
    let overall_status =
        if mqtt_status == ComponentStatus::Connected && storage_status == ComponentStatus::Ok {
            "healthy"
        } else if storage_status == ComponentStatus::Error {
            "unhealthy"
        } else {
            "degraded"
        };

    let health = HealthResponse {
        status: overall_status.to_string(),
        mqtt: mqtt_status,
        storage: storage_status,
        last_reading_age_seconds: age_seconds,
    };

    Ok(Json(ApiResponse::success(health)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use neural_core::{CoreError, HealthStatus};
    use std::collections::HashMap;

    mock! {
        pub Store {}

        #[async_trait::async_trait]
        impl Store for Store {
            async fn write(&self, point: neural_core::TimeSeriesPoint) -> Result<(), CoreError>;
            async fn write_batch(&self, points: Vec<neural_core::TimeSeriesPoint>) -> Result<(), CoreError>;
            async fn query(
                &self,
                location_id: &str,
                start: chrono::DateTime<chrono::Utc>,
                end: chrono::DateTime<chrono::Utc>,
                filters: Option<HashMap<String, String>>,
            ) -> Result<Vec<neural_core::TimeSeriesPoint>, CoreError>;
            async fn aggregate(
                &self,
                location_id: &str,
                start: chrono::DateTime<chrono::Utc>,
                end: chrono::DateTime<chrono::Utc>,
                aggregation: neural_core::AggregationType,
                interval: chrono::Duration,
            ) -> Result<Vec<neural_core::AggregatedPoint>, CoreError>;
            async fn health_check(&self) -> Result<HealthStatus, CoreError>;
        }
    }

    mock! {
        pub Source {}

        #[async_trait::async_trait]
        impl Source for Source {
            async fn fetch(&self) -> Result<Vec<neural_core::TimeSeriesPoint>, CoreError>;
            async fn health_check(&self) -> Result<HealthStatus, CoreError>;
        }
    }

    fn create_app_state(store: MockStore, source: MockSource) -> Arc<AppState> {
        Arc::new(AppState {
            store: Arc::new(store),
            source: Arc::new(source),
            start_time: chrono::Utc::now() - chrono::Duration::seconds(100),
        })
    }

    #[tokio::test]
    async fn test_health_endpoint_all_healthy() {
        let mut mock_store = MockStore::new();
        let mut mock_source = MockSource::new();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "Connected".to_string(),
                details: HashMap::new(),
            })
        });

        mock_store.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
                details: HashMap::new(),
            })
        });

        let state = create_app_state(mock_store, mock_source);
        let result = health_handler(State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.status, "success");
        assert_eq!(response.data.status, "healthy");
        assert_eq!(response.data.mqtt, ComponentStatus::Connected);
        assert_eq!(response.data.storage, ComponentStatus::Ok);
        assert!(response.data.last_reading_age_seconds >= 100);
    }

    #[tokio::test]
    async fn test_health_endpoint_degraded_mqtt_down() {
        let mut mock_store = MockStore::new();
        let mut mock_source = MockSource::new();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: false,
                message: "Disconnected".to_string(),
                details: HashMap::new(),
            })
        });

        mock_store.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
                details: HashMap::new(),
            })
        });

        let state = create_app_state(mock_store, mock_source);
        let result = health_handler(State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.status, "degraded");
        assert_eq!(response.data.mqtt, ComponentStatus::Disconnected);
        assert_eq!(response.data.storage, ComponentStatus::Ok);
    }

    #[tokio::test]
    async fn test_health_endpoint_unhealthy_storage_error() {
        let mut mock_store = MockStore::new();
        let mut mock_source = MockSource::new();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "Connected".to_string(),
                details: HashMap::new(),
            })
        });

        mock_store
            .expect_health_check()
            .returning(|| Err(CoreError::DatabaseError("Connection failed".to_string())));

        let state = create_app_state(mock_store, mock_source);
        let result = health_handler(State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.status, "unhealthy");
        assert_eq!(response.data.mqtt, ComponentStatus::Connected);
        assert_eq!(response.data.storage, ComponentStatus::Error);
    }

    #[tokio::test]
    async fn test_health_endpoint_last_reading_age_calculation() {
        let mut mock_store = MockStore::new();
        let mut mock_source = MockSource::new();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "Connected".to_string(),
                details: HashMap::new(),
            })
        });

        mock_store.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
                details: HashMap::new(),
            })
        });

        let state = Arc::new(AppState {
            store: Arc::new(mock_store),
            source: Arc::new(mock_source),
            start_time: chrono::Utc::now() - chrono::Duration::seconds(500),
        });

        let result = health_handler(State(state)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert!(response.data.last_reading_age_seconds >= 500);
    }
}
