use axum::{response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub server_type: String,
    pub uptime_ms: i64,
    pub leader: Option<String>,
}

impl HealthStatus {
    pub fn healthy(server_type: &str, uptime_ms: i64) -> Self {
        Self {
            status: "UP".to_string(),
            server_type: server_type.to_string(),
            uptime_ms,
            leader: None,
        }
    }

    pub fn unhealthy(server_type: &str, reason: &str) -> Self {
        Self {
            status: format!("DOWN: {}", reason),
            server_type: server_type.to_string(),
            uptime_ms: 0,
            leader: None,
        }
    }
}

pub async fn health_handler() -> impl IntoResponse {
    Json(HealthStatus::healthy("unknown", 0))
}
