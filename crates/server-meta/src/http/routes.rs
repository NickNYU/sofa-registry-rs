use axum::{Router, routing::get};
use std::sync::Arc;
use crate::server::MetaServerState;
use super::handlers;
use sofa_registry_server_shared::metrics as srv_metrics;
use sofa_registry_core::constants::server_type;

pub fn create_router(state: Arc<MetaServerState>) -> Router {
    Router::new()
        .route("/api/meta/health", get(handlers::health_check))
        .route("/api/meta/leader", get(handlers::get_leader))
        .route("/api/meta/slot/table", get(handlers::get_slot_table))
        .route("/api/meta/nodes/data", get(handlers::list_data_servers))
        .route("/api/meta/nodes/session", get(handlers::list_session_servers))
        .route("/api/meta/version", get(srv_metrics::version_handler(server_type::META)))
        .with_state(state)
        .merge(srv_metrics::metrics_router())
}
