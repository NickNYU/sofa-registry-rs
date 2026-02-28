use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataBox {
    pub data: Option<String>,
}

impl DataBox {
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: Some(data.into()),
        }
    }

    pub fn empty() -> Self {
        Self { data: None }
    }
}
