use air_quality_app::{api::create_router, config::AppConfig};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "air_quality_app=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = match AppConfig::from_yaml("config.yaml") {
        Ok(cfg) => {
            tracing::info!("Loaded configuration from config.yaml");
            cfg
        }
        Err(e) => {
            tracing::warn!("Failed to load config.yaml: {}, using defaults", e);
            AppConfig::default_config()
        }
    };

    tracing::info!(
        "Starting air quality server on {}:{}",
        config.server.host,
        config.server.port
    );

    // For now, we'll use mock implementations
    // In production, these would be real implementations
    let services = create_mock_services();

    // Create router
    let app = create_router(services);

    // Create TCP listener
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Listening on {}", addr);

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
}

fn create_mock_services() -> air_quality_app::api::routes::AppServices {
    use air_quality_app::api::handlers::alerts::AlertStore;
    use air_quality_app::api::handlers::locations::LocationStore;
    use neural_core::{Forecast, Source, Store};
    use std::sync::Arc;

    // Create mock implementations
    struct MockStore;
    struct MockSource;
    struct MockForecast;

    #[async_trait::async_trait]
    impl Store for MockStore {
        async fn write(&self, _point: neural_core::TimeSeriesPoint) -> Result<(), neural_core::CoreError> {
            Ok(())
        }

        async fn write_batch(
            &self,
            _points: Vec<neural_core::TimeSeriesPoint>,
        ) -> Result<(), neural_core::CoreError> {
            Ok(())
        }

        async fn query(
            &self,
            _location_id: &str,
            _start: chrono::DateTime<chrono::Utc>,
            _end: chrono::DateTime<chrono::Utc>,
            _filters: Option<std::collections::HashMap<String, String>>,
        ) -> Result<Vec<neural_core::TimeSeriesPoint>, neural_core::CoreError> {
            Ok(vec![])
        }

        async fn aggregate(
            &self,
            _location_id: &str,
            _start: chrono::DateTime<chrono::Utc>,
            _end: chrono::DateTime<chrono::Utc>,
            _aggregation: neural_core::AggregationType,
            _interval: chrono::Duration,
        ) -> Result<Vec<neural_core::AggregatedPoint>, neural_core::CoreError> {
            Ok(vec![])
        }

        async fn health_check(&self) -> Result<neural_core::HealthStatus, neural_core::CoreError> {
            Ok(neural_core::HealthStatus {
                healthy: true,
                message: "Mock store operational".to_string(),
                details: std::collections::HashMap::new(),
            })
        }
    }

    #[async_trait::async_trait]
    impl Source for MockSource {
        async fn fetch(&self) -> Result<Vec<neural_core::TimeSeriesPoint>, neural_core::CoreError> {
            Ok(vec![])
        }

        async fn health_check(&self) -> Result<neural_core::HealthStatus, neural_core::CoreError> {
            Ok(neural_core::HealthStatus {
                healthy: true,
                message: "Mock source operational".to_string(),
                details: std::collections::HashMap::new(),
            })
        }
    }

    #[async_trait::async_trait]
    impl Forecast for MockForecast {
        async fn train(
            &mut self,
            _data: Vec<neural_core::TimeSeriesPoint>,
        ) -> Result<neural_core::ModelMetrics, neural_core::CoreError> {
            Ok(neural_core::ModelMetrics {
                mae: 0.0,
                rmse: 0.0,
                mape: 0.0,
            })
        }

        async fn predict(
            &self,
            _location_id: &str,
            _horizon: usize,
        ) -> Result<Vec<neural_core::ForecastedPoint>, neural_core::CoreError> {
            Ok(vec![])
        }

        async fn evaluate(
            &self,
            _actual: Vec<neural_core::TimeSeriesPoint>,
        ) -> Result<neural_core::ModelMetrics, neural_core::CoreError> {
            Ok(neural_core::ModelMetrics {
                mae: 0.0,
                rmse: 0.0,
                mape: 0.0,
            })
        }
    }

    air_quality_app::api::routes::AppServices {
        store: Arc::new(MockStore),
        source: Arc::new(MockSource),
        forecast: Arc::new(MockForecast),
        alert_store: Arc::new(AlertStore::new()),
        location_store: Arc::new(LocationStore::new()),
    }
}
