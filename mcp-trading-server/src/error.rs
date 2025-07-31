use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] tokio_postgres::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("MCP error: {0}")]
    MCP(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

impl From<std::env::VarError> for Error {
    fn from(err: std::env::VarError) -> Self {
        Error::Config(format!("Environment variable error: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
