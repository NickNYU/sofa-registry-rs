use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    #[serde(rename = "zone")]
    Zone,
    #[serde(rename = "dataCenter")]
    #[default]
    DataCenter,
    #[serde(rename = "global")]
    Global,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zone => write!(f, "zone"),
            Self::DataCenter => write!(f, "dataCenter"),
            Self::Global => write!(f, "global"),
        }
    }
}
