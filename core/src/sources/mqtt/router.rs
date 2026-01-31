//! MQTT topic routing with pattern matching.
//!
//! This module provides the `TopicRouter` for routing MQTT messages
//! to appropriate streams based on topic pattern matching.

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
    /// AIR-012: Topic segment index to extract as ndp_id (0-indexed)
    pub ndp_id_topic_segment: Option<usize>,
}

impl RouteEntry {
    /// Check if a topic matches this route.
    pub fn matches(&self, topic: &str) -> bool {
        self.regex.is_match(topic)
    }

    /// AIR-012: Extract ndp_id from topic if configured.
    ///
    /// When `ndp_id_topic_segment` is set, extracts that segment from the topic
    /// as the ndp_id. This enables event-oriented streams where each device
    /// should have its own ndp_id derived from the topic path.
    ///
    /// # Example
    ///
    /// For topic "homeassistant/binary_sensor/door_backslider/state"
    /// with ndp_id_topic_segment = 2, returns Some("door_backslider")
    pub fn extract_ndp_id_from_topic(&self, topic: &str) -> Option<String> {
        let segment_index = self.ndp_id_topic_segment?;
        let segments: Vec<&str> = topic.split('/').collect();
        segments.get(segment_index).map(|s| s.to_string())
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
            sub.validate()
                .map_err(|e| RouterError::InvalidSubscription {
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
            let regex = mqtt_pattern_to_regex(&sub.topic_pattern).map_err(|e| {
                RouterError::InvalidPattern {
                    pattern: sub.topic_pattern.clone(),
                    error: e,
                }
            })?;

            routes.push(RouteEntry {
                pattern: sub.topic_pattern,
                regex,
                stream_id: sub.stream_id,
                parser_config: sub.parser,
                enabled: true,
                ndp_id_topic_segment: sub.ndp_id_topic_segment,
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
    fn test_pattern_literal_exact_match() {
        let regex = mqtt_pattern_to_regex("sensors/room1/temp").unwrap();
        assert!(regex.is_match("sensors/room1/temp"));
        assert!(!regex.is_match("sensors/room2/temp"));
        assert!(!regex.is_match("sensors/room1/humidity"));
    }

    #[test]
    fn test_pattern_single_level_wildcard_matches() {
        let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();
        assert!(regex.is_match("sensors/room1/temp"));
        assert!(regex.is_match("sensors/kitchen/temp"));
    }

    #[test]
    fn test_pattern_single_level_wildcard_requires_segment() {
        let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();
        assert!(!regex.is_match("sensors/temp")); // Missing level
        assert!(!regex.is_match("sensors//temp")); // Empty level
    }

    #[test]
    fn test_pattern_single_level_wildcard_single_level_only() {
        let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();
        assert!(!regex.is_match("sensors/room1/sub/temp")); // Too many levels
    }

    #[test]
    fn test_pattern_multi_level_wildcard_matches_zero_levels() {
        let regex = mqtt_pattern_to_regex("sensors/#").unwrap();
        assert!(regex.is_match("sensors/"));
    }

    #[test]
    fn test_pattern_multi_level_wildcard_matches_one_level() {
        let regex = mqtt_pattern_to_regex("sensors/#").unwrap();
        assert!(regex.is_match("sensors/room1"));
    }

    #[test]
    fn test_pattern_multi_level_wildcard_matches_many_levels() {
        let regex = mqtt_pattern_to_regex("sensors/#").unwrap();
        assert!(regex.is_match("sensors/room1/temp"));
        assert!(regex.is_match("sensors/room1/sub/deep/temp"));
    }

    #[test]
    fn test_pattern_multi_level_wildcard_no_match_different_prefix() {
        let regex = mqtt_pattern_to_regex("sensors/#").unwrap();
        assert!(!regex.is_match("other/room1/temp"));
    }

    #[test]
    fn test_pattern_combined_wildcards() {
        let regex = mqtt_pattern_to_regex("home/+/devices/#").unwrap();
        assert!(regex.is_match("home/room1/devices/"));
        assert!(regex.is_match("home/room1/devices/light"));
        assert!(regex.is_match("home/room1/devices/light/status"));
        assert!(!regex.is_match("home/devices/light")); // Missing single level
        assert!(!regex.is_match("home/room1/room2/devices/light")); // Extra level before devices
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
        let subs = vec![SubscriptionConfig::new(
            "air-quality",
            "airgradient/readings/+",
        )];
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
        let subs = vec![SubscriptionConfig::new("test", "test/+")];
        let router = TopicRouter::new(subs).unwrap();

        let route = router.route("unknown/topic");
        assert!(route.is_none());
    }

    #[test]
    fn test_router_empty_topic() {
        let subs = vec![SubscriptionConfig::new("test", "test/+")];
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
        let subs = vec![SubscriptionConfig::new("disabled", "test/+").with_enabled(false)];
        let result = TopicRouter::new(subs);
        assert!(matches!(result, Err(RouterError::NoEnabledSubscriptions)));
    }

    #[test]
    fn test_router_empty_subscriptions_error() {
        let subs: Vec<SubscriptionConfig> = vec![];
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

    #[test]
    fn test_router_route_count() {
        let subs = vec![
            SubscriptionConfig::new("a", "topic/a/+"),
            SubscriptionConfig::new("b", "topic/b/#"),
        ];
        let router = TopicRouter::new(subs).unwrap();

        assert_eq!(router.route_count(), 2);
    }

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
            router
                .route("homeassistant/climate/living_room/state")
                .unwrap()
                .stream_id,
            "hvac"
        );

        // Sensor -> sensors (second match)
        assert_eq!(
            router
                .route("homeassistant/sensor/temp/state")
                .unwrap()
                .stream_id,
            "sensors"
        );

        // Unknown HA topic -> catchall
        assert_eq!(
            router
                .route("homeassistant/switch/light/state")
                .unwrap()
                .stream_id,
            "catchall"
        );
    }

    // --- AIR-012: ndp_id Extraction Tests ---

    #[test]
    fn test_extract_ndp_id_from_topic_segment_2() {
        let sub = SubscriptionConfig::new("ha-state", "homeassistant/binary_sensor/+/state")
            .with_ndp_id_topic_segment(2);
        let router = TopicRouter::new(vec![sub]).unwrap();

        let route = router.route("homeassistant/binary_sensor/door_backslider/state").unwrap();
        let ndp_id = route.extract_ndp_id_from_topic("homeassistant/binary_sensor/door_backslider/state");

        assert_eq!(ndp_id, Some("door_backslider".to_string()));
    }

    #[test]
    fn test_extract_ndp_id_from_topic_different_segments() {
        // Test extracting from different segment indices
        let topic = "a/b/c/d/e";

        let sub0 = SubscriptionConfig::new("test", "a/+/+/+/+").with_ndp_id_topic_segment(0);
        let router0 = TopicRouter::new(vec![sub0]).unwrap();
        let route0 = router0.route(topic).unwrap();
        assert_eq!(route0.extract_ndp_id_from_topic(topic), Some("a".to_string()));

        let sub3 = SubscriptionConfig::new("test", "a/+/+/+/+").with_ndp_id_topic_segment(3);
        let router3 = TopicRouter::new(vec![sub3]).unwrap();
        let route3 = router3.route(topic).unwrap();
        assert_eq!(route3.extract_ndp_id_from_topic(topic), Some("d".to_string()));

        let sub4 = SubscriptionConfig::new("test", "a/+/+/+/+").with_ndp_id_topic_segment(4);
        let router4 = TopicRouter::new(vec![sub4]).unwrap();
        let route4 = router4.route(topic).unwrap();
        assert_eq!(route4.extract_ndp_id_from_topic(topic), Some("e".to_string()));
    }

    #[test]
    fn test_extract_ndp_id_from_topic_out_of_bounds() {
        let sub = SubscriptionConfig::new("test", "a/b/c")
            .with_ndp_id_topic_segment(10); // Out of bounds
        let router = TopicRouter::new(vec![sub]).unwrap();

        let route = router.route("a/b/c").unwrap();
        let ndp_id = route.extract_ndp_id_from_topic("a/b/c");

        assert_eq!(ndp_id, None);
    }

    #[test]
    fn test_extract_ndp_id_from_topic_not_configured() {
        // When ndp_id_topic_segment is not set, should return None
        let sub = SubscriptionConfig::new("test", "a/b/+");
        let router = TopicRouter::new(vec![sub]).unwrap();

        let route = router.route("a/b/c").unwrap();
        let ndp_id = route.extract_ndp_id_from_topic("a/b/c");

        assert_eq!(ndp_id, None);
    }

    #[test]
    fn test_extract_ndp_id_home_assistant_devices() {
        // Test realistic Home Assistant scenario with multiple devices
        let sub = SubscriptionConfig::new("ha-binary-sensor", "homeassistant/binary_sensor/+/state")
            .with_ndp_id_topic_segment(2);
        let router = TopicRouter::new(vec![sub]).unwrap();

        let devices = vec![
            ("homeassistant/binary_sensor/door_backslider/state", "door_backslider"),
            ("homeassistant/binary_sensor/door_officewindow/state", "door_officewindow"),
            ("homeassistant/binary_sensor/door_dinettewindow/state", "door_dinettewindow"),
        ];

        for (topic, expected_ndp_id) in devices {
            let route = router.route(topic).unwrap();
            let ndp_id = route.extract_ndp_id_from_topic(topic);
            assert_eq!(ndp_id, Some(expected_ndp_id.to_string()), "Failed for topic: {}", topic);
        }
    }
}
