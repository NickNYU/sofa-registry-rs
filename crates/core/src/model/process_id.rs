use serde::{Deserialize, Serialize};

/// Identifies a server process (hostAddress + timestamp + sequenceId)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessId {
    pub host_address: String,
    pub timestamp: i64,
    pub sequence_id: i64,
}

impl ProcessId {
    pub fn new(host_address: impl Into<String>, timestamp: i64, sequence_id: i64) -> Self {
        Self {
            host_address: host_address.into(),
            timestamp,
            sequence_id,
        }
    }
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}-{}-{}",
            self.host_address, self.timestamp, self.sequence_id
        )
    }
}
