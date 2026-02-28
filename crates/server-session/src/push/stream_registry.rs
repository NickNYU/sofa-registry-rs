use dashmap::DashMap;
use sofa_registry_core::pb::sofa::registry::ReceivedDataPb;
use tokio::sync::mpsc;
use tonic::Status;

/// Registry of active client subscribe streams.
/// Maps client_id to the sender half of their stream channel.
pub struct StreamRegistry {
    streams: DashMap<String, mpsc::Sender<Result<ReceivedDataPb, Status>>>,
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self {
            streams: DashMap::new(),
        }
    }

    /// Register a new stream for a client. Returns the Sender.
    pub fn register(&self, client_id: &str, tx: mpsc::Sender<Result<ReceivedDataPb, Status>>) {
        self.streams.insert(client_id.to_string(), tx);
    }

    /// Remove a client's stream (called on disconnect).
    pub fn unregister(&self, client_id: &str) {
        self.streams.remove(client_id);
    }

    /// Get the sender for a client's stream.
    pub fn get(&self, client_id: &str) -> Option<mpsc::Sender<Result<ReceivedDataPb, Status>>> {
        self.streams.get(client_id).map(|v| v.clone())
    }

    /// Get all client_ids that have active streams.
    pub fn active_client_ids(&self) -> Vec<String> {
        self.streams.iter().map(|e| e.key().clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.streams.len()
    }
}

impl Default for StreamRegistry {
    fn default() -> Self {
        Self::new()
    }
}
