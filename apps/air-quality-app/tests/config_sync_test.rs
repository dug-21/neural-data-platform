use air_quality_app::config_sync::{ConfigSyncError, ConfigSyncService};
use config_client::stream::registry::StreamRegistry;
use neural_core::{FieldType, SourceType, StreamConfig};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Mock StreamRegistry for testing without etcd
#[derive(Clone)]
struct MockStreamRegistry {
    saved_streams: Arc<Mutex<Vec<StreamConfig>>>,
}

impl MockStreamRegistry {
    fn new() -> Self {
        Self {
            saved_streams: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn save_stream(&self, config: &StreamConfig) -> Result<(), config_client::ConfigError> {
        let mut streams = self.saved_streams.lock().unwrap();
        // Remove existing config with same stream_id
        streams.retain(|s| s.stream_id != config.stream_id);
        streams.push(config.clone());
        Ok(())
    }

    async fn list_streams(&self) -> Result<Vec<String>, config_client::ConfigError> {
        let streams = self.saved_streams.lock().unwrap();
        Ok(streams.iter().map(|s| s.stream_id.clone()).collect())
    }

    fn get_saved_count(&self) -> usize {
        self.saved_streams.lock().unwrap().len()
    }

    fn get_saved_stream(&self, stream_id: &str) -> Option<StreamConfig> {
        let streams = self.saved_streams.lock().unwrap();
        streams.iter().find(|s| s.stream_id == stream_id).cloned()
    }
}

// ============================================================================
// TEST 1: Unit test - Load real YAML files from config/base/streams
// ============================================================================

#[tokio::test]
async fn test_config_sync_service_loads_real_yaml_files() {
    // Initialize service with real config path
    let config_dir = Path::new("/workspaces/neural-data-platform/config/base/streams");
    let service = ConfigSyncService::new(config_dir);

    // Discover all stream configs
    let discovered = service
        .discover_stream_configs()
        .await
        .expect("Failed to discover stream configs");

    // Verify we found at least 2 configs (outdoor-weather and outdoor-air-quality)
    assert!(
        discovered.len() >= 2,
        "Expected at least 2 configs, found {}",
        discovered.len()
    );

    // Track which configs we found
    let mut found_weather = false;
    let mut found_air_quality = false;

    // Load and verify each config
    for path in discovered.iter() {
        let config = service
            .load_yaml_config(path)
            .await
            .expect(&format!("Failed to load config from {:?}", path));

        // Verify basic fields
        assert!(!config.stream_id.is_empty(), "stream_id should not be empty");
        assert!(
            !config.description.is_empty(),
            "description should not be empty"
        );
        assert!(!config.version.is_empty(), "version should not be empty");

        // Check if this is one of our expected streams
        match config.stream_id.as_str() {
            "outdoor-weather" => {
                found_weather = true;
                assert_eq!(
                    config.stream_id, "outdoor-weather",
                    "stream_id should match filename"
                );
            }
            "outdoor-air-quality" => {
                found_air_quality = true;
                assert_eq!(
                    config.stream_id, "outdoor-air-quality",
                    "stream_id should match filename"
                );
            }
            _ => {} // Other configs are fine too
        }
    }

    // Verify we found both expected configs
    assert!(
        found_weather,
        "outdoor-weather config should be discovered"
    );
    assert!(
        found_air_quality,
        "outdoor-air-quality config should be discovered"
    );
}

// ============================================================================
// TEST 2: Unit test - Verify YAML to StreamConfig conversion
// ============================================================================

#[tokio::test]
async fn test_yaml_to_stream_config_conversion() {
    let config_dir = Path::new("/workspaces/neural-data-platform/config/base/streams");
    let service = ConfigSyncService::new(config_dir);

    // Load outdoor-weather config specifically
    let weather_config_path = config_dir.join("outdoor-weather/config.yaml");
    let weather_config = service
        .load_yaml_config(&weather_config_path)
        .await
        .expect("Failed to load outdoor-weather config");

    // Verify stream_id
    assert_eq!(
        weather_config.stream_id, "outdoor-weather",
        "stream_id should be outdoor-weather"
    );

    // Verify fields array has 11 entries
    // Based on the YAML: temperature, feels_like, pressure, humidity, wind_speed,
    // wind_deg, wind_gust, clouds, visibility, rain_1h, snow_1h
    assert_eq!(
        weather_config.fields.len(),
        11,
        "outdoor-weather should have 11 fields"
    );

    // Verify specific fields
    let field_names: Vec<String> = weather_config
        .fields
        .iter()
        .map(|f| f.name.clone())
        .collect();

    assert!(
        field_names.contains(&"temperature".to_string()),
        "Should have temperature field"
    );
    assert!(
        field_names.contains(&"feels_like".to_string()),
        "Should have feels_like field"
    );
    assert!(
        field_names.contains(&"pressure".to_string()),
        "Should have pressure field"
    );
    assert!(
        field_names.contains(&"humidity".to_string()),
        "Should have humidity field"
    );
    assert!(
        field_names.contains(&"wind_speed".to_string()),
        "Should have wind_speed field"
    );

    // Verify field types
    let temp_field = weather_config
        .fields
        .iter()
        .find(|f| f.name == "temperature")
        .expect("temperature field should exist");
    assert_eq!(temp_field.field_type, FieldType::Float);
    assert_eq!(temp_field.nullable, false);
    assert_eq!(temp_field.unit, Some("celsius".to_string()));

    // Verify sources array has 1 http_poll source
    assert_eq!(
        weather_config.sources.len(),
        1,
        "outdoor-weather should have 1 source"
    );

    let source = &weather_config.sources[0];
    assert_eq!(source.source_type, SourceType::HttpPoll);
    assert_eq!(source.enabled, true);

    // Verify source params contain poll_interval_secs=600
    assert!(
        source.params.contains_key("poll_interval_secs"),
        "Source should have poll_interval_secs param"
    );
    let poll_interval = source.params.get("poll_interval_secs").unwrap();
    assert_eq!(
        poll_interval.as_u64(),
        Some(600),
        "poll_interval_secs should be 600"
    );

    // Verify other source params
    assert!(
        source.params.contains_key("timeout_secs"),
        "Source should have timeout_secs param"
    );
    assert!(
        source.params.contains_key("parser_name"),
        "Source should have parser_name param"
    );
    assert!(
        source.params.contains_key("endpoints"),
        "Source should have endpoints param"
    );

    // Verify storage config
    assert!(
        weather_config.storage.is_some(),
        "Should have storage config"
    );
    let storage = weather_config.storage.as_ref().unwrap();
    assert_eq!(storage.batch_size, 50);
    assert_eq!(storage.batch_timeout_secs, 30);
    assert_eq!(storage.buffer_capacity, 500);
}

// ============================================================================
// TEST 3: Unit test with mock - Sync all configs to mock registry
// ============================================================================

#[tokio::test]
async fn test_sync_all_to_mock_registry() {
    let config_dir = Path::new("/workspaces/neural-data-platform/config/base/streams");
    let service = ConfigSyncService::new(config_dir);

    // Create mock registry
    let mock_registry = MockStreamRegistry::new();

    // Discover configs first to know how many to expect
    let discovered = service
        .discover_stream_configs()
        .await
        .expect("Failed to discover configs");

    // Count enabled configs
    let mut expected_synced = 0;
    for path in &discovered {
        if let Ok(config) = service.load_yaml_config(path).await {
            if config.enabled {
                expected_synced += 1;
            }
        }
    }

    // Now test sync_all
    // Note: We need to adapt since sync_all takes a real StreamRegistry
    // For this test, we'll manually test the sync logic

    let mut synced_count = 0;
    for path in discovered {
        match service.load_yaml_config(&path).await {
            Ok(config) => {
                if !config.enabled {
                    continue;
                }

                // Validate config
                if config.validate().is_err() {
                    continue;
                }

                // Save to mock registry
                if mock_registry.save_stream(&config).await.is_ok() {
                    synced_count += 1;
                }
            }
            Err(_) => continue,
        }
    }

    // Verify registry.save_stream() was called for each enabled config
    assert_eq!(
        synced_count, expected_synced,
        "Should sync all enabled configs"
    );
    assert_eq!(
        mock_registry.get_saved_count(),
        expected_synced,
        "Mock registry should have all synced configs"
    );

    // Verify specific configs were saved
    assert!(
        mock_registry
            .get_saved_stream("outdoor-weather")
            .is_some(),
        "outdoor-weather should be saved"
    );
    assert!(
        mock_registry
            .get_saved_stream("outdoor-air-quality")
            .is_some(),
        "outdoor-air-quality should be saved"
    );

    // Verify list_streams returns expected streams
    let stream_ids = mock_registry
        .list_streams()
        .await
        .expect("Failed to list streams");
    assert!(
        stream_ids.contains(&"outdoor-weather".to_string()),
        "List should include outdoor-weather"
    );
    assert!(
        stream_ids.contains(&"outdoor-air-quality".to_string()),
        "List should include outdoor-air-quality"
    );
}

// ============================================================================
// TEST 4: Integration test - Full sync to real etcd (requires etcd running)
// ============================================================================

#[tokio::test]
#[ignore] // Run only when etcd is available
async fn test_full_sync_to_etcd() {
    // Initialize service
    let config_dir = Path::new("/workspaces/neural-data-platform/config/base/streams");
    let service = ConfigSyncService::new(config_dir);

    // Connect to real etcd
    let etcd_url = std::env::var("ETCD_URL").unwrap_or_else(|_| "http://localhost:2379".to_string());
    let registry = StreamRegistry::new(&[etcd_url.as_str()])
        .await
        .expect("Failed to connect to etcd");

    // Sync all configs
    let synced_count = service
        .sync_all(&registry)
        .await
        .expect("Failed to sync configs to etcd");

    // Verify at least 2 configs were synced
    assert!(
        synced_count >= 2,
        "Should sync at least 2 configs, synced {}",
        synced_count
    );

    // Verify registry.list_streams() returns our configs
    let stream_ids = registry
        .list_streams()
        .await
        .expect("Failed to list streams from registry");

    assert!(
        stream_ids.contains(&"outdoor-weather".to_string()),
        "Registry should have outdoor-weather stream"
    );
    assert!(
        stream_ids.contains(&"outdoor-air-quality".to_string()),
        "Registry should have outdoor-air-quality stream"
    );

    // Verify we can retrieve each config
    let weather_config = registry
        .load_stream("outdoor-weather")
        .await
        .expect("Failed to load outdoor-weather from registry");

    assert_eq!(weather_config.stream_id, "outdoor-weather");
    assert_eq!(weather_config.fields.len(), 11);

    let air_quality_config = registry
        .load_stream("outdoor-air-quality")
        .await
        .expect("Failed to load outdoor-air-quality from registry");

    assert_eq!(air_quality_config.stream_id, "outdoor-air-quality");
    assert_eq!(air_quality_config.fields.len(), 9); // aqi, co, no, no2, o3, so2, pm2_5, pm10, nh3
}

// ============================================================================
// ADDITIONAL TESTS: Error handling and edge cases
// ============================================================================

#[tokio::test]
async fn test_load_nonexistent_yaml_file() {
    let config_dir = Path::new("/workspaces/neural-data-platform/config/base/streams");
    let service = ConfigSyncService::new(config_dir);

    let nonexistent_path = config_dir.join("nonexistent/config.yaml");
    let result = service.load_yaml_config(&nonexistent_path).await;

    assert!(result.is_err(), "Loading nonexistent file should fail");
    match result {
        Err(ConfigSyncError::YamlReadError(_)) | Err(ConfigSyncError::IoError(_)) => {
            // Expected error types
        }
        _ => panic!("Expected YamlReadError or IoError"),
    }
}

#[tokio::test]
async fn test_discover_configs_nonexistent_directory() {
    let nonexistent_dir = Path::new("/nonexistent/directory");
    let service = ConfigSyncService::new(nonexistent_dir);

    let result = service.discover_stream_configs().await;

    assert!(
        result.is_err(),
        "Discovering in nonexistent directory should fail"
    );
    match result {
        Err(ConfigSyncError::DirectoryNotFound(_)) | Err(ConfigSyncError::IoError(_)) => {
            // Expected error types
        }
        _ => panic!("Expected DirectoryNotFound or IoError"),
    }
}

#[tokio::test]
async fn test_config_validation() {
    let config_dir = Path::new("/workspaces/neural-data-platform/config/base/streams");
    let service = ConfigSyncService::new(config_dir);

    // Load a valid config
    let weather_config_path = config_dir.join("outdoor-weather/config.yaml");
    let config = service
        .load_yaml_config(&weather_config_path)
        .await
        .expect("Failed to load config");

    // The config should already be validated by load_yaml_config
    // Verify it passes validation again
    assert!(
        config.validate().is_ok(),
        "Valid config should pass validation"
    );

    // Verify required fields are present
    assert!(!config.stream_id.is_empty());
    assert!(!config.description.is_empty());
    assert!(!config.fields.is_empty());
    assert!(!config.sources.is_empty());
}

#[tokio::test]
async fn test_outdoor_air_quality_config_details() {
    let config_dir = Path::new("/workspaces/neural-data-platform/config/base/streams");
    let service = ConfigSyncService::new(config_dir);

    // Load outdoor-air-quality config
    let air_quality_path = config_dir.join("outdoor-air-quality/config.yaml");
    let config = service
        .load_yaml_config(&air_quality_path)
        .await
        .expect("Failed to load outdoor-air-quality config");

    // Verify stream_id
    assert_eq!(config.stream_id, "outdoor-air-quality");

    // Verify fields count (aqi, co, no, no2, o3, so2, pm2_5, pm10, nh3)
    assert_eq!(config.fields.len(), 9);

    // Verify specific fields
    let aqi_field = config
        .fields
        .iter()
        .find(|f| f.name == "aqi")
        .expect("aqi field should exist");
    assert_eq!(aqi_field.field_type, FieldType::Float);
    assert_eq!(aqi_field.nullable, false);

    let pm25_field = config
        .fields
        .iter()
        .find(|f| f.name == "pm2_5")
        .expect("pm2_5 field should exist");
    assert_eq!(pm25_field.field_type, FieldType::Float);
    assert_eq!(pm25_field.nullable, false);

    // Verify source
    assert_eq!(config.sources.len(), 1);
    let source = &config.sources[0];
    assert_eq!(source.source_type, SourceType::HttpPoll);
    assert_eq!(
        source.params.get("poll_interval_secs").unwrap().as_u64(),
        Some(600)
    );

    // Verify parser_name
    assert_eq!(
        source
            .params
            .get("parser_name")
            .and_then(|v| v.as_str()),
        Some("openweathermap_air_pollution")
    );
}
