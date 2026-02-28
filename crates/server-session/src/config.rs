use serde::{Deserialize, Serialize};
use sofa_registry_core::constants::defaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionServerConfig {
    /// Data center name.
    pub data_center: String,
    /// Cluster identifier.
    pub cluster_id: String,
    /// Local address for this session server.
    pub local_address: String,
    /// gRPC port for client-facing service.
    pub grpc_port: u16,
    /// HTTP admin console port.
    pub http_port: u16,
    /// Meta server addresses for registration and slot table.
    pub meta_server_addresses: Vec<String>,
    /// Push task timeout in milliseconds.
    pub push_task_timeout_ms: u64,
    /// Push task channel buffer size.
    pub push_task_buffer_size: usize,
    /// Number of slots in the registry.
    pub slot_num: u32,
    /// Connection idle timeout in seconds. Clients that don't heartbeat
    /// within this window are evicted.
    pub connection_idle_timeout_secs: u64,
}

impl Default for SessionServerConfig {
    fn default() -> Self {
        Self {
            data_center: defaults::DEFAULT_DATA_CENTER.to_string(),
            cluster_id: defaults::DEFAULT_CLUSTER_ID.to_string(),
            local_address: "127.0.0.1".to_string(),
            grpc_port: defaults::SESSION_GRPC_PORT,
            http_port: defaults::SESSION_HTTP_PORT,
            meta_server_addresses: vec![format!(
                "127.0.0.1:{}",
                defaults::META_GRPC_PORT
            )],
            push_task_timeout_ms: defaults::PUSH_TASK_TIMEOUT_MS,
            push_task_buffer_size: 4096,
            slot_num: 256,
            connection_idle_timeout_secs: 90,
        }
    }
}

impl SessionServerConfig {
    pub fn grpc_address(&self) -> String {
        format!("{}:{}", self.local_address, self.grpc_port)
    }

    pub fn http_address(&self) -> String {
        format!("{}:{}", self.local_address, self.http_port)
    }
}
