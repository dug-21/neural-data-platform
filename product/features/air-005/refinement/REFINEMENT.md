# AIR-005: Weather Data Integration - Refinement Document

**Version**: 1.1.0
**Last Updated**: 2025-12-16
**Status**: Design Phase
**SPARC Phase**: Refinement

---

## Overview

This document outlines the Test-Driven Development (TDD) approach for refactoring `HttpPollingSource` into a generic, configuration-driven HTTP polling system that supports weather APIs through the `ResponseParser` trait.

**Feature**: AIR-005 - Weather Data Source Integration
**Architecture**: Generic HTTP polling with pluggable parsers
**Test Strategy**: London School TDD with comprehensive unit and integration tests

---

## Implementation Phases

### Phase 1: ResponseParser Trait and Registry (TDD Red-Green-Refactor)

**Objective**: Create the core abstraction for pluggable response parsing.

#### 1.1 Test Cases - ResponseParser Trait

**RED Phase - Write Failing Tests**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // ========== RESPONSEPARSER TRAIT TESTS ==========

    #[test]
    fn test_parser_registry_registers_builtin_parsers() {
        let registry = ParserRegistry::new();

        assert!(registry.get("openweather_current").is_some());
        assert!(registry.get("openweather_air_pollution").is_some());
    }

    #[test]
    fn test_parser_registry_returns_none_for_unknown() {
        let registry = ParserRegistry::new();

        assert!(registry.get("unknown_parser").is_none());
    }

    #[test]
    fn test_parser_registry_custom_parser_registration() {
        let mut registry = ParserRegistry::new();

        struct CustomParser;
        impl ResponseParser for CustomParser {
            fn name(&self) -> &'static str { "custom" }
            fn parse(&self, _: &str, _: &str, _: DateTime<Utc>)
                -> CoreResult<Vec<TimeSeriesPoint>> {
                Ok(vec![])
            }
        }

        registry.register("custom".to_string(), Arc::new(CustomParser));

        assert!(registry.get("custom").is_some());
    }

    // ========== WEATHER PARSER TESTS ==========

    #[test]
    fn test_weather_parser_parses_valid_response() {
        let parser = WeatherParser;
        let response = r#"{
            "main": {
                "temp": 22.5,
                "feels_like": 21.8,
                "pressure": 1013,
                "humidity": 65
            },
            "wind": {
                "speed": 3.5,
                "deg": 180,
                "gust": 5.2
            },
            "clouds": {"all": 10},
            "visibility": 10000,
            "dt": 1702742400
        }"#;

        let points = parser.parse(
            response,
            "test-location",
            Utc::now()
        ).unwrap();

        assert!(points.len() >= 6);

        // Verify temperature point
        let temp = points.iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature".to_string()))
            .expect("Should have temperature");
        assert_eq!(temp.value, 22.5);
    }

    #[test]
    fn test_weather_parser_handles_optional_fields() {
        let parser = WeatherParser;
        let response = r#"{
            "main": {
                "temp": 22.5,
                "feels_like": 21.8,
                "pressure": 1013,
                "humidity": 65
            },
            "wind": {"speed": 3.5},
            "clouds": {"all": 10}
        }"#;

        let points = parser.parse(response, "test", Utc::now()).unwrap();

        // Should parse without wind_deg, wind_gust, visibility
        assert!(points.len() >= 5);
    }

    #[test]
    fn test_weather_parser_fails_on_invalid_json() {
        let parser = WeatherParser;

        let result = parser.parse("not json", "test", Utc::now());

        assert!(result.is_err());
    }

    // ========== AIR POLLUTION PARSER TESTS ==========

    #[test]
    fn test_air_pollution_parser_parses_valid_response() {
        let parser = AirPollutionParser;
        let response = r#"{
            "list": [{
                "main": {"aqi": 2},
                "components": {
                    "co": 201.94,
                    "no": 0.01,
                    "no2": 0.77,
                    "o3": 68.66,
                    "so2": 0.64,
                    "pm2_5": 0.5,
                    "pm10": 0.54,
                    "nh3": 0.12
                },
                "dt": 1702742400
            }]
        }"#;

        let points = parser.parse(
            response,
            "test-location",
            Utc::now()
        ).unwrap();

        assert_eq!(points.len(), 9); // aqi + 8 pollutants

        let aqi = points.iter()
            .find(|p| p.tags.get("metric") == Some(&"aqi".to_string()))
            .expect("Should have AQI");
        assert_eq!(aqi.value, 2.0);
    }

    #[test]
    fn test_air_pollution_parser_fails_on_empty_list() {
        let parser = AirPollutionParser;
        let response = r#"{"list": []}"#;

        let result = parser.parse(response, "test", Utc::now());

        assert!(result.is_err());
    }
}
```

**GREEN Phase - Minimal Implementation**:

```rust
// core/src/sources/parsers/mod.rs

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use crate::{CoreResult, TimeSeriesPoint};

/// Trait for parsing HTTP responses into TimeSeriesPoints
pub trait ResponseParser: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;
}

/// Registry of available parsers
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn ResponseParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Arc<dyn ResponseParser>> = HashMap::new();
        parsers.insert("openweather_current".to_string(), Arc::new(WeatherParser));
        parsers.insert("openweather_air_pollution".to_string(), Arc::new(AirPollutionParser));
        Self { parsers }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn ResponseParser>> {
        self.parsers.get(name).cloned()
    }

    pub fn register(&mut self, name: String, parser: Arc<dyn ResponseParser>) {
        self.parsers.insert(name, parser);
    }
}
```

---

### Phase 2: AuthMethod and Retry Logic (TDD Red-Green-Refactor)

**Objective**: Implement flexible authentication and robust retry handling.

#### 2.1 Test Cases - Authentication

**RED Phase - Write Failing Tests**:

```rust
#[cfg(test)]
mod auth_tests {
    use super::*;

    #[test]
    fn test_auth_none_leaves_request_unchanged() {
        let auth = AuthMethod::None;
        let client = reqwest::Client::new();
        let request = client.get("http://example.com");

        let result = apply_auth(request, &auth);

        // Should not add any auth headers or params
        assert!(result.is_ok());
    }

    #[test]
    fn test_auth_query_param_adds_key() {
        std::env::set_var("TEST_API_KEY", "secret123");

        let auth = AuthMethod::QueryParam {
            param_name: "appid".to_string(),
            value_env: "TEST_API_KEY".to_string(),
        };

        let client = reqwest::Client::new();
        let request = client.get("http://example.com");

        let result = apply_auth(request, &auth).unwrap();
        let url = result.build().unwrap().url().to_string();

        assert!(url.contains("appid=secret123"));

        std::env::remove_var("TEST_API_KEY");
    }

    #[test]
    fn test_auth_query_param_fails_without_env_var() {
        std::env::remove_var("NONEXISTENT_KEY");

        let auth = AuthMethod::QueryParam {
            param_name: "appid".to_string(),
            value_env: "NONEXISTENT_KEY".to_string(),
        };

        let client = reqwest::Client::new();
        let request = client.get("http://example.com");

        let result = apply_auth(request, &auth);

        assert!(result.is_err());
    }
}
```

#### 2.2 Test Cases - Retry Logic

```rust
#[cfg(test)]
mod retry_tests {
    use super::*;

    #[test]
    fn test_classify_error_429_as_rate_limited() {
        let error_type = ErrorType::classify(Some(429), None);

        match error_type {
            ErrorType::RateLimited { .. } => {}
            _ => panic!("Should classify 429 as RateLimited"),
        }
    }

    #[test]
    fn test_classify_error_401_as_permanent() {
        let error_type = ErrorType::classify(Some(401), None);

        assert!(matches!(error_type, ErrorType::Permanent));
    }

    #[test]
    fn test_classify_error_500_as_transient() {
        let error_type = ErrorType::classify(Some(500), None);

        assert!(matches!(error_type, ErrorType::Transient));
    }

    #[test]
    fn test_calculate_delay_exponential_backoff() {
        let config = RetryConfig::default();

        let delay0 = calculate_delay(0, &config);
        let delay1 = calculate_delay(1, &config);
        let delay2 = calculate_delay(2, &config);

        // Each delay should be roughly 2x the previous
        assert!(delay1.as_millis() > delay0.as_millis());
        assert!(delay2.as_millis() > delay1.as_millis());
    }

    #[test]
    fn test_calculate_delay_respects_max() {
        let config = RetryConfig {
            max_delay_ms: 1000,
            ..Default::default()
        };

        let delay = calculate_delay(10, &config);

        assert!(delay.as_millis() <= 1000);
    }
}
```

---

### Phase 3: Generic HttpPollingSource (TDD Red-Green-Refactor)

**Objective**: Refactor HttpPollingSource to use EndpointConfig and ParserRegistry.

#### 3.1 Test Cases - HttpPollingSource

```rust
#[cfg(test)]
mod http_source_tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path, query_param};

    #[tokio::test]
    async fn test_http_source_creates_with_valid_config() {
        let config = HttpPollingConfig {
            poll_interval_secs: 300,
            timeout_secs: 30,
            buffer_capacity: 100,
            retry: RetryConfig::default(),
            endpoints: vec![create_test_endpoint()],
        };

        let source = HttpPollingSource::new(config);

        assert!(source.is_ok());
    }

    #[tokio::test]
    async fn test_http_source_fails_with_no_endpoints() {
        let config = HttpPollingConfig {
            endpoints: vec![],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config);

        assert!(source.is_err());
    }

    #[tokio::test]
    async fn test_poll_endpoint_uses_correct_parser() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/weather"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string(SAMPLE_WEATHER_JSON))
            .mount(&mock_server)
            .await;

        let endpoint = EndpointConfig {
            id: "test-weather".to_string(),
            url: format!("{}/weather", mock_server.uri()),
            auth: AuthMethod::None,
            parser_type: "openweather_current".to_string(),
            enabled: true,
            query_params: HashMap::new(),
        };

        let config = HttpPollingConfig {
            endpoints: vec![endpoint],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let points = source.poll_endpoint(&source.config.endpoints[0]).await.unwrap();

        assert!(!points.is_empty());
        assert!(points.iter().any(|p|
            p.tags.get("metric") == Some(&"temperature".to_string())
        ));
    }

    #[tokio::test]
    async fn test_poll_endpoint_applies_auth() {
        let mock_server = MockServer::start().await;
        std::env::set_var("TEST_KEY", "secret");

        Mock::given(method("GET"))
            .and(query_param("appid", "secret"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_string(SAMPLE_WEATHER_JSON))
            .mount(&mock_server)
            .await;

        let endpoint = EndpointConfig {
            url: mock_server.uri(),
            auth: AuthMethod::QueryParam {
                param_name: "appid".to_string(),
                value_env: "TEST_KEY".to_string(),
            },
            parser_type: "openweather_current".to_string(),
            ..Default::default()
        };

        let config = HttpPollingConfig {
            endpoints: vec![endpoint],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let result = source.poll_endpoint(&source.config.endpoints[0]).await;

        assert!(result.is_ok());
        std::env::remove_var("TEST_KEY");
    }

    #[tokio::test]
    async fn test_poll_with_retry_retries_transient_errors() {
        let mock_server = MockServer::start().await;
        let call_count = Arc::new(AtomicU32::new(0));
        let counter = call_count.clone();

        Mock::given(method("GET"))
            .respond_with(move |_| {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    ResponseTemplate::new(500)
                } else {
                    ResponseTemplate::new(200)
                        .set_body_string(SAMPLE_WEATHER_JSON)
                }
            })
            .mount(&mock_server)
            .await;

        let endpoint = EndpointConfig {
            url: mock_server.uri(),
            parser_type: "openweather_current".to_string(),
            ..Default::default()
        };

        let config = HttpPollingConfig {
            endpoints: vec![endpoint],
            retry: RetryConfig {
                max_retries: 3,
                initial_delay_ms: 10,
                ..Default::default()
            },
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let result = source.poll_with_retry(&source.config.endpoints[0]).await;

        assert!(result.is_ok());
        assert!(call_count.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn test_poll_with_retry_fails_on_permanent_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let endpoint = EndpointConfig {
            url: mock_server.uri(),
            ..Default::default()
        };

        let config = HttpPollingConfig {
            endpoints: vec![endpoint],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let result = source.poll_with_retry(&source.config.endpoints[0]).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_check_detects_stale_endpoints() {
        let config = HttpPollingConfig {
            poll_interval_secs: 60,
            endpoints: vec![create_test_endpoint()],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();

        // No polls yet - should be unhealthy
        let health = source.health_check().await.unwrap();

        assert!(!health.healthy);
    }
}
```

---

### Phase 4: Integration with Application

**Objective**: Wire HTTP polling into air-quality-app.

#### 4.1 Test Cases - Application Integration

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_start_weather_polling_with_valid_streams() {
        let mock_server = MockServer::start().await;
        setup_weather_mock(&mock_server).await;

        let streams = vec![create_mock_stream_config(&mock_server)];
        let (tx, _rx) = mpsc::channel(100);

        let result = start_weather_polling(streams, tx).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_start_weather_polling_fails_with_no_http_streams() {
        let streams: Vec<StreamConfig> = vec![];
        let (tx, _rx) = mpsc::channel(100);

        let result = start_weather_polling(streams, tx).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_points_flow_through_channel() {
        let mock_server = MockServer::start().await;
        setup_weather_mock(&mock_server).await;

        let streams = vec![create_mock_stream_config(&mock_server)];
        let (tx, mut rx) = mpsc::channel(100);

        let source = start_weather_polling(streams, tx).await.unwrap();
        source.poll_all_endpoints().await.unwrap();

        // Should receive points
        let point = tokio::time::timeout(
            Duration::from_secs(1),
            rx.recv()
        ).await.expect("Should receive").expect("Point exists");

        assert_eq!(point.tags.get("source"), Some(&"openweather".to_string()));
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let mock_server = MockServer::start().await;
        setup_weather_mock(&mock_server).await;

        let streams = vec![create_mock_stream_config(&mock_server)];
        let (tx, _rx) = mpsc::channel(100);

        let source = start_weather_polling(streams, tx).await.unwrap();
        source.start().await.unwrap();

        let result = source.stop().await;

        assert!(result.is_ok());
    }
}
```

---

## Test Strategy

### Unit Testing Approach

**London School TDD Principles**:
1. Test behavior, not implementation
2. Mock external dependencies (HTTP API, time)
3. Fast, isolated tests
4. One assertion per test (when practical)

**Coverage Targets**:
- Line coverage: 85%
- Branch coverage: 80%
- Function coverage: 90%

**Test Organization**:
```
core/src/sources/
  parsers/
    mod.rs                              // ResponseParser trait + registry
    weather.rs                          // WeatherParser implementation
    air_pollution.rs                    // AirPollutionParser implementation
  http_poll.rs                          // HttpPollingSource (refactored)
    #[cfg(test)] mod tests { ... }      // Unit tests inline

apps/air-quality-app/src/ingestion/
  http_handler.rs                       // Handler for weather data
    #[cfg(test)] mod tests { ... }

tests/integration/
  weather_integration_test.rs           // Real API tests (#[ignore])
  weather_mock_test.rs                  // Mock-based integration
```

### Edge Cases to Test

1. **Parser Edge Cases**:
   - Empty response body
   - Malformed JSON
   - Missing required fields
   - Extra unexpected fields
   - Null values in optional fields

2. **Auth Edge Cases**:
   - Missing environment variable
   - Empty API key
   - Special characters in API key

3. **Retry Edge Cases**:
   - Max retries exceeded
   - Rate limit with Retry-After header
   - Intermittent failures

4. **Health Check Edge Cases**:
   - No successful polls yet
   - Some endpoints healthy, some stale
   - All endpoints failed

---

## Code Review Checklist

### Functionality
- [ ] ResponseParser trait implemented correctly
- [ ] ParserRegistry registers built-in parsers
- [ ] AuthMethod supports query param, header, basic auth
- [ ] Retry logic respects max retries and backoff
- [ ] Health checks detect stale endpoints
- [ ] Error classification correct (transient vs permanent)

### Code Quality
- [ ] No unwrap() calls in production code
- [ ] All errors use CoreError types
- [ ] Logging at appropriate levels
- [ ] No hardcoded values (use configuration)
- [ ] Functions under 50 lines
- [ ] Modules under 500 lines

### Security
- [ ] API keys loaded from environment variables only
- [ ] API keys never logged
- [ ] HTTPS enforced (https_only in client)
- [ ] Input validation on all external data

### Performance
- [ ] HTTP client reused (not created per request)
- [ ] Parser registry created once at startup
- [ ] No busy-wait loops
- [ ] Async operations don't block executor

---

## Known Risks and Mitigations

### Risk 1: Unknown Parser Type

**Impact**: High
**Probability**: Low

**Description**: Configuration references a parser type that doesn't exist.

**Mitigation**:
1. Validate parser_type at config load time
2. Return clear error message with available parsers
3. Log all registered parsers at startup

### Risk 2: Auth Environment Variable Missing

**Impact**: High
**Probability**: Medium

**Description**: Required API key environment variable not set.

**Mitigation**:
1. Validate at startup, fail fast
2. Clear error message naming the missing variable
3. Document required environment variables

### Risk 3: Parser Version Mismatch

**Impact**: Medium
**Probability**: Low

**Description**: API response format changes, parser fails.

**Mitigation**:
1. Use serde(default) for optional fields
2. Log unexpected fields (don't fail)
3. Version parsers (e.g., "openweather_current_v2")
4. Integration tests catch breaking changes

---

## Acceptance Criteria

Feature is considered complete when:

1. **Functional**:
   - ResponseParser trait allows pluggable parsing
   - Weather and AirPollution parsers work correctly
   - Auth supports query param, header, basic auth
   - Retry logic handles transient errors
   - Health checks report per-endpoint status

2. **Quality**:
   - Test coverage ≥ 85%
   - All edge cases tested
   - Code review approved

3. **Documentation**:
   - Architecture document updated
   - Parser interface documented
   - Configuration examples provided

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-16 | SPARC Agent | Initial refinement |
| 1.1.0 | 2025-12-16 | SPARC Agent | Updated for generic HTTP polling with ResponseParser trait, AuthMethod, RetryConfig |

---

## References

- [AIR-005 Architecture](../architecture/ARCHITECTURE.md)
- [AIR-005 Pseudocode](../pseudocode/PSEUDOCODE.md)
- [Existing HttpPollingSource](../../../../core/src/sources/http_poll.rs)
- [OpenWeatherMap API Docs](https://openweathermap.org/current)
