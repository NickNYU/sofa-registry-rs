use serde::{Deserialize, Serialize};

/// Uniquely identifies a client connection (clientHost:clientPort -> serverHost:serverPort)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectId {
    pub client_host_address: String,
    pub client_port: u16,
    pub server_host_address: String,
    pub server_port: u16,
}

impl ConnectId {
    pub fn new(
        client_host: impl Into<String>,
        client_port: u16,
        server_host: impl Into<String>,
        server_port: u16,
    ) -> Self {
        Self {
            client_host_address: client_host.into(),
            client_port,
            server_host_address: server_host.into(),
            server_port,
        }
    }
}

impl std::fmt::Display for ConnectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.client_host_address, self.client_port, self.server_host_address, self.server_port
        )
    }
}
