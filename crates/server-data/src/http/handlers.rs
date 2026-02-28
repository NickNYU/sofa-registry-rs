use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::DataServerState;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    server_type: String,
    uptime_ms: i64,
    slot_table_epoch: i64,
    leader_slots: usize,
    follower_slots: usize,
}

pub async fn health(State(state): State<Arc<DataServerState>>) -> impl IntoResponse {
    let uptime_ms = chrono::Utc::now().timestamp_millis() - state.start_time;
    let leader_count = state.slot_manager.my_leader_slots().len();
    let follower_count = state.slot_manager.my_follower_slots().len();
    let epoch = state.slot_manager.get_slot_table_epoch();

    Json(HealthResponse {
        status: "UP".to_string(),
        server_type: sofa_registry_core::constants::server_type::DATA.to_string(),
        uptime_ms,
        slot_table_epoch: epoch,
        leader_slots: leader_count,
        follower_slots: follower_count,
    })
}

#[derive(Serialize)]
struct DatumCountResponse {
    data_center: String,
    datum_count: usize,
    publisher_count: usize,
}

pub async fn datum_count(State(state): State<Arc<DataServerState>>) -> impl IntoResponse {
    let dc = &state.config.data_center;
    let datum_count = state.storage.datum_count(dc);
    let publisher_count = state.storage.publisher_count(dc);

    Json(DatumCountResponse {
        data_center: dc.clone(),
        datum_count,
        publisher_count,
    })
}

pub async fn slot_table(State(state): State<Arc<DataServerState>>) -> impl IntoResponse {
    let table = state.slot_manager.get_slot_table();
    Json(table)
}

#[derive(Deserialize)]
pub struct PublishersQuery {
    #[serde(rename = "dataInfoId")]
    pub data_info_id: Option<String>,
}

#[derive(Serialize)]
struct PublishersResponse {
    data_info_id: String,
    publisher_count: usize,
    publishers: Vec<PublisherInfo>,
}

#[derive(Serialize)]
struct PublisherInfo {
    regist_id: String,
    client_id: String,
    app_name: Option<String>,
    source_address: String,
    register_timestamp: i64,
}

pub async fn publishers(
    State(state): State<Arc<DataServerState>>,
    Query(query): Query<PublishersQuery>,
) -> impl IntoResponse {
    let dc = &state.config.data_center;

    match query.data_info_id {
        Some(data_info_id) => {
            let pubs = state.storage.get_publishers(dc, &data_info_id);
            let publishers: Vec<PublisherInfo> = pubs
                .values()
                .map(|p| PublisherInfo {
                    regist_id: p.regist_id.clone(),
                    client_id: p.client_id.clone(),
                    app_name: p.app_name.clone(),
                    source_address: p.source_address.to_string(),
                    register_timestamp: p.register_timestamp,
                })
                .collect();
            let count = publishers.len();
            Json(PublishersResponse {
                data_info_id,
                publisher_count: count,
                publishers,
            })
        }
        None => Json(PublishersResponse {
            data_info_id: String::new(),
            publisher_count: 0,
            publishers: vec![],
        }),
    }
}
