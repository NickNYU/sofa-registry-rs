use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::data_box::DataBox;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedData {
    #[serde(rename = "dataId")]
    pub data_id: String,

    pub group: String,

    #[serde(rename = "instanceId")]
    pub instance_id: String,

    pub segment: Option<String>,

    pub scope: Option<String>,

    #[serde(rename = "subscriberRegistIds", default)]
    pub subscriber_regist_ids: Vec<String>,

    #[serde(default)]
    pub data: HashMap<String, Vec<DataBox>>,

    pub version: Option<i64>,

    #[serde(rename = "localZone")]
    pub local_zone: Option<String>,

    #[serde(rename = "dataCount", default)]
    pub data_count: HashMap<String, u32>,
}
