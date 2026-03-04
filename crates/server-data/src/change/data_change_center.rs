use std::collections::HashMap;
use std::sync::Arc;

use sofa_registry_core::model::DatumVersion;
use sofa_registry_remoting::GrpcClientPool;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::lease::SessionLeaseManager;
use sofa_registry_server_shared::metrics as srv_metrics;

/// An event indicating a datum has changed.
#[derive(Debug, Clone)]
pub struct DataChangeEvent {
    pub data_center: String,
    pub data_info_id: String,
    pub version: DatumVersion,
}

/// Sender side: components call `on_change` when data is mutated.
pub struct DataChangeEventCenter {
    tx: mpsc::Sender<DataChangeEvent>,
}

impl DataChangeEventCenter {
    /// Create a new event center with the given buffer size.
    /// Returns the sender (center) and receiver for the merge loop.
    pub fn new(
        buffer_size: usize,
        pool: Arc<GrpcClientPool>,
        session_lease_manager: Arc<SessionLeaseManager>,
        my_address: String,
    ) -> (Self, DataChangeReceiver) {
        let (tx, rx) = mpsc::channel(buffer_size);
        let center = Self { tx };
        let receiver = DataChangeReceiver {
            rx,
            pool,
            session_lease_manager,
            my_address,
        };
        (center, receiver)
    }

    /// Notify the system that a datum has changed.
    pub fn on_change(&self, event: DataChangeEvent) {
        metrics::counter!(srv_metrics::DATA_CHANGES_TOTAL).increment(1);
        if let Err(e) = self.tx.try_send(event) {
            warn!("Failed to send data change event: {}", e);
        }
    }
}

impl Clone for DataChangeEventCenter {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

/// Receiver side: runs the debounce/merge loop.
pub struct DataChangeReceiver {
    rx: mpsc::Receiver<DataChangeEvent>,
    pool: Arc<GrpcClientPool>,
    session_lease_manager: Arc<SessionLeaseManager>,
    my_address: String,
}

impl DataChangeReceiver {
    /// Run the merge loop: collect events for `debounce_ms`, merge by
    /// data_info_id (keeping latest version), then notify session servers.
    pub async fn run_merge_loop(mut self, debounce_ms: u64, cancel: CancellationToken) {
        info!(
            "DataChangeReceiver merge loop started (debounce={}ms)",
            debounce_ms
        );
        let debounce = Duration::from_millis(debounce_ms);

        loop {
            // Wait for the first event or cancellation.
            let first = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    info!("DataChangeReceiver merge loop cancelled");
                    return;
                }
                evt = self.rx.recv() => {
                    match evt {
                        Some(e) => e,
                        None => {
                            info!("DataChangeReceiver channel closed");
                            return;
                        }
                    }
                }
            };

            // Merge events over the debounce window.
            let mut merged: HashMap<String, DataChangeEvent> = HashMap::new();
            merged.insert(first.data_info_id.clone(), first);

            let deadline = time::Instant::now() + debounce;
            loop {
                let remaining = deadline.saturating_duration_since(time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        info!("DataChangeReceiver merge loop cancelled during debounce");
                        return;
                    }
                    _ = time::sleep(remaining) => {
                        break;
                    }
                    evt = self.rx.recv() => {
                        match evt {
                            Some(e) => {
                                merged.insert(e.data_info_id.clone(), e);
                            }
                            None => {
                                break;
                            }
                        }
                    }
                }
            }

            debug!(
                "DataChangeReceiver merged {} data change events",
                merged.len()
            );

            // Notify all active session servers of the changes.
            self.notify_sessions(&merged).await;
        }
    }

    async fn notify_sessions(&self, merged: &HashMap<String, DataChangeEvent>) {
        use sofa_registry_core::pb::sofa::registry::session::{
            session_service_client::SessionServiceClient, DataChangeNotification,
        };

        let sessions = self.session_lease_manager.active_sessions();
        if sessions.is_empty() {
            debug!("No active session servers to notify");
            return;
        }

        debug!("Notifying {} session servers of {} changes", sessions.len(), merged.len());

        for session_addr in &sessions {
            let channel = match self.pool.get_channel(session_addr).await {
                Ok(ch) => ch,
                Err(e) => {
                    warn!(
                        "Failed to connect to session server {} for notification: {}",
                        session_addr, e
                    );
                    self.pool.remove_channel(session_addr);
                    continue;
                }
            };

            let mut client = SessionServiceClient::new(channel);

            for (data_info_id, event) in merged {
                let request = DataChangeNotification {
                    data_center: event.data_center.clone(),
                    data_info_id: data_info_id.clone(),
                    version: event.version.value,
                    slot_id: 0,
                    data_server_address: self.my_address.clone(),
                };

                match client.notify_data_change(request).await {
                    Ok(_) => {
                        metrics::counter!(srv_metrics::DATA_CHANGE_NOTIFICATIONS_TOTAL)
                            .increment(1);
                        debug!(
                            "Notified session {} of change: data_info_id={}",
                            session_addr, data_info_id
                        );
                    }
                    Err(e) => {
                        metrics::counter!(srv_metrics::DATA_CHANGE_NOTIFICATIONS_FAILED)
                            .increment(1);
                        warn!(
                            "Failed to notify session {} of change {}: {}",
                            session_addr, data_info_id, e
                        );
                        self.pool.remove_channel(session_addr);
                        break;
                    }
                }
            }
        }
    }
}
