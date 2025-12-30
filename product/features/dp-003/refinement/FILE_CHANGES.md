# DP-003: File Changes - MQTT Multi-Subscription Support

## Overview

This document details all files to create, modify, and test for the multi-subscription feature.

---

## New Files to Create

### 1. `core/src/sources/mqtt/mod.rs`

**Purpose**: Module organization for MQTT source components.

```rust
//! MQTT source implementation with multi-subscription support.
//!
//! This module provides:
//! - `SubscriptionConfig`: Configuration for individual subscriptions
//! - `TopicRouter`: MQTT topic pattern matching and routing
//! - `MqttSource`: Main MQTT data source implementation

mod subscription;
mod router;

pub use subscription::SubscriptionConfig;
pub use router::{TopicRouter, RouteEntry};

// Re-export existing types
pub use crate::sources::mqtt_source::{MqttConfig, MqttSource};
```

---

### 2. `core/src/sources/mqtt/subscription.rs`

**Purpose**: SubscriptionConfig struct definition.

```rust
//! MQTT subscription configuration.

use serde::{Deserialize, Serialize};
use crate::parsers::ParserConfig;

/// Configuration for a single MQTT subscription.
///
/// Each subscription maps a topic pattern to a stream and optional parser.
///
/// # Example
///
/// ```yaml
/// subscriptions:
///   - stream_id: air-quality
///     topic_pattern: "airgradient/readings/+"
///     enabled: true
///     parser:
///       parser_type: flat_json
///       location_id_field: serialno
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionConfig {
    /// Stream ID for routing (e.g., "air-quality", "homeassistant").
    /// Must be unique within the MQTT source configuration.
    pub stream_id: String,

    /// MQTT topic pattern with wildcards.
    /// - `+` matches a single topic level (e.g., "sensors/+/temp")
    /// - `#` matches multiple levels at the end (e.g., "sensors/#")
    pub topic_pattern: String,

    /// Parser configuration for this subscription.
    /// If not specified, the source-level parser is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<ParserConfig>,

    /// Whether this subscription is enabled.
    /// Disabled subscriptions are skipped during routing.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self {
            stream_id: String::new(),
            topic_pattern: String::new(),
            parser: None,
            enabled: true,
        }
    }
}

impl SubscriptionConfig {
    /// Create a new subscription configuration.
    pub fn new(stream_id: impl Into<String>, topic_pattern: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            topic_pattern: topic_pattern.into(),
            parser: None,
            enabled: true,
        }
    }

    /// Set the parser configuration.
    pub fn with_parser(mut self, parser: ParserConfig) -> Self {
        self.parser = Some(parser);
        self
    }

    /// Set the enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Validate the subscription configuration.
    pub fn validate(&self) -> Result<(), SubscriptionError> {
        if self.stream_id.is_empty() {
            return Err(SubscriptionError::EmptyStreamId);
        }
        if self.topic_pattern.is_empty() {
            return Err(SubscriptionError::EmptyTopicPattern);
        }
        validate_topic_pattern(&self.topic_pattern)?;
        Ok(())
    }
}

/// Errors for subscription configuration.
#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionError {
    EmptyStreamId,
    EmptyTopicPattern,
    InvalidTopicPattern(String),
}

impl std::fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyStreamId => write!(f, "stream_id cannot be empty"),
            Self::EmptyTopicPattern => write!(f, "topic_pattern cannot be empty"),
            Self::InvalidTopicPattern(msg) => write!(f, "invalid topic pattern: {}", msg),
        }
    }
}

impl std::error::Error for SubscriptionError {}

/// Validate an MQTT topic pattern.
fn validate_topic_pattern(pattern: &str) -> Result<(), SubscriptionError> {
    if pattern.starts_with('/') {
        return Err(SubscriptionError::InvalidTopicPattern(
            "pattern cannot start with /".to_string(),
        ));
    }
    if pattern.ends_with('/') && !pattern.ends_with("/#") {
        return Err(SubscriptionError::InvalidTopicPattern(
            "pattern cannot end with /".to_string(),
        ));
    }
    if pattern.contains("//") {
        return Err(SubscriptionError::InvalidTopicPattern(
            "pattern cannot contain empty segments (//)".to_string(),
        ));
    }

    // Validate # placement
    if let Some(pos) = pattern.find('#') {
        let after_hash = &pattern[pos + 1..];
        if !after_hash.is_empty() {
            return Err(SubscriptionError::InvalidTopicPattern(
                "# wildcard must be at end of pattern".to_string(),
            ));
        }
        // # must be preceded by / (unless it's the only character)
        if pos > 0 && !pattern[..pos].ends_with('/') {
            return Err(SubscriptionError::InvalidTopicPattern(
                "# wildcard must follow a /".to_string(),
            ));
        }
        // Only one # allowed
        if pattern.matches('#').count() > 1 {
            return Err(SubscriptionError::InvalidTopicPattern(
                "only one # wildcard allowed".to_string(),
            ));
        }
    }

    // Validate + placement (must be alone in segment)
    for segment in pattern.split('/') {
        if segment.contains('+') && segment != "+" {
            return Err(SubscriptionError::InvalidTopicPattern(
                format!("+ wildcard must be alone in segment, found: {}", segment),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_config_new() {
        let sub = SubscriptionConfig::new("air-quality", "airgradient/readings/+");
        assert_eq!(sub.stream_id, "air-quality");
        assert_eq!(sub.topic_pattern, "airgradient/readings/+");
        assert!(sub.enabled);
        assert!(sub.parser.is_none());
    }

    #[test]
    fn test_subscription_config_default_enabled() {
        let sub = SubscriptionConfig::default();
        assert!(sub.enabled);
    }

    #[test]
    fn test_subscription_config_with_enabled_false() {
        let sub = SubscriptionConfig::new("test", "test/+").with_enabled(false);
        assert!(!sub.enabled);
    }

    #[test]
    fn test_validate_empty_stream_id() {
        let sub = SubscriptionConfig {
            stream_id: "".to_string(),
            topic_pattern: "test/+".to_string(),
            parser: None,
            enabled: true,
        };
        assert!(matches!(sub.validate(), Err(SubscriptionError::EmptyStreamId)));
    }

    #[test]
    fn test_validate_empty_topic_pattern() {
        let sub = SubscriptionConfig {
            stream_id: "test".to_string(),
            topic_pattern: "".to_string(),
            parser: None,
            enabled: true,
        };
        assert!(matches!(sub.validate(), Err(SubscriptionError::EmptyTopicPattern)));
    }

    #[test]
    fn test_validate_pattern_starts_with_slash() {
        let result = validate_topic_pattern("/sensors/temp");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_pattern_ends_with_slash() {
        let result = validate_topic_pattern("sensors/temp/");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_pattern_double_slash() {
        let result = validate_topic_pattern("sensors//temp");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_hash_not_at_end() {
        let result = validate_topic_pattern("sensors/#/temp");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_multiple_hash() {
        let result = validate_topic_pattern("sensors/#/#");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_plus_mixed_segment() {
        let result = validate_topic_pattern("sensors/room+/temp");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid_single_level_wildcard() {
        let result = validate_topic_pattern("sensors/+/temp");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_valid_multi_level_wildcard() {
        let result = validate_topic_pattern("sensors/#");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_valid_combined_wildcards() {
        let result = validate_topic_pattern("home/+/devices/#");
        assert!(result.is_ok());
    }

    #[test]
    fn test_serde_deserialize() {
        let yaml = r#"
stream_id: air-quality
topic_pattern: "airgradient/readings/+"
enabled: true
"#;
        let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(sub.stream_id, "air-quality");
        assert_eq!(sub.topic_pattern, "airgradient/readings/+");
        assert!(sub.enabled);
    }

    #[test]
    fn test_serde_default_enabled() {
        let yaml = r#"
stream_id: test
topic_pattern: "test/+"
"#;
        let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(sub.enabled); // Default should be true
    }
}
```

---

### 3. `core/src/sources/mqtt/router.rs`

**Purpose**: TopicRouter implementation with MQTT pattern matching.

```rust
//! MQTT topic routing with pattern matching.

use regex::Regex;
use crate::parsers::ParserConfig;
use super::subscription::{SubscriptionConfig, SubscriptionError};

/// A compiled route entry for topic matching.
#[derive(Debug, Clone)]
pub struct RouteEntry {
    /// Original MQTT pattern (e.g., "airgradient/readings/+")
    pub pattern: String,
    /// Compiled regex for matching
    regex: Regex,
    /// Target stream ID
    pub stream_id: String,
    /// Parser configuration for this route
    pub parser_config: Option<ParserConfig>,
    /// Whether this route is enabled
    pub enabled: bool,
}

impl RouteEntry {
    /// Check if a topic matches this route.
    pub fn matches(&self, topic: &str) -> bool {
        self.regex.is_match(topic)
    }
}

/// Routes MQTT topics to streams based on pattern matching.
///
/// Uses first-match-wins semantics: the first matching route is returned.
#[derive(Debug)]
pub struct TopicRouter {
    routes: Vec<RouteEntry>,
}

impl TopicRouter {
    /// Create a new TopicRouter from subscription configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No enabled subscriptions are provided
    /// - A topic pattern is invalid
    pub fn new(subscriptions: Vec<SubscriptionConfig>) -> Result<Self, RouterError> {
        let mut routes = Vec::new();
        let mut seen_patterns = std::collections::HashSet::new();

        for sub in subscriptions {
            if !sub.enabled {
                tracing::debug!(stream_id = %sub.stream_id, "Skipping disabled subscription");
                continue;
            }

            // Validate subscription
            sub.validate().map_err(|e| RouterError::InvalidSubscription {
                stream_id: sub.stream_id.clone(),
                error: e,
            })?;

            // Check for duplicate patterns (warn but allow)
            if seen_patterns.contains(&sub.topic_pattern) {
                tracing::warn!(
                    pattern = %sub.topic_pattern,
                    "Duplicate topic pattern (first match wins)"
                );
            }
            seen_patterns.insert(sub.topic_pattern.clone());

            // Convert pattern to regex
            let regex = mqtt_pattern_to_regex(&sub.topic_pattern)
                .map_err(|e| RouterError::InvalidPattern {
                    pattern: sub.topic_pattern.clone(),
                    error: e,
                })?;

            routes.push(RouteEntry {
                pattern: sub.topic_pattern,
                regex,
                stream_id: sub.stream_id,
                parser_config: sub.parser,
                enabled: true,
            });
        }

        if routes.is_empty() {
            return Err(RouterError::NoEnabledSubscriptions);
        }

        tracing::info!(route_count = routes.len(), "Created TopicRouter");

        Ok(Self { routes })
    }

    /// Route a topic to its matching subscription.
    ///
    /// Returns the first matching route (first-match-wins).
    pub fn route(&self, topic: &str) -> Option<&RouteEntry> {
        if topic.is_empty() {
            tracing::warn!("Empty topic received");
            return None;
        }

        for route in &self.routes {
            if route.matches(topic) {
                tracing::debug!(
                    topic = %topic,
                    pattern = %route.pattern,
                    stream_id = %route.stream_id,
                    "Topic matched route"
                );
                return Some(route);
            }
        }

        tracing::debug!(topic = %topic, "No route found for topic");
        None
    }

    /// Get all topic patterns for MQTT subscription.
    pub fn topic_patterns(&self) -> Vec<&str> {
        self.routes.iter().map(|r| r.pattern.as_str()).collect()
    }

    /// Get the number of active routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

/// Errors from the topic router.
#[derive(Debug, Clone)]
pub enum RouterError {
    NoEnabledSubscriptions,
    InvalidSubscription {
        stream_id: String,
        error: SubscriptionError,
    },
    InvalidPattern {
        pattern: String,
        error: String,
    },
}

impl std::fmt::Display for RouterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEnabledSubscriptions => {
                write!(f, "no enabled subscriptions configured")
            }
            Self::InvalidSubscription { stream_id, error } => {
                write!(f, "invalid subscription '{}': {}", stream_id, error)
            }
            Self::InvalidPattern { pattern, error } => {
                write!(f, "invalid pattern '{}': {}", pattern, error)
            }
        }
    }
}

impl std::error::Error for RouterError {}

/// Convert an MQTT topic pattern to a regular expression.
///
/// # MQTT Wildcards
///
/// - `+` (single-level): Matches exactly one topic level
/// - `#` (multi-level): Matches zero or more topic levels (must be at end)
///
/// # Examples
///
/// - `sensors/+/temp` -> `^sensors/[^/]+/temp$`
/// - `sensors/#` -> `^sensors/.*$`
/// - `home/+/devices/#` -> `^home/[^/]+/devices/.*$`
pub fn mqtt_pattern_to_regex(pattern: &str) -> Result<Regex, String> {
    if pattern.is_empty() {
        return Err("pattern cannot be empty".to_string());
    }

    let mut regex_str = String::from("^");
    let segments: Vec<&str> = pattern.split('/').collect();

    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            regex_str.push('/');
        }

        match *segment {
            "+" => {
                // Single-level wildcard: match non-empty, non-slash string
                regex_str.push_str("[^/]+");
            }
            "#" => {
                // Multi-level wildcard: match everything (including empty)
                regex_str.push_str(".*");
                // # must be last segment
                break;
            }
            _ => {
                // Literal segment: escape regex special characters
                regex_str.push_str(&regex::escape(segment));
            }
        }
    }

    regex_str.push('$');

    Regex::new(&regex_str).map_err(|e| format!("regex compilation failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Pattern to Regex Tests ---

    #[test]
    fn test_pattern_literal() {
        let regex = mqtt_pattern_to_regex("sensors/room1/temp").unwrap();
        assert!(regex.is_match("sensors/room1/temp"));
        assert!(!regex.is_match("sensors/room2/temp"));
        assert!(!regex.is_match("sensors/room1/humidity"));
    }

    #[test]
    fn test_pattern_single_level_wildcard() {
        let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();
        assert!(regex.is_match("sensors/room1/temp"));
        assert!(regex.is_match("sensors/kitchen/temp"));
        assert!(!regex.is_match("sensors/temp"));
        assert!(!regex.is_match("sensors/room1/sub/temp"));
    }

    #[test]
    fn test_pattern_multi_level_wildcard() {
        let regex = mqtt_pattern_to_regex("sensors/#").unwrap();
        assert!(regex.is_match("sensors/"));
        assert!(regex.is_match("sensors/room1"));
        assert!(regex.is_match("sensors/room1/temp"));
        assert!(regex.is_match("sensors/room1/sub/deep/temp"));
        assert!(!regex.is_match("other/topic"));
    }

    #[test]
    fn test_pattern_combined_wildcards() {
        let regex = mqtt_pattern_to_regex("home/+/devices/#").unwrap();
        assert!(regex.is_match("home/room1/devices/"));
        assert!(regex.is_match("home/room1/devices/light"));
        assert!(regex.is_match("home/room1/devices/light/status"));
        assert!(!regex.is_match("home/devices/light"));
        assert!(!regex.is_match("home/room1/room2/devices/light"));
    }

    #[test]
    fn test_pattern_airgradient() {
        let regex = mqtt_pattern_to_regex("airgradient/readings/+").unwrap();
        assert!(regex.is_match("airgradient/readings/ABC123"));
        assert!(regex.is_match("airgradient/readings/d83bda1cd074"));
        assert!(!regex.is_match("airgradient/readings"));
        assert!(!regex.is_match("airgradient/readings/ABC/extra"));
    }

    #[test]
    fn test_pattern_homeassistant() {
        let regex = mqtt_pattern_to_regex("homeassistant/+/+/state").unwrap();
        assert!(regex.is_match("homeassistant/sensor/temp/state"));
        assert!(regex.is_match("homeassistant/binary_sensor/motion/state"));
        assert!(!regex.is_match("homeassistant/sensor/state"));
        assert!(!regex.is_match("homeassistant/sensor/room/temp/state"));
    }

    #[test]
    fn test_pattern_escaped_special_chars() {
        let regex = mqtt_pattern_to_regex("home/sensor.room1").unwrap();
        assert!(regex.is_match("home/sensor.room1"));
        assert!(!regex.is_match("home/sensorXroom1"));
    }

    #[test]
    fn test_pattern_empty_error() {
        let result = mqtt_pattern_to_regex("");
        assert!(result.is_err());
    }

    // --- Router Tests ---

    #[test]
    fn test_router_single_route() {
        let subs = vec![
            SubscriptionConfig::new("air-quality", "airgradient/readings/+"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        let route = router.route("airgradient/readings/ABC123");
        assert!(route.is_some());
        assert_eq!(route.unwrap().stream_id, "air-quality");
    }

    #[test]
    fn test_router_multiple_routes() {
        let subs = vec![
            SubscriptionConfig::new("air-quality", "airgradient/readings/+"),
            SubscriptionConfig::new("homeassistant", "homeassistant/+/+/state"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        let air = router.route("airgradient/readings/ABC123");
        assert_eq!(air.unwrap().stream_id, "air-quality");

        let ha = router.route("homeassistant/sensor/temp/state");
        assert_eq!(ha.unwrap().stream_id, "homeassistant");
    }

    #[test]
    fn test_router_first_match_wins() {
        let subs = vec![
            SubscriptionConfig::new("specific", "homeassistant/climate/+/state"),
            SubscriptionConfig::new("general", "homeassistant/+/+/state"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        // Climate topic should match specific route
        let route = router.route("homeassistant/climate/living_room/state");
        assert_eq!(route.unwrap().stream_id, "specific");

        // Sensor topic should match general route
        let route = router.route("homeassistant/sensor/temp/state");
        assert_eq!(route.unwrap().stream_id, "general");
    }

    #[test]
    fn test_router_no_match() {
        let subs = vec![
            SubscriptionConfig::new("test", "test/+"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        let route = router.route("unknown/topic");
        assert!(route.is_none());
    }

    #[test]
    fn test_router_empty_topic() {
        let subs = vec![
            SubscriptionConfig::new("test", "test/+"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        let route = router.route("");
        assert!(route.is_none());
    }

    #[test]
    fn test_router_disabled_subscription_skipped() {
        let subs = vec![
            SubscriptionConfig::new("disabled", "test/+").with_enabled(false),
            SubscriptionConfig::new("enabled", "test/#"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        assert_eq!(router.route_count(), 1);

        let route = router.route("test/topic");
        assert_eq!(route.unwrap().stream_id, "enabled");
    }

    #[test]
    fn test_router_no_enabled_subscriptions_error() {
        let subs = vec![
            SubscriptionConfig::new("disabled", "test/+").with_enabled(false),
        ];
        let result = TopicRouter::new(subs);
        assert!(matches!(result, Err(RouterError::NoEnabledSubscriptions)));
    }

    #[test]
    fn test_router_topic_patterns() {
        let subs = vec![
            SubscriptionConfig::new("a", "topic/a/+"),
            SubscriptionConfig::new("b", "topic/b/#"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        let patterns = router.topic_patterns();
        assert_eq!(patterns.len(), 2);
        assert!(patterns.contains(&"topic/a/+"));
        assert!(patterns.contains(&"topic/b/#"));
    }
}
```

---

## Existing Files to Modify

### 4. `core/src/sources/mqtt.rs`

**Changes**: Add subscriptions field, get_subscriptions() method, integrate TopicRouter.

```rust
// ADD: New imports
use crate::sources::mqtt::{SubscriptionConfig, TopicRouter, RouteEntry};

// MODIFY: MqttConfig struct
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,

    // NEW: Multiple subscriptions
    pub subscriptions: Vec<SubscriptionConfig>,

    // DEPRECATED: Legacy single topic pattern (backward compatibility)
    pub topic_pattern: Option<String>,

    pub qos: QoS,
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,
}

// ADD: get_subscriptions method
impl MqttConfig {
    /// Get all subscriptions (including legacy topic_pattern).
    ///
    /// If `subscriptions` is populated, returns those.
    /// If empty but `topic_pattern` is set, converts it to a subscription.
    pub fn get_subscriptions(&self) -> Vec<SubscriptionConfig> {
        let mut subs = self.subscriptions.clone();

        // Support legacy topic_pattern
        if let Some(pattern) = &self.topic_pattern {
            tracing::warn!(
                pattern = %pattern,
                "DEPRECATED: topic_pattern field is deprecated, use subscriptions array"
            );

            // Don't add if already exists
            if !subs.iter().any(|s| s.topic_pattern == *pattern) {
                subs.push(SubscriptionConfig {
                    stream_id: "legacy".to_string(),
                    topic_pattern: pattern.clone(),
                    parser: None,
                    enabled: true,
                });
            }
        }

        subs
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let subs = self.get_subscriptions();

        if subs.is_empty() {
            return Err(ConfigError::NoSubscriptions);
        }

        // Check for duplicate stream IDs
        let mut seen = std::collections::HashSet::new();
        for sub in &subs {
            if !seen.insert(&sub.stream_id) {
                return Err(ConfigError::DuplicateStreamId(sub.stream_id.clone()));
            }
        }

        Ok(())
    }
}

// ADD: ConfigError enum
#[derive(Debug, Clone)]
pub enum ConfigError {
    NoSubscriptions,
    DuplicateStreamId(String),
    InvalidPattern(String),
}

// MODIFY: MqttSource struct
pub struct MqttSource {
    config: MqttConfig,
    router: TopicRouter,  // NEW: Topic routing
    parser: Arc<dyn Parser + Send + Sync>,
    client: Option<AsyncClient>,
    receiver: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
    sender: mpsc::Sender<TimeSeriesPoint>,
    is_running: Arc<Mutex<bool>>,
    connection_healthy: Arc<Mutex<bool>>,
    cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
}

// MODIFY: MqttSource::new
impl MqttSource {
    pub fn new(config: MqttConfig, parser: Box<dyn Parser + Send + Sync>) -> Result<Self, CoreError> {
        // Validate config
        config.validate().map_err(|e| CoreError::Config(e.to_string()))?;

        // Build router from subscriptions
        let router = TopicRouter::new(config.get_subscriptions())
            .map_err(|e| CoreError::Config(e.to_string()))?;

        let (sender, receiver) = mpsc::channel(config.buffer_capacity);

        Ok(Self {
            config,
            router,
            parser: Arc::from(parser),
            client: None,
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_running: Arc::new(Mutex::new(false)),
            connection_healthy: Arc::new(Mutex::new(false)),
            cached_points: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

// MODIFY: process_events function - subscribe to all patterns
async fn process_events(
    config: MqttConfig,
    router: TopicRouter,  // NEW parameter
    parser: Arc<dyn Parser + Send + Sync>,
    mut event_loop: EventLoop,
    client: AsyncClient,
    cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
    is_running: Arc<Mutex<bool>>,
    connection_healthy: Arc<Mutex<bool>>,
) -> CoreResult<()> {
    // Subscribe to ALL topic patterns
    for pattern in router.topic_patterns() {
        client
            .subscribe(pattern, config.qos)
            .await
            .map_err(|e| CoreError::Source(format!("Failed to subscribe to {}: {}", pattern, e)))?;
        info!("Subscribed to topic pattern: {}", pattern);
    }

    // ... rest of event processing with routing
}

// MODIFY: Message handling to use router
// In the Publish handler:
Ok(Event::Incoming(Packet::Publish(publish))) => {
    debug!("Received MQTT message on topic: {}", publish.topic);

    // Route topic to subscription
    match router.route(&publish.topic) {
        Some(route) => {
            // Get parser (use route's parser or fall back to default)
            let parser_to_use = if route.parser_config.is_some() {
                // TODO: Create parser from config
                parser.clone()
            } else {
                parser.clone()
            };

            match serde_json::from_slice::<Value>(&publish.payload) {
                Ok(json) => {
                    let timestamp = Utc::now();
                    match parser_to_use.parse(&json, timestamp) {
                        Ok(mut points) => {
                            // Tag points with stream_id
                            for point in &mut points {
                                point.tags.insert("stream_id".to_string(), route.stream_id.clone());
                                point.tags.insert("topic".to_string(), publish.topic.clone());
                            }
                            let mut cache = cached_points.lock().await;
                            cache.extend(points);
                        }
                        Err(e) => {
                            error!(error = %e, topic = %publish.topic, "Failed to parse payload");
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, topic = %publish.topic, "Failed to parse JSON");
                }
            }
        }
        None => {
            warn!(topic = %publish.topic, "No route found for topic (dead letter)");
        }
    }
}
```

---

### 5. `core/src/sources/mod.rs`

**Changes**: Add mqtt module re-export.

```rust
// ADD: MQTT submodule
pub mod mqtt;

// Existing exports...
pub mod http_poll;
// etc.
```

---

### 6. `config/base/streams/air-quality/config.yaml`

**Changes**: Update to use new subscriptions format.

```yaml
# Air Quality Stream Configuration
# Using new multi-subscription format

sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "air-quality-app"
      qos: 1
      reconnect_delay_secs: 1
      max_reconnect_delay_secs: 30
      buffer_capacity: 1000

      # NEW: Subscriptions array
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "airgradient/readings/+"
          enabled: true
          parser:
            parser_type: flat_json
            location_id_field: serialno
            skip_fields:
              - serialno
              - firmware
              - model
              - ledMode
            default_tags:
              source: mqtt
              stream_id: air-quality

storage:
  # ... existing storage config
```

---

## Test Files to Create

### 7. `core/src/sources/mqtt/tests/subscription_tests.rs`

```rust
//! Unit tests for SubscriptionConfig.

use super::*;
use crate::parsers::{ParserConfig, ParserType};

#[test]
fn test_subscription_serde_full() {
    let yaml = r#"
stream_id: air-quality
topic_pattern: "airgradient/readings/+"
enabled: true
parser:
  parser_type: flat_json
  location_id_field: serialno
"#;
    let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(sub.stream_id, "air-quality");
    assert!(sub.parser.is_some());
}

// ... more tests as defined in TDD_SEQUENCE.md
```

### 8. `core/src/sources/mqtt/tests/router_tests.rs`

```rust
//! Unit tests for TopicRouter.

use super::*;

#[test]
fn test_router_complex_routing_scenario() {
    let subs = vec![
        SubscriptionConfig::new("hvac", "homeassistant/climate/+/state"),
        SubscriptionConfig::new("sensors", "homeassistant/sensor/+/state"),
        SubscriptionConfig::new("catchall", "homeassistant/#"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    // Climate -> hvac (first match)
    assert_eq!(
        router.route("homeassistant/climate/living_room/state").unwrap().stream_id,
        "hvac"
    );

    // Sensor -> sensors (second match)
    assert_eq!(
        router.route("homeassistant/sensor/temp/state").unwrap().stream_id,
        "sensors"
    );

    // Unknown HA topic -> catchall
    assert_eq!(
        router.route("homeassistant/switch/light/state").unwrap().stream_id,
        "catchall"
    );
}

// ... more tests as defined in TDD_SEQUENCE.md
```

---

## Summary

### Files to Create

| File | Purpose | Complexity |
|------|---------|------------|
| `core/src/sources/mqtt/mod.rs` | Module organization | S |
| `core/src/sources/mqtt/subscription.rs` | SubscriptionConfig struct | S |
| `core/src/sources/mqtt/router.rs` | TopicRouter implementation | M |
| `core/src/sources/mqtt/tests/subscription_tests.rs` | Subscription tests | S |
| `core/src/sources/mqtt/tests/router_tests.rs` | Router tests | M |

### Files to Modify

| File | Changes | Complexity |
|------|---------|------------|
| `core/src/sources/mqtt.rs` | Add subscriptions, router integration | L |
| `core/src/sources/mod.rs` | Add mqtt module export | S |
| `config/base/streams/air-quality/config.yaml` | New subscription format | S |

### Total Files

- **New files**: 5
- **Modified files**: 3
- **Total**: 8 files

---

## Related Documents

- IMPLEMENTATION_PLAN.md: Phase-by-phase implementation order
- TDD_SEQUENCE.md: Red-Green-Refactor test sequences
- TOPIC_ROUTER.md: Algorithm pseudocode
