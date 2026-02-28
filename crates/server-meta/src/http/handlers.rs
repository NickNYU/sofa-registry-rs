use axum::{Json, extract::State};
use serde::Serialize;
use std::sync::Arc;
use crate::server::MetaServerState;
use sofa_registry_store::traits::leader_elector::LeaderElector;
use sofa_registry_core::slot::SlotTable;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub leader: Option<String>,
    pub epoch: i64,
    pub data_server_count: usize,
    pub session_server_count: usize,
}

#[derive(Serialize)]
pub struct LeaderResponse {
    pub leader: Option<String>,
    pub epoch: i64,
    pub am_i_leader: bool,
}

#[derive(Serialize)]
pub struct NodeListResponse {
    pub nodes: Vec<String>,
    pub count: usize,
}

pub async fn health_check(
    State(state): State<Arc<MetaServerState>>,
) -> Json<HealthResponse> {
    let leader_info = state.leader_elector.get_leader_info();
    Json(HealthResponse {
        status: if leader_info.is_valid() { "UP".to_string() } else { "DOWN".to_string() },
        leader: leader_info.leader,
        epoch: leader_info.epoch,
        data_server_count: state.data_server_manager.count(),
        session_server_count: state.session_server_manager.count(),
    })
}

pub async fn get_leader(
    State(state): State<Arc<MetaServerState>>,
) -> Json<LeaderResponse> {
    let leader_info = state.leader_elector.get_leader_info();
    Json(LeaderResponse {
        leader: leader_info.leader,
        epoch: leader_info.epoch,
        am_i_leader: state.leader_elector.am_i_leader(),
    })
}

pub async fn get_slot_table(
    State(state): State<Arc<MetaServerState>>,
) -> Json<SlotTable> {
    Json(state.slot_manager.get_slot_table())
}

pub async fn list_data_servers(
    State(state): State<Arc<MetaServerState>>,
) -> Json<NodeListResponse> {
    let nodes = state.data_server_manager.get_data_server_addresses();
    let count = nodes.len();
    Json(NodeListResponse { nodes, count })
}

pub async fn list_session_servers(
    State(state): State<Arc<MetaServerState>>,
) -> Json<NodeListResponse> {
    let nodes = state.session_server_manager.get_session_server_addresses();
    let count = nodes.len();
    Json(NodeListResponse { nodes, count })
}

