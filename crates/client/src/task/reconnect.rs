use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{info, warn};

use crate::remoting::ClientConnection;

/// Handles reconnection to the session server with exponential backoff.
pub struct ReconnectTask {
    connection: Arc<ClientConnection>,
    initial_delay_ms: u64,
    max_delay_ms: u64,
    shutdown: Arc<Notify>,
}

impl ReconnectTask {
    pub fn new(
        connection: Arc<ClientConnection>,
        initial_delay_ms: u64,
        max_delay_ms: u64,
    ) -> Self {
        Self {
            connection,
            initial_delay_ms,
            max_delay_ms,
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Returns a handle that can be used to signal shutdown.
    pub fn shutdown_handle(&self) -> Arc<Notify> {
        self.shutdown.clone()
    }

    /// Run the reconnection loop. This blocks until shutdown is signalled
    /// or a connection is successfully established.
    pub async fn run(&self) {
        let mut delay_ms = self.initial_delay_ms;

        loop {
            if self.connection.is_connected() {
                // Already connected; wait a bit then check again
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(self.max_delay_ms)) => {},
                    _ = self.shutdown.notified() => {
                        info!("Reconnect task shutting down");
                        return;
                    }
                }
                // Reset delay when connected
                delay_ms = self.initial_delay_ms;
                continue;
            }

            info!(
                "Attempting to reconnect to {} (delay={}ms)",
                self.connection.address(),
                delay_ms
            );

            match self.connection.connect().await {
                Ok(()) => {
                    info!(
                        "Reconnected to session server at {}",
                        self.connection.address()
                    );
                    delay_ms = self.initial_delay_ms;
                }
                Err(e) => {
                    warn!(
                        "Reconnect to {} failed: {}. Retrying in {}ms",
                        self.connection.address(),
                        e,
                        delay_ms
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(delay_ms)) => {},
                        _ = self.shutdown.notified() => {
                            info!("Reconnect task shutting down");
                            return;
                        }
                    }
                    // Exponential backoff
                    delay_ms = (delay_ms * 2).min(self.max_delay_ms);
                }
            }
        }
    }
}
