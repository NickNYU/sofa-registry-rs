use serde::{Deserialize, Serialize};

/// Configuration for the registry client SDK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryClientConfig {
    /// Unique instance identifier for this client.
    pub instance_id: String,

    /// Availability zone.
    pub zone: String,

    /// Application name.
    pub app_name: String,

    /// Data center name.
    pub data_center: String,

    /// Session server addresses to connect to (host:port).
    pub session_server_addresses: Vec<String>,

    /// TCP connect timeout in milliseconds.
    pub connect_timeout_ms: u64,

    /// RPC request timeout in milliseconds.
    pub request_timeout_ms: u64,

    /// Initial reconnect delay in milliseconds.
    pub reconnect_delay_ms: u64,

    /// Maximum reconnect delay in milliseconds (for exponential backoff cap).
    pub max_reconnect_delay_ms: u64,

    /// Whether HMAC authentication is enabled.
    pub auth_enabled: bool,

    /// HMAC access key (required when auth_enabled is true).
    pub access_key: Option<String>,

    /// HMAC secret key (required when auth_enabled is true).
    pub secret_key: Option<String>,
}

impl Default for RegistryClientConfig {
    fn default() -> Self {
        Self {
            instance_id: "DEFAULT_INSTANCE_ID".to_string(),
            zone: "DEFAULT_ZONE".to_string(),
            app_name: "default-app".to_string(),
            data_center: "DefaultDataCenter".to_string(),
            session_server_addresses: vec!["127.0.0.1:9601".to_string()],
            connect_timeout_ms: 5000,
            request_timeout_ms: 10000,
            reconnect_delay_ms: 1000,
            max_reconnect_delay_ms: 30000,
            auth_enabled: false,
            access_key: None,
            secret_key: None,
        }
    }
}
