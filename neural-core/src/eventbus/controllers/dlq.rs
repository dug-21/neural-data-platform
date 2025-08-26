use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{warn, error};

use crate::eventbus::{
    types::{Event, EventId},
    error::EventBusError,
    traits::EventBus,
};

/// Dead Letter Queue for handling failed messages
pub struct DeadLetterQueue {
    config: DLQConfig,
    retry_policies: Arc<RwLock<HashMap<String, RetryPolicy>>>,
    retry_tracker: Arc<RwLock<HashMap<EventId, RetryInfo>>>,
    event_bus: Option<Arc<dyn EventBus>>,
}

#[derive(Debug, Clone)]
pub struct DLQConfig {
    pub max_retries: usize,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
    pub dlq_retention_hours: u64,
    pub enable_poison_detection: bool,
}

impl Default for DLQConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            multiplier: 2.0,
            dlq_retention_hours: 168,  // 7 days
            enable_poison_detection: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub backoff_strategy: BackoffStrategy,
    pub retry_conditions: Vec<RetryCondition>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_strategy: BackoffStrategy::Exponential(2.0),
            retry_conditions: vec![
                RetryCondition::Temporary,
                RetryCondition::Timeout,
                RetryCondition::RateLimit,
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Fixed(u64),
    Linear(u64),
    Exponential(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetryCondition {
    Temporary,
    Timeout,
    RateLimit,
    ConnectionError,
    All,
}

#[derive(Debug, Clone)]
struct RetryInfo {
    event_id: EventId,
    attempt: usize,
    last_error: String,
    first_failure: i64,
    last_retry: i64,
}

#[derive(Debug, Clone)]
pub enum MessageDisposition {
    Retry { attempt: usize, delay_ms: u64 },
    DeadLetter { reason: String, final_error: String },
    Dropped { reason: String },
}

impl DeadLetterQueue {
    pub fn new(config: DLQConfig) -> Self {
        Self {
            config,
            retry_policies: Arc::new(RwLock::new(HashMap::new())),
            retry_tracker: Arc::new(RwLock::new(HashMap::new())),
            event_bus: None,
        }
    }

    pub fn with_event_bus(mut self, event_bus: Arc<dyn EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    pub async fn set_retry_policy(&self, channel: &str, policy: RetryPolicy) {
        let mut policies = self.retry_policies.write().await;
        policies.insert(channel.to_string(), policy);
    }

    pub async fn handle_failed_message(
        &self,
        channel: &str,
        event_id: &EventId,
        event: &Event,
        error: &EventBusError,
    ) -> Result<MessageDisposition, EventBusError> {
        let policy = self.get_retry_policy(channel).await;
        
        let mut retry_info = self.get_or_create_retry_info(event_id, error).await;
        
        if self.should_retry(&retry_info, &policy, error) {
            retry_info.attempt += 1;
            retry_info.last_error = error.to_string();
            retry_info.last_retry = chrono::Utc::now().timestamp();
            
            let delay_ms = self.calculate_backoff(&policy.backoff_strategy, retry_info.attempt);
            
            self.update_retry_info(event_id, retry_info.clone()).await;
            
            // Schedule retry if event bus is available
            if self.event_bus.is_some() {
                self.schedule_retry(channel, event_id, event, delay_ms).await?;
            }
            
            Ok(MessageDisposition::Retry {
                attempt: retry_info.attempt,
                delay_ms,
            })
        } else if retry_info.attempt >= policy.max_attempts {
            // Move to DLQ
            self.send_to_dlq(channel, event_id, event, &retry_info).await?;
            self.remove_retry_info(event_id).await;
            
            Ok(MessageDisposition::DeadLetter {
                reason: format!("Max retries ({}) exceeded", policy.max_attempts),
                final_error: retry_info.last_error,
            })
        } else {
            // Drop message (non-retryable error)
            self.remove_retry_info(event_id).await;
            
            Ok(MessageDisposition::Dropped {
                reason: format!("Non-retryable error: {}", error),
            })
        }
    }

    async fn get_retry_policy(&self, channel: &str) -> RetryPolicy {
        let policies = self.retry_policies.read().await;
        policies.get(channel).cloned().unwrap_or_default()
    }

    async fn get_or_create_retry_info(&self, event_id: &EventId, error: &EventBusError) -> RetryInfo {
        let mut tracker = self.retry_tracker.write().await;
        tracker.entry(event_id.clone())
            .or_insert_with(|| RetryInfo {
                event_id: event_id.clone(),
                attempt: 0,
                last_error: error.to_string(),
                first_failure: chrono::Utc::now().timestamp(),
                last_retry: chrono::Utc::now().timestamp(),
            })
            .clone()
    }

    async fn update_retry_info(&self, event_id: &EventId, info: RetryInfo) {
        let mut tracker = self.retry_tracker.write().await;
        tracker.insert(event_id.clone(), info);
    }

    async fn remove_retry_info(&self, event_id: &EventId) {
        let mut tracker = self.retry_tracker.write().await;
        tracker.remove(event_id);
    }

    fn should_retry(&self, retry_info: &RetryInfo, policy: &RetryPolicy, error: &EventBusError) -> bool {
        if retry_info.attempt >= policy.max_attempts {
            return false;
        }

        // Check if poison message (too many failures in short time)
        if self.config.enable_poison_detection {
            let elapsed = chrono::Utc::now().timestamp() - retry_info.first_failure;
            if retry_info.attempt > 5 && elapsed < 60 {
                warn!("Poison message detected: {}", retry_info.event_id);
                return false;
            }
        }

        // Check retry conditions
        let is_retryable = match error {
            EventBusError::Throttled => policy.retry_conditions.contains(&RetryCondition::RateLimit),
            EventBusError::Timeout(_) => policy.retry_conditions.contains(&RetryCondition::Timeout),
            EventBusError::Backend(_) => policy.retry_conditions.contains(&RetryCondition::ConnectionError),
            _ => policy.retry_conditions.contains(&RetryCondition::All),
        };

        is_retryable
    }

    fn calculate_backoff(&self, strategy: &BackoffStrategy, attempt: usize) -> u64 {
        let delay = match strategy {
            BackoffStrategy::Fixed(ms) => *ms,
            BackoffStrategy::Linear(increment) => increment * attempt as u64,
            BackoffStrategy::Exponential(multiplier) => {
                let delay = self.config.base_delay_ms as f64 * multiplier.powi(attempt as i32 - 1);
                delay as u64
            }
        };

        delay.min(self.config.max_delay_ms)
    }

    async fn schedule_retry(
        &self,
        channel: &str,
        _event_id: &EventId,
        event: &Event,
        delay_ms: u64,
    ) -> Result<(), EventBusError> {
        let event_bus = self.event_bus.as_ref()
            .ok_or_else(|| EventBusError::Internal("No EventBus configured for DLQ".to_string()))?;
        
        let channel = channel.to_string();
        let event = event.clone();
        let event_bus = event_bus.clone();
        
        // Spawn async task to retry after delay
        tokio::spawn(async move {
            sleep(Duration::from_millis(delay_ms)).await;
            
            if let Err(e) = event_bus.publish(&channel, event).await {
                error!("Failed to retry message: {}", e);
            }
        });
        
        Ok(())
    }

    async fn send_to_dlq(
        &self,
        channel: &str,
        event_id: &EventId,
        event: &Event,
        retry_info: &RetryInfo,
    ) -> Result<(), EventBusError> {
        let dlq_channel = format!("stream:dlq:{}", channel.replace("stream:", ""));
        
        warn!(
            "Moving message {} to DLQ after {} retries: {}",
            event_id, retry_info.attempt, retry_info.last_error
        );
        
        if let Some(event_bus) = &self.event_bus {
            let mut metadata = event.metadata.clone();
            metadata.insert("original_channel".to_string(), channel.to_string());
            metadata.insert("retry_count".to_string(), retry_info.attempt.to_string());
            metadata.insert("final_error".to_string(), retry_info.last_error.clone());
            metadata.insert("first_failure".to_string(), retry_info.first_failure.to_string());
            
            let dlq_event = Event {
                event_type: "DeadLetter".to_string(),
                payload: event.payload.clone(),
                metadata,
                timestamp: chrono::Utc::now().timestamp(),
            };
            
            event_bus.publish(&dlq_channel, dlq_event).await?;
        }
        
        Ok(())
    }

    pub async fn get_retry_count(&self, event_id: &EventId) -> usize {
        let tracker = self.retry_tracker.read().await;
        tracker.get(event_id).map(|info| info.attempt).unwrap_or(0)
    }

    pub async fn clear_retry_history(&self) {
        let mut tracker = self.retry_tracker.write().await;
        tracker.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event() -> Event {
        Event {
            event_type: "TestEvent".to_string(),
            payload: vec![1, 2, 3],
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_retry_with_exponential_backoff() {
        let dlq = DeadLetterQueue::new(DLQConfig::default());
        
        let event_id = EventId::new();
        let event = create_test_event();
        let error = EventBusError::Timeout("Connection timeout".to_string());
        
        // First failure - should retry
        let disposition = dlq.handle_failed_message(
            "stream:symbol:AAPL",
            &event_id,
            &event,
            &error,
        ).await.unwrap();
        
        if let MessageDisposition::Retry { attempt, delay_ms } = disposition {
            assert_eq!(attempt, 1);
            assert_eq!(delay_ms, 1000);  // Base delay
        } else {
            panic!("Expected Retry disposition");
        }
        
        // Second failure - should retry with backoff
        let disposition = dlq.handle_failed_message(
            "stream:symbol:AAPL",
            &event_id,
            &event,
            &error,
        ).await.unwrap();
        
        if let MessageDisposition::Retry { attempt, delay_ms } = disposition {
            assert_eq!(attempt, 2);
            assert_eq!(delay_ms, 2000);  // Exponential backoff
        } else {
            panic!("Expected Retry disposition");
        }
    }

    #[tokio::test]
    async fn test_max_retries_reached() {
        let mut config = DLQConfig::default();
        config.max_retries = 2;
        let dlq = DeadLetterQueue::new(config);
        
        let event_id = EventId::new();
        let event = create_test_event();
        let error = EventBusError::Timeout("Connection timeout".to_string());
        
        // Simulate max retries
        for _ in 0..2 {
            let _ = dlq.handle_failed_message(
                "stream:symbol:AAPL",
                &event_id,
                &event,
                &error,
            ).await;
        }
        
        // Next failure should go to DLQ
        let disposition = dlq.handle_failed_message(
            "stream:symbol:AAPL",
            &event_id,
            &event,
            &error,
        ).await.unwrap();
        
        if let MessageDisposition::DeadLetter { reason, .. } = disposition {
            assert!(reason.contains("Max retries"));
        } else {
            panic!("Expected DeadLetter disposition");
        }
    }

    #[tokio::test]
    async fn test_non_retryable_error() {
        let dlq = DeadLetterQueue::new(DLQConfig::default());
        
        dlq.set_retry_policy(
            "stream:symbol:AAPL",
            RetryPolicy {
                max_attempts: 3,
                backoff_strategy: BackoffStrategy::Exponential(2.0),
                retry_conditions: vec![RetryCondition::Timeout],  // Only retry timeouts
            },
        ).await;
        
        let event_id = EventId::new();
        let event = create_test_event();
        let error = EventBusError::InvalidChannel("Bad channel".to_string());
        
        // Non-retryable error should be dropped
        let disposition = dlq.handle_failed_message(
            "stream:symbol:AAPL",
            &event_id,
            &event,
            &error,
        ).await.unwrap();
        
        if let MessageDisposition::Dropped { reason } = disposition {
            assert!(reason.contains("Non-retryable"));
        } else {
            panic!("Expected Dropped disposition");
        }
    }
}