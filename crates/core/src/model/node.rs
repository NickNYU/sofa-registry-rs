use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    Client,
    Session,
    Data,
    Meta,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client => write!(f, "Client"),
            Self::Session => write!(f, "Session"),
            Self::Data => write!(f, "Data"),
            Self::Meta => write!(f, "Meta"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Node {
    pub node_type: NodeType,
    pub ip: String,
    pub port: u16,
}

impl Node {
    pub fn new(node_type: NodeType, ip: impl Into<String>, port: u16) -> Self {
        Self {
            node_type,
            ip: ip.into(),
            port,
        }
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({}:{})", self.node_type, self.ip, self.port)
    }
}
