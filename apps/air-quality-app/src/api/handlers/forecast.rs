use crate::{error::ApiError, response::ApiResponse, ApiResult};
use axum::{
    extract::{Query, State},
    Json,
};
use neural_core::{Forecast, ForecastedPoint};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ForecastQuery {
    pub location_id: String,
    pub metric: String, // pm25, co2
    pub horizon: usize, // 1-24 hours
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastResponse {
    pub location_id: String,
    pub metric: String,
    pub horizon: usize,
    pub predictions: Vec<ForecastPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastPoint {
    pub timestamp: String,
    pub predicted_value: f64,
    pub confidence_lower: f64,
    pub confidence_upper: f64,
}

impl From<ForecastedPoint> for ForecastPoint {
    fn from(point: ForecastedPoint) -> Self {
        Self {
            timestamp: point.timestamp.to_rfc3339(),
            predicted_value: point.predicted_value,
            confidence_lower: point.confidence_lower,
            confidence_upper: point.confidence_upper,
        }
    }
}

pub async fn forecast_handler(
    State(forecast): State<Arc<dyn Forecast>>,
    Query(query): Query<ForecastQuery>,
) -> ApiResult<Json<ApiResponse<ForecastResponse>>> {
    if query.horizon == 0 || query.horizon > 24 {
        return Err(ApiError::BadRequest(
            "Horizon must be between 1 and 24 hours".to_string(),
        ));
    }

    let predictions = forecast
        .predict(&query.location_id, query.horizon)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    let forecast_points: Vec<ForecastPoint> =
        predictions.into_iter().map(ForecastPoint::from).collect();

    let response = ForecastResponse {
        location_id: query.location_id,
        metric: query.metric,
        horizon: query.horizon,
        predictions: forecast_points,
    };

    Ok(Json(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::{CoreError, ModelMetrics, TimeSeriesPoint};
    use mockall::mock;

    mock! {
        pub TestForecast {}

        #[async_trait::async_trait]
        impl Forecast for TestForecast {
            async fn train(&mut self, data: Vec<TimeSeriesPoint>) -> Result<ModelMetrics, CoreError>;
            async fn predict(&self, location_id: &str, horizon: usize) -> Result<Vec<ForecastedPoint>, CoreError>;
            async fn evaluate(&self, actual: Vec<TimeSeriesPoint>) -> Result<ModelMetrics, CoreError>;
        }
    }

    #[tokio::test]
    async fn test_forecast_handler_success() {
        let mut mock = MockTestForecast::new();
        let now = chrono::Utc::now();

        mock.expect_predict()
            .with(mockall::predicate::eq("test-loc"), mockall::predicate::eq(6))
            .returning(move |_loc, horizon| {
                let mut predictions = Vec::new();
                for i in 0..horizon {
                    predictions.push(ForecastedPoint {
                        timestamp: now + chrono::Duration::hours(i as i64),
                        location_id: "test-loc".to_string(),
                        predicted_value: 25.0 + i as f64,
                        confidence_lower: 20.0 + i as f64,
                        confidence_upper: 30.0 + i as f64,
                    });
                }
                Ok(predictions)
            });

        let query = ForecastQuery {
            location_id: "test-loc".to_string(),
            metric: "pm25".to_string(),
            horizon: 6,
        };

        let result = forecast_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.location_id, "test-loc");
        assert_eq!(response.data.metric, "pm25");
        assert_eq!(response.data.horizon, 6);
        assert_eq!(response.data.predictions.len(), 6);
        assert_eq!(response.data.predictions[0].predicted_value, 25.0);
    }

    #[tokio::test]
    async fn test_forecast_handler_confidence_intervals() {
        let mut mock = MockTestForecast::new();
        let now = chrono::Utc::now();

        mock.expect_predict().returning(move |_loc, _horizon| {
            Ok(vec![ForecastedPoint {
                timestamp: now + chrono::Duration::hours(1),
                location_id: "test-loc".to_string(),
                predicted_value: 50.0,
                confidence_lower: 45.0,
                confidence_upper: 55.0,
            }])
        });

        let query = ForecastQuery {
            location_id: "test-loc".to_string(),
            metric: "co2".to_string(),
            horizon: 1,
        };

        let result = forecast_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.predictions[0].predicted_value, 50.0);
        assert_eq!(response.data.predictions[0].confidence_lower, 45.0);
        assert_eq!(response.data.predictions[0].confidence_upper, 55.0);
    }

    #[tokio::test]
    async fn test_forecast_handler_invalid_horizon_zero() {
        let mock = MockTestForecast::new();

        let query = ForecastQuery {
            location_id: "test-loc".to_string(),
            metric: "pm25".to_string(),
            horizon: 0,
        };

        let result = forecast_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_err());
        match result {
            Err(ApiError::BadRequest(msg)) => assert!(msg.contains("between 1 and 24")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_forecast_handler_invalid_horizon_too_large() {
        let mock = MockTestForecast::new();

        let query = ForecastQuery {
            location_id: "test-loc".to_string(),
            metric: "pm25".to_string(),
            horizon: 25,
        };

        let result = forecast_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_err());
        match result {
            Err(ApiError::BadRequest(msg)) => assert!(msg.contains("between 1 and 24")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_forecast_handler_service_error() {
        let mut mock = MockTestForecast::new();

        mock.expect_predict()
            .returning(|_loc, _horizon| Err(CoreError::PredictionError("Model error".to_string())));

        let query = ForecastQuery {
            location_id: "test-loc".to_string(),
            metric: "pm25".to_string(),
            horizon: 12,
        };

        let result = forecast_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_err());
        match result {
            Err(ApiError::InternalError(_)) => {}
            _ => panic!("Expected InternalError"),
        }
    }
}
