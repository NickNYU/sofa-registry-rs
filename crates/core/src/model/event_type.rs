use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "REGISTER")]
    #[default]
    Register,
    #[serde(rename = "UNREGISTER")]
    Unregister,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Register => write!(f, "REGISTER"),
            Self::Unregister => write!(f, "UNREGISTER"),
        }
    }
}
