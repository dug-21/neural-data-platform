use serde::{Deserialize, Serialize};
use std::path::Path;

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
    pub client_id: String,
    pub topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub backend: String, // "memory", "postgres", "influxdb"
    pub connection_string: Option<String>,
}

impl AppConfig {
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn default_config() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
            },
            mqtt: MqttConfig {
                broker_url: "mqtt://localhost:1883".to_string(),
                client_id: "air-quality-app".to_string(),
                topic: "airgradient/+/measures".to_string(),
            },
            storage: StorageConfig {
                backend: "memory".to_string(),
                connection_string: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default_config();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.mqtt.client_id, "air-quality-app");
        assert_eq!(config.storage.backend, "memory");
    }

    #[test]
    fn test_from_yaml() {
        let yaml_content = r#"
server:
  host: "127.0.0.1"
  port: 8080
mqtt:
  broker_url: "mqtt://broker:1883"
  client_id: "test-client"
  topic: "test/topic"
storage:
  backend: "postgres"
  connection_string: "postgresql://localhost/airquality"
"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_config.yaml");
        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(yaml_content.as_bytes()).unwrap();

        let config = AppConfig::from_yaml(&temp_file).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.mqtt.broker_url, "mqtt://broker:1883");
        assert_eq!(config.storage.backend, "postgres");

        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_from_yaml_invalid_file() {
        let result = AppConfig::from_yaml("/nonexistent/file.yaml");
        assert!(result.is_err());
    }
}
