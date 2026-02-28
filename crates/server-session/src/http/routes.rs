use axum::{routing::get, Router};
use std::sync::Arc;

use super::handlers;
use crate::server::SessionServerState;
use sofa_registry_core::constants::server_type;
use sofa_registry_server_shared::metrics as srv_metrics;

pub fn create_router(state: Arc<SessionServerState>) -> Router {
    Router::new()
        .route("/api/session/health", get(handlers::health_check))
        .route(
            "/api/session/publishers/count",
            get(handlers::publisher_count),
        )
        .route(
            "/api/session/subscribers/count",
            get(handlers::subscriber_count),
        )
        .route("/api/session/connections", get(handlers::connections))
        .route(
            "/api/session/version",
            get(srv_metrics::version_handler(server_type::SESSION)),
        )
        .with_state(state)
        .merge(srv_metrics::metrics_router())
}
