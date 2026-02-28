use serde::{Deserialize, Serialize};
use sofa_registry_core::constants::defaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataServerConfig {
    pub data_center: String,
    pub cluster_id: String,
    pub local_address: String,
    pub grpc_port: u16,
    pub http_port: u16,
    pub meta_server_addresses: Vec<String>,
    pub slot_sync_interval_secs: u64,
    pub data_change_debounce_ms: u64,
    pub session_lease_secs: u64,
    pub slot_num: u32,
}

impl Default for DataServerConfig {
    fn default() -> Self {
        let local_address = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        Self {
            data_center: defaults::DEFAULT_DATA_CENTER.to_string(),
            cluster_id: defaults::DEFAULT_CLUSTER_ID.to_string(),
            local_address,
            grpc_port: defaults::DATA_GRPC_PORT,
            http_port: defaults::DATA_HTTP_PORT,
            meta_server_addresses: vec![format!("127.0.0.1:{}", defaults::META_GRPC_PORT)],
            slot_sync_interval_secs: defaults::SLOT_SYNC_INTERVAL_SECS,
            data_change_debounce_ms: defaults::DATA_CHANGE_DEBOUNCE_MS,
            session_lease_secs: defaults::SESSION_LEASE_SECS,
            slot_num: defaults::SLOT_NUM,
        }
    }
}

impl DataServerConfig {
    pub fn grpc_address(&self) -> String {
        format!("{}:{}", self.local_address, self.grpc_port)
    }

    pub fn http_address(&self) -> String {
        format!("{}:{}", self.local_address, self.http_port)
    }
}

fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}
