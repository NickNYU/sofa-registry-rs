use serde::{Deserialize, Serialize};
use sofa_registry_core::constants::defaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonConfig {
    pub data_center: String,
    pub cluster_id: String,
    pub local_address: String,
    pub session_lease_secs: u64,
    pub data_lease_secs: u64,
    pub slot_num: u32,
    pub slot_replicas: u32,
}

impl Default for CommonConfig {
    fn default() -> Self {
        // Detect local IP
        let local_address = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        Self {
            data_center: defaults::DEFAULT_DATA_CENTER.to_string(),
            cluster_id: defaults::DEFAULT_CLUSTER_ID.to_string(),
            local_address,
            session_lease_secs: defaults::SESSION_LEASE_SECS,
            data_lease_secs: defaults::DATA_LEASE_SECS,
            slot_num: defaults::SLOT_NUM,
            slot_replicas: defaults::SLOT_REPLICAS,
        }
    }
}

/// Get the local machine IP address (first non-loopback IPv4).
///
/// Connects a UDP socket to a remote address to determine which local interface
/// would be used, then returns that IP.
fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}
