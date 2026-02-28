use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::server::SessionServerState;

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    status: String,
    publisher_count: usize,
    subscriber_count: usize,
    connection_count: usize,
}

pub(crate) async fn health_check(
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
pub(crate) struct CountResponse {
    count: usize,
    data_info_id_count: usize,
}

pub(crate) async fn publisher_count(
    State(state): State<Arc<SessionServerState>>,
) -> Json<CountResponse> {
    Json(CountResponse {
        count: state.publisher_registry.count(),
        data_info_id_count: state.publisher_registry.data_info_id_count(),
    })
}

pub(crate) async fn subscriber_count(
    State(state): State<Arc<SessionServerState>>,
) -> Json<CountResponse> {
    Json(CountResponse {
        count: state.subscriber_registry.count(),
        data_info_id_count: state.subscriber_registry.data_info_id_count(),
    })
}

#[derive(Serialize)]
pub(crate) struct ConnectionResponse {
    client_id: String,
    address: String,
    connected_at: i64,
}

#[derive(Serialize)]
pub(crate) struct ConnectionsResponse {
    connections: Vec<ConnectionResponse>,
    count: usize,
}

pub(crate) async fn connections(
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
