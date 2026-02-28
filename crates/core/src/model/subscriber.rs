use super::connect_id::ConnectId;
use super::process_id::ProcessId;
use super::scope::Scope;

/// Server-side subscriber.
#[derive(Debug, Clone)]
pub struct Subscriber {
    pub data_info_id: String,
    pub data_id: String,
    pub instance_id: String,
    pub group: String,
    pub regist_id: String,
    pub client_id: String,
    pub scope: Scope,
    pub cell: Option<String>,
    pub app_name: Option<String>,
    pub process_id: ProcessId,
    pub source_address: ConnectId,
    pub accept_encoding: Option<String>,
    pub accept_multi: bool,
    pub register_timestamp: i64,
}
