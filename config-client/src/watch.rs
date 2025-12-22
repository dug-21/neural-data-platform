use crate::ConfigError;
use etcd_client::{Client, EventType, WatchOptions};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// Handle for a configuration watch
pub struct WatchHandle {
    cancel_tx: mpsc::Sender<()>,
}

impl WatchHandle {
    pub(crate) async fn new<F>(
        client: Client,
        prefix: &str,
        callback: F,
    ) -> Result<Self, ConfigError>
    where
        F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static,
    {
        let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);
        let prefix = prefix.to_string();

        tokio::spawn(async move {
            let opts = WatchOptions::new().with_prefix();

            match client.clone().watch(prefix.clone(), Some(opts)).await {
                Ok((mut watcher, mut stream)) => {
                    info!("Started watching: {}", prefix);

                    loop {
                        tokio::select! {
                            _ = cancel_rx.recv() => {
                                info!("Watch cancelled: {}", prefix);
                                let _ = watcher.cancel().await;
                                break;
                            }
                            msg = stream.message() => {
                                match msg {
                                    Ok(Some(resp)) => {
                                        for event in resp.events() {
                                            if let Some(kv) = event.kv() {
                                                let key = String::from_utf8_lossy(kv.key()).to_string();
                                                let value = match event.event_type() {
                                                    EventType::Put => {
                                                        serde_json::from_slice(kv.value()).ok()
                                                    }
                                                    EventType::Delete => None,
                                                };
                                                debug!("Config changed: {} -> {:?}", key, value);
                                                callback(key, value);
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        debug!("Watch stream ended: {}", prefix);
                                        break;
                                    }
                                    Err(e) => {
                                        error!("Watch error: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to start watch: {}", e);
                }
            }
        });

        Ok(Self { cancel_tx })
    }

    /// Cancel the watch
    pub async fn cancel(self) {
        let _ = self.cancel_tx.send(()).await;
    }
}
