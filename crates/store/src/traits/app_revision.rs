use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sofa_registry_core::error::Result;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRevision {
    pub data_center: String,
    pub revision: String,
    pub app_name: String,
    pub base_params: HashMap<String, String>,
    pub service_params: HashMap<String, HashMap<String, String>>,
    pub deleted: bool,
    pub gmt_create: DateTime<Utc>,
    pub gmt_modified: DateTime<Utc>,
}

#[async_trait]
pub trait AppRevisionRepository: Send + Sync {
    async fn register(&self, revision: AppRevision) -> Result<()>;

    async fn query_revision(&self, revision: &str) -> Result<Option<AppRevision>>;

    async fn heartbeat(&self, revision: &str) -> Result<bool>;

    async fn get_expired(&self, before: DateTime<Utc>, limit: i32) -> Result<Vec<AppRevision>>;

    async fn clean_deleted(&self, before: DateTime<Utc>, limit: i32) -> Result<i32>;
}
