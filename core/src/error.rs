use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Source error: {0}")]
    Source(String),

    #[error("Forecast error: {0}")]
    Forecast(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Polars error: {0}")]
    Polars(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Prediction error: {0}")]
    PredictionError(String),

    #[error("Parser error: {0}")]
    Parser(String),
}

impl From<polars::error::PolarsError> for CoreError {
    fn from(err: polars::error::PolarsError) -> Self {
        CoreError::Polars(err.to_string())
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
