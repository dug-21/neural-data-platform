//! GitOps Config Sync - Loads YAML configs into etcd
//!
//! Usage: cargo run --bin sync-config -- [environment]

use config_client::ConfigClient;
use std::path::Path;
use walkdir::WalkDir;

async fn sync_configs(config_dir: &Path, environment: &str, client: &ConfigClient) -> Result<(), Box<dyn std::error::Error>> {
    // Load base configs
    let base_dir = config_dir.join("base");
    if base_dir.exists() {
        load_service_configs(&base_dir, client).await?;
    }

    // Load environment overlays
    let overlay_dir = config_dir.join("overlays").join(environment);
    if overlay_dir.exists() {
        load_service_configs(&overlay_dir, client).await?;
    }

    Ok(())
}

async fn load_service_configs(dir: &Path, client: &ConfigClient) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let config_file = path.join("config.yaml");
            if config_file.exists() {
                let service_name = path.file_name().unwrap().to_str().unwrap();
                load_yaml_to_etcd(&config_file, service_name, client).await?;
            }
        }
    }
    Ok(())
}

async fn load_yaml_to_etcd(file: &Path, service: &str, client: &ConfigClient) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(file)?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

    flatten_and_store(&yaml, &format!("/{}", service), client).await?;
    println!("Loaded {} from {:?}", service, file);

    Ok(())
}

#[async_recursion::async_recursion]
async fn flatten_and_store(value: &serde_yaml::Value, prefix: &str, client: &ConfigClient) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                if let serde_yaml::Value::String(key) = k {
                    let new_prefix = format!("{}/{}", prefix, key);
                    if v.is_mapping() {
                        flatten_and_store(v, &new_prefix, client).await?;
                    } else {
                        let json_value = yaml_to_json(v);
                        client.set_raw(&new_prefix, &json_value).await?;
                    }
                }
            }
        }
        _ => {
            let json_value = yaml_to_json(value);
            client.set_raw(prefix, &json_value).await?;
        }
    }
    Ok(())
}

fn yaml_to_json(yaml: &serde_yaml::Value) -> serde_json::Value {
    match yaml {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::json!(i)
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s.clone()),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .filter_map(|(k, v)| {
                    k.as_str().map(|key| (key.to_string(), yaml_to_json(v)))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let environment = std::env::args().nth(1).unwrap_or_else(|| "development".to_string());
    let etcd_endpoint = std::env::var("ETCD_ENDPOINT").unwrap_or_else(|_| "http://localhost:2379".to_string());

    println!("Syncing config to {} for environment: {}", etcd_endpoint, environment);

    let client = ConfigClient::new(&[&etcd_endpoint]).await?;
    let config_dir = Path::new("./config");

    sync_configs(config_dir, &environment, &client).await?;

    println!("Config sync complete!");
    Ok(())
}
