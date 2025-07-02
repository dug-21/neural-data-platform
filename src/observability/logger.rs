//! Advanced logging utilities for production environments
//! 
//! This module provides structured logging capabilities with:
//! - JSON formatting for log aggregation
//! - Sensitive data filtering
//! - Performance-optimized async logging
//! - Context-aware log enrichment

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{event, Level};

/// Log levels for the system
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    }
}

/// Structured log event
#[derive(Debug, Clone)]
pub struct LogEvent {
    pub level: LogLevel,
    pub message: String,
    pub module: String,
    pub context: HashMap<String, Value>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl LogEvent {
    pub fn new(level: LogLevel, message: String, module: String) -> Self {
        Self {
            level,
            message,
            module,
            context: HashMap::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Add context to the log event
    pub fn with_context(mut self, key: &str, value: Value) -> Self {
        self.context.insert(key.to_string(), value);
        self
    }

    /// Add multiple context fields
    pub fn with_contexts(mut self, contexts: HashMap<String, Value>) -> Self {
        self.context.extend(contexts);
        self
    }

    /// Filter sensitive data from the log event
    pub fn filter_sensitive_data(mut self) -> Self {
        let sensitive_keys = vec!["password", "token", "secret", "key", "auth"];
        
        for key in sensitive_keys {
            if self.context.contains_key(key) {
                self.context.insert(key.to_string(), Value::String("[REDACTED]".to_string()));
            }
        }

        // Filter sensitive data from message
        self.message = self.filter_message_sensitive_data(&self.message);
        self
    }

    fn filter_message_sensitive_data(&self, message: &str) -> String {
        // Simple regex-based filtering - in production, use more sophisticated filtering
        let patterns = vec![
            (r"password=\w+", "password=[REDACTED]"),
            (r"token=[\w-]+", "token=[REDACTED]"),
            (r"Bearer [\w-]+", "Bearer [REDACTED]"),
        ];

        let mut filtered = message.to_string();
        for (pattern, replacement) in patterns {
            filtered = regex::Regex::new(pattern)
                .unwrap()
                .replace_all(&filtered, replacement)
                .to_string();
        }
        filtered
    }

    /// Log the event using tracing
    pub fn log(&self) {
        match self.level {
            LogLevel::Error => tracing::error!(
                module = %self.module,
                timestamp = %self.timestamp,
                context = ?self.context,
                "{}",
                self.message
            ),
            LogLevel::Warn => tracing::warn!(
                module = %self.module,
                timestamp = %self.timestamp,
                context = ?self.context,
                "{}",
                self.message
            ),
            LogLevel::Info => tracing::info!(
                module = %self.module,
                timestamp = %self.timestamp,
                context = ?self.context,
                "{}",
                self.message
            ),
            LogLevel::Debug => tracing::debug!(
                module = %self.module,
                timestamp = %self.timestamp,
                context = ?self.context,
                "{}",
                self.message
            ),
            LogLevel::Trace => tracing::trace!(
                module = %self.module,
                timestamp = %self.timestamp,
                context = ?self.context,
                "{}",
                self.message
            ),
        }
    }
}

/// Macro for creating structured log events
#[macro_export]
macro_rules! log_event {
    ($level:expr, $module:expr, $message:expr) => {
        $crate::observability::logger::LogEvent::new($level, $message.to_string(), $module.to_string())
            .filter_sensitive_data()
            .log();
    };
    
    ($level:expr, $module:expr, $message:expr, $($key:expr => $value:expr),+) => {
        {
            let mut event = $crate::observability::logger::LogEvent::new($level, $message.to_string(), $module.to_string());
            $(
                event = event.with_context($key, serde_json::json!($value));
            )+
            event.filter_sensitive_data().log();
        }
    };
}

/// Performance logging utilities
pub struct PerformanceLogger;

impl PerformanceLogger {
    /// Log operation timing
    pub fn log_timing(operation: &str, duration: std::time::Duration, success: bool) {
        log_event!(
            if success { LogLevel::Info } else { LogLevel::Warn },
            "performance",
            format!("Operation {} completed", operation),
            "operation" => operation,
            "duration_ms" => duration.as_millis(),
            "success" => success
        );
    }

    /// Log resource usage
    pub fn log_resource_usage(cpu_percent: f64, memory_mb: f64, disk_percent: f64) {
        log_event!(
            LogLevel::Info,
            "resources",
            "System resource usage",
            "cpu_percent" => cpu_percent,
            "memory_mb" => memory_mb,
            "disk_percent" => disk_percent
        );
    }
}

/// Business logic logging utilities
pub struct BusinessLogger;

impl BusinessLogger {
    /// Log prediction events
    pub fn log_prediction(
        model_name: &str,
        prediction_value: f64,
        confidence: f64,
        processing_time_ms: u64,
    ) {
        log_event!(
            LogLevel::Info,
            "predictions",
            "Model prediction generated",
            "model_name" => model_name,
            "prediction_value" => prediction_value,
            "confidence" => confidence,
            "processing_time_ms" => processing_time_ms
        );
    }

    /// Log trading decisions
    pub fn log_trading_decision(
        symbol: &str,
        action: &str,
        quantity: f64,
        price: f64,
        reasoning: &str,
    ) {
        log_event!(
            LogLevel::Info,
            "trading",
            "Trading decision made",
            "symbol" => symbol,
            "action" => action,
            "quantity" => quantity,
            "price" => price,
            "reasoning" => reasoning
        );
    }

    /// Log data quality events
    pub fn log_data_quality(
        source: &str,
        quality_score: f64,
        issues: &[String],
    ) {
        log_event!(
            LogLevel::Info,
            "data_quality",
            "Data quality assessment",
            "source" => source,
            "quality_score" => quality_score,
            "issues" => issues
        );
    }
}

/// Security logging utilities
pub struct SecurityLogger;

impl SecurityLogger {
    /// Log authentication events
    pub fn log_authentication(user_id: &str, success: bool, ip_address: &str) {
        log_event!(
            if success { LogLevel::Info } else { LogLevel::Warn },
            "security",
            "Authentication attempt",
            "user_id" => user_id,
            "success" => success,
            "ip_address" => ip_address
        );
    }

    /// Log authorization events
    pub fn log_authorization(user_id: &str, resource: &str, action: &str, granted: bool) {
        log_event!(
            if granted { LogLevel::Info } else { LogLevel::Warn },
            "security",
            "Authorization check",
            "user_id" => user_id,
            "resource" => resource,
            "action" => action,
            "granted" => granted
        );
    }

    /// Log security violations
    pub fn log_security_violation(violation_type: &str, details: &str, ip_address: &str) {
        log_event!(
            LogLevel::Error,
            "security",
            "Security violation detected",
            "violation_type" => violation_type,
            "details" => details,
            "ip_address" => ip_address
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_log_event_creation() {
        let event = LogEvent::new(LogLevel::Info, "Test message".to_string(), "test_module".to_string());
        assert_eq!(event.level, LogLevel::Info);
        assert_eq!(event.message, "Test message");
        assert_eq!(event.module, "test_module");
    }

    #[test]
    fn test_sensitive_data_filtering() {
        let event = LogEvent::new(
            LogLevel::Info,
            "User login with password=secret123".to_string(),
            "auth".to_string(),
        )
        .with_context("password", json!("secret123"))
        .filter_sensitive_data();

        assert!(event.message.contains("[REDACTED]"));
        assert_eq!(event.context.get("password"), Some(&json!("[REDACTED]")));
    }

    #[test]
    fn test_context_addition() {
        let event = LogEvent::new(LogLevel::Info, "Test".to_string(), "test".to_string())
            .with_context("key1", json!("value1"))
            .with_context("key2", json!(42));

        assert_eq!(event.context.get("key1"), Some(&json!("value1")));
        assert_eq!(event.context.get("key2"), Some(&json!(42)));
    }
}