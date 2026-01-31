//! etcd-based configuration loading
//!
//! Loads configuration from etcd with environment variable overrides.

use config_client::ConfigClient;
use serde::Deserialize;
use tracing::{info, warn};

/// Load configuration from etcd
/// Falls back to file-based config if etcd is unavailable
pub async fn load_from_etcd() -> Result<EtcdAppConfig, Box<dyn std::error::Error>> {
    let etcd_endpoint =
        std::env::var("ETCD_ENDPOINT").unwrap_or_else(|_| "http://localhost:2379".to_string());

    info!("Connecting to etcd at {}", etcd_endpoint);

    match ConfigClient::with_prefix(&[&etcd_endpoint], "/air-quality").await {
        Ok(client) => {
            info!("Connected to etcd, loading configuration");
            load_config_from_client(&client).await
        }
        Err(e) => {
            warn!(
                "Failed to connect to etcd: {}. Falling back to file config.",
                e
            );
            Err(Box::new(e))
        }
    }
}

async fn load_config_from_client(
    client: &ConfigClient,
) -> Result<EtcdAppConfig, Box<dyn std::error::Error>> {
    // Load each section, with env var overrides
    let server = ServerConfig {
        host: client
            .get_with_env("/server/host", "AIR_QUALITY")
            .await
            .unwrap_or_else(|_| "0.0.0.0".to_string()),
        port: client
            .get_with_env("/server/port", "AIR_QUALITY")
            .await
            .unwrap_or(8080),
    };

    let mqtt = MqttConfig {
        broker_url: client
            .get_with_env("/mqtt/broker_url", "AIR_QUALITY")
            .await
            .unwrap_or_else(|_| "localhost".to_string()),
        port: client
            .get_with_env("/mqtt/port", "AIR_QUALITY")
            .await
            .unwrap_or(1883),
        client_id: client
            .get_with_env("/mqtt/client_id", "AIR_QUALITY")
            .await
            .unwrap_or_else(|_| "air-quality-app".to_string()),
        topic_pattern: client
            .get_with_env::<String>("/mqtt/topic_pattern", "AIR_QUALITY")
            .await
            .ok(),
        subscriptions: Vec::new(), // Will be populated from YAML or subscriptions keys
        qos: client
            .get_with_env("/mqtt/qos", "AIR_QUALITY")
            .await
            .unwrap_or(1),
        reconnect_delay_secs: client
            .get_with_env("/mqtt/reconnect_delay_secs", "AIR_QUALITY")
            .await
            .unwrap_or(1),
        max_reconnect_delay_secs: client
            .get_with_env("/mqtt/max_reconnect_delay_secs", "AIR_QUALITY")
            .await
            .unwrap_or(30),
        buffer_capacity: client
            .get_with_env("/mqtt/buffer_capacity", "AIR_QUALITY")
            .await
            .unwrap_or(1000),
        default_stream_id: "air-quality".to_string(),
    };

    let storage = StorageConfig {
        base_path: {
            // Priority: etcd > DATA_DIR env var > STORAGE_PATH env var > default
            match client.get::<String>("/storage/base_path").await {
                Ok(path) => {
                    info!("Using storage base_path from etcd: {}", path);
                    path
                }
                Err(_) => {
                    if let Ok(data_dir) = std::env::var("DATA_DIR") {
                        info!(
                            "Using storage base_path from DATA_DIR env var: {}",
                            data_dir
                        );
                        data_dir
                    } else if let Ok(storage_path) = std::env::var("STORAGE_PATH") {
                        info!(
                            "Using storage base_path from STORAGE_PATH env var: {}",
                            storage_path
                        );
                        storage_path
                    } else {
                        warn!("No storage base_path in etcd or env vars, using default: ./data/parquet");
                        "./data/parquet".to_string()
                    }
                }
            }
        },
        wal_enabled: client
            .get_with_env("/storage/wal_enabled", "AIR_QUALITY")
            .await
            .unwrap_or(true),
        batch_size: client
            .get_with_env("/storage/batch_size", "AIR_QUALITY")
            .await
            .unwrap_or(100),
        batch_timeout_secs: client
            .get_with_env("/storage/batch_timeout_secs", "AIR_QUALITY")
            .await
            .unwrap_or(5),
    };

    Ok(EtcdAppConfig {
        server,
        mqtt,
        storage,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct EtcdAppConfig {
    pub server: ServerConfig,
    pub mqtt: MqttConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

/// Subscription configuration for MQTT multi-subscription support
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionConfig {
    pub stream_id: String,
    pub topic_pattern: String,
    #[serde(default = "default_true_etcd")]
    pub enabled: bool,
    /// AIR-012: Topic segment index to extract as ndp_id (0-indexed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ndp_id_topic_segment: Option<usize>,
}

fn default_true_etcd() -> bool {
    true
}

fn default_stream_id_etcd() -> String {
    "air-quality".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    /// Legacy single topic pattern - deprecated, use subscriptions instead
    #[serde(default)]
    pub topic_pattern: Option<String>,
    /// New multi-subscription support
    #[serde(default)]
    pub subscriptions: Vec<SubscriptionConfig>,
    pub qos: u8,
    pub reconnect_delay_secs: u64,
    pub max_reconnect_delay_secs: u64,
    pub buffer_capacity: usize,
    /// Default stream ID for legacy topic_pattern
    #[serde(default = "default_stream_id_etcd")]
    pub default_stream_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub base_path: String,
    pub wal_enabled: bool,
    pub batch_size: usize,
    pub batch_timeout_secs: u64,
}

impl MqttConfig {
    pub fn get_qos(&self) -> rumqttc::QoS {
        match self.qos {
            0 => rumqttc::QoS::AtMostOnce,
            1 => rumqttc::QoS::AtLeastOnce,
            _ => rumqttc::QoS::ExactlyOnce,
        }
    }

    pub fn get_reconnect_delay(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.reconnect_delay_secs)
    }

    pub fn get_max_reconnect_delay(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.max_reconnect_delay_secs)
    }
}
