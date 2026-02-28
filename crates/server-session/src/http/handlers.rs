use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::server::SessionServerState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub publisher_count: usize,
    pub subscriber_count: usize,
    pub connection_count: usize,
}

pub async fn health_check(
    State(state): State<Arc<SessionServerState>>,
) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "UP".to_string(),
        publisher_count: state.publisher_registry.count(),
        subscriber_count: state.subscriber_registry.count(),
        connection_count: state.connection_service.count(),
    })
}

#[derive(Serialize)]
pub struct CountResponse {
    pub count: usize,
    pub data_info_id_count: usize,
}

pub async fn publisher_count(
    State(state): State<Arc<SessionServerState>>,
) -> Json<CountResponse> {
    Json(CountResponse {
        count: state.publisher_registry.count(),
        data_info_id_count: state.publisher_registry.data_info_id_count(),
    })
}

pub async fn subscriber_count(
    State(state): State<Arc<SessionServerState>>,
) -> Json<CountResponse> {
    Json(CountResponse {
        count: state.subscriber_registry.count(),
        data_info_id_count: state.subscriber_registry.data_info_id_count(),
    })
}

#[derive(Serialize)]
pub struct ConnectionResponse {
    pub client_id: String,
    pub address: String,
    pub connected_at: i64,
}

#[derive(Serialize)]
pub struct ConnectionsResponse {
    pub connections: Vec<ConnectionResponse>,
    pub count: usize,
}

pub async fn connections(
    State(state): State<Arc<SessionServerState>>,
) -> Json<ConnectionsResponse> {
    let all = state.connection_service.get_all();
    let count = all.len();
    let connections = all
        .into_iter()
        .map(|c| ConnectionResponse {
            client_id: c.client_id,
            address: c.address,
            connected_at: c.connected_at,
        })
        .collect();
    Json(ConnectionsResponse { connections, count })
}

