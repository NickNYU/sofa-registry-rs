use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::data_info::DataInfo;
use super::publisher::Publisher;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DatumVersion {
    pub value: i64,
}

impl DatumVersion {
    pub fn of(value: i64) -> Self {
        Self { value }
    }

    /// Generate a new version based on the current timestamp in milliseconds.
    pub fn next() -> Self {
        Self {
            value: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// A datum holds all publishers for a specific dataInfoId in a data center.
#[derive(Debug, Clone)]
pub struct Datum {
    pub data_info_id: String,
    pub data_center: String,
    pub data_id: String,
    pub instance_id: String,
    pub group: String,
    /// registerId -> Publisher
    pub pub_map: HashMap<String, Publisher>,
    pub version: DatumVersion,
}

impl Datum {
    /// Create an empty datum, parsing data_id, instance_id, and group from data_info_id.
    /// The data_info_id format is "dataId#instanceId#group".
    pub fn new_empty(data_info_id: &str, data_center: &str) -> Self {
        let (data_id, instance_id, group) = DataInfo::parse(data_info_id)
            .unwrap_or_else(|| (data_info_id.to_string(), String::new(), String::new()));

        Self {
            data_info_id: data_info_id.to_string(),
            data_center: data_center.to_string(),
            data_id,
            instance_id,
            group,
            pub_map: HashMap::new(),
            version: DatumVersion::next(),
        }
    }

    pub fn publisher_count(&self) -> usize {
        self.pub_map.len()
    }
}
