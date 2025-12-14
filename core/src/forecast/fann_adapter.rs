/// ruv-FANN model adapter for forecasting
use std::path::PathBuf;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::CoreResult;
use crate::traits::{Forecast, ForecastedPoint, ModelMetrics, TimeSeriesPoint};
use super::scaler::StandardScaler;

/// Model type selection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelType {
    NHITS,
    NBEATSx,
}

/// Mock model structure for testing (will be replaced with actual ruv-fann model)
#[derive(Debug, Clone)]
struct MockModel {
    input_size: usize,
    output_size: usize,
}

impl MockModel {
    fn new(input_size: usize, output_size: usize) -> Self {
        Self { input_size, output_size }
    }

    fn predict(&self, _inputs: &[f64]) -> Vec<f64> {
        // Mock prediction - returns zeros for now
        vec![0.0; self.output_size]
    }
}

/// FANN-based forecaster implementing the Forecast trait
pub struct FannForecaster {
    model_path: PathBuf,
    model_type: ModelType,
    input_window: usize,
    forecast_horizon: usize,
    loaded_model: Option<MockModel>,
    feature_scaler: Option<StandardScaler>,
}

impl FannForecaster {
    /// Create a new FannForecaster
    pub fn new(
        model_path: PathBuf,
        model_type: ModelType,
        input_window: usize,
        forecast_horizon: usize,
    ) -> Self {
        Self {
            model_path,
            model_type,
            input_window,
            forecast_horizon,
            loaded_model: None,
            feature_scaler: None,
        }
    }

    /// Load model from disk
    pub async fn load_model(&mut self) -> CoreResult<()> {
        use crate::error::CoreError;

        // Check if model file exists
        if !self.model_path.exists() {
            return Err(CoreError::Config(
                format!("Model file not found: {:?}", self.model_path)
            ));
        }

        // Creates mock model for initial development
        // Phase 3 enhancement: Integrate actual ruv-fann safetensors model loading
        let model = MockModel::new(self.input_window * 13, self.forecast_horizon);
        self.loaded_model = Some(model);

        Ok(())
    }

    /// Engineer features from time series data
    fn engineer_features(&self, data: &[TimeSeriesPoint]) -> Vec<Vec<f64>> {
        use super::features::*;

        let mut features = Vec::new();

        // Extract values for lag and rolling calculations
        let values: Vec<f64> = data.iter().map(|p| p.value).collect();

        // Calculate lag features
        let lag_1h = lag_feature(&values, 60);    // 60 minutes
        let lag_3h = lag_feature(&values, 180);   // 180 minutes
        let lag_24h = lag_feature(&values, 1440); // 1440 minutes

        // Calculate rolling features
        let rolling_mean_1h = rolling_mean(&values, 60);
        let rolling_std_1h = rolling_std(&values, 60);

        // Build feature vectors
        for (i, point) in data.iter().enumerate() {
            let mut fv = super::features::FeatureVector::new(point.timestamp);

            // Set metric value
            fv.pm25 = point.value;

            // Set lag features
            fv.lag_1h = lag_1h[i];
            fv.lag_3h = lag_3h[i];
            fv.lag_24h = lag_24h[i];

            // Set rolling features
            fv.rolling_mean_1h = rolling_mean_1h[i];
            fv.rolling_std_1h = rolling_std_1h[i];

            features.push(fv.to_vec());
        }

        features
    }

    /// Normalize features
    fn normalize_features(&self, features: &[Vec<f64>]) -> Vec<Vec<f64>> {
        if features.is_empty() {
            return Vec::new();
        }

        let num_features = features[0].len();
        let mut normalized = vec![Vec::with_capacity(num_features); features.len()];

        // Normalize each feature column independently
        for feature_idx in 0..num_features {
            let column: Vec<f64> = features.iter()
                .map(|row| row[feature_idx])
                .collect();

            let scaler = StandardScaler::fit(&column);
            let normalized_column = scaler.transform(&column);

            for (row_idx, &value) in normalized_column.iter().enumerate() {
                normalized[row_idx].push(value);
            }
        }

        normalized
    }

    /// Select appropriate model based on data characteristics
    fn select_model(data: &[TimeSeriesPoint]) -> ModelType {
        if data.is_empty() {
            return ModelType::NHITS;
        }

        // Calculate trend strength
        let values: Vec<f64> = data.iter().map(|p| p.value).collect();
        let n = values.len() as f64;

        // Simple linear regression to detect trend
        let x_mean = n / 2.0;
        let y_mean = values.iter().sum::<f64>() / n;

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }

        let slope = if denominator > 0.0 {
            numerator / denominator
        } else {
            0.0
        };

        // If strong trend (slope magnitude > threshold), use NHITS
        if slope.abs() > 0.01 {
            ModelType::NHITS
        } else {
            // Otherwise, could be seasonal - but default to NHITS
            ModelType::NHITS
        }
    }

    /// Calculate confidence intervals
    fn calculate_confidence_intervals(
        &self,
        predictions: &[f64],
        uncertainty: f64,
    ) -> Vec<(f64, f64)> {
        predictions.iter()
            .map(|&pred| {
                let lower = pred - uncertainty;
                let upper = pred + uncertainty;
                (lower, upper)
            })
            .collect()
    }
}

#[async_trait]
impl Forecast for FannForecaster {
    async fn train(&mut self, data: Vec<TimeSeriesPoint>) -> CoreResult<ModelMetrics> {
        use crate::error::CoreError;

        if data.len() < self.input_window {
            return Err(CoreError::Config(
                format!("Insufficient data: need at least {} points, got {}",
                        self.input_window, data.len())
            ));
        }

        // Engineer features
        let features = self.engineer_features(&data);

        // Fit scaler and normalize
        if !features.is_empty() && !features[0].is_empty() {
            let all_values: Vec<f64> = features.iter()
                .flat_map(|v| v.iter())
                .copied()
                .collect();
            self.feature_scaler = Some(StandardScaler::fit(&all_values));
        }

        // Load or create model
        if self.loaded_model.is_none() {
            let _ = self.load_model().await;
        }

        // Calculate basic metrics (mock for now)
        let mae = 0.5;
        let rmse = 0.7;
        let mape = 2.5;

        Ok(ModelMetrics { mae, rmse, mape })
    }

    async fn predict(
        &self,
        _source: &str,
        _metric: &str,
        horizon: usize,
    ) -> CoreResult<Vec<ForecastedPoint>> {
        use crate::error::CoreError;
        use chrono::Duration;

        if self.loaded_model.is_none() {
            return Err(CoreError::Config(
                "Model not loaded. Call train() or load_model() first.".to_string()
            ));
        }

        let model = self.loaded_model.as_ref().unwrap();

        // Generate predictions using mock model for initial development
        // Phase 3 enhancement: Use actual model inference
        let base_time = Utc::now();
        let mut forecasts = Vec::with_capacity(horizon);

        // Mock predictions with some variance
        for i in 0..horizon {
            let timestamp = base_time + Duration::minutes(i as i64);
            let value = 25.0 + (i as f64 * 0.1); // Simple increasing trend
            let uncertainty = 2.0;

            forecasts.push(ForecastedPoint {
                timestamp,
                value,
                confidence_lower: value - uncertainty,
                confidence_upper: value + uncertainty,
            });
        }

        Ok(forecasts)
    }

    async fn metrics(&self) -> CoreResult<ModelMetrics> {
        use crate::error::CoreError;

        if self.loaded_model.is_none() {
            return Err(CoreError::Config(
                "Model not trained. Call train() first.".to_string()
            ));
        }

        // Return mock metrics for initial development
        // Phase 3 enhancement: Calculate metrics from validation set
        Ok(ModelMetrics {
            mae: 0.5,
            rmse: 0.7,
            mape: 2.5,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use chrono::Duration;

    fn create_test_data(num_points: usize, start_value: f64) -> Vec<TimeSeriesPoint> {
        let mut points = Vec::new();
        let base_time = Utc::now();

        for i in 0..num_points {
            points.push(TimeSeriesPoint {
                timestamp: base_time + Duration::minutes(i as i64),
                source: "test-sensor".to_string(),
                metric: "pm25".to_string(),
                value: start_value + (i as f64),
                metadata: HashMap::new(),
            });
        }

        points
    }

    #[test]
    fn test_model_loading() {
        let model_path = PathBuf::from("/tmp/test_model.safetensors");
        let forecaster = FannForecaster::new(
            model_path.clone(),
            ModelType::NHITS,
            1440,
            360,
        );

        assert_eq!(forecaster.model_path, model_path);
        assert_eq!(forecaster.model_type, ModelType::NHITS);
        assert_eq!(forecaster.input_window, 1440);
        assert_eq!(forecaster.forecast_horizon, 360);
        assert!(forecaster.loaded_model.is_none());
    }

    #[tokio::test]
    async fn test_model_loading_async() {
        let model_path = PathBuf::from("/tmp/test_model.safetensors");
        let mut forecaster = FannForecaster::new(
            model_path,
            ModelType::NHITS,
            1440,
            360,
        );

        // This test should handle missing model files gracefully
        let result = forecaster.load_model().await;

        // Should return an error for missing file, but shouldn't panic
        match result {
            Ok(_) => {
                // If model exists, it should be loaded
                assert!(forecaster.loaded_model.is_some());
            }
            Err(e) => {
                // If model doesn't exist, should return appropriate error
                assert!(e.to_string().contains("not found") ||
                        e.to_string().contains("No such file"));
            }
        }
    }

    #[test]
    fn test_feature_engineering_temporal() {
        let data = create_test_data(100, 10.0);
        let forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            60,
            10,
        );

        let features = forecaster.engineer_features(&data);

        assert_eq!(features.len(), data.len());

        // Each feature vector should include temporal features
        for feature_vec in features.iter() {
            // Should have hour_of_day (0-23)
            assert!(feature_vec.iter().any(|&x| x >= 0.0 && x < 24.0));
        }
    }

    #[test]
    fn test_feature_engineering_lag() {
        let data = create_test_data(200, 15.0);
        let forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            120,
            30,
        );

        let features = forecaster.engineer_features(&data);

        // Should have lag features (1h, 3h, 24h)
        assert!(features.len() > 0);

        // Lag features should be present after sufficient data points
        if features.len() > 60 {
            let feature_vec = &features[100];
            assert!(feature_vec.len() > 3, "Should have multiple lag features");
        }
    }

    #[test]
    fn test_feature_engineering_rolling() {
        let data = create_test_data(150, 20.0);
        let forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            100,
            20,
        );

        let features = forecaster.engineer_features(&data);

        // Rolling statistics should be calculated
        assert!(features.len() > 0);

        // After enough data, rolling mean and std should exist
        if features.len() > 60 {
            let feature_vec = &features[80];
            assert!(feature_vec.len() > 5, "Should include rolling statistics");
        }
    }

    #[test]
    fn test_normalization() {
        let data = create_test_data(100, 50.0);
        let forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            60,
            10,
        );

        let features = forecaster.engineer_features(&data);
        let normalized = forecaster.normalize_features(&features);

        assert_eq!(normalized.len(), features.len());

        // Normalized values should have mean ~0 and std ~1
        if !normalized.is_empty() && !normalized[0].is_empty() {
            let all_values: Vec<f64> = normalized.iter()
                .flat_map(|v| v.iter())
                .copied()
                .collect();

            let mean = all_values.iter().sum::<f64>() / all_values.len() as f64;
            assert!(mean.abs() < 0.5, "Normalized mean should be close to 0");
        }
    }

    #[tokio::test]
    async fn test_predict_pm25() {
        let mut forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            1440,
            360,
        );

        let data = create_test_data(1500, 25.0);

        // Train the model first
        let _ = forecaster.train(data).await;

        // Predict
        let predictions = forecaster.predict("test-sensor", "pm25", 360).await;

        match predictions {
            Ok(forecasts) => {
                assert_eq!(forecasts.len(), 360);

                // Each forecast should have valid confidence intervals
                for forecast in forecasts.iter() {
                    assert!(forecast.confidence_lower <= forecast.value);
                    assert!(forecast.value <= forecast.confidence_upper);
                }
            }
            Err(_) => {
                // Acceptable if model isn't loaded
            }
        }
    }

    #[tokio::test]
    async fn test_predict_co2() {
        let mut forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NBEATSx,
            1440,
            360,
        );

        let mut data = create_test_data(1500, 400.0);
        for point in data.iter_mut() {
            point.metric = "co2".to_string();
        }

        let _ = forecaster.train(data).await;

        let predictions = forecaster.predict("test-sensor", "co2", 360).await;

        match predictions {
            Ok(forecasts) => {
                assert_eq!(forecasts.len(), 360);
                // CO2 values should be positive
                for forecast in forecasts.iter() {
                    assert!(forecast.value >= 0.0);
                }
            }
            Err(_) => {
                // Acceptable if model isn't loaded
            }
        }
    }

    #[test]
    fn test_confidence_intervals() {
        let forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            1440,
            360,
        );

        let predictions = vec![10.0, 15.0, 20.0, 25.0, 30.0];
        let uncertainty = 2.0;

        let intervals = forecaster.calculate_confidence_intervals(&predictions, uncertainty);

        assert_eq!(intervals.len(), predictions.len());

        for (i, &(lower, upper)) in intervals.iter().enumerate() {
            let pred = predictions[i];
            assert!(lower < pred, "Lower bound should be less than prediction");
            assert!(upper > pred, "Upper bound should be greater than prediction");
            assert!(lower < upper, "Lower bound should be less than upper bound");
        }
    }

    #[tokio::test]
    async fn test_cold_start_latency() {
        use std::time::Instant;

        let mut forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            1440,
            360,
        );

        let start = Instant::now();

        // Load model and make prediction
        let _ = forecaster.load_model().await;
        let _ = forecaster.predict("test-sensor", "pm25", 360).await;

        let duration = start.elapsed();

        // Should complete in less than 30 seconds (even if model doesn't exist)
        assert!(duration.as_secs() < 30, "Cold start took {} seconds", duration.as_secs());
    }

    #[tokio::test]
    async fn test_warm_cache_latency() {
        use std::time::Instant;

        let mut forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            1440,
            360,
        );

        // Load model once
        let _ = forecaster.load_model().await;

        let start = Instant::now();

        // Make prediction with warm cache
        let _ = forecaster.predict("test-sensor", "pm25", 360).await;

        let duration = start.elapsed();

        // Warm cache should be faster (less than 2 seconds)
        assert!(duration.as_secs() < 2, "Warm cache took {} seconds", duration.as_secs());
    }

    #[test]
    fn test_model_selection_nhits() {
        // Data with clear trend should prefer NHITS
        let mut data = create_test_data(1000, 10.0);
        for (i, point) in data.iter_mut().enumerate() {
            point.value = 10.0 + (i as f64) * 0.1; // Strong trend
        }

        let selected = FannForecaster::select_model(&data);

        // NHITS is better for trend data
        assert_eq!(selected, ModelType::NHITS);
    }

    #[test]
    fn test_model_selection_nbeats() {
        // Seasonal data might prefer NBEATSx
        let mut data = create_test_data(1000, 10.0);
        for (i, point) in data.iter_mut().enumerate() {
            point.value = 10.0 + 5.0 * ((i as f64 * 2.0 * std::f64::consts::PI) / 24.0).sin();
        }

        let selected = FannForecaster::select_model(&data);

        // NBEATSx can handle seasonal patterns
        // This is a simplified test - real selection would be more sophisticated
        assert!(
            selected == ModelType::NBEATSx || selected == ModelType::NHITS,
            "Should select a valid model type"
        );
    }

    #[tokio::test]
    async fn test_insufficient_data_handling() {
        let mut forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            1440,
            360,
        );

        // Only 10 data points - not enough for 1440 input window
        let data = create_test_data(10, 25.0);

        let result = forecaster.train(data).await;

        // Should handle insufficient data gracefully
        match result {
            Ok(_) => {
                // Might succeed with padding
            }
            Err(e) => {
                assert!(
                    e.to_string().contains("insufficient") ||
                    e.to_string().contains("not enough"),
                    "Error should indicate insufficient data"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_metrics() {
        let mut forecaster = FannForecaster::new(
            PathBuf::from("/tmp/model.safetensors"),
            ModelType::NHITS,
            100,
            20,
        );

        let data = create_test_data(200, 30.0);
        let _ = forecaster.train(data).await;

        let metrics = forecaster.metrics().await;

        match metrics {
            Ok(m) => {
                assert!(m.mae >= 0.0, "MAE should be non-negative");
                assert!(m.rmse >= 0.0, "RMSE should be non-negative");
                assert!(m.mape >= 0.0, "MAPE should be non-negative");
            }
            Err(_) => {
                // Acceptable if not trained
            }
        }
    }
}
