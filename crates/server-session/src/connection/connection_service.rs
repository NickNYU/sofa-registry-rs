use dashmap::DashMap;

use sofa_registry_server_shared::metrics as srv_metrics;

/// Information about a single client connection.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub client_id: String,
    pub address: String,
    pub connected_at: i64,
    /// Timestamp of the last heartbeat or activity from this client.
    pub last_heartbeat: i64,
}

/// Tracks active client connections with heartbeat-based health checking.
pub struct ConnectionService {
    /// client_id -> ConnectionInfo
    connections: DashMap<String, ConnectionInfo>,
}

impl ConnectionService {
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
        }
    }

    /// Register a new client connection.
    pub fn connect(&self, client_id: String, address: String) {
        let now = chrono::Utc::now().timestamp_millis();
        let info = ConnectionInfo {
            client_id: client_id.clone(),
            address,
            connected_at: now,
            last_heartbeat: now,
        };
        self.connections.insert(client_id, info);
        metrics::gauge!(srv_metrics::SESSION_ACTIVE_CONNECTIONS).set(self.connections.len() as f64);
    }

    /// Remove a client connection.
    pub fn disconnect(&self, client_id: &str) -> Option<ConnectionInfo> {
        let result = self.connections.remove(client_id).map(|(_, v)| v);
        metrics::gauge!(srv_metrics::SESSION_ACTIVE_CONNECTIONS).set(self.connections.len() as f64);
        result
    }

    /// Check if a client is connected.
    pub fn is_connected(&self, client_id: &str) -> bool {
        self.connections.contains_key(client_id)
    }

    /// Update the heartbeat timestamp for a client.
    pub fn touch_heartbeat(&self, client_id: &str) {
        if let Some(mut entry) = self.connections.get_mut(client_id) {
            entry.last_heartbeat = chrono::Utc::now().timestamp_millis();
        }
    }

    /// Evict connections that have been idle for longer than `timeout_secs`.
    /// Returns the list of evicted client IDs.
    pub fn evict_idle(&self, timeout_secs: u64) -> Vec<String> {
        let cutoff = chrono::Utc::now().timestamp_millis() - (timeout_secs as i64 * 1000);
        let mut evicted = Vec::new();

        self.connections.retain(|client_id, info| {
            if info.last_heartbeat < cutoff {
                evicted.push(client_id.clone());
                false
            } else {
                true
            }
        });

        evicted
    }

    /// Get connection info for a client.
    pub fn get(&self, client_id: &str) -> Option<ConnectionInfo> {
        self.connections.get(client_id).map(|v| v.clone())
    }

    /// Total number of active connections.
    pub fn count(&self) -> usize {
        self.connections.len()
    }

    /// Get all connection infos.
    pub fn get_all(&self) -> Vec<ConnectionInfo> {
        self.connections.iter().map(|e| e.value().clone()).collect()
    }
}

impl Default for ConnectionService {
    fn default() -> Self {
        Self::new()
    }
}
