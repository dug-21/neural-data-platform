//! Output Sinks for Processor Outputs (DP-012 Phase 3)

mod mqtt;

pub use mqtt::{MqttOutput, MqttOutputConfig};

use async_trait::async_trait;
use thiserror::Error;

use crate::processors::ProcessorOutput;

/// Errors from output operations
#[derive(Debug, Error)]
pub enum OutputError {
    #[error("Failed to send output: {0}")]
    SendFailed(String),

    #[error("Not connected to output destination")]
    NotConnected,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Serialization failed: {0}")]
    SerializationFailed(String),
}

/// Trait for output destinations
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait OutputSink: Send + Sync {
    /// Get the name of this output sink
    fn name(&self) -> String;
    /// Send output to the destination
    async fn send(&self, output: &ProcessorOutput) -> Result<(), OutputError>;
    /// Check health of the output destination
    async fn health_check(&self) -> Result<(), OutputError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processors::{Alert, Severity};
    use chrono::Utc;

    #[tokio::test]
    async fn test_mock_output_sink() {
        let mut mock = MockOutputSink::new();
        mock.expect_name().returning(|| "mock".to_string());
        mock.expect_send().returning(|_| Box::pin(async { Ok(()) }));

        let alert = Alert {
            id: "test".to_string(),
            timestamp: Utc::now(),
            source: "test".to_string(),
            severity: Severity::Warning,
            alert_type: "threshold".to_string(),
            message: "Test".to_string(),
            field: None,
            value: None,
            threshold: None,
            context: None,
        };

        assert!(mock.send(&ProcessorOutput::Alert(alert)).await.is_ok());
    }
}
