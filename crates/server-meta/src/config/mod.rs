use serde::{Deserialize, Serialize};
use sofa_registry_core::constants::defaults;

/// Meta server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaServerConfig {
    /// Data center name
    pub data_center: String,
    /// Cluster identifier
    pub cluster_id: String,
    /// Local address (auto-detected if empty)
    pub local_address: String,
    /// gRPC port for session/data server communication
    pub grpc_port: u16,
    /// HTTP admin console port
    pub http_port: u16,
    /// Meta peer addresses for cluster formation
    pub meta_peers: Vec<String>,
    /// Database URL (sqlite:// or postgres://)
    pub db_url: String,
    /// Session server lease duration in seconds
    pub session_lease_secs: u64,
    /// Data server lease duration in seconds
    pub data_lease_secs: u64,
    /// Number of data partitioning slots
    pub slot_num: u32,
    /// Number of replicas per slot
    pub slot_replicas: u32,
    /// Leader election lock duration in milliseconds
    pub election_lock_duration_ms: i64,
    /// Election loop interval in milliseconds
    pub election_interval_ms: u64,
    /// Lease eviction check interval in seconds
    pub eviction_interval_secs: u64,
}

impl Default for MetaServerConfig {
    fn default() -> Self {
        Self {
            data_center: defaults::DEFAULT_DATA_CENTER.to_string(),
            cluster_id: defaults::DEFAULT_CLUSTER_ID.to_string(),
            local_address: "127.0.0.1".to_string(),
            grpc_port: defaults::META_GRPC_PORT,
            http_port: defaults::META_HTTP_PORT,
            meta_peers: vec!["127.0.0.1:9611".to_string()],
            db_url: "sqlite://sofa-registry-meta.db?mode=rwc".to_string(),
            session_lease_secs: defaults::SESSION_LEASE_SECS,
            data_lease_secs: defaults::DATA_LEASE_SECS,
            slot_num: defaults::SLOT_NUM,
            slot_replicas: defaults::SLOT_REPLICAS,
            election_lock_duration_ms: defaults::ELECTION_LOCK_DURATION_MS,
            election_interval_ms: 1000,
            eviction_interval_secs: 5,
        }
    }
}

impl MetaServerConfig {
    pub fn grpc_address(&self) -> String {
        format!("{}:{}", self.local_address, self.grpc_port)
    }

    pub fn http_address(&self) -> String {
        format!("{}:{}", self.local_address, self.http_port)
    }
}
