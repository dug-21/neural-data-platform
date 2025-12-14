use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub mqtt: MqttConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,
    pub qos: u8,
    pub reconnect_delay_secs: u64,
    pub max_reconnect_delay_secs: u64,
    pub buffer_capacity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub base_path: String,
    pub wal_enabled: bool,
    pub batch_size: usize,
    pub batch_timeout_secs: u64,
}

impl AppConfig {
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let mut config: AppConfig = serde_yaml::from_str(&content)?;

        // Apply environment variable overrides
        config.apply_env_overrides();

        Ok(config)
    }

    pub fn default_config() -> Self {
        let mut config = Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
            mqtt: MqttConfig {
                broker_url: "localhost".to_string(),
                port: 1883,
                client_id: "air-quality-app".to_string(),
                topic_pattern: "airgradient/readings/+".to_string(),
                qos: 1,
                reconnect_delay_secs: 1,
                max_reconnect_delay_secs: 30,
                buffer_capacity: 1000,
            },
            storage: StorageConfig {
                base_path: "./data/parquet".to_string(),
                wal_enabled: true,
                batch_size: 100,
                batch_timeout_secs: 5,
            },
        };

        // Apply environment variable overrides
        config.apply_env_overrides();

        config
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(broker_url) = std::env::var("MQTT_BROKER_URL") {
            self.mqtt.broker_url = broker_url;
        }

        if let Ok(port) = std::env::var("MQTT_PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                self.mqtt.port = port_num;
            }
        }

        if let Ok(storage_path) = std::env::var("STORAGE_PATH") {
            self.storage.base_path = storage_path;
        }
    }
}

impl MqttConfig {
    /// Get QoS level as rumqttc::QoS enum
    pub fn get_qos(&self) -> rumqttc::QoS {
        use rumqttc::QoS;

        match self.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::AtLeastOnce, // Default to QoS 1
        }
    }

    /// Get reconnect delay as Duration
    pub fn get_reconnect_delay(&self) -> Duration {
        Duration::from_secs(self.reconnect_delay_secs)
    }

    /// Get max reconnect delay as Duration
    pub fn get_max_reconnect_delay(&self) -> Duration {
        Duration::from_secs(self.max_reconnect_delay_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        // Clear environment variables to get true defaults
        let saved_broker = std::env::var("MQTT_BROKER_URL").ok();
        let saved_port = std::env::var("MQTT_PORT").ok();
        let saved_path = std::env::var("STORAGE_PATH").ok();

        std::env::remove_var("MQTT_BROKER_URL");
        std::env::remove_var("MQTT_PORT");
        std::env::remove_var("STORAGE_PATH");

        let config = AppConfig::default_config();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.mqtt.client_id, "air-quality-app");
        assert_eq!(config.mqtt.broker_url, "localhost");
        assert_eq!(config.mqtt.port, 1883);
        assert_eq!(config.mqtt.qos, 1);
        assert_eq!(config.mqtt.buffer_capacity, 1000);
        assert_eq!(config.storage.base_path, "./data/parquet");
        assert_eq!(config.storage.wal_enabled, true);
        assert_eq!(config.storage.batch_size, 100);
        assert_eq!(config.storage.batch_timeout_secs, 5);

        // Restore original state
        if let Some(val) = saved_broker {
            std::env::set_var("MQTT_BROKER_URL", val);
        }
        if let Some(val) = saved_port {
            std::env::set_var("MQTT_PORT", val);
        }
        if let Some(val) = saved_path {
            std::env::set_var("STORAGE_PATH", val);
        }
    }

    #[test]
    fn test_from_yaml() {
        let yaml_content = r#"
server:
  host: "127.0.0.1"
  port: 8080
mqtt:
  broker_url: "broker.example.com"
  port: 1883
  client_id: "test-client"
  topic_pattern: "test/topic/+"
  qos: 2
  reconnect_delay_secs: 5
  max_reconnect_delay_secs: 60
  buffer_capacity: 2000
storage:
  base_path: "/data/parquet"
  wal_enabled: false
  batch_size: 50
  batch_timeout_secs: 10
"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_config.yaml");
        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(yaml_content.as_bytes()).unwrap();

        let config = AppConfig::from_yaml(&temp_file).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.mqtt.broker_url, "broker.example.com");
        assert_eq!(config.mqtt.port, 1883);
        assert_eq!(config.mqtt.topic_pattern, "test/topic/+");
        assert_eq!(config.mqtt.qos, 2);
        assert_eq!(config.mqtt.reconnect_delay_secs, 5);
        assert_eq!(config.mqtt.max_reconnect_delay_secs, 60);
        assert_eq!(config.mqtt.buffer_capacity, 2000);
        assert_eq!(config.storage.base_path, "/data/parquet");
        assert_eq!(config.storage.wal_enabled, false);
        assert_eq!(config.storage.batch_size, 50);
        assert_eq!(config.storage.batch_timeout_secs, 10);

        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_from_yaml_invalid_file() {
        let result = AppConfig::from_yaml("/nonexistent/file.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_env_overrides() {
        // Save current env vars if they exist
        let saved_broker = std::env::var("MQTT_BROKER_URL").ok();
        let saved_port = std::env::var("MQTT_PORT").ok();
        let saved_path = std::env::var("STORAGE_PATH").ok();

        // Set test values
        std::env::set_var("MQTT_BROKER_URL", "env-broker.example.com");
        std::env::set_var("MQTT_PORT", "1884");
        std::env::set_var("STORAGE_PATH", "/env/data/parquet");

        let config = AppConfig::default_config();

        assert_eq!(config.mqtt.broker_url, "env-broker.example.com");
        assert_eq!(config.mqtt.port, 1884);
        assert_eq!(config.storage.base_path, "/env/data/parquet");

        // Restore original state
        if let Some(val) = saved_broker {
            std::env::set_var("MQTT_BROKER_URL", val);
        } else {
            std::env::remove_var("MQTT_BROKER_URL");
        }
        if let Some(val) = saved_port {
            std::env::set_var("MQTT_PORT", val);
        } else {
            std::env::remove_var("MQTT_PORT");
        }
        if let Some(val) = saved_path {
            std::env::set_var("STORAGE_PATH", val);
        } else {
            std::env::remove_var("STORAGE_PATH");
        }
    }

    #[test]
    fn test_mqtt_config_helpers() {
        let config = MqttConfig {
            broker_url: "test.broker.com".to_string(),
            port: 1883,
            client_id: "test-client".to_string(),
            topic_pattern: "test/+".to_string(),
            qos: 1,
            reconnect_delay_secs: 2,
            max_reconnect_delay_secs: 60,
            buffer_capacity: 500,
        };

        assert_eq!(config.get_reconnect_delay(), Duration::from_secs(2));
        assert_eq!(config.get_max_reconnect_delay(), Duration::from_secs(60));
    }

    #[test]
    fn test_qos_conversion() {
        use rumqttc::QoS;

        let config_qos0 = MqttConfig {
            broker_url: "test".to_string(),
            port: 1883,
            client_id: "test".to_string(),
            topic_pattern: "test/+".to_string(),
            qos: 0,
            reconnect_delay_secs: 1,
            max_reconnect_delay_secs: 30,
            buffer_capacity: 100,
        };
        assert!(matches!(config_qos0.get_qos(), QoS::AtMostOnce));

        let config_qos1 = MqttConfig { qos: 1, ..config_qos0.clone() };
        assert!(matches!(config_qos1.get_qos(), QoS::AtLeastOnce));

        let config_qos2 = MqttConfig { qos: 2, ..config_qos0.clone() };
        assert!(matches!(config_qos2.get_qos(), QoS::ExactlyOnce));

        // Invalid QoS defaults to AtLeastOnce
        let config_invalid = MqttConfig { qos: 99, ..config_qos0 };
        assert!(matches!(config_invalid.get_qos(), QoS::AtLeastOnce));
    }
}
