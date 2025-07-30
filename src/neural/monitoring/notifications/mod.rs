//! Notification System Module
//!
//! Provides intelligent notification systems for training triggers, alerts, and system events.

pub mod training;

pub use training::{
    TrainingNotifier, TrainingNotification, TrainingTriggerReason, TrainingPriority,
    TrainingAction, TrainingThresholds, PerformanceContext, NotifierConfig,
    NotificationStats, ResourceRequirements,
};

use anyhow::Result;
use tokio::sync::{broadcast, mpsc};
use tracing::info;

/// Complete notification system configuration
#[derive(Debug, Clone)]
pub struct NotificationSystemConfig {
    pub training: NotifierConfig,
    pub enable_training_notifications: bool,
}

impl Default for NotificationSystemConfig {
    fn default() -> Self {
        Self {
            training: NotifierConfig::default(),
            enable_training_notifications: true,
        }
    }
}

/// Complete notification system
pub struct NotificationSystem {
    training_notifier: Option<TrainingNotifier>,
    config: NotificationSystemConfig,
}

impl NotificationSystem {
    /// Create a new notification system
    pub fn new(
        config: NotificationSystemConfig,
        performance_rx: broadcast::Receiver<super::performance_channel::PerformanceEvent>,
        training_tx: mpsc::Sender<TrainingNotification>,
        thresholds: TrainingThresholds,
    ) -> Self {
        let training_notifier = if config.enable_training_notifications {
            Some(TrainingNotifier::new(
                performance_rx,
                training_tx,
                thresholds,
                config.training.clone(),
            ))
        } else {
            None
        };

        Self {
            training_notifier,
            config,
        }
    }

    /// Start the notification system
    pub async fn start(mut self) -> Result<()> {
        info!("Starting notification system with training notifications: {}", 
               self.config.enable_training_notifications);

        if let Some(mut training_notifier) = self.training_notifier.take() {
            training_notifier.start_monitoring().await?;
        }

        Ok(())
    }

    /// Get training notification statistics
    pub async fn get_training_stats(&self) -> Option<NotificationStats> {
        if let Some(notifier) = &self.training_notifier {
            notifier.get_notification_stats().await.ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_notification_system_creation() {
        let (_perf_tx, perf_rx) = broadcast::channel(100);
        let (training_tx, _training_rx) = mpsc::channel(100);
        let config = NotificationSystemConfig::default();
        let thresholds = TrainingThresholds::default();
        
        let system = NotificationSystem::new(config, perf_rx, training_tx, thresholds);
        assert!(system.training_notifier.is_some());
    }
}