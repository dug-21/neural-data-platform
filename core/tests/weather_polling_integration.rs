//! Integration tests for OpenWeatherMap polling workflow
//!
//! Tests the complete flow from HTTP polling to parsed TimeSeriesPoints

use platform_core::sources::{
    AuthMethod, EndpointConfig, GenericHttpPollingConfig, GenericHttpPollingSource, RetryConfig,
};
use platform_core::traits::Source;
use std::time::Duration;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Sample OpenWeatherMap Current Weather API response
fn weather_api_response() -> &'static str {
    r#"{
        "main": {
            "temp": 20.5,
            "feels_like": 19.8,
            "pressure": 1015.0,
            "humidity": 65.0
        },
        "wind": {
            "speed": 3.5,
            "deg": 180.0,
            "gust": 5.2
        },
        "clouds": {
            "all": 40.0
        },
        "visibility": 10000.0,
        "rain": {
            "1h": 0.5
        }
    }"#
}

/// Sample OpenWeatherMap Air Pollution API response
fn air_pollution_api_response() -> &'static str {
    r#"{
        "list": [{
            "main": {
                "aqi": 2
            },
            "components": {
                "co": 230.31,
                "no": 0.51,
                "no2": 15.34,
                "o3": 68.66,
                "so2": 3.73,
                "pm2_5": 8.59,
                "pm10": 12.15,
                "nh3": 0.92
            }
        }]
    }"#
}

#[tokio::test]
async fn test_weather_polling_integration() {
    let mock_server = MockServer::start().await;

    // Mock the weather endpoint
    Mock::given(method("GET"))
        .and(path("/data/2.5/weather"))
        .and(query_param("appid", "test_api_key"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "weather_test",
        format!("{}/data/2.5/weather", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    )
    .with_auth(AuthMethod::QueryParam {
        key: "appid".to_string(),
        value: "test_api_key".to_string(),
    });

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    // Wait for initial poll
    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();

    // Should have parsed weather data
    assert!(!points.is_empty());

    // Verify temperature point
    let temp_point = points
        .iter()
        .find(|p| p.tags.get("metric") == Some(&"temperature".to_string()));
    assert!(temp_point.is_some());
    assert_eq!(temp_point.unwrap().value, 20.5);

    // Verify other metrics exist
    assert!(points
        .iter()
        .any(|p| p.tags.get("metric") == Some(&"feels_like".to_string())));
    assert!(points
        .iter()
        .any(|p| p.tags.get("metric") == Some(&"pressure".to_string())));
    assert!(points
        .iter()
        .any(|p| p.tags.get("metric") == Some(&"humidity".to_string())));
    assert!(points
        .iter()
        .any(|p| p.tags.get("metric") == Some(&"wind_speed".to_string())));
    assert!(points
        .iter()
        .any(|p| p.tags.get("metric") == Some(&"wind_deg".to_string())));

    // Verify all points have correct location and source tags
    for point in &points {
        assert_eq!(point.location_id, "test_location");
        assert_eq!(
            point.tags.get("source"),
            Some(&"openweathermap".to_string())
        );
        assert_eq!(point.tags.get("api"), Some(&"current_weather".to_string()));
    }

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_air_pollution_polling_integration() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/data/2.5/air_pollution"))
        .and(query_param("appid", "test_api_key"))
        .respond_with(ResponseTemplate::new(200).set_body_string(air_pollution_api_response()))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "pollution_test",
        format!("{}/data/2.5/air_pollution", mock_server.uri()),
        "test_location",
        "openweathermap_air_pollution",
    )
    .with_auth(AuthMethod::QueryParam {
        key: "appid".to_string(),
        value: "test_api_key".to_string(),
    });

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();

    assert!(!points.is_empty());

    // Verify AQI point
    let aqi_point = points
        .iter()
        .find(|p| p.tags.get("metric") == Some(&"aqi".to_string()));
    assert!(aqi_point.is_some());
    assert_eq!(aqi_point.unwrap().value, 2.0);

    // Verify PM2.5 point
    let pm25_point = points
        .iter()
        .find(|p| p.tags.get("metric") == Some(&"pm2_5".to_string()));
    assert!(pm25_point.is_some());
    assert_eq!(pm25_point.unwrap().value, 8.59);

    // Verify PM10 point
    let pm10_point = points
        .iter()
        .find(|p| p.tags.get("metric") == Some(&"pm10".to_string()));
    assert!(pm10_point.is_some());
    assert_eq!(pm10_point.unwrap().value, 12.15);

    // Verify CO point
    let co_point = points
        .iter()
        .find(|p| p.tags.get("metric") == Some(&"co".to_string()));
    assert!(co_point.is_some());
    assert_eq!(co_point.unwrap().value, 230.31);

    // Verify all points have correct location and source tags
    for point in &points {
        assert_eq!(point.location_id, "test_location");
        assert_eq!(
            point.tags.get("source"),
            Some(&"openweathermap".to_string())
        );
        assert_eq!(point.tags.get("api"), Some(&"air_pollution".to_string()));
    }

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_retry_on_transient_error() {
    let mock_server = MockServer::start().await;

    // First request fails with 503, second succeeds
    Mock::given(method("GET"))
        .and(path("/data/2.5/weather"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/data/2.5/weather"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "retry_test",
        format!("{}/data/2.5/weather", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    );

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        },
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();
    assert!(!points.is_empty()); // Should have succeeded after retry

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_health_check_reports_unhealthy_endpoints() {
    let config = GenericHttpPollingConfig {
        endpoints: vec![EndpointConfig::new(
            "unreachable",
            "http://unreachable.invalid/api",
            "test_location",
            "openweathermap_current_weather",
        )],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_millis(100),
        retry_config: RetryConfig {
            max_retries: 0, // No retries
            ..Default::default()
        },
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let health = source.health_check().await.unwrap();
    // Should be unhealthy due to failed endpoint
    assert!(!health.healthy || health.details.get("unhealthy_endpoints").is_some());

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_multiple_endpoints_polling() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/weather"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/pollution"))
        .respond_with(ResponseTemplate::new(200).set_body_string(air_pollution_api_response()))
        .mount(&mock_server)
        .await;

    let config = GenericHttpPollingConfig {
        endpoints: vec![
            EndpointConfig::new(
                "weather",
                format!("{}/weather", mock_server.uri()),
                "location1",
                "openweathermap_current_weather",
            ),
            EndpointConfig::new(
                "pollution",
                format!("{}/pollution", mock_server.uri()),
                "location1",
                "openweathermap_air_pollution",
            ),
        ],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();

    // Should have points from both endpoints
    assert!(points.len() > 15); // Weather (11) + Pollution (9)

    // Verify both sources
    assert!(points
        .iter()
        .any(|p| p.tags.get("api") == Some(&"current_weather".to_string())));
    assert!(points
        .iter()
        .any(|p| p.tags.get("api") == Some(&"air_pollution".to_string())));

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_authentication_methods() {
    let mock_server = MockServer::start().await;

    // Test Query Parameter Auth
    Mock::given(method("GET"))
        .and(path("/query-auth"))
        .and(query_param("apikey", "test_key"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "query_auth_test",
        format!("{}/query-auth", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    )
    .with_auth(AuthMethod::QueryParam {
        key: "apikey".to_string(),
        value: "test_key".to_string(),
    });

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();
    assert!(!points.is_empty());

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_timeout_handling() {
    let mock_server = MockServer::start().await;

    // Simulate a slow endpoint
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(weather_api_response())
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "timeout_test",
        format!("{}/slow", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    );

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_millis(500), // Short timeout
        retry_config: RetryConfig {
            max_retries: 0, // No retries
            ..Default::default()
        },
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_secs(1)).await;

    // Should have no points due to timeout
    let points = source.fetch().await.unwrap();
    assert_eq!(points.len(), 0);

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_parser_error_handling() {
    let mock_server = MockServer::start().await;

    // Return invalid JSON
    Mock::given(method("GET"))
        .and(path("/invalid"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{ invalid json }"))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "invalid_json_test",
        format!("{}/invalid", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    );

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Should have no points due to parsing error
    let points = source.fetch().await.unwrap();
    assert_eq!(points.len(), 0);

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_rate_limiting_429_response() {
    let mock_server = MockServer::start().await;

    // First request returns 429, then succeeds
    Mock::given(method("GET"))
        .and(path("/rate-limited"))
        .respond_with(ResponseTemplate::new(429).set_body_string("Rate limited"))
        .up_to_n_times(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rate-limited"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "rate_limit_test",
        format!("{}/rate-limited", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    );

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        },
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();
    assert!(!points.is_empty()); // Should succeed after retry

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_permanent_error_no_retry() {
    let mock_server = MockServer::start().await;

    // Always return 404 - permanent errors should not retry
    // Note: May be called during initial poll and background poll
    Mock::given(method("GET"))
        .and(path("/not-found"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "not_found_test",
        format!("{}/not-found", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    );

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        },
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();
    assert_eq!(points.len(), 0); // Should have no points

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_disabled_endpoint_not_polled() {
    let mock_server = MockServer::start().await;

    // This should never be called
    Mock::given(method("GET"))
        .and(path("/disabled"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .expect(0)
        .mount(&mock_server)
        .await;

    let mut endpoint = EndpointConfig::new(
        "disabled_test",
        format!("{}/disabled", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    );
    endpoint.enabled = false;

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    // Should fail to start because no enabled endpoints
    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    let result = source.start().await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_custom_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/custom-headers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "headers_test",
        format!("{}/custom-headers", mock_server.uri()),
        "test_location",
        "openweathermap_current_weather",
    )
    .with_header("User-Agent", "TestClient/1.0")
    .with_header("Accept", "application/json");

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let points = source.fetch().await.unwrap();
    assert!(!points.is_empty());

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_endpoint_polling() {
    let mock_server = MockServer::start().await;

    // Setup multiple endpoints with delays to test concurrency
    for i in 1..=3 {
        Mock::given(method("GET"))
            .and(path(format!("/endpoint{}", i)))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(weather_api_response())
                    .set_delay(Duration::from_millis(100)),
            )
            .mount(&mock_server)
            .await;
    }

    let endpoints = (1..=3)
        .map(|i| {
            EndpointConfig::new(
                format!("endpoint{}", i),
                format!("{}/endpoint{}", mock_server.uri(), i),
                format!("location{}", i),
                "openweathermap_current_weather",
            )
        })
        .collect();

    let config = GenericHttpPollingConfig {
        endpoints,
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();

    let start = std::time::Instant::now();
    source.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let duration = start.elapsed();

    let points = source.fetch().await.unwrap();

    // Should have points from all three endpoints
    assert!(points.len() >= 30); // ~11 points per endpoint * 3

    // Even though each endpoint has 100ms delay, they should poll somewhat concurrently
    // So total time should be less than 3 * 100ms + overhead
    assert!(duration < Duration::from_millis(1000));

    source.stop().await.unwrap();
}

#[tokio::test]
async fn test_missing_parser_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/unknown-parser"))
        .respond_with(ResponseTemplate::new(200).set_body_string(weather_api_response()))
        .mount(&mock_server)
        .await;

    let endpoint = EndpointConfig::new(
        "unknown_parser_test",
        format!("{}/unknown-parser", mock_server.uri()),
        "test_location",
        "nonexistent_parser", // Parser that doesn't exist
    );

    let config = GenericHttpPollingConfig {
        endpoints: vec![endpoint],
        poll_interval: Duration::from_secs(60),
        timeout: Duration::from_secs(10),
        retry_config: RetryConfig::default(),
        buffer_capacity: 100,
    };

    let mut source = GenericHttpPollingSource::with_default_parsers(config).unwrap();
    source.start().await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Should have no points due to missing parser
    let points = source.fetch().await.unwrap();
    assert_eq!(points.len(), 0);

    source.stop().await.unwrap();
}
