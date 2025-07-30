//! Notification system for model training events
//! 
//! Provides real-time notifications about training lifecycle events
//! through a broadcast channel mechanism.

mod notification_channel;
mod notification_types;

pub use notification_channel::{NotificationChannel, NotificationReceiver};
pub use notification_types::{TrainingNotification, Priority, TrainingMetrics};

#[cfg(test)]
mod tests;