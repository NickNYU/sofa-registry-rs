use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Base registration information shared by publishers and subscribers.
/// Translated from Java `com.alipay.sofa.registry.core.model.BaseRegister`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BaseRegister {
    #[serde(default, rename = "instanceId")]
    pub instance_id: Option<String>,

    #[serde(default)]
    pub zone: Option<String>,

    #[serde(default, rename = "appName")]
    pub app_name: Option<String>,

    #[serde(default, rename = "dataId")]
    pub data_id: Option<String>,

    #[serde(default)]
    pub group: Option<String>,

    #[serde(default, rename = "processId")]
    pub process_id: Option<String>,

    #[serde(default, rename = "registId")]
    pub regist_id: Option<String>,

    #[serde(default, rename = "clientId")]
    pub client_id: Option<String>,

    #[serde(default, rename = "dataInfoId")]
    pub data_info_id: Option<String>,

    #[serde(default)]
    pub ip: Option<String>,

    #[serde(default)]
    pub port: Option<u16>,

    #[serde(default, rename = "eventType")]
    pub event_type: Option<String>,

    #[serde(default)]
    pub version: Option<i64>,

    #[serde(default)]
    pub timestamp: Option<i64>,

    #[serde(default)]
    pub attributes: HashMap<String, String>,
}
