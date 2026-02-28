use axum::{routing::get, Router};
use std::sync::Arc;

use super::handlers;
use crate::server::DataServerState;
use sofa_registry_core::constants::server_type;
use sofa_registry_server_shared::metrics as srv_metrics;

/// Build the HTTP router for the Data server admin API.
pub fn create_router(state: Arc<DataServerState>) -> Router {
    Router::new()
        .route("/api/data/health", get(handlers::health))
        .route("/api/data/datum/count", get(handlers::datum_count))
        .route("/api/data/slot/table", get(handlers::slot_table))
        .route("/api/data/publishers", get(handlers::publishers))
        .route(
            "/api/data/version",
            get(srv_metrics::version_handler(server_type::DATA)),
        )
        .with_state(state)
        .merge(srv_metrics::metrics_router())
}
