use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub success: bool,

    #[serde(rename = "registId")]
    pub regist_id: Option<String>,

    pub version: i64,

    pub refused: bool,

    pub message: Option<String>,
}

impl RegisterResponse {
    pub fn ok(regist_id: &str, version: i64) -> Self {
        Self {
            success: true,
            regist_id: Some(regist_id.to_string()),
            version,
            refused: false,
            message: None,
        }
    }

    pub fn failed(msg: &str) -> Self {
        Self {
            success: false,
            regist_id: None,
            version: 0,
            refused: false,
            message: Some(msg.to_string()),
        }
    }

    pub fn refused(msg: &str) -> Self {
        Self {
            success: false,
            regist_id: None,
            version: 0,
            refused: true,
            message: Some(msg.to_string()),
        }
    }
}
