/// Forecasting module using ruv-FANN models
pub mod fann_adapter;
pub mod features;
pub mod scaler;

pub use fann_adapter::{FannForecaster, ModelType};
pub use features::{FeatureVector, hour_of_day, day_of_week, is_weekend, lag_feature, rolling_mean, rolling_std};
pub use scaler::StandardScaler;
