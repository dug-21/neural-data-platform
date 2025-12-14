use crate::api::handlers::{
    alerts_handler, forecast_handler, health_handler, latest_readings_handler, locations_handler,
    readings_handler, aggregate_handler,
};
use crate::api::handlers::alerts::AlertStore;
use crate::api::handlers::health::AppState;
use crate::api::handlers::locations::LocationStore;
use axum::{
    routing::get,
    Router,
};
use neural_core::{Forecast, Source, Store};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub struct AppServices {
    pub store: Arc<dyn Store>,
    pub source: Arc<dyn Source>,
    pub forecast: Arc<dyn Forecast>,
    pub alert_store: Arc<AlertStore>,
    pub location_store: Arc<LocationStore>,
}

pub fn create_router(services: AppServices) -> Router {
    let health_state = Arc::new(AppState {
        store: services.store.clone(),
        source: services.source.clone(),
        start_time: chrono::Utc::now(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create subrouters with their specific states
    let health_router = Router::new()
        .route("/health", get(health_handler))
        .with_state(health_state);

    let readings_router = Router::new()
        .route("/api/v1/readings/latest", get(latest_readings_handler))
        .route("/api/v1/readings", get(readings_handler))
        .route("/api/v1/aggregate", get(aggregate_handler))
        .with_state(services.store);

    let forecast_router = Router::new()
        .route("/api/v1/forecast", get(forecast_handler))
        .with_state(services.forecast);

    let alerts_router = Router::new()
        .route("/api/v1/alerts", get(alerts_handler))
        .with_state(services.alert_store);

    let locations_router = Router::new()
        .route("/api/v1/locations", get(locations_handler))
        .with_state(services.location_store);

    // Merge all routers
    Router::new()
        .merge(health_router)
        .merge(readings_router)
        .merge(forecast_router)
        .merge(alerts_router)
        .merge(locations_router)
        .layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers::alerts::{Alert, AlertSeverity, AlertStatus};
    use crate::api::handlers::locations::Location;
    use axum_test::TestServer;
    use neural_core::{
        AggregatedPoint, AggregationType, CoreError, ForecastedPoint, HealthStatus, ModelMetrics,
        TimeSeriesPoint,
    };
    use mockall::mock;
    use std::collections::HashMap;

    mock! {
        pub TestStore {}

        #[async_trait::async_trait]
        impl Store for TestStore {
            async fn write(&self, point: TimeSeriesPoint) -> Result<(), CoreError>;
            async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<(), CoreError>;
            async fn query(
                &self,
                location_id: &str,
                start: chrono::DateTime<chrono::Utc>,
                end: chrono::DateTime<chrono::Utc>,
                filters: Option<HashMap<String, String>>,
            ) -> Result<Vec<TimeSeriesPoint>, CoreError>;
            async fn aggregate(
                &self,
                location_id: &str,
                start: chrono::DateTime<chrono::Utc>,
                end: chrono::DateTime<chrono::Utc>,
                aggregation: AggregationType,
                interval: chrono::Duration,
            ) -> Result<Vec<AggregatedPoint>, CoreError>;
            async fn health_check(&self) -> Result<HealthStatus, CoreError>;
        }
    }

    mock! {
        pub TestSource {}

        #[async_trait::async_trait]
        impl Source for TestSource {
            async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError>;
            async fn health_check(&self) -> Result<HealthStatus, CoreError>;
        }
    }

    mock! {
        pub TestForecast {}

        #[async_trait::async_trait]
        impl Forecast for TestForecast {
            async fn train(&mut self, data: Vec<TimeSeriesPoint>) -> Result<ModelMetrics, CoreError>;
            async fn predict(&self, location_id: &str, horizon: usize) -> Result<Vec<ForecastedPoint>, CoreError>;
            async fn evaluate(&self, actual: Vec<TimeSeriesPoint>) -> Result<ModelMetrics, CoreError>;
        }
    }

    fn create_test_services() -> AppServices {
        let mut mock_store = MockTestStore::new();
        let mut mock_source = MockTestSource::new();
        let mock_forecast = MockTestForecast::new();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
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

        let mut alert_store = AlertStore::new();
        alert_store.add_alert(Alert {
            id: "alert-1".to_string(),
            location_id: "test-loc".to_string(),
            metric: "pm25".to_string(),
            severity: AlertSeverity::Warning,
            status: AlertStatus::Active,
            threshold: 35.0,
            current_value: 40.0,
            triggered_at: chrono::Utc::now().to_rfc3339(),
            resolved_at: None,
            message: "PM2.5 elevated".to_string(),
        });

        let mut location_store = LocationStore::new();
        location_store.add_location(Location {
            id: "test-loc".to_string(),
            name: "Test Location".to_string(),
            latitude: 37.7749,
            longitude: -122.4194,
            device_type: "AirGradient ONE".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
        });

        AppServices {
            store: Arc::new(mock_store),
            source: Arc::new(mock_source),
            forecast: Arc::new(mock_forecast),
            alert_store: Arc::new(alert_store),
            location_store: Arc::new(location_store),
        }
    }

    #[tokio::test]
    async fn test_health_endpoint_success() {
        let services = create_test_services();
        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/health").await;

        response.assert_status_ok();
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "success");
        assert_eq!(json["data"]["status"], "healthy");
    }

    #[tokio::test]
    async fn test_locations_endpoint() {
        let services = create_test_services();
        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/v1/locations").await;

        response.assert_status_ok();
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "success");
        assert!(json["data"].is_array());
        assert_eq!(json["data"][0]["id"], "test-loc");
    }

    #[tokio::test]
    async fn test_alerts_endpoint() {
        let services = create_test_services();
        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/api/v1/alerts?location_id=test-loc&time_range=active")
            .await;

        response.assert_status_ok();
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "success");
        assert!(json["data"].is_array());
    }

    #[tokio::test]
    async fn test_cors_headers() {
        let services = create_test_services();
        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/health").await;

        assert!(response.headers().contains_key("access-control-allow-origin"));
    }

    #[tokio::test]
    async fn test_not_found_route() {
        let services = create_test_services();
        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/api/v1/nonexistent").await;

        response.assert_status_not_found();
    }

    #[tokio::test]
    async fn test_latest_readings_endpoint_with_data() {
        let mut mock_store = MockTestStore::new();
        let mut mock_source = MockTestSource::new();
        let now = chrono::Utc::now();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
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

        mock_store
            .expect_query()
            .returning(move |_loc, _start, _end, _filters| {
                Ok(vec![TimeSeriesPoint {
                    timestamp: now,
                    location_id: "test-loc".to_string(),
                    value: 25.0,
                    tags: HashMap::new(),
                }])
            });

        let services = AppServices {
            store: Arc::new(mock_store),
            source: Arc::new(mock_source),
            forecast: Arc::new(MockTestForecast::new()),
            alert_store: Arc::new(AlertStore::new()),
            location_store: Arc::new(LocationStore::new()),
        };

        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/api/v1/readings/latest?location_id=test-loc")
            .await;

        response.assert_status_ok();
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "success");
        assert_eq!(json["data"]["value"], 25.0);
    }

    #[tokio::test]
    async fn test_readings_time_range_query() {
        let mut mock_store = MockTestStore::new();
        let mut mock_source = MockTestSource::new();
        let now = chrono::Utc::now();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
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

        mock_store
            .expect_query()
            .returning(move |_loc, _start, _end, _filters| {
                Ok(vec![
                    TimeSeriesPoint {
                        timestamp: now - chrono::Duration::hours(2),
                        location_id: "test-loc".to_string(),
                        value: 20.0,
                        tags: HashMap::new(),
                    },
                    TimeSeriesPoint {
                        timestamp: now - chrono::Duration::hours(1),
                        location_id: "test-loc".to_string(),
                        value: 25.0,
                        tags: HashMap::new(),
                    },
                ])
            });

        let services = AppServices {
            store: Arc::new(mock_store),
            source: Arc::new(mock_source),
            forecast: Arc::new(MockTestForecast::new()),
            alert_store: Arc::new(AlertStore::new()),
            location_store: Arc::new(LocationStore::new()),
        };

        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let start = (now - chrono::Duration::hours(3)).to_rfc3339();
        let end = now.to_rfc3339();
        let url = format!(
            "/api/v1/readings?location_id=test-loc&start={}&end={}",
            urlencoding::encode(&start),
            urlencoding::encode(&end)
        );

        let response = server.get(&url).await;

        response.assert_status_ok();
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "success");
        assert!(json["data"].is_array());
        assert_eq!(json["data"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_aggregate_endpoint_mean() {
        let mut mock_store = MockTestStore::new();
        let mut mock_source = MockTestSource::new();
        let now = chrono::Utc::now();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
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

        mock_store
            .expect_aggregate()
            .returning(move |_loc, _start, _end, _agg, _interval| {
                Ok(vec![AggregatedPoint {
                    timestamp: now,
                    location_id: "test-loc".to_string(),
                    value: 22.5,
                    aggregation_type: AggregationType::Mean,
                }])
            });

        let services = AppServices {
            store: Arc::new(mock_store),
            source: Arc::new(mock_source),
            forecast: Arc::new(MockTestForecast::new()),
            alert_store: Arc::new(AlertStore::new()),
            location_store: Arc::new(LocationStore::new()),
        };

        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let start = (now - chrono::Duration::hours(1)).to_rfc3339();
        let end = now.to_rfc3339();
        let url = format!(
            "/api/v1/aggregate?location_id=test-loc&start={}&end={}&interval=5m&agg=mean",
            urlencoding::encode(&start),
            urlencoding::encode(&end)
        );

        let response = server.get(&url).await;

        response.assert_status_ok();
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "success");
        assert_eq!(json["data"][0]["value"], 22.5);
    }

    #[tokio::test]
    async fn test_forecast_endpoint() {
        let mut mock_store = MockTestStore::new();
        let mut mock_source = MockTestSource::new();
        let mut mock_forecast = MockTestForecast::new();
        let now = chrono::Utc::now();

        mock_source.expect_health_check().returning(|| {
            Ok(HealthStatus {
                healthy: true,
                message: "OK".to_string(),
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

        mock_forecast
            .expect_predict()
            .returning(move |_loc, horizon| {
                let mut predictions = Vec::new();
                for i in 0..horizon {
                    predictions.push(ForecastedPoint {
                        timestamp: now + chrono::Duration::hours(i as i64),
                        location_id: "test-loc".to_string(),
                        predicted_value: 25.0 + i as f64,
                        confidence_lower: 20.0,
                        confidence_upper: 30.0,
                    });
                }
                Ok(predictions)
            });

        let services = AppServices {
            store: Arc::new(mock_store),
            source: Arc::new(mock_source),
            forecast: Arc::new(mock_forecast),
            alert_store: Arc::new(AlertStore::new()),
            location_store: Arc::new(LocationStore::new()),
        };

        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/api/v1/forecast?location_id=test-loc&metric=pm25&horizon=6")
            .await;

        response.assert_status_ok();
        let json: serde_json::Value = response.json();
        assert_eq!(json["status"], "success");
        assert_eq!(json["data"]["horizon"], 6);
        assert_eq!(json["data"]["predictions"].as_array().unwrap().len(), 6);
    }

    #[tokio::test]
    async fn test_invalid_query_params() {
        let services = create_test_services();
        let app = create_router(services);
        let server = TestServer::new(app).unwrap();

        // Missing required location_id parameter
        let response = server.get("/api/v1/alerts").await;

        response.assert_status_bad_request();
    }
}
