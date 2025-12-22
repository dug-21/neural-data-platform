//! Stream configuration loading from etcd (GitOps YAML approach)
//!
//! Loads stream configuration from flattened etcd keys synced via GitOps.
//! Keys are synced from config/base/streams/{stream-id}/config.yaml
//! to /streams/{stream-id}/* in etcd.

use crate::config::{AppConfig, MqttConfig, ServerConfig, StorageConfig};
use config_client::ConfigClient;
use config_client::ConfigError;
use tracing::{debug, info, warn};

/// Load application configuration from stream-specific etcd keys
///
/// Reads from `/streams/{stream_id}/*` keys that are synced from GitOps YAML files.
/// This is the preferred method - uses the same GitOps approach as other configs.
pub async fn load_from_stream_config(
    etcd_endpoints: &[&str],
    stream_id: &str,
) -> Result<AppConfig, ConfigError> {
    let prefix = format!("/streams/{}", stream_id);
    info!("Loading stream configuration from etcd prefix: {}", prefix);

    let client = ConfigClient::with_prefix(etcd_endpoints, &prefix).await?;

    // Check if stream exists by looking for stream_id key
    let stored_stream_id: Result<String, _> = client.get("/stream_id").await;
    if stored_stream_id.is_err() {
        return Err(ConfigError::NotFound(format!(
            "Stream '{}' not found at {}/stream_id",
            stream_id, prefix
        )));
    }

    // Check if stream is enabled
    let enabled: bool = client.get("/enabled").await.unwrap_or(true);
    if !enabled {
        warn!("Stream '{}' is disabled in configuration", stream_id);
    }

    // Load MQTT configuration from /streams/{stream_id}/mqtt/*
    let mqtt_config = load_mqtt_config(&client).await?;

    // Load storage configuration from /streams/{stream_id}/storage/*
    let storage_config = load_storage_config(&client).await;

    // Server config comes from environment variables (not stream-specific)
    let server_config = load_server_config();

    info!(
        "Successfully loaded stream configuration for '{}'",
        stream_id
    );

    Ok(AppConfig {
        server: server_config,
        mqtt: mqtt_config,
        storage: storage_config,
    })
}

/// Load MQTT configuration from etcd keys
async fn load_mqtt_config(client: &ConfigClient) -> Result<MqttConfig, ConfigError> {
    debug!("Loading MQTT configuration from etcd");

    // broker_url is required
    let broker_url: String = client
        .get("/mqtt/broker_url")
        .await
        .map_err(|_| ConfigError::NotFound("Missing required key: mqtt/broker_url".to_string()))?;

    // Other MQTT settings with defaults
    let port: u16 = client.get("/mqtt/port").await.unwrap_or(1883);
    let client_id: String = client
        .get("/mqtt/client_id")
        .await
        .unwrap_or_else(|_| "air-quality-app".to_string());
    let topic_pattern: String = client
        .get("/mqtt/topic_pattern")
        .await
        .unwrap_or_else(|_| "airgradient/readings/+".to_string());
    let qos: u8 = client.get("/mqtt/qos").await.unwrap_or(1);
    let reconnect_delay_secs: u64 = client.get("/mqtt/reconnect_delay_secs").await.unwrap_or(1);
    let max_reconnect_delay_secs: u64 = client
        .get("/mqtt/max_reconnect_delay_secs")
        .await
        .unwrap_or(30);
    let buffer_capacity: usize = client.get("/mqtt/buffer_capacity").await.unwrap_or(1000);

    let mqtt_config = MqttConfig {
        broker_url,
        port,
        client_id,
        topic_pattern,
        qos,
        reconnect_delay_secs,
        max_reconnect_delay_secs,
        buffer_capacity,
    };

    debug!(
        "Loaded MQTT config: broker={}:{}, topic={}",
        mqtt_config.broker_url, mqtt_config.port, mqtt_config.topic_pattern
    );

    Ok(mqtt_config)
}

/// Load storage configuration from etcd keys
async fn load_storage_config(client: &ConfigClient) -> StorageConfig {
    debug!("Loading storage configuration from etcd");

    let batch_size: usize = client.get("/storage/batch_size").await.unwrap_or(100);
    let batch_timeout_secs: u64 = client.get("/storage/batch_timeout_secs").await.unwrap_or(5);

    // base_path and wal_enabled come from environment (not stream-specific)
    let base_path = std::env::var("DATA_DIR")
        .or_else(|_| std::env::var("STORAGE_PATH"))
        .unwrap_or_else(|_| "/app/data".to_string());
    let wal_enabled = std::env::var("WAL_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    let storage_config = StorageConfig {
        base_path,
        wal_enabled,
        batch_size,
        batch_timeout_secs,
    };

    debug!(
        "Loaded storage config: path={}, batch_size={}, batch_timeout={}s",
        storage_config.base_path, storage_config.batch_size, storage_config.batch_timeout_secs
    );

    storage_config
}

/// Load server configuration from environment variables
fn load_server_config() -> ServerConfig {
    let host = std::env::var("AIR_QUALITY_SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("AIR_QUALITY_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    ServerConfig { host, port }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_server_config_defaults() {
        // Clear any existing env vars
        std::env::remove_var("AIR_QUALITY_SERVER_HOST");
        std::env::remove_var("AIR_QUALITY_SERVER_PORT");

        let config = load_server_config();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn test_load_server_config_from_env() {
        std::env::set_var("AIR_QUALITY_SERVER_HOST", "127.0.0.1");
        std::env::set_var("AIR_QUALITY_SERVER_PORT", "9090");

        let config = load_server_config();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9090);

        // Cleanup
        std::env::remove_var("AIR_QUALITY_SERVER_HOST");
        std::env::remove_var("AIR_QUALITY_SERVER_PORT");
    }
}
