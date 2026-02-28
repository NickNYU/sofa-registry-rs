use std::collections::HashMap;

use super::connect_id::ConnectId;
use super::process_id::ProcessId;
use super::publish_type::{PublishSource, PublishType};
use super::register_version::RegisterVersion;
use super::server_data_box::ServerDataBox;

/// Server-side publisher stored in data server.
#[derive(Debug, Clone)]
pub struct Publisher {
    pub data_info_id: String,
    pub data_id: String,
    pub instance_id: String,
    pub group: String,
    pub regist_id: String,
    pub client_id: String,
    pub cell: Option<String>,
    pub app_name: Option<String>,
    pub process_id: ProcessId,
    pub version: RegisterVersion,
    pub source_address: ConnectId,
    pub session_process_id: ProcessId,
    pub data_list: Vec<ServerDataBox>,
    pub publish_type: PublishType,
    pub publish_source: PublishSource,
    pub attributes: HashMap<String, String>,
    pub register_timestamp: i64,
}
