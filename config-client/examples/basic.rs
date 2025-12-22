use config_client::ConfigClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    broker_url: String,
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to etcd
    let client = ConfigClient::with_prefix(&["http://localhost:2379"], "/air-quality").await?;

    // Set a config
    let config = AppConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
    };
    client.set("/mqtt", &config).await?;
    println!("Set config: {:?}", config);

    // Get it back
    let loaded: AppConfig = client.get("/mqtt").await?;
    println!("Got config: {:?}", loaded);

    // Watch for changes
    let handle = client
        .watch("/", |key, value| {
            println!("Config changed: {} = {:?}", key, value);
        })
        .await?;

    println!("Watching for changes... Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;
    handle.cancel().await;

    Ok(())
}
