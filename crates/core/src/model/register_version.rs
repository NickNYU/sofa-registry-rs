use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RegisterVersion {
    pub version: i64,
    pub timestamp: i64,
}

impl RegisterVersion {
    pub fn of(version: i64) -> Self {
        Self {
            version,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn new(version: i64, timestamp: i64) -> Self {
        Self { version, timestamp }
    }
}
