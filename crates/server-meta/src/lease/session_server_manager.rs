use super::lease_manager::LeaseManager;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Represents a registered session server node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNode {
    pub address: String,
    pub data_center: String,
    pub cluster_id: String,
    pub registered_timestamp: i64,
}

impl SessionNode {
    pub fn new(address: &str, data_center: &str, cluster_id: &str) -> Self {
        Self {
            address: address.to_string(),
            data_center: data_center.to_string(),
            cluster_id: cluster_id.to_string(),
            registered_timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Manages session server registrations and leases
pub struct SessionServerManager {
    lease_manager: LeaseManager<SessionNode>,
}

impl SessionServerManager {
    pub fn new(lease_duration_secs: u64) -> Self {
        Self {
            lease_manager: LeaseManager::new(lease_duration_secs),
        }
    }

    pub fn register(&self, node: SessionNode) -> bool {
        let key = node.address.clone();
        info!("Registering session server: {}", key);
        self.lease_manager.register(key, node)
    }

    pub fn renew(&self, address: &str) -> bool {
        self.lease_manager.renew(address)
    }

    pub fn get_session_server_list(&self) -> Vec<SessionNode> {
        self.lease_manager.get_all()
    }

    pub fn get_session_server_addresses(&self) -> Vec<String> {
        self.lease_manager
            .get_all()
            .iter()
            .map(|n| n.address.clone())
            .collect()
    }

    pub fn count(&self) -> usize {
        self.lease_manager.count()
    }

    pub fn evict_expired(&self) -> Vec<SessionNode> {
        self.lease_manager.evict_expired()
    }

    pub fn contains(&self, address: &str) -> bool {
        self.lease_manager.contains(address)
    }
}
