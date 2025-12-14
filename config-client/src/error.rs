use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration not found: {0}")]
    NotFound(String),

    #[error("etcd connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Watch error: {0}")]
    WatchError(String),

    #[error("Environment variable error: {0}")]
    EnvError(String),
}

impl From<etcd_client::Error> for ConfigError {
    fn from(e: etcd_client::Error) -> Self {
        ConfigError::ConnectionFailed(e.to_string())
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::SerializationError(e.to_string())
    }
}
