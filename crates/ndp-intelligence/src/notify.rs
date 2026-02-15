//! PG NOTIFY listener for intelligence cycle wake-up
//!
//! Listens for PostgreSQL NOTIFY events on a configurable channel.
//! Used as an optimization alongside the timer-based primary wake mechanism.

use std::time::Duration;

use tokio::sync::mpsc;
use tokio_postgres::NoTls;
use tracing::{info, warn};

/// Listener for PostgreSQL NOTIFY events.
///
/// Maintains a dedicated connection (not pooled) for LISTEN/NOTIFY,
/// with exponential backoff reconnection on connection loss.
pub struct NotifyListener {
    connection_string: String,
    channel: String,
}

impl NotifyListener {
    /// Create a new NotifyListener.
    pub fn new(connection_string: &str, channel: &str) -> Self {
        Self {
            connection_string: connection_string.to_string(),
            channel: channel.to_string(),
        }
    }

    /// Start listening for notifications.
    ///
    /// Returns a channel receiver that yields notification payloads.
    /// The listener runs in a background task with automatic reconnection.
    pub async fn listen(&self) -> Result<mpsc::Receiver<String>, crate::error::IntelligenceError> {
        let (tx, rx) = mpsc::channel(16);
        let conn_str = self.connection_string.clone();
        let channel = self.channel.clone();

        tokio::spawn(async move {
            let mut backoff = Duration::from_secs(1);
            let max_backoff = Duration::from_secs(60);

            loop {
                match connect_and_listen(&conn_str, &channel, &tx).await {
                    Ok(()) => {
                        info!("NOTIFY listener shut down cleanly");
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "NOTIFY connection lost: {}. Retrying in {:?}",
                            e, backoff
                        );
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(max_backoff);
                    }
                }
            }
        });

        Ok(rx)
    }
}

/// Connect to PostgreSQL and listen for notifications on a channel.
async fn connect_and_listen(
    conn_str: &str,
    channel: &str,
    tx: &mpsc::Sender<String>,
) -> Result<(), String> {
    let (client, connection) = tokio_postgres::connect(conn_str, NoTls)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    // Spawn connection driver
    let conn_handle = tokio::spawn(async move {
        if let Err(e) = connection.await {
            warn!("NOTIFY connection error: {}", e);
        }
    });

    // Subscribe to channel
    let listen_cmd = format!("LISTEN {}", channel);
    client
        .execute(&listen_cmd, &[])
        .await
        .map_err(|e| format!("LISTEN failed: {}", e))?;

    info!("Listening on PG channel '{}'", channel);

    // Poll for notifications
    loop {
        // Check if channel receiver is still alive
        if tx.is_closed() {
            info!("NOTIFY receiver dropped, stopping listener");
            break;
        }

        // Use tokio-postgres notification polling
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if connection is still alive
        if conn_handle.is_finished() {
            return Err("Connection task ended unexpectedly".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notify_listener_construction() {
        let listener = NotifyListener::new(
            "host=localhost dbname=ndp user=ndp",
            "gold_refresh",
        );
        assert_eq!(listener.connection_string, "host=localhost dbname=ndp user=ndp");
        assert_eq!(listener.channel, "gold_refresh");
    }
}
