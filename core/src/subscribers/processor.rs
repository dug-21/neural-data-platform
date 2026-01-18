//! ProcessorSubscriber wraps a Processor to consume events from EventBus (DP-012 Phase 3)
//!
//! This subscriber takes RawDataPoint events from the EventBus, passes them through
//! a Processor implementation, and routes any generated outputs to an OutputSink.
//!
//! # Architecture
//!
//! ```text
//! EventBus (broadcast)
//!     |
//!     | RawDataPoint events
//!     v
//! ProcessorSubscriber
//!     |
//!     +-- processor.accepts_stream()?
//!     |       |
//!     |       `-- Skip if false
//!     |
//!     +-- processor.process(event)
//!     |       |
//!     |       `-- None: no output generated
//!     |       `-- Some(output): send to sink
//!     |
//!     `-- output_sink.send(output)
//! ```
//!
//! # London TDD Pattern
//!
//! - Processor trait is mocked for unit tests
//! - OutputSink trait is mocked for unit tests
//! - Configuration drives behavior

use crate::outputs::{OutputError, OutputSink};
use crate::processors::Processor;
use crate::traits::HealthStatus;
use crate::types::RawDataPoint;

use super::{Subscriber, SubscriberError};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Configuration for ProcessorSubscriber
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorSubscriberConfig {
    /// Unique identifier for this subscriber
    #[serde(default = "default_subscriber_id")]
    pub subscriber_id: String,

    /// Stream IDs to process (empty = all streams)
    #[serde(default)]
    pub stream_filter: Vec<String>,

    /// Whether to continue on output errors
    #[serde(default = "default_continue_on_error")]
    pub continue_on_output_error: bool,
}

fn default_subscriber_id() -> String {
    "processor-subscriber".to_string()
}

fn default_continue_on_error() -> bool {
    true
}

impl Default for ProcessorSubscriberConfig {
    fn default() -> Self {
        Self {
            subscriber_id: default_subscriber_id(),
            stream_filter: Vec::new(),
            continue_on_output_error: default_continue_on_error(),
        }
    }
}

/// Subscriber state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessorSubscriberState {
    Idle,
    Running,
    Stopped,
}

/// ProcessorSubscriber wraps a Processor and OutputSink
pub struct ProcessorSubscriber<P, O>
where
    P: Processor,
    O: OutputSink,
{
    config: ProcessorSubscriberConfig,
    processor: Arc<P>,
    output_sink: Arc<O>,
    state: ProcessorSubscriberState,
    events_processed: u64,
    outputs_sent: u64,
    output_errors: u64,
    last_error: Option<String>,
    shutdown_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

impl<P, O> ProcessorSubscriber<P, O>
where
    P: Processor + 'static,
    O: OutputSink + 'static,
{
    /// Create a new ProcessorSubscriber
    pub fn new(config: ProcessorSubscriberConfig, processor: Arc<P>, output_sink: Arc<O>) -> Self {
        Self {
            config,
            processor,
            output_sink,
            state: ProcessorSubscriberState::Idle,
            events_processed: 0,
            outputs_sent: 0,
            output_errors: 0,
            last_error: None,
            shutdown_signal: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> ProcessorSubscriberState {
        self.state
    }

    /// Get events processed count
    pub fn events_processed(&self) -> u64 {
        self.events_processed
    }

    /// Get outputs sent count
    pub fn outputs_sent(&self) -> u64 {
        self.outputs_sent
    }

    /// Get output errors count
    pub fn output_errors(&self) -> u64 {
        self.output_errors
    }

    /// Process a single event
    async fn process_event(&mut self, raw: Arc<RawDataPoint>) -> Result<(), SubscriberError> {
        // Check stream filter (subscriber level)
        if !self.accepts_stream(&raw.source_id) {
            return Ok(());
        }

        // Check processor accepts this stream
        if !self.processor.accepts_stream(&raw.source_id) {
            return Ok(());
        }

        self.events_processed += 1;

        // Process the event
        if let Some(output) = self.processor.process(&raw) {
            debug!(
                processor = %self.processor.name(),
                source = %raw.source_id,
                "Processor generated output"
            );

            // Send to output sink
            match self.output_sink.send(&output).await {
                Ok(()) => {
                    self.outputs_sent += 1;
                }
                Err(e) => {
                    self.output_errors += 1;
                    self.last_error = Some(e.to_string());

                    if self.config.continue_on_output_error {
                        warn!(
                            error = %e,
                            sink = %self.output_sink.name(),
                            "Output error (continuing)"
                        );
                    } else {
                        error!(
                            error = %e,
                            sink = %self.output_sink.name(),
                            "Output error (stopping)"
                        );
                        return Err(SubscriberError::ProcessingError(e.to_string()));
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<P, O> Subscriber for ProcessorSubscriber<P, O>
where
    P: Processor + 'static,
    O: OutputSink + 'static,
{
    fn id(&self) -> &str {
        &self.config.subscriber_id
    }

    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>,
    ) -> Result<(), SubscriberError> {
        info!(
            id = %self.id(),
            processor = %self.processor.name(),
            "Starting ProcessorSubscriber"
        );

        self.state = ProcessorSubscriberState::Running;

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_signal = Some(shutdown_tx);

        // Event processing loop
        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = &mut shutdown_rx => {
                    info!(id = %self.id(), "Shutdown signal received");
                    break;
                }

                // Process events
                result = receiver.recv() => {
                    match result {
                        Ok(raw_point) => {
                            if let Err(e) = self.process_event(raw_point).await {
                                error!(error = %e, "Error processing event");
                                // Non-fatal errors already logged in process_event
                                // Fatal errors would have been returned there
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(lagged = n, "Receiver lagged, missed events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Event bus closed");
                            break;
                        }
                    }
                }
            }
        }

        self.state = ProcessorSubscriberState::Stopped;

        info!(
            id = %self.id(),
            events_processed = self.events_processed,
            outputs_sent = self.outputs_sent,
            output_errors = self.output_errors,
            "ProcessorSubscriber stopped"
        );

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        info!(id = %self.id(), "Stopping ProcessorSubscriber");

        // Signal shutdown
        if let Some(tx) = self.shutdown_signal.take() {
            let _ = tx.send(());
        }

        // Reset processor state if needed
        self.processor.reset();

        self.state = ProcessorSubscriberState::Stopped;
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        if self.config.stream_filter.is_empty() {
            true
        } else {
            self.config.stream_filter.iter().any(|s| s == stream_id)
        }
    }

    async fn health_check(&self) -> HealthStatus {
        let mut details = HashMap::new();

        // Check output sink health
        let sink_healthy = self.output_sink.health_check().await.is_ok();

        let healthy = sink_healthy && self.state == ProcessorSubscriberState::Running;

        let message = if healthy {
            "Healthy".to_string()
        } else if !sink_healthy {
            "Output sink unhealthy".to_string()
        } else {
            "Not running".to_string()
        };

        details.insert("state".to_string(), format!("{:?}", self.state));
        details.insert("processor".to_string(), self.processor.name());
        details.insert("output_sink".to_string(), self.output_sink.name());
        details.insert(
            "events_processed".to_string(),
            self.events_processed.to_string(),
        );
        details.insert("outputs_sent".to_string(), self.outputs_sent.to_string());
        details.insert("output_errors".to_string(), self.output_errors.to_string());

        if let Some(ref err) = self.last_error {
            details.insert("last_error".to_string(), err.clone());
        }

        HealthStatus {
            healthy,
            message,
            details,
        }
    }
}

impl From<OutputError> for SubscriberError {
    fn from(err: OutputError) -> Self {
        SubscriberError::ProcessingError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outputs::MockOutputSink;
    use crate::processors::{Alert, MockProcessor, ProcessorOutput, Severity};
    use chrono::Utc;

    fn create_test_config() -> ProcessorSubscriberConfig {
        ProcessorSubscriberConfig {
            subscriber_id: "test-processor-sub".to_string(),
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    fn create_test_alert() -> ProcessorOutput {
        ProcessorOutput::Alert(Alert {
            id: "test-alert".to_string(),
            timestamp: Utc::now(),
            source: "test".to_string(),
            severity: Severity::Warning,
            alert_type: "threshold".to_string(),
            message: "Test alert".to_string(),
            field: None,
            value: None,
            threshold: None,
            context: None,
        })
    }

    #[test]
    fn test_config_default() {
        let config = ProcessorSubscriberConfig::default();
        assert_eq!(config.subscriber_id, "processor-subscriber");
        assert!(config.stream_filter.is_empty());
        assert!(config.continue_on_output_error);
    }

    #[tokio::test]
    async fn test_processor_subscriber_new() {
        let config = create_test_config();

        let mut mock_processor = MockProcessor::new();
        mock_processor
            .expect_name()
            .returning(|| "mock-processor".to_string());

        let mut mock_output = MockOutputSink::new();
        mock_output
            .expect_name()
            .returning(|| "mock-output".to_string());

        let subscriber =
            ProcessorSubscriber::new(config, Arc::new(mock_processor), Arc::new(mock_output));

        assert_eq!(subscriber.id(), "test-processor-sub");
        assert_eq!(subscriber.state(), ProcessorSubscriberState::Idle);
        assert_eq!(subscriber.events_processed(), 0);
    }

    #[test]
    fn test_accepts_stream_no_filter() {
        let config = ProcessorSubscriberConfig::default();

        let mut mock_processor = MockProcessor::new();
        mock_processor
            .expect_name()
            .returning(|| "mock".to_string());

        let mut mock_output = MockOutputSink::new();
        mock_output.expect_name().returning(|| "mock".to_string());

        let subscriber =
            ProcessorSubscriber::new(config, Arc::new(mock_processor), Arc::new(mock_output));

        assert!(subscriber.accepts_stream("any-stream"));
        assert!(subscriber.accepts_stream("another-stream"));
    }

    #[test]
    fn test_accepts_stream_with_filter() {
        let config = ProcessorSubscriberConfig {
            stream_filter: vec!["air-quality".to_string()],
            ..Default::default()
        };

        let mut mock_processor = MockProcessor::new();
        mock_processor
            .expect_name()
            .returning(|| "mock".to_string());

        let mut mock_output = MockOutputSink::new();
        mock_output.expect_name().returning(|| "mock".to_string());

        let subscriber =
            ProcessorSubscriber::new(config, Arc::new(mock_processor), Arc::new(mock_output));

        assert!(subscriber.accepts_stream("air-quality"));
        assert!(!subscriber.accepts_stream("outdoor-weather"));
    }

    #[tokio::test]
    async fn test_health_check_idle() {
        let config = create_test_config();

        let mut mock_processor = MockProcessor::new();
        mock_processor
            .expect_name()
            .returning(|| "mock-processor".to_string());

        let mut mock_output = MockOutputSink::new();
        mock_output
            .expect_name()
            .returning(|| "mock-output".to_string());
        mock_output
            .expect_health_check()
            .returning(|| Box::pin(async { Ok(()) }));

        let subscriber =
            ProcessorSubscriber::new(config, Arc::new(mock_processor), Arc::new(mock_output));

        let status = subscriber.health_check().await;
        // Not running yet, so not healthy
        assert!(!status.healthy);
        assert!(status.details.contains_key("processor"));
    }
}
