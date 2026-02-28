use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router, routing::get};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::Serialize;
use std::sync::OnceLock;

static METRICS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the global Prometheus metrics recorder.
/// Must be called once at process startup before any `metrics::*` macros are used.
/// Returns the handle for rendering the metrics endpoint.
pub fn install_metrics_recorder() -> &'static PrometheusHandle {
    METRICS_HANDLE.get_or_init(|| {
        let builder = PrometheusBuilder::new();
        builder
            .install_recorder()
            .expect("failed to install Prometheus recorder")
    })
}

/// Axum handler for the `/metrics` endpoint.
/// Returns metrics in Prometheus text exposition format.
pub async fn metrics_handler() -> impl IntoResponse {
    match METRICS_HANDLE.get() {
        Some(handle) => (StatusCode::OK, handle.render()),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Metrics recorder not installed".to_string(),
        ),
    }
}

/// Create a stateless router with the `/metrics` Prometheus endpoint.
/// Merge this into your server's stateful router via `.merge()`.
pub fn metrics_router() -> Router {
    Router::new().route("/metrics", get(metrics_handler))
}

/// Shared version response for all server types.
#[derive(Serialize)]
pub struct VersionResponse {
    pub version: &'static str,
    pub server_type: &'static str,
}

impl VersionResponse {
    pub fn new(server_type: &'static str) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            server_type,
        }
    }
}

/// Axum handler that returns a `VersionResponse` JSON for the given server type.
pub fn version_handler(server_type: &'static str) -> impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Json<VersionResponse>> + Send>> + Clone {
    move || {
        let st = server_type;
        Box::pin(async move { Json(VersionResponse::new(st)) })
    }
}

// --- Metric name constants ---

// gRPC
pub const GRPC_REQUESTS_TOTAL: &str = "grpc_requests_total";
pub const GRPC_REQUEST_DURATION_SECONDS: &str = "grpc_request_duration_seconds";

// Session server
pub const SESSION_ACTIVE_PUBLISHERS: &str = "session_active_publishers";
pub const SESSION_ACTIVE_SUBSCRIBERS: &str = "session_active_subscribers";
pub const SESSION_ACTIVE_CONNECTIONS: &str = "session_active_connections";
pub const SESSION_PUSH_TASKS_TOTAL: &str = "session_push_tasks_total";
pub const SESSION_PUSH_TASKS_FAILED: &str = "session_push_tasks_failed";
pub const SESSION_WRITE_FORWARDS_TOTAL: &str = "session_write_forwards_total";
pub const SESSION_WRITE_FORWARDS_FAILED: &str = "session_write_forwards_failed";
pub const SESSION_SLOT_TABLE_EPOCH: &str = "session_slot_table_epoch";
pub const SESSION_ACTIVE_STREAMS: &str = "session_active_streams";

// Data server
pub const DATA_DATUM_COUNT: &str = "data_datum_count";
pub const DATA_SLOT_TABLE_EPOCH: &str = "data_slot_table_epoch";
pub const DATA_ACTIVE_SESSION_LEASES: &str = "data_active_session_leases";
pub const DATA_CHANGES_TOTAL: &str = "data_changes_total";
pub const DATA_CHANGE_NOTIFICATIONS_TOTAL: &str = "data_change_notifications_total";
pub const DATA_CHANGE_NOTIFICATIONS_FAILED: &str = "data_change_notifications_failed";

// Meta server
pub const META_DATA_SERVERS: &str = "meta_data_servers";
pub const META_SESSION_SERVERS: &str = "meta_session_servers";
pub const META_SLOT_TABLE_EPOCH: &str = "meta_slot_table_epoch";
pub const META_IS_LEADER: &str = "meta_is_leader";
pub const META_ELECTIONS_TOTAL: &str = "meta_elections_total";
pub const META_LEASE_EVICTIONS_TOTAL: &str = "meta_lease_evictions_total";
